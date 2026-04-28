//! src/types.rs — Core types for Backup, Templates & Snapshots
//!
//! Snapshot chain model (COW — Copy-on-Write):
//!
//!   Base image (full)
//!       │
//!       ├─ snap-001 (delta from base)     ← oldest
//!       │       │
//!       │       ├─ snap-002 (delta from 001)
//!       │       │       │
//!       │       │       └─ snap-003 (delta from 002) ← current
//!       │       │
//!       │       └─ snap-004 (branch — clone for testing)
//!       │
//!       └─ template-web-v2 (sealed, read-only, used for new VMs)
//!
//! Backup model (full + incremental):
//!   backup-2025-01-01T00:00:00 (full — Restic snapshot to S3/NFS)
//!       backup-2025-01-02T00:00:00 (incremental — only changed blocks)
//!       backup-2025-01-03T00:00:00 (incremental)
//!   backup-2025-01-08T00:00:00 (full — weekly)
//!
//! Template model:
//!   A Template is a sealed VM image + cloud-init config that can be
//!   cloned instantly (copy-on-write) to spawn new VMs in < 5 seconds.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Identifiers ───────────────────────────────────────────────────────────

pub type SnapshotId  = String;
pub type BackupId    = String;
pub type TemplateId  = String;
pub type VmId        = String;

// ── Snapshot ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Snapshot {
    pub id:          SnapshotId,
    pub vm_id:       VmId,
    pub vm_name:     String,
    pub name:        String,
    pub description: Option<String>,
    /// Disk image path (qcow2 with backing file chain)
    pub image_path:  String,
    /// Parent snapshot (None = base image)
    pub parent_id:   Option<SnapshotId>,
    /// Chain depth (0 = full, N = Nth delta)
    pub depth:       u32,
    /// Uncompressed disk size in MiB
    pub disk_mib:    u64,
    /// Actual size on disk (delta only) in MiB
    pub actual_mib:  u64,
    /// BLAKE3 hash of the qcow2 file
    pub checksum:    String,
    /// VM memory state included (for consistent crash-consistent snapshots)
    pub has_memory:  bool,
    /// Sealed → can no longer be used as base for new snapshots
    pub sealed:      bool,
    /// Crash-consistent (VM was running) or quiesced (guest agent synced)
    pub consistency: SnapshotConsistency,
    pub labels:      sqlx::types::Json<std::collections::HashMap<String, String>>,
    pub created_at:  DateTime<Utc>,
    pub created_by:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum SnapshotConsistency {
    /// VM was paused during snapshot (best)
    Quiesced,
    /// Snapshot taken while running — crash-consistent
    CrashConsistent,
    /// VM was stopped
    Offline,
}

// ── Backup ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Backup {
    pub id:           BackupId,
    pub vm_id:        VmId,
    pub vm_name:      String,
    pub name:         String,
    pub description:  Option<String>,
    pub backup_type:  BackupType,
    pub status:       BackupStatus,
    pub target:       BackupTarget,
    /// Parent full backup (for incremental)
    pub parent_id:    Option<BackupId>,
    /// Restic snapshot ID (in the target repo)
    pub restic_id:    Option<String>,
    /// Compressed+deduplicated size in MiB
    pub size_mib:     u64,
    /// Original (pre-compression) size in MiB
    pub raw_mib:      u64,
    /// Compression ratio (raw/compressed)
    pub ratio:        f64,
    /// Dedup savings in MiB
    pub dedup_mib:    u64,
    /// Checksum of the backup manifest
    pub checksum:     Option<String>,
    /// Retention policy applied
    pub retention:    sqlx::types::Json<RetentionPolicy>,
    /// Expires at (computed from retention)
    pub expires_at:   Option<DateTime<Utc>>,
    pub started_at:   DateTime<Utc>,
    pub finished_at:  Option<DateTime<Utc>>,
    pub duration_secs:Option<u64>,
    pub created_by:   String,
    pub error:        Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text")]
