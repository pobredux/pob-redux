//! Headless Path of Building (PoE2) engine.
//!
//! PoB's calculation engine is ~90k lines of Lua that assume a "SimpleGraphic"
//! host. This crate embeds LuaJIT, provides a headless implementation of that
//! host, boots PoB's own `Launch.lua`, and exposes a JSON-in/JSON-out method
//! table (`lua/bridge.lua`) on top of the live build object.
//!
//! Everything PoB computes is therefore computed by PoB itself; this crate only
//! marshals data in and out.

mod engine;
mod native;
pub mod pool;
mod pool_sync;
mod worker;

pub use engine::{Engine, EngineConfig};
pub use pool::{EnginePool, PoolStatus};
pub use worker::{EngineHandle, EngineState, EngineStatus};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("lua: {0}")]
    Lua(#[from] mlua::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("engine is not running: {0}")]
    NotRunning(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// reqwest's rustls-no-provider panics without a process default; ring is what tauri-plugin-updater installs.
pub fn install_tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
