//! collectors/vm.rs — VM metrics from state files + /proc/<pid>/stat

use std::collections::HashMap;
use anyhow::Result;
use serde::Deserialize;
use tracing::debug;

use super::VmMetrics;

#[derive(Debug, Deserialize)]
struct VmStateFile {
    vm_id:      u32,
    pid:        u32,
    mac:        String,
    mem_mib:    u64,
    cpus:       u8,
    uplink:     String,
    started_at: String,
    node:       Option<String>,
    labels:     Option<HashMap<String, String>>,
    status:     Option<String>,
}

pub async fn collect() -> Result<Vec<VmMetrics>> {
    tokio::task::spawn_blocking(collect_sync).await?
}

fn collect_sync() -> Result<Vec<VmMetrics>> {
    let state_dir = std::env::var("CAIMAN_STATE_DIR")
        .unwrap_or_else(|_| "/var/run/caiman".into());

    let Ok(dir) = std::fs::read_dir(&state_dir) else {
        return Ok(Vec::new());
    };

    let mut vms = Vec::new();

    for entry in dir.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "json") { continue; }

        let Ok(data) = std::fs::read_to_string(&path) else { continue };
        let Ok(state) = serde_json::from_str::<VmStateFile>(&data) else { continue };

        let vm_id   = format!("vm-{:03}", state.vm_id);
        let pid     = state.pid;
        let alive   = std::path::Path::new(&format!("/proc/{pid}")).exists();
        let status  = if !alive { "STOPPED".to_string() }
            else { state.status.unwrap_or_else(|| "RUNNING".to_string()) };

        // Read CPU usage from /proc/<pid>/stat
        let cpu_usage = if alive { read_proc_cpu(pid) } else { 0.0 };

        // Read RSS memory from /proc/<pid>/status
        let mem_used_mib = if alive { read_proc_mem_mib(pid) } else { 0 };

        // Read uptime from process start time
        let uptime_secs = read_proc_uptime(pid);

        // Read network stats from /proc/<pid>/net/dev (VMM process netns)
        let (net_rx, net_tx) = if alive { read_vm_net(pid) } else { (0.0, 0.0) };

        // Check for migration state file
        let migrating = read_migration_state(&vm_id);

        // Read name from labels or use vm_id
        let labels = state.labels.unwrap_or_default();
        let name = labels.get("name")
            .cloned()
            .unwrap_or_else(|| format!("vm-{}", state.vm_id));

        let hostname = std::fs::read_to_string("/etc/hostname")
            .unwrap_or_default().trim().to_string();

        vms.push(VmMetrics {
            id:               vm_id,
            name,
            status,
            node_id:          format!("n-{hostname}"),
            node_name:        state.node.unwrap_or(hostname),
            cpu_cores:        state.cpus as u32,
            cpu_usage_pct:    cpu_usage,
            mem_mib:          mem_used_mib,
            mem_total_mib:    state.mem_mib,
            disk_read_iops:   0.0,  // TODO: virtio-blk stats via KVM ioctl
            disk_write_iops:  0.0,
            net_rx_mbps:      net_rx,
            net_tx_mbps:      net_tx,
            net_rx_drops:     0,    // populated by xdp collector
            uptime_secs,
            mac:              state.mac,
            labels,
            started_at:       state.started_at,
            migrating,
            pid:              Some(pid),
        });
    }

    debug!("VM collector: found {} VMs", vms.len());
    Ok(vms)
}

/// Read CPU usage percentage for a process from /proc/<pid>/stat.
/// Uses jiffies delta between two quick reads (5ms apart).
fn read_proc_cpu(pid: u32) -> f64 {
    let path = format!("/proc/{pid}/stat");
    let Ok(stat1) = std::fs::read_to_string(&path) else { return 0.0 };
    std::thread::sleep(std::time::Duration::from_millis(50));
    let Ok(stat2) = std::fs::read_to_string(&path) else { return 0.0 };

    let fields1: Vec<u64> = stat1.split_whitespace()
        .filter_map(|s| s.parse().ok()).collect();
    let fields2: Vec<u64> = stat2.split_whitespace()
        .filter_map(|s| s.parse().ok()).collect();

    if fields1.len() < 15 || fields2.len() < 15 { return 0.0; }

    // utime=13, stime=14, cutime=15, cstime=16
    let cpu1 = fields1[13] + fields1[14];
    let cpu2 = fields2[13] + fields2[14];
    let delta = cpu2.saturating_sub(cpu1) as f64;

    // Normalize to percentage (50ms window, ~100 Hz clock)
    (delta / (0.05 * 100.0) * 100.0).min(100.0 * num_cpus::get() as f64)
}

fn read_proc_mem_mib(pid: u32) -> u64 {
    let path = format!("/proc/{pid}/status");
    let Ok(data) = std::fs::read_to_string(&path) else { return 0 };
    for line in data.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0) / 1024;
        }
    }
    0
}

fn read_proc_uptime(pid: u32) -> u64 {
    // /proc/<pid>/stat field 21 = starttime in jiffies since boot
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { return 0 };
    let fields: Vec<&str> = stat.split_whitespace().collect();
    if fields.len() < 22 { return 0; }
    let start_jiffies: u64 = fields[21].parse().unwrap_or(0);
    let uptime_secs = read_system_uptime();
    let hz = 100u64; // typical HZ value
    uptime_secs.saturating_sub(start_jiffies / hz)
}

fn read_system_uptime() -> u64 {
    std::fs::read_to_string("/proc/uptime").ok()
        .and_then(|s| s.split_whitespace().next().map(|v| v.to_string()))
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0)
}

fn read_vm_net(pid: u32) -> (f64, f64) {
    // Read /proc/<pid>/net/dev for the VMM's network namespace
    let path = format!("/proc/{pid}/net/dev");
    let Ok(data) = std::fs::read_to_string(path) else { return (0.0, 0.0) };
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in data.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 { continue; }
        let iface = parts[0].trim_end_matches(':');
        if iface == "lo" { continue; }
        rx += parts[1].parse::<u64>().unwrap_or(0);
        tx += parts[9].parse::<u64>().unwrap_or(0);
    }
    ((rx * 8) as f64 / 1_000_000.0, (tx * 8) as f64 / 1_000_000.0)
}

fn read_migration_state(vm_id: &str) -> Option<super::MigrationStatus> {
    let state_dir = std::env::var("CAIMAN_STATE_DIR")
        .unwrap_or_else(|_| "/var/run/caiman".into());
    let path = format!("{state_dir}/mig-{vm_id}.json");
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}
