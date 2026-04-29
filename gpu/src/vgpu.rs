//! gpu/src/vgpu.rs — NVIDIA vGPU via mediated devices (mdev)
//!
//! vGPU creates virtual GPU instances via the Linux mdev subsystem.
//! Each vGPU is a mediated device (/sys/bus/mdev/devices/<uuid>)
//! that gets passed to the VM via VFIO.
//!
//! Requirements:
//!   - NVIDIA vGPU software (separate license from nvidia.com/vgpu)
//!   - Grid-capable GPU (A10, A16, A30, A40, A100, H100, RTX series)
//!   - NVIDIA vGPU host driver installed

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

use crate::{GpuDevice, GpuAllocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VgpuProfile {
    pub name:       String,     // "A100-40C" (compute), "A100-40Q" (Quadro)
    pub vram_mib:   u32,
    pub max_heads:  u8,
    pub mdev_type:  String,     // "nvidia-105"
    pub framerate:  Option<u32>,
}

/// List available vGPU profiles for this GPU
pub async fn list_profiles(gpu: &GpuDevice) -> Result<Vec<VgpuProfile>> {
    let mdev_types_path = format!(
        "/sys/bus/pci/devices/{}/mdev_supported_types",
        gpu.pci_address
    );

    let mut profiles = Vec::new();
    let dir = std::fs::read_dir(&mdev_types_path)
        .with_context(|| format!("reading mdev types from {mdev_types_path}"))?;

    for entry in dir.flatten() {
        let mdev_type = entry.file_name().to_string_lossy().to_string();
        let base = entry.path();

        let name = std::fs::read_to_string(base.join("name"))
            .unwrap_or_default().trim().to_string();
        let available = std::fs::read_to_string(base.join("available_instances"))
            .unwrap_or_default().trim().parse::<u32>().unwrap_or(0);

        if available == 0 { continue; }

        // Parse memory from name (e.g. "GRID A100-40C" → 40 * 1024 MiB)
        let vram_mib = name.split('-')
            .last()
            .and_then(|s| s.trim_end_matches(char::is_alphabetic).parse::<u32>().ok())
            .unwrap_or(4) * 1024;

        profiles.push(VgpuProfile {
            name,
            vram_mib,
            max_heads: 1,
            mdev_type,
            framerate: None,
        });
    }

    Ok(profiles)
}

/// Allocate a vGPU mediated device for a VM
pub async fn allocate(gpu: GpuDevice, profile_str: &str, vm_id: u32) -> Result<GpuAllocation> {
    info!("vGPU allocate: {} profile={profile_str} vm={vm_id}", gpu.pci_address);

    // Find the mdev type for this profile
    let profiles = list_profiles(&gpu).await?;
    let profile = profiles.iter()
        .find(|p| p.name.to_lowercase().contains(&profile_str.to_lowercase()))
        .with_context(|| format!("vGPU profile '{profile_str}' not found"))?
        .clone();

    // Create mediated device with a new UUID
    let mdev_uuid = Uuid::new_v4().to_string();
    let mdev_create_path = format!(
        "/sys/bus/pci/devices/{}/mdev_supported_types/{}/create",
        gpu.pci_address, profile.mdev_type
    );

    std::fs::write(&mdev_create_path, &mdev_uuid)
        .with_context(|| format!("creating mdev device at {mdev_create_path}"))?;

    let mdev_path = format!("/sys/bus/mdev/devices/{mdev_uuid}");
    anyhow::ensure!(
        std::path::Path::new(&mdev_path).exists(),
        "mdev device {mdev_path} not created"
    );

    // Get vGPU ID for tracking
    let vgpu_id = vm_id; // use VM ID as vGPU ID for simplicity

    info!("vGPU created: uuid={mdev_uuid} profile={}", profile.name);

    Ok(GpuAllocation::VGpu {
        gpu,
        profile,
        vgpu_id,
        mdev_path,
    })
}

/// Release a vGPU mediated device
pub async fn release(gpu: &GpuDevice, vgpu_id: u32) -> Result<()> {
    info!("Releasing vGPU id={vgpu_id} on {}", gpu.pci_address);

    // Find the mdev device by scanning /sys/bus/mdev/devices/
    let mdev_dir = "/sys/bus/mdev/devices";
    if let Ok(entries) = std::fs::read_dir(mdev_dir) {
        for entry in entries.flatten() {
            let mdev_path = entry.path();
            // Check if this mdev belongs to our GPU
            let parent = mdev_path.join("../..").canonicalize().ok();
            if parent.as_deref().and_then(|p| p.to_str())
                .map(|s| s.contains(&gpu.pci_address))
                .unwrap_or(false)
            {
                let remove_path = mdev_path.join("remove");
                let _ = std::fs::write(remove_path, "1");
                info!("vGPU mdev removed: {}", mdev_path.display());
            }
        }
    }

    Ok(())
}

/// Get the VFIO device path for a vGPU mdev
pub fn vfio_args_from_mdev(mdev_path: &str) -> Vec<String> {
    vec![
        "--vfio-mdev".into(),
        mdev_path.to_string(),
    ]
}
