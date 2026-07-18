use std::path::Path;

use armee_proto::prost::Message;
use armee_proto::EnableRequest;
use armee_proto::HomingComplete;
use armee_proto::MitCommandBatch;
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
use crate::logs;
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
        .route("/command/enable", post(command_enable))
        .route("/command/testing_mit", post(command_testing_mit))
        .route("/command/home", post(command_home))
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

async fn command_home(
    State(state): State<SharedState>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let request = HomingComplete {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        node_id: "consul".into(),
    };
    let payload = request.encode_to_vec();
    state
        .publish_command_envelope(
            "robot/homing",
            "consul",
            "marengo.v1.HomingComplete",
            payload,
        )
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
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
}
