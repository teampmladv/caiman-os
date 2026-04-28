//! collectors/node.rs — Node metrics from /proc, sysinfo, and /sys/block

use std::collections::HashMap;
use anyhow::Result;
use sysinfo::{System, Disks, Networks};
use tracing::debug;

use super::NodeMetrics;

pub async fn collect() -> Result<Vec<NodeMetrics>> {
    // In a cluster deployment, each node runs its own caiman-api and reports
    // to a central aggregator. Here we collect the local node's metrics.
    let local = collect_local().await?;
    Ok(vec![local])
}

async fn collect_local() -> Result<NodeMetrics> {
    // Run blocking sysinfo calls in a thread pool
    tokio::task::spawn_blocking(collect_sync).await?
}

fn collect_sync() -> Result<NodeMetrics> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".into())
        .trim().to_string();

    // CPU
    let cpu_cores   = sys.cpus().len() as u32;
    let cpu_usage   = sys.global_cpu_usage() as f64;

    // Memory
    let mem_total   = sys.total_memory() / (1024 * 1024);
    let mem_used    = sys.used_memory()  / (1024 * 1024);

    // Disk IOPS (from /sys/block/*/stat, field 0=read_ios, 4=write_ios)
    let (read_iops, write_iops) = read_disk_stats();

    // Network throughput (aggregate across all interfaces)
    let (net_rx, net_tx) = read_net_stats();

    // Uptime from /proc/uptime
    let uptime_secs = read_uptime();

    // Kernel version
    let kernel = std::fs::read_to_string("/proc/version")
        .unwrap_or_default()
        .split_whitespace()
        .nth(2)
        .unwrap_or("unknown")
        .to_string();

    // Running VMs (state files)
    let vms = list_vm_ids();
    let vm_count = vms.len() as u32;

    Ok(NodeMetrics {
        id:              format!("n-{}", &hostname),
        hostname,
        status:          "HEALTHY".into(),
        cpu_cores,
        cpu_usage_pct:   cpu_usage,
        mem_total_mib:   mem_total,
        mem_used_mib:    mem_used,
        disk_read_iops:  read_iops,
        disk_write_iops: write_iops,
        net_rx_mbps:     net_rx,
        net_tx_mbps:     net_tx,
        load_score:      0.0,  // computed after
        vm_count,
        vms,
        kernel_version:  kernel,
        caiman_version:  env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs,
    })
}

fn read_disk_stats() -> (f64, f64) {
    let mut total_r = 0f64;
    let mut total_w = 0f64;

    let Ok(dir) = std::fs::read_dir("/sys/block") else { return (0.0, 0.0) };
    for entry in dir.flatten() {
        // Skip loop, dm devices
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("loop") || name.starts_with("dm-") { continue; }

        if let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) {
            let fields: Vec<u64> = stat.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if fields.len() >= 8 {
                total_r += fields[0] as f64;
                total_w += fields[4] as f64;
            }
        }
    }
    (total_r, total_w)
}

fn read_net_stats() -> (f64, f64) {
    // Read /proc/net/dev, sum RX/TX bytes across non-loopback interfaces
    // Convert to Mbps (approximate — we'd need delta between reads for accuracy)
    let Ok(data) = std::fs::read_to_string("/proc/net/dev") else { return (0.0, 0.0) };
    let mut rx_bytes = 0u64;
    let mut tx_bytes = 0u64;

    for line in data.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 { continue; }
        let iface = parts[0].trim_end_matches(':');
        if iface == "lo" { continue; }
        rx_bytes += parts[1].parse::<u64>().unwrap_or(0);
        tx_bytes += parts[9].parse::<u64>().unwrap_or(0);
    }

    // Convert bytes to Mbps (rough: assume 1s measurement window)
    ((rx_bytes * 8) as f64 / 1_000_000.0,
     (tx_bytes * 8) as f64 / 1_000_000.0)
}

fn read_uptime() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|v| v.to_string()))
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0)
}

fn list_vm_ids() -> Vec<String> {
    let state_dir = std::env::var("CAIMAN_STATE_DIR")
        .unwrap_or_else(|_| "/var/run/caiman".into());
    let Ok(dir) = std::fs::read_dir(&state_dir) else { return Vec::new() };
    dir.flatten()
        .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
        .filter_map(|e| {
            e.path().file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .collect()
}
