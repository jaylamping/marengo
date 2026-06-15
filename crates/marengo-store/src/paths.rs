use std::path::{Path, PathBuf};

/// Default database path under Marengo install root.
pub fn default_db_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join("var/marengo.db")
}

/// Hot log directory and blob archive root.
pub fn log_dir(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join("var/log")
}

pub fn blob_dir(root: impl AsRef<Path>) -> PathBuf {
    log_dir(root).join("blobs")
}

pub fn resolve_marengo_root() -> PathBuf {
    std::env::var("MARENGO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/marengo"))
}

pub fn resolve_db_path() -> PathBuf {
    std::env::var("MARENGO_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_db_path(resolve_marengo_root()))
}

pub const DEFAULT_ARCHIVE_DAYS: u32 = 30;
pub const DEFAULT_HOT_KEEP: usize = 50;
pub const DEFAULT_LOG_DISK_BUDGET_BYTES: u64 = 5 * 1024 * 1024 * 1024;
