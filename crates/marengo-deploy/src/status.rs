use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::job::{load_reconciled_job, DeployJob, DeployJobState};
use crate::paths::resolve_self_update_log_path;
use crate::rev::{shas_match, ParsedDeployRev};
use crate::upstream::fetch_upstream_sha;

/// The state the Consul sidebar should use for the installed version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateUiState {
    Unknown,
    Current,
    Stale,
    UpstreamUnknown,
    Updating,
    Failed,
}

/// Version and self-update status returned by the gateway.
#[derive(Debug, Clone, Serialize)]
pub struct VersionStatus {
    pub deploy_sha: String,
    pub deployed_at: Option<String>,
    pub upstream_sha: String,
    pub upstream_fetched_at: Option<String>,
    pub upstream_ok: bool,
    pub update_available: bool,
    pub ready_for_target: bool,
    pub deploy: DeployJob,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<String>,
    pub ui_state: UpdateUiState,
}

/// Derive the single authoritative UI state from deploy and job facts.
pub fn derive_ui_state(
    deploy_sha: &str,
    upstream_ok: bool,
    update_available: bool,
    job: &DeployJob,
    ready_for_target: bool,
) -> UpdateUiState {
    match job.state {
        DeployJobState::Failed => UpdateUiState::Failed,
        DeployJobState::Running => UpdateUiState::Updating,
        // Keep the sidebar in Updating until www/rev readiness lands after install.
        DeployJobState::Succeeded if !ready_for_target => UpdateUiState::Updating,
        DeployJobState::Idle | DeployJobState::Succeeded => {
            if !upstream_ok {
                UpdateUiState::UpstreamUnknown
            } else if deploy_sha.is_empty() {
                UpdateUiState::Unknown
            } else if update_available {
                UpdateUiState::Stale
            } else {
                UpdateUiState::Current
            }
        }
    }
}

/// Return whether the installed target is ready to serve.
///
/// The job argument is retained as part of the domain interface for callers
/// that already have the reconciled job; readiness is based on the same
/// revision and web-root checks for every job state.
pub fn ready_for_target(deploy_sha: &str, target: &str, _job: &DeployJob) -> bool {
    if target.is_empty() || !shas_match(deploy_sha, target) {
        return false;
    }
    www_index_present()
}

/// Assemble a status snapshot after job reconciliation and upstream fetching.
pub fn assemble_version_status(
    deploy: ParsedDeployRev,
    upstream_sha: String,
    upstream_ok: bool,
    fetched_at: u64,
    job: DeployJob,
    log_tail: Option<String>,
) -> VersionStatus {
    let update_available =
        upstream_ok && !deploy.sha.is_empty() && !shas_match(&deploy.sha, &upstream_sha);
    let target_for_ready = if job.target_sha.is_empty() {
        upstream_sha.as_str()
    } else {
        job.target_sha.as_str()
    };
    let ready_for_target = ready_for_target(&deploy.sha, target_for_ready, &job)
        && job.state == DeployJobState::Succeeded;
    let ui_state = derive_ui_state(
        &deploy.sha,
        upstream_ok,
        update_available,
        &job,
        ready_for_target,
    );

    VersionStatus {
        deploy_sha: deploy.sha,
        deployed_at: deploy.deployed_at,
        upstream_sha,
        upstream_fetched_at: (fetched_at > 0).then(|| crate::job::format_unix_iso(fetched_at)),
        upstream_ok,
        update_available,
        ready_for_target,
        deploy: job,
        log_tail,
        ui_state,
    }
}

/// Read, reconcile, fetch, and assemble the current version status.
pub async fn current_version_status(refresh: bool) -> VersionStatus {
    let (deploy, job) = load_reconciled_job();
    let (upstream_sha, upstream_ok, fetched_at) = fetch_upstream_sha(refresh).await;
    let log_tail = if job.state == DeployJobState::Failed {
        read_log_tail(&resolve_self_update_log_path(), 4000)
    } else {
        None
    };
    assemble_version_status(deploy, upstream_sha, upstream_ok, fetched_at, job, log_tail)
}

fn www_index_present() -> bool {
    let root = match std::env::var("MARENGO_ROOT") {
        Ok(value) => value,
        Err(_) => "/opt/marengo".to_string(),
    };
    PathBuf::from(root).join("www/index.html").is_file()
        || std::env::var("MARENGO_SKIP_WWW_READY").ok().as_deref() == Some("1")
}

fn read_log_tail(path: &Path, max_bytes: usize) -> Option<String> {
    let raw = std::fs::read(path).ok()?;
    if raw.is_empty() {
        return None;
    }
    let start = raw.len().saturating_sub(max_bytes);
    Some(String::from_utf8_lossy(&raw[start..]).into_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::job::DeployPhase;

    fn job(state: DeployJobState) -> DeployJob {
        DeployJob {
            state,
            ..DeployJob::default()
        }
    }

    #[test]
    fn ui_state_prioritizes_failed_and_running_jobs() {
        assert_eq!(
            derive_ui_state("", false, false, &job(DeployJobState::Failed), false),
            UpdateUiState::Failed
        );
        assert_eq!(
            derive_ui_state("", false, false, &job(DeployJobState::Running), false),
            UpdateUiState::Updating
        );
        assert_eq!(
            derive_ui_state(
                "abcdef0",
                true,
                false,
                &job(DeployJobState::Succeeded),
                false
            ),
            UpdateUiState::Updating
        );
    }

    #[test]
    fn ui_state_distinguishes_current_stale_and_unknown() {
        assert_eq!(
            derive_ui_state("", true, false, &job(DeployJobState::Idle), false),
            UpdateUiState::Unknown
        );
        assert_eq!(
            derive_ui_state("abcdef0", false, false, &job(DeployJobState::Idle), false),
            UpdateUiState::UpstreamUnknown
        );
        assert_eq!(
            derive_ui_state("abcdef0", true, true, &job(DeployJobState::Idle), false),
            UpdateUiState::Stale
        );
        assert_eq!(
            derive_ui_state("abcdef0", true, false, &job(DeployJobState::Idle), true),
            UpdateUiState::Current
        );
        assert_eq!(
            derive_ui_state(
                "abcdef0",
                true,
                false,
                &job(DeployJobState::Succeeded),
                true
            ),
            UpdateUiState::Current
        );
    }

    #[test]
    fn assemble_status_serializes_typed_phase_and_ui_state() {
        std::env::set_var("MARENGO_SKIP_WWW_READY", "1");
        let status = assemble_version_status(
            ParsedDeployRev {
                sha: "abcdef0".to_string(),
                deployed_at: None,
            },
            "abcdef0".to_string(),
            true,
            0,
            DeployJob {
                target_sha: "abcdef0".to_string(),
                phase: DeployPhase::Done,
                ..job(DeployJobState::Succeeded)
            },
            None,
        );
        std::env::remove_var("MARENGO_SKIP_WWW_READY");
        let json = serde_json::to_value(status).expect("serialize");
        assert_eq!(json["ui_state"], "current");
        assert_eq!(json["deploy"]["phase"], "done");
        assert_eq!(json["ready_for_target"], true);
    }
}
