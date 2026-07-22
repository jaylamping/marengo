//! Actuator harness HTTP: limits snapshot + tuning-only command path.

use armee_proto::prost::Message;
use armee_proto::{ActuatorLimitSnapshot, Envelope, OperatorCommand};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::ratelimit::CommandBucket;
use crate::state::{SharedState, TOPIC_ACTUATOR_COMMAND};

#[derive(Serialize)]
pub struct OkResponse {
    ok: bool,
}

pub async fn snapshot_actuator_limits(State(state): State<SharedState>) -> Response {
    match state.snapshot_actuator_limits() {
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

pub async fn command_actuator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    // Same shared-token gate as log/config mutations. Fails open when
    // MARENGO_GATEWAY_LOG_TOKEN is unset (LAN bench default). Not operator identity.
    crate::logs::authorize_logs(&headers).map_err(|status| {
        (
            status,
            "unauthorized: set x-marengo-log-token when MARENGO_GATEWAY_LOG_TOKEN is configured"
                .to_string(),
        )
    })?;
    let envelope =
        Envelope::decode(body.as_ref()).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if envelope.message_type != "marengo.v1.OperatorCommand" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unexpected message_type: {}", envelope.message_type),
        ));
    }
    let mut operator = OperatorCommand::decode(envelope.payload.as_slice())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let command = operator.command.as_mut().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "missing ActuatorCommand".to_string(),
        )
    })?;

    let tuning = match command.payload.as_ref() {
        Some(armee_proto::actuator_command::Payload::Tuning(t)) => t.clone(),
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "only Tuning ActuatorCommand payloads are accepted until motion PR-5".to_string(),
            ));
        }
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "missing ActuatorCommand payload".to_string(),
            ));
        }
    };

    let param = tuning.param.as_str();
    if param != "kp" && param != "kd" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unsupported TuningChange.param: {param} (allowed: kp, kd)"),
        ));
    }

    let client_id = operator.session_id.trim();
    if client_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "session_id (client id) required".to_string(),
        ));
    }

    let canonical = marengo_config::resolve_command_joint(&command.joint, &state.command_joints)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (
                StatusCode::FORBIDDEN,
                format!("joint not command-eligible: {}", command.joint),
            )
        })?;

    let limits = state.snapshot_actuator_limits().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "actuator limits snapshot unavailable".to_string(),
        )
    })?;
    let clamped = clamp_tuning_value(&limits, &canonical, param, tuning.value)?;
    if let Some(armee_proto::actuator_command::Payload::Tuning(t)) = command.payload.as_mut() {
        t.value = clamped;
    }
    command.joint = canonical.clone();

    let bucket = CommandBucket::Tuning;
    if !state.rate_limiter.allow(client_id, &canonical, bucket) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "actuator command rate limit exceeded".to_string(),
        ));
    }

    let payload = operator.encode_to_vec();
    if let Err(e) = state.publish_command_envelope(
        TOPIC_ACTUATOR_COMMAND,
        "consul",
        "marengo.v1.OperatorCommand",
        payload,
    ) {
        state.rate_limiter.refund(client_id, &canonical, bucket);
        return Err((StatusCode::BAD_GATEWAY, e));
    }
    Ok(Json(OkResponse { ok: true }))
}

