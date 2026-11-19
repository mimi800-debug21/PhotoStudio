//! PhotoStudio desktop app (Tauri v2).
//!
//! All heavy lifting lives in the `photostudio-core` library; the AI upscaling
//! runs in the bundled `ai-worker` sidecar (see `rust/worker`).

mod ai;
mod commands;
mod state;

use std::sync::{Arc, Mutex};

use photostudio_core::db::Db;
use photostudio_core::thumb::ThumbCache;
use tauri::Manager;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("cannot resolve app data dir: {e}"))?;
            std::fs::create_dir_all(&app_data)?;
            std::fs::create_dir_all(app_data.join("thumbs"))?;
            std::fs::create_dir_all(app_data.join("backups"))?;

            let db = Db::open(&app_data.join("photostudio.db")).map_err(|e| format!("cannot open database: {e}"))?;
            let thumb = ThumbCache::new(app_data.join("thumbs")).map_err(|e| format!("cannot create thumb cache: {e}"))?;
            let runtime_dir = app_data.join("ai-runtime");

            app.manage(Arc::new(AppState {
                handle: app.handle().clone(),
                app_data: app_data.clone(),
                db: Mutex::new(db),
                thumb,
                runtime_dir,
                ai: Mutex::new(None),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::pick_folder,
            commands::pick_save_file,
            commands::import_folder,
            commands::list_imported_folders,
            commands::remove_imported_folder,
            commands::rescan_folder,
            commands::list_photos,
            commands::get_photo,
            commands::set_favorite,
            commands::find_duplicates,
            commands::get_thumbnail,
            commands::get_image_bytes,
            commands::get_edits,
            commands::save_edits,
            commands::render_preview,
            commands::export_photo,
            commands::list_exports,
            commands::read_export_bytes,
            commands::list_albums,
            commands::create_album,
            commands::delete_album,
            commands::add_to_album,
            commands::remove_from_album,
            commands::album_contains,
            commands::ai_status,
            commands::ai_start_setup,
            commands::ai_cancel_setup,
            commands::ai_self_test,
            commands::ai_enhance,
            commands::ai_cancel_job,
            commands::ai_open_runtime_dir,
            commands::get_setting,
            commands::set_setting,
            commands::backup_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PhotoStudio");
}
