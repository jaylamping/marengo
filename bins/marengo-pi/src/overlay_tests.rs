#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use armee_proto::actuator_command::Payload;
use armee_proto::prost::Message;
use armee_proto::{ActionEvent, ActuatorLimitSnapshot, Envelope};
use berthier::{ControlLoop, ControlMode};
use davout::MemoryBus;
use marengo_config::{load_control_config_from, load_motors_config_from, CommandJointAllowlist};
use tokio::sync::broadcast;

use super::*;

use crate::limit_persist::{PersistDrainStatus, PersistTestHooks};

const PERSIST_BOUND: Duration = Duration::from_secs(2);

struct ReleasePersistGate(Sender<()>);

impl Drop for ReleasePersistGate {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn test_loop() -> ControlLoop<MemoryBus> {
    ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop")
}

fn allowlist_from_repo() -> CommandJointAllowlist {
    load_command_joint_allowlist_from(repo_root().join("config")).expect("allowlist")
}

fn test_persist_queue_at(install_root: PathBuf) -> (ConfigPersistQueue, Arc<Bus>) {
    let bus = Arc::new(Bus::new(16));
    let queue = ConfigPersistQueue::spawn(Arc::clone(&bus), install_root);
    (queue, bus)
}

fn test_persist_queue() -> (ConfigPersistQueue, Arc<Bus>) {
    test_persist_queue_at(repo_root())
}

fn test_overlay() -> ActuatorOverlay {
    let (persist, _bus) = test_persist_queue();
    ActuatorOverlay::new(allowlist_from_repo(), persist)
}

fn test_overlay_at(install_root: PathBuf) -> ActuatorOverlay {
    let (persist, _bus) = test_persist_queue_at(install_root);
    ActuatorOverlay::new(allowlist_from_repo(), persist)
}

fn test_overlay_with_bus_at(install_root: PathBuf) -> (ActuatorOverlay, Arc<Bus>) {
    let (persist, bus) = test_persist_queue_at(install_root);
    (ActuatorOverlay::new(allowlist_from_repo(), persist), bus)
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
        root.join("assets/urdf/marengo.urdf"),
        assets.join("marengo.urdf"),
    )
    .expect("copy urdf");
    let revision = profile_content_revision(&config_dir).expect("revision");
    (tmp, config_dir, revision)
}

