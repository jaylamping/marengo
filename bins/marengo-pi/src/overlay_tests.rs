#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use armee_proto::actuator_command::Payload;
use armee_proto::prost::Message;
use armee_proto::{ActionEvent, Envelope};
use berthier::ControlLoop;
use davout::MemoryBus;
use marengo_config::{load_control_config_from, CommandJointAllowlist};
use tokio::sync::broadcast;

use super::*;

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn test_loop() -> ControlLoop<MemoryBus> {
    ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop")
}

fn allowlist_from_repo() -> CommandJointAllowlist {
    load_command_joint_allowlist_from(repo_root().join("config")).expect("allowlist")
}

fn test_persist_queue_at(install_root: PathBuf) -> (ConfigPersistQueue, Arc<AtomicBool>, Arc<Bus>) {
    let shutdown = Arc::new(AtomicBool::new(false));
    let bus = Arc::new(Bus::new(16));
    let queue = ConfigPersistQueue::spawn(Arc::clone(&bus), Arc::clone(&shutdown), install_root);
    (queue, shutdown, bus)
}

fn test_persist_queue() -> (ConfigPersistQueue, Arc<AtomicBool>, Arc<Bus>) {
    test_persist_queue_at(repo_root())
}

fn test_overlay() -> (ActuatorOverlay, Arc<AtomicBool>) {
    let (persist, shutdown, _bus) = test_persist_queue();
    (
        ActuatorOverlay::new(allowlist_from_repo(), persist),
        shutdown,
    )
}

fn test_overlay_at(install_root: PathBuf) -> (ActuatorOverlay, Arc<AtomicBool>) {
    let (persist, shutdown, _bus) = test_persist_queue_at(install_root);
    (
        ActuatorOverlay::new(allowlist_from_repo(), persist),
        shutdown,
    )
}

fn test_overlay_with_bus_at(install_root: PathBuf) -> (ActuatorOverlay, Arc<AtomicBool>, Arc<Bus>) {
    let (persist, shutdown, bus) = test_persist_queue_at(install_root);
    (
        ActuatorOverlay::new(allowlist_from_repo(), persist),
        shutdown,
        bus,
    )
}

fn copy_profile_to_temp() -> (tempfile::TempDir, PathBuf, String) {
    let root = repo_root();
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_dir = tmp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(root.join("config").join(name), config_dir.join(name)).expect("copy");
    }
    let assets = tmp.path().join("assets/urdf");
    std::fs::create_dir_all(&assets).expect("assets");
    std::fs::copy(
        root.join("assets/urdf/arm_4dof.urdf"),
        assets.join("arm_4dof.urdf"),
    )
    .expect("copy urdf");
    let revision = profile_content_revision(&config_dir).expect("revision");
    (tmp, config_dir, revision)
}

fn limit_patch_op(revision: String) -> OperatorCommand {
    OperatorCommand {
        timestamp_ms: 1,
        session_id: "limit-test".into(),
        operator_id: "test".into(),
        seq: 1,
        command: Some(ActuatorCommand {
            joint: "elbow".into(),
            payload: Some(Payload::LimitPatch(LimitPatchCommand {
                position_lower_rad: 0.1,
                position_upper_rad: 1.4,
                torque_limit_nm: Some(2.0),
                position_soft_lower_rad: Some(0.0),
                position_soft_upper_rad: Some(2.0),
                velocity_max_rad_s: None,
                expected_revision: revision,
            })),
        }),
    }
}

fn tuning_operator(joint: &str, param: &str, value: f64, tier: i32) -> OperatorCommand {
    OperatorCommand {
        timestamp_ms: 1,
        session_id: "sess-test".to_string(),
        operator_id: "bench".to_string(),
        seq: 1,
        command: Some(ActuatorCommand {
            joint: joint.to_string(),
            payload: Some(Payload::Tuning(TuningChange {
                tier,
                param: param.to_string(),
                value,
                persist: tier == TuningTier::ConfigOverlay as i32,
            })),
        }),
    }
}

