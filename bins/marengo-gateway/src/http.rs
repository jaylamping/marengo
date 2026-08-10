use std::path::Path;

use armee_proto::prost::Message;
use armee_proto::ActiveReportingLeaseRequest;
use armee_proto::EnableRequest;
use armee_proto::MitCommandBatch;
use armee_proto::SetZeroRequest;
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::actuator;
use crate::config;
use crate::framing::{self, CHAPPE_STREAM_CONTENT_TYPE};
use crate::hardware;
use crate::logs;
use crate::restart;
use crate::state::{filter_topics, SharedState};

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    node: &'static str,
    /// Structured log inserts dropped due to DB-writer backpressure (0 in healthy operation).
    dropped_log_inserts: u64,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

#[derive(Serialize)]
struct TlsFingerprintEntry {
    algorithm: &'static str,
    value: String,
}

#[derive(Serialize)]
struct TlsFingerprintResponse {
    algorithm: &'static str,
    value: String,
    hashes: Vec<TlsFingerprintEntry>,
}

#[derive(Deserialize)]
struct StreamQuery {
    topics: String,
}

/// API routes plus optional Consul SPA static files (`web_root` for robot-hosted HTTPS).
///
/// CORS is intentionally permissive for Phase-1 LAN bench (ADR 0008): auth/mTLS is
/// deferred. Motion commands still require confirm/attestation + a global-per-joint
/// Motion rate limit that cannot be bypassed by rotating `client_id`.
pub fn router(state: SharedState, web_root: Option<&Path>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_private_network(tower_http::cors::AllowPrivateNetwork::yes());

    let api = Router::new()
        .route("/health", get(health))
        .route("/tls/fingerprint", get(tls_fingerprint))
        .route("/stream/chappe", get(stream_chappe))
        .route("/snapshot/robot/state", get(snapshot_state))
        .route("/snapshot/robot/safety", get(snapshot_safety))
        .route("/snapshot/robot/heartbeat", get(snapshot_heartbeat))
        .route("/snapshot/sensors/imu/torso", get(snapshot_imu_torso))
        .route("/snapshot/host/metrics/pi", get(snapshot_host_metrics_pi))
        .route(
            "/snapshot/host/metrics/jetson",
            get(snapshot_host_metrics_jetson),
        )
        .route(
            "/snapshot/actuator/limits",
            get(actuator::snapshot_actuator_limits),
        )
        .route("/snapshot/logs/recent", get(logs::snapshot_logs_recent))
        .route("/logs/sessions", get(logs::list_sessions))
        .route("/logs/sessions/latest/candump", get(logs::latest_candump))
        .route(
            "/logs/sessions/latest/candump/summary",
            get(logs::latest_candump_summary),
        )
        .route("/logs/sessions/{id}/bench", get(logs::session_bench))
        .route("/logs/sessions/{id}/trace", get(logs::session_trace))
        .route("/logs/sessions/{id}/candump", get(logs::session_candump))
        .route(
            "/logs/sessions/{id}/candump/summary",
            get(logs::session_candump_summary),
        )
        .route("/logs/sessions/{id}/download", get(logs::session_download))
        .route("/logs/structured", get(logs::structured_logs))
        .route("/settings", get(logs::get_settings))
        .route("/config/snapshot", get(config::get_config_snapshot))
        .route("/config/patch", post(config::post_config_patch))
        .route("/hardware/completeness", get(hardware::get_completeness))
        .route("/hardware/urdf", get(hardware::get_urdf))
        .route("/hardware/urdf/upload", post(hardware::post_urdf_upload))
        .route(
            "/hardware/urdf/resolve-preview",
            post(hardware::post_resolve_preview),
        )
        .route("/hardware/urdf/activate", post(hardware::post_activate))
        .route("/hardware/urdf/archive", get(hardware::get_archive_list))
        .route(
            "/hardware/urdf/archive/{id}",
            get(hardware::get_archive_fetch),
        )
        .route(
            "/hardware/urdf/archive/{id}/restore",
            post(hardware::post_archive_restore),
        )
        .route(
            "/hardware/commissioning-scope",
            get(hardware::get_commissioning_scope)
                .put(hardware::put_commissioning_scope)
                .delete(hardware::delete_commissioning_scope),
        )
        .route(
            "/control/restart-marengo-pi",
            post(restart::post_restart_marengo_pi),
        )
        .route("/command/enable", post(command_enable))
        .route("/command/testing_mit", post(command_testing_mit))
        // Retired: operator HomingComplete / Testing Home — use Hardware Set Zero.
        .route("/command/home", post(command_home_retired))
        .route("/command/set_zero", post(command_set_zero))
        .route(
            "/command/active_reporting_lease",
            post(command_active_reporting_lease),
        )
        .route("/command/actuator", post(actuator::command_actuator))
        .layer(cors)
        .with_state(state);

    match web_root {
        Some(root) => {
            let index = root.join("index.html");
            let assets = root.join("assets");
            let mut router = api;
            if assets.is_dir() {
                router = router.nest_service("/assets", ServeDir::new(assets));
            }
            router.fallback_service(ServeFile::new(index))
        }
        None => api,
    }
}

