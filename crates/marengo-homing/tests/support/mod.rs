//! Shared test-only resource ownership; no process-global environment mutation.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub struct TestDirectory {
    base: PathBuf,
    path: PathBuf,
}

impl TestDirectory {
    pub fn new(label: &str) -> Self {
        let base = std::env::temp_dir().canonicalize().expect("test temp root");
        let label: String = label
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("host clock")
            .as_nanos();
        for _ in 0..64 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "marengo-homing-{label}-{}-{time}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { base, path },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        }
        panic!("could not exclusively create a test directory");
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        // Delete only the exclusively created direct child after checking the
        // resolved target still belongs to the intended temporary directory.
        if self
            .path
            .canonicalize()
            .ok()
            .is_some_and(|path| path.parent() == Some(self.base.as_path()))
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
