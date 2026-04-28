//! vm/state.rs — VM state persisted to /var/run/caiman/{id}.json
//!
//! Each running VM has a JSON file on disk that tracks its state.
//! The API reads all *.json files to build the VM list.
//! caiman-vmm writes its PID and status to this file on startup.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use anyhow::{Context, Result};

pub const STATE_DIR: &str = "/var/run/caiman";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmStatus {
    Running,
    Stopped,
    Booting,
    Migrating,
    Error,
}

impl std::fmt::Display for VmStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmStatus::Running   => write!(f, "RUNNING"),
            VmStatus::Stopped   => write!(f, "STOPPED"),
            VmStatus::Booting   => write!(f, "BOOTING"),
            VmStatus::Migrating => write!(f, "MIGRATING"),
            VmStatus::Error     => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmState {
    pub id:          String,
    pub name:        String,
    pub status:      VmStatus,
    pub pid:         Option<u32>,
    pub cpus:        u8,
    pub mem_mib:     u64,
    pub node_name:   String,
    pub kernel:      String,
    pub disk:        Option<String>,
    pub mac:         String,
    pub uplink:      String,
    pub labels:      std::collections::HashMap<String, String>,
    pub created_at:  DateTime<Utc>,
    pub started_at:  Option<DateTime<Utc>>,

    // Live metrics (updated by collector)
    #[serde(default)]
    pub cpu_usage_pct:  f64,
    #[serde(default)]
    pub mem_used_mib:   u64,
    #[serde(default)]
    pub net_rx_mbps:    f64,
    #[serde(default)]
    pub net_tx_mbps:    f64,
    #[serde(default)]
    pub uptime_secs:    u64,
}

impl VmState {
    fn path(id: &str) -> PathBuf {
        PathBuf::from(STATE_DIR).join(format!("{id}.json"))
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(STATE_DIR)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(Self::path(&self.id), json)
            .with_context(|| format!("writing {}.json", self.id))
    }

    pub fn load(id: &str) -> Result<Self> {
        let data = std::fs::read_to_string(Self::path(id))
            .with_context(|| format!("reading {id}.json"))?;
        serde_json::from_str(&data)
            .with_context(|| format!("parsing {id}.json"))
    }

    pub fn delete(id: &str) -> Result<()> {
        let path = Self::path(id);
        if path.exists() { std::fs::remove_file(path)?; }
        Ok(())
    }

    pub fn list_all() -> Vec<Self> {
        let dir = Path::new(STATE_DIR);
        if !dir.exists() { return vec![]; }
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .filter_map(|e| {
                let id = e.path().file_stem()?.to_str()?.to_string();
                Self::load(&id).ok()
            })
            .collect()
    }

    /// Check if the VM process is actually alive
    pub fn is_alive(&self) -> bool {
        match self.pid {
            None => false,
            Some(pid) => {
                // /proc/{pid} exists iff process is running
                Path::new(&format!("/proc/{pid}")).exists()
            }
        }
    }

    /// Reconcile: mark as STOPPED if process died
    pub fn reconcile(&mut self) {
        if self.status == VmStatus::Running && !self.is_alive() {
            self.status = VmStatus::Stopped;
            self.pid    = None;
            let _ = self.save();
        }
    }
}
