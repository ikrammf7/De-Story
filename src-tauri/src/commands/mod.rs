use std::fs;
use std::path::Path;
use std::sync::Arc;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::db::repo::{
    append_gemini_keys, create_project, drop_cache, duplicate_project, gemini_secret_snapshot,
    get_bundle, get_project, get_secret, get_settings, insert_asset, list_projects, patch_settings,
    record_check, record_gemini_check_if_unchanged, remove_project, save_snapshot, NewAsset,
    NewProjectInput, ProjectBundle, ProjectSnapshot,
};
use crate::db::DbPool;
use crate::flow2api_runtime::{Flow2ApiExtensionInfo, Flow2ApiManager, Flow2ApiPanelInfo, Flow2ApiStatus};
use crate::jobs::JobManager;
use crate::media::exporter::{start_export, ExportOptions};
use crate::paths::{asset_abs_path, user_data_dir, whisper_dir};
use crate::secrets::{
    clear_secret, key_statuses, normalized_gemini_keys, reveal_secret, set_secret,
    GeminiKeyTestResult, KeyStatus, KeyTestResult,
};
use crate::services::audio::{probe_duration_ms, probe_size};
use crate::services::{elevenlabs, gemini, generation, higgsfield as hf, llm, story};
use crate::speech::narration::{generate_narration as run_narration, sync_captions as run_sync_captions};
use crate::speech::whisper::{WhisperManager, WhisperStatus};

// ==========================================
// Projects
// ==========================================

#[tauri::command]
pub fn projects_list(db: State<'_, DbPool>) -> Result<Vec<Value>, String> {
    let conn = db.lock().unwrap();
    list_projects(&conn)
}

#[tauri::command]
pub fn projects_create(db: State<'_, DbPool>, input: NewProjectInput) -> Result<ProjectBundle, String> {
    let project_id = {
        let conn = db.lock().unwrap();
        create_project(&conn, input)?.id
    };
    let conn = db.lock().unwrap();
    get_bundle(&conn, &project_id).ok_or_else(|| "Gagal memuat bundle proyek baru".into())
}

#[tauri::command]
pub fn projects_get(db: State<'_, DbPool>, id: String) -> Result<ProjectBundle, String> {
    let conn = db.lock().unwrap();
    get_bundle(&conn, &id).ok_or_else(|| "Proyek tidak ditemukan".into())
}

#[tauri::command]
pub fn projects_save(db: State<'_, DbPool>, snap: ProjectSnapshot) -> Result<Value, String> {
    let conn = db.lock().unwrap();
    let updated_at = save_snapshot(&conn, snap)?;
    Ok(json!({ "updatedAt": updated_at }))
}

#[tauri::command]
pub fn projects_remove(db: State<'_, DbPool>, id: String) -> Result<(), String> {
    let conn = db.lock().unwrap();
    remove_project(&conn, &id)
}

#[tauri::command]
pub fn projects_duplicate(db: State<'_, DbPool>, id: String) -> Result<ProjectBundle, String> {
    let new_id = {
        let conn = db.lock().unwrap();
        duplicate_project(&conn, &id)?
    };
    let conn = db.lock().unwrap();
    get_bundle(&conn, &new_id).ok_or_else(|| "Gagal memuat proyek hasil duplikasi".into())
}

// ==========================================
// Story
// ==========================================

#[tauri::command]
pub async fn story_script(db: State<'_, DbPool>, project_id: String) -> Result<ProjectBundle, String> {
    story::generate_script(&db, &project_id).await
}

#[tauri::command]
pub async fn story_music_prompt(db: State<'_, DbPool>, project_id: String) -> Result<Value, String> {
    story::generate_music_prompt(&db, &project_id).await
}

#[tauri::command]
pub async fn story_visuals(
    db: State<'_, DbPool>,
    project_id: String,
    mode: String,
) -> Result<ProjectBundle, String> {
    story::generate_visuals(&db, &project_id, &mode).await
}

// ==========================================
// Generation
// ==========================================

#[tauri::command]
pub fn generate_clip_image(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    clip_id: String,
) -> Result<Value, String> {
    generation::generate_clip_image(db.inner().clone(), job_mgr.inner().clone(), app, clip_id)
}

