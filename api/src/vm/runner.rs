//! vm/runner.rs -- spawn and manage caiman-vmm processes
//! v1.1.0: integra caiman-cni para red automatica (NAT/bridge)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::unix::process::CommandExt;
use std::os::unix::fs::OpenOptionsExt;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, warn};
use chrono::Utc;
use uuid::Uuid;

use super::state::{VmState, VmStatus, STATE_DIR};

pub const VMM_BINARY: &str = "caiman-vmm";
pub const CNI_BINARY: &str = "caiman-cni";

/// Build the kernel cmdline for a managed VM.
///
/// Preserves the existing boot model per call site:
/// - root_disk: add root=/dev/vda rootfstype=ext4 (disk-boot). initrd-based VMs pass false.
/// - has_disk:  announce the virtio-blk device (:6).
/// - ip_config: NAT config from CNI; empty string when not applicable.
///
/// virtio-net (:5) is always announced: every API-managed VM gets a CNI tap.
fn build_vm_cmdline(root_disk: bool, has_disk: bool, ip_config: &str) -> String {
    let mut p: Vec<String> = vec![
        "earlycon=uart8250,io,0x3f8,115200n8".to_string(),
        "console=ttyS0,115200".to_string(),
    ];
    if root_disk {
        p.push("root=/dev/vda".to_string());
        p.push("rootfstype=ext4".to_string());
    }
    p.push("rw".to_string());
    p.push("reboot=k".to_string());
    p.push("panic=1".to_string());
    p.push("virtio_mmio.device=0x1000@0xd0000000:5".to_string()); // virtio-net
    if has_disk {
        p.push("virtio_mmio.device=0x1000@0xd0010000:6".to_string()); // virtio-blk
    }
    if !ip_config.is_empty() {
        p.push(ip_config.to_string());
        p.push("nameserver=1.1.1.1".to_string());
    }
    p.join(" ")
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVmRequest {
    pub name:       String,
    pub cpus:       Option<u8>,
    pub mem_mib:    Option<u64>,
    pub kernel:     Option<String>,
    pub initrd:     Option<String>,
    pub disk:       Option<String>,
    pub base_image: Option<String>,
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
    // Create per-VM disk from base image
    let base = req.base_image.clone()
        .unwrap_or_else(|| "caiman-base-1.0.img".to_string());
    let disk = match VmState::create_disk(&id, &base) {
        Ok(path) => Some(path),
        Err(e) => {
            warn!("Failed to clone base image {base}: {e} -- using shared disk");
            req.disk.clone().or_else(|| Some("/var/lib/caiman/kernels/caiman-rootfs.img".into()))
        }
    };
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
        disk_path:     disk.clone(),
        base_image:    disk.clone(),
        disk_size_gib: None,
        volumes:       vec![],
        ip:            None,
        tap:           Some(tap_name.clone()),
        pty:           None,
        console_log:   None,
        power_state:   "Booting".to_string(),
        task_state:    None,
        flavor:        None,
        hypervisor:    "caiman-vmm".to_string(),
        zone: "caiman-zone-1".to_string(),
        autostart:     false,
        project_id:    None,
        user_id:       None,
        security_groups: vec![],
        launched_at:   None,
        terminated_at: None,
        uuid:          uuid::Uuid::new_v4().to_string(),
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
    // Use caller-provided cmdline as-is; otherwise derive it from the VM's devices.
    // Preserves prior behavior: networked VMs are initrd-based (no root=), the
    // diskless-network fallback boots root=/dev/vda directly. virtio-net is now
    // always announced so the guest can see its CNI-provided interface.
    let cmdline = if let Some(ref cl) = req.cmdline {
        cl.clone()
    } else {
        let root_disk = ip_config.is_empty();
        build_vm_cmdline(root_disk, disk.is_some(), &ip_config)
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
    let log_path = format!("/var/lib/caiman/vms/{id}/console.log");
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
        std::fs::OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOCTTY).open(&slave_name)
            .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())
    };
    let pty_slave2 = pty_slave.try_clone()?;

    cmd.stdout(pty_slave).stderr(log_stderr).stdin(pty_slave2).kill_on_drop(false).process_group(0);
    unsafe {
        cmd.pre_exec(|| {
            // Become session leader, detach from controlling terminal
            libc::setsid();
            // Ignore SIGHUP from parent
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            // Ignore TTY signals so we don't get stopped on background read/write
            libc::signal(libc::SIGTTIN, libc::SIG_IGN);
            libc::signal(libc::SIGTTOU, libc::SIG_IGN);
            Ok(())
        })
    };

    // Console socket bridge -- bidirectional PTY <-> Unix socket
    {
        use std::os::unix::io::IntoRawFd;
        let pty_fd = pty_master.into_raw_fd();
        let sock_path = format!("/var/lib/caiman/vms/{id}/console.sock");
        let _ = std::fs::remove_file(&sock_path);

        // broadcast: PTY output -> all connected WS clients
        let (bcast_tx, _bcast_init) = tokio::sync::broadcast::channel::<Vec<u8>>(256);
        let bcast_tx_reader = bcast_tx.clone();

        // ring buffer: last 64KB for late joiners
        let ringbuf: Arc<Mutex<std::collections::VecDeque<u8>>> =
            Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(65536)));
        let ringbuf_writer = Arc::clone(&ringbuf);

        // mpsc: client keystrokes -> PTY write thread
        let (write_tx, write_rx) = std::sync::mpsc::channel::<Vec<u8>>();

        // Thread: blocking PTY read -> broadcast + ringbuf
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                let n = unsafe {
                    libc::read(pty_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                };
                if n <= 0 { break; }
                let chunk = &buf[..n as usize];
                // Always append to ring buffer regardless of receivers
                if let Ok(mut rb) = ringbuf_writer.lock() {
                    for &b in chunk {
                        if rb.len() >= 65536 { rb.pop_front(); }
                        rb.push_back(b);
                    }
                }
                // Ignore send errors -- no receivers yet is fine
                let _ = bcast_tx_reader.send(chunk.to_vec());
            }
        });

        // Thread: mpsc -> blocking PTY write
        std::thread::spawn(move || {
            for data in write_rx {
                unsafe {
                    libc::write(pty_fd, data.as_ptr() as *const libc::c_void, data.len());
                }
            }
        });

        // Task: Unix socket listener -- one slot per WS client
        tokio::spawn(async move {
            use tokio::net::UnixListener;
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let Ok(listener) = UnixListener::bind(&sock_path) else { return };

            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                let (mut srx, mut stx) = tokio::io::split(stream);
                let mut brx = bcast_tx.subscribe();
                let wtx = write_tx.clone();
                let rb = Arc::clone(&ringbuf);

                // PTY output -> socket (send backlog first, then live)
                tokio::spawn(async move {
                    // Drain ring buffer first
                    let backlog: Vec<u8> = rb.lock()
                        .map(|r| r.iter().copied().collect())
                        .unwrap_or_default();
                    if !backlog.is_empty() {
                        if stx.write_all(&backlog).await.is_err() { return; }
                    }
                    // Then live stream
                    loop {
                        match brx.recv().await {
                            Ok(data) => { if stx.write_all(&data).await.is_err() { break; } }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                });

                // Socket input -> PTY
                tokio::spawn(async move {
                    let mut buf = [0u8; 256];
                    loop {
                        match srx.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => { let _ = wtx.send(buf[..n].to_vec()); }
                        }
                    }
                });
            }
        });
    }

    let mut child = cmd.spawn()
        .with_context(|| format!("spawning {VMM_BINARY}"))?;

    let pid = child.id().unwrap_or(0);
    info!("VM {} started: pid={} name={} tap={} net={}", id, pid, req.name, tap_name, net_mode);

    state.status     = VmStatus::Active;
    state.pid        = Some(pid);
    state.power_state = "Running".to_string();
    state.launched_at = Some(Utc::now());
    state.pty        = Some(format!("{STATE_DIR}/{id}.pty"));
    state.started_at = Some(Utc::now());
    // Extract IP from ip_config
    if !ip_config.is_empty() {
        state.ip = ip_config.splitn(2, '=').nth(1)
            .and_then(|s| s.splitn(2, ':').next())
            .map(|s| s.to_string());
    }
    state.save()?;

    // Reap the child to avoid zombies and reconcile state on exit.
    let reap_id = id.to_string();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(s)  => info!("VM {reap_id} process exited: {s}"),
            Err(e) => warn!("VM {reap_id} wait error: {e}"),
        }
        if let Ok(mut st) = VmState::load(&reap_id) {
            st.status      = VmStatus::ShutOff;
            st.power_state = "Stopped".to_string();
            st.pid         = None;
            let _ = st.save();
        }
    });
    Ok(state)
}


