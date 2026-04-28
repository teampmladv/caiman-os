//! compat/mod.rs — CNI ecosystem detection and shared types

use anyhow::{bail, Result};
use tracing::debug;

use crate::CniConfig;

pub mod adapters;
pub mod chain;
pub mod ipam;
pub mod multus;

// ── Shared environment passed to every adapter ──────────────────────────────

#[derive(Debug, Clone)]
pub struct CniEnv {
    pub command:      String,
    pub container_id: String,
    pub netns:        String,
    pub if_name:      String,
    pub config:       CniConfig,
}

// ── Ecosystem kinds ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcosystemKind {
    Standalone,   // host-local IPAM, pure caiman XDP
    Calico,       // BGP routes via Felix, Calico IPAM / NetworkPolicy
    Cilium,       // eBPF-native, XDP coexistence via TC fallback
    Flannel,      // VXLAN / host-gw overlay
    Antrea,       // OVS-based
    SrIov,        // Hardware VF passthrough via vfio-pci
    Weave,        // Mesh overlay with optional WireGuard encryption
}

impl EcosystemKind {
    /// Auto-detect the active CNI ecosystem by probing well-known sockets/files.
    pub async fn detect(config: &CniConfig) -> Self {
        // SR-IOV: device ID in config or SRIOV_RESOURCE env
        if config.device_id.is_some()
            || std::env::var("SRIOV_RESOURCE").is_ok()
        {
            debug!("detected: SR-IOV");
            return Self::SrIov;
        }

        // Cilium: agent socket at well-known path
        if std::path::Path::new("/var/run/cilium/cilium.sock").exists() {
            debug!("detected: Cilium");
            return Self::Cilium;
        }

        // Calico: Felix socket or CNI config marker
        if std::path::Path::new("/var/run/calico/felix.sock").exists()
            || std::path::Path::new("/etc/cni/net.d/10-calico.conflist").exists()
        {
            debug!("detected: Calico");
            return Self::Calico;
        }

        // Antrea: agent socket
        if std::path::Path::new("/var/run/antrea/antrea-agent.sock").exists() {
            debug!("detected: Antrea");
            return Self::Antrea;
        }

        // Flannel: subnet environment file
        if std::path::Path::new("/run/flannel/subnet.env").exists()
            || std::path::Path::new("/etc/cni/net.d/10-flannel.conflist").exists()
        {
            debug!("detected: Flannel");
            return Self::Flannel;
        }

        // Weave: weave socket
        if std::path::Path::new("/var/run/weave/weave.sock").exists() {
            debug!("detected: Weave");
            return Self::Weave;
        }

        debug!("no CNI ecosystem detected, using Standalone mode");
        Self::Standalone
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "standalone" => Ok(Self::Standalone),
            "calico"     => Ok(Self::Calico),
            "cilium"     => Ok(Self::Cilium),
            "flannel"    => Ok(Self::Flannel),
            "antrea"     => Ok(Self::Antrea),
            "sriov" | "sr-iov" => Ok(Self::SrIov),
            "weave"      => Ok(Self::Weave),
            other => bail!("unknown CNI adapter: {other}"),
        }
    }
}

/// Derive a stable u32 VM ID from the container ID (first 4 bytes of FNV hash).
pub fn stable_vm_id(container_id: &str) -> u32 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in container_id.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (hash & 0xffff) as u32
}