#[tauri::command]
pub fn generate_clip_video(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    clip_id: String,
) -> Result<Value, String> {
    generation::generate_clip_video(db.inner().clone(), job_mgr.inner().clone(), app, clip_id)
}

#[tauri::command]
pub fn generate_clip_tts(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    clip_id: String,
) -> Result<Value, String> {
    generation::generate_clip_tts(db.inner().clone(), job_mgr.inner().clone(), app, clip_id)
}

#[tauri::command]
pub fn generate_character_sheet(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    character_id: String,
) -> Result<Value, String> {
    generation::generate_character_sheet(db.inner().clone(), job_mgr.inner().clone(), app, character_id)
}

#[tauri::command]
pub fn generate_missing_images(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    project_id: String,
) -> Result<Vec<Value>, String> {
    generation::generate_missing_images(db.inner().clone(), job_mgr.inner().clone(), app, project_id)
}

#[tauri::command]
pub fn generate_missing_videos(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    project_id: String,
) -> Result<Vec<Value>, String> {
    generation::generate_missing_videos(db.inner().clone(), job_mgr.inner().clone(), app, project_id)
}

#[tauri::command]
pub fn generate_missing_tts(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    project_id: String,
) -> Result<Vec<Value>, String> {
    generation::generate_missing_tts(db.inner().clone(), job_mgr.inner().clone(), app, project_id)
}

#[tauri::command]
pub fn generate_narration(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    project_id: String,
    mode: String,
) -> Result<Value, String> {
    let res_dir = app.path().resource_dir().ok();
    run_narration(db.inner().clone(), job_mgr.inner().clone(), app, project_id, mode, res_dir)
}

#[tauri::command]
pub fn generate_sync_captions(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    project_id: String,
) -> Result<Value, String> {
    let res_dir = app.path().resource_dir().ok();
    run_sync_captions(db.inner().clone(), job_mgr.inner().clone(), app, project_id, res_dir)
}

#[tauri::command]
pub async fn generate_cancel(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    job_id: String,
) -> Result<(), String> {
    let hf_key = {
        let conn = db.lock().unwrap();
        get_secret(&conn, "higgsfield")
    };
    job_mgr.cancel_job(&job_id, hf_key.as_deref()).await;
    Ok(())
}

#[tauri::command]
pub async fn generate_estimate(
    db: State<'_, DbPool>,
    kind: String,
    model_id: String,
    aspect: String,
    seconds: Option<f64>,
    resolution: Option<String>,
) -> Result<Option<Value>, String> {
    Ok(generation::estimate_credits(
        &db,
        &kind,
        &model_id,
        &aspect,
        seconds,
        resolution.as_deref(),
    )
    .await)
}

// ==========================================
// Whisper
// ==========================================

#[tauri::command]
pub fn whisper_status(whisper_mgr: State<'_, Arc<WhisperManager>>) -> Result<WhisperStatus, String> {
    Ok(whisper_mgr.status())
}

#[tauri::command]
pub async fn whisper_download(
    whisper_mgr: State<'_, Arc<WhisperManager>>,
    app: AppHandle,
) -> Result<(), String> {
    let mgr = whisper_mgr.inner().clone();
    tokio::spawn(async move {
        let _ = mgr.download_model(app).await;
    });
    Ok(())
}

#[tauri::command]
pub fn whisper_cancel(whisper_mgr: State<'_, Arc<WhisperManager>>) -> Result<(), String> {
    whisper_mgr.cancel_download();
    Ok(())
}

#[tauri::command]
pub fn whisper_remove(
    whisper_mgr: State<'_, Arc<WhisperManager>>,
    app: AppHandle,
) -> Result<(), String> {
    whisper_mgr.remove_model(&app);
    Ok(())
}

#[tauri::command]
pub fn whisper_open_folder() -> Result<(), String> {
    let d = whisper_dir();
    let _ = fs::create_dir_all(&d);
    open::that(&d).map_err(|e| e.to_string())
}

// ==========================================
// Flow2API runtime
// ==========================================

#[tauri::command]
pub fn flow2api_status(
    manager: State<'_, Arc<Flow2ApiManager>>,
) -> Result<Flow2ApiStatus, String> {
    Ok(manager.status())
}

#[tauri::command]
pub fn flow2api_restart(
    manager: State<'_, Arc<Flow2ApiManager>>,
    app: AppHandle,
) -> Result<Flow2ApiStatus, String> {
    manager.start(app);
    Ok(manager.status())
}

