//! Log-store disk accounting shared by the retention pass and host metrics.

use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::paths::log_dir;

/// Bytes held by the database file plus everything under `<root>/var/log`.
///
/// Symlinks are not followed, so a cycle or an out-of-tree link cannot inflate
/// the count. A missing database or log directory counts as zero.
pub fn log_disk_usage_bytes(marengo_root: &Path, db_path: &Path) -> Result<u64> {
    let db = fs::metadata(db_path).map(|meta| meta.len()).unwrap_or(0);
    Ok(db + dir_size(&log_dir(marengo_root))?)
}

/// Bytes held by regular files under `path`; symlinks are not followed.
fn dir_size(path: &Path) -> Result<u64> {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(0);
    };
    if meta.is_file() {
        return Ok(meta.len());
    }
    if !meta.is_dir() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(path)? {
        total += dir_size(&entry?.path())?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn counts_database_and_log_files_but_not_symlink_targets() {
        let dir = tempfile::tempdir().expect("temp dir");
        let db = dir.path().join("var/marengo.db");
        let logs = log_dir(dir.path());
        fs::create_dir_all(logs.join("blobs")).expect("dirs");
        fs::write(&db, [0u8; 10]).expect("db");
        fs::write(logs.join("bench.log"), [0u8; 20]).expect("bench");
        fs::write(logs.join("blobs/a.gz"), [0u8; 30]).expect("blob");
        let outside = dir.path().join("outside");
        fs::write(&outside, [0u8; 1000]).expect("outside");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, logs.join("link")).expect("symlink");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&logs, logs.join("blobs/loop")).expect("loop");

        assert_eq!(log_disk_usage_bytes(dir.path(), &db).expect("usage"), 60);
        assert_eq!(
            log_disk_usage_bytes(&dir.path().join("absent"), &dir.path().join("absent.db"))
                .expect("missing paths"),
            0
        );
    }
}
