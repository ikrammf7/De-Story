pub mod repo;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use crate::paths::db_path;

pub type DbPool = Arc<Mutex<Connection>>;

pub(crate) const MIGRATIONS: &[&str] = &[
    // Migration 0
    r#"
    CREATE TABLE projects (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        synopsis TEXT NOT NULL DEFAULT '',
        duration_sec INTEGER NOT NULL DEFAULT 60,
        language TEXT NOT NULL DEFAULT 'id',
        aspect_ratio TEXT NOT NULL DEFAULT '16:9',
        style_id TEXT NOT NULL DEFAULT 'stickman',
        tts_provider TEXT NOT NULL DEFAULT 'gemini',
        tts_voice TEXT NOT NULL DEFAULT 'Charon',
        image_model TEXT,
        status TEXT NOT NULL DEFAULT 'draft',
        step INTEGER NOT NULL DEFAULT 1,
        cover_asset_id TEXT,
        editor_json TEXT NOT NULL DEFAULT '{}',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE characters (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        sheet_asset_id TEXT,
        sort INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX characters_project ON characters(project_id);
    CREATE TABLE clips (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        sort INTEGER NOT NULL,
        title TEXT NOT NULL DEFAULT '',
        visual_prompt TEXT NOT NULL DEFAULT '',
        narration TEXT NOT NULL DEFAULT '',
        duration_ms INTEGER NOT NULL DEFAULT 5000,
        character_ids TEXT NOT NULL DEFAULT '[]',
        motion_type TEXT NOT NULL DEFAULT 'camera',
        camera_preset TEXT NOT NULL DEFAULT 'zoomin',
        motion_strength TEXT NOT NULL DEFAULT 'halus',
        video_prompt TEXT NOT NULL DEFAULT '',
        transition TEXT NOT NULL DEFAULT 'fade',
        image_asset_id TEXT,
        video_asset_id TEXT,
        audio_asset_id TEXT
    );
    CREATE INDEX clips_project ON clips(project_id, sort);
    CREATE TABLE assets (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        clip_id TEXT,
        character_id TEXT,
        kind TEXT NOT NULL,
        provider TEXT,
        model TEXT,
        prompt TEXT,
        remote_id TEXT,
        remote_url TEXT,
        local_path TEXT NOT NULL,
        duration_ms INTEGER,
        width INTEGER,
        height INTEGER,
        meta_json TEXT NOT NULL DEFAULT '{}',
        created_at INTEGER NOT NULL
    );
    CREATE INDEX assets_project ON assets(project_id);
    CREATE TABLE jobs (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        clip_id TEXT,
        character_id TEXT,
        kind TEXT NOT NULL,
        provider TEXT NOT NULL,
        remote_id TEXT,
        status TEXT NOT NULL,
        progress REAL NOT NULL DEFAULT 0,
        message TEXT,
        error TEXT,
        payload_json TEXT NOT NULL DEFAULT '{}',
        result_asset_id TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX jobs_project ON jobs(project_id, status);
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE secrets (
        provider TEXT PRIMARY KEY,
        value BLOB NOT NULL,
        last_ok INTEGER,
        last_message TEXT,
        checked_at INTEGER
    );
    "#,
    // Migration 1
    r#"
    CREATE TABLE cache (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
    "#,
    // Migration 2
    r#"
    ALTER TABLE projects ADD COLUMN video_model TEXT;
    "#,
    // Migration 3
    r#"
    ALTER TABLE clips ADD COLUMN story TEXT NOT NULL DEFAULT '';
    ALTER TABLE clips ADD COLUMN visual_source TEXT;
    UPDATE projects SET step = step + 1 WHERE step >= 2;
    "#,
    // Migration 4
    r#"
    ALTER TABLE projects ADD COLUMN video_resolution TEXT;
    "#
];

pub fn init_db() -> Result<DbPool, rusqlite::Error> {
    let path = db_path();
    let conn = Connection::open(&path)?;

    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;

    let user_version: usize = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    for v in user_version..MIGRATIONS.len() {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(MIGRATIONS[v])?;
        tx.pragma_update(None, "user_version", (v + 1) as i64)?;
        tx.commit()?;
    }

    Ok(Arc::new(Mutex::new(conn)))
}
