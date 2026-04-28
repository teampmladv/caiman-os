//! collectors/xdp.rs — XDP stats from caiman_net via /sys
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct XdpStats {
    pub rx_packets:    u64,
    pub tx_packets:    u64,
    pub drops:         u64,
    pub throughput_gbps: f64,
}

/// Read XDP stats from sysfs (caiman_net.ko exposes them at /sys/module/caiman_net/stats/)
pub async fn collect() -> Result<XdpStats> {
    let base = "/sys/module/caiman_net/stats";
    if !std::path::Path::new(base).exists() {
        // Module not loaded — return zeros
        return Ok(XdpStats::default());
    }
    let rx    = read_u64(&format!("{base}/rx_packets")).unwrap_or(0);
    let tx    = read_u64(&format!("{base}/tx_packets")).unwrap_or(0);
    let drops = read_u64(&format!("{base}/drops")).unwrap_or(0);
    Ok(XdpStats { rx_packets: rx, tx_packets: tx, drops, throughput_gbps: 0.0 })
}

fn read_u64(path: &str) -> Result<u64> {
    Ok(std::fs::read_to_string(path)?.trim().parse()?)
}

/// Collect all XDP stats (called by CollectorLoop)
pub async fn collect_all() -> Result<XdpStats> {
    collect().await
}
