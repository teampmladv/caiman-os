//! compat/adapters/standalone.rs — default adapter (no external ecosystem)
use async_trait::async_trait;
use anyhow::Result;
use crate::compat::{CniEnv, ipam::IpamResult};

pub struct StandaloneAdapter;

#[async_trait]
impl super::CniAdapter for StandaloneAdapter {
    async fn setup_network(&self, env: &CniEnv, _tap_ifindex: u32,
                           _tap_mac: &[u8; 6], _ip: &IpamResult) -> Result<()> {
        Ok(())
    }
    async fn teardown_network(&self, _env: &CniEnv) -> Result<()> { Ok(()) }
    async fn check_network(&self, _env: &CniEnv) -> Result<()> { Ok(()) }
}
