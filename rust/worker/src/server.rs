//! Sidecar server: reads JSON-lines requests on stdin, writes responses and
//! events on stdout.

use std::io::{BufRead, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::enhance::{self, Jobs};
use crate::proto::{AiProgress, Request, Response};
use crate::setup::{self, SetupStateFile};

pub struct Server {
    pub runtime: PathBuf,
    out: Mutex<BufWriter<std::io::Stdout>>,
    setup_cancel: AtomicBool,
    setup_running: AtomicBool,
    jobs: Jobs,
}

impl Server {
    pub fn new(runtime: PathBuf) -> Server {
        Server {
            runtime,
            out: Mutex::new(BufWriter::new(std::io::stdout())),
            setup_cancel: AtomicBool::new(false),
            setup_running: AtomicBool::new(false),
            jobs: enhance::new_jobs(),
        }
    }

    /// Emit an event line (forwarded to the frontend as "ai-event").
    pub fn emit(&self, v: Value) {
        if let Ok(mut out) = self.out.lock() {
            let _ = writeln!(out, "{v}");
            let _ = out.flush();
        }
    }

    /// An owned, shareable event emitter for worker threads.
    pub fn emitter(self: &Arc<Server>) -> enhance::Emit {
        let s = Arc::clone(self);
        Arc::new(move |v: Value| s.emit(v))
    }

    fn respond(&self, r: Response) {
        if let Ok(mut out) = self.out.lock() {
            if let Ok(s) = serde_json::to_string(&r) {
                let _ = writeln!(out, "{s}");
                let _ = out.flush();
            }
        }
    }

    fn emit_progress(&self, phase: &str, current: u64, total: u64, message: &str) {
        let percent = if total > 0 {
            current as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        self.emit(crate::proto::event_progress(&AiProgress {
            job_id: "setup".to_string(),
            phase: phase.to_string(),
            current,
            total,
            percent,
            message: message.to_string(),
        }));
    }

    pub fn handle(self: &Arc<Server>, req: Request) {
        match req.cmd.as_str() {
            "ping" => self.respond(Response::ok(req.id, Value::String("pong".into()))),
            "status" => {
                let status = setup::compute_status(&self.runtime);
                self.respond(Response::ok(req.id, serde_json::to_value(status).unwrap()));
            }
            "setup" => {
                if self.setup_running.swap(true, Ordering::SeqCst) {
                    self.respond(Response::err(req.id, "setup already running"));
                    return;
                }
                self.setup_cancel.store(false, Ordering::SeqCst);
                self.respond(Response::ok(req.id, Value::Null));
                let server = Arc::clone(self);
                let runtime = self.runtime.clone();
                std::thread::spawn(move || {
                    let emit: &dyn Fn(Value) = &|v: Value| server.emit(v);
                    let server2 = Arc::clone(&server);
                    let progress = move |phase: &str, current: u64, total: u64, message: &str| {
                        server2.emit_progress(phase, current, total, message)
                    };
                    let _ = setup::run_setup(&runtime, emit, &progress, &server.setup_cancel);
                    server.setup_running.store(false, Ordering::SeqCst);
                    let status = setup::compute_status(&runtime);
                    server.emit(crate::proto::event_setup_status(&status));
                });
            }
            "cancel_setup" => {
                self.setup_cancel.store(true, Ordering::SeqCst);
                self.respond(Response::ok(req.id, Value::Null));
            }
            "selftest" => {
                self.respond(Response::ok(req.id, Value::Null)); // ack; fresh status arrives as event
                let server = Arc::clone(self);
                let runtime = self.runtime.clone();
                std::thread::spawn(move || {
                    let _ = setup::run_selftest(&runtime);
                    let status = setup::compute_status(&runtime);
                    server.emit(crate::proto::event_setup_status(&status));
                });
            }
            "enhance" => {
                let jobs = Arc::clone(&self.jobs);
                let runtime = self.runtime.clone();
                let emit = self.emitter();
                match enhance::start_job(&runtime, &jobs, &req.params, emit) {
                    Ok(job_id) => self.respond(Response::ok(req.id, Value::String(job_id))),
                    Err(e) => self.respond(Response::err(req.id, e)),
                }
            }
            "cancel_job" => {
                let job_id = req.params.get("job_id").and_then(|v| v.as_str()).unwrap_or("");
                let found = enhance::cancel_job(&self.jobs, job_id);
                self.respond(Response::ok(req.id, Value::Bool(found)));
            }
            other => self.respond(Response::err(req.id, format!("unknown command '{other}'"))),
        }
    }
}

/// Run the sidecar main loop. Never returns.
pub fn run(runtime: PathBuf) {
    if let Err(e) = setup::ensure_runtime(&runtime) {
        eprintln!("warning: {e}");
    }
    let server = Arc::new(Server::new(runtime));
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                server.respond(Response::err(0, format!("bad request json: {e}")));
                continue;
            }
        };
        server.handle(req);
    }
}

/// Persist/restore the last setup state (used by `status` after restart).
pub fn remember_state(runtime: &PathBuf, st: &SetupStateFile) {
    setup::save_state(runtime, st);
}
