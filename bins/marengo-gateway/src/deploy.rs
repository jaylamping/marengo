//! Thin HTTP adapters for deploy status and self-update control.

use std::sync::OnceLock;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use marengo_deploy::{
    current_version_status, enqueue_self_update, fetch_upstream_sha, load_reconciled_job,
    new_job_id, shas_match, web_root_ready, DeployJobState,
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

    let (deploy, job) = load_reconciled_job();
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
    if !deploy.sha.is_empty() && shas_match(&deploy.sha, &upstream_sha) && web_root_ready() {
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

    #[tokio::test]
    async fn deploy_refuses_when_runtime_evidence_is_missing() {
        use crate::access::{AccessPolicy, Capability};

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
