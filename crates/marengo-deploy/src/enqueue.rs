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
