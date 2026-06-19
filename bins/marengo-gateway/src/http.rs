use std::path::Path;

use armee_proto::prost::Message;
use armee_proto::{
    ActuatorCommand, EnableRequest, Envelope, OperatorCommand, SessionStartRequest,
    SessionStartResponse, TuningChange, TuningTier,
};
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

use crate::framing::{self, CHAPPE_STREAM_CONTENT_TYPE};
use crate::logs;
use crate::session;
use crate::state::{filter_topics, SharedState, TOPIC_ACTUATOR_COMMAND};

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
        .route("/command/enable", post(command_enable))
        .route("/session/start", post(session_start))
        .route("/command/actuator", post(command_actuator))
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

async fn session_start(body: axum::body::Bytes) -> Result<Response, (StatusCode, String)> {
    let request = if body.is_empty() {
        SessionStartRequest::default()
    } else {
        SessionStartRequest::decode(body.as_ref())
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    };
    let response = session::build_session_start_response(&request);
    protobuf_ok_response(&response)
}

async fn command_actuator(
    State(state): State<SharedState>,
    body: axum::body::Bytes,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let envelope = Envelope::decode(body.as_ref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if envelope.message_type != "marengo.v1.OperatorCommand" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unexpected message_type: {}", envelope.message_type),
        ));
    }
    let mut operator = OperatorCommand::decode(envelope.payload.as_slice())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let joint = operator
        .command
        .as_ref()
        .map(|c| c.joint.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "missing ActuatorCommand".to_string(),
            )
        })?;
    let canonical = state
        .resolve_actuator_joint(joint)
        .map_err(|e| (StatusCode::FORBIDDEN, e))?;
    if let Some(cmd) = operator.command.as_mut() {
        cmd.joint = canonical.to_string();
    }
    let payload = operator.encode_to_vec();
    state
        .publish_command_envelope(
            TOPIC_ACTUATOR_COMMAND,
            "consul",
            "marengo.v1.OperatorCommand",
            payload,
        )
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(OkResponse { ok: true }))
}

fn protobuf_ok_response<M: Message>(msg: &M) -> Result<Response, (StatusCode, String)> {
    let mut headers = HeaderMap::new();
    if let Ok(value) = header::HeaderValue::from_str("application/x-protobuf") {
        headers.insert(header::CONTENT_TYPE, value);
    }
    Ok((StatusCode::OK, headers, msg.encode_to_vec()).into_response())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use armee_proto::{Envelope, Heartbeat, OperatorCommand};
    use axum::body::Body;
    use chappe::Bus;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn test_state() -> SharedState {
        let bus = std::sync::Arc::new(Bus::default());
        std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)))
    }

    fn operator_envelope(joint: &str) -> Vec<u8> {
        let operator = OperatorCommand {
            timestamp_ms: 1,
            session_id: "sess-test".to_string(),
            operator_id: String::new(),
            seq: 1,
            command: Some(ActuatorCommand {
                joint: joint.to_string(),
                payload: Some(armee_proto::actuator_command::Payload::Tuning(
                    TuningChange {
                        tier: TuningTier::RuntimeMit as i32,
                        param: "kp".to_string(),
                        value: 10.0,
                        persist: false,
                    },
                )),
            }),
        };
        Envelope {
            timestamp_ms: 1,
            source_node: "consul".to_string(),
            message_type: "marengo.v1.OperatorCommand".to_string(),
            payload: operator.encode_to_vec(),
        }
        .encode_to_vec()
    }

    #[tokio::test]
    async fn stream_chappe_emits_length_prefixed_envelopes() {
        let state = test_state();
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
    async fn session_start_mints_uuid() {
        let app = router(test_state(), None);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/session/start")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let parsed = SessionStartResponse::decode(body.as_ref()).expect("decode");
        let uuid = Uuid::parse_str(&parsed.session_id).expect("uuid");
        assert_eq!(uuid.get_version(), Some(uuid::Version::Random));
        assert!(parsed.started_ms > 0);
    }

    #[tokio::test]
    async fn command_actuator_rejects_unwired_joint_with_403() {
        let app = router(test_state(), None);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope("left_wrist_pitch")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn command_actuator_rejects_malformed_proto_with_400() {
        let app = router(test_state(), None);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(vec![0xff, 0x00, 0x01]))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn command_actuator_publishes_canonical_joint_for_alias() {
        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = bus.subscribe(TOPIC_ACTUATOR_COMMAND);
        let app = router(state, None);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope("left_shoulder_pitch")))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        let published = rx.recv().await.expect("published");
        let env = Envelope::decode(published.as_slice()).expect("envelope");
        let operator = OperatorCommand::decode(env.payload.as_slice()).expect("operator");
        let joint = operator.command.expect("command").joint;
        assert_eq!(joint, "shoulder_pitch");
    }
}