async fn health(State(state): State<SharedState>) -> Json<HealthResponse> {
    let dropped_log_inserts = state
        .logs
        .as_ref()
        .map(|l| l.dropped_log_inserts())
        .unwrap_or(0);
    Json(HealthResponse {
        ok: true,
        node: "marengo-gateway",
        dropped_log_inserts,
    })
}

async fn tls_fingerprint(
    State(state): State<SharedState>,
) -> Result<Json<TlsFingerprintResponse>, StatusCode> {
    let value = state
        .tls_cert_sha256_base64()
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(TlsFingerprintResponse {
        algorithm: "sha-256",
        value: value.clone(),
        hashes: vec![TlsFingerprintEntry {
            algorithm: "sha-256",
            value,
        }],
    }))
}

async fn stream_chappe(
    State(state): State<SharedState>,
    Query(query): Query<StreamQuery>,
) -> Result<Response, StatusCode> {
    let raw_topics: Vec<String> = query
        .topics
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let topics = filter_topics(&raw_topics);
    if topics.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (mut writer, reader) = tokio::io::duplex(64 * 1024);
    let rx = state.subscribe_envelopes();
    let topics_clone = topics.clone();
    tokio::spawn(async move {
        let _ = framing::pump_envelope_stream(rx, &topics_clone, &mut writer).await;
    });

    let stream = ReaderStream::new(reader)
        .map(|chunk| chunk.map_err(|e| std::io::Error::other(e.to_string())));

    let mut headers = HeaderMap::new();
    if let Ok(value) = header::HeaderValue::from_str(CHAPPE_STREAM_CONTENT_TYPE) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache"),
    );

    Ok((StatusCode::OK, headers, Body::from_stream(stream)).into_response())
}

async fn snapshot_state(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_robot_state())
}

async fn snapshot_safety(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_safety())
}

async fn snapshot_heartbeat(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_heartbeat())
}

async fn snapshot_imu_torso(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_imu_torso())
}

async fn snapshot_host_metrics_pi(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_host_metrics_pi())
}

async fn snapshot_host_metrics_jetson(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_host_metrics_jetson())
}

fn protobuf_snapshot<M: Message>(msg: Option<M>) -> Response {
    match msg {
        Some(m) => {
            let mut headers = HeaderMap::new();
            if let Ok(value) = header::HeaderValue::from_str("application/x-protobuf") {
                headers.insert(header::CONTENT_TYPE, value);
            }
            (StatusCode::OK, headers, m.encode_to_vec()).into_response()
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, "no snapshot yet").into_response(),
    }
}

