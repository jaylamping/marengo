//! `/command/testing_mit` and `/command/enable` hardening through the real router.
#![allow(clippy::expect_used)]

use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::{ControlMode, EnableRequest, Envelope, MitCommandBatch, MitJointCommand};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use chappe::Bus;
use marengo_config::CommandJointAllowlist;
use tower::ServiceExt;

use crate::access::AccessPolicy;
use crate::state::AppState;

const TOKEN: &str = "isolated-command-hardening-operator";
const MIT_TOPIC: &str = "robot/testing/mit_command_batch";
const ENABLE_TOPIC: &str = "robot/enable";

fn fixture(topic: &str) -> (Router, tokio::sync::broadcast::Receiver<Vec<u8>>) {
    let bus = Arc::new(Bus::default());
    let rx = bus.subscribe(topic);
    let state = Arc::new(
        AppState::new(Arc::clone(&bus))
            .with_access(AccessPolicy::operator_fixture(TOKEN).expect("operator grant"))
            .with_command_joints(CommandJointAllowlist::from_joints([
                "right_shoulder_pitch",
                "right_elbow_pitch",
            ])),
    );
    (crate::http::router(state, None), rx)
}

async fn post(app: &Router, uri: &str, body: Vec<u8>) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", format!("Bearer {TOKEN}"))
                .header("content-type", "application/x-protobuf")
                .body(Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn batch(names: &[&str]) -> Vec<u8> {
    MitCommandBatch {
        mode: ControlMode::GravityComp as i32,
        joints: names
            .iter()
            .map(|name| MitJointCommand {
                name: (*name).into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
    .encode_to_vec()
}

fn enable(on: bool) -> Vec<u8> {
    EnableRequest {
        enable: on,
        ..Default::default()
    }
    .encode_to_vec()
}

fn published_names(rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>) -> Vec<String> {
    let bytes = rx.try_recv().expect("published envelope");
    let envelope = Envelope::decode(bytes.as_slice()).expect("envelope");
    assert_eq!(envelope.message_type, "marengo.v1.MitCommandBatch");
    MitCommandBatch::decode(envelope.payload.as_slice())
        .expect("batch")
        .joints
        .into_iter()
        .map(|j| j.name)
        .collect()
}

#[tokio::test]
async fn non_allowlisted_joint_refuses_whole_batch_without_publishing() {
    let (app, mut rx) = fixture(MIT_TOPIC);
    let (status, message) = post(
        &app,
        "/command/testing_mit",
        batch(&["right_shoulder_pitch", "left_knee"]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message, "joint not command-eligible: left_knee");
    assert!(rx.try_recv().is_err(), "nothing may be published");
}

#[tokio::test]
async fn alias_is_rewritten_to_canonical_name_in_published_envelope() {
    let (app, mut rx) = fixture(MIT_TOPIC);
    let (status, _) = post(&app, "/command/testing_mit", batch(&["elbow"])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(published_names(&mut rx), ["right_elbow_pitch"]);
}

#[tokio::test]
async fn wave_joint_segment_is_rewritten_and_rest_kept_verbatim() {
    let (app, mut rx) = fixture(MIT_TOPIC);
    let (status, _) = post(
        &app,
        "/command/testing_mit",
        batch(&["wave:elbow:0.42:0.7:8:0.75"]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        published_names(&mut rx),
        ["wave:right_elbow_pitch:0.42:0.7:8:0.75"]
    );
}

#[tokio::test]
async fn wave_with_non_allowlisted_joint_is_forbidden() {
    let (app, mut rx) = fixture(MIT_TOPIC);
    let (status, message) = post(
        &app,
        "/command/testing_mit",
        batch(&["wave:left_knee:0:1:2:0.5"]),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(message, "joint not command-eligible: left_knee");
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn malformed_wave_names_are_bad_request() {
    for name in [
        "wave:",
        "wave:right_elbow_pitch",
        "wave:right_elbow_pitch:0:1:2",
        "wave:right_elbow_pitch:0:1:2:0.5:extra",
        "wave::0:1:2:0.5",
    ] {
        let (app, mut rx) = fixture(MIT_TOPIC);
        let (status, _) = post(&app, "/command/testing_mit", batch(&[name])).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
        assert!(rx.try_recv().is_err(), "{name}");
    }
}

#[tokio::test]
async fn empty_and_oversized_batches_are_bad_request() {
    let (app, mut rx) = fixture(MIT_TOPIC);
    let (status, message) = post(&app, "/command/testing_mit", batch(&[])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(message, "joints required");
    let (status, _) = post(
        &app,
        "/command/testing_mit",
        batch(&[
            "right_elbow_pitch",
            "right_elbow_pitch",
            "right_elbow_pitch",
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn testing_mit_flood_is_rate_limited() {
    let (app, _rx) = fixture(MIT_TOPIC);
    let mut limited = false;
    for _ in 0..100 {
        let (status, _) = post(&app, "/command/testing_mit", batch(&["elbow"])).await;
        match status {
            StatusCode::OK => {}
            StatusCode::TOO_MANY_REQUESTS => {
                limited = true;
                break;
            }
            other => assert_eq!(other, StatusCode::OK, "unexpected status"),
        }
    }
    assert!(limited, "flood must eventually return 429");
}

#[tokio::test]
async fn testing_mit_burst_covers_compound_playback_without_429() {
    let (app, _rx) = fixture(MIT_TOPIC);
    for _ in 0..40 {
        let (status, _) = post(&app, "/command/testing_mit", batch(&["elbow"])).await;
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn enable_true_flood_is_limited_but_disable_never_is() {
    let (app, mut rx) = fixture(ENABLE_TOPIC);
    let mut limited = false;
    for _ in 0..50 {
        let (status, _) = post(&app, "/command/enable", enable(true)).await;
        match status {
            StatusCode::OK => {}
            StatusCode::TOO_MANY_REQUESTS => {
                limited = true;
                break;
            }
            other => assert_eq!(other, StatusCode::OK, "unexpected status"),
        }
    }
    assert!(limited, "enable=true flood must eventually return 429");
    while rx.try_recv().is_ok() {}
    // Enable bucket is exhausted; disable must still pass every time.
    for i in 0..50 {
        let (status, _) = post(&app, "/command/enable", enable(false)).await;
        assert_eq!(status, StatusCode::OK, "disable request {i}");
        assert!(rx.try_recv().is_ok(), "disable {i} must publish");
    }
}

#[tokio::test]
async fn malformed_bodies_stay_bad_request() {
    let (app, _rx) = fixture(MIT_TOPIC);
    for uri in ["/command/enable", "/command/testing_mit"] {
        let (status, _) = post(&app, uri, vec![0xff]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
    }
}