pub enum BackupType {
    Full,
    Incremental,
    Differential,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text")]
pub enum BackupStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Verifying,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackupTarget {
    S3    { bucket: String, prefix: String, endpoint: Option<String> },
    Nfs   { server: String, export: String, mount_path: String },
    Local { path: String },
    Restic { repo: String, password_env: String },
}

// ── Retention policy ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetentionPolicy {
    /// Keep last N hourly backups
    pub keep_hourly:  Option<u32>,
    /// Keep last N daily backups
    pub keep_daily:   Option<u32>,
    /// Keep last N weekly backups
    pub keep_weekly:  Option<u32>,
    /// Keep last N monthly backups
    pub keep_monthly: Option<u32>,
    /// Keep last N yearly backups
    pub keep_yearly:  Option<u32>,
    /// Always keep backups newer than this many days
    pub keep_within_days: Option<u32>,
}

// ── Backup schedule ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BackupSchedule {
    pub id:          String,
    pub vm_id:       Option<VmId>,     // None = all VMs
    pub name:        String,
    pub cron_expr:   String,           // "0 2 * * *" = daily at 02:00
    pub backup_type: BackupType,
    pub target:      sqlx::types::Json<BackupTarget>,
    pub retention:   sqlx::types::Json<RetentionPolicy>,
    pub enabled:     bool,
    pub last_run:    Option<DateTime<Utc>>,
    pub next_run:    Option<DateTime<Utc>>,
    pub created_at:  DateTime<Utc>,
}

// ── Template ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VmTemplate {
    pub id:           TemplateId,
    pub name:         String,
    pub description:  Option<String>,
    pub version:      String,          // semver: "1.2.0"
    pub os_type:      OsType,
    pub os_version:   String,          // "ubuntu-22.04", "debian-12"
    /// Base qcow2 image path (read-only, sealed)
    pub image_path:   String,
    pub image_mib:    u64,
    pub checksum:     String,
    /// Default VM config for instances of this template
    pub default_cfg:  sqlx::types::Json<TemplateDefaultCfg>,
    /// cloud-init meta-data template (Tera/Jinja2)
    pub cloud_init:   Option<String>,
    /// network-config template
    pub network_cfg:  Option<String>,
    pub labels:       sqlx::types::Json<std::collections::HashMap<String, String>>,
    /// Source: from snapshot, custom upload, or marketplace
    pub source:       TemplateSource,
    /// How many VMs have been cloned from this template
    pub clone_count:  u64,
    pub created_at:   DateTime<Utc>,
    pub created_by:   String,
    pub published:    bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum OsType { Linux, Windows, FreeBSD, Other }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDefaultCfg {
    pub mem_mib:  u64,
    pub cpus:     u8,
    pub disk_gib: u64,
    pub uplink:   String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum TemplateSource {
    Snapshot,     // created from a VM snapshot
    Upload,       // raw image uploaded by operator
    Marketplace,  // from caiman template registry
    Import,       // imported from OVF/OVA
}

// ── Restore request ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreRequest {
    pub source_type:  RestoreSource,
    pub source_id:    String,
    /// Target VM ID (None = create new VM)
    pub target_vm_id: Option<VmId>,
    pub target_name:  Option<String>,
    pub target_node:  Option<String>,
    /// For partial restores: specific disks/volumes
    pub disks:        Option<Vec<String>>,
    /// Overwrite existing VM (if target_vm_id is set)
    pub overwrite:    bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestoreSource {
    Snapshot,
    Backup,
    Template,
}

// ── Clone request ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneRequest {
    pub source_id:   String,   // snapshot or template ID
    pub name:        String,
    pub mem_mib:     Option<u64>,
    pub cpus:        Option<u8>,
    pub node:        Option<String>,
    pub labels:      Option<std::collections::HashMap<String, String>>,
    /// cloud-init user-data variables (injected into template)
    pub user_data:   Option<std::collections::HashMap<String, serde_json::Value>>,
    pub start_after: bool,
}

// ── Operation result ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub id:          String,
    pub op_type:     String,
    pub status:      String,
    pub resource_id: Option<String>,
    pub progress_pct:f64,
    pub phase:       String,
    pub started_at:  DateTime<Utc>,
    pub message:     Option<String>,
}