async fn command_enable(
    State(state): State<SharedState>,
    body: axum::body::Bytes,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let request = EnableRequest::decode(body.as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let payload = request.encode_to_vec();
    state
        .publish_command_envelope(
            "robot/enable",
            "consul",
            "marengo.v1.EnableRequest",
            payload,
        )
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn command_testing_mit(
    State(state): State<SharedState>,
    body: axum::body::Bytes,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let request = MitCommandBatch::decode(body.as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let payload = request.encode_to_vec();
    state
        .publish_command_envelope(
            "robot/testing/mit_command_batch",
            "consul",
            "marengo.v1.MitCommandBatch",
            payload,
        )
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn command_home_retired() -> (StatusCode, String) {
    (
        StatusCode::GONE,
        "POST /command/home retired; use Hardware Set Zero per joint".into(),
    )
}

#[derive(Deserialize)]
struct SetZeroBody {
    joint: String,
    /// Must be true — UI/agent confirm dialog (forgeable without auth; still blocks footguns).
    #[serde(default)]
    confirm: bool,
    /// Operator attestation that sign/direction was checked at mechanical home.
    #[serde(default)]
    sign_test_passed: bool,
    /// Per-client rate-limit key (mirrors actuator `session_id`).
    #[serde(default)]
    client_id: String,
}

async fn command_set_zero(
    State(state): State<SharedState>,
    Json(body): Json<SetZeroBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let joint = body.joint.trim();
    if joint.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "joint required".into()));
    }
    if !body.confirm {
        return Err((
            StatusCode::BAD_REQUEST,
            "confirm=true required for set-zero".into(),
        ));
    }
    if !body.sign_test_passed {
        return Err((
            StatusCode::BAD_REQUEST,
            "sign_test_passed=true required (operator attestation)".into(),
        ));
    }
    let client_id = body.client_id.trim();
    if client_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "client_id required".into()));
    }
    let canonical = marengo_config::resolve_command_joint(joint, &state.command_joints)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                format!("joint not command-eligible: {joint}"),
            )
        })?;

    let bucket = crate::ratelimit::CommandBucket::Motion;
    if !state.rate_limiter.allow(client_id, &canonical, bucket) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "set-zero rate limit exceeded".into(),
        ));
    }

    let request = SetZeroRequest {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        operator_id: "consul".into(),
        joint: canonical.clone(),
        confirm: true,
        sign_test_passed: true,
    };
    let payload = request.encode_to_vec();
    if let Err(e) = state.publish_command_envelope(
        "robot/set_zero",
        "consul",
        "marengo.v1.SetZeroRequest",
        payload,
    ) {
        state.rate_limiter.refund(client_id, &canonical, bucket);
        return Err((StatusCode::BAD_GATEWAY, e));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[derive(Deserialize)]
struct ActiveReportingLeaseBody {
    joint: String,
    #[serde(default)]
    client_id: String,
    /// acquire | renew | release
    action: String,
    #[serde(default)]
    lease_id: String,
}

const MAX_LEASE_ID_LEN: usize = 64;
const MAX_CLIENT_ID_LEN: usize = 64;

async fn command_active_reporting_lease(
    State(state): State<SharedState>,
    Json(body): Json<ActiveReportingLeaseBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let joint = body.joint.trim();
    if joint.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "joint required".into()));
    }
    let client_id = body.client_id.trim();
    if client_id.is_empty() || client_id.len() > MAX_CLIENT_ID_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            "client_id required (max 64 chars)".into(),
        ));
    }
    let lease_id = body.lease_id.trim();
    if lease_id.is_empty() || lease_id.len() > MAX_LEASE_ID_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            "lease_id required (max 64 chars)".into(),
        ));
    }
    let action = match body.action.trim().to_ascii_lowercase().as_str() {
        "acquire" => armee_proto::ActiveReportingLeaseAction::Acquire as i32,
        "renew" => armee_proto::ActiveReportingLeaseAction::Renew as i32,
        "release" => armee_proto::ActiveReportingLeaseAction::Release as i32,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "action must be acquire|renew|release".into(),
            ));
        }
    };
    let canonical = marengo_config::resolve_command_joint(joint, &state.command_joints)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                format!("joint not command-eligible: {joint}"),
            )
        })?;

    // RELEASE must never be starved by acquire/renew heartbeats.
    let is_release = action == armee_proto::ActiveReportingLeaseAction::Release as i32;
    let bucket = crate::ratelimit::CommandBucket::Diagnostics;
    if !is_release && !state.rate_limiter.allow(client_id, &canonical, bucket) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "active-reporting lease rate limit exceeded".into(),
        ));
    }

    let request = ActiveReportingLeaseRequest {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        operator_id: "consul".into(),
        joint: canonical.clone(),
        client_id: client_id.to_string(),
        action,
        lease_id: lease_id.to_string(),
    };
    let payload = request.encode_to_vec();
    if let Err(e) = state.publish_command_envelope(
        "robot/active_reporting_lease",
        "consul",
        "marengo.v1.ActiveReportingLeaseRequest",
        payload,
    ) {
        if !is_release {
            state.rate_limiter.refund(client_id, &canonical, bucket);
        }
        return Err((StatusCode::BAD_GATEWAY, e));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use armee_proto::{Envelope, Heartbeat};
    use axum::body::Body;
    use chappe::Bus;
    use tower::ServiceExt;

    #[tokio::test]
    async fn stream_chappe_emits_length_prefixed_envelopes() {
        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let app = router(state.clone(), None);

        let hb = Heartbeat {
            timestamp_ms: 1,
            node_id: "test".to_string(),
        };
        let envelope = Envelope {
            timestamp_ms: 1,
            source_node: "test".into(),
            message_type: "marengo.v1.Heartbeat".into(),
            payload: hb.encode_to_vec(),
        };
        state.ingest_runtime_frame(
            crate::state::TOPIC_HEARTBEAT.to_string(),
            envelope.encode_to_vec(),
        );

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/stream/chappe?topics=robot/heartbeat")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn command_set_zero_requires_confirm_and_attestation() {
        use marengo_config::CommandJointAllowlist;

        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(
            crate::state::AppState::new(std::sync::Arc::clone(&bus))
                .with_command_joints(CommandJointAllowlist::from_joints(["right_shoulder_pitch"])),
        );
        let app = router(state, None);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/set_zero")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"joint":"right_shoulder_pitch","client_id":"t"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn command_set_zero_rejects_unwired_joint_with_403() {
        use marengo_config::CommandJointAllowlist;

        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(
            crate::state::AppState::new(std::sync::Arc::clone(&bus))
                .with_command_joints(CommandJointAllowlist::from_joints(["right_shoulder_pitch"])),
        );
        let app = router(state, None);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/set_zero")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"joint":"not_a_joint","confirm":true,"sign_test_passed":true,"client_id":"t"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn command_set_zero_returns_429_when_rate_limited() {
        use marengo_config::CommandJointAllowlist;

        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(
            crate::state::AppState::new(std::sync::Arc::clone(&bus))
                .with_command_joints(CommandJointAllowlist::from_joints(["right_shoulder_pitch"])),
        );
        // Burst with distinct client_ids must still share the Motion bucket.
        let body_a = r#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"flood-a"}"#;
        let body_b = r#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"flood-b"}"#;
        let body_c = r#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"flood-c"}"#;
        for body in [body_a, body_b] {
            let app = router(std::sync::Arc::clone(&state), None);
            let response = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/command/set_zero")
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK);
        }
        let app = router(state, None);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/set_zero")
                    .header("content-type", "application/json")
                    .body(Body::from(body_c))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
