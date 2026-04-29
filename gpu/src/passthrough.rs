//! gpu/src/passthrough.rs — Full GPU passthrough via VFIO-PCI
//!
//! Flow:
//!   1. Unbind GPU from nvidia driver
//!   2. Bind to vfio-pci driver
//!   3. Pass /dev/vfio/<group> to caiman-vmm as a VFIO device
//!   4. Kernel assigns the PCIe device exclusively to the VM
//!
//! Requirements:
//!   - IOMMU enabled in BIOS (VT-d / AMD-Vi)
//!   - kernel cmdline: intel_iommu=on iommu=pt
//!   - vfio-pci module loaded
//!   - GPU in its own IOMMU group (or ACS override)

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::info;

use super::{GpuDevice, GpuAllocation};

pub async fn allocate(gpu: GpuDevice, vm_id: u32) -> Result<GpuAllocation> {
    info!("GPU passthrough: {} → vm_{vm_id}", gpu.pci_address);

    // 1. Check IOMMU is enabled
    check_iommu().await?;

    // 2. Load vfio-pci module
    Command::new("modprobe").arg("vfio-pci")
        .status().await
        .context("loading vfio-pci module")?;

    // 3. Unbind from current driver (nvidia or nouveau)
    unbind_driver(&gpu.pci_address).await?;

    // 4. Bind to vfio-pci
    bind_vfio(&gpu.pci_address, &gpu.gpu_uuid).await?;

    // 5. Find the VFIO device path
    let vfio_dev = format!("/dev/vfio/{}", gpu.iommu_group);
    if !std::path::Path::new(&vfio_dev).exists() {
        anyhow::bail!("VFIO device {vfio_dev} not found after binding");
    }

    info!("GPU passthrough ready: {} → {vfio_dev}", gpu.pci_address);
    Ok(GpuAllocation::Passthrough { gpu, vfio_dev })
}

pub async fn release(gpu: &GpuDevice) -> Result<()> {
    info!("Releasing GPU passthrough: {}", gpu.pci_address);

    // Unbind from vfio-pci
    let unbind = format!("/sys/bus/pci/drivers/vfio-pci/unbind");
    std::fs::write(&unbind, &gpu.pci_address)
        .with_context(|| format!("unbinding {}", gpu.pci_address))?;

    // Re-bind to nvidia driver
    let bind = "/sys/bus/pci/drivers/nvidia/bind";
    std::fs::write(bind, &gpu.pci_address)
        .with_context(|| format!("rebinding to nvidia: {}", gpu.pci_address))?;

    info!("GPU {} returned to nvidia driver", gpu.pci_address);
    Ok(())
}

/// Generate QEMU-style VFIO args for caiman-vmm
/// These are passed to the VMM which adds them to the KVM device list
pub fn vfio_args(alloc: &GpuAllocation) -> Vec<String> {
    match alloc {
        GpuAllocation::Passthrough { gpu, vfio_dev } => vec![
            "--vfio-device".into(),
            format!("{}:{}", gpu.pci_address, vfio_dev),
        ],
        _ => vec![],
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

async fn check_iommu() -> Result<()> {
    // Check dmesg or /sys for IOMMU groups
    let path = "/sys/kernel/iommu_groups";
    let entries = std::fs::read_dir(path)
        .context("IOMMU not enabled — add intel_iommu=on or amd_iommu=on to kernel cmdline")?;
    let count = entries.count();
    anyhow::ensure!(count > 0,
        "IOMMU groups empty — is IOMMU enabled in BIOS and kernel cmdline?");
    Ok(())
}

async fn unbind_driver(pci: &str) -> Result<()> {
    // Try nvidia first, then nouveau
    for driver in &["nvidia", "nouveau", "radeon", "amdgpu"] {
        let path = format!("/sys/bus/pci/drivers/{driver}/unbind");
        if std::path::Path::new(&path).exists() {
            let _ = std::fs::write(&path, pci);
        }
    }
    Ok(())
}

async fn bind_vfio(pci: &str, uuid: &str) -> Result<()> {
    // Write vendor:device id to vfio-pci new_id
    let vendor_device = get_vendor_device(pci).await?;
    let new_id = "/sys/bus/pci/drivers/vfio-pci/new_id";
    std::fs::write(new_id, &vendor_device)
        .with_context(|| format!("writing {vendor_device} to vfio-pci/new_id"))?;

    // Bind
    let bind = "/sys/bus/pci/drivers/vfio-pci/bind";
    std::fs::write(bind, pci)
        .with_context(|| format!("binding {pci} to vfio-pci"))?;

    info!("Bound {pci} to vfio-pci (vendor:device={vendor_device})");
    Ok(())
}

async fn get_vendor_device(pci: &str) -> Result<String> {
    let vendor = std::fs::read_to_string(
        format!("/sys/bus/pci/devices/{pci}/vendor")
    )?.trim().to_string();
    let device = std::fs::read_to_string(
        format!("/sys/bus/pci/devices/{pci}/device")
    )?.trim().to_string();
    // Format: "10de 2330" (NVIDIA vendor + device ID)
    Ok(format!("{} {}", vendor.trim_start_matches("0x"), device.trim_start_matches("0x")))
}
