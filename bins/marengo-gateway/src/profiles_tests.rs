#![allow(clippy::expect_used)]

use crate::action_ack::{encode_action_event_envelope, recv_action_ack, LIVE_LIMIT_PATCH_ACTION};

use super::*;

fn is_live_limit_patch_ack(session_id: &str, event: &ActionEvent) -> bool {
    event.session_id == session_id && event.action == LIVE_LIMIT_PATCH_ACTION
}

fn limits(upper: f64) -> LimitPatch {
    LimitPatch {
        joint: "right_elbow_pitch".to_string(),
        position_lower_rad: -0.1,
        position_upper_rad: upper,
        torque_limit_nm: Some(2.0),
        position_soft_lower_rad: Some(-0.05),
        position_soft_upper_rad: Some(upper - 0.05),
        velocity_max_rad_s: Some(0.8),
    }
}

#[test]
fn resolves_preset_id_or_profile_slug() {
    assert_eq!(resolve_target_slug("bench_4dof"), Some("arm_4dof_right"));
    assert_eq!(
        resolve_target_slug("arm_3dof_right"),
        Some("arm_3dof_right")
    );
    assert_eq!(resolve_target_slug("bench_unknown"), None);
    assert_eq!(resolve_target_slug("../arm_4dof_right"), None);
}

#[test]
fn preview_classifies_add_overwrite_noop_and_unsupported() {
    let source = limits(1.2);
    assert_eq!(classify_preview(None, &source, true), ApplyDecision::Add);
    assert_eq!(
        classify_preview(Some(&limits(1.0)), &source, true),
        ApplyDecision::Overwrite
    );
    assert_eq!(
        classify_preview(Some(&source), &source, true),
        ApplyDecision::Noop
    );
    assert_eq!(
        classify_preview(None, &source, false),
        ApplyDecision::UnsupportedMembership
    );
}

#[test]
fn preview_reads_active_limits_without_writing_target() {
    let root = marengo_config::resolve_repo_root();
    let active = resolve_bringup_dir(&root, "arm_4dof_right").expect("active profile");
    let target = resolve_bringup_dir(&root, "arm_3dof_right").expect("target profile");
    let revision_before = profile_content_revision(&target).expect("revision");
    let request = ApplyActuatorJson {
        target_profile: "bench_3dof".to_string(),
        expected_revision: Some(revision_before.clone()),
        operator_id: "test-operator".to_string(),
        op: ApplyOperation::Preview,
        joint: "right_elbow_pitch".to_string(),
        position_lower_rad: None,
        position_upper_rad: None,
        torque_limit_nm: None,
        position_soft_lower_rad: None,
        position_soft_upper_rad: None,
        velocity_max_rad_s: None,
    };

    let (status, Json(result)) = apply_actuator(&root, &active, request);

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(result.decision, Some(ApplyDecision::UnsupportedMembership));
    assert!(result.after.is_some());
    assert_eq!(
        profile_content_revision(&target).expect("unchanged revision"),
        revision_before
    );
}

#[tokio::test]
async fn profile_snapshot_rejects_non_allowlisted_slug() {
    let result = get_profile_snapshot(Path("../config".to_string())).await;
    assert!(matches!(result, Err(StatusCode::NOT_FOUND)));
}

#[test]
fn inactive_upsert_advances_revision_and_rejects_stale_cas() {
    let root = marengo_config::resolve_repo_root();
    let source = resolve_bringup_dir(&root, "arm_3dof_right").expect("3dof");
    let repo_tmp = tempfile::tempdir().expect("repo temp");
    let config_dir = repo_tmp.path().join("config/bringup/arm_3dof_right");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        std::fs::copy(source.join(name), config_dir.join(name)).expect("copy");
    }
    let robot = marengo_config::load_robot_config_from(&config_dir).expect("robot");
    let urdf_rel = std::path::PathBuf::from(&robot.robot.urdf);
    let urdf_dest = repo_tmp.path().join(&urdf_rel);
    if let Some(parent) = urdf_dest.parent() {
        std::fs::create_dir_all(parent).expect("urdf dir");
    }
    std::fs::copy(root.join(&urdf_rel), &urdf_dest).expect("copy urdf");
    let before = profile_content_revision(&config_dir).expect("rev");
    let patch = LimitPatch {
        joint: "right_shoulder_pitch".to_string(),
        position_lower_rad: -0.5,
        position_upper_rad: 2.5,
        torque_limit_nm: Some(3.0),
        position_soft_lower_rad: Some(-0.4),
        position_soft_upper_rad: Some(2.4),
        velocity_max_rad_s: None,
    };
    let ok =
        upsert_joint_limits(repo_tmp.path(), &config_dir, &patch, Some(&before)).expect("cas ok");
    assert_ne!(ok.revision, before);
    let stale = upsert_joint_limits(repo_tmp.path(), &config_dir, &patch, Some(&before));
    assert!(stale.is_err(), "stale CAS must fail");
}

