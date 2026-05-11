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
    pub initrd:    Option<String>,
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
    let kernel  = req.kernel.clone().unwrap_or_else(|| "/var/lib/caiman/kernels/vmlinuz-alpine".into());
    let disk    = req.disk.clone().or_else(|| Some("/var/lib/caiman/kernels/caiman-rootfs.img".into()));
    let initrd  = req.initrd.clone().unwrap_or_else(|| "/var/lib/caiman/kernels/caiman-initrd.img".into());
    let uplink  = req.uplink.clone().unwrap_or_else(detect_uplink);
    let net_mode = req.net_mode.clone().unwrap_or_else(|| "nat".into());

    let vm_num = id.trim_start_matches("vm-")
        .chars().take(3).collect::<String>();
    let mac = format!("02:aa:bb:00:{}:{}", &vm_num[..2], &vm_num[2..]);

    // 1. Setup network via CNI
    let (tap_name, ip_config) = cni_add(&id, &net_mode, &uplink).await?;

    // 2. Write initial state
    let disk_id = disk.as_ref().map(|d| {
        format!("disk-{}", &uuid::Uuid::new_v4().to_string()[..8])
    });

    let mut state = VmState {
        id:            id.clone(),
        name:          req.name.clone(),
        status:        VmStatus::Booting,
        pid:           None,
        cpus,
        mem_mib,
        node_name:     node_name.to_string(),
        kernel:        kernel.clone(),
        initrd:        Some(initrd.clone()),
        disk:          disk.clone(),
        disk_id,
        base_image:    disk.clone(),
        ip:            None,
        tap:           Some(tap_name.clone()),
        pty:           None,
        net_mode:      Some(net_mode.clone()),
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
            "earlycon=uart8250,io,0x3f8,115200n8 console=ttyS0,115200 rw reboot=k panic=1              virtio_mmio.device=0x1000@0xd0010000:6              {ip_config} nameserver=1.1.1.1"
        )
    } else {
        "earlycon=uart8250,io,0x3f8,115200n8 console=ttyS0,115200 root=/dev/vda rootfstype=ext4 rw reboot=k panic=1 virtio_mmio.device=0x1000@0xd0010000:6".to_string()
    };

    // 4. Spawn caiman-vmm with correct TAP
    let mut cmd = Command::new(VMM_BINARY);
    cmd.arg("--kernel").arg(&kernel)
        .arg("--mem-mib").arg(mem_mib.to_string())
        .arg("--cpus").arg(cpus.to_string())
        .arg("--tap").arg(&tap_name)
        .arg("--uplink").arg(&uplink)
        .arg("--vm-id").arg(&id)
        .arg("--vm-name").arg(&req.name)
        .arg("--cmdline").arg(&cmdline);

    cmd.arg("--initrd").arg(&initrd);
    if let Some(ref disk) = disk {
        cmd.arg("--disk").arg(disk);
    }

    // Create PTY for serial console -- gives shell a real TTY
    let log_path = format!("{STATE_DIR}/{id}.log");
    let err_path = format!("{STATE_DIR}/{id}.vmm.log");
    let log_stderr = std::fs::File::create(&err_path)
        .with_context(|| format!("creating vmm log {err_path}"))?;

    // Open PTY master/slave
    let pty_master = std::fs::OpenOptions::new()
        .read(true).write(true)
        .open("/dev/ptmx")
        .context("opening /dev/ptmx")?;

    // Grant and unlock PTY
    unsafe {
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&pty_master);
        libc::grantpt(fd);
        libc::unlockpt(fd);
        // Get slave PTY name and save it
        let slave_name = std::ffi::CStr::from_ptr(libc::ptsname(fd)).to_str().unwrap_or("").to_string();
        std::fs::write(&log_path, format!("PTY:{}
", slave_name))
            .with_context(|| format!("writing pty path to {log_path}"))?;
        // Save pty master fd for WebSocket streaming
        let pty_link = format!("{STATE_DIR}/{id}.pty");
        let _ = std::os::unix::fs::symlink(&slave_name, &pty_link);
        // Store PTY path -- will be saved to state after spawn
    }

    // Use PTY slave as stdout for VMM
    let pty_slave = {
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&pty_master);
        let slave_name = unsafe {
            std::ffi::CStr::from_ptr(libc::ptsname(fd)).to_str().unwrap_or("/dev/null").to_string()
        };
        std::fs::OpenOptions::new().read(true).write(true).open(&slave_name)
            .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())
    };
    let pty_slave2 = pty_slave.try_clone()?;

    cmd.stdout(pty_slave).stderr(log_stderr).stdin(pty_slave2).kill_on_drop(false).process_group(0);

    // Save PTY master for log streaming
    {
        let pty_log = format!("{STATE_DIR}/{id}.log");
        let mut master = pty_master;
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let std_master = unsafe {
                use std::os::unix::io::FromRawFd;
                let fd = std::os::unix::io::IntoRawFd::into_raw_fd(master);
                std::fs::File::from_raw_fd(fd)
            };
            let mut async_master = tokio::fs::File::from_std(std_master);
            let mut log = tokio::fs::OpenOptions::new().create(true).append(true)
                .open(&pty_log).await.unwrap();
            let mut buf = [0u8; 1024];
            loop {
                match async_master.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => { let _ = log.write_all(&buf[..n]).await; }
                }
            }
        });
    }

    let child = cmd.spawn()
        .with_context(|| format!("spawning {VMM_BINARY}"))?;

    let pid = child.id().unwrap_or(0);
    info!("VM {} started: pid={} name={} tap={} net={}", id, pid, req.name, tap_name, net_mode);

    state.status     = VmStatus::Running;
    state.pid        = Some(pid);
    state.pty        = Some(format!("{STATE_DIR}/{id}.pty"));
    state.started_at = Some(Utc::now());
    // Extract IP from ip_config
    if !ip_config.is_empty() {
        state.ip = ip_config.splitn(2, '=').nth(1)
            .and_then(|s| s.splitn(2, ':').next())
            .map(|s| s.to_string());
    }
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
