//! Enabling without CAN pose must solicit status before calculating servo output.

#![allow(clippy::expect_used)]

use berthier::{ControlLoop, ControlMode, LoopError};
use davout::MemoryBus;
use marengo_config::LimitPatch;

fn enabled_controller() -> ControlLoop<MemoryBus> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut controller =
        ControlLoop::from_repo(root, MemoryBus::default(), 200, 50).expect("repository fixture");
    let motors = controller.supervisor().motors.motors.clone();
    controller
        .supervisor_mut()
        .homing_registry_mut()
        .bench_mark_all_verified(&motors)
        .expect("recording reference");
    controller
        .supervisor_mut()
        .set_homing_complete()
        .expect("ready");
    for _ in 0..5 {
        controller.tick(None).expect("disabled tick");
    }
    controller
        .supervisor_mut()
        .request_enable(true)
        .expect("recording enable");
    controller.set_control_mode(ControlMode::Impedance);
    controller.supervisor_mut().bus_mut().tx.clear();
    controller
}

fn assert_neutral_frames(controller: &mut ControlLoop<MemoryBus>) {
    let motor_count = controller.supervisor().motors.motors.len();
    let frames = &controller.supervisor_mut().bus_mut().tx;
    assert_eq!(frames.len(), 2 * motor_count);
    for frame in frames {
        // Vendor zero torque, zero desired speed, zero kp/kd. This oracle does
        // not call the encoder/decoder that produced the command.
        assert_eq!((frame.id >> 24) & 0x1f, 1);
        assert_eq!((frame.id >> 8) & 0xffff, 0x7fff);
        assert_eq!(&frame.data[2..4], &[0x7f, 0xff]);
        assert_eq!(&frame.data[4..8], &[0, 0, 0, 0]);
    }
}

fn assert_stopped(controller: &mut ControlLoop<MemoryBus>) {
    let snapshot = controller.supervisor().safety_snapshot();
    assert!(snapshot.is_latched());
    assert_eq!(controller.control_mode(), ControlMode::Disabled);
    let report = snapshot.last_stop.expect("stop outcome");
    assert_eq!(report.failed_writes(), 0);
    assert_eq!(
        report.attempts.len(),
        controller.supervisor().motors.motors.len() * 3
    );
    for frame in &controller.supervisor_mut().bus_mut().tx {
        match (frame.id >> 24) & 0x1f {
            1 => {
                assert_eq!((frame.id >> 8) & 0xffff, 0x7fff);
                assert_eq!(&frame.data[2..4], &[0x7f, 0xff]);
                assert_eq!(&frame.data[4..8], &[0, 0, 0, 0]);
            }
            4 => assert_eq!(
                frame.data, [0; 8],
                "routine stop never clears device faults"
            ),
            18 => {
                assert_eq!(
                    &frame.data[0..2],
                    &[0x0a, 0x70],
                    "only zero speed during stop"
                );
                assert_eq!(&frame.data[4..8], &[0; 4]);
            }
            24 => {} // Free-drive diagnostics may resume after software Disable.
            other => assert!(
                matches!(other, 1 | 4 | 18 | 24),
                "unexpected stop frame type {other}"
            ),
        }
    }
}

#[test]
fn active_feedback_bootstrap_emits_only_neutral_frames_then_expires() {
    let mut controller = enabled_controller();
    for _ in 0..2 {
        controller.tick(None).expect("bounded bootstrap tick");
    }
    assert_neutral_frames(&mut controller);
    controller.supervisor_mut().bus_mut().tx.clear();
    assert!(matches!(
        controller.tick(None),
        Err(LoopError::MissingFeedback { .. })
    ));
    assert_stopped(&mut controller);
}

#[test]
fn reenable_between_ticks_starts_a_new_bounded_neutral_bootstrap() {
    let mut controller = enabled_controller();
    for _ in 0..2 {
        controller.tick(None).expect("first bootstrap tick");
    }
    // No controller tick observes Disabled. The enable session, rather than
    // only the last observed operational mode, owns the fresh feedback window.
    controller
        .supervisor_mut()
        .disable_all()
        .expect("recording disable");
    let targets: Vec<_> = controller
        .supervisor()
        .motors
        .motors
        .iter()
        .map(|motor| motor.joint.clone())
        .collect();
    controller
        .supervisor_mut()
        .enable_targets(&targets)
        .expect("recording re-enable");
    controller.set_control_mode(ControlMode::Impedance);
    controller.supervisor_mut().bus_mut().tx.clear();
    for _ in 0..2 {
        controller.tick(None).expect("new session bootstrap tick");
    }
    assert_neutral_frames(&mut controller);
    controller.supervisor_mut().bus_mut().tx.clear();
    assert!(matches!(
        controller.tick(None),
        Err(LoopError::MissingFeedback { .. })
    ));
    assert_stopped(&mut controller);
}

#[test]
fn neutral_bootstrap_supports_taught_ranges_that_exclude_zero() {
    for (lower, upper) in [(0.2, 0.8), (-0.8, -0.2)] {
        let mut controller = enabled_controller();
        controller
            .supervisor_mut()
            .disable_all()
            .expect("disabled setup");
        let joint = "right_elbow_pitch";
        controller
            .supervisor_mut()
            .apply_limit_patch(&LimitPatch {
                joint: joint.into(),
                position_lower_rad: lower,
                position_upper_rad: upper,
                position_soft_lower_rad: Some(lower + 0.025),
                position_soft_upper_rad: Some(upper - 0.025),
                velocity_max_rad_s: None,
                torque_limit_nm: None,
            })
            .expect("valid taught range");
        controller
            .supervisor_mut()
            .set_synthetic_joint_feedback(joint, ((lower + upper) / 2.0) as f32, 0.0)
            .expect("in-range Disabled pose");
        controller
            .supervisor_mut()
            .enable_targets(&[joint.into()])
            .expect("scoped enable");
        controller.set_control_mode(ControlMode::Impedance);
        assert!(controller.supervisor().joint_feedback(joint).is_none());
        controller.supervisor_mut().bus_mut().tx.clear();
        for _ in 0..2 {
            controller
                .tick(None)
                .expect("inert solicit for range excluding zero");
        }
        let frames = &controller.supervisor_mut().bus_mut().tx;
        assert_eq!(frames.len(), 2);
        for frame in frames {
            assert_eq!((frame.id >> 24) & 0x1f, 1);
            assert_eq!((frame.id >> 8) & 0xffff, 0x7fff);
            assert_eq!(&frame.data[2..4], &[0x7f, 0xff]);
            assert_eq!(&frame.data[4..8], &[0, 0, 0, 0]);
            // Elbow direction is -1 and gearing is 1 in this independent
            // master fixture. Position is inert with kp/kd zero but must still
            // be a valid finite target in the installed hard envelope.
            let word = f64::from(u16::from_be_bytes([frame.data[0], frame.data[1]]));
            let joint_target = -(word / 32767.0 - 1.0) * 4.0 * std::f64::consts::PI;
            assert!(joint_target >= lower - 0.001 && joint_target <= upper + 0.001);
        }
        controller.supervisor_mut().bus_mut().tx.clear();
        assert!(matches!(
            controller.tick(None),
            Err(LoopError::MissingFeedback { .. })
        ));
        assert_stopped(&mut controller);
    }
}