#[tauri::command]
pub fn flow2api_log_folder() -> Result<(), String> {
    let d = user_data_dir().join("flow2api");
    let _ = fs::create_dir_all(&d);
    open::that(&d).map_err(|e| e.to_string())
}

/// Address + credentials of Flow2API's own admin panel, so Settings can send
/// the user there instead of duplicating every CAPTCHA form.
///
/// The password is read from the data-dir file the backend generated. It stays
/// inside the app's own window: never logged, never put in a URL.
#[tauri::command]
pub fn flow2api_panel_info(
    manager: State<'_, Arc<Flow2ApiManager>>,
) -> Result<Flow2ApiPanelInfo, String> {
    manager.panel_info()
}

/// Connection details for the bundled Chrome extension CAPTCHA worker.
///
/// Available before the backend is ready when a fixed port is configured, so
/// the user can set the extension up once and keep it across restarts.
#[tauri::command]
pub fn flow2api_extension_info(
    app: AppHandle,
    manager: State<'_, Arc<Flow2ApiManager>>,
) -> Result<Flow2ApiExtensionInfo, String> {
    let ws_url = manager.captcha_ws_url()?;
    let bundle = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("flow2api").join("extension"))
        .filter(|dir| dir.is_dir());
    Ok(Flow2ApiExtensionInfo {
        ws_url,
        key: manager.extension_key(),
        extension_dir: bundle.map(|dir| dir.to_string_lossy().to_string()),
        fixed_port: manager.fixed_port() != 0,
    })
}

/// Open the bundled `extension/` folder so it can be loaded unpacked.
#[tauri::command]
pub fn flow2api_extension_folder(app: AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("Folder resource app tidak tersedia: {e}"))?
        .join("flow2api")
        .join("extension");
    if !dir.is_dir() {
        return Err("Folder ekstensi tidak ada di paket ini.".into());
    }
    open::that(&dir).map_err(|e| e.to_string())
}

/// Opt-in fixed loopback port for the embedded backend (0 = ephemeral default).
///
/// Only meaningful for the extension worker, which cannot follow a port that
/// changes on every launch. Applying it restarts the backend.
#[tauri::command]
pub fn flow2api_set_fixed_port(
    db: State<'_, DbPool>,
    manager: State<'_, Arc<Flow2ApiManager>>,
    app: AppHandle,
    port: u16,
) -> Result<Flow2ApiStatus, String> {
    {
        let conn = db.lock().unwrap();
        patch_settings(&conn, json!({ "flow2apiFixedPort": port }))?;
    }
    manager.set_fixed_port(port);
    manager.start(app);
    Ok(manager.status())
}

// ==========================================
// Assets
// ==========================================

#[tauri::command]
pub fn assets_set_active(
    db: State<'_, DbPool>,
    app: AppHandle,
    clip_id: String,
    asset_id: String,
) -> Result<(), String> {
    generation::set_active_asset(&db, &app, &clip_id, &asset_id)
}

#[tauri::command]
pub async fn assets_pick_music(
    db: State<'_, DbPool>,
    project_id: String,
) -> Result<Option<Value>, String> {
    {
        let conn = db.lock().unwrap();
        get_project(&conn, &project_id).ok_or("Proyek tidak ditemukan")?;
    }

    let file_opt = rfd::FileDialog::new()
        .set_title("Pilih musik latar")
        .add_filter("Audio", &["mp3", "wav", "m4a", "ogg"])
        .pick_file();

    let src = match file_opt {
        Some(p) => p,
        None => return Ok(None),
    };

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_lowercase()))
        .unwrap_or_else(|| ".mp3".to_string());

    let rel = format!("music/{}{}", Uuid::new_v4(), ext);
    let abs = asset_abs_path(&project_id, &rel);

    if let Some(parent) = abs.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::copy(&src, &abs).map_err(|e| e.to_string())?;

    let duration_ms = probe_duration_ms(&abs).await.ok().map(|d| d as i64);
    let prompt = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("musik")
        .to_string();

    let new_asset = NewAsset {
        project_id: project_id.clone(),
        clip_id: None,
        character_id: None,
        kind: "music".to_string(),
        provider: "local".to_string(),
        model: None,
        prompt: Some(prompt),
        remote_id: None,
        remote_url: None,
        local_path: rel,
        duration_ms,
        width: None,
        height: None,
        meta: json!({}),
    };

    let asset_row = {
        let conn = db.lock().unwrap();
        insert_asset(&conn, new_asset)?
    };

    Ok(Some(serde_json::to_value(asset_row).unwrap_or_default()))
}

