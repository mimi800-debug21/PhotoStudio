//! PhotoStudio AI sidecar binary.
//!
//! Usage: `ai-worker --runtime <dir>`
//!
//! Speaks JSON-lines on stdin/stdout (see `ai_worker::proto`).

use std::path::PathBuf;

fn default_runtime_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|a| PathBuf::from(a).join("PhotoStudio").join("ai-runtime"))
            .unwrap_or_else(|_| PathBuf::from("ai-runtime"))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local/share/PhotoStudio/ai-runtime"))
            .unwrap_or_else(|_| PathBuf::from("ai-runtime"))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut runtime = default_runtime_dir();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--runtime" && i + 1 < args.len() {
            runtime = PathBuf::from(&args[i + 1]);
            i += 2;
        } else {
            i += 1;
        }
    }
    ai_worker::server::run(runtime);
}