/// Restart an existing VM using its saved state (same id, disk, tap, mac)
pub async fn restart_vm(id: &str, node_name: &str) -> Result<VmState> {
    let mut state = VmState::load(id)?;

    // Check not already running
    if let Some(pid) = state.pid {
        let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
        if alive {
            anyhow::bail!("VM {id} is already running (pid={pid})");
        }
    }

    let kernel   = state.kernel.clone();
    let initrd   = state.initrd.clone().unwrap_or_else(|| "/var/lib/caiman/kernels/caiman-initrd.img".into());
    let disk     = state.disk_path.clone();
    let tap      = state.tap.clone().unwrap_or_else(|| format!("caim{}", &id[id.len().saturating_sub(8)..]));
    let uplink   = state.uplink.clone();
    let net_mode = state.net_mode.clone().unwrap_or_else(|| "nat".into());
    let cpus     = state.cpus;
    let mem_mib  = state.mem_mib;

    // Recreate tap if gone
    let tap_exists = std::path::Path::new(&format!("/sys/class/net/{tap}")).exists();
    let mut ip_config = String::new();
    if !tap_exists {
        let (_new_tap, ipc) = cni_add(id, &net_mode, &uplink).await?;
        ip_config = ipc;
        if !ip_config.is_empty() {
            state.ip = ip_config.splitn(2, '=').nth(1)
                .and_then(|s| s.splitn(2, ':').next())
                .map(|s| s.to_string());
        }
    }

    // initrd-based restart: no root= (initrd mounts /dev/vda); now announces
    // virtio-net (:5) and carries ip_config when the tap was recreated.
    let cmdline = build_vm_cmdline(false, disk.is_some(), &ip_config);

    let mut cmd = Command::new(VMM_BINARY);
    cmd.arg("--kernel").arg(&kernel)
        .arg("--mem-mib").arg(mem_mib.to_string())
        .arg("--cpus").arg(cpus.to_string())
        .arg("--tap").arg(&tap)
        .arg("--uplink").arg(&uplink)
        .arg("--vm-id").arg(id)
        .arg("--vm-name").arg(&state.name)
        .arg("--cmdline").arg(&cmdline)
        .arg("--initrd").arg(&initrd);
    if let Some(ref d) = disk {
        cmd.arg("--disk").arg(d);
    }

    let vm_dir   = format!("/var/lib/caiman/vms/{id}");
    let log_path = format!("{vm_dir}/console.log");
    let err_path = format!("{STATE_DIR}/{id}.vmm.log");
    let log_stderr = std::fs::File::create(&err_path)?;

    let pty_master = std::fs::OpenOptions::new()
        .read(true).write(true)
        .open("/dev/ptmx")
        .context("opening /dev/ptmx")?;

    unsafe {
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&pty_master);
        libc::grantpt(fd);
        libc::unlockpt(fd);
        let slave_name = std::ffi::CStr::from_ptr(libc::ptsname(fd)).to_str().unwrap_or("").to_string();
        std::fs::write(&log_path, format!("PTY:{}
", slave_name))?;
        let pty_link = format!("{STATE_DIR}/{id}.pty");
        let _ = std::fs::remove_file(&pty_link);
        let _ = std::os::unix::fs::symlink(&slave_name, &pty_link);
    }

    let pty_slave = {
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&pty_master);
        let slave_name = unsafe {
            std::ffi::CStr::from_ptr(libc::ptsname(fd)).to_str().unwrap_or("/dev/null").to_string()
        };
        std::fs::OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOCTTY).open(&slave_name)
            .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())
    };
    let pty_slave2 = pty_slave.try_clone()?;
    cmd.stdout(pty_slave).stderr(log_stderr).stdin(pty_slave2).kill_on_drop(false).process_group(0);
    unsafe {
        cmd.pre_exec(|| {
            // Become session leader, detach from controlling terminal
            libc::setsid();
            // Ignore SIGHUP from parent
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            // Ignore TTY signals so we don't get stopped on background read/write
            libc::signal(libc::SIGTTIN, libc::SIG_IGN);
            libc::signal(libc::SIGTTOU, libc::SIG_IGN);
            Ok(())
        })
    };

    // Console socket bridge
    {
        use std::os::unix::io::IntoRawFd;
        let pty_fd = pty_master.into_raw_fd();
        let sock_path = format!("{vm_dir}/console.sock");
        let _ = std::fs::remove_file(&sock_path);

        let (bcast_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(256);
        let bcast_tx_reader = bcast_tx.clone();
        let ringbuf: Arc<Mutex<std::collections::VecDeque<u8>>> =
            Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(65536)));
        let ringbuf_writer = Arc::clone(&ringbuf);
        let (write_tx, write_rx) = std::sync::mpsc::channel::<Vec<u8>>();

        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                let n = unsafe { libc::read(pty_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 { break; }
                let chunk = &buf[..n as usize];
                if let Ok(mut rb) = ringbuf_writer.lock() {
                    for &b in chunk { if rb.len() >= 65536 { rb.pop_front(); } rb.push_back(b); }
                }
                let _ = bcast_tx_reader.send(chunk.to_vec());
            }
        });
        std::thread::spawn(move || {
            for data in write_rx {
                unsafe { libc::write(pty_fd, data.as_ptr() as *const libc::c_void, data.len()); }
            }
        });
        tokio::spawn(async move {
            use tokio::net::UnixListener;
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let Ok(listener) = UnixListener::bind(&sock_path) else { return };
            loop {
                let Ok((stream, _)) = listener.accept().await else { break };
                let (mut srx, mut stx) = tokio::io::split(stream);
                let mut brx = bcast_tx.subscribe();
                let wtx = write_tx.clone();
                let rb = Arc::clone(&ringbuf);
                tokio::spawn(async move {
                    let backlog: Vec<u8> = rb.lock().map(|r| r.iter().copied().collect()).unwrap_or_default();
                    if !backlog.is_empty() { if stx.write_all(&backlog).await.is_err() { return; } }
                    loop {
                        match brx.recv().await {
                            Ok(data) => { if stx.write_all(&data).await.is_err() { break; } }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                });
                tokio::spawn(async move {
                    let mut buf = [0u8; 256];
                    loop {
                        match srx.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => { let _ = wtx.send(buf[..n].to_vec()); }
                        }
                    }
                });
            }
        });
    }

    let mut child = cmd.spawn().with_context(|| format!("spawning {VMM_BINARY}"))?;
    let pid = child.id().unwrap_or(0);
    info!("VM {id} restarted: pid={pid}");

    state.status      = VmStatus::Active;
    state.pid         = Some(pid);
    state.power_state = "Running".to_string();
    state.launched_at = Some(Utc::now());
    state.started_at  = Some(Utc::now());
    state.pty         = Some(format!("{STATE_DIR}/{id}.pty"));
    state.node_name   = node_name.to_string();
    state.save()?;

    // Reap the child to avoid zombies and reconcile state on exit.
    let reap_id = id.to_string();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(s)  => info!("VM {reap_id} process exited: {s}"),
            Err(e) => warn!("VM {reap_id} wait error: {e}"),
        }
        if let Ok(mut st) = VmState::load(&reap_id) {
            st.status      = VmStatus::ShutOff;
            st.power_state = "Stopped".to_string();
            st.pid         = None;
            let _ = st.save();
        }
    });
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
    state.status = VmStatus::ShutOff;
    state.pid    = None;
    state.save()
}

/// Send SIGKILL to a VM process (force stop)
pub fn kill_vm(state: &mut VmState) -> Result<()> {
    if let Some(pid) = state.pid {
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        info!("VM {} sent SIGKILL to pid={}", state.id, pid);
    }
    state.status = VmStatus::ShutOff;
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
