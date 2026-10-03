use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::{resolve_deploy_rev_path, resolve_job_file_path};
use crate::rev::{is_full_sha, read_deploy_rev, ParsedDeployRev};
use crate::DeployError;

/// Process-unique suffix for job temp files (shared with shell writers' `.$$`).
static JOB_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
impl JobFileRead {
    /// Fold the read into a `DeployJob`: missing → idle, corrupt → failed
    /// sentinel (never Idle), mirroring [`read_job_file`] without re-reading.
    pub fn job_or_sentinel(self) -> DeployJob {
        match self {
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
}

/// Read a job file. Missing → idle. Corrupt → failed sentinel (never Idle).
pub fn read_job_file(path: &Path) -> DeployJob {
    read_job_file_strict(path).job_or_sentinel()
}

/// Advisory lock shared by every `deploy-job.json` writer (Rust, shell, MCP).
///
/// The gateway, `pi-enqueue-self-update.sh`, `pi-self-update.sh` and the MCP
/// `pi_native` path all serialize through this sidecar so a status
/// reconcile/demote read-modify-write cannot tear or overwrite a newer phase.
pub fn job_file_lock_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.lock", path.display()))
}

/// Acquire the shared [`job_file_lock_path`] advisory lock.
///
/// The returned file releases the lock when closed. Hold it across a whole
/// read-modify-write (reconcile, demote) so a concurrent phase advance from
/// another writer cannot slip between the read and the write.
fn lock_job_file(path: &Path) -> Result<std::fs::File> {
    let lock_path = job_file_lock_path(path);
    if let Some(parent) = lock_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| DeployError::Io {
                operation: "mkdir job lock dir",
                source,
            })?;
        }
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| DeployError::Io {
            operation: "open job lock",
            source,
        })?;
    lock.lock_exclusive().map_err(|source| DeployError::Io {
        operation: "lock job file",
        source,
    })?;
    Ok(lock)
}

/// Overwrite the ledger only when it still holds `expected_job_id`.
///
/// Holds the job lock across the re-read and the write, so a status
/// reconcile/demote cannot overwrite a newer phase (different job id) that
/// landed after the caller read. Returns true when the write happened.
pub(crate) fn compare_and_write_job(
    path: &Path,
    expected_job_id: &str,
    new_job: &DeployJob,
) -> Result<bool> {
    let lock = lock_job_file(path)?;
    let current = read_job_file_strict(path);
    let same = matches!(&current, JobFileRead::Ok(job) if job.job_id == expected_job_id);
    if !same {
        let _ = fs2::FileExt::unlock(&lock);
        return Ok(false);
    }
    let raw = serde_json::to_string_pretty(new_job)?;
    let result = write_job_file_locked(path, raw.as_bytes());
    let _ = fs2::FileExt::unlock(&lock);
    result.map(|()| true)
}

