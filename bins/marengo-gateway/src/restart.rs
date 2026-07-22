//! Restart marengo-pi so Davout reloads hard limits from motors.yaml.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use armee_proto::OperationalMode;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{info, warn};

use crate::logs::log_token_from_env;
use crate::state::SharedState;

/// Refuse Active restart when heartbeat is newer than this.
pub const HEARTBEAT_FRESH_MS: u64 = 5_000;
const RESTART_TIMEOUT: Duration = Duration::from_secs(60);

static RESTART_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn restart_lock() -> &'static Mutex<()> {
    RESTART_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Deserialize)]
pub struct RestartMarengoPiJson {
    pub confirm: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestartMarengoPiResultJson {
    pub ok: bool,
    pub message: String,
}

/// Fail closed when log token is unset (stricter than generic `authorize_logs`).
pub fn authorize_restart(headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = log_token_from_env() else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let provided = headers
        .get("x-marengo-log-token")
        .and_then(|v| v.to_str().ok());
    if provided == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Refuse when mode is Active and heartbeat timestamp is fresh.
pub fn refuse_active_fresh(
    mode: Option<i32>,
    heartbeat_ts_ms: Option<u64>,
    now: u64,
    fresh_ms: u64,
) -> bool {
    if mode != Some(OperationalMode::Active as i32) {
        return false;
    }
    let Some(ts) = heartbeat_ts_ms else {
        return false;
    };
    now.saturating_sub(ts) <= fresh_ms
}

pub fn resolve_restart_script() -> PathBuf {
    if let Ok(override_path) = std::env::var("MARENGO_RESTART_MARENGO_PI_SCRIPT") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let root = std::env::var("MARENGO_ROOT").unwrap_or_else(|_| "/opt/marengo".to_string());
    PathBuf::from(root).join("scripts/pi-restart-marengo-pi.sh")
}

pub async fn post_restart_marengo_pi(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<RestartMarengoPiJson>,
) -> Result<(StatusCode, Json<RestartMarengoPiResultJson>), StatusCode> {
    authorize_restart(&headers)?;

    if !body.confirm {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(RestartMarengoPiResultJson {
                ok: false,
                message: "confirm must be true".to_string(),
            }),
        ));
    }

    let mode = state.snapshot_safety().map(|s| s.mode);
    let hb_ts = state.snapshot_heartbeat().map(|h| h.timestamp_ms);
    if refuse_active_fresh(mode, hb_ts, now_ms(), HEARTBEAT_FRESH_MS) {
        warn!(
            ?mode,
            ?hb_ts,
            "restart refused: operational mode Active with fresh heartbeat"
        );
        return Ok((
            StatusCode::CONFLICT,
            Json(RestartMarengoPiResultJson {
                ok: false,
                message: "motors are ACTIVE — disable before restarting marengo-pi".to_string(),
            }),
        ));
    }

    let Ok(guard) = restart_lock().try_lock() else {
        return Ok((
            StatusCode::CONFLICT,
            Json(RestartMarengoPiResultJson {
                ok: false,
                message: "restart already in progress".to_string(),
            }),
        ));
    };

    let script = resolve_restart_script();
    if !script.is_file() {
        warn!(path = %script.display(), "restart script missing");
        drop(guard);
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RestartMarengoPiResultJson {
                ok: false,
                message: format!("restart script not found: {}", script.display()),
            }),
        ));
    }

    info!(path = %script.display(), "restarting marengo-pi via canonical script");
    let result = timeout(RESTART_TIMEOUT, run_restart_script(&script)).await;
    drop(guard);

    match result {
        Ok(Ok(output)) => {
            info!(ok = output.ok, "marengo-pi restart finished");
            Ok((StatusCode::OK, Json(output)))
        }
        Ok(Err(message)) => {
            warn!(%message, "marengo-pi restart failed");
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RestartMarengoPiResultJson { ok: false, message }),
            ))
        }
        Err(_) => {
            warn!("marengo-pi restart timed out");
            Ok((
                StatusCode::GATEWAY_TIMEOUT,
                Json(RestartMarengoPiResultJson {
                    ok: false,
                    message: "restart timed out".to_string(),
                }),
            ))
        }
    }
}

