//! standalone.rs — default CNI mode (caiman manages the interface directly)
use anyhow::Result;
use crate::tap;
use crate::ipam;

pub async fn add(container_id: &str, ifname: &str, netns: &str) -> Result<()> {
    let tap_name = format!("caiman{}", &container_id[..6]);
    tap::create_tap(&tap_name, "caiman0")?;
    let _ip = ipam::allocate("host-local", "{}").await?;
    Ok(())
}

pub async fn del(container_id: &str, ifname: &str) -> Result<()> {
    let tap_name = format!("caiman{}", &container_id[..6]);
    tap::delete_tap(&tap_name)?;
    Ok(())
}