#[tauri::command]
pub async fn assets_pick_image(
    db: State<'_, DbPool>,
    project_id: String,
) -> Result<Option<Value>, String> {
    {
        let conn = db.lock().unwrap();
        get_project(&conn, &project_id).ok_or("Proyek tidak ditemukan")?;
    }

    let file_opt = rfd::FileDialog::new()
        .set_title("Pilih gambar logo atau watermark")
        .add_filter("Gambar", &["png", "jpg", "jpeg", "webp"])
        .pick_file();

    let src = match file_opt {
        Some(p) => p,
        None => return Ok(None),
    };

    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_lowercase()))
        .unwrap_or_else(|| ".png".to_string());

    let rel = format!("overlays/{}{}", Uuid::new_v4(), ext);
    let abs = asset_abs_path(&project_id, &rel);

    if let Some(parent) = abs.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::copy(&src, &abs).map_err(|e| e.to_string())?;

    let (w, h) = probe_size(&abs)
        .await
        .map_err(|_| "Gambar tidak bisa dibaca. Pakai file PNG, JPG, atau WebP.".to_string())?;

    let prompt = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("overlay")
        .to_string();

    let new_asset = NewAsset {
        project_id: project_id.clone(),
        clip_id: None,
        character_id: None,
        kind: "overlay".to_string(),
        provider: "local".to_string(),
        model: None,
        prompt: Some(prompt),
        remote_id: None,
        remote_url: None,
        local_path: rel,
        duration_ms: None,
        width: Some(w as i64),
        height: Some(h as i64),
        meta: json!({}),
    };

    let asset_row = {
        let conn = db.lock().unwrap();
        insert_asset(&conn, new_asset)?
    };

    Ok(Some(serde_json::to_value(asset_row).unwrap_or_default()))
}

// ==========================================
// Settings
// ==========================================

#[tauri::command]
pub fn settings_get(db: State<'_, DbPool>) -> Result<Value, String> {
    let conn = db.lock().unwrap();
    Ok(get_settings(&conn))
}

#[tauri::command]
pub fn settings_set(db: State<'_, DbPool>, patch: Value) -> Result<Value, String> {
    let conn = db.lock().unwrap();
    patch_settings(&conn, patch)
}

#[tauri::command]
pub fn settings_keys(db: State<'_, DbPool>) -> Result<Vec<KeyStatus>, String> {
    let conn = db.lock().unwrap();
    key_statuses(&conn)
}

fn summarize_gemini_tests(results: &[GeminiKeyTestResult]) -> KeyTestResult {
    let working = results.iter().filter(|result| result.ok).count();
    KeyTestResult {
        ok: working > 0,
        message: Some(if results.is_empty() {
            "Kunci Gemini belum diatur".to_string()
        } else {
            format!("{working}/{} kunci Gemini terhubung", results.len())
        }),
    }
}

async fn test_gemini_pool_with<F, Fut>(keys: Vec<String>, mut test: F) -> Vec<GeminiKeyTestResult>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = KeyTestResult>,
{
    let mut results = Vec::with_capacity(keys.len());
    for (index, key) in keys.into_iter().enumerate() {
        let preview = crate::secrets::mask(&key);
        let result = test(key).await;
        results.push(GeminiKeyTestResult {
            index: index + 1,
            preview,
            ok: result.ok,
            message: result.message,
        });
    }
    results
}

async fn test_gemini_pool(keys: Vec<String>) -> Vec<GeminiKeyTestResult> {
    test_gemini_pool_with(keys, |key| async move { gemini::test_key(&key).await }).await
}

async fn check_gemini_snapshot(
    db: &DbPool,
    snapshot: Option<(Vec<u8>, Vec<String>)>,
) -> Result<Vec<GeminiKeyTestResult>, String> {
    let Some((blob, keys)) = snapshot else {
        return Ok(Vec::new());
    };
    let results = test_gemini_pool(keys).await;
    let summary = summarize_gemini_tests(&results);
    let conn = db.lock().unwrap();
    record_gemini_check_if_unchanged(&conn, &blob, summary.ok, summary.message.as_deref())?;
    Ok(results)
}