#[test]
fn runtime_overlay_applies_kp_via_gain_override() {
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = test_loop();
    let op = tuning_operator("elbow", "kp", 88.0, TuningTier::RuntimeMit as i32);
    let outcomes = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect("apply");
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(outcomes[0], OverlayOutcome::Tuning(_)));
    let OverlayOutcome::Tuning(ref event) = outcomes[0] else {
        return;
    };
    assert!((event.after - 88.0).abs() < 1e-9);
    let ov = loop_ctrl.gain_override("elbow").expect("override");
    assert!((ov.kp - 88.0).abs() < 1e-9);
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn runtime_overlay_rejects_pos_vel_torque_ff() {
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = test_loop();
    for param in ["pos", "vel", "torque_ff"] {
        let op = tuning_operator("elbow", param, 1.0, TuningTier::RuntimeMit as i32);
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect_err(param);
        assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    }
    assert!(loop_ctrl.gain_override("elbow").is_none());
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn runtime_overlay_rejects_under_gravity_comp() {
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = test_loop();
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    let op = tuning_operator("elbow", "kp", 88.0, TuningTier::RuntimeMit as i32);
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("gravity comp");
    assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    assert!(loop_ctrl.gain_override("elbow").is_none());
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn runtime_overlay_rejects_negative_kp() {
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = test_loop();
    let op = tuning_operator("elbow", "kp", -1.0, TuningTier::RuntimeMit as i32);
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("negative");
    assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    assert!(loop_ctrl.gain_override("elbow").is_none());
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn runtime_overlay_clamps_kp_to_motor_type_max() {
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = test_loop();
    let op = tuning_operator("elbow", "kp", 600.0, TuningTier::RuntimeMit as i32);
    overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect("apply");
    let ov = loop_ctrl.gain_override("elbow").expect("override");
    // rs02 kp_max is 500 in default config
    assert!(ov.kp <= 500.0 + 1e-9);
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn config_overlay_rejects_over_max_kp_before_persist() {
    let root = repo_root();
    let src = root.join("config");
    let tmp = tempfile::tempdir().expect("tempdir");
    for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
    }
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    let op = tuning_operator(
        "elbow",
        "impedance.kp",
        9999.0,
        TuningTier::ConfigOverlay as i32,
    );
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
        .expect_err("over max");
    assert!(matches!(err, OverlayError::Config(_)));
    let disk = load_control_config_from(tmp.path()).expect("disk");
    assert!(disk.control.joints["elbow"].impedance.kp < 9999.0);
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn config_overlay_queues_persist_and_applies_live() {
    let root = repo_root();
    let src = root.join("config");
    let tmp = tempfile::tempdir().expect("tempdir");
    for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
    }
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    let op = tuning_operator(
        "elbow",
        "impedance.kp",
        33.0,
        TuningTier::ConfigOverlay as i32,
    );
    let outcomes = overlay
        .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
        .expect("apply");
    assert!(outcomes
        .iter()
        .any(|o| matches!(o, OverlayOutcome::Tuning(_))));
    assert!(outcomes.iter().any(|o| {
        matches!(
            o,
            OverlayOutcome::Action(ActionEvent {
                action,
                accepted: true,
                ..
            }) if action == "config_persist"
        )
    }));
    assert!(
        (loop_ctrl.supervisor().control.control.joints["elbow"]
            .impedance
            .kp
            - 33.0)
            .abs()
            < 1e-9,
        "live must apply before disk write completes"
    );
    assert!(
        overlay.persist.wait_idle_for_test(Duration::from_secs(2)),
        "persist worker did not drain"
    );
    let reloaded = load_control_config_from(tmp.path()).expect("reload disk");
    assert!((reloaded.control.joints["elbow"].impedance.kp - 33.0).abs() < 1e-9);
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn persist_queue_coalesces_to_latest_draft() {
    let root = repo_root();
    let src = root.join("config");
    let tmp = tempfile::tempdir().expect("tempdir");
    for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
    }
    let (queue, shutdown, _bus) = test_persist_queue();
    let mut draft = load_control_config_from(tmp.path()).expect("load");
    draft
        .control
        .joints
        .get_mut("elbow")
        .expect("elbow")
        .impedance
        .kp = 11.0;
    queue
        .enqueue(PersistRequest {
            config_dir: tmp.path().to_path_buf(),
            motors: None,
            control: draft.clone(),
            timestamp_ms: 1,
            session_id: "s".into(),
            operator_id: "o".into(),
            joint: "elbow".into(),
            param: "impedance.kp".into(),
        })
        .expect("enqueue1");
    draft
        .control
        .joints
        .get_mut("elbow")
        .expect("elbow")
        .impedance
        .kp = 44.0;
    queue
        .enqueue(PersistRequest {
            config_dir: tmp.path().to_path_buf(),
            motors: None,
            control: draft,
            timestamp_ms: 2,
            session_id: "s".into(),
            operator_id: "o".into(),
            joint: "elbow".into(),
            param: "impedance.kp".into(),
        })
        .expect("enqueue2");
    // At most one pending slot (latest wins) — may already be writing.
    assert!(queue.pending_count_for_test() <= 1);
    assert!(queue.wait_idle_for_test(Duration::from_secs(2)));
    let reloaded = load_control_config_from(tmp.path()).expect("reload");
    assert!((reloaded.control.joints["elbow"].impedance.kp - 44.0).abs() < 1e-9);
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn rejects_unwired_joint_with_not_wired_error() {
    let (mut overlay, shutdown) = test_overlay();
    let mut loop_ctrl = test_loop();
    let op = tuning_operator("left_knee", "kp", 1.0, TuningTier::RuntimeMit as i32);
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("unwired");
    assert!(matches!(err, OverlayError::NotWired(_)));
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn limit_snapshot_marks_wired_joints() {
    let bus = MemoryBus::default();
    let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
    let allowlist = allowlist_from_repo();
    let snap = build_limit_snapshot(&sup, &allowlist, 42);
    let elbow = snap
        .joints
        .iter()
        .find(|j| j.joint == "elbow")
        .expect("elbow");
    assert!(elbow.wired);
    assert!(elbow.kp_max > 0.0);
    assert!(elbow.pos_upper_rad > elbow.pos_lower_rad);
    assert!(elbow.pos_soft_upper_rad >= elbow.pos_soft_lower_rad);
    assert!(elbow.pos_soft_lower_rad >= elbow.pos_lower_rad - 1e-9);
    assert!(elbow.pos_soft_upper_rad <= elbow.pos_upper_rad + 1e-9);
}

#[test]
fn limit_patch_applies_live_and_queues_persist() {
    let root = repo_root();
    let (tmp, config_dir, revision) = copy_profile_to_temp();
    let (mut overlay, shutdown) = test_overlay_at(tmp.path().to_path_buf());
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    let op = limit_patch_op(revision);
    let outcomes = overlay
        .apply_operator_command(&mut loop_ctrl, &config_dir, &op)
        .expect("apply");
    assert!(outcomes.iter().any(|o| {
        matches!(
            o,
            OverlayOutcome::Action(ActionEvent {
                action,
                accepted: true,
                persist_status,
                ..
            }) if action == "limit_patch"
                && *persist_status == PersistStatus::Pending as i32
        )
    }));
    let policy = loop_ctrl
        .supervisor()
        .joint_limit_policy("elbow")
        .expect("policy");
    assert!((policy.hard_upper() - 1.4).abs() < 1e-9);
    assert!(
        overlay.persist.wait_idle_for_test(Duration::from_secs(2)),
        "persist drain"
    );
    let motors = marengo_config::load_motors_config_from(&config_dir).expect("motors");
    let elbow = motors
        .motors
        .iter()
        .find(|m| m.joint == "elbow")
        .expect("elbow");
    assert!((elbow.bench.position_upper_rad - 1.4).abs() < 1e-9);
    shutdown.store(true, Ordering::SeqCst);
}

#[test]
fn limit_patch_rolls_back_live_when_persist_queue_dead() {
    let root = repo_root();
    let (tmp, config_dir, revision) = copy_profile_to_temp();
    let shutdown = Arc::new(AtomicBool::new(true));
    let bus = Arc::new(Bus::new(16));
    let persist = ConfigPersistQueue::spawn(
        Arc::clone(&bus),
        Arc::clone(&shutdown),
        tmp.path().to_path_buf(),
    );
    // Let the worker observe shutdown and drop its wake receiver.
    thread::sleep(Duration::from_millis(50));
    let mut overlay = ActuatorOverlay::new(allowlist_from_repo(), persist);
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    let before = *loop_ctrl
        .supervisor()
        .joint_limit_policy("elbow")
        .expect("policy");
    let urdf_before = loop_ctrl
        .supervisor()
        .urdf_robot()
        .joints
        .iter()
        .find(|j| j.name == "elbow")
        .map(|j| (j.limit.lower, j.limit.upper))
        .expect("elbow urdf");
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &config_dir, &limit_patch_op(revision))
        .expect_err("dead persist queue");
    assert!(matches!(err, OverlayError::PersistQueue(_)));
    let after = *loop_ctrl
        .supervisor()
        .joint_limit_policy("elbow")
        .expect("policy");
    assert_eq!(after.hard_upper(), before.hard_upper());
    assert_eq!(after.hard_lower(), before.hard_lower());
    let urdf_after = loop_ctrl
        .supervisor()
        .urdf_robot()
        .joints
        .iter()
        .find(|j| j.name == "elbow")
        .map(|j| (j.limit.lower, j.limit.upper))
        .expect("elbow urdf");
    assert_eq!(urdf_after, urdf_before);
}

#[test]
fn limit_patch_persist_failure_emits_distinct_failed_action() {
    let root = repo_root();
    let (tmp, config_dir, revision) = copy_profile_to_temp();
    let (mut overlay, shutdown, bus) = test_overlay_with_bus_at(tmp.path().to_path_buf());
    let mut audit = bus.subscribe(TOPIC_AUDIT_ACTION);
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    // Make the profile dir unwritable before apply so enqueue succeeds but write-behind fails.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o555))
            .expect("chmod");
    }
    #[cfg(not(unix))]
    {
        // Best-effort on non-unix: remove a required write target after copy.
        let _ = std::fs::remove_file(config_dir.join("motors.yaml"));
        let _ = std::fs::File::create(config_dir.join("motors.yaml"));
        let mut perms = std::fs::metadata(config_dir.join("motors.yaml"))
            .expect("meta")
            .permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(config_dir.join("motors.yaml"), perms).expect("readonly");
    }
    overlay
        .apply_operator_command(&mut loop_ctrl, &config_dir, &limit_patch_op(revision))
        .expect("apply");
    assert!(
        overlay.persist.wait_idle_for_test(Duration::from_secs(2)),
        "persist drain"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_failed = false;
    while Instant::now() < deadline {
        match audit.try_recv() {
            Ok(bytes) => {
                let Ok(env) = Envelope::decode(bytes.as_slice()) else {
                    continue;
                };
                let Ok(event) = ActionEvent::decode(env.payload.as_slice()) else {
                    continue;
                };
                if event.action == "limit_patch_persist"
                    && !event.accepted
                    && event.persist_status == PersistStatus::Failed as i32
                {
                    saw_failed = true;
                    break;
                }
                // Live Pending ACK must keep action limit_patch (not the persist action).
                if event.action == "limit_patch" {
                    assert!(event.accepted);
                    assert_eq!(event.persist_status, PersistStatus::Pending as i32);
                }
            }
            Err(broadcast::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }
    assert!(
        saw_failed,
        "expected limit_patch_persist Failed ActionEvent"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755));
    }
    shutdown.store(true, Ordering::SeqCst);
}
