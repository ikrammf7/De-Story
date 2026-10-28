use std::collections::HashMap;
use std::fs;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::paths::{asset_url, project_dir, projects_root};
use crate::secrets::{decrypt_dpapi, encrypt_dpapi, mask, normalized_gemini_keys, KeyStatus, PROVIDERS};

// ==========================================
// Domain Structs
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub title: String,
    pub synopsis: String,
    pub duration_sec: i64,
    pub language: String,
    pub aspect_ratio: String,
    pub style_id: String,
    pub tts_provider: String,
    pub tts_voice: String,
    pub image_model: Option<String>,
    pub video_model: Option<String>,
    pub video_resolution: Option<String>,
    pub status: String,
    pub step: i64,
    pub cover_asset_id: Option<String>,
    pub editor: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub sheet_asset_id: Option<String>,
    pub sort: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipRow {
    pub id: String,
    pub project_id: String,
    pub sort: i64,
    pub title: String,
    pub story: String,
    pub visual_prompt: String,
    pub visual_source: Option<String>,
    pub narration: String,
    pub duration_ms: i64,
    pub character_ids: Vec<String>,
    pub motion_type: String,
    pub camera_preset: String,
    pub motion_strength: String,
    pub video_prompt: String,
    pub transition: String,
    pub image_asset_id: Option<String>,
    pub video_asset_id: Option<String>,
    pub audio_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRow {
    pub id: String,
    pub project_id: String,
    pub clip_id: Option<String>,
    pub character_id: Option<String>,
    pub kind: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub remote_id: Option<String>,
    pub remote_url: Option<String>,
    pub local_path: String,
    pub url: String,
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub meta: Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBundle {
    pub project: ProjectRow,
    pub characters: Vec<CharacterRow>,
    pub clips: Vec<ClipRow>,
    pub assets: Vec<AssetRow>,
    pub jobs: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAsset {
    pub project_id: String,
    pub clip_id: Option<String>,
    pub character_id: Option<String>,
    pub kind: String,
    pub provider: String,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub remote_id: Option<String>,
    pub remote_url: Option<String>,
    pub local_path: String,
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub meta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewProjectInput {
    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub duration_sec: Option<i64>,
    pub language: Option<String>,
    pub aspect_ratio: Option<String>,
    pub style_id: Option<String>,
    pub tts_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project: Value,
    #[serde(default)]
    pub characters: Option<Vec<Value>>,
    #[serde(default)]
    pub clips: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewScriptScene {
    pub title: String,
    pub story: String,
    pub narration: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneVisualUpdate {
    pub clip_id: String,
    pub visual_prompt: String,
    pub character_names: Vec<String>,
    pub camera_preset: String,
    pub motion_strength: String,
    pub video_prompt: String,
    pub transition: String,
}

// ==========================================
// Defaults
// ==========================================

pub fn default_editor() -> Value {
    json!({
        "captionStyle": "karaoke",
        "captionPosition": "bawah",
        "captionSize": "sedang",
        "musicAssetId": null,
        "musicVolumeDb": -18,
        "duckMusic": true,
        "overlays": []
    })
}

pub fn default_settings() -> Value {
    json!({
        "defaultTtsProvider": "gemini",
        "defaultLanguage": "id",
        "exportFolder": null,
        "llmProvider": "gemini",
        "llmModels": { "gemini": "gemini-3.8-flash", "openrouter": "", "groq": "", "custom": "" },
        "customBaseUrl": "",
        "flow2apiBaseUrl": "http://localhost:8000",
        "flow2apiMode": "bundled",
        "geminiTtsModel": "gemini-3.8-flash-tts",
        "elevenModel": "eleven_multilingual_v2",
        "imageModel": "marketing-studio/image/sunburst",
        "videoModel": "kling-video/v3.0/std/image-to-video",
        "whisperAuto": true
    })
}

// ==========================================
// Settings
// ==========================================

pub fn get_settings(conn: &Connection) -> Value {
    let mut map = default_settings();
    let mut stmt = match conn.prepare("SELECT key, value FROM settings") {
        Ok(s) => s,
        Err(_) => return map,
    };
    let rows = match stmt.query_map([], |row| {
        let k: String = row.get(0)?;
        let v: String = row.get(1)?;
        Ok((k, v))
    }) {
        Ok(r) => r,
        Err(_) => return map,
    };

    let mut llm_models = map["llmModels"].clone();

    for r in rows.flatten() {
        if let Ok(parsed) = serde_json::from_str::<Value>(&r.1) {
            if r.0 == "llmModels" {
                if let (Some(target), Some(source)) = (llm_models.as_object_mut(), parsed.as_object()) {
                    for (k, v) in source {
                        target.insert(k.clone(), v.clone());
                    }
                }
            } else if let Some(target) = map.as_object_mut() {
                target.insert(r.0, parsed);
            }
        }
    }
    map["llmModels"] = llm_models;
    map
}

pub fn set_settings(conn: &Connection, patch: Value) -> Result<Value, String> {
    let current = get_settings(conn);
    let mut stmt = conn.prepare(
        "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    ).map_err(|e| e.to_string())?;

    if let Some(obj) = patch.as_object() {
        for (k, v) in obj {
            let val = if k == "llmModels" {
                let mut base = current["llmModels"].clone();
                if let (Some(b), Some(p)) = (base.as_object_mut(), v.as_object()) {
                    for (pk, pv) in p {
                        b.insert(pk.clone(), pv.clone());
                    }
                }
                base
            } else {
                v.clone()
            };
            stmt.execute(params![k, serde_json::to_string(&val).unwrap_or_default()]).map_err(|e| e.to_string())?;
        }
    }
    Ok(get_settings(conn))
}

pub use set_settings as patch_settings;

// ==========================================
// Cache
// ==========================================

pub fn read_cache(conn: &Connection, key: &str) -> Option<(Value, i64)> {
    conn.query_row("SELECT value, updated_at FROM cache WHERE key = ?", params![key], |row| {
        let val: String = row.get(0)?;
        let updated: i64 = row.get(1)?;
        Ok((val, updated))
    }).ok().and_then(|(v, t)| serde_json::from_str::<Value>(&v).ok().map(|val| (val, t)))
}

pub fn write_cache(conn: &Connection, key: &str, val: &Value) -> Result<i64, String> {
    let now = chrono_now();
    let s = serde_json::to_string(val).unwrap_or_default();
    conn.execute(
        "INSERT INTO cache (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, s, now]
    ).map_err(|e| e.to_string())?;
    Ok(now)
}

pub fn drop_cache(conn: &Connection, prefix: &str) -> Result<(), String> {
    conn.execute("DELETE FROM cache WHERE key LIKE ?", params![format!("{prefix}%")]).map_err(|e| e.to_string())?;
    Ok(())
}

// ==========================================
// Secrets
// ==========================================

pub fn key_statuses(conn: &Connection) -> Result<Vec<KeyStatus>, String> {
    let mut list = Vec::new();
    let settings = get_settings(conn);
    let custom_url = settings.get("customBaseUrl").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let flow_url = settings.get("flow2apiBaseUrl").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();

    for &p in PROVIDERS {
        let row: Option<(Vec<u8>, Option<i64>, Option<String>, Option<i64>)> = conn.query_row(
            "SELECT value, last_ok, last_message, checked_at FROM secrets WHERE provider = ?",
            params![p],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        ).optional().map_err(|e| e.to_string())?;

        let (has_key, key_count, preview, unreadable, last_ok, last_message, checked_at) = match row {
            Some((blob, ok, msg, ts)) if !blob.is_empty() => {
                match decrypt_dpapi(&blob) {
                    Ok(plain) if !plain.is_empty() => {
                        if blob.starts_with(b"v10") {
                            let _ = set_secret(conn, p, &plain);
                        }
                        let keys = if p == "gemini" {
                            normalized_gemini_keys(&plain)
                        } else {
                            vec![plain]
                        };
                        let count = keys.len();
                        let preview = keys.first().map(|k| mask(k));
                        (count > 0, count, preview, false, ok.map(|v| v != 0), msg, ts)
                    }
                    _ => (true, 0, None, true, Some(false), Some("Kunci tersimpan tapi tidak bisa dibuka lagi. Masukkan ulang kuncinya di Pengaturan.".into()), ts),
                }
            }
            Some((_blob, ok, msg, ts)) => {
                (false, 0, None, false, ok.map(|v| v != 0), msg, ts)
            }
            None => (false, 0, None, false, None, None, None),
        };

        let configured = if p == "custom" {
            !custom_url.is_empty()
        } else if p == "flow2api" {
            !flow_url.is_empty()
        } else {
            has_key
        };

        list.push(KeyStatus {
            provider: p.to_string(),
            configured,
            key_count,
            last_ok,
            last_message,
            checked_at,
            preview,
            unreadable,
        });
    }
    Ok(list)
}

pub fn get_secret(conn: &Connection, provider: &str) -> Option<String> {
    let blob: Option<Vec<u8>> = conn.query_row(
        "SELECT value FROM secrets WHERE provider = ?",
        params![provider],
        |r| r.get(0)
    ).optional().ok().flatten();

    match blob {
        Some(b) if !b.is_empty() => {
            if let Ok(plain) = decrypt_dpapi(&b) {
                if b.starts_with(b"v10") {
                    let _ = set_secret(conn, provider, &plain);
                }
                Some(plain)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Read the exact encrypted value alongside parsed keys so a later network check
/// cannot overwrite the status of a different pool.
pub fn gemini_secret_snapshot(conn: &Connection) -> Result<Option<(Vec<u8>, Vec<String>)>, String> {
    let blob: Option<Vec<u8>> = conn.query_row(
        "SELECT value FROM secrets WHERE provider = 'gemini'", [], |r| r.get(0)
    ).optional().map_err(|e| e.to_string())?;
    match blob {
        Some(blob) if !blob.is_empty() => {
            let plain = decrypt_dpapi(&blob)
                .map_err(|_| "Kunci Gemini tersimpan tapi tidak bisa dibuka lagi. Masukkan ulang kuncinya di Pengaturan.".to_string())?;
            Ok(Some((blob, normalized_gemini_keys(&plain))))
        }
        _ => Ok(None),
    }
}

pub fn append_gemini_keys(conn: &Connection, keys: &[String]) -> Result<(), String> {
    // The caller holds the DB lock across both the read and write, so the pool
    // that is appended to cannot change in between. Never treat decryption
    // failure as an empty pool: that would silently destroy old keys.
    let mut pool = match gemini_secret_snapshot(conn) {
        Ok(snapshot) => snapshot.map(|(_, keys)| keys).unwrap_or_default(),
        Err(e) => return Err(format!("Gagal membaca kunci Gemini lama: {e}")),
    };
    for key in normalized_gemini_keys(&keys.join("\n")) {
        if !pool.contains(&key) {
            pool.push(key);
        }
    }
    if pool.is_empty() {
        return Err("Kunci Gemini masih kosong".into());
    }
    set_secret(conn, "gemini", &pool.join("\n"))
}

pub fn set_secret(conn: &Connection, provider: &str, plain: &str) -> Result<(), String> {
    let normalized;
    let plain = if provider == "gemini" {
        normalized = normalized_gemini_keys(plain).join("\n");
        if normalized.is_empty() {
            return Err("Kunci Gemini masih kosong".into());
        }
        normalized.as_str()
    } else {
        plain
    };
    let enc = encrypt_dpapi(plain)?;
    conn.execute(
        "INSERT INTO secrets (provider, value, last_ok, last_message, checked_at) VALUES (?, ?, NULL, NULL, NULL)
         ON CONFLICT(provider) DO UPDATE SET value = excluded.value, last_ok = NULL, last_message = NULL, checked_at = NULL",
        params![provider, enc]
    ).map_err(|e| e.to_string())?;
    if provider == "gemini" {
        clear_gemini_model_cache(conn)?;
    }
    Ok(())
}

fn clear_gemini_model_cache(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM cache WHERE key IN ('models:gemini', 'models:gemini-tts')", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn record_gemini_check_if_unchanged(
    conn: &Connection,
    blob: &[u8],
    ok: bool,
    message: Option<&str>,
) -> Result<bool, String> {
    let changed = conn.execute(
        "UPDATE secrets SET last_ok = ?, last_message = ?, checked_at = ? WHERE provider = 'gemini' AND value = ?",
        params![if ok { 1 } else { 0 }, message, chrono_now(), blob],
    ).map_err(|e| e.to_string())?;
    Ok(changed > 0)
}

pub fn clear_secret(conn: &Connection, provider: &str) -> Result<(), String> {
    conn.execute("DELETE FROM secrets WHERE provider = ?", params![provider]).map_err(|e| e.to_string())?;
    if provider == "gemini" {
        clear_gemini_model_cache(conn)?;
    }
    if provider == "flow2api" {
        let _ = set_settings(conn, json!({ "flow2apiBaseUrl": "http://localhost:8000" }));
    }
    Ok(())
}

pub fn reveal_secret(conn: &Connection, provider: &str) -> Option<String> {
    get_secret(conn, provider)
}

pub fn record_check(conn: &Connection, provider: &str, ok: bool, message: Option<&str>) -> Result<(), String> {
    let now = chrono_now();
    let changes = conn.execute(
        "UPDATE secrets SET last_ok = ?, last_message = ?, checked_at = ? WHERE provider = ?",
        params![if ok { 1 } else { 0 }, message, now, provider]
    ).map_err(|e| e.to_string())?;

    if changes == 0 && (provider == "custom" || provider == "flow2api") {
        conn.execute(
            "INSERT INTO secrets (provider, value, last_ok, last_message, checked_at) VALUES (?, X'', ?, ?, ?)",
            params![provider, if ok { 1 } else { 0 }, message, now]
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ==========================================
// Projects
// ==========================================

pub fn list_projects(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.style_id, p.status, p.step, p.updated_at,
            (SELECT COUNT(*) FROM clips c WHERE c.project_id = p.id) AS clip_count,
            (SELECT COALESCE(SUM(duration_ms), 0) FROM clips c WHERE c.project_id = p.id) AS total_ms,
            (SELECT a.local_path FROM assets a WHERE a.id = p.cover_asset_id) AS cover_path
         FROM projects p ORDER BY p.updated_at DESC"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let style_id: String = row.get(2)?;
        let status: String = row.get(3)?;
        let step: i64 = row.get(4)?;
        let updated_at: i64 = row.get(5)?;
        let clip_count: i64 = row.get(6)?;
        let total_ms: i64 = row.get(7)?;
        let cover_path: Option<String> = row.get(8)?;

        let cover_url = cover_path.map(|cp| asset_url(&id, &cp));

        Ok(json!({
            "id": id,
            "title": title,
            "styleId": style_id,
            "status": status,
            "step": step,
            "clipCount": clip_count,
            "totalMs": total_ms,
            "coverUrl": cover_url,
            "updatedAt": updated_at
        }))
    }).map_err(|e| e.to_string())?;

    let mut list = Vec::new();
    for r in rows.flatten() {
        list.push(r);
    }
    Ok(list)
}

pub fn get_project(conn: &Connection, id: &str) -> Option<ProjectRow> {
    let mut stmt = conn.prepare("SELECT * FROM projects WHERE id = ?").ok()?;
    stmt.query_row(params![id], |row| {
        let p_id: String = row.get("id")?;
        let title: String = row.get("title")?;
        let synopsis: String = row.get("synopsis")?;
        let duration_sec: i64 = row.get("duration_sec")?;
        let language: String = row.get("language")?;
        let aspect_ratio: String = row.get("aspect_ratio")?;
        let style_id: String = row.get("style_id")?;
        let tts_provider: String = row.get("tts_provider")?;
        let tts_voice: String = row.get("tts_voice")?;
        let image_model: Option<String> = row.get("image_model")?;
        let video_model: Option<String> = row.get("video_model")?;
        let video_resolution: Option<String> = row.get("video_resolution")?;
        let status: String = row.get("status")?;
        let step: i64 = row.get("step")?;
        let cover_asset_id: Option<String> = row.get("cover_asset_id")?;
        let editor_json: String = row.get("editor_json")?;
        let created_at: i64 = row.get("created_at")?;
        let updated_at: i64 = row.get("updated_at")?;

        let mut editor = default_editor();
        if let Ok(parsed) = serde_json::from_str::<Value>(&editor_json) {
            if let (Some(target), Some(source)) = (editor.as_object_mut(), parsed.as_object()) {
                for (k, v) in source {
                    target.insert(k.clone(), v.clone());
                }
            }
        }

        Ok(ProjectRow {
            id: p_id,
            title,
            synopsis,
            duration_sec,
            language,
            aspect_ratio,
            style_id,
            tts_provider,
            tts_voice,
            image_model,
            video_model,
            video_resolution,
            status,
            step,
            cover_asset_id,
            editor,
            created_at,
            updated_at,
        })
    }).ok()
}

pub fn create_project(conn: &Connection, input: NewProjectInput) -> Result<ProjectRow, String> {
    let s = get_settings(conn);
    let now = chrono_now();
    let id = Uuid::new_v4().to_string();

    let tts = input.tts_provider.as_deref()
        .or_else(|| s.get("defaultTtsProvider").and_then(|v| v.as_str()))
        .unwrap_or("gemini");

    let voice = if tts == "gemini" { "Charon" } else { "" };
    let lang = input.language.as_deref()
        .or_else(|| s.get("defaultLanguage").and_then(|v| v.as_str()))
        .unwrap_or("id");

    conn.execute(
        "INSERT INTO projects (id, title, synopsis, duration_sec, language, aspect_ratio, style_id, tts_provider, tts_voice,
            image_model, video_model, status, step, editor_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft', 1, ?, ?, ?)",
        params![
            id,
            input.title.unwrap_or_else(|| "Proyek tanpa judul".to_string()),
            input.synopsis.unwrap_or_default(),
            input.duration_sec.unwrap_or(60),
            lang,
            input.aspect_ratio.unwrap_or_else(|| "16:9".to_string()),
            input.style_id.unwrap_or_else(|| "stickman".to_string()),
            tts,
            voice,
            s.get("imageModel").and_then(|v| v.as_str()),
            s.get("videoModel").and_then(|v| v.as_str()),
            serde_json::to_string(&default_editor()).unwrap_or_default(),
            now,
            now
        ]
    ).map_err(|e| e.to_string())?;

    let _ = project_dir(&id);
    get_project(conn, &id).ok_or_else(|| "Failed to retrieve created project".to_string())
}

pub fn touch_project(conn: &Connection, project_id: &str, status: Option<&str>) -> Result<(), String> {
    let now = chrono_now();
    if let Some(s) = status {
        conn.execute(
            "UPDATE projects SET updated_at = ?, status = ? WHERE id = ?",
            params![now, s, project_id],
        )
    } else {
        conn.execute(
            "UPDATE projects SET updated_at = ? WHERE id = ?",
            params![now, project_id],
        )
    }.map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_bundle(conn: &Connection, id: &str) -> Option<ProjectBundle> {
    let proj = get_project(conn, id)?;
    let characters = characters_of(conn, id);
    let clips = clips_of(conn, id);

    let mut a_stmt = conn.prepare("SELECT * FROM assets WHERE project_id = ? ORDER BY created_at").ok()?;
    let assets: Vec<AssetRow> = a_stmt.query_map(params![id], |row| {
        let p_id: String = row.get("project_id")?;
        let local_path: String = row.get("local_path")?;
        let meta_str: String = row.get("meta_json")?;
        let meta: Value = serde_json::from_str(&meta_str).unwrap_or_else(|_| json!({}));

        Ok(AssetRow {
            id: row.get::<_, String>("id")?,
            project_id: p_id.clone(),
            clip_id: row.get::<_, Option<String>>("clip_id")?,
            character_id: row.get::<_, Option<String>>("character_id")?,
            kind: row.get::<_, String>("kind")?,
            provider: row.get::<_, Option<String>>("provider")?,
            model: row.get::<_, Option<String>>("model")?,
            prompt: row.get::<_, Option<String>>("prompt")?,
            remote_id: row.get::<_, Option<String>>("remote_id")?,
            remote_url: row.get::<_, Option<String>>("remote_url")?,
            local_path: local_path.clone(),
            url: asset_url(&p_id, &local_path),
            duration_ms: row.get::<_, Option<i64>>("duration_ms")?,
            width: row.get::<_, Option<i64>>("width")?,
            height: row.get::<_, Option<i64>>("height")?,
            meta,
            created_at: row.get::<_, i64>("created_at")?,
        })
    }).ok()?.flatten().collect();

    let mut j_stmt = conn.prepare(
        "SELECT * FROM jobs WHERE project_id = ? AND (status IN ('queued','running') OR updated_at > ?) ORDER BY created_at"
    ).ok()?;
    let cutoff = chrono_now() - 10 * 60 * 1000;
    let jobs: Vec<Value> = j_stmt.query_map(params![id, cutoff], |row| {
        row_to_job(row)
    }).ok()?.flatten().collect();

    Some(ProjectBundle {
        project: proj,
        characters,
        clips,
        assets,
        jobs,
    })
}

pub fn save_snapshot(conn: &Connection, snap: ProjectSnapshot) -> Result<i64, String> {
    let now = chrono_now();
    let p = &snap.project;
    let p_id = p.get("id").and_then(|v| v.as_str()).ok_or("Missing project id")?;

    conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;

    let editor_str = serde_json::to_string(&p.get("editor").cloned().unwrap_or_else(default_editor)).unwrap_or_default();

    let res: Result<(), String> = (|| {
        conn.execute(
            "UPDATE projects SET title = ?, synopsis = ?, duration_sec = ?, language = ?,
                aspect_ratio = ?, style_id = ?, tts_provider = ?, tts_voice = ?,
                image_model = ?, video_model = ?, video_resolution = ?, status = ?, step = ?, editor_json = ?, updated_at = ?
             WHERE id = ?",
            params![
                p.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                p.get("synopsis").and_then(|v| v.as_str()).unwrap_or(""),
                p.get("durationSec").and_then(|v| v.as_i64()).unwrap_or(60),
                p.get("language").and_then(|v| v.as_str()).unwrap_or("id"),
                p.get("aspectRatio").and_then(|v| v.as_str()).unwrap_or("16:9"),
                p.get("styleId").and_then(|v| v.as_str()).unwrap_or("stickman"),
                p.get("ttsProvider").and_then(|v| v.as_str()).unwrap_or("gemini"),
                p.get("ttsVoice").and_then(|v| v.as_str()).unwrap_or(""),
                p.get("imageModel").and_then(|v| v.as_str()),
                p.get("videoModel").and_then(|v| v.as_str()),
                p.get("videoResolution").and_then(|v| v.as_str()),
                p.get("status").and_then(|v| v.as_str()).unwrap_or("draft"),
                p.get("step").and_then(|v| v.as_i64()).unwrap_or(1),
                editor_str,
                now,
                p_id
            ]
        ).map_err(|e| e.to_string())?;

        // Characters
        if let Some(ref chars) = snap.characters {
            let char_ids: Vec<String> = chars.iter().filter_map(|c| c.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())).collect();
            let placeholders = if char_ids.is_empty() { "''".to_string() } else { char_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",") };
            let del_query = format!("DELETE FROM characters WHERE project_id = ? AND id NOT IN ({placeholders})");
            let mut del_params: Vec<&dyn rusqlite::ToSql> = vec![&p_id];
            for cid in &char_ids {
                del_params.push(cid);
            }
            conn.execute(&del_query, rusqlite::params_from_iter(del_params)).map_err(|e| e.to_string())?;

            for c in chars {
                if let Some(cid) = c.get("id").and_then(|v| v.as_str()) {
                    conn.execute(
                        "INSERT INTO characters (id, project_id, name, description, sort) VALUES (?, ?, ?, ?, ?)
                         ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description, sort = excluded.sort",
                        params![
                            cid,
                            p_id,
                            c.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("sort").and_then(|v| v.as_i64()).unwrap_or(0)
                        ]
                    ).map_err(|e| e.to_string())?;
                }
            }
        }

        // Clips
        if let Some(ref clips) = snap.clips {
            let clip_ids: Vec<String> = clips.iter().filter_map(|c| c.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())).collect();
            let placeholders = if clip_ids.is_empty() { "''".to_string() } else { clip_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",") };
            let del_query = format!("DELETE FROM clips WHERE project_id = ? AND id NOT IN ({placeholders})");
            let mut del_params: Vec<&dyn rusqlite::ToSql> = vec![&p_id];
            for cid in &clip_ids {
                del_params.push(cid);
            }
            conn.execute(&del_query, rusqlite::params_from_iter(del_params)).map_err(|e| e.to_string())?;

            for c in clips {
                if let Some(cid) = c.get("id").and_then(|v| v.as_str()) {
                    let char_ids_str = serde_json::to_string(&c.get("characterIds").cloned().unwrap_or_else(|| json!([]))).unwrap_or_default();
                    conn.execute(
                        "INSERT INTO clips (id, project_id, sort, title, story, visual_prompt, narration, duration_ms, character_ids, motion_type,
                             camera_preset, motion_strength, video_prompt, transition)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT(id) DO UPDATE SET sort = excluded.sort, title = excluded.title, story = excluded.story, visual_prompt = excluded.visual_prompt,
                             narration = excluded.narration, duration_ms = excluded.duration_ms, character_ids = excluded.character_ids,
                             motion_type = excluded.motion_type, camera_preset = excluded.camera_preset, motion_strength = excluded.motion_strength,
                             video_prompt = excluded.video_prompt, transition = excluded.transition",
                        params![
                            cid,
                            p_id,
                            c.get("sort").and_then(|v| v.as_i64()).unwrap_or(0),
                            c.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("story").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("visualPrompt").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("narration").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("durationMs").and_then(|v| v.as_i64()).unwrap_or(5000),
                            char_ids_str,
                            c.get("motionType").and_then(|v| v.as_str()).unwrap_or("camera"),
                            c.get("cameraPreset").and_then(|v| v.as_str()).unwrap_or("zoomin"),
                            c.get("motionStrength").and_then(|v| v.as_str()).unwrap_or("halus"),
                            c.get("videoPrompt").and_then(|v| v.as_str()).unwrap_or(""),
                            c.get("transition").and_then(|v| v.as_str()).unwrap_or("fade")
                        ]
                    ).map_err(|e| e.to_string())?;
                }
            }
        }

        Ok(())
    })();

    match res {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            Ok(now)
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(err)
        }
    }
}

pub fn remove_project(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM projects WHERE id = ?", params![id]).map_err(|e| e.to_string())?;
    let dir = projects_root().join(id);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

pub fn duplicate_project(conn: &Connection, id: &str) -> Result<String, String> {
    let src = get_bundle(conn, id).ok_or("Proyek sumber tidak ditemukan")?;
    let new_id = Uuid::new_v4().to_string();
    let now = chrono_now();

    let mut id_map = HashMap::<String, String>::new();
    let mut map_id = |old: Option<&str>| -> Option<String> {
        old.map(|s| id_map.entry(s.to_string()).or_insert_with(|| Uuid::new_v4().to_string()).clone())
    };

    let p = &src.project;
    let cover = map_id(p.cover_asset_id.as_deref());

    conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;

    let res: Result<(), String> = (|| {
        let title = format!("{} (salinan)", p.title);

        conn.execute(
            "INSERT INTO projects (id, title, synopsis, duration_sec, language, aspect_ratio, style_id, tts_provider, tts_voice,
                 image_model, video_model, video_resolution, status, step, cover_asset_id, editor_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                new_id,
                title,
                p.synopsis,
                p.duration_sec,
                p.language,
                p.aspect_ratio,
                p.style_id,
                p.tts_provider,
                p.tts_voice,
                p.image_model,
                p.video_model,
                p.video_resolution,
                "draft",
                1,
                cover,
                serde_json::to_string(&p.editor).unwrap_or_default(),
                now,
                now
            ]
        ).map_err(|e| e.to_string())?;

        for a in &src.assets {
            let aid = map_id(Some(&a.id)).unwrap_or_else(|| Uuid::new_v4().to_string());
            let cid = map_id(a.clip_id.as_deref());
            let chid = map_id(a.character_id.as_deref());
            conn.execute(
                "INSERT INTO assets (id, project_id, clip_id, character_id, kind, provider, model, prompt, remote_id, remote_url, local_path,
                     duration_ms, width, height, meta_json, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    aid,
                    new_id,
                    cid,
                    chid,
                    a.kind,
                    a.provider,
                    a.model,
                    a.prompt,
                    a.remote_id,
                    a.remote_url,
                    a.local_path,
                    a.duration_ms,
                    a.width,
                    a.height,
                    serde_json::to_string(&a.meta).unwrap_or_default(),
                    a.created_at
                ]
            ).map_err(|e| e.to_string())?;
        }

        for c in &src.characters {
            let cid = map_id(Some(&c.id)).unwrap_or_else(|| Uuid::new_v4().to_string());
            let sid = map_id(c.sheet_asset_id.as_deref());
            conn.execute(
                "INSERT INTO characters (id, project_id, name, description, sheet_asset_id, sort) VALUES (?, ?, ?, ?, ?, ?)",
                params![cid, new_id, c.name, c.description, sid, c.sort]
            ).map_err(|e| e.to_string())?;
        }

        for c in &src.clips {
            let cid = map_id(Some(&c.id)).unwrap_or_else(|| Uuid::new_v4().to_string());
            let iid = map_id(c.image_asset_id.as_deref());
            let vid = map_id(c.video_asset_id.as_deref());
            let aid = map_id(c.audio_asset_id.as_deref());
            let char_ids_mapped: Vec<Option<String>> = c.character_ids.iter().map(|ch| map_id(Some(ch))).collect();
            let char_ids_str = serde_json::to_string(&char_ids_mapped).unwrap_or_default();

            conn.execute(
                "INSERT INTO clips (id, project_id, sort, title, story, visual_prompt, visual_source, narration, duration_ms, character_ids,
                     motion_type, camera_preset, motion_strength, video_prompt, transition, image_asset_id, video_asset_id, audio_asset_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    cid,
                    new_id,
                    c.sort,
                    c.title,
                    c.story,
                    c.visual_prompt,
                    c.visual_source,
                    c.narration,
                    c.duration_ms,
                    char_ids_str,
                    c.motion_type,
                    c.camera_preset,
                    c.motion_strength,
                    c.video_prompt,
                    c.transition,
                    iid,
                    vid,
                    aid
                ]
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    })();

    match res {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            let src_dir = projects_root().join(id);
            let dst_dir = project_dir(&new_id);
            if src_dir.exists() {
                copy_dir_all(&src_dir, &dst_dir);
            }
            Ok(new_id)
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(err)
        }
    }
}

// ==========================================
// Characters & Clips
// ==========================================

pub fn characters_of(conn: &Connection, project_id: &str) -> Vec<CharacterRow> {
    let mut stmt = match conn.prepare("SELECT id, project_id, name, description, sheet_asset_id, sort FROM characters WHERE project_id = ? ORDER BY sort") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(CharacterRow {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            sheet_asset_id: row.get(4)?,
            sort: row.get(5)?,
        })
    });
    match rows {
        Ok(mapped) => mapped.flatten().collect(),
        Err(_) => Vec::new(),
    }
}

pub fn get_character(conn: &Connection, id: &str) -> Option<CharacterRow> {
    conn.query_row(
        "SELECT id, project_id, name, description, sheet_asset_id, sort FROM characters WHERE id = ?",
        params![id],
        |row| {
            Ok(CharacterRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                sheet_asset_id: row.get(4)?,
                sort: row.get(5)?,
            })
        },
    ).ok()
}

pub fn set_character_sheet(conn: &Connection, character_id: &str, sheet_asset_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE characters SET sheet_asset_id = ? WHERE id = ?",
        params![sheet_asset_id, character_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn clips_of(conn: &Connection, project_id: &str) -> Vec<ClipRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, project_id, sort, title, story, visual_prompt, visual_source, narration, duration_ms, character_ids, motion_type, camera_preset, motion_strength, video_prompt, transition, image_asset_id, video_asset_id, audio_asset_id FROM clips WHERE project_id = ? ORDER BY sort"
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![project_id], |row| {
        let char_ids_str: String = row.get(9)?;
        let char_ids: Vec<String> = serde_json::from_str(&char_ids_str).unwrap_or_default();
        Ok(ClipRow {
            id: row.get(0)?,
            project_id: row.get(1)?,
            sort: row.get(2)?,
            title: row.get(3)?,
            story: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            visual_prompt: row.get(5)?,
            visual_source: row.get(6)?,
            narration: row.get(7)?,
            duration_ms: row.get(8)?,
            character_ids: char_ids,
            motion_type: row.get(10)?,
            camera_preset: row.get(11)?,
            motion_strength: row.get(12)?,
            video_prompt: row.get(13)?,
            transition: row.get(14)?,
            image_asset_id: row.get(15)?,
            video_asset_id: row.get(16)?,
            audio_asset_id: row.get(17)?,
        })
    });
    match rows {
        Ok(mapped) => mapped.flatten().collect(),
        Err(_) => Vec::new(),
    }
}

pub fn get_clip(conn: &Connection, id: &str) -> Option<ClipRow> {
    conn.query_row(
        "SELECT id, project_id, sort, title, story, visual_prompt, visual_source, narration, duration_ms, character_ids, motion_type, camera_preset, motion_strength, video_prompt, transition, image_asset_id, video_asset_id, audio_asset_id FROM clips WHERE id = ?",
        params![id],
        |row| {
            let char_ids_str: String = row.get(9)?;
            let char_ids: Vec<String> = serde_json::from_str(&char_ids_str).unwrap_or_default();
            Ok(ClipRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                sort: row.get(2)?,
                title: row.get(3)?,
                story: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                visual_prompt: row.get(5)?,
                visual_source: row.get(6)?,
                narration: row.get(7)?,
                duration_ms: row.get(8)?,
                character_ids: char_ids,
                motion_type: row.get(10)?,
                camera_preset: row.get(11)?,
                motion_strength: row.get(12)?,
                video_prompt: row.get(13)?,
                transition: row.get(14)?,
                image_asset_id: row.get(15)?,
                video_asset_id: row.get(16)?,
                audio_asset_id: row.get(17)?,
            })
        },
    ).ok()
}

pub fn patch_clip(conn: &Connection, id: &str, patch: Value) -> Result<(), String> {
    let obj = patch.as_object().ok_or("Patch must be an object")?;
    let mut sets = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    for (k, v) in obj {
        match k.as_str() {
            "imageAssetId" => {
                sets.push("image_asset_id = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "videoAssetId" => {
                sets.push("video_asset_id = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "audioAssetId" => {
                sets.push("audio_asset_id = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "motionType" => {
                sets.push("motion_type = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "cameraPreset" => {
                sets.push("camera_preset = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "motionStrength" => {
                sets.push("motion_strength = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "videoPrompt" => {
                sets.push("video_prompt = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "transition" => {
                sets.push("transition = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "durationMs" => {
                sets.push("duration_ms = ?");
                vals.push(Box::new(v.as_i64()));
            }
            "sort" => {
                sets.push("sort = ?");
                vals.push(Box::new(v.as_i64()));
            }
            "title" => {
                sets.push("title = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "story" => {
                sets.push("story = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "visualPrompt" => {
                sets.push("visual_prompt = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "visualSource" => {
                sets.push("visual_source = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "narration" => {
                sets.push("narration = ?");
                vals.push(Box::new(v.as_str().map(|s| s.to_string())));
            }
            "characterIds" => {
                sets.push("character_ids = ?");
                vals.push(Box::new(serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())));
            }
            _ => {}
        }
    }

    if sets.is_empty() {
        return Ok(());
    }

    let sql = format!("UPDATE clips SET {} WHERE id = ?", sets.join(", "));
    vals.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, rusqlite::params_from_iter(params_ref)).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn replace_script(
    conn: &Connection,
    project_id: &str,
    final_title: &str,
    new_scenes: Vec<NewScriptScene>,
) -> Result<(), String> {
    let now = chrono_now();

    conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;

    let res: Result<(), String> = (|| {
        conn.execute(
            "UPDATE projects SET title = ?, updated_at = ? WHERE id = ?",
            params![final_title, now, project_id],
        ).map_err(|e| e.to_string())?;

        conn.execute("DELETE FROM clips WHERE project_id = ?", params![project_id])
            .map_err(|e| e.to_string())?;

        for (i, sc) in new_scenes.into_iter().enumerate() {
            let cid = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO clips (id, project_id, sort, title, story, visual_prompt, narration, duration_ms, character_ids, motion_type,
                     camera_preset, motion_strength, video_prompt, transition)
                 VALUES (?, ?, ?, ?, ?, '', ?, ?, '[]', 'camera', 'zoomin', 'halus', '', 'fade')",
                params![cid, project_id, i as i64, sc.title, sc.story, sc.narration, sc.duration_ms]
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    })();

    match res {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(err)
        }
    }
}

pub fn apply_visuals(
    conn: &Connection,
    project_id: &str,
    new_chars: Vec<(String, String)>,
    updates: Vec<SceneVisualUpdate>,
) -> Result<(), String> {
    conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;

    let res: Result<(), String> = (|| {
        let mut existing_chars = characters_of(conn, project_id);
        let mut next_sort = existing_chars.iter().map(|c| c.sort).max().unwrap_or(0) + 1;

        for (name, desc) in new_chars {
            let exists = existing_chars.iter().any(|c| c.name.eq_ignore_ascii_case(&name));
            if !exists {
                let cid = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO characters (id, project_id, name, description, sort) VALUES (?, ?, ?, ?, ?)",
                    params![cid, project_id, name, desc, next_sort],
                ).map_err(|e| e.to_string())?;
                next_sort += 1;
            }
        }

        existing_chars = characters_of(conn, project_id);

        for u in updates {
            let matched_ids: Vec<String> = u.character_names.iter().filter_map(|cname| {
                existing_chars.iter().find(|ec| ec.name.eq_ignore_ascii_case(cname)).map(|ec| ec.id.clone())
            }).collect();
            let char_ids_str = serde_json::to_string(&matched_ids).unwrap_or_else(|_| "[]".to_string());

            conn.execute(
                "UPDATE clips SET visual_prompt = ?, character_ids = ?, camera_preset = ?, motion_strength = ?, video_prompt = ?, transition = ? WHERE id = ?",
                params![
                    u.visual_prompt,
                    char_ids_str,
                    u.camera_preset,
                    u.motion_strength,
                    u.video_prompt,
                    u.transition,
                    u.clip_id
                ],
            ).map_err(|e| e.to_string())?;
        }

        let now = chrono_now();
        conn.execute("UPDATE projects SET updated_at = ? WHERE id = ?", params![now, project_id])
            .map_err(|e| e.to_string())?;

        Ok(())
    })();

    match res {
        Ok(_) => {
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(err)
        }
    }
}

// ==========================================
// Assets
// ==========================================

pub fn insert_asset(conn: &Connection, a: NewAsset) -> Result<AssetRow, String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono_now();

    conn.execute(
        "INSERT INTO assets (id, project_id, clip_id, character_id, kind, provider, model, prompt, remote_id, remote_url, local_path,
             duration_ms, width, height, meta_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            a.project_id,
            a.clip_id,
            a.character_id,
            a.kind,
            a.provider,
            a.model,
            a.prompt,
            a.remote_id,
            a.remote_url,
            a.local_path,
            a.duration_ms,
            a.width,
            a.height,
            serde_json::to_string(&a.meta).unwrap_or_default(),
            now
        ]
    ).map_err(|e| e.to_string())?;

    get_asset(conn, &id).ok_or_else(|| "Asset not found after insert".to_string())
}

pub fn get_asset(conn: &Connection, id: &str) -> Option<AssetRow> {
    conn.query_row("SELECT * FROM assets WHERE id = ?", params![id], |row| {
        let p_id: String = row.get("project_id")?;
        let local_path: String = row.get("local_path")?;
        let meta_str: String = row.get("meta_json")?;
        let meta: Value = serde_json::from_str(&meta_str).unwrap_or_else(|_| json!({}));

        Ok(AssetRow {
            id: row.get::<_, String>("id")?,
            project_id: p_id.clone(),
            clip_id: row.get::<_, Option<String>>("clip_id")?,
            character_id: row.get::<_, Option<String>>("character_id")?,
            kind: row.get::<_, String>("kind")?,
            provider: row.get::<_, Option<String>>("provider")?,
            model: row.get::<_, Option<String>>("model")?,
            prompt: row.get::<_, Option<String>>("prompt")?,
            remote_id: row.get::<_, Option<String>>("remote_id")?,
            remote_url: row.get::<_, Option<String>>("remote_url")?,
            local_path: local_path.clone(),
            url: asset_url(&p_id, &local_path),
            duration_ms: row.get::<_, Option<i64>>("duration_ms")?,
            width: row.get::<_, Option<i64>>("width")?,
            height: row.get::<_, Option<i64>>("height")?,
            meta,
            created_at: row.get::<_, i64>("created_at")?,
        })
    }).ok()
}

pub fn update_asset_meta(conn: &Connection, id: &str, meta: Value) -> Result<(), String> {
    let s = serde_json::to_string(&meta).unwrap_or_default();
    conn.execute("UPDATE assets SET meta_json = ? WHERE id = ?", params![s, id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ==========================================
// Jobs
// ==========================================

pub fn insert_job(
    conn: &Connection,
    project_id: &str,
    kind: &str,
    provider: &str,
    payload: Value,
    clip_id: Option<&str>,
    character_id: Option<&str>,
) -> Result<Value, String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono_now();

    conn.execute(
        "INSERT INTO jobs (id, project_id, clip_id, character_id, kind, provider, status, progress, payload_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 'queued', 0, ?, ?, ?)",
        params![
            id,
            project_id,
            clip_id,
            character_id,
            kind,
            provider,
            serde_json::to_string(&payload).unwrap_or_default(),
            now,
            now
        ]
    ).map_err(|e| e.to_string())?;

    get_job(conn, &id).ok_or_else(|| "Job not found after insert".to_string())
}

pub fn get_job(conn: &Connection, id: &str) -> Option<Value> {
    conn.query_row("SELECT * FROM jobs WHERE id = ?", params![id], |row| {
        row_to_job(row)
    }).ok()
}

pub fn update_job(conn: &Connection, id: &str, patch: Value) -> Result<Value, String> {
    let now = chrono_now();
    let mut sets = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(st) = patch.get("status").and_then(|v| v.as_str()) {
        sets.push("status = ?");
        vals.push(Box::new(st.to_string()));
    }
    if let Some(p) = patch.get("progress").and_then(|v| v.as_f64()) {
        sets.push("progress = ?");
        vals.push(Box::new(p));
    }
    if let Some(m) = patch.get("message") {
        sets.push("message = ?");
        vals.push(Box::new(m.as_str().map(|s| s.to_string())));
    }
    if let Some(e) = patch.get("error") {
        sets.push("error = ?");
        vals.push(Box::new(e.as_str().map(|s| s.to_string())));
    }
    if let Some(r) = patch.get("remoteId").and_then(|v| v.as_str()) {
        sets.push("remote_id = ?");
        vals.push(Box::new(r.to_string()));
    }
    if let Some(ra) = patch.get("resultAssetId").and_then(|v| v.as_str()) {
        sets.push("result_asset_id = ?");
        vals.push(Box::new(ra.to_string()));
    }

    sets.push("updated_at = ?");
    vals.push(Box::new(now));

    let sql = format!("UPDATE jobs SET {} WHERE id = ?", sets.join(", "));
    vals.push(Box::new(id.to_string()));

    let params_ref: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, rusqlite::params_from_iter(params_ref)).map_err(|e| e.to_string())?;

    get_job(conn, id).ok_or_else(|| "Job not found".to_string())
}

pub fn active_job_for(
    conn: &Connection,
    clip_id: Option<&str>,
    character_id: Option<&str>,
    kind: &str,
) -> Option<Value> {
    let mut sql = "SELECT * FROM jobs WHERE kind = ? AND status IN ('queued', 'running')".to_string();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(kind.to_string())];

    if let Some(cid) = clip_id {
        sql.push_str(" AND clip_id = ?");
        params.push(Box::new(cid.to_string()));
    }
    if let Some(chid) = character_id {
        sql.push_str(" AND character_id = ?");
        params.push(Box::new(chid.to_string()));
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT 1");

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    conn.query_row(&sql, rusqlite::params_from_iter(params_ref), |row| {
        row_to_job(row)
    }).ok()
}

pub fn active_project_job(conn: &Connection, project_id: &str, kind: &str) -> Option<Value> {
    conn.query_row(
        "SELECT * FROM jobs WHERE project_id = ? AND kind = ? AND status IN ('queued', 'running') ORDER BY created_at DESC LIMIT 1",
        params![project_id, kind],
        |row| row_to_job(row)
    ).ok()
}

// ==========================================
// Helpers
// ==========================================

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, String>("id")?,
        "projectId": row.get::<_, String>("project_id")?,
        "clipId": row.get::<_, Option<String>>("clip_id")?,
        "characterId": row.get::<_, Option<String>>("character_id")?,
        "kind": row.get::<_, String>("kind")?,
        "provider": row.get::<_, String>("provider")?,
        "remoteId": row.get::<_, Option<String>>("remote_id")?,
        "status": row.get::<_, String>("status")?,
        "progress": row.get::<_, f64>("progress")?,
        "message": row.get::<_, Option<String>>("message")?,
        "error": row.get::<_, Option<String>>("error")?,
        "resultAssetId": row.get::<_, Option<String>>("result_asset_id")?,
        "createdAt": row.get::<_, i64>("created_at")?,
        "updatedAt": row.get::<_, i64>("updated_at")?
    }))
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) {
    let _ = fs::create_dir_all(dst);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let ty = entry.file_type();
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if let Ok(ft) = ty {
                if ft.is_dir() {
                    copy_dir_all(&src_path, &dst_path);
                } else {
                    let _ = fs::copy(&src_path, &dst_path);
                }
            }
        }
    }
}