#[tauri::command]
pub async fn settings_set_gemini_keys(
    db: State<'_, DbPool>, keys: Vec<String>,
) -> Result<Vec<GeminiKeyTestResult>, String> {
    let snapshot = {
        let conn = db.lock().unwrap();
        set_secret(&conn, "gemini", &keys.join("\n"))?;
        gemini_secret_snapshot(&conn)?
    };
    check_gemini_snapshot(&db, snapshot).await
}

#[tauri::command]
pub async fn settings_add_gemini_keys(
    db: State<'_, DbPool>, keys: Vec<String>,
) -> Result<Vec<GeminiKeyTestResult>, String> {
    if normalized_gemini_keys(&keys.join("\n")).is_empty() {
        return Err("Kunci Gemini masih kosong".into());
    }
    let snapshot = {
        let conn = db.lock().unwrap();
        append_gemini_keys(&conn, &keys)?;
        gemini_secret_snapshot(&conn)?
    };
    check_gemini_snapshot(&db, snapshot).await
}

#[tauri::command]
pub async fn settings_test_gemini_keys(db: State<'_, DbPool>) -> Result<Vec<GeminiKeyTestResult>, String> {
    let snapshot = {
        let conn = db.lock().unwrap();
        gemini_secret_snapshot(&conn)?
    };
    check_gemini_snapshot(&db, snapshot).await
}

async fn test_provider(
    db: &DbPool,
    manager: Option<&Flow2ApiManager>,
    provider: &str,
    key: Option<&str>,
) -> KeyTestResult {
    let k = match key {
        Some(s) => s.to_string(),
        None => {
            let conn = db.lock().unwrap();
            get_secret(&conn, provider).unwrap_or_default()
        }
    };
    if provider == "higgsfield" {
        hf::test_credentials(&k).await
    } else if provider == "elevenlabs" {
        elevenlabs::test_key(&k).await
    } else if provider == "gemini" {
        let results = test_gemini_pool(normalized_gemini_keys(&k)).await;
        summarize_gemini_tests(&results)
    } else if provider == "flow2api" {
        let (mode, flow_base, saved_key) = {
            let conn = db.lock().unwrap();
            let s = get_settings(&conn);
            (
                s.get("flow2apiMode").and_then(|x| x.as_str()).unwrap_or("bundled").to_string(),
                s.get("flow2apiBaseUrl").and_then(|x| x.as_str()).unwrap_or("http://localhost:8000").to_string(),
                get_secret(&conn, "flow2api"),
            )
        };
        let external_key = if k.is_empty() { saved_key } else { Some(k.clone()) };
        match crate::flow2api_runtime::resolve_target(&mode, &flow_base, external_key.as_deref(), manager).await {
            Ok((base, credential)) => {
                crate::services::flow2api::test_connection(&base, credential.as_deref()).await
            }
            Err(error) => KeyTestResult { ok: false, message: Some(error) },
        }
    } else {
        let custom_base = {
            let conn = db.lock().unwrap();
            let s = get_settings(&conn);
            s.get("customBaseUrl").and_then(|x| x.as_str()).unwrap_or("").to_string()
        };
        llm::test_llm(provider, Some(&k), if provider == "custom" { Some(&custom_base) } else { None }).await
    }
}

#[tauri::command]
pub async fn settings_set_key(
    db: State<'_, DbPool>,
    manager: State<'_, Arc<Flow2ApiManager>>,
    provider: String,
    key: String,
) -> Result<KeyTestResult, String> {
    let clean = key.trim().to_string();
    if clean.is_empty() {
        return Err("Kunci masih kosong".into());
    }

    if provider == "gemini" {
        let snapshot = {
            let conn = db.lock().unwrap();
            set_secret(&conn, "gemini", &clean)?;
            gemini_secret_snapshot(&conn)?
        };
        let results = check_gemini_snapshot(&db, snapshot).await?;
        return Ok(summarize_gemini_tests(&results));
    }

    {
        let conn = db.lock().unwrap();
        set_secret(&conn, &provider, &clean)?;
        let _ = drop_cache(&conn, &format!("models:{provider}"));
    }

    if provider == "higgsfield" {
        generation::forget_estimates(&db);
    }

    let result = test_provider(&db, Some(&manager), &provider, Some(&clean)).await;
    {
        let conn = db.lock().unwrap();
        let _ = record_check(&conn, &provider, result.ok, result.message.as_deref());
    }

    Ok(result)
}

