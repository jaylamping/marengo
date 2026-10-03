use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::error::Result;
use crate::job::{format_unix_iso, unix_now};
use crate::paths::resolve_enqueue_script;
use crate::{DeployError, DeployError::EnqueueFailed};

/// Generate the job identifier used by the self-update scripts.
pub fn new_job_id(target_sha: &str) -> String {
    let now = unix_now();
    let mut hasher = DefaultHasher::new();
    target_sha.hash(&mut hasher);
    now.hash(&mut hasher);
    format!(
        "{}-{:x}",
        format_unix_iso(now).replace(':', ""),
        hasher.finish() & 0xffff
    )
}

/// Enqueue a self-update through the canonical script.
pub async fn enqueue_self_update(target_sha: &str, job_id: &str) -> Result<()> {
    let script = resolve_enqueue_script();
    if !script.is_file() {
        return Err(DeployError::EnqueueScriptMissing { path: script });
    }

    let skip_sudo = match std::env::var("MARENGO_SELF_UPDATE_SKIP_SUDO") {
        Ok(value) => value == "1" || value.eq_ignore_ascii_case("true"),
        Err(_) => false,
    };

    let mut command = if skip_sudo {
        let mut command = Command::new(&script);
        command.arg(target_sha).arg(job_id);
        command
    } else {
        let mut command = Command::new("sudo");
        command.arg("-n").arg(&script).arg(target_sha).arg(job_id);
        command
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .map_err(|_| DeployError::EnqueueTimeout)?
        .map_err(DeployError::EnqueueSpawn)?;
    if !output.status.success() {
        let output = String::from_utf8_lossy(&output.stderr)
            .chars()
            .chain(String::from_utf8_lossy(&output.stdout).chars())
            .take(500)
            .collect::<String>();
        return Err(EnqueueFailed { output });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use tokio::sync::Mutex as AsyncMutex;

    static TEST_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

    struct EnvGuard {
        key: &'static str,
        prior: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prior = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, prior }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.prior.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn helper_script(body: &str) -> tempfile::TempPath {
        let mut file = tempfile::Builder::new()
            .prefix("enqueue-helper")
            .suffix(".sh")
            .tempfile()
            .expect("temp helper");
        use std::io::Write;
        file.write_all(body.as_bytes()).expect("write helper");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o755))
                .expect("chmod helper");
        }
        file.into_temp_path()
    }

    #[tokio::test]
    async fn enqueue_reports_helper_success() {
        let _serial = TEST_LOCK.lock().await;
        let helper = helper_script("#!/bin/sh\nexit 0\n");
        let _cmd = EnvGuard::set(
            "MARENGO_SELF_UPDATE_ENQUEUE_CMD",
            helper.to_str().expect("path"),
        );
        let _sudo = EnvGuard::set("MARENGO_SELF_UPDATE_SKIP_SUDO", "1");
        enqueue_self_update("abcdef0123456789abcdef0123456789abcdef01", "test-job-1")
            .await
            .expect("helper exit 0 enqueues");
    }

    #[tokio::test]
    async fn enqueue_surfaces_helper_failure_output() {
        let _serial = TEST_LOCK.lock().await;
        let helper = helper_script("#!/bin/sh\necho helper-boom >&2\nexit 3\n");
        let _cmd = EnvGuard::set(
            "MARENGO_SELF_UPDATE_ENQUEUE_CMD",
            helper.to_str().expect("path"),
        );
        let _sudo = EnvGuard::set("MARENGO_SELF_UPDATE_SKIP_SUDO", "1");
        let error = enqueue_self_update("abcdef0123456789abcdef0123456789abcdef01", "test-job-2")
            .await
            .expect_err("helper exit 3 must fail");
        assert!(
            error.to_string().contains("helper-boom"),
            "stderr surfaced: {error}"
        );
    }

    #[tokio::test]
    async fn enqueue_refuses_missing_helper() {
        let _serial = TEST_LOCK.lock().await;
        let _cmd = EnvGuard::set(
            "MARENGO_SELF_UPDATE_ENQUEUE_CMD",
            "/nonexistent/marengo-test/pi-enqueue-self-update.sh",
        );
        let _sudo = EnvGuard::set("MARENGO_SELF_UPDATE_SKIP_SUDO", "1");
        let error = enqueue_self_update("abcdef0123456789abcdef0123456789abcdef01", "test-job-3")
            .await
            .expect_err("missing helper must fail");
        assert!(
            matches!(error, DeployError::EnqueueScriptMissing { .. }),
            "missing helper is typed: {error}"
        );
    }
}
