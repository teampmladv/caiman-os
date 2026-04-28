//! compat/adapters/mod.rs — CniAdapter trait + factory
//!
//! Each ecosystem adapter implements CniAdapter:
//!   setup_network    — called after tap creation + IPAM, sets up routes/BGP/OVS/etc.
//!   teardown_network — reverse of setup
//!   check_network    — verify the setup is still valid

use anyhow::Result;
use async_trait::async_trait;

use crate::compat::{CniEnv, EcosystemKind};
use crate::ipam::IpamResult;

pub mod antrea;
pub mod calico;
pub mod cilium;
pub mod flannel;
pub mod sriov;
pub mod standalone;
pub mod weave;

// ── Trait ──────────────────────────────────────────────────────────────────

#[async_trait]
pub trait CniAdapter: Send + Sync {
    /// Called after tap creation and IPAM allocation.
    async fn setup_network(
        &self,
        env:         &CniEnv,
        tap_ifindex: u32,
        tap_mac:     &[u8; 6],
        ip_result:   &IpamResult,
    ) -> Result<()>;

    /// Called on CNI DEL.
    async fn teardown_network(&self, env: &CniEnv) -> Result<()>;

    /// Called on CNI CHECK.
    async fn check_network(&self, env: &CniEnv) -> Result<()>;
}

/// Return the correct adapter for the detected ecosystem.
pub fn get(kind: EcosystemKind) -> Box<dyn CniAdapter> {
    match kind {
        EcosystemKind::Calico     => Box::new(calico::CalicoAdapter),
        EcosystemKind::Cilium     => Box::new(cilium::CiliumAdapter),
        EcosystemKind::Flannel    => Box::new(flannel::FlannelAdapter),
        EcosystemKind::Antrea     => Box::new(antrea::AntreaAdapter),
        EcosystemKind::SrIov      => Box::new(sriov::SrIovAdapter),
        EcosystemKind::Weave      => Box::new(weave::WeaveAdapter),
        EcosystemKind::Standalone => Box::new(standalone::StandaloneAdapter),
    }
}