/// B13 (P-armee-proto-04): `seq` is deprecated (always 1, never read). Fixtures
/// keep writing it so envelope bytes match production during the window.
#[allow(deprecated)]
fn limit_patch_op(revision: String) -> OperatorCommand {
    OperatorCommand {
        timestamp_ms: 1,
        session_id: "limit-test".into(),
        operator_id: "test".into(),
        seq: 1,
        command: Some(ActuatorCommand {
            joint: "right_elbow_pitch".into(),
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

#[allow(deprecated)]
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
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    loop_ctrl.set_control_mode(ControlMode::Impedance);
    let op = tuning_operator(
        "right_elbow_pitch",
        "kp",
        88.0,
        TuningTier::RuntimeMit as i32,
    );
    let outcomes = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect("apply");
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(outcomes[0], OverlayOutcome::Tuning(_)));
    let OverlayOutcome::Tuning(ref event) = outcomes[0] else {
        return;
    };
    assert!((event.after - 88.0).abs() < 1e-9);
    let ov = loop_ctrl
        .gain_override("right_elbow_pitch")
        .expect("override");
    assert!((ov.kp - 88.0).abs() < 1e-9);
}

#[test]
fn runtime_overlay_rejects_pos_vel_torque_ff() {
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    for param in ["pos", "vel", "torque_ff"] {
        let op = tuning_operator(
            "right_elbow_pitch",
            param,
            1.0,
            TuningTier::RuntimeMit as i32,
        );
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect_err(param);
        assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    }
    assert!(loop_ctrl.gain_override("right_elbow_pitch").is_none());
}

#[test]
fn runtime_overlay_rejects_under_gravity_comp() {
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    let op = tuning_operator(
        "right_elbow_pitch",
        "kp",
        88.0,
        TuningTier::RuntimeMit as i32,
    );
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("gravity comp");
    assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    assert!(loop_ctrl.gain_override("right_elbow_pitch").is_none());
}

#[test]
fn runtime_overlay_rejects_under_disabled() {
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    let op = tuning_operator(
        "right_elbow_pitch",
        "kp",
        88.0,
        TuningTier::RuntimeMit as i32,
    );
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("disabled");
    assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    assert!(loop_ctrl.gain_override("right_elbow_pitch").is_none());
}

#[test]
fn runtime_overlay_rejects_negative_kp() {
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    loop_ctrl.set_control_mode(ControlMode::Impedance);
    let op = tuning_operator(
        "right_elbow_pitch",
        "kp",
        -1.0,
        TuningTier::RuntimeMit as i32,
    );
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("negative");
    assert!(matches!(err, OverlayError::UnsupportedParam(_)));
    assert!(loop_ctrl.gain_override("right_elbow_pitch").is_none());
}

#[test]
fn runtime_overlay_clamps_kp_to_motor_type_max() {
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    loop_ctrl.set_control_mode(ControlMode::Impedance);
    let op = tuning_operator(
        "right_elbow_pitch",
        "kp",
        600.0,
        TuningTier::RuntimeMit as i32,
    );
    overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect("apply");
    let ov = loop_ctrl
        .gain_override("right_elbow_pitch")
        .expect("override");
    // rs02 kp_max is 500 in default config
    assert!(ov.kp <= 500.0 + 1e-9);
}

#[test]
fn config_overlay_rejects_over_max_kp_before_persist() {
    let root = repo_root();
    let src = root.join("config");
    let tmp = tempfile::tempdir().expect("tempdir");
    for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
    }
    let mut overlay = test_overlay();
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    let op = tuning_operator(
        "right_elbow_pitch",
        "impedance.kp",
        9999.0,
        TuningTier::ConfigOverlay as i32,
    );
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
        .expect_err("over max");
    assert!(matches!(err, OverlayError::Config(_)));
    let disk = load_control_config_from(tmp.path()).expect("disk");
    assert!(disk.control.joints["right_elbow_pitch"].impedance.kp < 9999.0);
}

#[test]
fn config_overlay_rejects_negative_friction_before_live_or_disk_mutation() {
    for param in ["friction.fv", "friction.k"] {
        for persist in [false, true] {
            let (tmp, config_dir, _) = copy_profile_to_temp();
            let mut overlay = test_overlay_at(tmp.path().to_path_buf());
            let mut loop_ctrl = test_loop();
            let before_live = format!("{:?}", loop_ctrl.supervisor().control);
            let before_disk =
                std::fs::read(config_dir.join("control.yaml")).expect("read disk config");
            let mut op = tuning_operator(
                "right_elbow_pitch",
                param,
                -1.0,
                TuningTier::ConfigOverlay as i32,
            );
            if let Some(ActuatorCommand {
                payload: Some(Payload::Tuning(ref mut tuning)),
                ..
            }) = op.command
            {
                tuning.persist = persist;
            }
            let outcome = overlay.apply_operator_command(&mut loop_ctrl, &config_dir, &op);
            assert!(
                outcome.is_err(),
                "{param} persist={persist} admitted negative friction"
            );
            assert_eq!(
                format!("{:?}", loop_ctrl.supervisor().control),
                before_live,
                "rejected tuning must leave the installed policy intact"
            );
            assert!(overlay.persist.wait_idle(Duration::from_secs(2)));
            assert_eq!(
                std::fs::read(config_dir.join("control.yaml")).expect("read after"),
                before_disk,
                "rejected tuning must not enqueue a durable write"
            );
        }
    }
}

#[test]
fn config_overlay_queues_persist_and_applies_live() {
    let root = repo_root();
    let src = root.join("config");
    let tmp = tempfile::tempdir().expect("tempdir");
    for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
    }
    let mut overlay = test_overlay();
    let mut loop_ctrl = ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
    let op = tuning_operator(
        "right_elbow_pitch",
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
        (loop_ctrl.supervisor().control.control.joints["right_elbow_pitch"]
            .impedance
            .kp
            - 33.0)
            .abs()
            < 1e-9,
        "live must apply before disk write completes"
    );
    assert!(
        overlay.persist.wait_idle(Duration::from_secs(2)),
        "persist worker did not drain"
    );
    let reloaded = load_control_config_from(tmp.path()).expect("reload disk");
    assert!((reloaded.control.joints["right_elbow_pitch"].impedance.kp - 33.0).abs() < 1e-9);
}

