//! Enhancement jobs: spawn the Python worker (`photostudio_worker.py enhance`)
//! as a subprocess, stream its JSON-lines progress, and emit job events.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::proto::{event_job_done, event_job_error, event_progress, AiProgress, EnhanceResult, MODELS};
use crate::setup::{ensure_runtime, models_dir, python_exe, worker_py};

/// Handle to a running job (for cancellation).
pub struct JobHandle {
    pub child: Mutex<Option<Child>>,
    pub cancelled: std::sync::atomic::AtomicBool,
}

pub type Jobs = Arc<Mutex<HashMap<String, Arc<JobHandle>>>>;

pub fn new_jobs() -> Jobs {
    Arc::new(Mutex::new(HashMap::new()))
}

pub type Emit = Arc<dyn Fn(Value) + Send + Sync>;

/// Validate params and start an enhance job. Returns the job id.
pub fn start_job(runtime: &Path, jobs: &Jobs, params: &Value, emit: Emit) -> Result<String, String> {
    let input = params
        .get("input")
        .and_then(|v| v.as_str())
        .ok_or("missing 'input'")?
        .to_string();
    let out_path = params
        .get("out_path")
        .and_then(|v| v.as_str())
        .ok_or("missing 'out_path'")?
        .to_string();
    let model_id = params
        .get("model")
        .and_then(|v| v.as_str())
        .ok_or("missing 'model'")?
        .to_string();
    let model_def = MODELS
        .iter()
        .find(|(id, _, _, _)| *id == model_id)
        .ok_or_else(|| format!("unknown model '{model_id}'"))?;
    let scale = params.get("scale").and_then(|v| v.as_u64()).unwrap_or(4);
    let tile = params.get("tile").and_then(|v| v.as_i64()).unwrap_or(-1);
    let fp16 = params.get("fp16").and_then(|v| v.as_bool()).unwrap_or(false);

    if !Path::new(&input).is_file() {
        return Err(format!("input file not found: {input}"));
    }
    let model_file = models_dir(runtime).join(model_def.1);
    if !model_file.is_file() {
        return Err(format!("model file missing: {} (run first-run setup)", model_file.display()));
    }
    ensure_runtime(runtime)?;
    if let Some(parent) = Path::new(&out_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create output dir: {e}"))?;
    }

    let job_id = uuid::Uuid::new_v4().to_string();
    let handle = Arc::new(JobHandle {
        child: Mutex::new(None),
        cancelled: std::sync::atomic::AtomicBool::new(false),
    });
    jobs.lock().unwrap().insert(job_id.clone(), Arc::clone(&handle));

    let rt = runtime.to_path_buf();
    let emit2 = emit;
    let handle2 = Arc::clone(&handle);
    let jobs2 = Arc::clone(jobs);
    let job_id2 = job_id.clone();
    std::thread::spawn(move || {
        let result = run_job(&rt, &job_id2, &input, &out_path, &model_id, model_def.1, scale, tile, fp16, &handle2, &*emit2);
        if let Err(e) = result {
            emit2(event_job_error(&job_id2, &e));
        }
        jobs2.lock().unwrap().remove(&job_id2);
    });

    Ok(job_id)
}

#[allow(clippy::too_many_arguments)]
fn run_job(
    runtime: &Path,
    job_id: &str,
    input: &str,
    out_path: &str,
    model_id: &str,
    model_file_name: &str,
    scale: u64,
    tile: i64,
    fp16: bool,
    handle: &Arc<JobHandle>,
    emit: &dyn Fn(Value),
) -> Result<(), String> {
    let py = python_exe(runtime);
    let wp = worker_py(runtime);
    let mut cmd = Command::new(&py);
    cmd.arg("-u")
        .arg(&wp)
        .arg("enhance")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(out_path)
        .arg("--models-dir")
        .arg(models_dir(runtime))
        .arg("--model")
        .arg(model_id)
        .arg("--model-file")
        .arg(model_file_name)
        .arg("--scale")
        .arg(scale.to_string())
        .arg("--tile")
        .arg(tile.to_string())
        .arg("--fp16")
        .arg(if fp16 { "1" } else { "0" })
        .current_dir(runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("cannot spawn python worker: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    // drain stderr in the background so the child can never block on a full pipe
    let stderr_tail: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    if let Some(stderr) = child.stderr.take() {
        let tail = Arc::clone(&stderr_tail);
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut { stderr }, &mut s);
            let mut t = tail.lock().unwrap();
            *t = s.chars().rev().take(2000).collect::<String>().chars().rev().collect();
        });
    }
    // the handle owns the child so cancel_job can kill it; run_job takes it
    // back at the end to collect the exit status
    *handle.child.lock().unwrap() = Some(child);

    let mut result: Option<EnhanceResult> = None;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("progress") => {
                if let Some(mut p) = v.get("progress").cloned() {
                    p["job_id"] = Value::String(job_id.to_string());
                    if let Ok(p) = serde_json::from_value::<AiProgress>(p) {
                        emit(event_progress(&p));
                    }
                }
            }
            Some("result") => {
                if let Some(r) = v.get("result").cloned() {
                    let mut r = r;
                    r["job_id"] = Value::String(job_id.to_string());
                    let parsed: EnhanceResult = serde_json::from_value(r).map_err(|e| e.to_string())?;
                    result = Some(parsed);
                }
            }
            Some("error") => {
                let msg = v.get("error").and_then(|e| e.as_str()).unwrap_or("worker error").to_string();
                return Err(msg);
            }
            _ => {}
        }
    }

    if handle.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("cancelled".to_string());
    }
    // take the child back (None => cancel_job already killed+reaped it)
    let status = match handle.child.lock().unwrap().take() {
        Some(mut c) => c.wait().map_err(|e| e.to_string())?,
        None => return Err("cancelled".to_string()),
    };
    match result {
        Some(r) if status.success() => {
            emit(event_job_done(job_id, &r));
            Ok(())
        }
        Some(_) => Err(format!("worker exited with {status}: {}", stderr_tail.lock().unwrap())),
        None => Err(format!(
            "worker produced no result (exit {status}): {}",
            stderr_tail.lock().unwrap()
        )),
    }
}

/// Kill a running job (cancel).
pub fn cancel_job(jobs: &Jobs, job_id: &str) -> bool {
    let handle = jobs.lock().unwrap().get(job_id).cloned();
    if let Some(h) = handle {
        h.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(mut child) = h.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_job_validates_params() {
        let dir = tempfile::tempdir().unwrap();
        let rt = dir.path().join("rt");
        std::fs::create_dir_all(&rt).unwrap();
        let jobs = new_jobs();
        let emit: Emit = Arc::new(|_v: Value| {});
        // missing input
        let r = start_job(&rt, &jobs, &serde_json::json!({"model":"realesrgan_x4plus"}), emit.clone());
        assert!(r.unwrap_err().contains("input"));
        // unknown model
        let input = dir.path().join("in.jpg");
        std::fs::write(&input, b"x").unwrap();
        let r = start_job(
            &rt,
            &jobs,
            &serde_json::json!({"input": input.to_str().unwrap(), "model": "nope", "out_path": "/tmp/o.jpg"}),
            emit.clone(),
        );
        assert!(r.unwrap_err().contains("unknown model"));
        // missing model file (setup not run)
        let r = start_job(
            &rt,
            &jobs,
            &serde_json::json!({"input": input.to_str().unwrap(), "model": "realesrgan_x4plus", "out_path": "/tmp/o.jpg"}),
            emit,
        );
        assert!(r.unwrap_err().contains("model file missing"));
    }
}