async fn run_restart_script(
    script: &std::path::Path,
) -> Result<RestartMarengoPiResultJson, String> {
    let skip_sudo = std::env::var("MARENGO_RESTART_SKIP_SUDO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut cmd = if skip_sudo {
        let mut c = Command::new(script);
        c.arg("restart");
        c
    } else {
        let mut c = Command::new("sudo");
        c.arg("-n").arg(script).arg("restart");
        c
    };

    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("spawn restart: {e}"))?;

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("wait restart: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let truncated: String = combined.chars().take(2_000).collect();

    if output.status.success() {
        Ok(RestartMarengoPiResultJson {
            ok: true,
            message: if truncated.trim().is_empty() {
                "marengo-pi restarted".to_string()
            } else {
                truncated
            },
        })
    } else {
        Err(if truncated.trim().is_empty() {
            format!("restart exited {}", output.status)
        } else {
            truncated
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use armee_proto::prost::Message;
    use armee_proto::{Envelope, Heartbeat, OperationalMode, SafetyState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::post;
    use axum::Router;
    use chappe::Bus;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::state::AppState;

    const TOKEN: &str = "restart-test-token";
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn envelope_bytes<M: armee_proto::prost::Message>(message_type: &str, msg: &M) -> Vec<u8> {
        let mut payload = Vec::new();
        msg.encode(&mut payload).expect("encode");
        Envelope {
            timestamp_ms: 1,
            source_node: "test".to_string(),
            message_type: message_type.to_string(),
            payload,
        }
        .encode_to_vec()
    }

    fn state_with_safety(mode: OperationalMode, hb_ts: Option<u64>) -> SharedState {
        let state = Arc::new(AppState::new(Arc::new(Bus::new(64))));
        {
            let mut snap = state.snapshots.write().expect("snapshots");
            snap.safety_state = Some(envelope_bytes(
                "marengo.v1.SafetyState",
                &SafetyState {
                    timestamp_ms: 1,
                    mode: mode as i32,
                    hardware_estop_asserted: false,
                    software_estop_latched: false,
                    active_faults: vec![],
                },
            ));
            if let Some(ts) = hb_ts {
                snap.heartbeat = Some(envelope_bytes(
                    "marengo.v1.Heartbeat",
                    &Heartbeat {
                        timestamp_ms: ts,
                        node_id: "test".to_string(),
                    },
                ));
            }
        }
        state
    }

    fn router(state: SharedState) -> Router {
        Router::new()
            .route("/control/restart-marengo-pi", post(post_restart_marengo_pi))
            .with_state(state)
    }

    #[test]
    fn refuse_active_only_when_heartbeat_fresh() {
        let now = 100_000;
        assert!(refuse_active_fresh(
            Some(OperationalMode::Active as i32),
            Some(now - 1_000),
            now,
            HEARTBEAT_FRESH_MS
        ));
        assert!(!refuse_active_fresh(
            Some(OperationalMode::Active as i32),
            Some(now - 10_000),
            now,
            HEARTBEAT_FRESH_MS
        ));
        assert!(!refuse_active_fresh(
            Some(OperationalMode::Active as i32),
            None,
            now,
            HEARTBEAT_FRESH_MS
        ));
        assert!(!refuse_active_fresh(
            Some(OperationalMode::Ready as i32),
            Some(now),
            now,
            HEARTBEAT_FRESH_MS
        ));
    }

    #[tokio::test]
    async fn restart_fails_closed_without_token() {
        let _env = ENV_LOCK.lock().await;
        std::env::remove_var("MARENGO_GATEWAY_LOG_TOKEN");
        std::env::remove_var("MARENGO_RESTART_SKIP_SUDO");
        std::env::remove_var("MARENGO_RESTART_MARENGO_PI_SCRIPT");
        let state = state_with_safety(OperationalMode::Disabled, None);
        let app = router(state);
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/control/restart-marengo-pi")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"confirm":true}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn restart_refuses_active_with_fresh_heartbeat() {
        let _env = ENV_LOCK.lock().await;
        std::env::set_var("MARENGO_GATEWAY_LOG_TOKEN", TOKEN);
        let now = now_ms();
        let state = state_with_safety(OperationalMode::Active, Some(now));
        let app = router(state);
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/control/restart-marengo-pi")
                    .header("content-type", "application/json")
                    .header("x-marengo-log-token", TOKEN)
                    .body(Body::from(r#"{"confirm":true}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::CONFLICT);
        std::env::remove_var("MARENGO_GATEWAY_LOG_TOKEN");
    }

    #[tokio::test]
    async fn restart_allows_active_with_stale_heartbeat_via_stub() {
        let _env = ENV_LOCK.lock().await;
        std::env::set_var("MARENGO_GATEWAY_LOG_TOKEN", TOKEN);
        let dir = tempfile::tempdir().expect("tempdir");
        let stub_path = dir.path().join("restart-stub.sh");
        std::fs::write(&stub_path, "#!/bin/sh\necho RESTART_OK\nexit 0\n").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&stub_path).expect("meta").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&stub_path, perms).expect("chmod");
        }
        std::env::set_var("MARENGO_RESTART_MARENGO_PI_SCRIPT", &stub_path);
        std::env::set_var("MARENGO_RESTART_SKIP_SUDO", "1");

        let now = now_ms();
        let state = state_with_safety(OperationalMode::Active, Some(now.saturating_sub(30_000)));
        let app = router(state);
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/control/restart-marengo-pi")
                    .header("content-type", "application/json")
                    .header("x-marengo-log-token", TOKEN)
                    .body(Body::from(r#"{"confirm":true}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .expect("body");
        assert_eq!(
            status,
            StatusCode::OK,
            "body={}",
            String::from_utf8_lossy(&body)
        );
        let parsed: RestartMarengoPiResultJson = serde_json::from_slice(&body).expect("json");
        assert!(parsed.ok);

        std::env::remove_var("MARENGO_GATEWAY_LOG_TOKEN");
        std::env::remove_var("MARENGO_RESTART_MARENGO_PI_SCRIPT");
        std::env::remove_var("MARENGO_RESTART_SKIP_SUDO");
    }

    #[tokio::test]
    async fn restart_requires_confirm() {
        let _env = ENV_LOCK.lock().await;
        std::env::set_var("MARENGO_GATEWAY_LOG_TOKEN", TOKEN);
        let state = state_with_safety(OperationalMode::Disabled, None);
        let app = router(state);
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/control/restart-marengo-pi")
                    .header("content-type", "application/json")
                    .header("x-marengo-log-token", TOKEN)
                    .body(Body::from(r#"{"confirm":false}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        std::env::remove_var("MARENGO_GATEWAY_LOG_TOKEN");
    }
}