#[test]
fn persist_queue_coalesces_to_latest_draft() {
    // G04 regression: a pending limit_patch (motors: Some) coalesced with a
    // later ConfigOverlay draft (motors: None) must preserve the queued hard
    // bounds via carry_pending_limit_patch. The old replace-with-None dropped
    // the motors.yaml + URDF write while the live config kept the bounds.
    let (tmp, config_dir, _) = copy_profile_to_temp();
    let bus = Arc::new(Bus::new(16));
    let mut audit = bus.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleasePersistGate(release_tx.clone());
    let gate = Mutex::new(release_rx);
    let timed_out = Arc::new(AtomicBool::new(false));
    let hook_timed_out = Arc::clone(&timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&bus),
        tmp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |request| {
                if request.session_id == "coalesce-first" {
                    let _ = entered_tx.send(());
                    if gate
                        .lock()
                        .expect("first writer gate")
                        .recv_timeout(PERSIST_BOUND)
                        .is_err()
                    {
                        hook_timed_out.store(true, Ordering::SeqCst);
                    }
                }
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    // First: an ordinary control-only draft, gated while in flight so the
    // next two requests meet in the not-yet-started slot and must coalesce.
    let mut first_control = load_control_config_from(&config_dir).expect("actual source draft");
    first_control
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("elbow")
        .impedance
        .kp = 11.0;
    queue
        .enqueue(PersistRequest {
            config_dir: config_dir.clone(),
            motors: None,
            control: first_control,
            timestamp_ms: 1,
            session_id: "coalesce-first".into(),
            operator_id: "coalesce-regression".into(),
            joint: "right_elbow_pitch".into(),
            param: "impedance.kp".into(),
        })
        .expect("accept actual first draft");
    entered_rx
        .recv_timeout(PERSIST_BOUND)
        .expect("first is really in flight before later drafts");
    // Pending limit patch carrying hard bounds plus soft limits.
    let mut patch_motors = load_motors_config_from(&config_dir).expect("motors fixture");
    patch_motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == "right_elbow_pitch")
        .expect("fixture motor")
        .bench
        .position_lower_rad = -1.23;
    patch_motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == "right_elbow_pitch")
        .expect("fixture motor")
        .bench
        .position_upper_rad = 2.34;
    let mut patch_control = load_control_config_from(&config_dir).expect("control fixture");
    let patch_entry = patch_control
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("fixture control");
    patch_entry.position_soft_lower_rad = Some(-1.1);
    patch_entry.position_soft_upper_rad = Some(2.2);
    queue
        .enqueue(PersistRequest {
            config_dir: config_dir.clone(),
            motors: Some(patch_motors),
            control: patch_control,
            timestamp_ms: 2,
            session_id: "pending-limit".into(),
            operator_id: "coalesce-regression".into(),
            joint: "right_elbow_pitch".into(),
            param: "limit_patch".into(),
        })
        .expect("accept pending limit patch");
    // Later ConfigOverlay draft: control-only with a newer kp.
    let mut latest_control = load_control_config_from(&config_dir).expect("actual source draft");
    latest_control
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("elbow")
        .impedance
        .kp = 44.0;
    queue
        .enqueue(PersistRequest {
            config_dir: config_dir.clone(),
            motors: None,
            control: latest_control,
            timestamp_ms: 3,
            session_id: "coalesce-latest".into(),
            operator_id: "coalesce-regression".into(),
            joint: "right_elbow_pitch".into(),
            param: "impedance.kp".into(),
        })
        .expect("accept actual latest draft");
    let _ = release_tx.send(());
    let drain = queue.close_and_drain(PERSIST_BOUND);
    drop(queue);
    exited_rx
        .recv_timeout(PERSIST_BOUND)
        .expect("actual coalescing worker returned");
    let mut actions = Vec::new();
    while let Ok(bytes) = audit.try_recv() {
        let envelope = Envelope::decode(bytes.as_slice()).expect("actual terminal envelope");
        actions.push(
            ActionEvent::decode(envelope.payload.as_slice()).expect("actual terminal action"),
        );
    }
    println!(
        "coalesced pending limit_patch into latest draft: actions={actions:?}, drain={drain:?}"
    );
    assert!(
        !timed_out.load(Ordering::SeqCst),
        "first writer explicitly released"
    );
    assert_eq!(drain.status, PersistDrainStatus::Complete);
    assert!(drain.worker_terminated);
    assert_eq!(drain.successful_writes, 2);
    assert_eq!(drain.failed_writes, 0);
    assert_eq!(drain.publication_failures, 0);
    assert_eq!(drain.coalesced_requests, 1);
    assert!(drain.is_idle());
    assert_eq!(
        actions
            .iter()
            .map(|event| event.session_id.as_str())
            .collect::<Vec<_>>(),
        ["coalesce-first", "coalesce-latest"],
        "only actually retained writes publish completion"
    );
    // The discriminating G04 assertions: the queued hard bounds and soft
    // limits survive coalescing into the control-only draft, which keeps its
    // own newer kp.
    let disk_motor = load_motors_config_from(&config_dir)
        .expect("actual final disk motors")
        .motors
        .into_iter()
        .find(|motor| motor.joint == "right_elbow_pitch")
        .expect("actual final disk elbow");
    assert_eq!(disk_motor.bench.position_lower_rad, -1.23);
    assert_eq!(disk_motor.bench.position_upper_rad, 2.34);
    let disk_control = load_control_config_from(&config_dir)
        .expect("actual final disk control")
        .control
        .joints["right_elbow_pitch"]
        .clone();
    assert_eq!(disk_control.position_soft_lower_rad, Some(-1.1));
    assert_eq!(disk_control.position_soft_upper_rad, Some(2.2));
    assert_eq!(disk_control.impedance.kp, 44.0);
}