fn clamp_tuning_value(
    snapshot: &ActuatorLimitSnapshot,
    joint: &str,
    param: &str,
    value: f64,
) -> Result<f64, (StatusCode, String)> {
    let limit = snapshot
        .joints
        .iter()
        .find(|j| j.joint == joint)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("no live limits for joint {joint}"),
            )
        })?;
    let max = match param {
        "kp" => limit.kp_max,
        "kd" => limit.kd_max,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unsupported TuningChange.param: {param}"),
            ));
        }
    };
    if !max.is_finite() || max < 0.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid live {param} max for joint {joint}"),
        ));
    }
    if !value.is_finite() {
        return Err((
            StatusCode::BAD_REQUEST,
            "TuningChange.value must be finite".to_string(),
        ));
    }
    Ok(value.clamp(0.0, max))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use std::ffi::OsString;
    use std::sync::MutexGuard;

    use super::*;
    use armee_proto::{
        ActuatorCommand, JointActuatorLimit, OperatorCommand, TuningChange, TuningTier,
    };
    use axum::body::Body;
    use axum::routing::{get, post};
    use axum::Router;
    use chappe::Bus;
    use marengo_config::CommandJointAllowlist;
    use tower::ServiceExt;

    use crate::logs::lock_test_env;
    use crate::state::AppState;

    const TEST_LOG_TOKEN: &str = "actuator-test-token";

    struct EnvVarGuard {
        _lock: MutexGuard<'static, ()>,
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            Self::replace(key, Some(value.into()))
        }

        fn remove(key: &'static str) -> Self {
            Self::replace(key, None)
        }

        fn replace(key: &'static str, value: Option<OsString>) -> Self {
            let lock = lock_test_env();
            let previous = std::env::var_os(key);
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
            Self {
                _lock: lock,
                key,
                previous,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn test_router(state: SharedState) -> Router {
        Router::new()
            .route("/snapshot/actuator/limits", get(snapshot_actuator_limits))
            .route("/command/actuator", post(command_actuator))
            .with_state(state)
    }

    fn test_state() -> SharedState {
        let bus = std::sync::Arc::new(Bus::default());
        let allowlist = CommandJointAllowlist::from_joints([
            "right_shoulder_roll",
            "right_shoulder_pitch",
            "shoulder_pitch",
            "shoulder_roll",
        ]);
        std::sync::Arc::new(
            AppState::new(std::sync::Arc::clone(&bus)).with_command_joints(allowlist),
        )
    }

    fn seed_limits(state: &SharedState, joint: &str, kp_max: f64, kd_max: f64) {
        let snapshot = ActuatorLimitSnapshot {
            timestamp_ms: 42,
            joints: vec![JointActuatorLimit {
                joint: joint.to_string(),
                kp_max,
                kd_max,
                velocity_max_rad_s: 2.0,
                tau_ff_max_nm: 3.0,
                pos_lower_rad: -1.0,
                pos_upper_rad: 1.0,
                pos_soft_lower_rad: -0.9,
                pos_soft_upper_rad: 0.9,
                wired: true,
            }],
        };
        let envelope = Envelope {
            timestamp_ms: 42,
            source_node: "marengo-pi".into(),
            message_type: "marengo.v1.ActuatorLimitSnapshot".into(),
            payload: snapshot.encode_to_vec(),
        };
        state.ingest_runtime_frame(
            crate::state::TOPIC_ACTUATOR_LIMITS.to_string(),
            envelope.encode_to_vec(),
        );
    }

    fn operator_envelope(joint: &str, payload: armee_proto::actuator_command::Payload) -> Vec<u8> {
        let operator = OperatorCommand {
            timestamp_ms: 1,
            session_id: "client-test".to_string(),
            operator_id: String::new(),
            seq: 1,
            command: Some(ActuatorCommand {
                joint: joint.to_string(),
                payload: Some(payload),
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

    fn tuning_payload(param: &str, value: f64) -> armee_proto::actuator_command::Payload {
        armee_proto::actuator_command::Payload::Tuning(TuningChange {
            tier: TuningTier::RuntimeMit as i32,
            param: param.to_string(),
            value,
            persist: false,
        })
    }

    #[tokio::test]
    async fn command_actuator_requires_configured_log_token() {
        let _token_guard = EnvVarGuard::set("MARENGO_GATEWAY_LOG_TOKEN", TEST_LOG_TOKEN);
        let state = test_state();
        seed_limits(&state, "right_shoulder_pitch", 50.0, 5.0);
        let app = test_router(state);
        let body = operator_envelope("right_shoulder_pitch", tuning_payload("kp", 10.0));

        let unauthorized = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let authorized = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .header("x-marengo-log-token", TEST_LOG_TOKEN)
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(authorized.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn command_actuator_rejects_unwired_joint_with_403() {
        let _token_guard = EnvVarGuard::remove("MARENGO_GATEWAY_LOG_TOKEN");
        let state = test_state();
        seed_limits(&state, "right_shoulder_pitch", 50.0, 5.0);
        let app = test_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope(
                        "left_wrist_pitch",
                        tuning_payload("kp", 10.0),
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn command_actuator_rejects_non_tuning_with_400() {
        let _token_guard = EnvVarGuard::remove("MARENGO_GATEWAY_LOG_TOKEN");
        let state = test_state();
        seed_limits(&state, "right_shoulder_pitch", 50.0, 5.0);
        let app = test_router(state);
        let payload =
            armee_proto::actuator_command::Payload::Jog(armee_proto::JogCommand { delta_rad: 0.1 });
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope(
                        "right_shoulder_pitch",
                        payload,
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn command_actuator_rejects_without_limits_snapshot() {
        let _token_guard = EnvVarGuard::remove("MARENGO_GATEWAY_LOG_TOKEN");
        let app = test_router(test_state());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope(
                        "right_shoulder_pitch",
                        tuning_payload("kp", 10.0),
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn command_actuator_clamps_to_live_kp_max() {
        let _token_guard = EnvVarGuard::remove("MARENGO_GATEWAY_LOG_TOKEN");
        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(
            AppState::new(std::sync::Arc::clone(&bus))
                .with_command_joints(CommandJointAllowlist::from_joints(["right_shoulder_pitch"])),
        );
        seed_limits(&state, "right_shoulder_pitch", 20.0, 5.0);
        let mut rx = bus.subscribe(TOPIC_ACTUATOR_COMMAND);
        let app = test_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope(
                        "right_shoulder_pitch",
                        tuning_payload("kp", 99.0),
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        let published = rx.recv().await.expect("published");
        let env = Envelope::decode(published.as_slice()).expect("envelope");
        let operator = OperatorCommand::decode(env.payload.as_slice()).expect("operator");
        let cmd = operator.command.expect("command");
        assert_eq!(cmd.joint, "right_shoulder_pitch");
        match cmd.payload.expect("payload") {
            armee_proto::actuator_command::Payload::Tuning(t) => {
                assert!((t.value - 20.0).abs() < f64::EPSILON);
            }
            other => panic!("expected tuning, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn command_actuator_publishes_canonical_joint_for_left_alias() {
        let _token_guard = EnvVarGuard::remove("MARENGO_GATEWAY_LOG_TOKEN");
        let bus = std::sync::Arc::new(Bus::default());
        let state = std::sync::Arc::new(
            AppState::new(std::sync::Arc::clone(&bus))
                .with_command_joints(CommandJointAllowlist::from_joints(["shoulder_pitch"])),
        );
        seed_limits(&state, "shoulder_pitch", 50.0, 5.0);
        let mut rx = bus.subscribe(TOPIC_ACTUATOR_COMMAND);
        let app = test_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(operator_envelope(
                        "left_shoulder_pitch",
                        tuning_payload("kp", 10.0),
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        let published = rx.recv().await.expect("published");
        let env = Envelope::decode(published.as_slice()).expect("envelope");
        let operator = OperatorCommand::decode(env.payload.as_slice()).expect("operator");
        assert_eq!(operator.command.expect("command").joint, "shoulder_pitch");
    }

    #[tokio::test]
    async fn command_actuator_returns_429_when_rate_limited() {
        let _token_guard = EnvVarGuard::remove("MARENGO_GATEWAY_LOG_TOKEN");
        let state = test_state();
        seed_limits(&state, "right_shoulder_pitch", 50.0, 5.0);
        let app = test_router(state);
        let body = operator_envelope("right_shoulder_pitch", tuning_payload("kp", 10.0));
        for _ in 0..10 {
            let response = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/command/actuator")
                        .header("content-type", "application/x-protobuf")
                        .body(Body::from(body.clone()))
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK);
        }
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/command/actuator")
                    .header("content-type", "application/x-protobuf")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn snapshot_actuator_limits_503_when_cache_empty() {
        let app = test_router(test_state());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/snapshot/actuator/limits")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
