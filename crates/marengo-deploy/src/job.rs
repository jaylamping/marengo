use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::{resolve_deploy_rev_path, resolve_job_file_path};
use crate::rev::{read_deploy_rev, shas_match, ParsedDeployRev};
use crate::DeployError;

/// Max age for a `running` job before reconciliation marks it failed.
pub const DEPLOY_JOB_MAX_AGE_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeployJobState {
    Idle,
    Running,
    Succeeded,
    Failed,
}

impl Default for DeployJobState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Progress phases emitted by `pi-self-update.sh`.
///
/// `Unknown` accepts a phase added by a newer script without making an older
/// gateway unable to read the job file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeployPhase {
    Init,
    Dirty,
    Fetch,
    Lfs,
    Build,
    Install,
    Enqueue,
    Done,
    Timeout,
    Orphan,
    Error,
    #[serde(other)]
    Unknown,
}

impl Default for DeployPhase {
    fn default() -> Self {
        Self::Init
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DeployJob {
    pub state: DeployJobState,
    #[serde(default)]
    pub job_id: String,
    #[serde(default)]
    pub target_sha: String,
    #[serde(default)]
    pub result_sha: String,
    #[serde(default)]
    pub unit_name: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub phase: DeployPhase,
}

/// Result of reading the on-disk job ledger.
#[derive(Debug, Clone, PartialEq)]
pub enum JobFileRead {
    /// No job file yet.
    Missing,
    /// Parsed job.
    Ok(DeployJob),
    /// File exists but is not valid DeployJob JSON — fail closed for enqueue.
    Corrupt,
}

/// Read a job file without coercing corruption to Idle.
pub fn read_job_file_strict(path: &Path) -> JobFileRead {
    match std::fs::read_to_string(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => JobFileRead::Missing,
        Err(_) => JobFileRead::Corrupt,
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return JobFileRead::Missing;
            }
            match serde_json::from_str::<DeployJob>(trimmed) {
                Ok(job) => JobFileRead::Ok(job),
                Err(_) => JobFileRead::Corrupt,
            }
        }
    }
}

/// Read a job file. Missing → idle. Corrupt → failed sentinel (never Idle).
pub fn read_job_file(path: &Path) -> DeployJob {
    match read_job_file_strict(path) {
        JobFileRead::Missing => DeployJob::default(),
        JobFileRead::Ok(job) => job,
        JobFileRead::Corrupt => DeployJob {
            state: DeployJobState::Failed,
            message: "corrupt deploy-job.json — refusing Idle fallback".to_string(),
            phase: DeployPhase::Error,
            ..DeployJob::default()
        },
    }
}

/// Atomically write a job file, creating its parent directory as needed.
pub fn write_job_file(path: &Path, job: &DeployJob) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| DeployError::Io {
            operation: "mkdir job dir",
            source,
        })?;
    }
    let raw = serde_json::to_string_pretty(job)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|source| DeployError::Io {
        operation: "write job tmp",
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| DeployError::Io {
        operation: "rename job",
        source,
    })
}

fn parse_iso_unix(iso: &str) -> Option<u64> {
    // Accept `YYYY-MM-DDTHH:MM:SSZ` only (what our scripts write).
    let trimmed = iso.trim().trim_end_matches('Z');
    let (date, clock) = trimmed.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u8 = date_parts.next()?.parse().ok()?;
    let day: u8 = date_parts.next()?.parse().ok()?;
    let mut time_parts = clock.split(':');
    let hour: u8 = time_parts.next()?.parse().ok()?;
    let minute: u8 = time_parts.next()?.parse().ok()?;
    let second: u8 = time_parts.next()?.parse().ok()?;
    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let time = time::Time::from_hms(hour, minute, second).ok()?;
    let datetime = time::PrimitiveDateTime::new(date, time).assume_utc();
    Some(datetime.unix_timestamp().max(0) as u64)
}

pub(crate) fn unix_now() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

pub(crate) fn format_unix_iso(secs: u64) -> String {
    let Ok(datetime) = time::OffsetDateTime::from_unix_timestamp(secs as i64) else {
        return String::new();
    };
    match datetime.format(&time::format_description::well_known::Rfc3339) {
        Ok(value) => value.replace("+00:00", "Z"),
        Err(_) => String::new(),
    }
}