#[test]
fn rejects_unwired_joint_with_not_wired_error() {
    let mut overlay = test_overlay();
    let mut loop_ctrl = test_loop();
    let op = tuning_operator("left_knee", "kp", 1.0, TuningTier::RuntimeMit as i32);
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("unwired");
    assert!(matches!(err, OverlayError::NotWired(_)));
}

#[test]
fn limit_patch_refuses_empty_expected_revision() {
    let (tmp, config_dir, _) = copy_profile_to_temp();
    let mut overlay = test_overlay_at(tmp.path().to_path_buf());
    let mut loop_ctrl =
        ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop");

    let error = overlay
        .apply_operator_command(&mut loop_ctrl, &config_dir, &limit_patch_op(String::new()))
        .expect_err("empty revision must not bypass compare-and-swap");

    assert!(matches!(error, OverlayError::Config(_)));
    assert!(error.to_string().contains("expected_revision is required"));
}

#[test]
fn limit_patch_refuses_stale_expected_revision() {
    let (tmp, config_dir, _) = copy_profile_to_temp();
    let mut overlay = test_overlay_at(tmp.path().to_path_buf());
    let mut loop_ctrl =
        ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop");

    let error = overlay
        .apply_operator_command(
            &mut loop_ctrl,
            &config_dir,
            &limit_patch_op("stale".to_string()),
        )
        .expect_err("stale revision must not apply");

    assert!(matches!(error, OverlayError::Config(_)));
    assert!(error.to_string().contains("profile revision mismatch"));
}
#[test]
/// B13 (P-armee-proto-04): reads deprecated `wired` until the reserve step.
#[allow(deprecated)]
fn limit_snapshot_marks_wired_joints() {
    let bus = MemoryBus::default();
    let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
    let allowlist = allowlist_from_repo();
    let snap = build_limit_snapshot(&sup, &allowlist, 42);
    let right_elbow_pitch = snap
        .joints
        .iter()
        .find(|j| j.joint == "right_elbow_pitch")
        .expect("right_elbow_pitch");
    assert!(right_elbow_pitch.wired);
    assert!(right_elbow_pitch.kp_max > 0.0);
    assert!(right_elbow_pitch.pos_upper_rad > right_elbow_pitch.pos_lower_rad);
    assert!(right_elbow_pitch.pos_soft_upper_rad >= right_elbow_pitch.pos_soft_lower_rad);
    assert!(right_elbow_pitch.pos_soft_lower_rad >= right_elbow_pitch.pos_lower_rad - 1e-9);
    assert!(right_elbow_pitch.pos_soft_upper_rad <= right_elbow_pitch.pos_upper_rad + 1e-9);
}

