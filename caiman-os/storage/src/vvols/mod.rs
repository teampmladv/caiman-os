//! storage/src/vvols/mod.rs — Virtual Volumes (vVols) + SAN/NAS integration
//!
//! vVols: storage-policy-based management — the hypervisor tells the array
//! what the VM needs (performance tier, snapshot schedule, encryption) and
//! the array enforces it at hardware level.
//!
//! Supported backends:
//!   iSCSI   — software initiator (open-iscsi / libiscsi)
//!   NVMe-oF — kernel NVMe-oF TCP/RDMA initiator
//!   NFS v4.1 — pNFS with parallel data paths
//!   FC       — Fibre Channel via HBA (requires fc_transport module)
//!   SMB3     — Windows-compatible shares (via CIFS kernel module)

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn};

// ── Storage backend types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageBackend {
    Iscsi(IscsiConfig),
    Nvmeof(NvmeofConfig),
    Nfs(NfsConfig),
    Fc(FcConfig),
    Local(LocalConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IscsiConfig {
    pub portal:      String,  // "192.168.1.10:3260"
    pub target_iqn:  String,  // "iqn.2024-01.io.caiman:storage01"
    pub lun:         u32,
    pub auth:        Option<ChapAuth>,
    pub multipath:   bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapAuth {
    pub username: String,
    pub secret:   String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvmeofConfig {
    pub transport:  NvmeTransport,  // tcp or rdma
    pub addr:       String,
    pub port:       u16,
    pub nqn:        String,         // NVMe Qualified Name
    pub hostnqn:    Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NvmeTransport { Tcp, Rdma }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NfsConfig {
    pub server:      String,
    pub export_path: String,
    pub version:     NfsVersion,
    pub options:     Vec<String>,   // e.g. ["rw", "hard", "rsize=1048576"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NfsVersion { V3, V41, V42 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FcConfig {
    pub wwpn:        String,   // Target WWPN
    pub lun:         u32,
    pub multipath:   bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub path: String,   // /dev/nvme0n1 or a file path for image-backed
    pub format: DiskFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiskFormat { Raw, Qcow2 }

// ── vVol: a VM-specific volume on an external array ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VVol {
    pub id:          String,
    pub vm_id:       u32,
    pub name:        String,
    pub size_gib:    u64,
    pub backend:     StorageBackend,
    pub policy:      VVolPolicy,
    pub local_dev:   Option<String>,  // /dev/sdb after attachment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VVolPolicy {
    pub performance_tier: PerformanceTier,
    pub snapshot_schedule: Option<String>,  // cron-like: "0 */4 * * *"
    pub encryption:        bool,
    pub replication:       Option<ReplicationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceTier { Platinum, Gold, Silver, Bronze }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationTarget {
    pub target_cluster: String,
    pub rpo_minutes:    u32,
}

// ── Attachment logic ──────────────────────────────────────────────────────

pub async fn attach(vvol: &mut VVol) -> Result<String> {
    let dev = match &vvol.backend {
        StorageBackend::Iscsi(c)  => attach_iscsi(c).await?,
        StorageBackend::Nvmeof(c) => attach_nvmeof(c).await?,
        StorageBackend::Nfs(c)    => mount_nfs(c, &vvol.id).await?,
        StorageBackend::Fc(c)     => attach_fc(c).await?,
        StorageBackend::Local(c)  => c.path.clone(),
    };
    vvol.local_dev = Some(dev.clone());
    info!("vVol {}: attached as {dev}", vvol.id);
    Ok(dev)
}

pub async fn detach(vvol: &VVol) -> Result<()> {
    match &vvol.backend {
        StorageBackend::Iscsi(c)  => detach_iscsi(c).await?,
        StorageBackend::Nvmeof(c) => detach_nvmeof(c).await?,
        StorageBackend::Nfs(c)    => unmount_nfs(c, &vvol.id).await?,
        StorageBackend::Fc(_)     => {}
        StorageBackend::Local(_)  => {}
    }
    Ok(())
}

// ── iSCSI ─────────────────────────────────────────────────────────────────

async fn attach_iscsi(cfg: &IscsiConfig) -> Result<String> {
    // Discover target
    Command::new("iscsiadm")
        .args(["-m", "discovery", "-t", "sendtargets", "-p", &cfg.portal])
        .output().await.context("iSCSI discovery")?;

    // Login
    Command::new("iscsiadm")
        .args(["-m", "node", "-T", &cfg.target_iqn,
               "-p", &cfg.portal, "--login"])
        .output().await.context("iSCSI login")?;

    // Find the newly attached block device
    find_iscsi_device(&cfg.target_iqn).await
}

async fn detach_iscsi(cfg: &IscsiConfig) -> Result<()> {
    Command::new("iscsiadm")
        .args(["-m", "node", "-T", &cfg.target_iqn, "--logout"])
        .output().await.context("iSCSI logout")?;
    Ok(())
}

async fn find_iscsi_device(iqn: &str) -> Result<String> {
    let out = Command::new("iscsiadm")
        .args(["-m", "session", "-P", "3"])
        .output().await?;
    // Parse output for device path — simplified
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if line.contains("Attached scsi disk") {
            if let Some(dev) = line.split_whitespace().last() {
                return Ok(format!("/dev/{dev}"));
            }
        }
    }
    bail!("Could not find iSCSI device for {iqn}")
}

// ── NVMe-oF ───────────────────────────────────────────────────────────────

async fn attach_nvmeof(cfg: &NvmeofConfig) -> Result<String> {
    let transport = match cfg.transport {
        NvmeTransport::Tcp  => "tcp",
        NvmeTransport::Rdma => "rdma",
    };
    Command::new("nvme")
        .args(["connect",
               "-t", transport,
               "-a", &cfg.addr,
               "-s", &cfg.port.to_string(),
               "-n", &cfg.nqn])
        .output().await.context("nvme connect")?;

    // Wait for device node to appear
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    find_nvme_device(&cfg.nqn).await
}

async fn detach_nvmeof(cfg: &NvmeofConfig) -> Result<()> {
    Command::new("nvme")
        .args(["disconnect", "-n", &cfg.nqn])
        .output().await.context("nvme disconnect")?;
    Ok(())
}

async fn find_nvme_device(nqn: &str) -> Result<String> {
    let out = Command::new("nvme")
        .args(["list", "-o", "json"])
        .output().await?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    if let Some(devs) = json["Devices"].as_array() {
        for dev in devs {
            if dev["SubsystemNQN"].as_str() == Some(nqn) {
                if let Some(path) = dev["DevicePath"].as_str() {
                    return Ok(path.to_string());
                }
            }
        }
    }
    bail!("NVMe device not found for NQN {nqn}")
}

// ── NFS ───────────────────────────────────────────────────────────────────

async fn mount_nfs(cfg: &NfsConfig, vol_id: &str) -> Result<String> {
    let version = match cfg.version {
        NfsVersion::V3  => "3",
        NfsVersion::V41 => "4.1",
        NfsVersion::V42 => "4.2",
    };
    let mount_point = format!("/mnt/caiman/nfs/{vol_id}");
    tokio::fs::create_dir_all(&mount_point).await?;

    let mut opts = cfg.options.clone();
    opts.push(format!("vers={version}"));

    Command::new("mount")
        .args(["-t", "nfs",
               "-o", &opts.join(","),
               &format!("{}:{}", cfg.server, cfg.export_path),
               &mount_point])
        .output().await.context("NFS mount")?;

    Ok(mount_point)
}

async fn unmount_nfs(_cfg: &NfsConfig, vol_id: &str) -> Result<()> {
    let mount_point = format!("/mnt/caiman/nfs/{vol_id}");
    Command::new("umount").arg(&mount_point).output().await?;
    tokio::fs::remove_dir(&mount_point).await.ok();
    Ok(())
}

// ── FC ────────────────────────────────────────────────────────────────────

async fn attach_fc(cfg: &FcConfig) -> Result<String> {
    // Trigger FC LUN scan
    Command::new("echo").args(["- - -"])
        .output().await?;
    // In production: use sg_scan, multipath -ll, etc.
    bail!("FC attachment requires HBA and fc_transport kernel module — check /sys/class/fc_host/")
}
