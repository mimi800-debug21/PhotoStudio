//! Bridge to the `ai-worker` sidecar process (local Real-ESRGAN runtime).

use std::path::PathBuf;

use ai_worker::client::SidecarClient;
use ai_worker::proto::AiStatus;
use serde_json::Value;
use tauri::Emitter;

use crate::state::AppState;

pub struct AiManager {
    pub client: SidecarClient,
}

/// Locate the sidecar binary: next to the app executable, or the
/// PHOTOSTUDIO_AI_WORKER override (used in dev/tests).
pub fn find_sidecar_exe() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("PHOTOSTUDIO_AI_WORKER") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("no exe dir")?;
    // Tauri bundles externalBin as `ai-worker-<target-triple>[.exe]`
    let candidates = [
        "ai-worker-x86_64-pc-windows-msvc.exe".to_string(),
        "ai-worker-x86_64-pc-windows-gnu.exe".to_string(),
        "ai-worker.exe".to_string(),
        "ai-worker".to_string(),
    ];
    for name in candidates {
        let p = dir.join(&name);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err("ai-worker sidecar binary not found next to the app executable".to_string())
}

pub fn worker_available() -> bool {
    find_sidecar_exe().is_ok()
}

/// Spawn the sidecar if needed and return a call handle.
pub fn ensure(state: &AppState) -> Result<(), String> {
    let mut guard = state.ai.lock().map_err(|_| "ai lock poisoned".to_string())?;
    if guard.is_some() {
        return Ok(());
    }
    let exe = find_sidecar_exe()?;
    let client = SidecarClient::spawn(&exe, &state.runtime_dir).map_err(|e| format!("cannot spawn ai-worker: {e}"))?;
    let events = client.take_events().ok_or("no event channel")?;
    let handle = state.handle.clone();
    std::thread::spawn(move || {
        while let Ok(ev) = events.recv() {
            let _ = handle.emit("ai-event", ev);
        }
    });
    *guard = Some(AiManager { client });
    Ok(())
}

/// Run a sidecar command; if the sidecar cannot be started, return a synthetic
/// "not installed" status (for `status`) or the error.
pub fn call(state: &AppState, cmd: &str, params: Value) -> Result<Value, String> {
    ensure(state)?;
    let guard = state.ai.lock().map_err(|_| "ai lock poisoned".to_string())?;
    let ai = guard.as_ref().ok_or("ai worker not running")?;
    ai.client.call(cmd, params)
}

/// Status even when the sidecar is missing.
pub fn status(state: &AppState) -> AiStatus {
    match call(state, "status", Value::Null) {
        Ok(v) => serde_json::from_value(v).unwrap_or_else(|_| fallback_status(state, "bad status payload")),
        Err(e) => fallback_status(state, &e),
    }
}

fn fallback_status(state: &AppState, error: &str) -> AiStatus {
    AiStatus {
        installed: false,
        state: "not_started".to_string(),
        mode: "unknown".to_string(),
        gpu_available: false,
        device_name: None,
        cuda_version: None,
        torch_version: None,
        steps: vec![],
        error: Some(error.to_string()),
        runtime_dir: Some(state.runtime_dir.to_string_lossy().to_string()),
        models: vec![],
    }
}