#[test]
fn limit_patch_applies_live_and_queues_persist() {
    let root = repo_root();
    let (tmp, config_dir, revision) = copy_profile_to_temp();
    let mut overlay = test_overlay_at(tmp.path().to_path_buf());
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
        .joint_limit_policy("right_elbow_pitch")
        .expect("policy");
    assert!((policy.hard_upper() - 1.4).abs() < 1e-9);
    assert!(
        overlay.persist.wait_idle(Duration::from_secs(2)),
        "persist drain"
    );
    let motors = marengo_config::load_motors_config_from(&config_dir).expect("motors");
    let right_elbow_pitch = motors
        .motors
        .iter()
        .find(|m| m.joint == "right_elbow_pitch")
        .expect("right_elbow_pitch");
    assert!((right_elbow_pitch.bench.position_upper_rad - 1.4).abs() < 1e-9);
}

#[test]
fn limit_patch_after_owner_exit_rejects_closed_admission_without_live_mutation() {
    // This is a candidate-before-cutover regression on the real extracted
    // owner lifecycle, replacing the old flag=true +50ms dead-worker recipe.
    // Neighboring no-exit and exit cases use the same valid real transaction.
    for owner_exited in [false, true] {
        let (tmp, config_dir, revision) = copy_profile_to_temp();
        let shutdown = Arc::new(AtomicBool::new(false));
        let bus = Arc::new(Bus::new(16));
        let mut audit = bus.subscribe(TOPIC_AUDIT_ACTION);
        let (exited_tx, exited_rx) = std::sync::mpsc::channel();
        let persist = ConfigPersistQueue::spawn_with_test_hooks(
            Arc::clone(&bus),
            tmp.path().to_path_buf(),
            crate::limit_persist::PersistTestHooks {
                on_worker_exit: Some(Arc::new(move || {
                    let _ = exited_tx.send(());
                })),
                ..crate::limit_persist::PersistTestHooks::default()
            },
        );
        let mut overlay = ActuatorOverlay::new(
            load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
            persist,
        );
        let mut loop_ctrl =
            ControlLoop::from_repo(tmp.path(), MemoryBus::default(), 200, 50).expect("real loop");
        let exit_outcome = if owner_exited {
            shutdown.store(true, Ordering::SeqCst);
            Some(crate::finish_owner_shutdown(
                &mut loop_ctrl,
                &overlay,
                true,
                Duration::from_secs(2),
                None,
            ))
        } else {
            None
        };
        let before = *loop_ctrl
            .supervisor()
            .joint_limit_policy("right_elbow_pitch")
            .expect("original full policy");
        let motors_before = format!("{:?}", loop_ctrl.supervisor().motors);
        let control_before = format!("{:?}", loop_ctrl.supervisor().control);
        let urdf_before = format!("{:?}", loop_ctrl.supervisor().urdf_robot());
        let original_bytes = [
            config_dir.join("motors.yaml"),
            config_dir.join("control.yaml"),
            tmp.path().join("assets/urdf/marengo.urdf"),
        ]
        .map(|path| std::fs::read(path).expect("original durable resource"));
        let mut op = limit_patch_op(revision);
        if let Some(ActuatorCommand {
            payload: Some(Payload::LimitPatch(ref mut patch)),
            ..
        }) = op.command
        {
            // Expand only the exclusive model fixture so missing rollback also
            // changes real model state. Torque remains lowered, velocity unset.
            patch.position_upper_rad = 3.5;
        }
        let result = overlay.apply_operator_command(&mut loop_ctrl, &config_dir, &op);
        let idle = overlay.persist.wait_idle(Duration::from_secs(2));
        drop(overlay);
        exited_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("actual worker returned after closure/publication");
        let mut actions = Vec::new();
        while let Ok(bytes) = audit.try_recv() {
            let envelope = Envelope::decode(bytes.as_slice()).expect("actual worker envelope");
            actions.push(
                ActionEvent::decode(envelope.payload.as_slice()).expect("actual worker terminal"),
            );
        }
        let disk =
            marengo_config::load_motors_config_from(&config_dir).expect("actual disk motors");
        let disk_upper = disk
            .motors
            .iter()
            .find(|motor| motor.joint == "right_elbow_pitch")
            .expect("actual disk elbow")
            .bench
            .position_upper_rad;
        println!(
            "owner_exited={owner_exited}, exit={exit_outcome:?}, patch={result:?}, disk_upper={disk_upper}, terminal={actions:?}"
        );
        assert!(idle, "actual retained writes/publication completed");

        if !owner_exited {
            let outcomes = result.expect("neighboring valid patch reaches actual live apply");
            assert!(outcomes.iter().any(|outcome| matches!(
                outcome,
                OverlayOutcome::Action(event) if event.action == "limit_patch"
                    && event.accepted && event.persist_status == PersistStatus::Pending as i32
            )));
            assert_eq!(disk_upper, 3.5);
            assert_eq!(actions.len(), 1);
            let event = &actions[0];
            assert_eq!(event.action, "limit_patch_persist");
            assert_eq!(event.session_id, "limit-test");
            assert_eq!(event.operator_id, "test");
            assert_eq!(event.joint, "right_elbow_pitch");
            assert!(event.accepted);
            assert_eq!(event.persist_status, PersistStatus::Durable as i32);
            assert!(!event.config_revision.is_empty());
            let loaded = Supervisor::from_repo(tmp.path(), MemoryBus::default())
                .expect("reload actual written model and policy");
            assert_eq!(
                loaded
                    .urdf_robot()
                    .joints
                    .iter()
                    .find(|joint| joint.name == "right_elbow_pitch")
                    .expect("actual disk model elbow")
                    .limit
                    .upper,
                3.5
            );
            continue;
        }

        let exit = exit_outcome.expect("actual owner exit path reached");
        assert!(exit.persist_idle);
        assert!(matches!(
            exit.stop,
            crate::ExitStopOutcome::Attempted { result: Ok(()), .. }
        ));
        // Decisive existing-interface assertion after real worker cleanup:
        // returning from owner exit must not leave durable admission open.
        assert!(
            matches!(result, Err(OverlayError::PersistQueue(_))),
            "owner exit must close the actual persist queue before a later limit patch"
        );
        assert_eq!(
            *loop_ctrl
                .supervisor()
                .joint_limit_policy("right_elbow_pitch")
                .expect("after rejected patch policy"),
            before
        );
        assert_eq!(
            format!("{:?}", loop_ctrl.supervisor().motors),
            motors_before
        );
        assert_eq!(
            format!("{:?}", loop_ctrl.supervisor().control),
            control_before
        );
        assert_eq!(
            format!("{:?}", loop_ctrl.supervisor().urdf_robot()),
            urdf_before
        );
        assert_eq!(
            [
                config_dir.join("motors.yaml"),
                config_dir.join("control.yaml"),
                tmp.path().join("assets/urdf/marengo.urdf"),
            ]
            .map(|path| std::fs::read(path).expect("rejected request durable bytes")),
            original_bytes
        );
        assert!(
            actions.is_empty(),
            "rejected work has no false durable publication"
        );
    }
}

