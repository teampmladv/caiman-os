//! gpu/src/mig.rs — NVIDIA Multi-Instance GPU (MIG) management
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::info;

use crate::{GpuAllocation, GpuDevice};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigProfile {
    pub name:         String,
    pub compute_size: u8,
    pub memory_gib:   u8,
    pub profile_id:   u32,
}

pub async fn list_profiles(gpu: &GpuDevice) -> Result<Vec<MigProfile>> {
    let out = Command::new("nvidia-smi")
        .args(["mig", "-lgip", "-i", &gpu.pci_address])
        .output().await
        .context("nvidia-smi mig -lgip")?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut profiles = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[1] == "MIG" {
            let name = parts[2].to_string();
            if let Some((c, m)) = parse_profile_name(&name) {
                profiles.push(MigProfile {
                    profile_id:   parts[0].parse().unwrap_or(0),
                    name,
                    compute_size: c,
                    memory_gib:   m,
                });
            }
        }
    }
    Ok(profiles)
}

pub async fn allocate(gpu: GpuDevice, profile_str: &str, vm_id: u32) -> Result<GpuAllocation> {
    info!("MIG allocate: {} profile={profile_str} vm={vm_id}", gpu.pci_address);

    // Enable MIG mode if needed
    let status = Command::new("nvidia-smi")
        .args(["-i", &gpu.pci_address, "--query-gpu=mig.mode.current", "--format=csv,noheader"])
        .output().await?;
    let mode = String::from_utf8_lossy(&status.stdout);
    if !mode.trim().eq_ignore_ascii_case("enabled") {
        Command::new("nvidia-smi").args(["-i", &gpu.pci_address, "-mig", "1"])
            .status().await.context("enabling MIG mode")?;
    }

    // Create GPU instance
    let out = Command::new("nvidia-smi")
        .args(["mig", "-cgi", profile_str, "-i", &gpu.pci_address])
        .output().await.context("creating GPU instance")?;

    if !out.status.success() {
        bail!("Failed to create MIG instance: {}", String::from_utf8_lossy(&out.stderr));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let gi_id: u32 = stdout.split("GPU instance ID")
        .nth(1).and_then(|s| s.trim().split_whitespace().next())
        .and_then(|s| s.parse().ok()).unwrap_or(0);

    // Create compute instance
    let _ = Command::new("nvidia-smi")
        .args(["mig", "-cci", "-gi", &gi_id.to_string(), "-i", &gpu.pci_address])
        .status().await;

    let (compute_size, memory_gib) = parse_profile_name(profile_str).unwrap_or((1, 10));

    info!("MIG slice allocated: {profile_str} gi={gi_id}");
    Ok(GpuAllocation::MigSlice {
        gpu,
        profile: MigProfile { name: profile_str.to_string(), compute_size, memory_gib, profile_id: 0 },
        instance_id: gi_id,
        vfio_dev: format!("/dev/nvidia-caps/nvidia-cap{gi_id}"),
    })
}

pub async fn release(gpu: &GpuDevice, instance_id: u32) -> Result<()> {
    info!("Releasing MIG instance {instance_id} on {}", gpu.pci_address);
    let _ = Command::new("nvidia-smi")
        .args(["mig", "-dci", "-ci", "0", "-gi", &instance_id.to_string(), "-i", &gpu.pci_address])
        .status().await;
    Command::new("nvidia-smi")
        .args(["mig", "-dgi", "-gi", &instance_id.to_string(), "-i", &gpu.pci_address])
        .status().await.context("destroying GPU instance")?;
    Ok(())
}

fn parse_profile_name(name: &str) -> Option<(u8, u8)> {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() != 2 { return None; }
    let compute = parts[0].trim_end_matches('g').parse::<u8>().ok()?;
    let memory  = parts[1].trim_end_matches("gb").parse::<u8>().ok()?;
    Some((compute, memory))
}
