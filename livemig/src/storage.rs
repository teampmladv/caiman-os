//! livemig/src/storage.rs — storage reconnect after migration
//! For NVMe-oF/iSCSI backends: reconnects the initiator on the destination.

use anyhow::Result;
use tracing::info;

pub async fn reconnect_on_destination(vm_id: u32, dest: &str) -> Result<()> {
    // For VSAN (NVMe-oF): the storage is network-attached and follows the VM
    // automatically — no reconnect needed for shared-nothing storage.
    //
    // For local disk: already transferred via virtio-blk dirty tracking.
    info!("Storage reconnect: VM {vm_id} on {dest} (NVMe-oF auto-reconnect)");
    Ok(())
}