#[test]
fn limit_patch_persist_failure_emits_distinct_failed_action() {
    let (tmp, config_dir, revision) = copy_profile_to_temp();
    let original_bytes = [
        config_dir.join("motors.yaml"),
        config_dir.join("control.yaml"),
    ]
    .map(|path| std::fs::read(path).expect("original copied YAML"));
    let original_model =
        std::fs::read(tmp.path().join("assets/urdf/marengo.urdf")).expect("original copied model");
    let bus = Arc::new(Bus::new(16));
    let mut audit = bus.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleasePersistGate(release_tx.clone());
    let gate = Mutex::new(release_rx);
    let timed_out = Arc::new(AtomicBool::new(false));
    let hook_timed_out = Arc::clone(&timed_out);
    let persist = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&bus),
        tmp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |_| {
                let _ = entered_tx.send(());
                if gate
                    .lock()
                    .expect("real admitted writer gate")
                    .recv_timeout(PERSIST_BOUND)
                    .is_err()
                {
                    hook_timed_out.store(true, Ordering::SeqCst);
                }
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let mut overlay = ActuatorOverlay::new(
        load_command_joint_allowlist_from(&config_dir).expect("actual copied allowlist"),
        persist,
    );
    let mut loop_ctrl = ControlLoop::from_repo(tmp.path(), MemoryBus::default(), 200, 50)
        .expect("actual copied owner");
    let outcomes = overlay
        .apply_operator_command(&mut loop_ctrl, &config_dir, &limit_patch_op(revision))
        .expect("valid revision and live patch admitted before I/O failure");
    entered_rx
        .recv_timeout(PERSIST_BOUND)
        .expect("actual accepted patch reached BeforeWrite");
    // Every path is inside this exclusive TempDir. A regular-file parent makes
    // the real worker fail on every OS/root privilege level, after admission.
    let saved_dir = tmp.path().join("config-before-write");
    assert_eq!(config_dir.parent(), Some(tmp.path()));
    std::fs::rename(&config_dir, &saved_dir).expect("preserve exclusive original config");
    std::fs::write(&config_dir, b"regular-file parent blocker").expect("actual filesystem blocker");
    let _ = release_tx.send(());
    let drain = overlay.close_persist_and_drain(PERSIST_BOUND);
    drop(overlay);
    exited_rx
        .recv_timeout(PERSIST_BOUND)
        .expect("actual failed writer returned");

    let bytes = audit
        .try_recv()
        .expect("actual terminal Failed action after writer return");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual failure envelope");
    let event = ActionEvent::decode(envelope.payload.as_slice()).expect("actual failure action");
    println!("valid Pending outcomes={outcomes:?}, actual Failed={event:?}, drain={drain:?}");
    assert!(!timed_out.load(Ordering::SeqCst));
    assert!(outcomes.iter().any(|outcome| matches!(
        outcome, OverlayOutcome::Action(event) if event.action == "limit_patch" && event.accepted
            && event.persist_status == PersistStatus::Pending as i32
    )));
    assert_eq!(drain.status, PersistDrainStatus::CompletedWithFailures);
    assert!(drain.worker_terminated);
    assert!(drain.is_idle());
    assert_eq!(drain.successful_writes, 0);
    assert_eq!(drain.failed_writes, 1);
    assert_eq!(drain.publication_failures, 0);
    assert_eq!(event.timestamp_ms, 1);
    assert_eq!(event.session_id, "limit-test");
    assert_eq!(event.operator_id, "test");
    assert_eq!(event.joint, "right_elbow_pitch");
    assert_eq!(event.action, "limit_patch_persist");
    assert!(!event.accepted);
    assert_eq!(event.persist_status, PersistStatus::Failed as i32);
    assert!(
        !event.reject_reason.is_empty(),
        "real filesystem failure carries its diagnostic"
    );
    assert!(event.config_revision.is_empty());
    assert!(
        matches!(audit.try_recv(), Err(broadcast::error::TryRecvError::Empty)),
        "no false Durable completion"
    );
    assert_eq!(
        std::fs::read(&config_dir).expect("preserved blocker"),
        b"regular-file parent blocker"
    );
    assert_eq!(
        [
            saved_dir.join("motors.yaml"),
            saved_dir.join("control.yaml")
        ]
        .map(|path| std::fs::read(path).expect("preserved original YAML")),
        original_bytes
    );
    assert_eq!(
        std::fs::read(tmp.path().join("assets/urdf/marengo.urdf")).expect("preserved model"),
        original_model
    );
}