#[tauri::command]
pub async fn settings_set_custom(
    db: State<'_, DbPool>,
    base_url: String,
    key: String,
) -> Result<KeyTestResult, String> {
    let url = base_url.trim().trim_end_matches('/').to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Alamat endpoint harus diawali http:// atau https://".into());
    }

    {
        let conn = db.lock().unwrap();
        patch_settings(&conn, json!({ "customBaseUrl": url }))?;
        let clean_k = key.trim();
        if !clean_k.is_empty() {
            set_secret(&conn, "custom", clean_k)?;
        }
        let _ = drop_cache(&conn, "models:custom");
    }

    let result = test_provider(&db, None, "custom", if key.trim().is_empty() { None } else { Some(key.trim()) }).await;
    {
        let conn = db.lock().unwrap();
        let _ = record_check(&conn, "custom", result.ok, result.message.as_deref());
    }

    Ok(result)
}

#[tauri::command]
pub async fn settings_set_flow2api(
    db: State<'_, DbPool>,
    manager: State<'_, Arc<Flow2ApiManager>>,
    base_url: String,
    key: String,
    mode: Option<String>,
) -> Result<KeyTestResult, String> {
    let mode = match mode.as_deref() {
        Some("external") => "external".to_string(),
        Some("bundled") | None => "bundled".to_string(),
        Some(other) => return Err(format!("Mode Flow2API tidak dikenal: {other}")),
    };
    let mut url = base_url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        url = "http://localhost:8000".to_string();
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Alamat endpoint harus diawali http:// atau https://".into());
    }

    {
        let conn = db.lock().unwrap();
        patch_settings(&conn, json!({ "flow2apiBaseUrl": url, "flow2apiMode": mode }))?;
        let clean_k = key.trim();
        if !clean_k.is_empty() {
            set_secret(&conn, "flow2api", clean_k)?;
        }
    }

    let result = test_provider(&db, Some(&manager), "flow2api", if key.trim().is_empty() { None } else { Some(key.trim()) }).await;
    {
        let conn = db.lock().unwrap();
        let _ = record_check(&conn, "flow2api", result.ok, result.message.as_deref());
    }

    Ok(result)
}

#[tauri::command]
pub fn settings_clear_key(db: State<'_, DbPool>, provider: String) -> Result<(), String> {
    let conn = db.lock().unwrap();
    clear_secret(&conn, &provider)?;
    let _ = drop_cache(&conn, &format!("models:{provider}"));
    if provider == "higgsfield" {
        let _ = drop_cache(&conn, "estimate:");
    }
    if provider == "custom" {
        let _ = patch_settings(&conn, json!({ "customBaseUrl": "" }));
    }
    if provider == "flow2api" {
        let _ = patch_settings(&conn, json!({ "flow2apiBaseUrl": "http://localhost:8000" }));
    }
    Ok(())
}

#[tauri::command]
pub async fn models_list(
    db: State<'_, DbPool>,
    source: String,
    refresh: Option<bool>,
) -> Result<Value, String> {
    llm::list_models(&db, &source, refresh.unwrap_or(false)).await
}

#[tauri::command]
pub async fn settings_test_key(
    db: State<'_, DbPool>,
    manager: State<'_, Arc<Flow2ApiManager>>,
    provider: String,
) -> Result<KeyTestResult, String> {
    if provider == "gemini" {
        let snapshot = {
            let conn = db.lock().unwrap();
            gemini_secret_snapshot(&conn)?
        };
        let results = check_gemini_snapshot(&db, snapshot).await?;
        return Ok(summarize_gemini_tests(&results));
    }
    let result = test_provider(&db, Some(&manager), &provider, None).await;
    {
        let conn = db.lock().unwrap();
        let _ = record_check(&conn, &provider, result.ok, result.message.as_deref());
    }
    Ok(result)
}

#[cfg(test)]
mod gemini_pool_tests {
    use super::*;

