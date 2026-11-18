//! Wire protocol types shared between the Tauri app, this sidecar, and the
//! TypeScript frontend. Field names match `src/lib/types.ts` exactly.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStep {
    pub name: String,
    pub state: String, // pending | running | done | error | skipped
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModel {
    pub name: String,
    pub file: String,
    pub present: bool,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatus {
    pub installed: bool,
    pub state: String, // not_started | installing | ready | error
    pub mode: String,  // cuda | cpu | unknown
    pub gpu_available: bool,
    pub device_name: Option<String>,
    pub cuda_version: Option<String>,
    pub torch_version: Option<String>,
    pub steps: Vec<AiStep>,
    pub error: Option<String>,
    pub runtime_dir: Option<String>,
    pub models: Vec<AiModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProgress {
    pub job_id: String,
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhanceResult {
    pub job_id: String,
    pub input: String,
    pub output: String,
    pub scale: u64,
    pub model: String,
    pub device: String,
    pub elapsed_ms: u64,
}

/// One request line on stdin: {"id": n, "cmd": "...", "params": {...}}
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub cmd: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// One response line on stdout: {"id": n, "ok": bool, "data": ..., "error": ...}
#[derive(Debug, Serialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: u64, data: serde_json::Value) -> Self {
        Response { id, ok: true, data: Some(data), error: None }
    }
    pub fn err(id: u64, error: impl Into<String>) -> Self {
        Response { id, ok: false, data: None, error: Some(error.into()) }
    }
}

// ---------------------------------------------------------------- events
// Events are plain JSON lines with a "kind" field, forwarded verbatim to the
// frontend as the payload of the Tauri "ai-event":
//   {"kind":"setup-status","status":{...}}
//   {"kind":"progress","progress":{...}}
//   {"kind":"job-done","job_id":"...","result":{...}}
//   {"kind":"job-error","job_id":"...","error":"..."}

pub fn event_setup_status(status: &AiStatus) -> serde_json::Value {
    serde_json::json!({ "kind": "setup-status", "status": status })
}

pub fn event_progress(p: &AiProgress) -> serde_json::Value {
    serde_json::json!({ "kind": "progress", "progress": p })
}

pub fn event_job_done(job_id: &str, result: &EnhanceResult) -> serde_json::Value {
    serde_json::json!({ "kind": "job-done", "job_id": job_id, "result": result })
}

pub fn event_job_error(job_id: &str, error: &str) -> serde_json::Value {
    serde_json::json!({ "kind": "job-error", "job_id": job_id, "error": error })
}

// ---------------------------------------------------------------- models

/// (model id used by the frontend, file name, download URL, expected SHA-256)
pub const MODELS: &[(&str, &str, &str, &str)] = &[
    (
        "realesrgan_x4plus",
        "RealESRGAN_x4plus.pth",
        "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.1.0/RealESRGAN_x4plus.pth",
        "4fa0d38905f75ac06eb49a7951b426670021be3018265fd191d2125df9d682f1",
    ),
    (
        "realesrgan_x2plus",
        "RealESRGAN_x2plus.pth",
        "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.1/RealESRGAN_x2plus.pth",
        "49fafd45f8fd7aa8d31ab2a22d14d91b536c34494a5cfe31eb5d89c2fa266abb",
    ),
];

pub const PYTHON_EMBED_URL: &str =
    "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip";
pub const PYTHON_EMBED_SHA256: &str =
    "009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b";
pub const GET_PIP_URL: &str = "https://bootstrap.pypa.io/get-pip.py";
pub const GET_PIP_SHA256: &str =
    "356e0d0de21cdc348c3394084d90939b85e4246c4d14c76aee6ca4f46646ea98";

pub const PYTHON_VERSION: &str = "3.11.9";

/// The inference script, embedded into the sidecar and written into the
/// runtime directory during setup (single source of truth: ai-worker/).
pub const EMBEDDED_WORKER_PY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../ai-worker/photostudio_worker.py"
));
