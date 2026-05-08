//! vm/runner.rs -- spawn and manage caiman-vmm processes
//! v1.1.0: integra caiman-cni para red automatica (NAT/bridge)

use std::collections::HashMap;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, warn};
use chrono::Utc;
use uuid::Uuid;

use super::state::{VmState, VmStatus, STATE_DIR};

pub const VMM_BINARY: &str = "caiman-vmm";
pub const CNI_BINARY: &str = "caiman-cni";

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
    /// Network mode: "nat" (default) | "bridge" | "none"
    pub net_mode:  Option<String>,
}

// ── CNI integration ───────────────────────────────────────────────────────

/// Call caiman-cni ADD to setup network for a VM
/// Returns the TAP interface name and IP config string
async fn cni_add(vm_id: &str, net_mode: &str, uplink: &str) -> Result<(String, String)> {
    let tap_name = format!("caim{}", &vm_id[vm_id.len().saturating_sub(8)..]);

    let output = Command::new(CNI_BINARY)
        .env("CNI_COMMAND",     "ADD")
        .env("CNI_CONTAINERID", vm_id)
        .env("CNI_IFNAME",      "eth0")
        .env("CNI_NETNS",       "")
        .env("CNI_PATH",        "/usr/local/bin")
        .env("CAIMAN_NET_MODE", net_mode)
        .env("CAIMAN_UPLINK",   uplink)
        .output().await;

    match output {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Extract CAIMAN_IP_CONFIG from stderr
            let ip_config = stderr.lines()
                .find(|l| l.starts_with("CAIMAN_IP_CONFIG="))
                .map(|l| l.trim_start_matches("CAIMAN_IP_CONFIG=").to_string())
                .unwrap_or_default();
            info!("CNI ADD: tap={tap_name} ip_config={ip_config}");
            Ok((tap_name, ip_config))
        }
        Err(e) => {
            warn!("caiman-cni not found, using fallback tap: {e}");
            // Fallback: create tap manually
            let _ = tokio::process::Command::new("ip")
                .args(["tuntap", "add", "dev", &tap_name, "mode", "tap"])
                .status().await;
            let _ = tokio::process::Command::new("ip")
                .args(["link", "set", &tap_name, "up"])
                .status().await;
            Ok((tap_name, String::new()))
        }
    }
}

/// Call caiman-cni DEL to cleanup network
async fn cni_del(vm_id: &str) {
    let _ = Command::new(CNI_BINARY)
        .env("CNI_COMMAND",     "DEL")
        .env("CNI_CONTAINERID", vm_id)
        .env("CNI_IFNAME",      "eth0")
        .env("CNI_NETNS",       "")
        .output().await;
    info!("CNI DEL: vm={vm_id}");
}

// ── VM lifecycle ──────────────────────────────────────────────────────────

/// Spawn caiman-vmm and return the VM state with PID
pub async fn spawn_vm(req: CreateVmRequest, node_name: &str) -> Result<VmState> {
    let id      = format!("vm-{}", &Uuid::new_v4().to_string()[..8]);
    let cpus    = req.cpus.unwrap_or(1);
    let mem_mib = req.mem_mib.unwrap_or(256);
    let kernel  = req.kernel.clone().unwrap_or_else(|| "/boot/vmlinuz".into());
    let uplink  = req.uplink.clone().unwrap_or_else(detect_uplink);
    let net_mode = req.net_mode.clone().unwrap_or_else(|| "nat".into());

    let vm_num = id.trim_start_matches("vm-")
        .chars().take(3).collect::<String>();
    let mac = format!("02:aa:bb:00:{}:{}", &vm_num[..2], &vm_num[2..]);

    // 1. Setup network via CNI
    let (tap_name, ip_config) = cni_add(&id, &net_mode, &uplink).await?;

    // 2. Write initial state
    let mut state = VmState {
        id:            id.clone(),
        name:          req.name.clone(),
        status:        VmStatus::Booting,
        pid:           None,
        cpus,
        mem_mib,
        node_name:     node_name.to_string(),
        kernel:        kernel.clone(),
        disk:          req.disk.clone(),
        mac:           mac.clone(),
        uplink:        uplink.clone(),
        labels:        req.labels.unwrap_or_default(),
        created_at:    Utc::now(),
        started_at:    None,
        cpu_usage_pct: 0.0,
        mem_used_mib:  0,
        net_rx_mbps:   0.0,
        net_tx_mbps:   0.0,
        uptime_secs:   0,
    };
    state.save()?;

    // 3. Build cmdline -- inject IP config if CNI provided it
    let cmdline = if let Some(ref cl) = req.cmdline {
        cl.clone()
    } else if !ip_config.is_empty() {
        format!(
            "console=ttyS0,115200 reboot=k panic=1 nomodules \
             virtio_mmio.device=0x1000@0xd0000000:5 \
             virtio_mmio.device=0x1000@0xd0010000:6 \
             {ip_config} nameserver=1.1.1.1"
        )
    } else {
        "console=ttyS0,115200 reboot=k panic=1 nomodules \
         virtio_mmio.device=0x1000@0xd0000000:5 \
         virtio_mmio.device=0x1000@0xd0010000:6".to_string()
    };

    // 4. Spawn caiman-vmm with correct TAP
    let mut cmd = Command::new(VMM_BINARY);
    cmd.arg("--kernel").arg(&kernel)
        .arg("--mem-mib").arg(mem_mib.to_string())
        .arg("--cpus").arg(cpus.to_string())
        .arg("--tap").arg(&tap_name)
        .arg("--uplink").arg(&uplink)
        .arg("--vm-id").arg(&id)
        .arg("--cmdline").arg(&cmdline);

    if let Some(ref disk) = req.disk {
        cmd.arg("--disk").arg(disk);
    }

    let log_path = format!("{STATE_DIR}/{id}.log");
    let log_file = std::fs::File::create(&log_path)
        .with_context(|| format!("creating log {log_path}"))?;
    let log_stderr = log_file.try_clone()?;

    cmd.stdout(log_file).stderr(log_stderr).kill_on_drop(false);

    let child = cmd.spawn()
        .with_context(|| format!("spawning {VMM_BINARY}"))?;

    let pid = child.id().unwrap_or(0);
    info!("VM {} started: pid={} name={} tap={} net={}", id, pid, req.name, tap_name, net_mode);

    state.status     = VmStatus::Running;
    state.pid        = Some(pid);
    state.started_at = Some(Utc::now());
    state.save()?;

    std::mem::forget(child);
    Ok(state)
}

/// Detect default network interface
fn detect_uplink() -> String {
    if let Ok(out) = std::process::Command::new("ip")
        .args(["route", "show", "default"]).output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(dev) = s.split_whitespace()
            .skip_while(|w| *w != "dev").nth(1)
        {
            return dev.to_string();
        }
    }
    for iface in &["eth0", "ens3", "enp3s0", "wlan0", "bond0"] {
        if std::path::Path::new(&format!("/sys/class/net/{iface}")).exists() {
            return iface.to_string();
        }
    }
    "eth0".to_string()
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

/// Delete a VM and cleanup network
pub fn delete_vm(id: &str) -> Result<()> {
    VmState::delete(id)?;
    let log = format!("{STATE_DIR}/{id}.log");
    if std::path::Path::new(&log).exists() {
        std::fs::remove_file(&log)?;
    }
    info!("VM {id} deleted");
    Ok(())
}
