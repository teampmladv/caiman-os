//! node/metrics.rs — real node metrics from /proc + sysinfo

use serde::{Deserialize, Serialize};
use sysinfo::{System, Disks, Networks};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    pub hostname:      String,
    pub status:        String,
    pub cpu_cores:     usize,
    pub cpu_usage_pct: f64,
    pub mem_total_mib: u64,
    pub mem_used_mib:  u64,
    pub disk_total_gib:u64,
    pub disk_used_gib: u64,
    pub net_rx_mbps:   f64,
    pub net_tx_mbps:   f64,
    pub load_score:    f64,
    pub vm_count:      usize,
    pub uptime_secs:   u64,
}

impl NodeMetrics {
    pub fn collect(vm_count: usize) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = System::host_name()
            .unwrap_or_else(|| "unknown".into());

        let cpu_cores = sys.cpus().len();
        let cpu_usage = if cpu_cores == 0 { 0.0 } else {
            sys.cpus().iter().map(|c| c.cpu_usage() as f64).sum::<f64>()
                / cpu_cores as f64
        };

        let mem_total = sys.total_memory() / (1024 * 1024);
        let mem_used  = sys.used_memory()  / (1024 * 1024);

        let disks = Disks::new_with_refreshed_list();
        let (disk_total, disk_used) = disks.iter()
            .filter(|d| d.mount_point() == std::path::Path::new("/"))
            .next()
            .map(|d| (
                d.total_space()     / (1024 * 1024 * 1024),
                (d.total_space() - d.available_space()) / (1024 * 1024 * 1024),
            ))
            .unwrap_or((0, 0));

        // Network — sum all interfaces
        let mut nets = Networks::new_with_refreshed_list();
        std::thread::sleep(std::time::Duration::from_millis(200));
        nets.refresh();
        let (rx_bytes, tx_bytes): (u64, u64) = nets.iter()
            .filter(|(name, _)| !name.starts_with("lo"))
            .fold((0, 0), |(rx, tx), (_, data)| {
                (rx + data.received(), tx + data.transmitted())
            });
        let rx_mbps = rx_bytes as f64 * 8.0 / 1_000_000.0 / 0.2;
        let tx_mbps = tx_bytes as f64 * 8.0 / 1_000_000.0 / 0.2;

        let mem_pct   = if mem_total > 0 { mem_used as f64 / mem_total as f64 } else { 0.0 };
        let load_score = (cpu_usage / 100.0 * 0.6 + mem_pct * 0.4).min(1.0);

        let uptime = System::uptime();

        let status = if cpu_usage > 80.0 || mem_pct > 0.85 {
            "HIGH_LOAD"
        } else {
            "HEALTHY"
        }.to_string();

        Self {
            hostname, status,
            cpu_cores, cpu_usage_pct: cpu_usage,
            mem_total_mib: mem_total, mem_used_mib: mem_used,
            disk_total_gib: disk_total, disk_used_gib: disk_used,
            net_rx_mbps: rx_mbps, net_tx_mbps: tx_mbps,
            load_score, vm_count, uptime_secs: uptime,
        }
    }
}
