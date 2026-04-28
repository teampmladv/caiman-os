//! gpu/src/mig.rs — NVIDIA Multi-Instance GPU partitioning
//!
//! MIG slices provide hardware-level isolation. Each slice has:
//!   - Dedicated SM (streaming multiprocessors)
//!   - Dedicated HBM memory partition
//!   - Dedicated memory bandwidth
//!   - Dedicated L2 cache partition
//!   - CE (copy engine), DEC, ENC engines
//!
//! Profile naming: {compute_instances}g.{memory_gib}gb
//!   A100-80GB: 1g.10gb, 2g.20gb, 3g.40gb, 4g.40gb, 7g.80gb
//!   H100-80GB: 1g.10gb, 2g.20gb, 3g.40gb, 4g.40gb, 7g.80gb

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::info;

use crate::{GpuAllocation, GpuDevice};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigProfile {
    pub name:         String,   // "3g.40gb"
    pub sm_count:     u32,
    pub memory_gib:   u32,
    pub instance_id:  Option<u32>,
}

pub async fn allocate(
    gpu:         GpuDevice,
    profile_str: &str,
    vm_id:       u32,
) -> Result<GpuAllocation> {
    info!("MIG: enabling MIG mode on {}", gpu.pci_address);

    // Enable MIG mode (requires driver restart on some systems)
    enable_mig_mode(&gpu.pci_address).await
        .context("enabling MIG mode")?;

    // Create GPU instance with requested profile
    let instance_id = create_mig_instance(&gpu.gpu_uuid, profile_str).await
        .context("creating MIG instance")?;

    // Create compute instance on the GPU instance
    create_compute_instance(&gpu.gpu_uuid, instance_id, profile_str).await
        .context("creating compute instance")?;

    // Get the vfio device for the MIG instance
    let vfio_dev = mig_instance_to_vfio(&gpu.pci_address, instance_id).await
        .context("getting MIG vfio device")?;

    let profile = MigProfile {
        name:        profile_str.to_string(),
        sm_count:    parse_sm_count(profile_str),
        memory_gib:  parse_memory_gib(profile_str),
        instance_id: Some(instance_id),
    };

    info!("MIG: created slice {} (instance {instance_id}) for VM {vm_id}",
          profile_str);

    Ok(GpuAllocation::MigSlice {
        gpu,
        profile,
        instance_id,
        vfio_dev,
    })
}

pub async fn release(gpu: &GpuDevice, instance_id: u32) -> Result<()> {
    info!("MIG: releasing instance {instance_id} on {}", gpu.pci_address);

    // Destroy compute instance, then GPU instance
    Command::new("nvidia-smi")
        .args(["mig", "-dci", "-gi", &instance_id.to_string()])
        .output().await.ok();

    Command::new("nvidia-smi")
        .args(["mig", "-dgi", "-gi", &instance_id.to_string()])
        .output().await.ok();

    Ok(())
}

async fn enable_mig_mode(pci: &str) -> Result<()> {
    let out = Command::new("nvidia-smi")
        .args(["-i", pci, "-mig", "1"])
        .output().await?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Failed to enable MIG mode: {err}");
    }
    // Some GPUs need a driver restart — check status
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    Ok(())
}

async fn create_mig_instance(gpu_uuid: &str, profile: &str) -> Result<u32> {
    let out = Command::new("nvidia-smi")
        .args(["mig", "-cgi", profile, "-C", "-i", gpu_uuid])
        .output().await?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    // Parse "Successfully created GPU instance ID 1 for..."
    for line in stdout.lines() {
        if line.contains("Successfully created GPU instance ID") {
            if let Some(id_str) = line.split("ID ").nth(1) {
                if let Ok(id) = id_str.split_whitespace().next()
                    .unwrap_or("").parse::<u32>() {
                    return Ok(id);
                }
            }
        }
    }
    bail!("Could not parse MIG instance ID from: {stdout}")
}

async fn create_compute_instance(gpu_uuid: &str, gi_id: u32, profile: &str) -> Result<()> {
    Command::new("nvidia-smi")
        .args(["mig", "-cci", profile, "-gi", &gi_id.to_string(), "-i", gpu_uuid])
        .output().await?;
    Ok(())
}

async fn mig_instance_to_vfio(pci: &str, instance_id: u32) -> Result<String> {
    // MIG instances appear as mdev devices under the GPU's PCI device
    let mdev_dir = format!("/sys/bus/pci/devices/{pci}/mdev_supported_types");
    let entries = std::fs::read_dir(&mdev_dir)?;
    for entry in entries.flatten() {
        let path = entry.path().join("devices");
        if let Ok(devs) = std::fs::read_dir(path) {
            for dev in devs.flatten() {
                let uuid = dev.file_name().to_string_lossy().to_string();
                let iommu_link = dev.path().join("iommu_group");
                if let Ok(target) = std::fs::read_link(iommu_link) {
                    if let Some(group) = target.file_name() {
                        return Ok(format!("/dev/vfio/{}", group.to_string_lossy()));
                    }
                }
            }
        }
    }
    bail!("vfio device not found for MIG instance {instance_id}")
}

fn parse_sm_count(profile: &str) -> u32 {
    profile.split('g').next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn parse_memory_gib(profile: &str) -> u32 {
    profile.split('.').nth(1)
        .and_then(|s| s.strip_suffix("gb"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
}
