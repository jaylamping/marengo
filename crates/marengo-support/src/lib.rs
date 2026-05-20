//! Shared helpers for Marengo binaries (tracing, etc.).

use tracing_subscriber::EnvFilter;

/// Initialize `tracing` with `RUST_LOG` / `.env` filter (call once from `main`).
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}
