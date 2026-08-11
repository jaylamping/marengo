//! Version status + Pi self-update enqueue (Consul sidebar Update).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

use crate::restart::{authorize_restart, now_ms, refuse_active_fresh, HEARTBEAT_FRESH_MS};
use crate::state::SharedState;

/// Max age for a `running` job before boot/GET reconcile marks it failed.
pub const DEPLOY_JOB_MAX_AGE_SECS: u64 = 30 * 60;
/// In-memory GitHub tip cache TTL.
pub const UPSTREAM_CACHE_TTL_SECS: u64 = 60;

static UPSTREAM_CACHE: OnceLock<Mutex<UpstreamCache>> = OnceLock::new();
static UPSTREAM_FETCH_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

fn upstream_cache() -> &'static Mutex<UpstreamCache> {
    UPSTREAM_CACHE.get_or_init(|| Mutex::new(UpstreamCache::default()))
}

fn upstream_fetch_lock() -> &'static AsyncMutex<()> {
    UPSTREAM_FETCH_LOCK.get_or_init(|| AsyncMutex::new(()))
}

#[derive(Debug, Default, Clone)]
struct UpstreamCache {
    sha: Option<String>,
    fetched_at_unix: u64,
    ok: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub phase: String,
}

#[derive(Debug, Deserialize)]
pub struct VersionStatusQuery {
    #[serde(default)]
    pub refresh: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VersionStatusJson {
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
}

#[derive(Debug, Deserialize)]
pub struct DeployRequestJson {
    pub confirm: bool,
}

#[derive(Debug, Serialize)]
pub struct DeployResponseJson {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_current: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDeployRev {
    pub sha: String,
    pub deployed_at: Option<String>,
}

/// Parse `/opt/marengo/.deploy-rev` (`SHA` or `SHA ISO8601`).
pub fn parse_deploy_rev(raw: &str) -> ParsedDeployRev {
    let cleaned = raw
        .trim()
        .replace("\\n", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut parts = cleaned.split(' ');
    let rev = parts.next().unwrap_or("").to_string();
    if rev.len() >= 7 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
        let rest: Vec<&str> = parts.collect();
        return ParsedDeployRev {
            sha: rev,
            deployed_at: if rest.is_empty() {
                None
            } else {
                Some(rest.join(" "))
            },
        };
    }
    ParsedDeployRev {
        sha: cleaned,
        deployed_at: None,
    }
}

/// True when installed SHA matches upstream (full or prefix).
pub fn shas_match(installed: &str, upstream: &str) -> bool {
    let a = installed.trim().to_ascii_lowercase();
    let b = upstream.trim().to_ascii_lowercase();
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a.len() >= 7 && b.len() >= 7 {
        return a == b || a.starts_with(&b) || b.starts_with(&a);
    }
    a == b
}

pub fn resolve_deploy_rev_path() -> PathBuf {
    if let Ok(p) = std::env::var("MARENGO_DEPLOY_REV_PATH") {
        let t = p.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join(".deploy-rev")
}

pub fn resolve_job_file_path() -> PathBuf {
    if let Ok(p) = std::env::var("MARENGO_DEPLOY_JOB_FILE") {
        let t = p.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join("var/deploy-job.json")
}

pub fn resolve_enqueue_script() -> PathBuf {
    if let Ok(p) = std::env::var("MARENGO_SELF_UPDATE_ENQUEUE_CMD") {
        let t = p.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join("scripts/pi-enqueue-self-update.sh")
}

pub fn resolve_upstream_cache_path() -> PathBuf {
    if let Ok(p) = std::env::var("MARENGO_UPSTREAM_CACHE_PATH") {
        let t = p.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join("var/upstream-sha.json")
}

pub fn resolve_self_update_log_path() -> PathBuf {
    if let Ok(p) = std::env::var("MARENGO_SELF_UPDATE_LOG") {
        let t = p.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join("var/self-update.log")
}

pub fn read_deploy_rev(path: &Path) -> ParsedDeployRev {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_deploy_rev(&raw),
        Err(_) => ParsedDeployRev {
            sha: String::new(),
            deployed_at: None,
        },
    }
}

pub fn read_job_file(path: &Path) -> DeployJob {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return DeployJob::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn write_job_file(path: &Path, job: &DeployJob) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir job dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(job).map_err(|e| format!("serialize job: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| format!("write job tmp: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename job: {e}"))
}

fn parse_iso_unix(iso: &str) -> Option<u64> {
    // Accept `YYYY-MM-DDTHH:MM:SSZ` only (what our scripts write).
    let trimmed = iso.trim().trim_end_matches('Z');
    let (date, clock) = trimmed.split_once('T')?;
    let mut d = date.split('-');
    let y: i32 = d.next()?.parse().ok()?;
    let m: u8 = d.next()?.parse().ok()?;
    let day: u8 = d.next()?.parse().ok()?;
    let mut t = clock.split(':');
    let hh: u8 = t.next()?.parse().ok()?;
    let mm: u8 = t.next()?.parse().ok()?;
    let ss: u8 = t.next()?.parse().ok()?;
    let month = time::Month::try_from(m).ok()?;
    let date = time::Date::from_calendar_date(y, month, day).ok()?;
    let tod = time::Time::from_hms(hh, mm, ss).ok()?;
    let dt = time::PrimitiveDateTime::new(date, tod).assume_utc();
    Some(dt.unix_timestamp().max(0) as u64)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Reconcile stuck `running` jobs (dead unit / max age / deploy-rev already matches).
pub fn reconcile_job(job: &mut DeployJob, deploy_sha: &str, max_age_secs: u64) -> bool {
    if job.state != DeployJobState::Running {
        return false;
    }
    if !job.target_sha.is_empty() && shas_match(deploy_sha, &job.target_sha) {
        job.state = DeployJobState::Succeeded;
        job.result_sha = deploy_sha.to_string();
        job.message = "reconciled: deploy-rev matches target".to_string();
        job.phase = "done".to_string();
        job.updated_at = format_unix_iso(unix_now());
        return true;
    }
    let started = parse_iso_unix(&job.started_at).unwrap_or(0);
    if started > 0 && unix_now().saturating_sub(started) > max_age_secs {
        job.state = DeployJobState::Failed;
        job.message = format!("stale running job older than {max_age_secs}s");
        job.phase = "timeout".to_string();
        job.updated_at = format_unix_iso(unix_now());
        return true;
    }
    if !job.unit_name.is_empty() && !unit_is_active(&job.unit_name) {
        // Unit finished but job file not updated (killed mid-write) — wait for age unless rev matches.
        if started > 0 && unix_now().saturating_sub(started) > 120 {
            job.state = DeployJobState::Failed;
            job.message = format!("unit {} inactive without success", job.unit_name);
            job.phase = "orphan".to_string();
            job.updated_at = format_unix_iso(unix_now());
            return true;
        }
    }
    false
}

fn unit_is_active(unit: &str) -> bool {
    let name = if unit.ends_with(".service") {
        unit.to_string()
    } else {
        format!("{unit}.service")
    };
    std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", &name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn format_unix_iso(secs: u64) -> String {
    let Ok(dt) = time::OffsetDateTime::from_unix_timestamp(secs as i64) else {
        return String::new();
    };
    dt.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
        .replace("+00:00", "Z")
}

fn github_repo() -> String {
    std::env::var("MARENGO_GITHUB_REPO").unwrap_or_else(|_| "jaylamping/marengo".to_string())
}

fn github_ref() -> String {
    std::env::var("MARENGO_GITHUB_REF").unwrap_or_else(|_| "main".to_string())
}

async fn fetch_upstream_sha(force: bool) -> (String, bool, u64) {
    if let Ok(fixed) = std::env::var("MARENGO_UPSTREAM_SHA") {
        let t = fixed.trim().to_string();
        if !t.is_empty() {
            let now = unix_now();
            if let Ok(mut c) = upstream_cache().lock() {
                c.sha = Some(t.clone());
                c.fetched_at_unix = now;
                c.ok = true;
            }
            return (t, true, now);
        }
    }

    {
        let guard = upstream_cache().lock().unwrap_or_else(|e| e.into_inner());
        if !force {
            if let Some(sha) = &guard.sha {
                if unix_now().saturating_sub(guard.fetched_at_unix) < UPSTREAM_CACHE_TTL_SECS {
                    return (sha.clone(), guard.ok, guard.fetched_at_unix);
                }
            }
        }
    }

    let _fetch = upstream_fetch_lock().lock().await;

    {
        let guard = upstream_cache().lock().unwrap_or_else(|e| e.into_inner());
        if !force {
            if let Some(sha) = &guard.sha {
                if unix_now().saturating_sub(guard.fetched_at_unix) < UPSTREAM_CACHE_TTL_SECS {
                    return (sha.clone(), guard.ok, guard.fetched_at_unix);
                }
            }
        }
    }

    match curl_github_tip().await {
        Ok(sha) => {
            let now = unix_now();
            persist_upstream_cache(&sha, now, true);
            if let Ok(mut c) = upstream_cache().lock() {
                c.sha = Some(sha.clone());
                c.fetched_at_unix = now;
                c.ok = true;
            }
            (sha, true, now)
        }
        Err(err) => {
            warn!(%err, "GitHub upstream fetch failed");
            // Fall back to disk cache / memory.
            if let Some((sha, at, _ok)) = load_upstream_disk_cache() {
                if let Ok(mut c) = upstream_cache().lock() {
                    c.sha = Some(sha.clone());
                    c.fetched_at_unix = at;
                    c.ok = false;
                }
                return (sha, false, at);
            }
            let guard = upstream_cache().lock().unwrap_or_else(|e| e.into_inner());
            (
                guard.sha.clone().unwrap_or_default(),
                false,
                guard.fetched_at_unix,
            )
        }
    }
}

async fn curl_github_tip() -> Result<String, String> {
    let repo = github_repo();
    let git_ref = github_ref();
    let url = format!("https://api.github.com/repos/{repo}/commits/{git_ref}");
    let mut cmd = Command::new("curl");
    cmd.args([
        "-fsSL",
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "User-Agent: marengo-gateway",
        "--max-time",
        "15",
    ]);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        let t = token.trim();
        if !t.is_empty() {
            cmd.args(["-H", &format!("Authorization: Bearer {t}")]);
        }
    }
    cmd.arg(&url);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = cmd.output().await.map_err(|e| format!("curl spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("github json: {e}"))?;
    let sha = v
        .get("sha")
        .and_then(|s| s.as_str())
        .ok_or_else(|| "github response missing sha".to_string())?
        .to_string();
    if sha.len() < 7 {
        return Err("github sha too short".to_string());
    }
    Ok(sha)
}

fn persist_upstream_cache(sha: &str, at: u64, ok: bool) {
    let path = resolve_upstream_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = serde_json::json!({ "sha": sha, "fetched_at_unix": at, "ok": ok });
    let _ = std::fs::write(path, body.to_string());
}

fn load_upstream_disk_cache() -> Option<(String, u64, bool)> {
    let raw = std::fs::read_to_string(resolve_upstream_cache_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let sha = v.get("sha")?.as_str()?.to_string();
    let at = v.get("fetched_at_unix")?.as_u64()?;
    let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    if sha.is_empty() {
        return None;
    }
    Some((sha, at, ok))
}

fn www_index_present() -> bool {
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join("www/index.html").is_file()
}

fn ready_for_target(deploy_sha: &str, target: &str, job: &DeployJob) -> bool {
    if target.is_empty() || !shas_match(deploy_sha, target) {
        return false;
    }
    if job.state == DeployJobState::Succeeded && shas_match(&job.target_sha, target) {
        return www_index_present()
            || std::env::var("MARENGO_SKIP_WWW_READY").ok().as_deref() == Some("1");
    }
    // Also true when deploy-rev matches even if job already idle after success.
    www_index_present() || std::env::var("MARENGO_SKIP_WWW_READY").ok().as_deref() == Some("1")
}

fn log_tail(path: &Path, max_bytes: usize) -> Option<String> {
    let raw = std::fs::read(path).ok()?;
    if raw.is_empty() {
        return None;
    }
    let start = raw.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&raw[start..]).into_owned().into()
}

pub async fn get_version_status(
    Query(query): Query<VersionStatusQuery>,
) -> Result<Json<VersionStatusJson>, StatusCode> {
    let refresh = matches!(
        query.refresh.as_deref(),
        Some("1") | Some("true") | Some("yes")
    );
    let deploy = read_deploy_rev(&resolve_deploy_rev_path());
    let job_path = resolve_job_file_path();
    let mut job = read_job_file(&job_path);
    if reconcile_job(&mut job, &deploy.sha, DEPLOY_JOB_MAX_AGE_SECS) {
        let _ = write_job_file(&job_path, &job);
    }

    let (upstream_sha, upstream_ok, fetched_at) = fetch_upstream_sha(refresh).await;
    let update_available =
        upstream_ok && !deploy.sha.is_empty() && !shas_match(&deploy.sha, &upstream_sha);
    let target_for_ready = if job.target_sha.is_empty() {
        upstream_sha.as_str()
    } else {
        job.target_sha.as_str()
    };
    let ready = ready_for_target(&deploy.sha, target_for_ready, &job)
        && job.state == DeployJobState::Succeeded;

    let log_tail = if job.state == DeployJobState::Failed {
        log_tail(&resolve_self_update_log_path(), 4000)
    } else {
        None
    };

    Ok(Json(VersionStatusJson {
        deploy_sha: deploy.sha,
        deployed_at: deploy.deployed_at,
        upstream_sha,
        upstream_fetched_at: if fetched_at > 0 {
            Some(format_unix_iso(fetched_at))
        } else {
            None
        },
        upstream_ok,
        update_available,
        ready_for_target: ready,
        deploy: job,
        log_tail,
    }))
}

pub async fn post_control_deploy(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<DeployRequestJson>,
) -> Result<(StatusCode, Json<DeployResponseJson>), StatusCode> {
    authorize_restart(&headers)?;

    if !body.confirm {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(DeployResponseJson {
                ok: false,
                message: "confirm must be true".to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    }

    let mode = state.snapshot_safety().map(|s| s.mode);
    let hb_ts = state.snapshot_heartbeat().map(|h| h.timestamp_ms);
    if refuse_active_fresh(mode, hb_ts, now_ms(), HEARTBEAT_FRESH_MS) {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message: "motors are ACTIVE — disable before updating".to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    }

    if state.persist_pending() {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message: "config write-behind still pending — wait for durable ACK before update"
                    .to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    }

    let job_path = resolve_job_file_path();
    let deploy = read_deploy_rev(&resolve_deploy_rev_path());
    let mut job = read_job_file(&job_path);
    if reconcile_job(&mut job, &deploy.sha, DEPLOY_JOB_MAX_AGE_SECS) {
        let _ = write_job_file(&job_path, &job);
    }
    if job.state == DeployJobState::Running {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message: format!("deploy already in progress ({})", job.job_id),
                already_current: None,
                job_id: Some(job.job_id),
                target_sha: Some(job.target_sha),
            }),
        ));
    }

    let (upstream_sha, upstream_ok, _) = fetch_upstream_sha(true).await;
    if !upstream_ok || upstream_sha.is_empty() {
        return Ok((
            StatusCode::BAD_GATEWAY,
            Json(DeployResponseJson {
                ok: false,
                message: "could not fetch GitHub tip of main".to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    }

    if !deploy.sha.is_empty() && shas_match(&deploy.sha, &upstream_sha) {
        return Ok((
            StatusCode::OK,
            Json(DeployResponseJson {
                ok: true,
                message: "Already up to date".to_string(),
                already_current: Some(true),
                job_id: None,
                target_sha: Some(upstream_sha),
            }),
        ));
    }

    let job_id = format!("{}-{}", format_unix_iso(unix_now()).replace(':', ""), {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        upstream_sha.hash(&mut h);
        unix_now().hash(&mut h);
        format!("{:x}", h.finish() & 0xffff)
    });

    match enqueue_self_update(&upstream_sha, &job_id).await {
        Ok(()) => {
            info!(%job_id, target = %upstream_sha, "self-update enqueued");
            Ok((
                StatusCode::ACCEPTED,
                Json(DeployResponseJson {
                    ok: true,
                    message: "self-update enqueued".to_string(),
                    already_current: Some(false),
                    job_id: Some(job_id),
                    target_sha: Some(upstream_sha),
                }),
            ))
        }
        Err(message) => {
            warn!(%message, "self-update enqueue failed");
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeployResponseJson {
                    ok: false,
                    message,
                    already_current: None,
                    job_id: None,
                    target_sha: Some(upstream_sha),
                }),
            ))
        }
    }
}

async fn enqueue_self_update(target_sha: &str, job_id: &str) -> Result<(), String> {
    let script = resolve_enqueue_script();
    if !script.is_file() {
        return Err(format!("enqueue script not found: {}", script.display()));
    }

    let skip_sudo = std::env::var("MARENGO_SELF_UPDATE_SKIP_SUDO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut cmd = if skip_sudo {
        let mut c = Command::new(&script);
        c.arg(target_sha).arg(job_id);
        c
    } else {
        let mut c = Command::new("sudo");
        c.arg("-n").arg(&script).arg(target_sha).arg(job_id);
        c
    };
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = tokio::time::timeout(Duration::from_secs(30), cmd.output())
        .await
        .map_err(|_| "enqueue timed out".to_string())?
        .map_err(|e| format!("enqueue spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "enqueue failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .chain(String::from_utf8_lossy(&output.stdout).chars())
                .take(500)
                .collect::<String>()
        ));
    }
    Ok(())
}

/// Load disk upstream cache into memory on process start.
pub fn init_upstream_cache_from_disk() {
    if let Some((sha, at, disk_ok)) = load_upstream_disk_cache() {
        if let Ok(mut c) = upstream_cache().lock() {
            c.sha = Some(sha);
            c.fetched_at_unix = at;
            c.ok = disk_ok;
        }
    }
    let job_path = resolve_job_file_path();
    let deploy = read_deploy_rev(&resolve_deploy_rev_path());
    let mut job = read_job_file(&job_path);
    if reconcile_job(&mut job, &deploy.sha, DEPLOY_JOB_MAX_AGE_SECS) {
        let _ = write_job_file(&job_path, &job);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_deploy_rev_sha_and_timestamp() {
        let p = parse_deploy_rev("abcdef0123456789 2026-08-11T12:00:00Z\n");
        assert_eq!(p.sha, "abcdef0123456789");
        assert_eq!(p.deployed_at.as_deref(), Some("2026-08-11T12:00:00Z"));
    }

    #[test]
    fn shas_match_prefix_and_full() {
        assert!(shas_match(
            "abcdef0123456789abcdef0123456789abcdef01",
            "abcdef0"
        ));
        assert!(shas_match(
            "abcdef0",
            "abcdef0123456789abcdef0123456789abcdef01"
        ));
        assert!(!shas_match("aaaaaaa", "bbbbbbb"));
        assert!(!shas_match("", "abcdef0"));
    }

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
    }

    #[test]
    fn job_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("deploy-job.json");
        let job = DeployJob {
            state: DeployJobState::Running,
            job_id: "j1".into(),
            target_sha: "abc".into(),
            message: "hi".into(),
            ..DeployJob::default()
        };
        write_job_file(&path, &job).unwrap();
        let loaded = read_job_file(&path);
        assert_eq!(loaded.job_id, "j1");
        assert_eq!(loaded.state, DeployJobState::Running);
    }

    #[test]
    fn read_deploy_rev_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".deploy-rev");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "abc1234 2026-08-11T01:02:03Z").unwrap();
        let p = read_deploy_rev(&path);
        assert_eq!(p.sha, "abc1234");
    }
}
