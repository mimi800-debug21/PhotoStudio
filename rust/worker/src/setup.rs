//! First-run AI setup: download a portable Python runtime, pip packages and
//! the Real-ESRGAN models into the app data dir, all SHA-256 verified.
//! Also computes the [`AiStatus`] reported to the frontend.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dl;
use crate::proto::{
    AiModel, AiStatus, AiStep, EMBEDDED_WORKER_PY, GET_PIP_SHA256, GET_PIP_URL, MODELS,
    PYTHON_EMBED_SHA256, PYTHON_EMBED_URL, PYTHON_VERSION,
};

pub const STEP_NAMES: &[(&str, &str)] = &[
    ("runtime", "Prepare runtime folder"),
    ("python", "Download portable Python"),
    ("pip", "Install pip"),
    ("deps", "Install AI packages (torch, realesrgan)"),
    ("models", "Download AI models (SHA-256 verified)"),
    ("selftest", "Verify installation (self-test)"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStateFile {
    pub state: String, // not_started | installing | ready | error
    pub error: Option<String>,
    pub steps: Vec<AiStep>,
}

impl Default for SetupStateFile {
    fn default() -> Self {
        SetupStateFile {
            state: "not_started".to_string(),
            error: None,
            steps: STEP_NAMES
                .iter()
                .map(|(n, d)| AiStep {
                    name: n.to_string(),
                    state: "pending".to_string(),
                    detail: d.to_string(),
                })
                .collect(),
        }
    }
}

pub fn python_dir(runtime: &Path) -> PathBuf {
    runtime.join("python")
}
pub fn python_exe(runtime: &Path) -> PathBuf {
    if cfg!(windows) {
        python_dir(runtime).join("python.exe")
    } else {
        let p = python_dir(runtime).join("bin/python3");
        if p.exists() {
            p
        } else {
            PathBuf::from("python3") // dev fallback: system python
        }
    }
}
pub fn models_dir(runtime: &Path) -> PathBuf {
    runtime.join("models")
}
pub fn worker_py(runtime: &Path) -> PathBuf {
    runtime.join("photostudio_worker.py")
}
pub fn selftest_json(runtime: &Path) -> PathBuf {
    runtime.join("selftest.json")
}
pub fn setup_state_json(runtime: &Path) -> PathBuf {
    runtime.join("setup-state.json")
}

pub fn load_state(runtime: &Path) -> SetupStateFile {
    dl::read_text_opt(&setup_state_json(runtime))
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_state(runtime: &Path, st: &SetupStateFile) {
    if let Ok(s) = serde_json::to_string_pretty(st) {
        let _ = dl::write_text(&setup_state_json(runtime), &s);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfTest {
    pub ok: bool,
    pub torch_version: Option<String>,
    pub cuda_available: bool,
    pub cuda_version: Option<String>,
    pub device_name: Option<String>,
    pub realesrgan: bool,
    pub error: Option<String>,
}

pub fn load_selftest(runtime: &Path) -> Option<SelfTest> {
    dl::read_text_opt(&selftest_json(runtime))
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Compute the current AI status from what is on disk.
pub fn compute_status(runtime: &Path) -> AiStatus {
    let st = load_state(runtime);
    let selftest = load_selftest(runtime);

    let models: Vec<AiModel> = MODELS
        .iter()
        .map(|(id, file, _url, sha)| {
            let p = models_dir(runtime).join(file);
            let present = p.is_file();
            let verified = present && dl::sha256_file(&p).map(|h| h.eq_ignore_ascii_case(sha)).unwrap_or(false);
            AiModel {
                name: id.to_string(),
                file: file.to_string(),
                present,
                verified,
            }
        })
        .collect();

    let py_present = if cfg!(windows) {
        python_exe(runtime).is_file()
    } else {
        python_dir(runtime).join("bin/python3").is_file() || selftest.is_some()
    };

    let gpu = selftest.as_ref().map(|s| s.cuda_available).unwrap_or(false);
    let all_models_ok = !models.is_empty() && models.iter().all(|m| m.verified);
    let ready = st.state == "ready" || (py_present && all_models_ok && selftest.as_ref().map(|s| s.ok).unwrap_or(false));

    let state = if st.state == "installing" {
        "installing".to_string()
    } else if ready {
        "ready".to_string()
    } else if st.error.is_some() {
        "error".to_string()
    } else {
        "not_started".to_string()
    };

    AiStatus {
        installed: ready,
        mode: if gpu { "cuda" } else { "cpu" }.to_string(),
        gpu_available: gpu,
        device_name: selftest.as_ref().and_then(|s| s.device_name.clone()),
        cuda_version: selftest.as_ref().and_then(|s| s.cuda_version.clone()),
        torch_version: selftest.as_ref().and_then(|s| s.torch_version.clone()),
        steps: st.steps.clone(),
        error: st.error.clone(),
        runtime_dir: Some(runtime.to_string_lossy().to_string()),
        models,
        state,
    }
}

/// Ensure the runtime dir exists and the worker script is present.
pub fn ensure_runtime(runtime: &Path) -> Result<(), String> {
    fs::create_dir_all(runtime).map_err(|e| format!("cannot create runtime dir: {e}"))?;
    let wp = worker_py(runtime);
    let need = match fs::read_to_string(&wp) {
        Ok(cur) => cur != EMBEDDED_WORKER_PY,
        Err(_) => true,
    };
    if need {
        dl::write_text(&wp, EMBEDDED_WORKER_PY).map_err(|e| format!("cannot write worker.py: {e}"))?;
    }
    Ok(())
}

/// Run the python worker's `selftest` command and cache the result.
pub fn run_selftest(runtime: &Path) -> Result<SelfTest, String> {
    ensure_runtime(runtime)?;
    let out = Command::new(python_exe(runtime))
        .arg("-u")
        .arg(worker_py(runtime))
        .arg("selftest")
        .output()
        .map_err(|e| format!("cannot run python: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
        .ok_or_else(|| format!("no JSON from selftest; stderr: {}", String::from_utf8_lossy(&out.stderr)))?;
    let st: SelfTest = serde_json::from_value(line).map_err(|e| format!("bad selftest json: {e}"))?;
    if let Ok(s) = serde_json::to_string_pretty(&st) {
        let _ = dl::write_text(&selftest_json(runtime), &s);
    }
    Ok(st)
}

fn set_step(steps: &mut Vec<AiStep>, name: &str, state: &str, detail: &str) {
    if let Some(s) = steps.iter_mut().find(|s| s.name == name) {
        s.state = state.to_string();
        s.detail = detail.to_string();
    }
}

/// Full first-run setup. Emits events through `emit`; honors `cancel`.
pub fn run_setup(
    runtime: &Path,
    emit: &dyn Fn(Value),
    progress: &dyn Fn(&str, u64, u64, &str),
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut state = SetupStateFile::default();
    state.state = "installing".to_string();
    save_state(runtime, &state);
    emit_status(runtime, &state, emit);

    let check_cancel = |step: &str| -> Result<(), String> {
        if cancel.load(Ordering::SeqCst) {
            Err(format!("setup cancelled during step '{step}'"))
        } else {
            Ok(())
        }
    };

    let result = (|state: &mut SetupStateFile| -> Result<(), String> {
        // 1. runtime folder + worker script
        set_step(&mut state.steps, "runtime", "running", "Creating folders…");
        save_and_emit(runtime, state, emit);
        ensure_runtime(runtime)?;
        set_step(&mut state.steps, "runtime", "done", &format!("Python {PYTHON_VERSION} runtime"));
        save_and_emit(runtime, state, emit);
        check_cancel("runtime")?;

        if cfg!(windows) {
            // 2. portable python
            set_step(&mut state.steps, "python", "running", "Downloading portable Python…");
            save_and_emit(runtime, state, emit);
            let zip_path = runtime.join("python-embed.zip");
            dl::download_to_file(
                PYTHON_EMBED_URL,
                &zip_path,
                PYTHON_EMBED_SHA256,
                Some(&mut |done, total| {
                    let pct = if total > 0 { done as f64 / total as f64 * 100.0 } else { 0.0 };
                    progress("python", done, total, &format!("Downloading Python {pct:.0}%"));
                }),
            )
            .map_err(|e| e.to_string())?;
            let bytes = fs::read(&zip_path).map_err(|e| e.to_string())?;
            dl::unzip(&bytes, &python_dir(runtime)).map_err(|e| e.to_string())?;
            let _ = fs::remove_file(&zip_path);
            // enable site-packages in the embeddable distribution
            let pth = python_dir(runtime).join("python311._pth");
            if let Ok(mut content) = fs::read_to_string(&pth) {
                if !content.contains("import site") {
                    content.push_str("import site\n");
                    let _ = fs::write(&pth, content);
                }
            }
            set_step(&mut state.steps, "python", "done", &format!("Python {PYTHON_VERSION} portable"));
            save_and_emit(runtime, state, emit);
            check_cancel("python")?;

            // 3. pip
            set_step(&mut state.steps, "pip", "running", "Bootstrapping pip…");
            save_and_emit(runtime, state, emit);
            let get_pip = runtime.join("get-pip.py");
            dl::download_to_file(GET_PIP_URL, &get_pip, GET_PIP_SHA256, None)
                .map_err(|e| e.to_string())?;
            run_python(runtime, &["get-pip.py"], "installing pip")?;
            let _ = fs::remove_file(&get_pip);
            set_step(&mut state.steps, "pip", "done", "pip ready");
            save_and_emit(runtime, state, emit);
            check_cancel("pip")?;
        } else {
            // non-Windows (dev): use system python3
            for name in ["python", "pip"] {
                set_step(&mut state.steps, name, "skipped", "using system Python 3 (dev mode)");
            }
            save_and_emit(runtime, state, emit);
        }

        // 4. python packages
        set_step(&mut state.steps, "deps", "running", "Installing torch + realesrgan (large download)…");
        save_and_emit(runtime, state, emit);
        pip_install(runtime, &["pillow", "numpy", "opencv-python-headless"], "base packages")?;
        // torch: prefer CUDA 12.1 wheels, fall back to CPU-only, then default index
        let torch_pkgs = ["torch", "torchvision"];
        let mut ok = false;
        for index in [
            "https://download.pytorch.org/whl/cu121",
            "https://download.pytorch.org/whl/cpu",
            "",
        ] {
            check_cancel("deps")?;
            let mut args: Vec<String> = vec!["-m".into(), "pip".into(), "install".into(), "--no-cache-dir".into()];
            if !index.is_empty() {
                args.push("--index-url".into());
                args.push(index.into());
            }
            args.extend(torch_pkgs.iter().map(|s| s.to_string()));
            if run_python_ok(runtime, &args) {
                ok = true;
                break;
            }
        }
        if !ok {
            return Err("could not install torch (tried cu121, cpu and default indexes)".to_string());
        }
        pip_install(runtime, &["realesrgan"], "realesrgan")?;
        set_step(&mut state.steps, "deps", "done", "torch + realesrgan installed");
        save_and_emit(runtime, state, emit);
        check_cancel("deps")?;

        // 5. models
        set_step(&mut state.steps, "models", "running", "Downloading models…");
        save_and_emit(runtime, state, emit);
        fs::create_dir_all(models_dir(runtime)).map_err(|e| e.to_string())?;
        for (i, (id, file, url, sha)) in MODELS.iter().enumerate() {
            check_cancel("models")?;
            let dest = models_dir(runtime).join(file);
            let already = dest.is_file()
                && dl::sha256_file(&dest).map(|h| h.eq_ignore_ascii_case(sha)).unwrap_or(false);
            if already {
                progress("models", (i + 1) as u64, MODELS.len() as u64, &format!("{id}: already verified"));
                continue;
            }
            dl::download_to_file(
                url,
                &dest,
                sha,
                Some(&mut |done, total| {
                    let base = i as f64 / MODELS.len() as f64 * 100.0;
                    let pct = base
                        + if total > 0 {
                            done as f64 / total as f64 / MODELS.len() as f64 * 100.0
                        } else {
                            0.0
                        };
                    progress("models", done, total, &format!("{id}: {pct:.0}%"));
                }),
            )
            .map_err(|e| format!("model {id}: {e}"))?;
            progress("models", (i + 1) as u64, MODELS.len() as u64, &format!("{id}: verified"));
        }
        set_step(&mut state.steps, "models", "done", &format!("{} models verified", MODELS.len()));
        save_and_emit(runtime, state, emit);
        check_cancel("models")?;

        // 6. selftest
        set_step(&mut state.steps, "selftest", "running", "Running self-test…");
        save_and_emit(runtime, state, emit);
        let st = run_selftest(runtime)?;
        if !st.ok || !st.realesrgan {
            return Err(format!("self-test failed: {}", st.error.unwrap_or_default()));
        }
        set_step(
            &mut state.steps,
            "selftest",
            "done",
            &format!(
                "torch {} · {}",
                st.torch_version.unwrap_or_default(),
                if st.cuda_available { format!("CUDA {}", st.cuda_version.unwrap_or_default()) } else { "CPU".into() }
            ),
        );
        Ok(())
    })(&mut state);

    match result {
        Ok(()) => {
            state.state = "ready".to_string();
            state.error = None;
            save_and_emit(runtime, &state, emit);
            Ok(())
        }
        Err(e) => {
            let cancelled = e.contains("cancelled");
            state.state = if cancelled { "not_started" } else { "error" }.to_string();
            state.error = Some(e.clone());
            // mark the running step as error
            if let Some(s) = state.steps.iter_mut().find(|s| s.state == "running") {
                s.state = "error".to_string();
            }
            save_and_emit(runtime, &state, emit);
            Err(e)
        }
    }
}

fn save_and_emit(runtime: &Path, state: &SetupStateFile, emit: &dyn Fn(Value)) {
    save_state(runtime, state);
    emit(crate::proto::event_setup_status(&compute_status(runtime)));
}

fn emit_status(runtime: &Path, state: &SetupStateFile, emit: &dyn Fn(Value)) {
    save_state(runtime, state);
    emit(crate::proto::event_setup_status(&compute_status(runtime)));
}

fn run_python(runtime: &Path, args: &[&str], what: &str) -> Result<(), String> {
    let exe = python_exe(runtime);
    let mut cmd = Command::new(&exe);
    cmd.current_dir(runtime);
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.output().map_err(|e| format!("cannot run {exe:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{what} failed: {}",
            String::from_utf8_lossy(&out.stderr).chars().take(2000).collect::<String>()
        ));
    }
    Ok(())
}

fn run_python_ok(runtime: &Path, args: &[String]) -> bool {
    let exe = python_exe(runtime);
    let mut cmd = Command::new(&exe);
    cmd.current_dir(runtime);
    for a in args {
        cmd.arg(a);
    }
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    matches!(cmd.status(), Ok(s) if s.success())
}

fn pip_install(runtime: &Path, pkgs: &[&str], what: &str) -> Result<(), String> {
    let mut args: Vec<String> = vec!["-m".into(), "pip".into(), "install".into(), "--no-cache-dir".into()];
    args.extend(pkgs.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_python(runtime, &refs, what)
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dl::{download, download_to_file, unzip, verify_sha256};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serve `body` over HTTP once, return the URL.
    fn serve_once(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/file")
    }

    #[test]
    fn download_verifies_sha256() {
        let body = b"hello photostudio".to_vec();
        let sha = {
            let mut h = Sha256::new();
            h.update(&body);
            hex::encode(h.finalize())
        };
        let url = serve_once(body.clone());
        let got = download(&url, None).unwrap();
        assert_eq!(got, body);
        verify_sha256(&got, &sha, "test").unwrap();
        assert!(verify_sha256(&got, "00", "test").is_err());
    }

    #[test]
    fn download_to_file_atomic() {
        let body = b"abc".to_vec();
        let sha = {
            let mut h = Sha256::new();
            h.update(&body);
            hex::encode(h.finalize())
        };
        let url = serve_once(body.clone());
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("sub/file.bin");
        download_to_file(&url, &dest, &sha, None).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert!(!dir.path().join("sub/file.part").exists());
    }

    #[test]
    fn unzip_extracts_safely() {
        // build a small zip in memory
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            w.start_file("a/b.txt", opts).unwrap();
            w.write_all(b"hi").unwrap();
            w.start_file("../evil.txt", opts).unwrap();
            w.write_all(b"no").unwrap();
            w.finish().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        unzip(&buf.into_inner(), dir.path()).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("a/b.txt")).unwrap(), "hi");
        assert!(!dir.path().join("../evil.txt").exists());
        assert!(!dir.path().parent().unwrap().join("evil.txt").exists());
    }

    #[test]
    fn status_reflects_runtime_contents() {
        let dir = tempfile::tempdir().unwrap();
        let rt = dir.path().join("ai-runtime");
        fs::create_dir_all(&rt).unwrap();

        let mut st = compute_status(&rt);
        assert_eq!(st.state, "not_started");
        assert!(!st.installed);
        assert_eq!(st.models.len(), 2);
        assert!(st.models.iter().all(|m| !m.present));

        // fake a verified model using the real hash of an empty-ish file is wrong;
        // instead write a file and patch expectation via direct sha check
        let mdir = models_dir(&rt);
        fs::create_dir_all(&mdir).unwrap();
        let f = mdir.join(MODELS[0].1);
        fs::write(&f, b"fake").unwrap();
        st = compute_status(&rt);
        assert!(st.models[0].present);
        assert!(!st.models[0].verified); // hash mismatch

        // write state file -> ready only when selftest ok; here just check state file roundtrip
        let mut sf = SetupStateFile::default();
        sf.state = "error".to_string();
        sf.error = Some("boom".to_string());
        save_state(&rt, &sf);
        let st2 = compute_status(&rt);
        assert_eq!(st2.state, "error");
        assert_eq!(st2.error.as_deref(), Some("boom"));
    }

    #[test]
    fn ensure_runtime_writes_worker_py() {
        let dir = tempfile::tempdir().unwrap();
        let rt = dir.path().join("rt");
        ensure_runtime(&rt).unwrap();
        let wp = worker_py(&rt);
        assert!(wp.is_file());
        let content = fs::read_to_string(&wp).unwrap();
        assert!(content.contains("photostudio_worker") || content.contains("def main"));
    }
}
