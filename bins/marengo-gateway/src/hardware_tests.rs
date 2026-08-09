#![allow(clippy::await_holding_lock, clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use armee_proto::prost::Message;
use armee_proto::{Envelope, Heartbeat, OperationalMode, SafetyState};
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use chappe::Bus;
use tower::ServiceExt;

use crate::config::authorize_config_mutation;
use crate::http;
use crate::logs::lock_test_env;
use crate::state::{AppState, SharedState, TOPIC_HEARTBEAT, TOPIC_SAFETY};

const TEST_TOKEN: &str = "hardware-test-token";
const TOKEN_ENV: &str = "MARENGO_GATEWAY_LOG_TOKEN";

fn test_app(state: crate::state::SharedState) -> axum::Router {
    http::router(state, None)
}

fn auth_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-marengo-log-token", HeaderValue::from_static(TEST_TOKEN));
    headers
}

fn envelope_bytes<M: Message>(message_type: &str, message: &M) -> Vec<u8> {
    Envelope {
        timestamp_ms: 1,
        source_node: "test".to_string(),
        message_type: message_type.to_string(),
        payload: message.encode_to_vec(),
    }
    .encode_to_vec()
}

fn state_with_safety(mode: OperationalMode, heartbeat_ts_ms: u64) -> SharedState {
    let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(Bus::default())));
    state.ingest_runtime_frame(
        TOPIC_SAFETY.to_string(),
        envelope_bytes(
            "marengo.v1.SafetyState",
            &SafetyState {
                timestamp_ms: heartbeat_ts_ms,
                mode: mode as i32,
                hardware_estop_asserted: false,
                software_estop_latched: false,
                active_faults: vec![],
            },
        ),
    );
    state.ingest_runtime_frame(
        TOPIC_HEARTBEAT.to_string(),
        envelope_bytes(
            "marengo.v1.Heartbeat",
            &Heartbeat {
                timestamp_ms: heartbeat_ts_ms,
                node_id: "test".to_string(),
            },
        ),
    );
    state
}

#[tokio::test]
async fn urdf_read_rejects_missing_auth() {
    let _env = lock_test_env();
    std::env::remove_var(TOKEN_ENV);
    let bus = std::sync::Arc::new(Bus::default());
    let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
    let app = test_app(state);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/urdf")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn urdf_read_returns_bytes_with_auth() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let bus = std::sync::Arc::new(Bus::default());
    let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
    let app = test_app(state);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/urdf")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn archive_reads_require_auth() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(Bus::default())));
    let app = test_app(state);

    for uri in [
        "/hardware/urdf/archive",
        "/hardware/urdf/archive/upload-test",
    ] {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "uri={uri}");
    }

    let list = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/urdf/archive")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(list.status(), StatusCode::OK);

    let fetch = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/urdf/archive/upload-definitely-missing")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(fetch.status(), StatusCode::NOT_FOUND);

    std::env::remove_var(TOKEN_ENV);
}

#[tokio::test]
async fn activate_and_resolve_reject_unsafe_upload_ids() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(Bus::default())));
    let app = test_app(state);

    for (uri, upload_id) in [
        ("/hardware/urdf/activate", ".."),
        ("/hardware/urdf/resolve-preview", "/tmp/escape"),
    ] {
        let body = serde_json::json!({
            "upload_id": upload_id,
            "resolutions": [],
        });
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .header("x-marengo-log-token", TEST_TOKEN)
                    .body(Body::from(body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "uri={uri}");
    }

    std::env::remove_var(TOKEN_ENV);
}

#[tokio::test]
async fn activate_refuses_active_with_fresh_heartbeat() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let state = state_with_safety(OperationalMode::Active, crate::restart::now_ms());
    let app = test_app(state);
    let body = serde_json::json!({
        "upload_id": "upload-active-test",
        "resolutions": [],
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/hardware/urdf/activate")
                .header("content-type", "application/json")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let result: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(result["ok"], false);
    assert_eq!(result["restart_required"], false);
    assert_eq!(
        result["message"],
        "urdf activate refused while operational mode Active"
    );

    std::env::remove_var(TOKEN_ENV);
}

#[tokio::test]
async fn completeness_is_advisory_and_upload_not_blocked() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let root = marengo_config::resolve_repo_root();
    let tmp = tempfile::tempdir().expect("tmp");
    let assets = tmp.path().join("assets/urdf");
    fs::create_dir_all(assets.join("staging")).expect("staging");
    fs::create_dir_all(assets.join("archive")).expect("archive");
    fs::copy(
        root.join("assets/urdf/marengo.urdf"),
        assets.join("marengo.urdf"),
    )
    .expect("urdf");

    let config_dir = tmp.path().join("config");
    fs::create_dir_all(&config_dir).expect("config");
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        fs::copy(root.join("config").join(name), config_dir.join(name)).expect("yaml");
    }
    let mut motors = marengo_config::load_motors_config_from(&config_dir).expect("motors");
    let elbow = motors
        .motors
        .iter_mut()
        .find(|m| m.joint == "right_elbow_pitch")
        .expect("elbow");
    elbow.bench.position_upper_rad = 99.0;
    fs::write(
        config_dir.join("motors.yaml"),
        serde_yaml::to_string(&motors).expect("motors yaml"),
    )
    .expect("write motors");

    std::env::set_var("MARENGO_ROOT", tmp.path());
    std::env::set_var("MARENGO_CONFIG_DIR", config_dir.as_os_str());

    let bus = std::sync::Arc::new(Bus::default());
    let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
    let app = test_app(state);

    let completeness = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/completeness")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(completeness.status(), StatusCode::OK);

    let contributor = fs::read_to_string(root.join("assets/urdf/marengo.urdf")).expect("urdf");
    let upload = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/hardware/urdf/upload")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::from(contributor))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(upload.status(), StatusCode::OK);

    std::env::remove_var("MARENGO_ROOT");
    std::env::remove_var("MARENGO_CONFIG_DIR");
}

