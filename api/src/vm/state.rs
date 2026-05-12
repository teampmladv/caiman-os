//! vm/state.rs -- VM state persisted to /var/lib/caiman/vms/{id}/state.json

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::{Context, Result};

pub const STATE_DIR: &str = "/var/lib/caiman/vms";
pub const BASE_IMAGES_DIR: &str = "/var/lib/caiman/base-images";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmStatus {
    Active,
    Stopped,
    Booting,
    Migrating,
    Error,
    ShutOff,
}

impl std::fmt::Display for VmStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmStatus::Active    => write!(f, "ACTIVE"),
            VmStatus::Stopped   => write!(f, "STOPPED"),
            VmStatus::Booting   => write!(f, "BOOTING"),
            VmStatus::Migrating => write!(f, "MIGRATING"),
            VmStatus::Error     => write!(f, "ERROR"),
            VmStatus::ShutOff   => write!(f, "SHUT_OFF"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmState {
    // Identity
    pub id:              String,
    pub uuid:            String,
    pub name:            String,
    pub status:          VmStatus,

    // Runtime
    pub pid:             Option<u32>,
    pub power_state:     String,
    pub task_state:      Option<String>,

    // Compute
    pub cpus:            u8,
    pub mem_mib:         u64,
    pub flavor:          Option<String>,

    // Storage
    pub base_image:      Option<String>,
    pub disk_path:       Option<String>,
    pub disk_size_gib:   Option<u64>,
    pub volumes:         Vec<String>,

    // Network
    pub ip:              Option<String>,
    pub mac:             String,
    pub uplink:          String,
    pub tap:             Option<String>,
    pub net_mode:        Option<String>,

    // Host
    pub node_name:       String,
    /// caiman-vmm
    pub hypervisor:      String,
    pub zone: String,
    pub autostart:       bool,

    // Console
    pub pty:             Option<String>,
    pub console_log:     Option<String>,

    // Metadata
    pub kernel:          String,
    pub initrd:          Option<String>,
    pub labels:          std::collections::HashMap<String, String>,
    pub project_id:      Option<String>,
    pub user_id:         Option<String>,
    pub security_groups: Vec<String>,

    // Timestamps
    pub created_at:      DateTime<Utc>,
    pub started_at:      Option<DateTime<Utc>>,
    pub launched_at:     Option<DateTime<Utc>>,
    pub terminated_at:   Option<DateTime<Utc>>,

    // Live metrics
    #[serde(default)]
    pub cpu_usage_pct:   f64,
    #[serde(default)]
    pub mem_used_mib:    u64,
    #[serde(default)]
    pub net_rx_mbps:     f64,
    #[serde(default)]
    pub net_tx_mbps:     f64,
    #[serde(default)]
    pub uptime_secs:     u64,
}

impl VmState {
    fn dir(id: &str) -> PathBuf {
        PathBuf::from(STATE_DIR).join(id)
    }

    fn path(id: &str) -> PathBuf {
        Self::dir(id).join("state.json")
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::dir(&self.id);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(Self::path(&self.id), json)
            .with_context(|| format!("writing state for {}", self.id))
    }

    pub fn load(id: &str) -> Result<Self> {
        let data = std::fs::read_to_string(Self::path(id))
            .with_context(|| format!("reading state for {id}"))?;
        serde_json::from_str(&data)
            .with_context(|| format!("parsing state for {id}"))
    }

    pub fn delete(id: &str) -> Result<()> {
        let dir = Self::dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn list_all() -> Vec<Self> {
        let dir = Path::new(STATE_DIR);
        if !dir.exists() { return vec![]; }
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let id = e.file_name().to_str()?.to_string();
                match Self::load(&id) {
                    Ok(mut v) => { v.reconcile(); Some(v) }
                    Err(e) => { eprintln!("[VmState] failed to load {id}: {e}"); None }
                }
            })
            .collect()
    }

    pub fn is_alive(&self) -> bool {
        match self.pid {
            None => false,
            Some(pid) => Path::new(&format!("/proc/{pid}")).exists()
        }
    }

    pub fn reconcile(&mut self) {
        if self.status == VmStatus::Active && !self.is_alive() {
            self.status = VmStatus::ShutOff;
            self.power_state = "Stopped".to_string();
            self.terminated_at = Some(Utc::now());
            let _ = self.save();
        }
    }

    /// Create VM directory and clone base image
    pub fn create_disk(id: &str, base_image: &str) -> Result<String> {
        let dir = Self::dir(id);
        std::fs::create_dir_all(&dir)?;
        let disk_path = dir.join("disk.img");
        let base = PathBuf::from(BASE_IMAGES_DIR).join(base_image);

        // Try reflink (CoW) first, fall back to copy
        let status = std::process::Command::new("cp")
            .args(["--reflink=auto", base.to_str().unwrap(), disk_path.to_str().unwrap()])
            .status();

        match status {
            Ok(s) if s.success() => Ok(disk_path.to_str().unwrap().to_string()),
            _ => {
                // Fallback: regular copy
                std::fs::copy(&base, &disk_path)
                    .with_context(|| format!("copying base image {base_image}"))?;
                Ok(disk_path.to_str().unwrap().to_string())
            }
        }
    }
}
