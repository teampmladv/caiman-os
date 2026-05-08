//! ipam.rs -- IPAM propio sin dependencias externas
//! Gestiona un pool de IPs en /var/lib/caiman/ipam/

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::Ipv4Addr;

const IPAM_DIR:  &str = "/var/lib/caiman/ipam";
const SUBNET:    &str = "10.100.0.0/24";
const GATEWAY:   Ipv4Addr = Ipv4Addr::new(10, 100, 0, 1);
const IP_START:  u32 = 0x0A640002; // 10.100.0.2
const IP_END:    u32 = 0x0A6400FE; // 10.100.0.254

pub struct AllocatedIp {
    pub ip:      String,
    pub gateway: String,
    pub subnet:  String,
    pub prefix:  u8,
}

fn load_allocations() -> HashMap<String, String> {
    std::fs::create_dir_all(IPAM_DIR).ok();
    let path = format!("{IPAM_DIR}/allocations.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_allocations(map: &HashMap<String, String>) {
    let path = format!("{IPAM_DIR}/allocations.json");
    if let Ok(json) = serde_json::to_string_pretty(map) {
        std::fs::write(path, json).ok();
    }
}

pub fn allocate(vm_id: &str) -> Result<AllocatedIp> {
    let mut allocs = load_allocations();

    // Return existing allocation if present
    if let Some(ip) = allocs.get(vm_id) {
        return Ok(AllocatedIp {
            ip:      ip.clone(),
            gateway: GATEWAY.to_string(),
            subnet:  SUBNET.to_string(),
            prefix:  24,
        });
    }

    // Find next free IP
    let used: std::collections::HashSet<u32> = allocs.values()
        .filter_map(|ip| ip.parse::<Ipv4Addr>().ok())
        .map(|ip| u32::from(ip))
        .collect();

    let next = (IP_START..=IP_END)
        .find(|ip| !used.contains(ip))
        .context("IPAM pool exhausted -- no free IPs in 10.100.0.0/24")?;

    let ip = Ipv4Addr::from(next).to_string();
    allocs.insert(vm_id.to_string(), ip.clone());
    save_allocations(&allocs);

    Ok(AllocatedIp {
        ip,
        gateway: GATEWAY.to_string(),
        subnet:  SUBNET.to_string(),
        prefix:  24,
    })
}

pub fn release(vm_id: &str) {
    let mut allocs = load_allocations();
    allocs.remove(vm_id);
    save_allocations(&allocs);
}

pub fn list() -> HashMap<String, String> {
    load_allocations()
}
