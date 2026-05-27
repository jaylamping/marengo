use std::path::Path;

use armee_proto::prost::Message;
use armee_proto::EnableRequest;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::state::SharedState;

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    node: &'static str,
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
        .route("/snapshot/robot/state", get(snapshot_state))
        .route("/snapshot/robot/safety", get(snapshot_safety))
        .route("/snapshot/robot/heartbeat", get(snapshot_heartbeat))
        .route("/command/enable", post(command_enable))
        .layer(cors)
        .with_state(state);

    match web_root {
        Some(root) => {
            let index = root.join("index.html");
            let static_files = ServeDir::new(root).not_found_service(ServeFile::new(index));
            api.fallback_service(static_files)
        }
        None => api,
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        node: "marengo-gateway",
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

async fn snapshot_state(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_robot_state())
}

async fn snapshot_safety(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_safety())
}

async fn snapshot_heartbeat(State(state): State<SharedState>) -> Response {
    protobuf_snapshot(state.snapshot_heartbeat())
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
    body: Bytes,
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
