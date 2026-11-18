//! Client side of the sidecar protocol, used by the Tauri app: spawns the
//! `ai-worker` binary and exchanges JSON-lines with it over stdio.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::proto::{Request, Response};

pub struct SidecarClient {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>,
    events: Mutex<Option<Receiver<Value>>>,
}

impl SidecarClient {
    /// Spawn the sidecar binary with `--runtime <dir>`.
    pub fn spawn(exe: &Path, runtime_dir: &Path) -> std::io::Result<SidecarClient> {
        let mut child = Command::new(exe)
            .arg("--runtime")
            .arg(runtime_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");

        let pending: Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, events_rx) = channel::<Value>();

        let pending2 = Arc::clone(&pending);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break, // sidecar exited
                };
                let v: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                    let resp: Result<Value, String> = if v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
                        Ok(v.get("data").cloned().unwrap_or(Value::Null))
                    } else {
                        Err(v
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("unknown error")
                            .to_string())
                    };
                    let tx = pending2.lock().unwrap().remove(&id);
                    if let Some(tx) = tx {
                        let _ = tx.send(resp);
                    }
                } else if v.get("kind").is_some() {
                    let _ = events_tx.send(v);
                }
            }
            // wake all pending waiters so they don't hang forever
            let map = std::mem::take(&mut *pending2.lock().unwrap());
            for (_, tx) in map {
                let _ = tx.send(Err("ai-worker exited".to_string()));
            }
        });

        Ok(SidecarClient {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            next_id: AtomicU64::new(1),
            pending,
            events: Mutex::new(Some(events_rx)),
        })
    }

    /// Take the event receiver (events are forwarded to the frontend).
    pub fn take_events(&self) -> Option<Receiver<Value>> {
        self.events.lock().unwrap().take()
    }

    /// Send a command and block for its response (long timeout: selftest can
    /// take a while on first import of torch).
    pub fn call(&self, cmd: &str, params: Value) -> Result<Value, String> {
        self.call_timeout(cmd, params, Duration::from_secs(900))
    }

    pub fn call_timeout(&self, cmd: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        self.pending.lock().unwrap().insert(id, tx);
        let req = Request { id, cmd: cmd.to_string(), params };
        let line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        {
            let mut stdin = self.stdin.lock().map_err(|_| "stdin lock poisoned")?;
            stdin
                .write_all(line.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .and_then(|_| stdin.flush())
                .map_err(|e| format!("failed to write to ai-worker: {e}"))?;
        }
        match rx.recv_timeout(timeout) {
            Ok(res) => {
                self.pending.lock().unwrap().remove(&id);
                res
            }
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("timeout waiting for '{cmd}'"))
            }
        }
    }

    pub fn shutdown(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for SidecarClient {
    fn drop(&mut self) {
        let _ = self.child.get_mut().map(|c| c.kill());
    }
}

/// Keep the `Response` type referenced (used by the reader thread docs).
#[allow(dead_code)]
fn _assert_response_used(r: Response) -> Response {
    r
}
