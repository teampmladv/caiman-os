//! demo/state.rs — in-memory VM simulation for Railway demo mode
//!
//! When DEMO_MODE=true (Railway, no /dev/kvm):
//!   - VMs are stored in memory (no /var/run/caiman)
//!   - Metrics are realistically randomized
//!   - VM status transitions: BOOTING → RUNNING after 3 seconds
//!   - No real processes spawned

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use rand::Rng;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmStatus { Running, Stopped, Booting, Error }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoVm {
    pub id:            String,
    pub name:          String,
    pub status:        VmStatus,
    pub cpus:          u8,
    pub mem_mib:       u64,
    pub node_name:     String,
    pub kernel:        String,
    pub disk:          Option<String>,
    pub mac:           String,
    pub uplink:        String,
    pub cpu_usage_pct: f64,
    pub mem_used_mib:  u64,
    pub net_rx_mbps:   f64,
    pub net_tx_mbps:   f64,
    pub uptime_secs:   u64,
    pub created_at:    DateTime<Utc>,
    pub started_at:    Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoNode {
    pub hostname:       String,
    pub status:         String,
    pub cpu_cores:      u32,
    pub cpu_usage_pct:  f64,
    pub mem_total_mib:  u64,
    pub mem_used_mib:   u64,
    pub disk_total_gib: u64,
    pub disk_used_gib:  u64,
    pub net_rx_mbps:    f64,
    pub net_tx_mbps:    f64,
    pub load_score:     f64,
    pub vm_count:       usize,
    pub uptime_secs:    u64,
}

pub struct DemoStore {
    vms:      HashMap<String, DemoVm>,
    boot_time: std::time::Instant,
}

pub type SharedDemo = Arc<RwLock<DemoStore>>;

impl DemoStore {
    pub fn new() -> Self {
        let mut store = Self {
            vms: HashMap::new(),
            boot_time: std::time::Instant::now(),
        };
        // Pre-populate with one demo VM so the page isn't empty
        let id = "demo-preloaded-01".to_string();
        store.vms.insert(id.clone(), DemoVm {
            id,
            name: "demo-workload".into(),
            status: VmStatus::Running,
            cpus: 4,
            mem_mib: 1024,
            node_name: "demo-node-01".into(),
            kernel: "/var/lib/caiman/vmlinuz".into(),
            disk: None,
            mac: "02:aa:bb:00:00:01".into(),
            uplink: "eth0".into(),
            cpu_usage_pct: 23.4,
            mem_used_mib: 412,
            net_rx_mbps: 1.24,
            net_tx_mbps: 0.87,
            uptime_secs: 3600,
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
        });
        store
    }

    pub fn create_vm(&mut self, name: String, cpus: u8, mem_mib: u64) -> DemoVm {
        let id  = format!("vm-{}", &Uuid::new_v4().to_string()[..8]);
        let num = self.vms.len() + 1;
        let vm  = DemoVm {
            id:  id.clone(),
            name,
            status:        VmStatus::Booting,
            cpus,
            mem_mib,
            node_name:     "demo-node-01".into(),
            kernel:        "/var/lib/caiman/vmlinuz".into(),
            disk:          None,
            mac:           format!("02:aa:bb:00:00:{:02x}", num),
            uplink:        "eth0".into(),
            cpu_usage_pct: 0.0,
            mem_used_mib:  0,
            net_rx_mbps:   0.0,
            net_tx_mbps:   0.0,
            uptime_secs:   0,
            created_at:    Utc::now(),
            started_at:    None,
        };
        self.vms.insert(id, vm.clone());
        vm
    }

    pub fn list_vms(&self) -> Vec<DemoVm> {
        let mut v: Vec<_> = self.vms.values().cloned().collect();
        v.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        v
    }

    pub fn get_vm(&self, id: &str) -> Option<DemoVm> {
        self.vms.get(id).cloned()
    }

    pub fn stop_vm(&mut self, id: &str) {
        if let Some(vm) = self.vms.get_mut(id) {
            vm.status = VmStatus::Stopped;
        }
    }

    pub fn start_vm(&mut self, id: &str) {
        if let Some(vm) = self.vms.get_mut(id) {
            vm.status = VmStatus::Booting;
        }
    }

    pub fn delete_vm(&mut self, id: &str) {
        self.vms.remove(id);
    }

    pub fn transition_booting(&mut self) {
        let uptime = self.boot_time.elapsed().as_secs();
        let mut rng = rand::thread_rng();
        for vm in self.vms.values_mut() {
            match vm.status {
                VmStatus::Booting => {
                    // Transition to Running after 3s
                    if Utc::now()
                        .signed_duration_since(vm.created_at)
                        .num_seconds() > 3
                    {
                        vm.status        = VmStatus::Running;
                        vm.started_at    = Some(Utc::now());
                        vm.cpu_usage_pct = rng.gen_range(5.0..30.0);
                        vm.mem_used_mib  = vm.mem_mib * rng.gen_range(20..45) / 100;
                    }
                }
                VmStatus::Running => {
                    // Jitter metrics realistically
                    vm.cpu_usage_pct = (vm.cpu_usage_pct
                        + rng.gen_range(-3.0..3.0)).clamp(2.0, 85.0);
                    vm.mem_used_mib  = (vm.mem_used_mib as i64
                        + rng.gen_range(-10..10))
                        .max(128).min(vm.mem_mib as i64 * 9 / 10) as u64;
                    vm.net_rx_mbps   = (vm.net_rx_mbps
                        + rng.gen_range(-0.1..0.2)).max(0.0);
                    vm.net_tx_mbps   = (vm.net_tx_mbps
                        + rng.gen_range(-0.05..0.1)).max(0.0);
                    vm.uptime_secs   = Utc::now()
                        .signed_duration_since(vm.started_at.unwrap_or(vm.created_at))
                        .num_seconds().max(0) as u64;
                }
                _ => {}
            }
        }
    }

    pub fn node_metrics(&self) -> DemoNode {
        let mut rng = rand::thread_rng();
        let vms     = self.list_vms();
        let running = vms.iter().filter(|v| v.status == VmStatus::Running).count();
        let cpu_sum: f64 = vms.iter().map(|v| v.cpu_usage_pct).sum();
        let mem_sum: u64 = vms.iter().map(|v| v.mem_used_mib).sum();

        DemoNode {
            hostname:       "demo-node-01".into(),
            status:         "HEALTHY".into(),
            cpu_cores:      16,
            cpu_usage_pct:  (cpu_sum / 16.0 + rng.gen_range(5.0..15.0)).clamp(5.0, 80.0),
            mem_total_mib:  65536,
            mem_used_mib:   mem_sum + 4096,
            disk_total_gib: 2000,
            disk_used_gib:  340,
            net_rx_mbps:    vms.iter().map(|v| v.net_rx_mbps).sum::<f64>() + rng.gen_range(0.5..2.0),
            net_tx_mbps:    vms.iter().map(|v| v.net_tx_mbps).sum::<f64>() + rng.gen_range(0.2..1.0),
            load_score:     (cpu_sum / 16.0 / 100.0).clamp(0.05, 0.8),
            vm_count:       running,
            uptime_secs:    self.boot_time.elapsed().as_secs() + 86400,
        }
    }
}
