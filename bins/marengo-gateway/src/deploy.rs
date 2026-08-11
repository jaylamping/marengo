//! Thin HTTP adapters for deploy status and self-update control.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use marengo_deploy::{
    current_version_status, enqueue_self_update, fetch_upstream_sha, load_reconciled_job,
    new_job_id, shas_match,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::restart::{authorize_restart, now_ms, refuse_active_fresh, HEARTBEAT_FRESH_MS};
use crate::state::SharedState;

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

    let mode = state.snapshot_safety().map(|snapshot| snapshot.mode);
    let heartbeat = state
        .snapshot_heartbeat()
        .map(|snapshot| snapshot.timestamp_ms);
    if refuse_active_fresh(mode, heartbeat, now_ms(), HEARTBEAT_FRESH_MS) {
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

    let (deploy, job) = load_reconciled_job();
    if job.state == marengo_deploy::DeployJobState::Running {
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
