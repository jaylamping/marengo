use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;

use crate::ConfigError;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub fn write_profile_file_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), ConfigError> {
    write_atomic(path.as_ref(), bytes)
}

/// Cross-process serialization for profile writers sharing one config directory.
pub struct ProfileWriteLock {
    file: File,
}

impl ProfileWriteLock {
    pub fn acquire(config_dir: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let config_dir = config_dir.as_ref();
        fs::create_dir_all(config_dir).map_err(|error| ConfigError::Io {
            path: config_dir.to_path_buf(),
            message: error.to_string(),
        })?;
        let path = config_dir.join(".marengo-profile.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| ConfigError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
        file.lock_exclusive().map_err(|error| ConfigError::Io {
            path,
            message: error.to_string(),
        })?;
        Ok(Self { file })
    }
}

impl Drop for ProfileWriteLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let (temporary, mut file) = create_temp(parent, file_name)?;
    let result = (|| {
        file.write_all(bytes).map_err(|error| ConfigError::Io {
            path: temporary.clone(),
            message: error.to_string(),
        })?;
        file.sync_all().map_err(|error| ConfigError::Io {
            path: temporary.clone(),
            message: error.to_string(),
        })?;
        fs::rename(&temporary, path).map_err(|error| ConfigError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| ConfigError::Io {
                path: parent.to_path_buf(),
                message: error.to_string(),
            })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_temp(parent: &Path, file_name: &str) -> Result<(PathBuf, File), ConfigError> {
    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ConfigError::Io {
                    path: temporary,
                    message: error.to_string(),
                });
            }
        }
    }
    Err(ConfigError::Io {
        path: parent.to_path_buf(),
        message: "could not allocate a unique temporary file".to_string(),
    })
}
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::write_profile_file_atomic;

    #[test]
    fn failed_rename_preserves_target_and_removes_temporary_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let target = temp.path().join("profile.yaml");
        std::fs::create_dir(&target).expect("target directory");
        let marker = target.join("marker");
        std::fs::write(&marker, b"existing").expect("marker");

        assert!(write_profile_file_atomic(&target, b"replacement").is_err());

        assert_eq!(
            std::fs::read(marker).expect("preserved marker"),
            b"existing"
        );
        let entries = std::fs::read_dir(temp.path())
            .expect("parent")
            .map(|entry| entry.expect("entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec![std::ffi::OsString::from("profile.yaml")]);
    }
}
