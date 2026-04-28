//! livemig/src/memory.rs — dirty page tracking + TCP transfer
//!
//! Communicates with caiman-vmm via its control socket at
//! /var/run/caiman/{vm_id}.sock
//! to get dirty pages, pause vCPUs, and transfer memory state.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::proto::{MigStream, MsgType};

/// Control message to caiman-vmm via Unix socket
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum VmmCtrl {
    EnableDirtyTracking,
    GetDirtyLog,
    PauseVcpus,
    ResumeVcpus,
    GetVcpuState,
    GetMemoryRegions,
}

/// Response from caiman-vmm control socket
#[derive(Debug, Serialize, Deserialize)]
pub struct VmmResponse {
    pub ok:      bool,
    pub message: Option<String>,
    pub data:    Option<serde_json::Value>,
}

/// Memory region descriptor from the VMM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemRegion {
    pub slot:       u32,
    pub guest_addr: u64,
    pub size:       u64,
}

fn ctrl_path(vm_id: u32) -> PathBuf {
    PathBuf::from(format!("/var/run/caiman/{vm_id}.sock"))
}

fn send_ctrl(vm_id: u32, cmd: VmmCtrl) -> Result<VmmResponse> {
    let path = ctrl_path(vm_id);
    let mut stream = UnixStream::connect(&path)
        .with_context(|| format!("connecting to VMM socket {}", path.display()))?;

    let msg = serde_json::to_string(&cmd)? + "\n";
    stream.write_all(msg.as_bytes())?;

    let mut buf = String::new();
    let mut byte = [0u8];
    loop {
        stream.read_exact(&mut byte)?;
        if byte[0] == b'\n' { break; }
        buf.push(byte[0] as char);
    }
    serde_json::from_str(&buf).context("parsing VMM response")
}

/// Enable KVM dirty page tracking on the source VM
pub async fn enable_dirty_tracking(vm_id: u32) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        send_ctrl(vm_id, VmmCtrl::EnableDirtyTracking)
    }).await??;
    info!("Dirty page tracking enabled for VM {vm_id}");
    Ok(())
}

/// Get dirty pages from VMM, send them over the migration stream
/// Returns the number of dirty pages sent
pub async fn copy_dirty_pages(
    vm_id:         u32,
    stream:        &mut MigStream,
    bandwidth_mbps:u64,
) -> Result<u64> {
    let resp = tokio::task::spawn_blocking(move || {
        send_ctrl(vm_id, VmmCtrl::GetDirtyLog)
    }).await??;

    let pages = resp.data
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default();

    let count = pages.len() as u64;
    let mut sent = 0u64;

    // Rate limiting: bytes per tick
    let tick_us   = 10_000u64; // 10ms tick
    let bytes_per_tick = if bandwidth_mbps > 0 {
        bandwidth_mbps * 1_000_000 / 8 * tick_us / 1_000_000
    } else {
        u64::MAX
    };
    let mut tick_bytes = 0u64;
    let mut tick_start = std::time::Instant::now();

    for page in &pages {
        let gpa  = page["gpa"].as_u64().unwrap_or(0);
        let data = page["data"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|b| b as u8)).collect())
            .unwrap_or_else(|| vec![0u8; 4096]);

        stream.send_page(gpa, &data).await?;
        sent += 1;
        tick_bytes += 4096;

        // Rate limiting
        if bandwidth_mbps > 0 && tick_bytes >= bytes_per_tick {
            let elapsed = tick_start.elapsed().as_micros() as u64;
            if elapsed < tick_us {
                tokio::time::sleep(
                    std::time::Duration::from_micros(tick_us - elapsed)
                ).await;
            }
            tick_bytes = 0;
            tick_start = std::time::Instant::now();
        }
    }

    debug!("Copied {sent}/{count} dirty pages");
    Ok(count - sent)
}

/// Copy ALL remaining dirty pages (final pass before resume)
pub async fn copy_all_remaining(vm_id: u32, stream: &mut MigStream) -> Result<u64> {
    copy_dirty_pages(vm_id, stream, 0).await
}

/// Pause all vCPUs on the source VM
pub async fn pause_vm(vm_id: u32) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        send_ctrl(vm_id, VmmCtrl::PauseVcpus)
    }).await??;
    info!("VM {vm_id} paused");
    Ok(())
}

/// Transfer vCPU register state to destination
pub async fn transfer_vcpu_state(vm_id: u32, stream: &mut MigStream) -> Result<()> {
    let resp = tokio::task::spawn_blocking(move || {
        send_ctrl(vm_id, VmmCtrl::GetVcpuState)
    }).await??;

    let state = resp.data.unwrap_or(serde_json::json!({}));
    stream.send_vcpu_state(&state).await?;
    info!("vCPU state transferred");
    Ok(())
}
