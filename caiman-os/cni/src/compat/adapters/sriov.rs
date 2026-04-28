//! adapters/sriov.rs — SR-IOV Virtual Function passthrough
//!
//! When SR-IOV is available, bypass the virtual tap/XDP networking entirely
//! and pass a physical VF directly to the VM via VFIO-PCI.
//!
//! The SR-IOV device plugin (github.com/k8s-sigs/sriov-network-device-plugin)
//! allocates VFs and passes the device ID via the CNI config or runtime config.
//!
//! Flow:
//!   1. Read VF PCI address from config.deviceID or SRIOV_RESOURCE env
//!   2. Unbind VF from its current driver
//!   3. Bind to vfio-pci
//!   4. Pass PCI address to VMM via a state file
//!   5. Update DPDK/kernel datapath metadata

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn};

use crate::compat::CniEnv;
use crate::compat::ipam::IpamResult;

pub struct SrIovAdapter;

/// Called directly from main (not via CniAdapter trait) because SR-IOV
/// completely replaces the tap+XDP path.
pub async fn add(env: &CniEnv) -> Result<String> {
    let device_id = env.config.device_id
        .as_deref()
        .or_else(|| env.config.runtime_cfg.as_ref()?.device_id.as_deref())
        .context("SR-IOV: deviceID not specified in CNI config")?
        .to_string();

    info!("SR-IOV: passthrough device={device_id} container={}", env.container_id);

    // Validate PCI address format (e.g. "0000:01:00.1")
    if !is_valid_pci(&device_id) {
        bail!("Invalid PCI address: {device_id}");
    }

    // Get VF network device name (e.g. "enp1s0f1")
    let vf_netdev = pci_to_netdev(&device_id)
        .await
        .with_context(|| format!("resolving netdev for PCI {device_id}"))?;

    // Unbind from current driver
    unbind_vf(&device_id)
        .await
        .with_context(|| format!("unbinding VF {device_id}"))?;

    // Bind to vfio-pci for passthrough
    bind_vfio(&device_id)
        .await
        .with_context(|| format!("binding VF {device_id} to vfio-pci"))?;

    // Get the IOMMU group for the VF
    let iommu_group = iommu_group(&device_id)
        .with_context(|| format!("getting IOMMU group for {device_id}"))?;

    // Write passthrough config for the VMM to pick up
    let state_path = format!("/var/run/caiman/sriov-{}.json", env.container_id);
    let state = serde_json::json!({
        "pci_address":  device_id,
        "vfio_device":  format!("/dev/vfio/{iommu_group}"),
        "iommu_group":  iommu_group,
        "vf_netdev":    vf_netdev,
    });
    tokio::fs::write(&state_path, serde_json::to_string(&state)?)
        .await
        .context("writing SR-IOV state file")?;

    // IPAM: allocate an IP (network is configured in the VM via DHCP or static)
    let ip_result = crate::compat::ipam::allocate(&env.config, &env.container_id, &env.netns)
        .await
        .unwrap_or_default();

    info!("SR-IOV: VF {device_id} ready (IOMMU group {iommu_group}), VMM will passthrough via vfio-pci");

    let result = serde_json::json!({
        "cniVersion": env.config.cni_version,
        "interfaces": [{
            "name": vf_netdev,
            "pci":  device_id,
            "sandbox": env.netns,
        }],
        "ips":    ip_result.ips,
        "routes": ip_result.routes,
        "dns":    ip_result.dns,
    });
    Ok(serde_json::to_string(&result)?)
}

pub async fn del(env: &CniEnv) -> Result<String> {
    let state_path = format!("/var/run/caiman/sriov-{}.json", env.container_id);

    if let Ok(data) = tokio::fs::read_to_string(&state_path).await {
        if let Ok(state) = serde_json::from_str::<serde_json::Value>(&data) {
            let pci = state["pci_address"].as_str().unwrap_or("");
            if !pci.is_empty() {
                // Unbind from vfio-pci and rebind to original driver
                rebind_original_driver(pci).await.ok();
            }
        }
        tokio::fs::remove_file(&state_path).await.ok();
    }

    crate::compat::ipam::release(&env.config, &env.container_id, &env.netns)
        .await.ok();
    Ok("{}".into())
}

// ── SR-IOV helpers ──────────────────────────────────────────────────────────

fn is_valid_pci(addr: &str) -> bool {
    // Format: DDDD:BB:SS.F  (domain:bus:slot.function)
    let re = regex_lite::Regex::new(
        r"^[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-7]$"
    ).unwrap();
    re.is_match(addr)
}

async fn pci_to_netdev(pci: &str) -> Result<String> {
    let path = format!("/sys/bus/pci/devices/{pci}/net");
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .with_context(|| format!("reading {path}"))?;
    if let Some(entry) = dir.next_entry().await? {
        return Ok(entry.file_name().to_string_lossy().into_owned());
    }
    bail!("No netdev found for PCI {pci}")
}

async fn unbind_vf(pci: &str) -> Result<()> {
    let unbind_path = format!("/sys/bus/pci/devices/{pci}/driver/unbind");
    if std::path::Path::new(&unbind_path).exists() {
        tokio::fs::write(&unbind_path, pci)
            .await
            .context("unbind write")?;
    }
    Ok(())
}

async fn bind_vfio(pci: &str) -> Result<()> {
    // Load vfio-pci module
    let _ = Command::new("modprobe").arg("vfio-pci").output().await;

    // Override driver
    tokio::fs::write("/sys/bus/pci/drivers/vfio-pci/new_id",
        format!("{} {}", vendor_device(pci).await.unwrap_or_default(), ""))
        .await.ok();

    tokio::fs::write(format!("/sys/bus/pci/devices/{pci}/driver_override"), "vfio-pci")
        .await.context("driver_override")?;

    tokio::fs::write("/sys/bus/pci/drivers_probe", pci)
        .await.context("drivers_probe")?;
    Ok(())
}

async fn rebind_original_driver(pci: &str) -> Result<()> {
    // Remove driver_override so the kernel uses the default driver
    tokio::fs::write(format!("/sys/bus/pci/devices/{pci}/driver_override"), "\n")
        .await.ok();
    tokio::fs::write("/sys/bus/pci/drivers_probe", pci)
        .await.ok();
    Ok(())
}

fn iommu_group(pci: &str) -> Result<String> {
    let link = format!("/sys/bus/pci/devices/{pci}/iommu_group");
    let target = std::fs::read_link(&link)
        .with_context(|| format!("reading symlink {link}"))?;
    target.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .context("iommu_group: no filename")
}

async fn vendor_device(pci: &str) -> Result<String> {
    let vendor = tokio::fs::read_to_string(
        format!("/sys/bus/pci/devices/{pci}/vendor")).await?;
    let device = tokio::fs::read_to_string(
        format!("/sys/bus/pci/devices/{pci}/device")).await?;
    Ok(format!("{} {}", vendor.trim().trim_start_matches("0x"),
                        device.trim().trim_start_matches("0x")))
}
