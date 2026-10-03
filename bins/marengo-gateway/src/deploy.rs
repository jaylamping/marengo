//! Thin HTTP adapters for deploy status and self-update control.

use std::sync::OnceLock;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use marengo_deploy::{
    current_version_status, enqueue_self_update, fetch_upstream_sha, is_full_sha,
    load_reconciled_job, new_job_id, shas_match, web_root_ready, DeployJob, DeployJobState,
    JobFileRead,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::restart::{now_ms, refuse_unsafe_management_state, HEARTBEAT_FRESH_MS};
use crate::state::SharedState;

static DEPLOY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn deploy_lock() -> &'static Mutex<()> {
    DEPLOY_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Deserialize)]
pub struct VersionStatusQuery {
    #[serde(default)]
    pub refresh: Option<String>,
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

pub async fn get_version_status(
    Query(query): Query<VersionStatusQuery>,
) -> Result<Json<marengo_deploy::VersionStatus>, StatusCode> {
    let refresh = matches!(
        query.refresh.as_deref(),
        Some("1") | Some("true") | Some("yes")
    );
    Ok(Json(current_version_status(refresh).await))
}

pub async fn post_control_deploy(
    State(state): State<SharedState>,
    Json(body): Json<DeployRequestJson>,
) -> Result<(StatusCode, Json<DeployResponseJson>), StatusCode> {
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

    let Ok(_guard) = deploy_lock().try_lock() else {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message: "deploy already in progress".to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    };

    let mode = state.snapshot_safety().map(|snapshot| snapshot.mode);
    let heartbeat = state
        .snapshot_heartbeat()
        .map(|snapshot| snapshot.timestamp_ms);
    if refuse_unsafe_management_state(mode, heartbeat, now_ms(), HEARTBEAT_FRESH_MS) {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message: "update refused: require a known, fresh non-Active runtime state"
                    .to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    }
    if state.persist_degraded() {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message:
                    "config write-behind failed — resolve persist-degraded state before update"
                        .to_string(),
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

    // Ledger/systemctl I/O must not hold a Tokio worker: reconcile on the
    // blocking pool. A failed join still refuses fail-closed below.
    let (deploy, read) = tokio::task::spawn_blocking(load_reconciled_job)
        .await
        .unwrap_or_else(|_| marengo_deploy::empty_reconciled());
    let JobFileRead::Ok(job) = read else {
        return Ok((
            StatusCode::CONFLICT,
            Json(DeployResponseJson {
                ok: false,
                message: "corrupt deploy-job.json — refusing enqueue until the ledger is repaired"
                    .to_string(),
                already_current: None,
                job_id: None,
                target_sha: None,
            }),
        ));
    };
    // Corrupt ledgers never reach here as Running: `load_reconciled_job`
    // returns them as `Corrupt` above instead of coercing to a Failed
    // sentinel, so a torn write blocks enqueue fail-closed.
    let job: DeployJob = job;
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

    // Matching rev alone is not "current" if Consul www is missing — allow repair enqueue.
    // The installed rev must be a full SHA: a short prefix never counts as current.
    if is_full_sha(&deploy.sha) && shas_match(&deploy.sha, &upstream_sha) && web_root_ready() {
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

    let job_id = new_job_id(&upstream_sha);
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
        Err(error) => {
            warn!(%error, "self-update enqueue failed");
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeployResponseJson {
                    ok: false,
                    message: error.to_string(),
                    already_current: None,
                    job_id: None,
                    target_sha: Some(upstream_sha),
                }),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    const TOKEN: &str = "deploy-gate-test-token";

    /// Both deploy tests take the process-wide `DEPLOY_LOCK` via `try_lock`
    /// and mutate `MARENGO_DEPLOY_JOB_FILE`: run them one at a time.
    static TEST_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    struct JobFileEnv {
        prior: Option<String>,
    }

    impl JobFileEnv {
        fn point_at(path: &std::path::Path) -> Self {
            let prior = std::env::var("MARENGO_DEPLOY_JOB_FILE").ok();
            std::env::set_var("MARENGO_DEPLOY_JOB_FILE", path);
            Self { prior }
        }
    }

    impl Drop for JobFileEnv {
        fn drop(&mut self) {
            match self.prior.take() {
                Some(value) => std::env::set_var("MARENGO_DEPLOY_JOB_FILE", value),
                None => std::env::remove_var("MARENGO_DEPLOY_JOB_FILE"),
            }
        }
    }

    fn envelope_bytes(message_type: &str, payload: Vec<u8>) -> Vec<u8> {
        use armee_proto::prost::Message;
        armee_proto::Envelope {
            timestamp_ms: 1,
            source_node: "deploy-test".into(),
            message_type: message_type.into(),
            payload,
        }
        .encode_to_vec()
    }

    #[tokio::test]
    async fn deploy_refuses_enqueue_on_corrupt_ledger() {
        use crate::access::{AccessPolicy, Capability};
        use armee_proto::{Heartbeat, OperationalMode, SafetyState};

        let _serial = TEST_SERIAL.lock().await;
        let dir = tempfile::tempdir().expect("fixture");
        let job_path = dir.path().join("deploy-job.json");
        std::fs::write(&job_path, b"{ torn ledger").expect("corrupt fixture");
        let _env = JobFileEnv::point_at(&job_path);

        let state = std::sync::Arc::new(
            crate::state::AppState::new(std::sync::Arc::new(chappe::Bus::default())).with_access(
                AccessPolicy::role_fixture(TOKEN, Capability::Management)
                    .expect("management grant"),
            ),
        );
        {
            use armee_proto::prost::Message;
            let mut snap = state.snapshots.write().expect("snapshots");
            snap.safety_state = Some(envelope_bytes(
                "marengo.v1.SafetyState",
                SafetyState {
                    timestamp_ms: 1,
                    mode: OperationalMode::Disabled as i32,
                    hardware_estop_asserted: false,
                    software_estop_latched: false,
                    active_faults: vec![],
                }
                .encode_to_vec(),
            ));
            snap.heartbeat = Some(envelope_bytes(
                "marengo.v1.Heartbeat",
                Heartbeat {
                    timestamp_ms: crate::restart::now_ms(),
                    node_id: "deploy-test".to_string(),
                }
                .encode_to_vec(),
            ));
        }
        let app = crate::http::router(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/control/deploy")
                    .header("authorization", format!("Bearer {TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"confirm":true}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body");
        assert!(
            String::from_utf8_lossy(&body).contains("corrupt"),
            "corrupt ledger refuses enqueue: {body:?}"
        );
    }

    #[tokio::test]
    async fn deploy_refuses_when_runtime_evidence_is_missing() {
        use crate::access::{AccessPolicy, Capability};
        let _serial = TEST_SERIAL.lock().await;

        let state = std::sync::Arc::new(
            crate::state::AppState::new(std::sync::Arc::new(chappe::Bus::default())).with_access(
                AccessPolicy::role_fixture(TOKEN, Capability::Management)
                    .expect("management grant"),
            ),
        );
        let app = crate::http::router(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/control/deploy")
                    .header("authorization", format!("Bearer {TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"confirm":true}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
