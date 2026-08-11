use std::path::PathBuf;

fn env_path(name: &str) -> Option<PathBuf> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn marengo_root() -> PathBuf {
    match std::env::var("MARENGO_ROOT") {
        Ok(value) => PathBuf::from(value),
        Err(_) => PathBuf::from("/opt/marengo"),
    }
}

/// Resolve the installed revision marker.
pub fn resolve_deploy_rev_path() -> PathBuf {
    env_path("MARENGO_DEPLOY_REV_PATH").unwrap_or_else(|| marengo_root().join(".deploy-rev"))
}

/// Resolve the self-update job state file.
pub fn resolve_job_file_path() -> PathBuf {
    env_path("MARENGO_DEPLOY_JOB_FILE")
        .unwrap_or_else(|| marengo_root().join("var/deploy-job.json"))
}

/// Resolve the canonical self-update enqueue script.
pub fn resolve_enqueue_script() -> PathBuf {
    env_path("MARENGO_SELF_UPDATE_ENQUEUE_CMD")
        .unwrap_or_else(|| marengo_root().join("scripts/pi-enqueue-self-update.sh"))
}

/// Resolve the on-disk GitHub tip cache.
pub fn resolve_upstream_cache_path() -> PathBuf {
    env_path("MARENGO_UPSTREAM_CACHE_PATH")
        .unwrap_or_else(|| marengo_root().join("var/upstream-sha.json"))
}

/// Resolve the self-update log consumed by the version status endpoint.
pub fn resolve_self_update_log_path() -> PathBuf {
    env_path("MARENGO_SELF_UPDATE_LOG")
        .unwrap_or_else(|| marengo_root().join("var/self-update.log"))
}
