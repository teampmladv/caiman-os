// caiman-bridge acquisition: pull a guest disk off a Proxmox (PVE) host over
// SSH and stream it into a local staging file, agentless.
//
// v1 scope: directory-backed storage, where the disk is a qcow2 (or raw) file
// on the PVE host. We stream it byte-for-byte with `cat` over an SSH exec
// channel. Block-backed storage (LVM-thin / ZFS / Ceph) is a later step: the
// only change is the remote command (qemu-img convert on the source), the
// streaming machinery here is identical.
//
// Agentless: nothing is installed on the PVE host; we use the SSH access the
// operator already has. The qcow2 conversion happens afterwards on the Caiman
// side via crate::qcow2, which needs a seekable file -- hence we land the
// source to a staging file here rather than converting a live stream.
//
// API note: written against the current russh API (native async traits,
// russh::keys re-export, PrivateKeyWithHashAlg). If `cargo add russh` pulls a
// version whose surface differs, the connect/auth block and the ExtendedData
// match arm are where to adjust.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use russh::keys::*;
use russh::*;
use tokio::io::AsyncWriteExt;

// Cap stderr capture so a misbehaving remote command cannot grow it without
// bound.
const STDERR_CAPTURE_CAP: usize = 8 * 1024;

pub enum SshAuth {
    Key {
        path: PathBuf,
        passphrase: Option<String>,
    },
    Password(String),
}

pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    // Pin the host key fingerprint to prevent man-in-the-middle. None accepts
    // any key (convenient in a lab, unsafe for moving production data). See
    // the handler below: host-key verification is deliberately left as a
    // conscious decision rather than silently hard-coded.
    pub expected_host_fingerprint: Option<String>,
}

#[derive(Debug)]
pub enum AcquireError {
    Io(std::io::Error),
    Ssh(russh::Error),
    Key(String),
    AuthFailed,
    RemoteCommandFailed { code: u32, stderr: String },
}

impl From<std::io::Error> for AcquireError {
    fn from(e: std::io::Error) -> Self {
        AcquireError::Io(e)
    }
}

impl From<russh::Error> for AcquireError {
    fn from(e: russh::Error) -> Self {
        AcquireError::Ssh(e)
    }
}

impl std::fmt::Display for AcquireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcquireError::Io(e) => write!(f, "io error: {}", e),
            AcquireError::Ssh(e) => write!(f, "ssh error: {}", e),
            AcquireError::Key(e) => write!(f, "key error: {}", e),
            AcquireError::AuthFailed => write!(f, "ssh authentication failed"),
            AcquireError::RemoteCommandFailed { code, stderr } => {
                write!(f, "remote command failed (code {}): {}", code, stderr)
            }
        }
    }
}

impl std::error::Error for AcquireError {}

type Result<T> = std::result::Result<T, AcquireError>;

// SSH client event handler.
//
// SECURITY: check_server_key currently accepts any host key. This is a known
// gap, left explicit on purpose: moving production disk images over a channel
// with no host-key verification is MITM-exposed. Before this tool touches
// real data, verify expected_host_fingerprint here (compare against the
// server key's SHA256 fingerprint) and return Ok(false) on mismatch.
struct ClientHandler {
    _expected_fingerprint: Option<String>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TODO(security): if self._expected_fingerprint is Some, compute the
        // server key fingerprint and return Ok(got == expected); reject
        // otherwise. Accept-any is for the lab only.
        Ok(true)
    }
}

// Wrap a path in single quotes for safe use in a remote shell command,
// escaping any embedded single quotes.
fn shell_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

// Connect to the PVE host, run `cat <remote_path>`, and stream stdout into
// local_staging. Returns the number of bytes written.
pub async fn acquire_disk(
    target: &SshTarget,
    remote_path: &str,
    local_staging: &Path,
) -> Result<u64> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    });

    let handler = ClientHandler {
        _expected_fingerprint: target.expected_host_fingerprint.clone(),
    };

    let mut session =
        client::connect(config, (target.host.as_str(), target.port), handler).await?;

    match &target.auth {
        SshAuth::Key { path, passphrase } => {
            let key = load_secret_key(path, passphrase.as_deref())
                .map_err(|e| AcquireError::Key(e.to_string()))?;
            let auth = session
                .authenticate_publickey(
                    target.user.as_str(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(key),
                        session.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await?;
            if !auth.success() {
                return Err(AcquireError::AuthFailed);
            }
        }
        SshAuth::Password(pw) => {
            let auth = session
                .authenticate_password(target.user.as_str(), pw.as_str())
                .await?;
            if !auth.success() {
                return Err(AcquireError::AuthFailed);
            }
        }
    }

    let mut channel = session.channel_open_session().await?;
    let command = format!("cat {}", shell_single_quote(remote_path));
    channel.exec(true, command.as_str()).await?;

    if let Some(parent) = local_staging.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut out = tokio::fs::File::create(local_staging).await?;

    let mut written: u64 = 0;
    let mut stderr_buf: Vec<u8> = Vec::new();
    let mut exit_code: Option<u32> = None;

    loop {
        let Some(msg) = channel.wait().await else {
            break;
        };
        match msg {
            ChannelMsg::Data { ref data } => {
                out.write_all(data).await?;
                written += data.len() as u64;
            }
            ChannelMsg::ExtendedData { ref data, ext } => {
                // ext == 1 is stderr.
                if ext == 1 && stderr_buf.len() < STDERR_CAPTURE_CAP {
                    let room = STDERR_CAPTURE_CAP - stderr_buf.len();
                    let take = std::cmp::min(room, data.len());
                    stderr_buf.extend_from_slice(&data[..take]);
                }
            }
            ChannelMsg::ExitStatus { exit_status } => {
                exit_code = Some(exit_status);
                // Do not break: more data may still be in flight.
            }
            _ => {}
        }
    }

    out.flush().await?;

    match exit_code {
        Some(0) => Ok(written),
        Some(code) => Err(AcquireError::RemoteCommandFailed {
            code,
            stderr: String::from_utf8_lossy(&stderr_buf).into_owned(),
        }),
        None => Err(AcquireError::RemoteCommandFailed {
            code: u32::MAX,
            stderr: String::from_utf8_lossy(&stderr_buf).into_owned(),
        }),
    }
}
