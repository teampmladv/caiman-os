//! livemig/src/coord.rs — source/destination coordination
//!
//! Source connects to destination API to:
//!   1. Tell it to prepare a VM shell (same config, no vCPUs yet)
//!   2. Open the migration TCP stream
//!   3. Signal it to start the VM after state transfer

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::proto::{self, MigStream, MsgType};
use crate::MigrationJob;

#[derive(Debug, Serialize, Deserialize)]
pub struct DestShell {
    pub vm_id:      u32,
    pub ram_bytes:  u64,
    pub dest_node:  String,
}

/// Prepare destination and open migration TCP stream.
/// Returns (stream, dest_shell)
pub async fn setup(job: &MigrationJob) -> Result<(MigStream, DestShell)> {
    // 1. Tell destination API to prepare the VM shell
    let client = reqwest::Client::new();
    let dest_api = format!("http://{}:8765", job.dest);

    let shell_resp = client
        .post(format!("{dest_api}/api/migrate/prepare"))
        .json(&serde_json::json!({
            "vmId":     job.vm_id,
            "ramBytes": job.total_ram,
        }))
        .send()
        .await
        .context("notifying destination API")?;

    let shell: DestShell = shell_resp.json().await
        .context("parsing destination shell response")?;

    info!("Destination {} prepared VM shell", job.dest);

    // 2. Connect migration TCP stream
    let mut stream = proto::connect(&job.dest).await?;

    // 3. Handshake
    stream.send_hello(&serde_json::json!({
        "vmId":    job.vm_id,
        "ramMib":  job.total_ram / (1024 * 1024),
    })).await?;

    stream.expect(MsgType::Ready).await
        .context("waiting for destination READY")?;
    info!("Migration stream established");

    Ok((stream, shell))
}

/// Tell the destination to start running the VM
pub async fn start_destination_vm(stream: &mut MigStream) -> Result<()> {
    stream.send_done().await?;
    stream.expect(MsgType::Running).await
        .context("waiting for destination VM RUNNING")?;
    info!("Destination VM started");
    Ok(())
}

/// Clean up source VM after successful migration
pub async fn cleanup_source(vm_id: u32) -> Result<()> {
    // Delete the source VM via local API
    let client = reqwest::Client::new();
    client
        .delete(format!("http://localhost:8765/api/vms/{vm_id}"))
        .send()
        .await
        .context("deleting source VM")?;
    info!("Source VM {vm_id} cleaned up");
    Ok(())
}

/// Destination side: receive and reconstruct VM state
pub async fn receive_migration(listener: &tokio::net::TcpListener) -> Result<()> {
    let (tcp_stream, src_addr) = listener.accept().await?;
    info!("Incoming migration from {src_addr}");
    let mut stream = MigStream::new(tcp_stream);

    // Handshake
    let hello_payload = stream.expect(MsgType::Hello).await?;
    let config: serde_json::Value = serde_json::from_slice(&hello_payload)?;
    info!("Migration config: {config}");
    stream.send_ready().await?;

    // Receive pages
    let mut pages_received = 0u64;
    loop {
        let (msg_type, payload) = stream.recv_msg().await?;
        match msg_type {
            MsgType::Page => {
                // First 8 bytes: GPA, rest: page data
                if payload.len() >= 8 {
                    let _gpa = u64::from_be_bytes(payload[0..8].try_into().unwrap());
                    let _data = &payload[8..];
                    // TODO: write to destination VM's guest memory
                    pages_received += 1;
                }
            }
            MsgType::Pause => {
                info!("Source paused — {pages_received} pages received so far");
            }
            MsgType::VcpuState => {
                let state: serde_json::Value = serde_json::from_slice(&payload)?;
                info!("vCPU state received: {} vCPUs", state.as_array().map(|a| a.len()).unwrap_or(0));
                // TODO: apply to destination vCPUs
            }
            MsgType::Done => {
                info!("Migration complete: {pages_received} pages total");
                stream.send_running().await?;
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