#[tokio::test]
async fn activate_archives_replaced_active_and_promotes_merge() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let root = marengo_config::resolve_repo_root();
    let tmp = tempfile::tempdir().expect("tmp");
    let assets = tmp.path().join("assets/urdf");
    fs::create_dir_all(assets.join("staging")).expect("staging");
    fs::create_dir_all(assets.join("archive")).expect("archive");
    fs::copy(
        root.join("assets/urdf/marengo.urdf"),
        assets.join("marengo.urdf"),
    )
    .expect("urdf");

    let config_dir = tmp.path().join("config");
    fs::create_dir_all(&config_dir).expect("config");
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        fs::copy(root.join("config").join(name), config_dir.join(name)).expect("yaml");
    }

    std::env::set_var("MARENGO_ROOT", tmp.path());
    std::env::set_var("MARENGO_CONFIG_DIR", config_dir.as_os_str());

    let master_before = fs::read_to_string(assets.join("marengo.urdf")).expect("master");
    let contributor = master_before.replace(
        "<limit lower=\"-0.50\" upper=\"1.2\"",
        "<limit lower=\"-0.50\" upper=\"1.5\"",
    );

    let upload_id = "upload-test-activate";
    let staging = assets.join("staging").join(upload_id);
    fs::create_dir_all(&staging).expect("staging dir");
    fs::write(staging.join("contributor.urdf"), &contributor).expect("write contributor");

    let bus = std::sync::Arc::new(Bus::default());
    let db = tmp.path().join("activate-test.db");
    let store = marengo_store::Store::open(&db, tmp.path()).expect("store");
    let logs = crate::logs::LogServices::open(store);
    let state = std::sync::Arc::new(
        crate::state::AppState::new(std::sync::Arc::clone(&bus)).with_logs(logs),
    );
    let app = test_app(state);

    let body = serde_json::json!({
        "upload_id": upload_id,
        "resolutions": [{
            "joint": "right_elbow_pitch",
            "field": "limit_upper",
            "choice": "contributor"
        }],
        "operator_id": "test"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/hardware/urdf/activate")
                .header("content-type", "application/json")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let result: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["restart_required"], true);

    let archive = assets.join("archive").join(upload_id);
    assert!(archive.join("replaced_active.urdf").is_file());
    assert!(archive.join("contributor.urdf").is_file());
    assert!(archive.join("manifest.json").is_file());
    let replaced = fs::read_to_string(archive.join("replaced_active.urdf")).expect("replaced");
    assert_eq!(replaced, master_before);
    let live = fs::read_to_string(assets.join("marengo.urdf")).expect("live");
    assert!(live.contains("upper=\"1.5\""));

    std::env::remove_var("MARENGO_ROOT");
    std::env::remove_var("MARENGO_CONFIG_DIR");
}

