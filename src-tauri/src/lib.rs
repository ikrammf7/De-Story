pub mod commands;
pub mod db;
pub mod diag;
pub mod flow2api_migration;
pub mod flow2api_runtime;
pub mod jobs;
pub mod media;
pub mod paths;
pub mod protocol;
pub mod secrets;
pub mod services;
pub mod speech;

use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    diag::install_panic_hook();
    diag::log_line(&format!("--- app start (v{}) ---", env!("CARGO_PKG_VERSION")));

    tauri::Builder::default()
        .on_window_event(|_window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed) {
                diag::log_line(&format!("window event: {event:?}"));
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .register_uri_scheme_protocol("studio", |ctx, request| {
            let res_dir = ctx.app_handle().path().resource_dir().ok();
            match protocol::handle_studio_request(res_dir.as_deref(), &request) {
                Ok(res) => res,
                Err(e) => tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(e.to_string().into_bytes())
                    .unwrap(),
            }
        })
        .setup(|app| {
            let db_pool = db::init_db().expect("Failed to initialize SQLite database");
            let job_manager = Arc::new(jobs::JobManager::new(db_pool.clone(), app.handle().clone()));
            let whisper_manager = Arc::new(speech::whisper::WhisperManager::new());
            let flow2api_manager = Arc::new(flow2api_runtime::Flow2ApiManager::new());
            // Apply the saved fixed port before the supervisor starts, so the
            // very first child already binds the address the extension expects.
            {
                let conn = db_pool.lock().unwrap();
                let saved = db::repo::get_settings(&conn)
                    .get("flow2apiFixedPort")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                flow2api_manager.set_fixed_port(saved.min(u16::MAX as u64) as u16);
            }
            app.manage(db_pool);
            app.manage(job_manager);
            app.manage(whisper_manager);
            app.manage(flow2api_manager.clone());
            flow2api_manager.start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // projects
            commands::projects_list,
            commands::projects_create,
            commands::projects_get,
            commands::projects_save,
            commands::projects_remove,
            commands::projects_duplicate,
            // story
            commands::story_script,
            commands::story_music_prompt,
            commands::story_visuals,
            // generate
            commands::generate_clip_image,
            commands::generate_clip_video,
            commands::generate_clip_tts,
            commands::generate_character_sheet,
            commands::generate_missing_images,
            commands::generate_missing_videos,
            commands::generate_missing_tts,
            commands::generate_narration,
            commands::generate_sync_captions,
            commands::generate_cancel,
            commands::generate_estimate,
            // whisper
            commands::whisper_status,
            commands::whisper_download,
            commands::whisper_cancel,
            commands::whisper_remove,
            commands::whisper_open_folder,
            // assets
            commands::assets_set_active,
            commands::assets_pick_music,
            commands::assets_pick_image,
            // settings
            commands::settings_get,
            commands::settings_set,
            commands::settings_keys,
            commands::settings_set_key,
            commands::settings_set_custom,
            commands::settings_clear_key,
            commands::settings_set_flow2api,
            commands::settings_set_gemini_keys,
            commands::settings_add_gemini_keys,
            commands::settings_test_gemini_keys,
            commands::models_list,
            commands::settings_test_key,
            commands::settings_reveal_key,
            commands::settings_voices,
            // exports
            commands::export_start,
            commands::export_default_folder,
            commands::export_pick_folder,
            commands::export_reveal,
            // flow2api runtime
            commands::flow2api_status,
            commands::flow2api_restart,
            commands::flow2api_log_folder,
            commands::flow2api_panel_info,
            commands::flow2api_extension_info,
            commands::flow2api_extension_folder,
            commands::flow2api_set_fixed_port,
            // app
            commands::app_copy_text,
            commands::app_open_external,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| match event {
            tauri::RunEvent::ExitRequested { code, .. } => {
                diag::log_line(&format!("exit requested: {code:?}"));
            }
            tauri::RunEvent::Exit => diag::log_line("--- app exit ---"),
            _ => {}
        });
}
