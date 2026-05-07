//! virtio/tap.rs -- Linux TUN/TAP interface
//!
//! Creates a TAP interface on the host. The VMM reads packets the guest
//! sends via virtio-net TX queue and writes them to the TAP fd.
//! Packets arriving on the TAP fd are injected into the guest via the RX queue.
//!
//! Usage:
//!   let tap = Tap::new("caiman0")?;
//!   tap.set_ip("10.0.0.1/24")?;
//!   tap.up()?;

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use anyhow::{Context, Result};
use tracing::info;

const TUNSETIFF:  u64 = 0x400454CA;
const IFF_TAP:    i16 = 0x0002;
const IFF_NO_PI:  i16 = 0x1000;

#[repr(C)]
struct Ifreq {
    ifr_name:  [u8; 16],
    ifr_flags: i16,
    _pad:      [u8; 22],
}

pub struct Tap {
    file: File,
    name: String,
}

impl Tap {
    /// Create (or open existing) TAP interface with the given name.
    pub fn new(name: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/net/tun")
            .context("/dev/net/tun -- is TUN/TAP module loaded?")?;

        let mut ifr = Ifreq {
            ifr_name:  [0u8; 16],
            ifr_flags: IFF_TAP | IFF_NO_PI,
            _pad:      [0u8; 22],
        };
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(15);
        ifr.ifr_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        let rc = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETIFF, &ifr as *const _) };
        if rc < 0 {
            anyhow::bail!("TUNSETIFF failed: {}", std::io::Error::last_os_error());
        }

        let actual_name = std::str::from_utf8(&ifr.ifr_name)
            .unwrap_or(name)
            .trim_end_matches('\0')
            .to_string();

        // Set non-blocking
        unsafe {
            let flags = libc::fcntl(file.as_raw_fd(), libc::F_GETFL, 0);
            libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        info!("TAP interface '{}' created (fd={})", actual_name, file.as_raw_fd());
        Ok(Self { file, name: actual_name })
    }

    /// Bring the TAP interface up and optionally assign an IP.
    pub fn up(&self) -> Result<()> {
        std::process::Command::new("ip")
            .args(["link", "set", &self.name, "up"])
            .status()
            .context("ip link set up")?;
        info!("TAP '{}' is up", self.name);
        Ok(())
    }

    /// Assign an IP address (e.g. "10.0.0.1/24").
    pub fn set_ip(&self, cidr: &str) -> Result<()> {
        std::process::Command::new("ip")
            .args(["addr", "add", cidr, "dev", &self.name])
            .status()
            .context("ip addr add")?;
        info!("TAP '{}' addr {}", self.name, cidr);
        Ok(())
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn as_raw_fd(&self) -> RawFd { self.file.as_raw_fd() }

    /// Read one packet from the TAP (non-blocking, returns None if no data).
    pub fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        match self.file.read(buf) {
            Ok(n) if n > 0 => Some(n),
            _ => None,
        }
    }

    /// Write one packet to the TAP.
    pub fn send(&mut self, buf: &[u8]) -> Result<()> {
        self.file.write_all(buf).context("TAP write")
    }
}