fn sample_action(
    session_id: &str,
    action: &str,
    accepted: bool,
    persist: ProtoPersistStatus,
) -> ActionEvent {
    ActionEvent {
        timestamp_ms: 1,
        session_id: session_id.to_string(),
        operator_id: "op".to_string(),
        joint: "right_elbow_pitch".to_string(),
        action: action.to_string(),
        revision: 1,
        accepted,
        reject_reason: if accepted {
            String::new()
        } else {
            "rejected".to_string()
        },
        persist_status: persist as i32,
        config_revision: "rev-1".to_string(),
    }
}

#[test]
fn live_ack_matcher_ignores_write_behind_persist_action() {
    let session = "sess-1";
    assert!(is_live_limit_patch_ack(
        session,
        &sample_action(session, "limit_patch", true, ProtoPersistStatus::Pending)
    ));
    assert!(!is_live_limit_patch_ack(
        session,
        &sample_action(
            session,
            "limit_patch_persist",
            false,
            ProtoPersistStatus::Failed
        )
    ));
    assert!(!is_live_limit_patch_ack(
        "other",
        &sample_action(session, "limit_patch", true, ProtoPersistStatus::Pending)
    ));
}

#[tokio::test]
async fn recv_live_ack_skips_persist_failed_until_pending() {
    let bus = std::sync::Arc::new(chappe::Bus::new(64));
    let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
    let mut rx = state.subscribe_envelopes();
    let session = "sess-live";
    state.ingest_runtime_frame(
        TOPIC_AUDIT_ACTION.to_string(),
        encode_action_event_envelope(&sample_action(
            session,
            "limit_patch_persist",
            false,
            ProtoPersistStatus::Failed,
        )),
    );
    state.ingest_runtime_frame(
        TOPIC_AUDIT_ACTION.to_string(),
        encode_action_event_envelope(&sample_action(
            session,
            "limit_patch",
            true,
            ProtoPersistStatus::Pending,
        )),
    );
    let event = timeout(
        Duration::from_secs(1),
        recv_action_ack(
            &mut rx,
            session,
            "right_elbow_pitch",
            "op",
            LIVE_LIMIT_PATCH_ACTION,
        ),
    )
    .await
    .expect("ack");
    assert!(event.accepted);
    assert_eq!(event.action, "limit_patch");
    assert_eq!(event.persist_status, ProtoPersistStatus::Pending as i32);
}

#[tokio::test]
async fn recv_live_ack_surfaces_live_reject() {
    let bus = std::sync::Arc::new(chappe::Bus::new(64));
    let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
    let mut rx = state.subscribe_envelopes();
    let session = "sess-reject";
    state.ingest_runtime_frame(
        TOPIC_AUDIT_ACTION.to_string(),
        encode_action_event_envelope(&sample_action(
            session,
            "limit_patch",
            false,
            ProtoPersistStatus::NotApplicable,
        )),
    );
    let event = timeout(
        Duration::from_secs(1),
        recv_action_ack(
            &mut rx,
            session,
            "right_elbow_pitch",
            "op",
            LIVE_LIMIT_PATCH_ACTION,
        ),
    )
    .await
    .expect("ack");
    assert!(!event.accepted);
    assert_eq!(event.reject_reason, "rejected");
}

#[tokio::test]
async fn recv_live_ack_times_out_without_matching_session() {
    let bus = std::sync::Arc::new(chappe::Bus::new(64));
    let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
    let mut rx = state.subscribe_envelopes();
    state.ingest_runtime_frame(
        TOPIC_AUDIT_ACTION.to_string(),
        encode_action_event_envelope(&sample_action(
            "other-session",
            "limit_patch",
            true,
            ProtoPersistStatus::Pending,
        )),
    );
    let timed_out = timeout(
        Duration::from_millis(50),
        recv_action_ack(
            &mut rx,
            "wanted-session",
            "right_elbow_pitch",
            "op",
            LIVE_LIMIT_PATCH_ACTION,
        ),
    )
    .await;
    assert!(
        timed_out.is_err(),
        "mismatched session must not satisfy wait"
    );
}
