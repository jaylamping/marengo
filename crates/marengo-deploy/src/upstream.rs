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
            // Negative cache: stamp the attempt so non-forced polls back off.
            // Prefer the last known-good SHA (memory, then disk) for display.
            let now = unix_now();
            let fallback = lock_cache()
                .sha
                .clone()
                .or_else(|| load_upstream_disk_cache().map(|(sha, _, _)| sha));
            persist_upstream_cache(fallback.as_deref().unwrap_or(""), now, false);
            let mut cache = lock_cache();
            if fallback.is_some() {
                cache.sha = fallback.clone();
            }
            cache.fetched_at_unix = now;
            cache.ok = false;
            (fallback.unwrap_or_default(), false, now)
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
    // Freshness covers failures too: a failed fetch stamps `fetched_at_unix`,
    // so sidebar polls back off for the TTL instead of re-running curl
    // serialized behind the fetch lock while GitHub is unreachable.
    // `fetched_at_unix == 0` (never attempted) is never fresh.
    cache.fetched_at_unix > 0
        && unix_now().saturating_sub(cache.fetched_at_unix) < UPSTREAM_CACHE_TTL_SECS
}

async fn curl_github_tip() -> Result<String, String> {
    let repo = github_repo();
    let git_ref = github_ref();
    let url = format!("https://api.github.com/repos/{repo}/commits/{git_ref}");
    // A `GITHUB_TOKEN` must never appear in argv (visible in
    // /proc/<pid>/cmdline): pass it via a 0600 `-K` config file instead.
    let token_config = token_config_file()?;
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
    if let Some(path) = token_config.as_ref().and_then(|cfg| cfg.path.as_ref()) {
        command.arg("-K").arg(path);
    }
    command.arg(url);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(20), command.output())
        .await
        .map_err(|_| "curl timed out".to_string())?
        .map_err(|error| format!("curl spawn: {error}"))?;
    drop(token_config);
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

/// Owner of a 0600 curl `-K` file carrying the GitHub bearer token.
/// Removed on drop so the token never lingers on disk or in argv.
struct TokenConfig {
    path: Option<PathBuf>,
}

impl Drop for TokenConfig {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn token_config_file() -> Result<Option<TokenConfig>, String> {
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let token = token.trim().to_string();
    if token.is_empty() {
        return Ok(None);
    }
    if token.contains(['\n', '\r']) {
        return Err("GITHUB_TOKEN contains a line break".to_string());
    }
    let mut path = std::env::temp_dir();
    path.push(format!("marengo-gh-{}.curlrc", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .map_err(|error| format!("token config create: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("token config chmod: {error}"))?;
    }
    {
        use std::io::Write;
        // Quoted inside the -K file so curl, not the shell, parses the value.
        let header = format!("header = \"Authorization: Bearer {token}\"\n");
        file.write_all(header.as_bytes())
            .map_err(|error| format!("token config write: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("token config fsync: {error}"))?;
    }
    Ok(Some(TokenConfig { path: Some(path) }))
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
    let ok = value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|value| value);
    (!sha.is_empty()).then_some((sha, fetched_at, ok))
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

        fn remove(key: &'static str) -> Self {
            let prior = std::env::var(key).ok();
            std::env::remove_var(key);
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

    struct CacheGuard {
        prior: UpstreamCache,
    }

    impl CacheGuard {
        fn save() -> Self {
            Self {
                prior: lock_cache().clone(),
            }
        }
    }

    impl Drop for CacheGuard {
        fn drop(&mut self) {
            *lock_cache() = std::mem::take(&mut self.prior);
        }
    }

    #[tokio::test]
    async fn failed_fetch_is_negatively_cached() {
        let _serial = TEST_LOCK.lock().await;
        let _cache = CacheGuard::save();
        let _override = EnvGuard::remove("MARENGO_UPSTREAM_SHA");
        // Seed a recent failure: no curl may run while it is fresh.
        let now = unix_now();
        {
            let mut cache = lock_cache();
            cache.sha = Some("abcdef0123456789abcdef0123456789abcdef01".to_string());
            cache.fetched_at_unix = now;
            cache.ok = false;
        }
        let (sha, ok, fetched_at) = fetch_upstream_sha(false).await;
        assert!(!ok, "negative cache reports failure, not success");
        assert_eq!(sha, "abcdef0123456789abcdef0123456789abcdef01");
        assert_eq!(fetched_at, now, "no refetch while the failure is fresh");
    }

    #[tokio::test]
    async fn fixed_override_still_wins_over_negative_cache() {
        let _serial = TEST_LOCK.lock().await;
        let _cache = CacheGuard::save();
        let _fixed = EnvGuard::set(
            "MARENGO_UPSTREAM_SHA",
            "1111111111111111111111111111111111111111",
        );
        {
            let mut cache = lock_cache();
            cache.sha = Some("abcdef0123456789abcdef0123456789abcdef01".to_string());
            cache.fetched_at_unix = unix_now();
            cache.ok = false;
        }
        let (sha, ok, _) = fetch_upstream_sha(false).await;
        assert!(ok);
        assert_eq!(sha, "1111111111111111111111111111111111111111");
    }

    #[test]
    fn token_config_file_never_touches_argv() {
        let _serial = TEST_LOCK.blocking_lock();
        let _token = EnvGuard::set("GITHUB_TOKEN", "test-token-abc123");
        let config = token_config_file().expect("token config");
        let path = config
            .as_ref()
            .and_then(|cfg| cfg.path.clone())
            .expect("token present");
        let body = std::fs::read_to_string(&path).expect("config readable");
        assert!(body.contains("test-token-abc123"), "token in -K file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "token file is owner-only");
        }
        drop(config);
        assert!(!path.exists(), "token file removed on drop");
    }

    #[test]
    fn token_with_line_break_is_refused() {
        let _serial = TEST_LOCK.blocking_lock();
        let _token = EnvGuard::set("GITHUB_TOKEN", "abc\ndef");
        assert!(token_config_file().is_err());
    }

    #[test]
    fn absent_token_needs_no_config_file() {
        let _serial = TEST_LOCK.blocking_lock();
        let _token = EnvGuard::remove("GITHUB_TOKEN");
        assert!(token_config_file().expect("no token").is_none());
    }
}
