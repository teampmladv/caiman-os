-- migrations/001_bts_catalog.sql
-- Caimán BTS: Backup, Templates & Snapshots catalog
-- Applied by SQLx migrate at startup

-- ── Snapshots ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS snapshots (
    id            TEXT PRIMARY KEY,
    vm_id         TEXT NOT NULL,
    vm_name       TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT,
    image_path    TEXT NOT NULL,
    parent_id     TEXT REFERENCES snapshots(id) ON DELETE SET NULL,
    depth         INTEGER NOT NULL DEFAULT 0,
    disk_mib      INTEGER NOT NULL DEFAULT 0,
    actual_mib    INTEGER NOT NULL DEFAULT 0,
    checksum      TEXT NOT NULL DEFAULT '',
    has_memory    BOOLEAN NOT NULL DEFAULT 0,
    sealed        BOOLEAN NOT NULL DEFAULT 0,
    consistency   TEXT NOT NULL DEFAULT 'CrashConsistent',
    labels        TEXT NOT NULL DEFAULT '{}',
    created_at    TEXT NOT NULL,
    created_by    TEXT NOT NULL DEFAULT 'system'
);

CREATE INDEX IF NOT EXISTS idx_snaps_vm_id    ON snapshots(vm_id);
CREATE INDEX IF NOT EXISTS idx_snaps_parent   ON snapshots(parent_id);
CREATE INDEX IF NOT EXISTS idx_snaps_created  ON snapshots(created_at DESC);

-- ── Backups ────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS backups (
    id            TEXT PRIMARY KEY,
    vm_id         TEXT NOT NULL,
    vm_name       TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT,
    backup_type   TEXT NOT NULL DEFAULT 'Full',
    status        TEXT NOT NULL DEFAULT 'Pending',
    target        TEXT NOT NULL DEFAULT '{}',
    parent_id     TEXT REFERENCES backups(id) ON DELETE SET NULL,
    restic_id     TEXT,
    size_mib      INTEGER NOT NULL DEFAULT 0,
    raw_mib       INTEGER NOT NULL DEFAULT 0,
    ratio         REAL    NOT NULL DEFAULT 1.0,
    dedup_mib     INTEGER NOT NULL DEFAULT 0,
    checksum      TEXT,
    retention     TEXT NOT NULL DEFAULT '{}',
    expires_at    TEXT,
    started_at    TEXT NOT NULL,
    finished_at   TEXT,
    duration_secs INTEGER,
    created_by    TEXT NOT NULL DEFAULT 'system',
    error         TEXT
);

CREATE INDEX IF NOT EXISTS idx_bup_vm_id   ON backups(vm_id);
CREATE INDEX IF NOT EXISTS idx_bup_status  ON backups(status);
CREATE INDEX IF NOT EXISTS idx_bup_started ON backups(started_at DESC);

-- ── Backup schedules ────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS backup_schedules (
    id            TEXT PRIMARY KEY,
    vm_id         TEXT,
    name          TEXT NOT NULL,
    cron_expr     TEXT NOT NULL,
    backup_type   TEXT NOT NULL DEFAULT 'Full',
    target        TEXT NOT NULL DEFAULT '{}',
    retention     TEXT NOT NULL DEFAULT '{}',
    enabled       BOOLEAN NOT NULL DEFAULT 1,
    last_run      TEXT,
    next_run      TEXT,
    created_at    TEXT NOT NULL
);

-- ── Templates ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS templates (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    description   TEXT,
    version       TEXT NOT NULL DEFAULT '1.0.0',
    os_type       TEXT NOT NULL DEFAULT 'Linux',
    os_version    TEXT NOT NULL,
    image_path    TEXT NOT NULL,
    image_mib     INTEGER NOT NULL DEFAULT 0,
    checksum      TEXT NOT NULL DEFAULT '',
    default_cfg   TEXT NOT NULL DEFAULT '{}',
    cloud_init    TEXT,
    network_cfg   TEXT,
    labels        TEXT NOT NULL DEFAULT '{}',
    source        TEXT NOT NULL DEFAULT 'Snapshot',
    clone_count   INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    created_by    TEXT NOT NULL DEFAULT 'system',
    published     BOOLEAN NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_tmpl_published ON templates(published);
CREATE INDEX IF NOT EXISTS idx_tmpl_os        ON templates(os_type, os_version);

-- ── Restore jobs ───────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS restore_jobs (
    id             TEXT PRIMARY KEY,
    source_type    TEXT NOT NULL,
    source_id      TEXT NOT NULL,
    target_vm_id   TEXT,
    target_name    TEXT,
    status         TEXT NOT NULL DEFAULT 'pending',
    progress_pct   REAL NOT NULL DEFAULT 0,
    phase          TEXT NOT NULL DEFAULT 'init',
    started_at     TEXT NOT NULL,
    finished_at    TEXT,
    error          TEXT
);

-- ── Audit log ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS bts_audit (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    op_type     TEXT NOT NULL,
    resource_id TEXT,
    user        TEXT,
    metadata    TEXT NOT NULL DEFAULT '{}',
    created_at  TEXT NOT NULL
);
