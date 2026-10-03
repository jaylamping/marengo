use std::path::Path;

use armee_proto::prost::Message;
use armee_proto::ActiveReportingLeaseRequest;
use armee_proto::EnableRequest;
use armee_proto::MitCommandBatch;
use armee_proto::MotorStatusPollRequest;
use armee_proto::SetZeroRequest;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::access::Capability;
use crate::actuator;
use crate::config;
use crate::deploy;
use crate::framing::{self, CHAPPE_STREAM_CONTENT_TYPE};
use crate::hardware;
use crate::logs;
use crate::restart;
use crate::state::{filter_topics, SharedState};

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    node: &'static str,
    listeners: crate::state::ListenerHealth,
    ipc_connected: bool,
    store_available: bool,
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
/// HTTP and HTTPS share access admission before extraction/handler work.
/// Runtime confirmation, attestation, freshness and Davout gates remain additional.
pub fn router(state: SharedState, web_root: Option<&Path>) -> Router {
    let access = std::sync::Arc::clone(&state.access);
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, parts| {
            access.allows_origin(origin, &parts.headers)
        }))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::HeaderName::from_static("x-marengo-log-token"),
        ])
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
        .route("/hardware/urdf/upload", post(hardware::post_urdf_upload))
        .route(
            "/hardware/urdf/resolve-preview",
            post(hardware::post_resolve_preview),
        )
        .route("/hardware/urdf/activate", post(hardware::post_activate))
        .route("/hardware/urdf/archive", get(hardware::get_archive_list))
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
        .route("/version/status", get(deploy::get_version_status))
        .route("/control/deploy", post(deploy::post_control_deploy))
        .route(
            "/command/enable",
            post(command_enable).layer(DefaultBodyLimit::max(COMMAND_BODY_LIMIT)),
        )
        .route(
            "/command/testing_mit",
            post(command_testing_mit).layer(DefaultBodyLimit::max(COMMAND_BODY_LIMIT)),
        )
        // Retired: operator HomingComplete / Testing Home — use Hardware Set Zero.
        .route("/command/home", post(command_home_retired))
        .route("/command/set_zero", post(command_set_zero))
        .route(
            "/command/active_reporting_lease",
            post(command_active_reporting_lease),
        )
        .route(
            "/command/motor_status_poll",
            post(command_motor_status_poll),
        )
        .route("/command/actuator", post(actuator::command_actuator))
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(
            std::sync::Arc::clone(&state),
            authorize_api,
        ))
        .with_state(state);

    match web_root {
        Some(root) => {
            let index = root.join("index.html");
            let assets = root.join("assets");
            let mut router = api;
            if assets.is_dir() {
                let assets_svc = ServiceBuilder::new()
                    .layer(SetResponseHeaderLayer::overriding(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("public, max-age=31536000, immutable"),
                    ))
                    .service(ServeDir::new(assets));
                router = router.nest_service("/assets", assets_svc);
            }
            let index_svc = ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache"),
                ))
                .service(ServeFile::new(index));
            router.fallback_service(index_svc)
        }
        None => api,
    }
}

async fn authorize_api(
    State(state): State<SharedState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if let Err(status) = state.access.validate_origin(request.headers()) {
        return (status, "browser Origin refused").into_response();
    }
    if request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }
    let path = request.uri().path();
    let capability = if path == "/stream/chappe" {
        let Ok(Query(query)) = Query::<StreamQuery>::try_from_uri(request.uri()) else {
            return (StatusCode::BAD_REQUEST, "invalid topics").into_response();
        };
        query
            .topics
            .split(',')
            .map(str::trim)
            .any(sensitive_topic)
            .then_some(Capability::SensitiveRead)
    } else if path.starts_with("/logs/") || path == "/snapshot/logs/recent" || path == "/settings" {
        Some(Capability::SensitiveRead)
    } else if path == "/config/snapshot"
        || path == "/hardware/completeness"
        || path == "/hardware/urdf/archive"
        || path == "/hardware/commissioning-scope"
        || path.starts_with("/hardware/urdf")
    {
        Some(Capability::Configuration)
    } else if matches!(
        *request.method(),
        axum::http::Method::POST | axum::http::Method::PUT | axum::http::Method::DELETE
    ) {
        if path == "/command/set_zero" {
            Some(Capability::Calibration)
        } else if path.starts_with("/command/") && path != "/command/home" {
            Some(Capability::Control)
        } else if path.starts_with("/config/") || path.starts_with("/hardware/") {
            Some(Capability::Configuration)
        } else if path.starts_with("/control/") {
            Some(Capability::Management)
        } else if path == "/command/home" {
            None
        } else {
            // Future mutations must opt into a narrower role, never anonymous access.
            Some(Capability::Management)
        }
    } else {
        None
    };
    if let Some(capability) = capability {
        if let Err(status) = state.access.authorize(request.headers(), capability) {
            return (
                status,
                "gateway access credential required for this operation",
            )
                .into_response();
        }
    }
    if path == "/config/patch" {
        if let Err(status) = state
            .access
            .authorize(request.headers(), Capability::Control)
        {
            return (
                status,
                "config patch requires control and configuration credentials",
            )
                .into_response();
        }
    }
    next.run(request).await
}

