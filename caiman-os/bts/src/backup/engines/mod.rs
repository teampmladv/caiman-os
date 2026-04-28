//! backup/engines/mod.rs — Backup engine with multiple backends
//!
//! Uses Restic under the hood — content-addressable, encrypted, deduplicated.
//! Restic handles:
//!   - Block-level deduplication (same blocks across VMs → stored once)
//!   - Encryption at rest (AES-256-CTR + HMAC-SHA256)
//!   - Compression (zstd)
//!   - Parallel uploads
//!   - Integrity verification
//!
//! Backends:
//!   S3 / MinIO / Ceph RADOS GW  → restic -r s3:endpoint/bucket
//!   NFS (mounted)               → restic -r /mnt/backup
//!   Local                       → restic -r /var/lib/caiman/backups
//!   SFTP                        → restic -r sftp:host:/path
//!   B2 (Backblaze)              → restic -r b2:bucket

use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::*;

pub struct BackupEngine {
    db:       sqlx::SqlitePool,
    event_tx: broadcast::Sender<BackupEvent>,
}

#[derive(Debug, Clone)]
pub enum BackupEvent {
    Progress { id: String, pct: f64, phase: String, bytes_done: u64 },
    Completed { id: String, size_mib: u64, ratio: f64 },
    Failed    { id: String, error: String },
}

impl BackupEngine {
    pub fn new(db: sqlx::SqlitePool) -> (Self, broadcast::Receiver<BackupEvent>) {
        let (tx, rx) = broadcast::channel(64);
        (Self { db, event_tx: tx }, rx)
    }

    // ── Full backup ────────────────────────────────────────────────────────

    pub async fn backup_vm(
        &self,
        vm_id:      &str,
        vm_name:    &str,
        target:     BackupTarget,
        btype:      BackupType,
        parent_id:  Option<BackupId>,
        retention:  RetentionPolicy,
        created_by: &str,
        description:Option<String>,
    ) -> Result<Backup> {
        let backup_id = Uuid::new_v4().to_string();
        let name = format!("{vm_name}-{}", Utc::now().format("%Y%m%d-%H%M%S"));

        info!("Backup: starting {btype:?} of {vm_id} → {backup_id}");

        let mut backup = Backup {
            id:           backup_id.clone(),
            vm_id:        vm_id.to_string(),
            vm_name:      vm_name.to_string(),
            name:         name.clone(),
            description,
            backup_type:  btype,
            status:       BackupStatus::Running,
            target:       target.clone(),
            parent_id:    parent_id.clone(),
            restic_id:    None,
            size_mib:     0,
            raw_mib:      0,
            ratio:        1.0,
            dedup_mib:    0,
            checksum:     None,
            retention:    sqlx::types::Json(retention.clone()),
            expires_at:   None,
            started_at:   Utc::now(),
            finished_at:  None,
            duration_secs:None,
            created_by:   created_by.to_string(),
            error:        None,
        };

        self.save_backup(&backup).await?;

        // Find VM disk
        let disk_path = self.find_vm_disk(vm_id)?;

        // Initialize or check Restic repo
        let repo_path = self.repo_path(&target);
        let password  = self.repo_password(&target);
        self.init_repo_if_needed(&repo_path, &password).await?;

        // Build Restic backup command
        let tags = format!("vm={vm_id},name={vm_name},type={:?}", btype);
        let mut cmd_args = vec![
            "backup".to_string(),
            "--json".to_string(),
            format!("--tag={tags}"),
            "--compression=max".to_string(),
        ];

        // For incremental: use --parent flag
        if let Some(ref pid) = parent_id {
            let parent = self.get_backup(pid).await?;
            if let Some(restic_id) = parent.restic_id {
                cmd_args.push(format!("--parent={restic_id}"));
            }
        }

        cmd_args.push(disk_path.clone());

        let start = std::time::Instant::now();
        let out = self.run_restic(&repo_path, &password, &cmd_args).await?;

        let duration = start.elapsed().as_secs();

        // Parse Restic JSON output for snapshot ID and stats
        let (restic_id, size_mib, raw_mib, dedup_mib) = self.parse_restic_output(&out)?;
        let ratio = if size_mib > 0 { raw_mib as f64 / size_mib as f64 } else { 1.0 };

        // Apply retention policy
        self.apply_retention(&repo_path, &password, vm_id, &retention).await.ok();

        backup.status       = BackupStatus::Completed;
        backup.restic_id    = Some(restic_id);
        backup.size_mib     = size_mib;
        backup.raw_mib      = raw_mib;
        backup.ratio        = ratio;
        backup.dedup_mib    = dedup_mib;
        backup.finished_at  = Some(Utc::now());
        backup.duration_secs= Some(duration);

        self.update_backup(&backup).await?;

        let _ = self.event_tx.send(BackupEvent::Completed {
            id: backup_id.clone(), size_mib, ratio,
        });

        info!("Backup {backup_id} complete: {size_mib}MiB compressed ({ratio:.1}x), dedup saved {dedup_mib}MiB in {duration}s");
        Ok(backup)
    }

