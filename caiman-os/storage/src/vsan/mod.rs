//! storage/src/vsan/mod.rs — caiman VSAN (Virtual SAN)
//!
//! Distributed block storage across cluster nodes using local NVMe/SSD disks.
//! Equivalent to VMware VSAN / Ceph but designed for caiman VMs.
//!
//! Architecture:
//!   - Each node contributes local disks to the shared storage pool
//!   - Data is replicated across N nodes (configurable per StoragePolicy)
//!   - VMs see a virtio-blk device backed by VSAN volumes
//!   - Control plane via Kubernetes StorageClass + CSI driver
//!   - Data plane: NVMe-oF over RDMA/TCP between nodes
//!
//! Replication models:
//!   FTT=1 (default): 2 copies across 2 nodes + 1 witness
//!   FTT=2:           3 copies across 3 nodes + 1 witness
//!   RAID-5/6:        erasure coding for capacity efficiency

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub mod csi;
pub mod nvmeof;
pub mod policy;
pub mod replication;

// ── Storage policy ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePolicy {
    pub name:             String,
    pub failures_to_tolerate: u8,   // FTT: 0, 1, or 2
    pub raid_type:        RaidType,
    pub stripe_width:     u8,       // for RAID-5/6
    pub iops_limit:       Option<u64>,
    pub throughput_limit: Option<u64>,  // bytes/sec
    pub encryption:       bool,
    pub compression:      bool,
    pub deduplication:    bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RaidType {
    Mirroring,   // FTT=1→RAID-1, FTT=2→RAID-1 with 3 copies
    Erasure5,    // RAID-5 (FTT=1, min 3 nodes)
    Erasure6,    // RAID-6 (FTT=2, min 4 nodes)
}

impl Default for StoragePolicy {
    fn default() -> Self {
        Self {
            name:                 "default".into(),
            failures_to_tolerate: 1,
            raid_type:            RaidType::Mirroring,
            stripe_width:         1,
            iops_limit:           None,
            throughput_limit:     None,
            encryption:           false,
            compression:          true,
            deduplication:        false,
        }
    }
}

// ── VSAN volume ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsanVolume {
    pub id:        String,    // UUID
    pub name:      String,
    pub size_gib:  u64,
    pub policy:    StoragePolicy,
    pub components: Vec<VolumeComponent>,
    pub state:     VolumeState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeComponent {
    pub node:       String,   // node hostname
    pub disk:       String,   // /dev/nvme0n1 etc.
    pub offset_gib: u64,
    pub replica_id: u8,       // 0=primary, 1,2=replicas, 255=witness
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VolumeState { Healthy, Degraded, Offline, Resyncing }

// ── VSAN cluster ──────────────────────────────────────────────────────────

pub struct VsanCluster {
    volumes: RwLock<HashMap<String, VsanVolume>>,
    nodes:   RwLock<Vec<VsanNode>>,
}

#[derive(Debug, Clone)]
pub struct VsanNode {
    pub hostname:     String,
    pub nvmeof_addr:  String,  // IP:port for NVMe-oF target
    pub disks:        Vec<DiskInfo>,
    pub capacity_gib: u64,
    pub used_gib:     u64,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub path:         String,
    pub model:        String,
    pub capacity_gib: u64,
    pub nvme_ns_id:   Option<u32>,
}

impl VsanCluster {
    pub fn new() -> Self {
        Self {
            volumes: RwLock::new(HashMap::new()),
            nodes:   RwLock::new(Vec::new()),
        }
    }

    /// Create a new distributed volume, placing components across nodes.
    pub async fn create_volume(
        &self,
        name:     &str,
        size_gib: u64,
        policy:   StoragePolicy,
    ) -> Result<VsanVolume> {
        let nodes = self.nodes.read().await;
        let required_nodes = match policy.failures_to_tolerate {
            0 => 1,
            1 => if policy.raid_type == RaidType::Erasure5 { 3 } else { 2 },
            2 => if policy.raid_type == RaidType::Erasure6 { 4 } else { 3 },
            n => bail!("FTT={n} not supported (max 2)"),
        };

        if nodes.len() < required_nodes {
            bail!(
                "Not enough nodes: need {required_nodes}, have {}. \
                 FTT={} requires {} nodes.",
                nodes.len(), policy.failures_to_tolerate, required_nodes
            );
        }

        // Select nodes with most free space
        let mut sorted_nodes = nodes.clone();
        sorted_nodes.sort_by_key(|n| n.capacity_gib - n.used_gib);
        sorted_nodes.reverse();

        let mut components = Vec::new();
        for (i, node) in sorted_nodes.iter().take(required_nodes).enumerate() {
            let disk = node.disks.first()
                .context("node has no disks")?;
            components.push(VolumeComponent {
                node:       node.hostname.clone(),
                disk:       disk.path.clone(),
                offset_gib: node.used_gib,
                replica_id: i as u8,
            });
        }

        let vol = VsanVolume {
            id:         uuid::Uuid::new_v4().to_string(),
            name:       name.to_string(),
            size_gib,
            policy,
            components,
            state:      VolumeState::Healthy,
        };

        info!("VSAN: created volume {} ({size_gib} GiB, FTT={})",
              vol.id, vol.policy.failures_to_tolerate);

        self.volumes.write().await.insert(vol.id.clone(), vol.clone());
        Ok(vol)
    }

    /// Attach a volume to a VM via NVMe-oF TCP initiator.
    pub async fn attach_volume(&self, vol_id: &str, vm_id: u32) -> Result<String> {
        let volumes = self.volumes.read().await;
        let vol = volumes.get(vol_id)
            .with_context(|| format!("volume {vol_id} not found"))?;

        // Connect to the primary component's NVMe-oF target
        let primary = vol.components.iter()
            .find(|c| c.replica_id == 0)
            .context("no primary component")?;

        let dev_path = nvmeof::connect_initiator(
            &primary.node, vol_id, vm_id
        ).await
        .context("NVMe-oF connect")?;

        info!("VSAN: volume {vol_id} attached to VM {vm_id} as {dev_path}");
        Ok(dev_path)
    }

    /// Handle a node failure: start resyncing affected volumes.
    pub async fn handle_node_failure(&self, failed_node: &str) {
        warn!("VSAN: node {failed_node} failed — starting resync");
        let mut volumes = self.volumes.write().await;
        for vol in volumes.values_mut() {
            let affected = vol.components.iter()
                .any(|c| c.node == failed_node);
            if affected {
                vol.state = VolumeState::Degraded;
                tokio::spawn(replication::resync_volume(vol.id.clone()));
            }
        }
    }
}