pub(crate) fn sensitive_topic(topic: &str) -> bool {
    matches!(
        topic,
        "logs/structured"
            | "robot/audit/action"
            | "robot/audit/tuning"
            | "robot/testing/mit_command_batch"
    )
}

async fn health(State(state): State<SharedState>) -> Json<HealthResponse> {
    let dropped_log_inserts = state
        .logs
        .as_ref()
        .map(|l| l.dropped_log_inserts())
        .unwrap_or(0);
    let listeners = state.listener_health();
    Json(HealthResponse {
        ok: listeners.ready(),
        node: "marengo-gateway",
        listeners,
        ipc_connected: state.runtime_connected(),
        store_available: state.logs.is_some(),
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

    let Some(permit) = state.acquire_http_stream() else {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    };
    let (mut writer, reader) = tokio::io::duplex(64 * 1024);
    let rx = state.subscribe_envelopes();
    let topics_clone = topics.clone();
    tokio::spawn(async move {
        let _permit = permit;
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

/// Rate-limit key for `/command/enable` (`enable == true` only); global Motion bucket.
const ENABLE_RATE_KEY: &str = "__enable__";
/// Rate-limit key for `/command/testing_mit`; global Testing bucket.
const TESTING_MIT_RATE_KEY: &str = "__testing_mit__";
/// Control payloads are tiny protobufs; 64 KiB is a generous ceiling.
const COMMAND_BODY_LIMIT: usize = 64 * 1024;

async fn command_enable(
    State(state): State<SharedState>,
    body: axum::body::Bytes,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let request = EnableRequest::decode(body.as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    // Disable/stop must never be refused for rate reasons; only enabling is capped.
    let bucket = crate::ratelimit::CommandBucket::Motion;
    let limited = request.enable;
    if limited
        && !state
            .rate_limiter
            .allow(ENABLE_RATE_KEY, ENABLE_RATE_KEY, bucket)
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "enable rate limit exceeded".into(),
        ));
    }
    let payload = request.encode_to_vec();
    if let Err(e) = state.publish_command_envelope(
        "robot/enable",
        "consul",
        "marengo.v1.EnableRequest",
        payload,
    ) {
        if limited {
            state
                .rate_limiter
                .refund(ENABLE_RATE_KEY, ENABLE_RATE_KEY, bucket);
        }
        return Err((StatusCode::BAD_GATEWAY, e));
    }
    Ok(Json(OkResponse { ok: true }))
}

/// Resolve a testing-MIT entry name to its canonical wired form.
///
/// Plain names resolve directly; in-band `wave:<joint>:<min>:<max>:<cycles>:<half>` names
/// resolve only the joint segment and keep the remaining fields verbatim.
fn canonicalize_testing_mit_name(
    name: &str,
    state: &crate::state::AppState,
) -> Result<String, (StatusCode, String)> {
    let not_eligible = |joint: &str| {
        (
            StatusCode::FORBIDDEN,
            format!("joint not command-eligible: {joint}"),
        )
    };
    let Some(rest) = name.strip_prefix("wave:") else {
        return marengo_config::resolve_command_joint(name, &state.command_joints)
            .map(str::to_owned)
            .ok_or_else(|| not_eligible(name));
    };
    let fields: Vec<&str> = rest.split(':').collect();
    if fields.len() != 5 || fields[0].is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "malformed wave name; expected wave:<joint>:<min>:<max>:<cycles>:<half>".into(),
        ));
    }
    let canonical = marengo_config::resolve_command_joint(fields[0], &state.command_joints)
        .ok_or_else(|| not_eligible(fields[0]))?;
    Ok(format!("wave:{canonical}:{}", fields[1..].join(":")))
}

async fn command_testing_mit(
    State(state): State<SharedState>,
    body: axum::body::Bytes,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let mut request = MitCommandBatch::decode(body.as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if request.joints.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "joints required".into()));
    }
    if request.joints.len() > state.command_joints.iter().count() {
        return Err((
            StatusCode::BAD_REQUEST,
            "too many joint entries in batch".into(),
        ));
    }
    // Validate and rewrite every entry before any publication: all-or-nothing.
    for joint in &mut request.joints {
        joint.name = canonicalize_testing_mit_name(&joint.name, &state)?;
    }

    let bucket = crate::ratelimit::CommandBucket::Testing;
    if !state
        .rate_limiter
        .allow(TESTING_MIT_RATE_KEY, TESTING_MIT_RATE_KEY, bucket)
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "testing MIT rate limit exceeded".into(),
        ));
    }
    let payload = request.encode_to_vec();
    if let Err(e) = state.publish_command_envelope(
        "robot/testing/mit_command_batch",
        "consul",
        "marengo.v1.MitCommandBatch",
        payload,
    ) {
        state
            .rate_limiter
            .refund(TESTING_MIT_RATE_KEY, TESTING_MIT_RATE_KEY, bucket);
        return Err((StatusCode::BAD_GATEWAY, e));
    }
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

