//! gpu/src/main.rs — NVIDIA MIG + vGPU integration for caiman VMs
//!
//! Three GPU sharing modes (selected per VM via annotation):
//!
//!   1. Full passthrough (gpu.caiman.io/mode: passthrough)
//!      Entire GPU passed to VM via VFIO-PCI. Exclusive access.
//!      Best performance, no sharing.
//!
//!   2. MIG slice  (gpu.caiman.io/mode: mig, gpu.caiman.io/profile: 3g.40gb)
//!      NVIDIA Multi-Instance GPU: hardware-partitioned GPU slices.
//!      Each slice has dedicated SM, memory, and bandwidth.
//!      Isolation guaranteed in hardware. Requires Ampere+ (A100/A30/H100).
//!
//!   3. vGPU (gpu.caiman.io/mode: vgpu, gpu.caiman.io/profile: A100-40C)
//!      NVIDIA vGPU driver: time-sliced or SR-IOV vGPU.
//!      Requires NVIDIA vGPU software license and signed drivers.
//!      Provides best density; isolation via driver.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn};

pub mod mig;
pub mod passthrough;
pub mod vgpu;

// ── GPU device descriptor ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    pub pci_address:  String,         // "0000:01:00.0"
    pub gpu_uuid:     String,         // "GPU-xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
    pub model:        String,         // "NVIDIA A100-SXM4-80GB"
    pub vram_mib:     u64,
    pub driver_ver:   String,
    pub cuda_ver:     String,
    pub mig_capable:  bool,
    pub vgpu_capable: bool,
    pub iommu_group:  u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuAllocation {
    Passthrough {
        gpu:          GpuDevice,
        vfio_dev:     String,          // "/dev/vfio/15"
    },
    MigSlice {
        gpu:          GpuDevice,
        profile:      mig::MigProfile,
        instance_id:  u32,
        vfio_dev:     String,
    },
    VGpu {
        gpu:          GpuDevice,
        profile:      vgpu::VgpuProfile,
        vgpu_id:      u32,
        mdev_path:    String,          // "/sys/bus/mdev/devices/<uuid>"
    },
}

// ── GPU inventory ──────────────────────────────────────────────────────────

pub async fn list_gpus() -> Result<Vec<GpuDevice>> {
    let out = Command::new("nvidia-smi")
        .args(["--query-gpu=pci.bus_id,gpu_uuid,name,memory.total,driver_version,compute_cap",
               "--format=csv,noheader,nounits"])
        .output()
        .await
        .context("nvidia-smi not found — is the NVIDIA driver installed?")?;

    let stdout = String::from_utf8(out.stdout)?;
    let mut gpus = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(", ").collect();
        if parts.len() < 6 { continue; }

        let pci = parts[0].trim().to_string();
        let iommu = read_iommu_group(&pci).unwrap_or(0);
        let mig_cap  = check_mig_capable(&pci).await;
        let vgpu_cap = check_vgpu_capable(&pci).await;

        gpus.push(GpuDevice {
            pci_address:  pci,
            gpu_uuid:     parts[1].trim().to_string(),
            model:        parts[2].trim().to_string(),
            vram_mib:     parts[3].trim().parse().unwrap_or(0),
            driver_ver:   parts[4].trim().to_string(),
            cuda_ver:     parts[5].trim().to_string(),
            mig_capable:  mig_cap,
            vgpu_capable: vgpu_cap,
            iommu_group:  iommu,
        });
    }

    Ok(gpus)
}

// ── Allocation entry point ─────────────────────────────────────────────────

pub async fn allocate_gpu(
    mode:    &str,
    profile: Option<&str>,
    vm_id:   u32,
) -> Result<GpuAllocation> {
    let gpus = list_gpus().await?;
    if gpus.is_empty() {
        bail!("No NVIDIA GPUs found on this node");
    }

    match mode {
        "passthrough" => {
            let gpu = gpus.into_iter().next().unwrap();
            passthrough::allocate(gpu, vm_id).await
        }
        "mig" => {
            let profile_str = profile.context("MIG mode requires --gpu-profile (e.g. 3g.40gb)")?;
            let mig_gpu = gpus.into_iter()
                .find(|g| g.mig_capable)
                .context("No MIG-capable GPU found (requires Ampere A100/A30/H100)")?;
            mig::allocate(mig_gpu, profile_str, vm_id).await
        }
        "vgpu" => {
            let profile_str = profile.context("vGPU mode requires --gpu-profile (e.g. A100-40C)")?;
            let vgpu_gpu = gpus.into_iter()
                .find(|g| g.vgpu_capable)
                .context("No vGPU-capable GPU found (requires NVIDIA vGPU software)")?;
            vgpu::allocate(vgpu_gpu, profile_str, vm_id).await
        }
        other => bail!("Unknown GPU mode: {other} (use: passthrough, mig, vgpu)"),
    }
}

pub async fn release_gpu(alloc: &GpuAllocation) -> Result<()> {
    match alloc {
        GpuAllocation::Passthrough { gpu, .. } =>
            passthrough::release(gpu).await,
        GpuAllocation::MigSlice { gpu, instance_id, .. } =>
            mig::release(gpu, *instance_id).await,
        GpuAllocation::VGpu { gpu, vgpu_id, .. } =>
            vgpu::release(gpu, *vgpu_id).await,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn read_iommu_group(pci: &str) -> Result<u32> {
    let link = format!("/sys/bus/pci/devices/{pci}/iommu_group");
    let target = std::fs::read_link(link)?;
    target.file_name()
        .and_then(|n| n.to_str())
        .and_then(|s| s.parse().ok())
        .context("parsing IOMMU group")
}

async fn check_mig_capable(pci: &str) -> bool {
    let out = Command::new("nvidia-smi")
        .args(["-i", pci, "--query-gpu=mig.mode.current", "--format=csv,noheader"])
        .output().await;
    matches!(out, Ok(o) if String::from_utf8_lossy(&o.stdout).contains("Enabled")
                        || String::from_utf8_lossy(&o.stdout).contains("N/A") == false)
}

async fn check_vgpu_capable(pci: &str) -> bool {
    // vGPU capable if /sys/bus/pci/devices/<pci>/mdev_supported_types exists
    std::path::Path::new(&format!("/sys/bus/pci/devices/{pci}/mdev_supported_types")).exists()
}