fn write_job_file_locked(path: &Path, raw: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("deploy-job.json");
    let unique = JOB_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!("{file_name}.tmp.{}.{}", std::process::id(), unique);
    let tmp = path.with_file_name(tmp_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|source| DeployError::Io {
            operation: "write job tmp",
            source,
        })?;
    let result = (|| {
        file.write_all(raw).map_err(|source| DeployError::Io {
            operation: "write job tmp",
            source,
        })?;
        file.sync_all().map_err(|source| DeployError::Io {
            operation: "fsync job tmp",
            source,
        })?;
        drop(file);
        std::fs::rename(&tmp, path).map_err(|source| DeployError::Io {
            operation: "rename job",
            source,
        })?;
        if let Some(parent) = path.parent() {
            let dir = std::fs::File::open(parent).map_err(|source| DeployError::Io {
                operation: "open job dir",
                source,
            })?;
            dir.sync_all().map_err(|source| DeployError::Io {
                operation: "fsync job dir",
                source,
            })?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
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
///
/// `file_mtime_secs` is the job file's mtime and backstops an unparseable
/// `started_at`: without a trustworthy start time a running job must still
/// time out instead of wedging `Updating` forever. Pass `None` only when the
/// mtime is unavailable — then an unparseable timestamp fails closed.
pub fn reconcile_job(
    job: &mut DeployJob,
    deploy_sha: &str,
    max_age_secs: u64,
    file_mtime_secs: Option<u64>,
) -> bool {
    if job.state != DeployJobState::Running {
        return false;
    }
    // Promotion needs a full installed SHA, not a prefix: a short or edited
    // `.deploy-rev` must never flip a running job to Succeeded.
    if !job.target_sha.is_empty() && promotion_match(deploy_sha, &job.target_sha) {
        job.state = DeployJobState::Succeeded;
        job.result_sha = deploy_sha.to_string();
        job.message = "reconciled: deploy-rev matches target".to_string();
        job.phase = DeployPhase::Done;
        job.updated_at = format_unix_iso(unix_now());
        return true;
    }
    let started = parse_iso_unix(&job.started_at).or(file_mtime_secs);
    let Some(started) = started else {
        job.state = DeployJobState::Failed;
        job.message = "running job has no parseable start time and no file mtime".to_string();
        job.phase = DeployPhase::Timeout;
        job.updated_at = format_unix_iso(unix_now());
        return true;
    };
    if unix_now().saturating_sub(started) > max_age_secs {
        job.state = DeployJobState::Failed;
        job.message = format!("stale running job older than {max_age_secs}s");
        job.phase = DeployPhase::Timeout;
        job.updated_at = format_unix_iso(unix_now());
        return true;
    }
    if !job.unit_name.is_empty() && !unit_is_active(&job.unit_name) {
        // Unit finished but job file not updated (killed mid-write) — wait for age unless rev matches.
        if unix_now().saturating_sub(started) > 120 {
            job.state = DeployJobState::Failed;
            job.message = format!("unit {} inactive without success", job.unit_name);
            job.phase = DeployPhase::Orphan;
            job.updated_at = format_unix_iso(unix_now());
            return true;
        }
    }
    false
}

/// Promotion-grade SHA comparison: the installed revision must be a full
/// 40-hex SHA that equals the target or extends a ≥7-char target prefix.
pub(crate) fn promotion_match(installed: &str, target: &str) -> bool {
    let installed = installed.trim().to_ascii_lowercase();
    let target = target.trim().to_ascii_lowercase();
    if !is_full_sha(&installed) || target.is_empty() {
        return false;
    }
    installed == target || (target.len() >= 7 && installed.starts_with(&target))
}

/// Empty fallback when the reconcile blocking task cannot join: no revision,
/// no job. Callers treat it as "nothing running" for display and refuse
/// enqueue fail-closed only when the ledger itself reads `Corrupt`
/// (a join failure is not ledger evidence, so it stays `Missing`).
pub fn empty_reconciled() -> (ParsedDeployRev, JobFileRead) {
    (
        ParsedDeployRev {
            sha: String::new(),
            deployed_at: None,
        },
        JobFileRead::Missing,
    )
}

/// Read and reconcile the persisted job used by both status and control handlers.
///
/// Returns the strict read: a corrupt ledger stays `Corrupt` so callers can
/// refuse to enqueue instead of coercing it. Reconcile write failures are
/// reported via tracing and retried on the next poll; the in-memory
/// reconciled job is still returned.
///
/// Blocking file I/O plus a `systemctl` probe — call from `spawn_blocking`,
/// never directly on a Tokio worker.
pub fn load_reconciled_job() -> (ParsedDeployRev, JobFileRead) {
    let deploy = read_deploy_rev(&resolve_deploy_rev_path());
    let job_path = resolve_job_file_path();
    // Hold the shared job lock across the whole read-reconcile-write so a
    // concurrent writer (enqueue helper, self-update script, MCP) cannot
    // land a newer phase between our read and our write.
    let lock = match lock_job_file(&job_path) {
        Ok(lock) => lock,
        Err(error) => {
            tracing::warn!(%error, "job lock unavailable; serving unreconciled read");
            return (deploy, read_job_file_strict(&job_path));
        }
    };
    let mtime = std::fs::metadata(&job_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|age| age.as_secs());
    let read = read_job_file_strict(&job_path);
    let JobFileRead::Ok(mut job) = read.clone() else {
        let _ = fs2::FileExt::unlock(&lock);
        return (deploy, read);
    };
    if reconcile_job(&mut job, &deploy.sha, DEPLOY_JOB_MAX_AGE_SECS, mtime) {
        let raw = match serde_json::to_string_pretty(&job) {
            Ok(raw) => raw,
            Err(error) => {
                let _ = fs2::FileExt::unlock(&lock);
                tracing::warn!(%error, "reconciled job serialize failed");
                return (deploy, JobFileRead::Ok(job));
            }
        };
        if let Err(error) = write_job_file_locked(&job_path, raw.as_bytes()) {
            tracing::warn!(%error, "reconciled job write failed; retrying on next poll");
        }
    }
    let _ = fs2::FileExt::unlock(&lock);
    (deploy, JobFileRead::Ok(job))
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

    /// Locked durable write for fixtures: the same shared lock and
    /// tmp+fsync+rename path production RMW/CAS writes go through.
    fn locked_test_write(path: &Path, job: &DeployJob) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir fixture dir");
        }
        let raw = serde_json::to_string_pretty(job).expect("serialize fixture");
        let lock = lock_job_file(path).expect("fixture lock");
        let result = write_job_file_locked(path, raw.as_bytes());
        let _ = fs2::FileExt::unlock(&lock);
        result.expect("fixture write");
    }

    use super::*;

    #[test]
    fn reconcile_promotes_when_rev_matches() {
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef01".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(
            &mut job,
            "abcdef0123456789abcdef0123456789abcdef01",
            10,
            None
        ));
        assert_eq!(job.state, DeployJobState::Succeeded);
        assert_eq!(job.phase, DeployPhase::Done);
    }

    #[test]
    fn reconcile_promotes_when_installed_extends_target_prefix() {
        let installed = "abcdef0123456789abcdef0123456789abcdef01";
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0".into(),
            started_at: "2026-01-01T00:00:00Z".into(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(&mut job, installed, 10, None));
        assert_eq!(job.state, DeployJobState::Succeeded);
    }

    #[test]
    fn reconcile_never_promotes_on_short_installed_prefix() {
        // A short or hand-edited `.deploy-rev` must not flip Running to
        // Succeeded, even when the prefix matches the target.
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef01".into(),
            started_at: format_unix_iso(unix_now()),
            unit_name: String::new(),
            ..DeployJob::default()
        };
        assert!(!reconcile_job(&mut job, "abcdef0", 10, Some(unix_now())));
        assert_eq!(job.state, DeployJobState::Running);
    }

    #[test]
    fn reconcile_fails_stale_running() {
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef02".into(),
            started_at: "2020-01-01T00:00:00Z".into(),
            unit_name: String::new(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(&mut job, "deadbeef", 60, None));
        assert_eq!(job.state, DeployJobState::Failed);
        assert_eq!(job.phase, DeployPhase::Timeout);
    }

    #[test]
    fn reconcile_unparseable_start_falls_back_to_mtime() {
        // The MCP direct path can leave `started_at` malformed with an empty
        // unit: the job must still time out instead of wedging Updating.
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef03".into(),
            started_at: "not-a-timestamp".into(),
            unit_name: String::new(),
            ..DeployJob::default()
        };
        let old_mtime = unix_now().saturating_sub(DEPLOY_JOB_MAX_AGE_SECS + 60);
        assert!(reconcile_job(
            &mut job,
            "deadbeef",
            DEPLOY_JOB_MAX_AGE_SECS,
            Some(old_mtime)
        ));
        assert_eq!(job.state, DeployJobState::Failed);
        assert_eq!(job.phase, DeployPhase::Timeout);
    }

    #[test]
    fn reconcile_unparseable_start_without_mtime_fails_closed() {
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef04".into(),
            started_at: String::new(),
            unit_name: String::new(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(
            &mut job,
            "deadbeef",
            DEPLOY_JOB_MAX_AGE_SECS,
            None
        ));
        assert_eq!(job.state, DeployJobState::Failed);
        assert_eq!(job.phase, DeployPhase::Timeout);
    }

    #[test]
    fn reconcile_fresh_mtime_without_start_stays_running() {
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef05".into(),
            started_at: "garbage".into(),
            unit_name: String::new(),
            ..DeployJob::default()
        };
        assert!(!reconcile_job(
            &mut job,
            "deadbeef",
            DEPLOY_JOB_MAX_AGE_SECS,
            Some(unix_now())
        ));
        assert_eq!(job.state, DeployJobState::Running);
    }

    #[test]
    fn reconcile_orphans_inactive_unit_without_success() {
        // A running job whose unit is long gone (killed mid-write, or the MCP
        // direct path with an empty unit) must fail as Orphan, never wedge.
        // `systemctl` is absent on workstations and the unit does not exist
        // on the Pi, so both report inactive without hardware.
        // Younger than the 30 min timeout but older than the 120 s orphan
        // horizon, so the inactive-unit branch (not the age branch) fires.
        let mut job = DeployJob {
            state: DeployJobState::Running,
            target_sha: "abcdef0123456789abcdef0123456789abcdef06".into(),
            started_at: format_unix_iso(unix_now().saturating_sub(600)),
            unit_name: "marengo-test-nonexistent-unit".into(),
            ..DeployJob::default()
        };
        assert!(reconcile_job(
            &mut job,
            "deadbeef",
            DEPLOY_JOB_MAX_AGE_SECS,
            None
        ));
        assert_eq!(job.state, DeployJobState::Failed);
        assert_eq!(job.phase, DeployPhase::Orphan);
    }

    #[test]
    fn compare_and_write_refuses_a_newer_phase() {
        // A status demote must not overwrite a phase that landed after its
        // read: the write only happens when the job id still matches.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("deploy-job.json");
        let first = DeployJob {
            state: DeployJobState::Succeeded,
            job_id: "job-1".into(),
            ..DeployJob::default()
        };
        locked_test_write(&path, &first);
        let demoted = DeployJob {
            state: DeployJobState::Failed,
            job_id: "job-1".into(),
            ..DeployJob::default()
        };
        assert!(compare_and_write_job(&path, "job-1", &demoted).expect("cas"));
        assert_eq!(read_job_file(&path).state, DeployJobState::Failed);

        let newer = DeployJob {
            state: DeployJobState::Running,
            job_id: "job-2".into(),
            ..DeployJob::default()
        };
        locked_test_write(&path, &newer);
        assert!(!compare_and_write_job(&path, "job-1", &demoted).expect("cas"));
        assert_eq!(read_job_file(&path).job_id, "job-2");
        assert_eq!(read_job_file(&path).state, DeployJobState::Running);
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
        locked_test_write(&path, &job);
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
