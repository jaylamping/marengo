//! Original-public HTTP proof: isolated Bus only, no IPC, CAN or installed files.
#![allow(clippy::expect_used)]

use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::{ControlMode, EnableRequest, MitCommandBatch, MitJointCommand};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chappe::Bus;
use marengo_config::CommandJointAllowlist;
use tower::ServiceExt;

const TOKEN: &str = "isolated-gateway-access-fixture";
const CHILD: &str = "MARENGO_ACCESS_PUBLIC_FIXTURE_CHILD";
const TEST: &str =
    "gateway_access_public_test::configured_routes_require_credentials_before_publication";

#[tokio::test]
async fn configured_routes_require_credentials_before_publication() {
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .env("MARENGO_GATEWAY_LOG_TOKEN", TOKEN)
            .env("MARENGO_GATEWAY_OPERATOR_TOKEN", TOKEN)
            .output()
            .expect("isolated fixture child");
        assert!(
            child.status.success(),
            "isolated configured fixture failed:\n{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        return;
    }
    assert_eq!(
        std::env::var("MARENGO_GATEWAY_LOG_TOKEN").as_deref(),
        Ok(TOKEN)
    );
    let enable = EnableRequest {
        enable: true,
        ..Default::default()
    }
    .encode_to_vec();
    let mit = MitCommandBatch {
        mode: ControlMode::GravityComp as i32,
        joints: vec![MitJointCommand {
            name: "right_shoulder_pitch".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
    .encode_to_vec();
    let zero = br#"{"joint":"right_shoulder_pitch","confirm":true,"sign_test_passed":true,"client_id":"isolated-fixture"}"#.to_vec();
    let routes = [
        (
            "/command/enable",
            "robot/enable",
            "application/x-protobuf",
            enable,
        ),
        (
            "/command/testing_mit",
            "robot/testing/mit_command_batch",
            "application/x-protobuf",
            mit,
        ),
        (
            "/command/set_zero",
            "robot/set_zero",
            "application/json",
            zero,
        ),
    ];
    let mut failures = Vec::new();
    let mut cases = 0;
    for (route, topic, kind, body) in &routes {
        for (case, credential, origin, expected) in [
            (
                "missing",
                None,
                "http://localhost:5173",
                StatusCode::UNAUTHORIZED,
            ),
            (
                "wrong",
                Some("different-isolated-fixture"),
                "http://localhost:5173",
                StatusCode::UNAUTHORIZED,
            ),
            (
                "foreign origin",
                Some(TOKEN),
                "https://foreign.example",
                StatusCode::FORBIDDEN,
            ),
        ] {
            cases += 1;
            let bus = Arc::new(Bus::default());
            let mut published = bus.subscribe(topic);
            let state = Arc::new(
                crate::state::AppState::new(Arc::clone(&bus)).with_command_joints(
                    CommandJointAllowlist::from_joints(["right_shoulder_pitch"]),
                ),
            );
            let mut request = Request::builder()
                .method("POST")
                .uri(*route)
                .header("content-type", *kind)
                .header("origin", origin);
            if let Some(value) = credential {
                request = request.header("x-marengo-log-token", value);
            }
            let response = crate::http::router(state, None)
                .oneshot(
                    request
                        .body(Body::from(body.clone()))
                        .expect("fixture request"),
                )
                .await
                .expect("actual router response");
            let did_publish = published.try_recv().is_ok();
            eprintln!(
                "{route} {case}: status={} published={did_publish}",
                response.status()
            );
            if response.status() != expected || did_publish {
                failures.push(format!(
                    "{route} {case}: expected {expected} with no publication"
                ));
            }
        }
    }
    for credential in [None, Some("different-isolated-fixture")] {
        cases += 1;
        let state = Arc::new(crate::state::AppState::new(Arc::new(Bus::default())));
        let mut request = Request::builder()
            .uri("/stream/chappe?topics=logs/structured")
            .header("origin", "http://localhost:5173");
        if let Some(value) = credential {
            request = request.header("x-marengo-log-token", value);
        }
        let response = crate::http::router(state, None)
            .oneshot(request.body(Body::empty()).expect("stream request"))
            .await
            .expect("actual stream response");
        eprintln!(
            "sensitive stream credential={}: status={}",
            credential.is_some(),
            response.status()
        );
        if response.status() != StatusCode::UNAUTHORIZED {
            failures.push("unauthorized sensitive stream admitted".into());
        }
    }
    // Real positive controls retain ordinary telemetry and an authenticated command
    // into this fixture's memory Bus. Neither can reach a physical runtime.
    cases += 1;
    let state = Arc::new(crate::state::AppState::new(Arc::new(Bus::default())));
    let response = crate::http::router(state, None)
        .oneshot(
            Request::builder()
                .uri("/stream/chappe?topics=robot/heartbeat")
                .body(Body::empty())
                .expect("public request"),
        )
        .await
        .expect("public response");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "public telemetry positive control"
    );
    cases += 1;
    let bus = Arc::new(Bus::default());
    let mut published = bus.subscribe("robot/enable");
    let state = Arc::new(crate::state::AppState::new(bus));
    let response = crate::http::router(state, None)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/command/enable")
                .header("content-type", "application/x-protobuf")
                .header("x-marengo-log-token", TOKEN)
                .header("origin", "http://localhost:5173")
                .body(Body::from(routes[0].3.clone()))
                .expect("valid fixture command"),
        )
        .await
        .expect("valid response");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "authenticated fixture positive control"
    );
    assert!(
        published.try_recv().is_ok(),
        "positive command must reach the isolated Bus"
    );
    eprintln!(
        "public fixture: cases={cases} failed_refusals={} positive_controls=2 physical_actions=0",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
