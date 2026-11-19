//! Tauri commands — the backend API consumed by `src/lib/api.ts`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use photostudio_core::db::{Album, DuplicateGroup, ExportRecord, ImportedFolder, Photo, PhotoFilter, PhotoList};
use photostudio_core::edits::{self, EditParams};
use photostudio_core::paths;
use photostudio_core::scan::{self, ImportResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::ai;
use crate::state::AppState;

type S = Arc<AppState>;

// ---------------------------------------------------------------- info

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub app_data_dir: String,
    pub db_path: String,
    pub thumb_dir: String,
    pub runtime_dir: String,
    pub worker_available: bool,
}

#[tauri::command]
pub fn app_info(state: State<'_, S>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        app_data_dir: state.app_data.to_string_lossy().to_string(),
        db_path: state.db_path().to_string_lossy().to_string(),
        thumb_dir: state.thumb.dir.to_string_lossy().to_string(),
        runtime_dir: state.runtime_dir.to_string_lossy().to_string(),
        worker_available: ai::worker_available(),
    }
}

// ---------------------------------------------------------------- dialogs

fn fp_string(f: tauri_plugin_dialog::FilePath) -> Option<String> {
    match f {
        tauri_plugin_dialog::FilePath::Path(p) => Some(p.to_string_lossy().to_string()),
        _ => None,
    }
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, mut rx) = tauri::async_runtime::channel::<Option<tauri_plugin_dialog::FilePath>>(1);
    app.dialog().file().pick_folder(move |f| {
        let _ = tx.try_send(f);
    });
    let res = rx.recv().await.ok_or("dialog cancelled")?;
    Ok(res.and_then(fp_string))
}

#[tauri::command]
pub async fn pick_save_file(
    app: tauri::AppHandle,
    suggested_name: String,
    default_dir: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, mut rx) = tauri::async_runtime::channel::<Option<tauri_plugin_dialog::FilePath>>(1);
    let mut b = app.dialog().file();
    b = b.set_file_name(&suggested_name);
    if let Some(d) = &default_dir {
        if Path::new(d).is_dir() {
            b = b.set_directory(d);
        }
    }
    b.save_file(move |f| {
        let _ = tx.try_send(f);
    });
    let res = rx.recv().await.ok_or("dialog cancelled")?;
    Ok(res.and_then(fp_string))
}

// ---------------------------------------------------------------- import

