//! src/routes.rs — REST API for Backup, Templates & Snapshots
//!
//! Mounted at /api/bts/ by caiman-api via reverse proxy
//!
//! SNAPSHOTS
//!   POST   /api/bts/snapshots/:vm_id           Take snapshot
//!   GET    /api/bts/snapshots                  List all
//!   GET    /api/bts/snapshots/:id              Get detail
//!   GET    /api/bts/snapshots/:vm_id/chain     Show COW chain tree
//!   POST   /api/bts/snapshots/:id/restore      Restore snapshot
//!   POST   /api/bts/snapshots/:id/clone        Clone to new VM
//!   POST   /api/bts/snapshots/:id/seal         Seal (read-only)
//!   DELETE /api/bts/snapshots/:id              Delete + merge delta
//!
//! BACKUPS
//!   POST   /api/bts/backups/:vm_id             Start backup
//!   GET    /api/bts/backups                    List all
//!   GET    /api/bts/backups/:id                Get detail
//!   POST   /api/bts/backups/:id/restore        Restore backup
//!   POST   /api/bts/backups/:id/verify         Verify integrity
//!   DELETE /api/bts/backups/:id                Delete backup
//!   GET    /api/bts/schedules                  List schedules
//!   POST   /api/bts/schedules                  Create schedule
//!   PUT    /api/bts/schedules/:id              Update schedule
//!   DELETE /api/bts/schedules/:id              Delete schedule
//!
//! TEMPLATES
//!   GET    /api/bts/templates                  List (published)
//!   GET    /api/bts/templates/all              List all (admin)
//!   GET    /api/bts/templates/:id              Get detail
//!   POST   /api/bts/templates                  Create from snapshot
//!   POST   /api/bts/templates/:id/clone        Clone → new VM
//!   POST   /api/bts/templates/:id/publish      Publish
//!   POST   /api/bts/templates/:id/unpublish    Unpublish
//!   DELETE /api/bts/templates/:id              Delete template

use std::sync::Arc;
use axum::{
    Router, routing::{get, post, delete, put},
    extract::{State, Path, Query},
    Json, http::StatusCode,
};
use serde::Deserialize;

use crate::state::BtsState;
use crate::types::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn err(code: StatusCode, msg: impl ToString) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({ "error": msg.to_string() })))
}

pub fn router(state: Arc<BtsState>) -> Router {
    Router::new()
        // Snapshots
        .route("/snapshots",           get(list_snapshots))
        .route("/snapshots/:vm_id",    post(take_snapshot))
        .route("/snapshots/detail/:id",get(get_snapshot))
        .route("/snapshots/:vm_id/chain", get(snapshot_chain))
        .route("/snapshots/:id/restore", post(restore_snapshot))
        .route("/snapshots/:id/clone",   post(clone_snapshot))
        .route("/snapshots/:id/seal",    post(seal_snapshot))
        .route("/snapshots/:id",        delete(delete_snapshot))
        // Backups
        .route("/backups",             get(list_backups))
        .route("/backups/:vm_id",      post(start_backup))
        .route("/backups/detail/:id",  get(get_backup))
        .route("/backups/:id/restore", post(restore_backup))
        .route("/backups/:id/verify",  post(verify_backup))
        .route("/backups/:id",         delete(delete_backup))
        // Schedules
        .route("/schedules",           get(list_schedules).post(create_schedule))
        .route("/schedules/:id",       put(update_schedule).delete(delete_schedule))
        // Templates
        .route("/templates",           get(list_templates).post(create_template))
        .route("/templates/all",       get(list_all_templates))
        .route("/templates/:id",       get(get_template).delete(delete_template))
        .route("/templates/:id/clone", post(clone_template))
        .route("/templates/:id/publish",   post(publish_template))
        .route("/templates/:id/unpublish", post(unpublish_template))
        // Stats
        .route("/stats",               get(bts_stats))
        .with_state(state)
}