    // ── Restore ───────────────────────────────────────────────────────────

    pub async fn restore(
        &self,
        backup_id:     &str,
        target_vm_id:  Option<&str>,
        target_name:   Option<&str>,
        target_path:   Option<&str>,
    ) -> Result<OperationResult> {
        let backup = self.get_backup(backup_id).await?;
        let restic_id = backup.restic_id.as_deref()
            .context("backup has no Restic snapshot ID")?;

        let repo_path = self.repo_path(&backup.target);
        let password  = self.repo_password(&backup.target);

        let restore_path = target_path
            .map(|p| p.to_string())
            .unwrap_or_else(|| format!("/var/lib/caiman/restore/{}", &backup.vm_id));

        tokio::fs::create_dir_all(&restore_path).await?;

        info!("Restore: {backup_id} (restic:{restic_id}) → {restore_path}");

        self.run_restic(&repo_path, &password, &[
            "restore".to_string(),
            restic_id.to_string(),
            format!("--target={restore_path}"),
        ]).await?;

        info!("Restore complete: {restore_path}");

        Ok(OperationResult {
            id:           Uuid::new_v4().to_string(),
            op_type:      "backup_restore".into(),
            status:       "completed".into(),
            resource_id:  Some(backup_id.to_string()),
            progress_pct: 100.0,
            phase:        "done".into(),
            started_at:   Utc::now(),
            message:      Some(format!("Restored to {restore_path}")),
        })
    }

    // ── Verify ────────────────────────────────────────────────────────────

    pub async fn verify(&self, backup_id: &str) -> Result<bool> {
        let backup = self.get_backup(backup_id).await?;
        let repo   = self.repo_path(&backup.target);
        let pass   = self.repo_password(&backup.target);

        let out = self.run_restic(&repo, &pass, &["check".to_string()]).await;
        Ok(out.is_ok())
    }

    // ── List backups ───────────────────────────────────────────────────────

    pub async fn list_backups(&self, vm_id: Option<&str>) -> Result<Vec<Backup>> {
        let backups = if let Some(id) = vm_id {
            sqlx::query_as::<_, Backup>(
                "SELECT * FROM backups WHERE vm_id = ? ORDER BY started_at DESC"
            ).bind(id).fetch_all(&self.db).await?
        } else {
            sqlx::query_as::<_, Backup>(
                "SELECT * FROM backups ORDER BY started_at DESC LIMIT 200"
            ).fetch_all(&self.db).await?
        };
        Ok(backups)
    }

    // ── Retention ─────────────────────────────────────────────────────────

    async fn apply_retention(
        &self,
        repo: &str,
        pass: &str,
        vm_id: &str,
        policy: &RetentionPolicy,
    ) -> Result<()> {
        let mut args = vec!["forget".to_string(), "--prune".to_string(),
                            format!("--tag=vm={vm_id}")];
        if let Some(h) = policy.keep_hourly  { args.push(format!("--keep-hourly={h}")); }
        if let Some(d) = policy.keep_daily   { args.push(format!("--keep-daily={d}")); }
        if let Some(w) = policy.keep_weekly  { args.push(format!("--keep-weekly={w}")); }
        if let Some(m) = policy.keep_monthly { args.push(format!("--keep-monthly={m}")); }
        if let Some(y) = policy.keep_yearly  { args.push(format!("--keep-yearly={y}")); }

        self.run_restic(repo, pass, &args).await?;
        Ok(())
    }

    // ── Restic subprocess ──────────────────────────────────────────────────

