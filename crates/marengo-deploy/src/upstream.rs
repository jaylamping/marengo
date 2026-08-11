use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;
use tracing::warn;

use crate::job::load_reconciled_job;
use crate::paths::resolve_upstream_cache_path;

/// In-memory GitHub tip cache TTL.
pub const UPSTREAM_CACHE_TTL_SECS: u64 = 60;

#[derive(Debug, Default, Clone)]
struct UpstreamCache {
    sha: Option<String>,
    fetched_at_unix: u64,
    ok: bool,
}

static UPSTREAM_CACHE: OnceLock<Mutex<UpstreamCache>> = OnceLock::new();
static UPSTREAM_FETCH_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

fn upstream_cache() -> &'static Mutex<UpstreamCache> {
    UPSTREAM_CACHE.get_or_init(|| Mutex::new(UpstreamCache::default()))
}

fn upstream_fetch_lock() -> &'static AsyncMutex<()> {
    UPSTREAM_FETCH_LOCK.get_or_init(|| AsyncMutex::new(()))
}

fn lock_cache() -> std::sync::MutexGuard<'static, UpstreamCache> {
    match upstream_cache().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn github_repo() -> String {
    match std::env::var("MARENGO_GITHUB_REPO") {
        Ok(value) => value,
        Err(_) => "jaylamping/marengo".to_string(),
    }
}

fn github_ref() -> String {
    match std::env::var("MARENGO_GITHUB_REF") {
        Ok(value) => value,
        Err(_) => "main".to_string(),
    }
}

fn unix_now() -> u64 {
    crate::job::unix_now()
}

/// Load the disk cache and reconcile a persisted running job at process start.
///
/// The reconciliation side effect preserves the gateway's previous boot behavior;
/// the cache itself remains private to this module.
pub fn init_upstream_cache_from_disk() {
    if let Some((sha, fetched_at, disk_ok)) = load_upstream_disk_cache() {
        let mut cache = lock_cache();
        cache.sha = Some(sha);
        cache.fetched_at_unix = fetched_at;
        cache.ok = disk_ok;
    }
    let _ = load_reconciled_job();
}

/// Fetch the configured GitHub tip, using the TTL cache unless `force` is true.
pub async fn fetch_upstream_sha(force: bool) -> (String, bool, u64) {
    if let Ok(fixed) = std::env::var("MARENGO_UPSTREAM_SHA") {
        let sha = fixed.trim().to_string();
        if !sha.is_empty() {
            let now = unix_now();
            let mut cache = lock_cache();
            cache.sha = Some(sha.clone());
            cache.fetched_at_unix = now;
            cache.ok = true;
            return (sha, true, now);
        }
    }

    {
        let cache = lock_cache();
        if !force && cache_is_fresh(&cache) {
            return cached_result(&cache);
        }
    }

    let _fetch = upstream_fetch_lock().lock().await;

    {
        let cache = lock_cache();
        if !force && cache_is_fresh(&cache) {
            return cached_result(&cache);
        }
    }

    match curl_github_tip().await {
        Ok(sha) => {
            let now = unix_now();
            persist_upstream_cache(&sha, now, true);
            let mut cache = lock_cache();
            cache.sha = Some(sha.clone());
            cache.fetched_at_unix = now;
            cache.ok = true;
            (sha, true, now)
        }
        Err(error) => {
            warn!(%error, "GitHub upstream fetch failed");
            if let Some((sha, fetched_at, _disk_ok)) = load_upstream_disk_cache() {
                let mut cache = lock_cache();
                cache.sha = Some(sha.clone());
                cache.fetched_at_unix = fetched_at;
                cache.ok = false;
                return (sha, false, fetched_at);
            }
            let cache = lock_cache();
            (cached_sha(&cache), false, cache.fetched_at_unix)
        }
    }
}

fn cached_result(cache: &UpstreamCache) -> (String, bool, u64) {
    (cached_sha(cache), cache.ok, cache.fetched_at_unix)
}

fn cached_sha(cache: &UpstreamCache) -> String {
    match &cache.sha {
        Some(sha) => sha.clone(),
        None => String::new(),
    }
}

fn cache_is_fresh(cache: &UpstreamCache) -> bool {
    cache.sha.is_some()
        && unix_now().saturating_sub(cache.fetched_at_unix) < UPSTREAM_CACHE_TTL_SECS
}

async fn curl_github_tip() -> Result<String, String> {
    let repo = github_repo();
    let git_ref = github_ref();
    let url = format!("https://api.github.com/repos/{repo}/commits/{git_ref}");
    let mut command = Command::new("curl");
    command.args([
        "-fsSL",
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "User-Agent: marengo-gateway",
        "--max-time",
        "15",
    ]);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        let token = token.trim();
        if !token.is_empty() {
            command.args(["-H", &format!("Authorization: Bearer {token}")]);
        }
    }
    command.arg(url);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = tokio::time::timeout(Duration::from_secs(20), command.output())
        .await
        .map_err(|_| "curl timed out".to_string())?
        .map_err(|error| format!("curl spawn: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| format!("github json: {error}"))?;
    let sha = value
        .get("sha")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "github response missing sha".to_string())?
        .to_string();
    if sha.len() < 7 {
        return Err("github sha too short".to_string());
    }
    Ok(sha)
}

fn persist_upstream_cache(sha: &str, fetched_at: u64, ok: bool) {
    let path = resolve_upstream_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = serde_json::json!({ "sha": sha, "fetched_at_unix": fetched_at, "ok": ok });
    let _ = std::fs::write(path, body.to_string());
}

fn load_upstream_disk_cache() -> Option<(String, u64, bool)> {
    let path: PathBuf = resolve_upstream_cache_path();
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let sha = value.get("sha")?.as_str()?.to_string();
    let fetched_at = value.get("fetched_at_unix")?.as_u64()?;
    let ok = match value.get("ok").and_then(serde_json::Value::as_bool) {
        Some(value) => value,
        None => false,
    };
    (!sha.is_empty()).then_some((sha, fetched_at, ok))
}
