//! `ai-worker`: the PhotoStudio AI sidecar library.
//!
//! - [`proto`]: wire protocol types (shared with the TS frontend)
//! - [`client`]: spawn + talk to the sidecar binary (used by the Tauri app)
//! - [`server`]: the sidecar main loop (binary entry point)
//! - [`setup`]: first-run runtime/model setup + status
//! - [`enhance`]: enhancement job management
//! - [`dl`]: downloading / hashing / unzipping helpers

pub mod client;
pub mod dl;
pub mod enhance;
pub mod proto;
pub mod server;
pub mod setup;
