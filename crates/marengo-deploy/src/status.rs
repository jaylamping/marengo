use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::job::{load_reconciled_job, DeployJob, DeployJobState, DeployPhase, JobFileRead};
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

fn idle_like_ui_state(
    deploy_sha: &str,
    upstream_ok: bool,
    update_available: bool,
) -> UpdateUiState {
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

/// Derive the single authoritative UI state from deploy and job facts.
fn derive_ui_state(
    deploy_sha: &str,
    upstream_ok: bool,
    update_available: bool,
    job: &DeployJob,
    ready_for_target: bool,
) -> UpdateUiState {
    match job.state {
        DeployJobState::Running => UpdateUiState::Updating,
        // Terminal install without a servable UI — never stick in Updating.
        DeployJobState::Succeeded if !ready_for_target => UpdateUiState::Failed,
        // Failed must not block retry: if still behind main, surface Stale + Update.
        DeployJobState::Failed if update_available => UpdateUiState::Stale,
        DeployJobState::Failed => UpdateUiState::Failed,
        DeployJobState::Idle | DeployJobState::Succeeded => {
            idle_like_ui_state(deploy_sha, upstream_ok, update_available)
        }
    }
}

/// Return whether the installed target is ready to serve: the revision matches
/// and the Consul web root is present, for every job state.
fn ready_for_target(deploy_sha: &str, target: &str) -> bool {
    // Readiness is promotion-grade: a short or hand-edited `.deploy-rev`
    // prefix never counts as ready, even when it prefix-matches the target.
    if target.is_empty() || !crate::job::promotion_match(deploy_sha, target) {
        return false;
    }
    web_root_ready()
}

/// Assemble a status snapshot after job reconciliation and upstream fetching.
fn assemble_version_status(
    deploy: ParsedDeployRev,
    upstream_sha: String,
    upstream_ok: bool,
    fetched_at: u64,
    job: DeployJob,
    log_tail: Option<String>,
) -> VersionStatus {
    let update_available =
        upstream_ok && !deploy.sha.is_empty() && !shas_match(&deploy.sha, &upstream_sha);
    let mut job = job;
    let target_for_ready = if job.target_sha.is_empty() {
        upstream_sha.clone()
    } else {
        job.target_sha.clone()
    };
    let ready_for_target =
        ready_for_target(&deploy.sha, &target_for_ready) && job.state == DeployJobState::Succeeded;
    // Persist a retryable failure when install "succeeded" without a web root.
    // Compare-and-swap on job_id: if a newer phase landed after our read,
    // keep it and display the fresh ledger instead of overwriting it.
    if job.state == DeployJobState::Succeeded && !ready_for_target {
        let mut demoted = job.clone();
        demoted.state = DeployJobState::Failed;
        demoted.phase = DeployPhase::Error;
        demoted.message =
            "install finished but www/index.html missing — retry Update after building Consul"
                .to_string();
        demoted.updated_at = crate::job::format_unix_iso(crate::job::unix_now());
        match crate::job::compare_and_write_job(
            &crate::paths::resolve_job_file_path(),
            &job.job_id,
            &demoted,
        ) {
            Ok(true) => job = demoted,
            Ok(false) => {
                job = crate::job::read_job_file(&crate::paths::resolve_job_file_path());
            }
            Err(error) => {
                tracing::warn!(%error, "demote write failed; displaying in-memory state");
            }
        }
    }
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
///
/// Blocking ledger/systemctl/log-tail I/O runs on the blocking pool: this
/// future never holds a Tokio worker across a filesystem or `systemctl`
/// call. Only the GitHub-tip fetch stays on the async worker.
pub async fn current_version_status(refresh: bool) -> VersionStatus {
    let (deploy, read) = tokio::task::spawn_blocking(load_reconciled_job)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "reconcile task failed; serving unreconciled empty status");
            (
                ParsedDeployRev {
                    sha: String::new(),
                    deployed_at: None,
                },
                JobFileRead::Missing,
            )
        });
    let job = read.job_or_sentinel();
    let (upstream_sha, upstream_ok, fetched_at) = fetch_upstream_sha(refresh).await;
    let log_tail = if job.state == DeployJobState::Failed {
        let log_path = resolve_self_update_log_path();
        tokio::task::spawn_blocking(move || read_log_tail(&log_path, 4000))
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    assemble_version_status(deploy, upstream_sha, upstream_ok, fetched_at, job, log_tail)
}

/// True when Consul `www/index.html` is present (or readiness check skipped).
pub fn web_root_ready() -> bool {
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
    fn ui_state_prioritizes_running_and_surfaces_failed_without_www() {
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
            UpdateUiState::Failed
        );
        // Failed + behind → Stale so Update stays available.
        assert_eq!(
            derive_ui_state("abcdef0", true, true, &job(DeployJobState::Failed), false),
            UpdateUiState::Stale
        );
        assert_eq!(
            derive_ui_state("abcdef0", true, false, &job(DeployJobState::Failed), false),
            UpdateUiState::Failed
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
        // Readiness is promotion-grade: the installed rev must be a full SHA.
        let installed = "abcdef0123456789abcdef0123456789abcdef01";
        let status = assemble_version_status(
            ParsedDeployRev {
                sha: installed.to_string(),
                deployed_at: None,
            },
            installed.to_string(),
            true,
            0,
            DeployJob {
                target_sha: installed.to_string(),
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