/// Reconcile a running job with the installed revision and service state.
pub fn reconcile_job(job: &mut DeployJob, deploy_sha: &str, max_age_secs: u64) -> bool {
    if job.state != DeployJobState::Running {
        return false;
    }
    if !job.target_sha.is_empty() && shas_match(deploy_sha, &job.target_sha) {
        job.state = DeployJobState::Succeeded;
        job.result_sha = deploy_sha.to_string();
        job.message = "reconciled: deploy-rev matches target".to_string();
        job.phase = DeployPhase::Done;
        job.updated_at = format_unix_iso(unix_now());
        return true;
    }
    let started = parse_iso_unix(&job.started_at).map_or(0, |value| value);
    if started > 0 && unix_now().saturating_sub(started) > max_age_secs {
        job.state = DeployJobState::Failed;
        job.message = format!("stale running job older than {max_age_secs}s");
        job.phase = DeployPhase::Timeout;
        job.updated_at = format_unix_iso(unix_now());
        return true;
    }
    if !job.unit_name.is_empty() && !unit_is_active(&job.unit_name) {
        // Unit finished but job file not updated (killed mid-write) — wait for age unless rev matches.
        if started > 0 && unix_now().saturating_sub(started) > 120 {
            job.state = DeployJobState::Failed;
            job.message = format!("unit {} inactive without success", job.unit_name);
            job.phase = DeployPhase::Orphan;
            job.updated_at = format_unix_iso(unix_now());
            return true;
        }
    }
    false
}

/// Read and reconcile the persisted job used by both status and control handlers.
pub fn load_reconciled_job() -> (ParsedDeployRev, DeployJob) {
    let deploy = read_deploy_rev(&resolve_deploy_rev_path());
    let job_path = resolve_job_file_path();
    let mut job = read_job_file(&job_path);
    if reconcile_job(&mut job, &deploy.sha, DEPLOY_JOB_MAX_AGE_SECS) {
        let _ = write_job_file(&job_path, &job);
    }
    (deploy, job)
}

fn unit_is_active(unit: &str) -> bool {
    let name = if unit.ends_with(".service") {
        unit.to_string()
    } else {
        format!("{unit}.service")
    };
    match std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", &name])
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn reconcile_promotes_when_rev_matches() {
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(&mut job, "abcdef0123456789", 10));
        assert_eq!(job.state, DeployJobState::Succeeded);
        assert_eq!(job.phase, DeployPhase::Done);
    }

    #[test]
    fn reconcile_fails_stale_running() {
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789".into(),
            started_at: "2020-01-01T00:00:00Z".into(),
            unit_name: String::new(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(&mut job, "deadbeef", 60));
        assert_eq!(job.state, DeployJobState::Failed);
        assert_eq!(job.phase, DeployPhase::Timeout);
    }

    #[test]
    fn job_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("deploy-job.json");
        let job = DeployJob {
            state: DeployJobState::Running,
            job_id: "j1".into(),
            target_sha: "abc".into(),
            message: "hi".into(),
            phase: DeployPhase::Build,
            ..DeployJob::default()
        };
        write_job_file(&path, &job).expect("write");
        let loaded = read_job_file(&path);
        assert_eq!(loaded.job_id, "j1");
        assert_eq!(loaded.state, DeployJobState::Running);
        assert_eq!(loaded.phase, DeployPhase::Build);
    }

    #[test]
    fn unknown_phase_is_forward_compatible() {
        let job: DeployJob =
            serde_json::from_str(r#"{"state":"running","phase":"future_phase"}"#).expect("json");
        assert_eq!(job.phase, DeployPhase::Unknown);
    }

    #[test]
    fn corrupt_job_file_fails_closed_not_idle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("deploy-job.json");
        std::fs::write(&path, "{not-json").expect("write");
        assert_eq!(read_job_file_strict(&path), JobFileRead::Corrupt);
        let job = read_job_file(&path);
        assert_eq!(job.state, DeployJobState::Failed);
        assert_ne!(job.state, DeployJobState::Idle);
    }
}