#[derive(Deserialize)]
struct MotorStatusPollBody {
    /// Rate-limit key only (global StatusPoll bucket ignores rotation).
    #[serde(default)]
    client_id: String,
}

/// Light Hardware-page solicit: Pi re-TX Disable (type-4) per motor → OperationStatus.
/// Rate-limited (~0.5/s, burst 2) globally so the bus is not flooded.
async fn command_motor_status_poll(
    State(state): State<SharedState>,
    Json(body): Json<MotorStatusPollBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let client_id = body.client_id.trim();
    if client_id.is_empty() || client_id.len() > MAX_CLIENT_ID_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            "client_id required (max 64 chars)".into(),
        ));
    }
    let bucket = crate::ratelimit::CommandBucket::StatusPoll;
    if !state.rate_limiter.allow(client_id, "_", bucket) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "motor status poll rate limit exceeded".into(),
        ));
    }

    let request = MotorStatusPollRequest {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        operator_id: "consul".into(),
    };
    let payload = request.encode_to_vec();
    if let Err(e) = state.publish_command_envelope(
        "robot/motor_status_poll",
        "consul",
        "marengo.v1.MotorStatusPollRequest",
        payload,
    ) {
        state.rate_limiter.refund(client_id, "_", bucket);
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

    const ACCESS_TOKEN: &str = "isolated-http-admission-fixture";

    fn command_state(bus: std::sync::Arc<Bus>) -> SharedState {
        let mut state = crate::state::AppState::new(bus).with_command_joints(
            marengo_config::CommandJointAllowlist::from_joints(["right_shoulder_pitch"]),
        );
        state.access = std::sync::Arc::new(
            crate::access::AccessPolicy::operator_fixture(ACCESS_TOKEN).expect("fixture policy"),
        );
        std::sync::Arc::new(state)
    }

    fn command_request() -> axum::http::request::Builder {
        axum::http::Request::builder().header("authorization", format!("Bearer {ACCESS_TOKEN}"))
    }

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
    async fn http_stream_subscribers_are_bounded() {
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::new(
            Bus::default(),
        )));
        let app = router(std::sync::Arc::clone(&state), None);
        let mut bodies = Vec::new();
        for _ in 0..64 {
            let response = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/stream/chappe?topics=robot/heartbeat")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK);
            bodies.push(response.into_body());
        }
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/stream/chappe?topics=robot/heartbeat")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        drop(bodies);
        drop(state);
    }

    #[tokio::test]
    async fn command_set_zero_requires_confirm_and_attestation() {
        let bus = std::sync::Arc::new(Bus::default());
        let state = command_state(bus);
        let app = router(state, None);
        let response = app
            .oneshot(
                command_request()
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
        let bus = std::sync::Arc::new(Bus::default());
        let state = command_state(bus);
        let app = router(state, None);
        let response = app
            .oneshot(
                command_request()
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
        let bus = std::sync::Arc::new(Bus::default());
        let state = command_state(bus);
        // Burst with distinct client_ids must still share the Motion bucket.
        let body_a = r#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"flood-a"}"#;
        let body_b = r#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"flood-b"}"#;
        let body_c = r#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"flood-c"}"#;
        for body in [body_a, body_b] {
            let app = router(std::sync::Arc::clone(&state), None);
            let response = app
                .oneshot(
                    command_request()
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
                command_request()
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

    #[tokio::test]
    async fn command_motor_status_poll_rate_limits_globally() {
        let bus = std::sync::Arc::new(Bus::default());
        let state = command_state(bus);
        let app = router(std::sync::Arc::clone(&state), None);
        let ok_a = app
            .oneshot(
                command_request()
                    .method("POST")
                    .uri("/command/motor_status_poll")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"client_id":"consul-a"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ok_a.status(), StatusCode::OK);

        let app = router(std::sync::Arc::clone(&state), None);
        let ok_b = app
            .oneshot(
                command_request()
                    .method("POST")
                    .uri("/command/motor_status_poll")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"client_id":"consul-b"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ok_b.status(), StatusCode::OK);

        let app = router(state, None);
        let limited = app
            .oneshot(
                command_request()
                    .method("POST")
                    .uri("/command/motor_status_poll")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"client_id":"consul-c"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
