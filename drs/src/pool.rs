//! drs/src/pool.rs — Resource pools (hierarchical resource allocation)
//!
//! Equivalent to vSphere Resource Pools.
//! Each pool has CPU and memory reservations, limits, and shares.
//!
//!   Reservation: guaranteed minimum allocation
//!   Limit:       hard maximum (can be -1 = unlimited)
//!   Shares:      relative weight when cluster is contended (Low=1, Normal=2, High=4, Custom=N)
//!
//! VMs are placed into pools via annotation:
//!   caiman.io/resource-pool: "production/tier1"

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePool {
    pub name:     String,
    pub parent:   Option<String>,          // path to parent pool ("production")
    pub cpu:      ResourceAllocation,
    pub memory:   ResourceAllocation,
    pub vms:      Vec<u32>,                // VM IDs in this pool
    pub children: Vec<String>,             // child pool names
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// Guaranteed minimum (MHz for CPU, MiB for memory). 0 = no reservation.
    pub reservation: u64,
    /// Hard maximum. u64::MAX = unlimited.
    pub limit:       u64,
    /// Relative shares (used during contention).
    pub shares:      Shares,
    /// Expandable reservation: child pools can use parent's unreserved capacity.
    pub expandable:  bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Shares {
    Low,     // weight = 1
    Normal,  // weight = 2 (default)
    High,    // weight = 4
    Custom(u32),
}

impl Shares {
    pub fn weight(&self) -> u32 {
        match self {
            Self::Low        => 1,
            Self::Normal     => 2,
            Self::High       => 4,
            Self::Custom(w)  => *w,
        }
    }
}

impl Default for ResourceAllocation {
    fn default() -> Self {
        Self {
            reservation: 0,
            limit:       u64::MAX,
            shares:      Shares::Normal,
            expandable:  true,
        }
    }
}

// ── Pool registry ─────────────────────────────────────────────────────────

pub struct PoolRegistry {
    pools: HashMap<String, ResourcePool>,
}

impl PoolRegistry {
    pub fn new() -> Self {
        let mut pools = HashMap::new();
        // Always create a root pool
        pools.insert("root".into(), ResourcePool {
            name:     "root".into(),
            parent:   None,
            cpu:      ResourceAllocation::default(),
            memory:   ResourceAllocation::default(),
            vms:      Vec::new(),
            children: Vec::new(),
        });
        Self { pools }
    }

    pub fn get_pool_for_vm(&self, vm_labels: &HashMap<String, String>) -> Option<&ResourcePool> {
        let pool_name = vm_labels.get("caiman.io/resource-pool")?;
        self.pools.get(pool_name.as_str())
    }

    /// Check if a VM can be admitted to its pool given current usage.
    pub fn can_admit(&self, pool_name: &str, cpu_mhz: u64, mem_mib: u64) -> bool {
        let Some(pool) = self.pools.get(pool_name) else { return true };

        // Check CPU reservation available
        let used_cpu: u64 = pool.vms.len() as u64 * 1000; // simplified
        if pool.cpu.reservation > 0 && used_cpu + cpu_mhz > pool.cpu.limit {
            return false;
        }

        // Check memory limit
        let used_mem: u64 = pool.vms.iter().count() as u64 * 512; // simplified
        if pool.memory.limit != u64::MAX && used_mem + mem_mib > pool.memory.limit {
            return false;
        }

        true
    }
}