#[test]
/// B13 (P-armee-proto-04): reads deprecated `wired` until the reserve step.
#[allow(deprecated)]
fn limit_patch_publishes_actuator_limits_before_chappe_tick() {
    // Regression: Consul refresh after Durable Set Limits must read the live
    // ActuatorLimitSnapshot, not wait up to 1/chappe_state_hz for maybe_publish_limits.
    let (tmp, config_dir, revision) = copy_profile_to_temp();
    let (mut overlay, bus) = test_overlay_with_bus_at(tmp.path().to_path_buf());
    let mut limits_rx = bus.subscribe(TOPIC_ACTUATOR_LIMITS);
    let mut cmd_rx = bus.subscribe(TOPIC_ACTUATOR_COMMAND);
    let mut loop_ctrl =
        ControlLoop::from_repo(tmp.path(), MemoryBus::default(), 200, 50).expect("loop");

    let op = limit_patch_op(revision);
    bus.publish(
        TOPIC_ACTUATOR_COMMAND,
        "marengo-gateway",
        "marengo.v1.OperatorCommand",
        &op,
    )
    .expect("actual Chappe limit_patch envelope");

    overlay.drain_commands_until_shutdown(
        &mut loop_ctrl,
        &config_dir,
        &bus,
        &mut cmd_rx,
        &AtomicBool::new(false),
    );
    // Capture immediately, before any control/telemetry tick or storage wait.
    // A delayed publish cannot satisfy this observation after the fact.
    let immediate = limits_rx.try_recv();
    let drain = overlay.close_persist_and_drain(PERSIST_BOUND);
    drop(overlay);
    assert_eq!(drain.status, PersistDrainStatus::Complete);
    assert!(drain.worker_terminated);
    let bytes = immediate.expect("synchronous limit_patch must publish limits before returning");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual immediate snapshot envelope");
    let snapshot = ActuatorLimitSnapshot::decode(envelope.payload.as_slice())
        .expect("actual immediate snapshot");
    let elbow = snapshot
        .joints
        .iter()
        .find(|joint| joint.joint == "right_elbow_pitch")
        .expect("actual elbow bounds");
    assert!(elbow.wired);
    assert_eq!(elbow.pos_lower_rad, 0.1);
    assert_eq!(elbow.pos_upper_rad, 1.4);
    assert_eq!(elbow.tau_ff_max_nm, 2.0);
    assert!(elbow.pos_soft_lower_rad >= elbow.pos_lower_rad);
    assert!(elbow.pos_soft_upper_rad <= elbow.pos_upper_rad);
    assert_eq!(
        loop_ctrl.tick_count(),
        0,
        "no telemetry/control tick supplied the snapshot"
    );
    assert_eq!(snapshot.timestamp_ms, 1);
}

