use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by deploy-domain side effects.
#[derive(Debug, Error)]
pub enum DeployError {
    #[error("{operation}: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("serialize deploy job: {0}")]
    Json(#[from] serde_json::Error),
    #[error("enqueue script not found: {path}")]
    EnqueueScriptMissing { path: PathBuf },
    #[error("enqueue timed out")]
    EnqueueTimeout,
    #[error("enqueue spawn: {0}")]
    EnqueueSpawn(#[source] std::io::Error),
    #[error("enqueue failed: {output}")]
    EnqueueFailed { output: String },
}

pub type Result<T> = std::result::Result<T, DeployError>;