#[tauri::command]
pub async fn import_folder(state: State<'_, S>, path: String, recursive: bool) -> Result<ImportResult, String> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
        scan::scan_folder(&db, Path::new(&path), recursive).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_imported_folders(state: State<'_, S>) -> Result<Vec<ImportedFolder>, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.list_imported_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_imported_folder(state: State<'_, S>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.remove_imported_folder(id).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rescan_folder(state: State<'_, S>, id: i64) -> Result<ImportResult, String> {
    let (path, recursive) = {
        let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
        let f = db.get_imported_folder(id).map_err(|e| e.to_string())?.ok_or("folder not found")?;
        (f.path, f.recursive)
    };
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
        scan::scan_folder(&db, Path::new(&path), recursive).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- photos

#[tauri::command]
pub fn list_photos(state: State<'_, S>, query: PhotoFilter) -> Result<PhotoList, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.list_photos(&query).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_photo(state: State<'_, S>, id: i64) -> Result<Photo, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.get_photo(id).map_err(|e| e.to_string())?.ok_or_else(|| "photo not found".to_string())
}

#[tauri::command]
pub fn set_favorite(state: State<'_, S>, id: i64, favorite: bool) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.set_favorite(id, favorite).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_duplicates(state: State<'_, S>) -> Result<Vec<DuplicateGroup>, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.find_duplicates().map_err(|e| e.to_string())
}

/// Resolve + validate the on-disk path of a photo id.
fn validated_photo_path(state: &AppState, id: i64) -> Result<PathBuf, String> {
    let (path, folders) = {
        let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
        let p = db.photo_path(id).map_err(|e| e.to_string())?.ok_or_else(|| "photo not found".to_string())?;
        let f = db.folder_paths().map_err(|e| e.to_string())?;
        (p, f)
    };
    paths::validate_photo_path(Path::new(&path), &folders, &state.app_data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_thumbnail(state: State<'_, S>, id: i64, size: u32) -> Result<Vec<u8>, String> {
    let (path, thumb_dir) = {
        let p = validated_photo_path(&state, id)?;
        (p, state.thumb.dir.clone())
    };
    tauri::async_runtime::spawn_blocking(move || {
        let cache = photostudio_core::thumb::ThumbCache { dir: thumb_dir };
        cache.get_or_make(id, &path, size).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_image_bytes(state: State<'_, S>, id: i64) -> Result<Vec<u8>, String> {
    let path = validated_photo_path(&state, id)?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- edits

#[tauri::command]
pub fn get_edits(state: State<'_, S>, id: i64) -> Result<Option<EditParams>, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    match db.get_edits(id).map_err(|e| e.to_string())? {
        None => Ok(None),
        Some(json) => Ok(Some(serde_json::from_str(&json).map_err(|e| format!("bad edits json: {e}"))?)),
    }
}

#[tauri::command]
pub fn save_edits(state: State<'_, S>, id: i64, params: EditParams) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    let json = serde_json::to_string(&params).map_err(|e| e.to_string())?;
    db.save_edits(id, &json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn render_preview(state: State<'_, S>, id: i64, params: EditParams, max_dim: u32) -> Result<Vec<u8>, String> {
    let path = validated_photo_path(&state, id)?;
    tauri::async_runtime::spawn_blocking(move || {
        edits::render(&path, &params, max_dim).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_photo(state: State<'_, S>, id: i64, params: EditParams, out_path: String) -> Result<ExportRecord, String> {
    let path = validated_photo_path(&state, id)?;
    let out = PathBuf::from(&out_path);
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        edits::export_file(&path, &params, &out).map(|_| ()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.add_export(id, &out_path, "edit").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_exports(state: State<'_, S>, id: i64) -> Result<Vec<ExportRecord>, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.list_exports(id).map_err(|e| e.to_string())
}

/// Read bytes of a previously exported file (must be a recorded export or
/// live under the app data dir).
#[tauri::command]
pub async fn read_export_bytes(state: State<'_, S>, path: String) -> Result<Vec<u8>, String> {
    let allowed = {
        let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
        let recorded = db
            .list_exports_all()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|e| e.out_path == path);
        recorded || paths::is_under_any(Path::new(&path), &[state.app_data.clone()])
    };
    if !allowed {
        return Err("path is not a recorded export".to_string());
    }
    let p = PathBuf::from(&path);
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::read(&p).map_err(|e| format!("cannot read {}: {e}", p.display()))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------- albums

#[tauri::command]
pub fn list_albums(state: State<'_, S>) -> Result<Vec<Album>, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.list_albums().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_album(state: State<'_, S>, name: String) -> Result<Album, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.create_album(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_album(state: State<'_, S>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.delete_album(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_to_album(state: State<'_, S>, album_id: i64, photo_id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.add_to_album(album_id, photo_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_from_album(state: State<'_, S>, album_id: i64, photo_id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.remove_from_album(album_id, photo_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn album_contains(state: State<'_, S>, album_id: i64, photo_id: i64) -> Result<bool, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.album_contains(album_id, photo_id).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- AI

#[tauri::command]
pub fn ai_status(state: State<'_, S>) -> ai_worker::proto::AiStatus {
    ai::status(&state)
}

#[tauri::command]
pub fn ai_start_setup(state: State<'_, S>) -> Result<(), String> {
    ai::call(&state, "setup", Value::Null).map(|_| ())
}

#[tauri::command]
pub fn ai_cancel_setup(state: State<'_, S>) -> Result<(), String> {
    ai::call(&state, "cancel_setup", Value::Null).map(|_| ())
}

#[tauri::command]
pub fn ai_self_test(state: State<'_, S>) -> Result<ai_worker::proto::AiStatus, String> {
    ai::call(&state, "selftest", Value::Null)?;
    // give the sidecar a moment to persist selftest.json, then read status
    std::thread::sleep(std::time::Duration::from_millis(300));
    let v = ai::call(&state, "status", Value::Null)?;
    serde_json::from_value(v).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEnhanceParams {
    photo_id: i64,
    model: String,
    scale: u64,
    tile: i64,
    fp16: bool,
    out_path: String,
}

#[tauri::command]
pub fn ai_enhance(state: State<'_, S>, params: AiEnhanceParams) -> Result<String, String> {
    let input = validated_photo_path(&state, params.photo_id)?;
    let v = ai::call(
        &state,
        "enhance",
        serde_json::json!({
            "input": input.to_string_lossy(),
            "out_path": params.out_path,
            "model": params.model,
            "scale": params.scale,
            "tile": params.tile,
            "fp16": params.fp16,
        }),
    )?;
    v.as_str().map(|s| s.to_string()).ok_or_else(|| "bad job id response".to_string())
}

#[tauri::command]
pub fn ai_cancel_job(state: State<'_, S>, job_id: String) -> Result<(), String> {
    ai::call(&state, "cancel_job", serde_json::json!({ "job_id": job_id })).map(|_| ())
}

#[tauri::command]
pub fn ai_open_runtime_dir(state: State<'_, S>) -> Result<(), String> {
    let dir = state.runtime_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(&dir);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(&dir);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&dir);
        c
    };
    cmd.spawn().map_err(|e| format!("cannot open folder: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------- settings

#[tauri::command]
pub fn get_setting(state: State<'_, S>, key: String) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(state: State<'_, S>, key: String, value: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_database(state: State<'_, S>) -> Result<String, String> {
    let ts = photostudio_core::db::now_string().replace([' ', ':'], "-");
    let dest = state.app_data.join("backups").join(format!("photostudio-{ts}.db"));
    let db = state.db.lock().map_err(|_| "db lock poisoned".to_string())?;
    db.backup_to(&dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().to_string())
}