#[allow(deprecated)]
fn motion_operator(payload: Payload) -> OperatorCommand {
    OperatorCommand {
        timestamp_ms: 1,
        session_id: "sess-test".to_string(),
        operator_id: "bench".to_string(),
        seq: 1,
        command: Some(ActuatorCommand {
            joint: "right_elbow_pitch".to_string(),
            payload: Some(payload),
        }),
    }
}

/// L-armee-proto-06: the OperatorCommand motion arms stay inert end to end
/// even when a raw Chappe publisher bypasses the gateway's 400.
#[test]
fn operator_motion_payloads_are_rejected_without_touching_the_controller() {
    use armee_proto::{EnableChange, HoldCommand, JogCommand, ModeChange, PresetCommand};

    let payloads = [
        Payload::Enable(EnableChange { enable: true }),
        Payload::Mode(ModeChange {
            mode: armee_proto::ControlMode::Position as i32,
        }),
        Payload::Jog(JogCommand { delta_rad: 0.1 }),
        Payload::Hold(HoldCommand {
            engage: true,
            position_rad: 0.2,
        }),
        Payload::Preset(PresetCommand {
            preset_id: "home".to_string(),
        }),
    ];
    for payload in payloads {
        let mut overlay = test_overlay();
        let mut loop_ctrl = test_loop();
        let outcomes = overlay
            .apply_operator_command(
                &mut loop_ctrl,
                &repo_root().join("config"),
                &motion_operator(payload.clone()),
            )
            .expect("rejection is an audit event, not an error");
        assert!(
            matches!(
                outcomes.as_slice(),
                [OverlayOutcome::Action(event)]
                    if !event.accepted && event.reject_reason.contains("gated")
            ),
            "{payload:?}: {outcomes:?}"
        );
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
        assert_eq!(
            loop_ctrl.supervisor().mode(),
            davout::OperationalMode::Disabled
        );
        assert!(loop_ctrl.position_setpoints().is_none());
    }
}

/// Runtime kp/kd retunes move a held joint: only the motion owner may send them.
#[test]
fn runtime_tuning_from_non_owner_chappe_is_refused() {
    use crate::motion_owner::{CommandSource, MotionLease};

    let mut overlay = test_overlay();
    overlay.set_motion_lease(MotionLease::new(CommandSource::Stdin));
    let mut loop_ctrl = test_loop();
    loop_ctrl.set_control_mode(ControlMode::Impedance);
    let op = tuning_operator(
        "right_elbow_pitch",
        "kp",
        88.0,
        TuningTier::RuntimeMit as i32,
    );
    let err = overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect_err("non-owner runtime tuning");
    assert!(matches!(err, OverlayError::Motion(_)), "{err}");
    assert!(err.to_string().contains("owned by stdin"), "{err}");
    assert!(loop_ctrl.gain_override("right_elbow_pitch").is_none());

    overlay.set_motion_lease(MotionLease::new(CommandSource::Chappe));
    overlay
        .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
        .expect("owner runtime tuning");
    assert!(loop_ctrl.gain_override("right_elbow_pitch").is_some());
}