#[tokio::test]
async fn activate_saves_manifest_before_failed_live_promote() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let root = marengo_config::resolve_repo_root();
    let tmp = tempfile::tempdir().expect("tmp");
    let assets = tmp.path().join("assets/urdf");
    fs::create_dir_all(assets.join("staging")).expect("staging");
    fs::create_dir_all(assets.join("archive")).expect("archive");
    fs::copy(
        root.join("assets/urdf/marengo.urdf"),
        assets.join("marengo.urdf"),
    )
    .expect("urdf");
    std::env::set_var("MARENGO_ROOT", tmp.path());

    let master_before = fs::read_to_string(assets.join("marengo.urdf")).expect("master");
    let contributor = master_before.replace(
        "<limit lower=\"-0.50\" upper=\"1.2\"",
        "<limit lower=\"-0.50\" upper=\"1.5\"",
    );
    let upload_id = "upload-test-promote-failure";
    let staging = assets.join("staging").join(upload_id);
    fs::create_dir_all(&staging).expect("staging dir");
    fs::write(staging.join("contributor.urdf"), contributor).expect("contributor");
    fs::create_dir(assets.join("marengo.urdf.tmp")).expect("block live temp write");

    let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(Bus::default())));
    let app = test_app(state);
    let body = serde_json::json!({
        "upload_id": upload_id,
        "resolutions": [{
            "joint": "right_elbow_pitch",
            "field": "limit_upper",
            "choice": "contributor"
        }],
        "operator_id": "test"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/hardware/urdf/activate")
                .header("content-type", "application/json")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let result: serde_json::Value = serde_json::from_slice(&body).expect("json");

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(result["ok"], false);
    assert!(result["message"]
        .as_str()
        .expect("message")
        .contains("archive was saved but activate failed"));
    assert_eq!(
        fs::read_to_string(assets.join("marengo.urdf")).expect("live"),
        master_before
    );
    assert!(assets
        .join("archive")
        .join(upload_id)
        .join("manifest.json")
        .is_file());
    assert!(staging.is_dir());

    std::env::remove_var(TOKEN_ENV);
    std::env::remove_var("MARENGO_ROOT");
}

#[test]
fn authorize_config_mutation_matches_log_token() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let headers = auth_headers();
    assert!(authorize_config_mutation(&headers).is_ok());
}

#[tokio::test]
async fn commissioning_scope_crud_auth_widen_and_unknown() {
    let _env = lock_test_env();
    let tmp = tempfile::tempdir().expect("temp");
    let repo = tmp.path();
    let config_dir = repo.join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    // Minimal robot.yaml master joints for validation.
    fs::write(
        config_dir.join("robot.yaml"),
        r#"
robot:
  name: test
  urdf: assets/urdf/marengo.urdf
  bench:
    max_joint_velocity_rad_s: 1.0
    max_joint_torque_nm: 10.0
  joints:
    - right_shoulder_roll
    - right_shoulder_pitch
    - right_upper_arm_yaw
"#,
    )
    .expect("robot");
    std::env::set_var("MARENGO_ROOT", repo);
    std::env::set_var("MARENGO_CONFIG_DIR", config_dir.as_os_str());
    std::env::set_var(
        "MARENGO_JOINT_SUBSET",
        "right_shoulder_roll,right_shoulder_pitch,right_upper_arm_yaw",
    );
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);

    let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(Bus::default())));
    let app = test_app(state);

    // Missing auth → 401
    let unauth = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/commissioning-scope")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

    // GET empty
    let get0 = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/hardware/commissioning-scope")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(get0.status(), StatusCode::OK);
    let body0: serde_json::Value =
        serde_json::from_slice(&axum::body::to_bytes(get0.into_body(), 1 << 20).await.expect("b"))
            .expect("json");
    assert_eq!(body0["persisted"], false);

    // PUT unknown joint rejected
    let bad = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/hardware/commissioning-scope")
                .header("x-marengo-log-token", TEST_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"joints":["left_wrist_roll"],"confirm_widen":true}"#,
                ))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    // PUT narrow set (from empty → first write is a widen)
    let put_widen_missing = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/hardware/commissioning-scope")
                .header("x-marengo-log-token", TEST_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"joints":["right_shoulder_roll","right_shoulder_pitch"]}"#,
                ))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(put_widen_missing.status(), StatusCode::CONFLICT);

    let put_ok = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/hardware/commissioning-scope")
                .header("x-marengo-log-token", TEST_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"joints":["right_shoulder_roll","right_shoulder_pitch"],"confirm_widen":true}"#,
                ))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(put_ok.status(), StatusCode::OK);
    let put_body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(put_ok.into_body(), 1 << 20)
            .await
            .expect("b"),
    )
    .expect("json");
    assert_eq!(put_body["persisted"], true);
    assert_eq!(put_body["effective"].as_array().expect("arr").len(), 2);

    // Narrow without confirm_widen
    let put_narrow = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/hardware/commissioning-scope")
                .header("x-marengo-log-token", TEST_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"joints":["right_shoulder_roll"]}"#))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(put_narrow.status(), StatusCode::OK);

    // DELETE clears
    let del = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/hardware/commissioning-scope")
                .header("x-marengo-log-token", TEST_TOKEN)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(del.status(), StatusCode::OK);
    let del_body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(del.into_body(), 1 << 20)
            .await
            .expect("b"),
    )
    .expect("json");
    assert_eq!(del_body["persisted"], false);

    std::env::remove_var(TOKEN_ENV);
    std::env::remove_var("MARENGO_ROOT");
    std::env::remove_var("MARENGO_CONFIG_DIR");
    std::env::remove_var("MARENGO_JOINT_SUBSET");
}

#[tokio::test]
async fn command_home_is_gone() {
    let _env = lock_test_env();
    let state = std::sync::Arc::new(AppState::new(std::sync::Arc::new(Bus::default())));
    let app = test_app(state);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/command/home")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(response.status(), StatusCode::GONE);
}
