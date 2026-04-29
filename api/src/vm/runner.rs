//! vm/runner.rs — spawn and manage caiman-vmm processes
//!
//! Each VM runs as a separate caiman-vmm process.
//! The API spawns it, tracks the PID, and can stop it via signals.

use std::collections::HashMap;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, warn};
use chrono::Utc;
use uuid::Uuid;

use super::state::{VmState, VmStatus, STATE_DIR};

pub const VMM_BINARY: &str = "caiman-vmm";

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVmRequest {
    pub name:      String,
    pub cpus:      Option<u8>,
    pub mem_mib:   Option<u64>,
    pub kernel:    Option<String>,
    pub disk:      Option<String>,
    pub uplink:    Option<String>,
    pub cmdline:   Option<String>,
    pub labels:    Option<HashMap<String, String>>,
}

/// Spawn caiman-vmm and return the VM state with PID
pub async fn spawn_vm(req: CreateVmRequest, node_name: &str) -> Result<VmState> {
    let uuid_str = Uuid::new_v4().to_string();
    let id      = format!("vm-{}", &uuid_str[..8]);
    // caiman-vmm expects --vm-id as a u32 — use unix timestamp mod 65535
    let vm_num: u32 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_micros() % 65535 + 1;
    let cpus    = req.cpus.unwrap_or(1);
    let mem_mib = req.mem_mib.unwrap_or(256);
    let kernel  = req.kernel.clone()
        .unwrap_or_else(|| "/boot/vmlinuz".into());
    let uplink  = req.uplink.clone().unwrap_or_else(|| "eth0".into());
    let vm_num  = id.trim_start_matches("vm-")
        .chars().take(3).collect::<String>();
    let mac     = format!("02:aa:bb:00:{}:{}", &vm_num[..2], &vm_num[2..]);

    // Write initial state (BOOTING) before spawning
    let mut state = VmState {
        id:           id.clone(),
        name:         req.name.clone(),
        status:       VmStatus::Booting,
        pid:          None,
        cpus,
        mem_mib,
        node_name:    node_name.to_string(),
        kernel:       kernel.clone(),
        disk:         req.disk.clone(),
        mac:          mac.clone(),
        uplink:       uplink.clone(),
        labels:       req.labels.unwrap_or_default(),
        created_at:   Utc::now(),
        started_at:   None,
        cpu_usage_pct: 0.0,
        mem_used_mib:  0,
        net_rx_mbps:   0.0,
        net_tx_mbps:   0.0,
        uptime_secs:   0,
    };
    state.save()?;

    // Build caiman-vmm arguments
    let mut cmd = Command::new(VMM_BINARY);
    cmd.arg("--kernel").arg(&kernel)
        .arg("--mem-mib").arg(mem_mib.to_string())
        .arg("--cpus").arg(cpus.to_string())
        .arg("--uplink").arg(&uplink)
        .arg("--vm-id").arg(vm_num.to_string());

    if let Some(ref disk) = req.disk {
        cmd.arg("--disk").arg(disk);
    }
    if let Some(ref cmdline) = req.cmdline {
        cmd.arg("--cmdline").arg(cmdline);
    }

    // Redirect serial output to log file
    let log_path = format!("{STATE_DIR}/{id}.log");
    let log_file = std::fs::File::create(&log_path)
        .with_context(|| format!("creating log {log_path}"))?;
    let log_stderr = log_file.try_clone()?;

    cmd.stdout(log_file)
        .stderr(log_stderr)
        .kill_on_drop(false);

    let child = cmd.spawn()
        .with_context(|| format!("spawning {VMM_BINARY}"))?;

    let pid = child.id().unwrap_or(0);
    info!("VM {} started: pid={} name={}", id, pid, req.name);

    // Update state with PID
    state.status     = VmStatus::Running;
    state.pid        = Some(pid);
    state.started_at = Some(Utc::now());
    state.save()?;

    // Detach child (it runs independently)
    std::mem::forget(child);

    Ok(state)
}

/// Send SIGTERM to a VM process (graceful stop)
pub fn stop_vm(state: &mut VmState) -> Result<()> {
    if let Some(pid) = state.pid {
        let rc = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ESRCH) {
                anyhow::bail!("kill({pid}, SIGTERM): {err}");
            }
        }
        info!("VM {} sent SIGTERM to pid={}", state.id, pid);
    }
    state.status = VmStatus::Stopped;
    state.pid    = None;
    state.save()
}

/// Send SIGKILL to a VM process (force stop)
pub fn kill_vm(state: &mut VmState) -> Result<()> {
    if let Some(pid) = state.pid {
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        info!("VM {} sent SIGKILL to pid={}", state.id, pid);
    }
    state.status = VmStatus::Stopped;
    state.pid    = None;
    state.save()
}

/// Delete a VM's state and log files
pub fn delete_vm(id: &str) -> Result<()> {
    VmState::delete(id)?;
    let log = format!("{STATE_DIR}/{id}.log");
    if std::path::Path::new(&log).exists() {
        std::fs::remove_file(&log)?;
    }
    info!("VM {id} deleted");
    Ok(())
}
