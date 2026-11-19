//! Shared application state.

use std::path::PathBuf;
use std::sync::Mutex;

use photostudio_core::db::Db;
use photostudio_core::thumb::ThumbCache;
use tauri::AppHandle;

use crate::ai::AiManager;

pub struct AppState {
    pub handle: AppHandle,
    pub app_data: PathBuf,
    pub db: Mutex<Db>,
    pub thumb: ThumbCache,
    pub runtime_dir: PathBuf,
    pub ai: Mutex<Option<AiManager>>,
}

impl AppState {
    pub fn db_path(&self) -> PathBuf {
        self.app_data.join("photostudio.db")
    }
}