    async fn run_restic(&self, repo: &str, pass: &str, args: &[String]) -> Result<String> {
        let out = Command::new("restic")
            .arg("-r").arg(repo)
            .args(args)
            .env("RESTIC_PASSWORD", pass)
            .output().await
            .context("restic not found — install with: apt install restic")?;

        if !out.status.success() {
            bail!("restic error: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    async fn init_repo_if_needed(&self, repo: &str, pass: &str) -> Result<()> {
        let check = Command::new("restic")
            .args(["-r", repo, "snapshots", "--json"])
            .env("RESTIC_PASSWORD", pass)
            .output().await?;
        if !check.status.success() {
            info!("Initializing new Restic repo: {repo}");
            self.run_restic(repo, pass, &["init".to_string()]).await?;
        }
        Ok(())
    }

    fn repo_path(&self, target: &BackupTarget) -> String {
        match target {
            BackupTarget::S3    { endpoint, bucket, prefix } => {
                let ep = endpoint.as_deref().unwrap_or("s3.amazonaws.com");
                format!("s3:{ep}/{bucket}/{prefix}")
            }
            BackupTarget::Nfs   { mount_path, .. } => mount_path.clone(),
            BackupTarget::Local { path }           => path.clone(),
            BackupTarget::Restic{ repo, .. }       => repo.clone(),
        }
    }

    fn repo_password(&self, target: &BackupTarget) -> String {
        match target {
            BackupTarget::Restic { password_env, .. } =>
                std::env::var(password_env).unwrap_or_else(|_| "changeme".into()),
            _ =>
                std::env::var("CAIMAN_BACKUP_PASSWORD").unwrap_or_else(|_| "changeme".into()),
        }
    }

    fn find_vm_disk(&self, vm_id: &str) -> Result<String> {
        let state_dir = std::env::var("CAIMAN_STATE_DIR")
            .unwrap_or_else(|_| "/var/run/caiman".into());
        let num = vm_id.trim_start_matches("vm-");
        Ok(format!("{state_dir}/{num}.img"))
    }

    fn parse_restic_output(&self, output: &str) -> Result<(String, u64, u64, u64)> {
        // Restic --json outputs one JSON object per line, last line is summary
        for line in output.lines().rev() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v["message_type"] == "summary" {
                    let id       = v["snapshot_id"].as_str().unwrap_or("").to_string();
                    let size_mib = v["data_added"].as_u64().unwrap_or(0) / (1024*1024);
                    let raw_mib  = v["total_bytes_processed"].as_u64().unwrap_or(0) / (1024*1024);
                    let dedup    = raw_mib.saturating_sub(size_mib);
                    return Ok((id, size_mib, raw_mib, dedup));
                }
            }
        }
        Ok(("unknown".into(), 0, 0, 0))
    }

    // ── Database helpers ───────────────────────────────────────────────────

    async fn save_backup(&self, b: &Backup) -> Result<()> {
        sqlx::query(
            "INSERT INTO backups (id,vm_id,vm_name,name,description,backup_type,status,
             target,parent_id,restic_id,size_mib,raw_mib,ratio,dedup_mib,
             retention,started_at,created_by)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
        )
        .bind(&b.id).bind(&b.vm_id).bind(&b.vm_name).bind(&b.name)
        .bind(&b.description).bind(format!("{:?}", b.backup_type))
        .bind(format!("{:?}", b.status))
        .bind(serde_json::to_string(&b.target).unwrap_or_default())
        .bind(&b.parent_id).bind(&b.restic_id)
        .bind(b.size_mib as i64).bind(b.raw_mib as i64)
        .bind(b.ratio).bind(b.dedup_mib as i64)
        .bind(serde_json::to_string(&b.retention).unwrap_or_default())
        .bind(b.started_at).bind(&b.created_by)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn update_backup(&self, b: &Backup) -> Result<()> {
        sqlx::query(
            "UPDATE backups SET status=?,restic_id=?,size_mib=?,raw_mib=?,
             ratio=?,dedup_mib=?,finished_at=?,duration_secs=?,error=?
             WHERE id=?"
        )
        .bind(format!("{:?}", b.status)).bind(&b.restic_id)
        .bind(b.size_mib as i64).bind(b.raw_mib as i64)
        .bind(b.ratio).bind(b.dedup_mib as i64)
        .bind(b.finished_at).bind(b.duration_secs.map(|d| d as i64))
        .bind(&b.error).bind(&b.id)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn get_backup(&self, id: &str) -> Result<Backup> {
        sqlx::query_as::<_, Backup>("SELECT * FROM backups WHERE id = ?")
            .bind(id).fetch_one(&self.db).await
            .with_context(|| format!("backup {id} not found"))
    }
}
