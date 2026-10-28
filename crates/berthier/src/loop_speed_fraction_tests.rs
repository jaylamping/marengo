//! ADR 0040 amendment (2026-10-28): the optional, fail-closed hold-at speed fraction.
//!
//! A fraction outside `(0, 1]` is refused before any state changes; a valid one scales the
//! planned speeds of the move (never its acceleration limit); no fraction is the old move.
#![allow(clippy::expect_used, clippy::panic)]

use super::*;
use crate::test_support::queue_all_status;
use davout::simulation::{InitialVirtualReference, SimulationBus};

/// `position_law: scaled_pd` in the master config (v 1.25 rad/s, a 1.5 rad/s²).
const PITCH: &str = "right_shoulder_pitch";
/// Legacy law in the master config (v 1.43 rad/s, a 3.0 rad/s²).
const YAW: &str = "right_upper_arm_yaw";
const HZ: u32 = 200;
/// 0.75 s: inside every stall-fuse budget while the joint is held still.
const TICKS: usize = 150;
const BAD_FRACTIONS: [f64; 8] = [
    0.0,
    -0.0,
    -0.25,
    1.0 + f64::EPSILON,
    1.5,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every configured drive Active, `joint` at `q`, the rest at 0.
fn active_loop(joint: &str, q: f64) -> ControlLoop<SimulationBus> {
    let mut ctrl = ControlLoop::from_simulation(
        repo_root(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        HZ,
        50,
    )
    .expect("declared virtual initial reference");
    ctrl.supervisor_mut().set_homing_complete().expect("ready");
    let joints: Vec<String> = ctrl
        .supervisor()
        .motors
        .motors
        .iter()
        .map(|motor| motor.joint.clone())
        .collect();
    ctrl.supervisor_mut()
        .enable_targets(&joints)
        .expect("enable");
    feed(&mut ctrl, joint, q, 0.0);
    ctrl
}

fn feed(ctrl: &mut ControlLoop<SimulationBus>, joint: &str, q: f64, dq: f64) {
    queue_all_status(ctrl.supervisor_mut(), Some((joint, q, dq)));
    ctrl.supervisor_mut()
        .drain_feedback()
        .expect("finite raw observations");
}

fn index(ctrl: &ControlLoop<SimulationBus>, joint: &str) -> usize {
    ctrl.joint_names()
        .iter()
        .position(|name| name == joint)
        .expect("configured joint")
}

/// Planner reference of `joint`, tick by tick, for a hold-at 0 → `target` with the joint held
/// at 0 (constant feedback: Davout derives velocity from wall-clock receive times, so a moving
/// replay would be judged at test speed, not at 200 Hz). The ideal-tracking peak-speed gate is
/// `position_hold_tests/speed_fraction.rs`. `speed` `None` uses the speedless API,
/// `Some(None)` the new API without a fraction.
fn planned_reference(joint: &str, target: f64, speed: Option<Option<f64>>) -> Vec<f64> {
    let mut ctrl = active_loop(joint, 0.0);
    match speed {
        None => ctrl.enter_position_hold_at(Some(joint), target),
        Some(fraction) => ctrl.enter_position_hold_at_speed(Some(joint), target, fraction),
    }
    .expect("hold-at admitted");
    let i = index(&ctrl, joint);
    let mut reference = vec![ctrl.position_hold_commands().expect("armed")[i]];
    for _ in 0..TICKS {
        feed(&mut ctrl, joint, 0.0, 0.0);
        ctrl.tick(None).expect("hold tick");
        reference.push(ctrl.position_hold_commands().expect("armed")[i]);
    }
    reference
}

#[test]
fn out_of_range_speed_fraction_is_refused_before_anything_is_armed() {
    let mut ctrl = active_loop(PITCH, 0.02);
    for value in BAD_FRACTIONS {
        let err = ctrl
            .enter_position_hold_at_speed(Some(PITCH), 0.1, Some(value))
            .expect_err("enter_position_hold_at_speed must refuse");
        assert!(
            matches!(err, LoopError::InvalidSpeedFraction(_)),
            "{value}: {err:?}"
        );
        let err = ctrl
            .set_joint_position_setpoint_at_speed(PITCH, 0.1, Some(value))
            .expect_err("set_joint_position_setpoint_at_speed must refuse");
        assert!(
            matches!(err, LoopError::InvalidSpeedFraction(_)),
            "{value}: {err:?}"
        );
    }
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    assert!(ctrl.position_setpoints().is_none(), "nothing armed");
    assert!(ctrl.position_hold_commands().is_none(), "no planner");
}

#[test]
fn out_of_range_speed_fraction_leaves_a_running_hold_untouched() {
    let mut ctrl = active_loop(PITCH, 0.02);
    ctrl.enter_position_hold_at_speed(Some(PITCH), 0.1, Some(0.5))
        .expect("valid fraction");
    feed(&mut ctrl, PITCH, 0.02, 0.0);
    ctrl.tick(None).expect("hold ticks");
    let targets = ctrl.position_setpoints().expect("armed").to_vec();
    let reference = ctrl.position_hold_commands().expect("armed");
    let speed = ctrl.position_speed_fraction(PITCH);
    assert_eq!(speed.map(SpeedFraction::get), Some(0.5));
    for value in BAD_FRACTIONS {
        assert!(ctrl
            .enter_position_hold_at_speed(Some(PITCH), 0.4, Some(value))
            .is_err());
        assert!(ctrl
            .set_joint_position_setpoint_at_speed(PITCH, 0.4, Some(value))
            .is_err());
        assert_eq!(ctrl.position_setpoints(), Some(targets.as_slice()));
        assert_eq!(ctrl.position_hold_commands(), Some(reference.clone()));
        assert_eq!(ctrl.position_speed_fraction(PITCH), speed);
        assert_eq!(ctrl.control_mode(), ControlMode::Position);
    }
}

#[test]
fn hold_at_without_a_fraction_plans_exactly_the_old_move() {
    for (joint, target) in [(PITCH, 0.6), (YAW, 0.6)] {
        let old = planned_reference(joint, target, None);
        assert!(
            old.windows(2).any(|pair| pair[1] != pair[0]),
            "{joint}: the reference moves"
        );
        assert_eq!(planned_reference(joint, target, Some(None)), old, "{joint}");
        assert_eq!(
            planned_reference(joint, target, Some(Some(1.0))),
            old,
            "{joint}"
        );
        assert_ne!(
            planned_reference(joint, target, Some(Some(0.5))),
            old,
            "{joint}"
        );
    }
}

#[test]
fn the_fraction_is_latched_with_the_target() {
    let mut ctrl = active_loop(PITCH, 0.0);
    ctrl.enter_position_hold_at_speed(Some(PITCH), 0.3, Some(0.25))
        .expect("slow hold-at");
    assert_eq!(
        ctrl.position_speed_fraction(PITCH).map(SpeedFraction::get),
        Some(0.25)
    );
    // A same-target hold-at is a no-op, fraction included.
    ctrl.enter_position_hold_at(Some(PITCH), 0.3)
        .expect("same target");
    assert_eq!(
        ctrl.position_speed_fraction(PITCH).map(SpeedFraction::get),
        Some(0.25)
    );
    // A new target without a fraction is a full-speed move.
    ctrl.set_joint_position_setpoint(PITCH, 0.5)
        .expect("retarget");
    assert_eq!(
        ctrl.position_speed_fraction(PITCH),
        Some(SpeedFraction::FULL)
    );
    // hold-on latches the measured pose: no earlier fraction survives.
    ctrl.set_joint_position_setpoint_at_speed(PITCH, 0.2, Some(0.5))
        .expect("slow retarget");
    ctrl.enter_position_hold().expect("hold-on");
    assert_eq!(
        ctrl.position_speed_fraction(PITCH),
        Some(SpeedFraction::FULL)
    );
}