    #[tokio::test]
    async fn tests_keys_sequentially_and_preserves_per_key_results() {
        let tested = Arc::new(std::sync::Mutex::new(Vec::new()));
        let results = test_gemini_pool_with(
            vec!["first-key-123456".into(), "second-key-7890".into()],
            |key| {
                let tested = tested.clone();
                async move {
                    tested.lock().unwrap().push(format!("start:{key}"));
                    tokio::task::yield_now().await;
                    tested.lock().unwrap().push(format!("end:{key}"));
                    KeyTestResult {
                        ok: key.starts_with("second"),
                        message: Some(if key.starts_with("second") { "ok" } else { "denied" }.into()),
                    }
                }
            },
        )
        .await;

        assert_eq!(*tested.lock().unwrap(), vec![
            "start:first-key-123456", "end:first-key-123456",
            "start:second-key-7890", "end:second-key-7890",
        ]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 1);
        assert_eq!(results[0].preview, crate::secrets::mask("first-key-123456"));
        assert!(!results[0].ok);
        assert_eq!(results[0].message.as_deref(), Some("denied"));
        assert_eq!(results[1].index, 2);
        assert!(results[1].ok);
        assert_eq!(results[1].message.as_deref(), Some("ok"));
        let summary = summarize_gemini_tests(&results);
        assert!(summary.ok);
        assert_eq!(summary.message.as_deref(), Some("1/2 kunci Gemini terhubung"));
    }

    #[test]
    fn no_success_in_pool_means_failed_aggregate() {
        let results = vec![GeminiKeyTestResult {
            index: 1,
            preview: "••••".into(),
            ok: false,
            message: Some("denied".into()),
        }];
        let summary = summarize_gemini_tests(&results);
        assert!(!summary.ok);
        assert_eq!(summary.message.as_deref(), Some("0/1 kunci Gemini terhubung"));
    }
}

#[tauri::command]
pub fn settings_reveal_key(db: State<'_, DbPool>, provider: String) -> Result<Option<String>, String> {
    let conn = db.lock().unwrap();
    Ok(reveal_secret(&conn, &provider))
}

#[tauri::command]
pub async fn settings_voices(db: State<'_, DbPool>, provider: String) -> Result<Vec<Value>, String> {
    if provider == "gemini" {
        Ok(gemini::list_voices())
    } else {
        let key = {
            let conn = db.lock().unwrap();
            get_secret(&conn, "elevenlabs").unwrap_or_default()
        };
        elevenlabs::list_voices(&key).await
    }
}

// ==========================================
// Export
// ==========================================

#[tauri::command]
pub fn export_start(
    db: State<'_, DbPool>,
    job_mgr: State<'_, Arc<JobManager>>,
    app: AppHandle,
    project_id: String,
    opts: ExportOptions,
) -> Result<Value, String> {
    let res_dir = app.path().resource_dir().ok();
    start_export(db.inner().clone(), job_mgr.inner().clone(), app, project_id, opts, res_dir)
}

#[tauri::command]
pub fn export_default_folder(db: State<'_, DbPool>) -> Result<String, String> {
    let conn = db.lock().unwrap();
    let s = get_settings(&conn);
    if let Some(folder) = s.get("exportFolder").and_then(|x| x.as_str()) {
        if !folder.is_empty() {
            return Ok(folder.to_string());
        }
    }

    let p = dirs::video_dir()
        .map(|v| v.join("De Story"))
        .unwrap_or_else(|| user_data_dir().join("exports"));

    Ok(p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn export_pick_folder(db: State<'_, DbPool>) -> Result<Option<String>, String> {
    let default = export_default_folder(db.clone())?;
    let folder_opt = rfd::FileDialog::new()
        .set_title("Pilih folder ekspor")
        .set_directory(&default)
        .pick_folder();

    match folder_opt {
        Some(p) => {
            let s = p.to_string_lossy().to_string();
            let conn = db.lock().unwrap();
            let _ = patch_settings(&conn, json!({ "exportFolder": s }));
            Ok(Some(s))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn export_reveal(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if let Some(parent) = p.parent() {
        open::that(parent).map_err(|e| e.to_string())
    } else {
        open::that(&path).map_err(|e| e.to_string())
    }
}

// ==========================================
// App Utilities
// ==========================================

#[tauri::command]
pub fn app_copy_text(text: String) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_open_external(url: String) -> Result<(), String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        open::that(&url).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
