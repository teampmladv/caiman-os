-- caiman-api initial schema
CREATE TABLE IF NOT EXISTS audit_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    action_id  TEXT NOT NULL,
    user       TEXT,
    input      TEXT,
    timestamp  INTEGER NOT NULL
);
