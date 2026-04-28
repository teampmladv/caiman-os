//! livemig/src/proto.rs — migration wire protocol
//!
//! Binary protocol over TCP port 7777:
//!
//!   [4 bytes: msg_type] [4 bytes: payload_len] [payload_len bytes: payload]
//!
//! Message types:
//!   HELLO      → migration handshake (JSON config)
//!   READY      ← destination ready to receive
//!   PAGE       → guest memory page (GPA + 4096 bytes data)
//!   PAUSE      → source VM paused, final transfer starting
//!   VCPU_STATE → vCPU register state (JSON)
//!   DONE       → all state transferred, start the VM
//!   RUNNING    ← destination VM is running
//!   ERROR      ← something went wrong

use std::net::SocketAddr;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

pub const MPORT: u16 = 7777;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MsgType {
    Hello     = 1,
    Ready     = 2,
    Page      = 3,
    Pause     = 4,
    VcpuState = 5,
    Done      = 6,
    Running   = 7,
    Error     = 8,
}

impl TryFrom<u32> for MsgType {
    type Error = anyhow::Error;
    fn try_from(v: u32) -> Result<Self> {
        Ok(match v {
            1 => Self::Hello, 2 => Self::Ready, 3 => Self::Page,
            4 => Self::Pause, 5 => Self::VcpuState, 6 => Self::Done,
            7 => Self::Running, 8 => Self::Error,
            _ => anyhow::bail!("unknown msg type {v}"),
        })
    }
}

/// A page message payload
pub struct PageMsg {
    pub gpa:  u64,
    pub data: Vec<u8>, // 4096 bytes
}

/// Wraps a TCP stream with our migration protocol
pub struct MigStream {
    inner: TcpStream,
}

impl MigStream {
    pub fn new(stream: TcpStream) -> Self {
        Self { inner: stream }
    }

    // ── Send helpers ──────────────────────────────────────────────────────

    async fn send_msg(&mut self, msg_type: MsgType, payload: &[u8]) -> Result<()> {
        let header = [
            (msg_type as u32).to_be_bytes(),
            (payload.len() as u32).to_be_bytes(),
        ].concat();
        self.inner.write_all(&header).await?;
        self.inner.write_all(payload).await?;
        Ok(())
    }

    pub async fn send_hello(&mut self, config: &serde_json::Value) -> Result<()> {
        let json = serde_json::to_vec(config)?;
        self.send_msg(MsgType::Hello, &json).await
    }

    pub async fn send_ready(&mut self) -> Result<()> {
        self.send_msg(MsgType::Ready, b"").await
    }

    pub async fn send_page(&mut self, gpa: u64, data: &[u8]) -> Result<()> {
        let mut payload = gpa.to_be_bytes().to_vec();
        payload.extend_from_slice(data);
        self.send_msg(MsgType::Page, &payload).await
    }

    pub async fn send_pause(&mut self) -> Result<()> {
        self.send_msg(MsgType::Pause, b"").await
    }

    pub async fn send_vcpu_state(&mut self, state: &serde_json::Value) -> Result<()> {
        let json = serde_json::to_vec(state)?;
        self.send_msg(MsgType::VcpuState, &json).await
    }

    pub async fn send_done(&mut self) -> Result<()> {
        self.send_msg(MsgType::Done, b"").await
    }

    pub async fn send_running(&mut self) -> Result<()> {
        self.send_msg(MsgType::Running, b"").await
    }

    // ── Receive helpers ───────────────────────────────────────────────────

    pub async fn recv_msg(&mut self) -> Result<(MsgType, Vec<u8>)> {
        let mut header = [0u8; 8];
        self.inner.read_exact(&mut header).await
            .context("reading msg header")?;

        let msg_type = u32::from_be_bytes(header[0..4].try_into().unwrap());
        let payload_len = u32::from_be_bytes(header[4..8].try_into().unwrap());

        let mut payload = vec![0u8; payload_len as usize];
        if payload_len > 0 {
            self.inner.read_exact(&mut payload).await
                .context("reading msg payload")?;
        }

        let t = MsgType::try_from(msg_type)?;
        debug!("recv {:?} len={}", t, payload_len);
        Ok((t, payload))
    }

    pub async fn expect(&mut self, expected: MsgType) -> Result<Vec<u8>> {
        let (t, payload) = self.recv_msg().await?;
        anyhow::ensure!(t == expected,
            "expected {:?} but got {:?}", expected, t);
        Ok(payload)
    }
}

/// Connect to destination migration server
pub async fn connect(dest: &str) -> Result<MigStream> {
    let addr: SocketAddr = format!("{dest}:{MPORT}").parse()
        .with_context(|| format!("parsing destination address {dest}:{MPORT}"))?;
    let stream = TcpStream::connect(addr).await
        .with_context(|| format!("connecting to {addr}"))?;
    info!("Connected to migration server {addr}");
    Ok(MigStream::new(stream))
}

/// Listen for incoming migration connections
pub async fn listen() -> Result<TcpListener> {
    let listener = TcpListener::bind(format!("0.0.0.0:{MPORT}")).await
        .with_context(|| format!("binding migration port {MPORT}"))?;
    info!("Migration server listening on :{MPORT}");
    Ok(listener)
}