// ── Snapshots ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TakeSnapshotReq {
    name:        String,
    description: Option<String>,
    consistency: Option<String>,
    labels:      Option<std::collections::HashMap<String, String>>,
}

async fn take_snapshot(
    State(s): State<Arc<BtsState>>,
    Path(vm_id): Path<String>,
    Json(req): Json<TakeSnapshotReq>,
) -> ApiResult<Snapshot> {
    let vm_name = vm_id.clone(); // TODO: look up from caiman-api
    let consistency = match req.consistency.as_deref() {
        Some("quiesced") => SnapshotConsistency::Quiesced,
        Some("offline")  => SnapshotConsistency::Offline,
        _                => SnapshotConsistency::CrashConsistent,
    };
    s.snaps.take_snapshot(
        &vm_id, &vm_name, &req.name, req.description,
        consistency, req.labels.unwrap_or_default(), "operator",
    ).await
    .map(Json)
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
struct ListQuery { vm_id: Option<String> }

async fn list_snapshots(
    State(s): State<Arc<BtsState>>,
    Query(q): Query<ListQuery>,
) -> Json<serde_json::Value> {
    let snaps = sqlx::query_as::<_, Snapshot>(
        if q.vm_id.is_some() {
            "SELECT * FROM snapshots WHERE vm_id = ? ORDER BY created_at DESC"
        } else {
            "SELECT * FROM snapshots ORDER BY created_at DESC LIMIT 200"
        }
    )
    .bind(q.vm_id.as_deref().unwrap_or(""))
    .fetch_all(&s.db).await.unwrap_or_default();
    Json(serde_json::json!({ "snapshots": snaps, "total": snaps.len() }))
}

async fn get_snapshot(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let snap = sqlx::query_as::<_, Snapshot>("SELECT * FROM snapshots WHERE id = ?")
        .bind(&id).fetch_optional(&s.db).await.ok().flatten();
    Json(serde_json::to_value(snap).unwrap_or(serde_json::json!(null)))
}

async fn snapshot_chain(State(s): State<Arc<BtsState>>, Path(vm_id): Path<String>)
    -> Json<serde_json::Value>
{
    let snaps = sqlx::query_as::<_, Snapshot>(
        "SELECT * FROM snapshots WHERE vm_id = ? ORDER BY created_at ASC"
    ).bind(&vm_id).fetch_all(&s.db).await.unwrap_or_default();

    // Build tree structure
    let tree: Vec<serde_json::Value> = snaps.iter().map(|sn| serde_json::json!({
        "id":        sn.id,
        "name":      sn.name,
        "parentId":  sn.parent_id,
        "depth":     sn.depth,
        "actualMib": sn.actual_mib,
        "diskMib":   sn.disk_mib,
        "sealed":    sn.sealed,
        "createdAt": sn.created_at,
    })).collect();

    Json(serde_json::json!({ "vmId": vm_id, "chain": tree }))
}

#[derive(Deserialize)]
struct RestoreSnapshotReq {
    #[serde(rename = "targetVmId")]   target_vm_id:  Option<String>,
    #[serde(rename = "targetName")]   target_name:   Option<String>,
    #[serde(rename = "targetNode")]   target_node:   Option<String>,
}

async fn restore_snapshot(
    State(s): State<Arc<BtsState>>,
    Path(id): Path<String>,
    Json(req): Json<RestoreSnapshotReq>,
) -> ApiResult<OperationResult> {
    s.snaps.restore(&id, req.target_vm_id.as_deref(),
        req.target_name.as_deref(), req.target_node.as_deref()).await
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn clone_snapshot(
    State(s): State<Arc<BtsState>>,
    Path(id): Path<String>,
    Json(req): Json<CloneRequest>,
) -> ApiResult<OperationResult> {
    // Snapshot clone is done via template registry (seal + clone)
    s.snaps.restore(&id, None, Some(&req.name), req.node.as_deref()).await
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn seal_snapshot(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let result = s.snaps.seal(&id).await;
    Json(serde_json::json!({ "sealed": result.is_ok(), "id": id }))
}

async fn delete_snapshot(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let result = s.snaps.delete(&id).await;
    Json(serde_json::json!({ "deleted": result.is_ok(), "id": id }))
}

// ── Backups ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct StartBackupReq {
    backup_type: Option<String>,
    target:      BackupTarget,
    parent_id:   Option<BackupId>,
    retention:   Option<RetentionPolicy>,
    description: Option<String>,
}

async fn start_backup(
    State(s): State<Arc<BtsState>>,
    Path(vm_id): Path<String>,
    Json(req): Json<StartBackupReq>,
) -> ApiResult<Backup> {
    let btype = match req.backup_type.as_deref() {
        Some("incremental")  => BackupType::Incremental,
        Some("differential") => BackupType::Differential,
        _                    => BackupType::Full,
    };
    let retention = req.retention.unwrap_or_default();
    let vm_name   = vm_id.clone();

    s.backups.backup_vm(
        &vm_id, &vm_name, req.target, btype,
        req.parent_id, retention, "operator", req.description,
    ).await
    .map(Json)
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn list_backups(State(s): State<Arc<BtsState>>, Query(q): Query<ListQuery>)
    -> Json<serde_json::Value>
{
    let bups = s.backups.list_backups(q.vm_id.as_deref()).await.unwrap_or_default();
    Json(serde_json::json!({ "backups": bups, "total": bups.len() }))
}

async fn get_backup(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let b = sqlx::query_as::<_, Backup>("SELECT * FROM backups WHERE id = ?")
        .bind(&id).fetch_optional(&s.db).await.ok().flatten();
    Json(serde_json::to_value(b).unwrap_or(serde_json::json!(null)))
}

async fn restore_backup(
    State(s): State<Arc<BtsState>>,
    Path(id): Path<String>,
    Json(req): Json<RestoreSnapshotReq>,
) -> ApiResult<OperationResult> {
    s.backups.restore(&id, req.target_vm_id.as_deref(),
        req.target_name.as_deref(), None).await
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn verify_backup(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let ok = s.backups.verify(&id).await.unwrap_or(false);
    Json(serde_json::json!({ "id": id, "valid": ok }))
}

async fn delete_backup(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    sqlx::query("DELETE FROM backups WHERE id = ?")
        .bind(&id).execute(&s.db).await.ok();
    Json(serde_json::json!({ "deleted": true, "id": id }))
}

// ── Schedules ─────────────────────────────────────────────────────────────

async fn list_schedules(State(s): State<Arc<BtsState>>) -> Json<serde_json::Value> {
    let scheds = sqlx::query_as::<_, BackupSchedule>("SELECT * FROM backup_schedules")
        .fetch_all(&s.db).await.unwrap_or_default();
    Json(serde_json::json!({ "schedules": scheds }))
}

async fn create_schedule(State(s): State<Arc<BtsState>>, Json(body): Json<serde_json::Value>)
    -> Json<serde_json::Value>
{
    Json(serde_json::json!({ "status": "created", "schedule": body }))
}

async fn update_schedule(State(_): State<Arc<BtsState>>, Path(id): Path<String>, Json(body): Json<serde_json::Value>)
    -> Json<serde_json::Value>
{
    Json(serde_json::json!({ "status": "updated", "id": id }))
}

async fn delete_schedule(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    sqlx::query("DELETE FROM backup_schedules WHERE id = ?")
        .bind(&id).execute(&s.db).await.ok();
    Json(serde_json::json!({ "deleted": true, "id": id }))
}

// ── Templates ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateTemplateReq {
    snap_id:     String,
    name:        String,
    version:     String,
    description: Option<String>,
    os_type:     Option<String>,
    os_version:  String,
    default_mem: Option<u64>,
    default_cpus:Option<u8>,
    cloud_init:  Option<String>,
    labels:      Option<std::collections::HashMap<String, String>>,
}

async fn create_template(
    State(s): State<Arc<BtsState>>,
    Json(req): Json<CreateTemplateReq>,
) -> ApiResult<VmTemplate> {
    let os = match req.os_type.as_deref() {
        Some("windows") => OsType::Windows,
        Some("freebsd") => OsType::FreeBSD,
        _               => OsType::Linux,
    };
    let cfg = TemplateDefaultCfg {
        mem_mib:  req.default_mem.unwrap_or(512),
        cpus:     req.default_cpus.unwrap_or(1),
        disk_gib: 20,
        uplink:   "eth0".into(),
    };
    s.templates.create_from_snapshot(
        &req.snap_id, &req.name, &req.version,
        req.description, os, &req.os_version,
        cfg, req.cloud_init, req.labels.unwrap_or_default(), "operator",
    ).await
    .map(Json)
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn list_templates(State(s): State<Arc<BtsState>>) -> Json<serde_json::Value> {
    let tmpls = s.templates.list(true).await.unwrap_or_default();
    Json(serde_json::json!({ "templates": tmpls, "total": tmpls.len() }))
}

async fn list_all_templates(State(s): State<Arc<BtsState>>) -> Json<serde_json::Value> {
    let tmpls = s.templates.list(false).await.unwrap_or_default();
    Json(serde_json::json!({ "templates": tmpls }))
}

async fn get_template(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let t = sqlx::query_as::<_, VmTemplate>("SELECT * FROM templates WHERE id = ?")
        .bind(&id).fetch_optional(&s.db).await.ok().flatten();
    Json(serde_json::to_value(t).unwrap_or(serde_json::json!(null)))
}

async fn clone_template(
    State(s): State<Arc<BtsState>>,
    Path(id): Path<String>,
    Json(mut req): Json<CloneRequest>,
) -> ApiResult<OperationResult> {
    let mut r = req;
    r.source_id = id;
    s.templates.clone(&r).await
        .map(Json)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

async fn publish_template(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let ok = s.templates.publish(&id).await.is_ok();
    Json(serde_json::json!({ "published": ok, "id": id }))
}

async fn unpublish_template(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    let ok = s.templates.unpublish(&id).await.is_ok();
    Json(serde_json::json!({ "published": false, "id": id }))
}

async fn delete_template(State(s): State<Arc<BtsState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(&id).execute(&s.db).await.ok();
    Json(serde_json::json!({ "deleted": true, "id": id }))
}

// ── Stats ─────────────────────────────────────────────────────────────────

async fn bts_stats(State(s): State<Arc<BtsState>>) -> Json<serde_json::Value> {
    let snap_count:  (i64,) = sqlx::query_as("SELECT COUNT(*) FROM snapshots").fetch_one(&s.db).await.unwrap_or((0,));
    let snap_mib:    (i64,) = sqlx::query_as("SELECT COALESCE(SUM(actual_mib),0) FROM snapshots").fetch_one(&s.db).await.unwrap_or((0,));
    let backup_count:(i64,) = sqlx::query_as("SELECT COUNT(*) FROM backups WHERE status='Completed'").fetch_one(&s.db).await.unwrap_or((0,));
    let backup_mib:  (i64,) = sqlx::query_as("SELECT COALESCE(SUM(size_mib),0) FROM backups WHERE status='Completed'").fetch_one(&s.db).await.unwrap_or((0,));
    let tmpl_count:  (i64,) = sqlx::query_as("SELECT COUNT(*) FROM templates WHERE published=1").fetch_one(&s.db).await.unwrap_or((0,));
    let clone_count: (i64,) = sqlx::query_as("SELECT COALESCE(SUM(clone_count),0) FROM templates").fetch_one(&s.db).await.unwrap_or((0,));

    Json(serde_json::json!({
        "snapshots":  { "count": snap_count.0,   "totalMib": snap_mib.0 },
        "backups":    { "count": backup_count.0,  "totalMib": backup_mib.0 },
        "templates":  { "published": tmpl_count.0,"totalClones": clone_count.0 },
    }))
}
