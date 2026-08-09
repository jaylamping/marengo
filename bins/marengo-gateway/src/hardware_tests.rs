#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use chappe::Bus;
use tower::ServiceExt;

use crate::config::authorize_config_mutation;
use crate::http;
use crate::logs::lock_test_env;

const TEST_TOKEN: &str = "hardware-test-token";
const TOKEN_ENV: &str = "MARENGO_GATEWAY_LOG_TOKEN";

fn test_app(state: crate::state::SharedState) -> axum::Router {
    http::router(state, None)
}

fn auth_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-marengo-log-token",
        HeaderValue::from_static(TEST_TOKEN),
    );
    headers
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
async fn completeness_is_advisory_and_upload_not_blocked() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let root = marengo_config::resolve_repo_root();
    let tmp = tempfile::tempdir().expect("tmp");
    let assets = tmp.path().join("assets/urdf");
    fs::create_dir_all(assets.join("staging")).expect("staging");
    fs::create_dir_all(assets.join("archive")).expect("archive");
    fs::copy(root.join("assets/urdf/marengo.urdf"), assets.join("marengo.urdf")).expect("urdf");

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
    let db = tmp.path().join("test.db");
    let store = marengo_store::Store::open(&db, tmp.path()).expect("store");
    let logs = crate::logs::LogServices::open(store);
    let state = std::sync::Arc::new(
        crate::state::AppState::new(std::sync::Arc::clone(&bus)).with_logs(logs),
    );
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
    fs::copy(root.join("assets/urdf/marengo.urdf"), assets.join("marengo.urdf")).expect("urdf");

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
    assert_eq!(response.status(), StatusCode::OK);

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

#[test]
fn authorize_config_mutation_matches_log_token() {
    let _env = lock_test_env();
    std::env::set_var(TOKEN_ENV, TEST_TOKEN);
    let headers = auth_headers();
    assert!(authorize_config_mutation(&headers).is_ok());
}
