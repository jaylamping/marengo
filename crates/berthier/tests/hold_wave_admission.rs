//! Position entry never commands a joint toward a stale or unplanned target:
//! a refused hold-at leaves nothing armed (BER-01), a wave must start where the
//! joint is held (BER-02), and only the wave's own joint cancels it (BER-09).

#![allow(clippy::expect_used)]

mod support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use berthier::{ControlLoop, ControlMode, LoopError, WAVE_START_TOLERANCE_RAD};
use davout::simulation::{InitialVirtualReference, SimulationBus};

const PITCH: &str = "right_shoulder_pitch";
const ELBOW: &str = "right_elbow_pitch";
const PERIOD: Duration = Duration::from_millis(5);

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Enabled five-joint loop in GravityComp with every joint reporting 0.
fn gravity_comp_loop(label: &str) -> (support::FixtureTree, ControlLoop<SimulationBus>) {
    let fixture = support::fixture_tree_without_diagnostics(label, &source());
    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("sim");
    let joints = ctrl.joint_names().to_vec();
    ctrl.supervisor_mut()
        .enable_targets(&joints)
        .expect("enable");
    queue(&mut ctrl, 0.0);
    ctrl.supervisor_mut().drain_feedback().expect("drain");
    ctrl.set_control_mode(ControlMode::GravityComp);
    for _ in 0..5 {
        tick(&mut ctrl, 0.0);
    }
    (fixture, ctrl)
}

/// Every joint reports 0 except pitch, which reports `pitch`.
fn queue(ctrl: &mut ControlLoop<SimulationBus>, pitch: f64) {
    for joint in ctrl.joint_names().to_vec() {
        let position = if joint == PITCH { pitch } else { 0.0 };
        support::queue_joint_status(ctrl.supervisor_mut(), &joint, position, 0.0);
    }
}

fn tick(ctrl: &mut ControlLoop<SimulationBus>, pitch: f64) {
    std::thread::sleep(PERIOD);
    queue(ctrl, pitch);
    ctrl.tick(None).expect("tick");
}

fn index(ctrl: &ControlLoop<SimulationBus>, joint: &str) -> usize {
    ctrl.joint_names()
        .iter()
        .position(|name| name == joint)
        .expect("joint")
}

/// Refuse a hold-at, move pitch by hand in GravityComp, then hold the elbow:
/// pitch must hold where it now is, not where it was when the refusal came.
fn refused_hold_at_then_elbow(
    refused: impl FnOnce(&mut ControlLoop<SimulationBus>) -> Result<(), LoopError>,
) {
    let (_fixture, mut ctrl) = gravity_comp_loop("ber01");
    refused(&mut ctrl).expect_err("hold-at refused");
    assert_eq!(ctrl.control_mode(), ControlMode::GravityComp);
    for k in 1..=100 {
        tick(&mut ctrl, 0.004 * f64::from(k));
    }
    ctrl.enter_position_hold_at(Some(ELBOW), 0.0)
        .expect("elbow hold-at");
    let pitch_target = ctrl.position_setpoints().expect("armed")[index(&ctrl, PITCH)];
    assert!(
        (pitch_target - 0.4).abs() < 1e-3,
        "pitch must hold its measured 0.4 rad, got stale target {pitch_target}"
    );
}

#[test]
fn hold_at_without_joint_name_leaves_nothing_armed() {
    refused_hold_at_then_elbow(|ctrl| ctrl.enter_position_hold_at(None, 0.2));
}

#[test]
fn hold_at_unknown_joint_leaves_nothing_armed() {
    refused_hold_at_then_elbow(|ctrl| ctrl.enter_position_hold_at(Some("no_such_joint"), 0.2));
}

#[test]
fn wave_far_from_its_start_is_refused() {
    let (_fixture, mut ctrl) = gravity_comp_loop("ber02-far");
    let err = ctrl
        .start_position_wave(PITCH, 0.5, 0.7, 1, 2.0)
        .expect_err("pitch is at 0, the wave starts at 0.5");
    assert!(
        matches!(err, LoopError::WaveStartTooFar { .. }),
        "unexpected error: {err}"
    );
    assert_eq!(ctrl.control_mode(), ControlMode::GravityComp);
    assert!(!ctrl.position_wave_active());
}

#[test]
fn wave_at_its_start_is_admitted() {
    let (_fixture, mut ctrl) = gravity_comp_loop("ber02-near");
    ctrl.start_position_wave(PITCH, 0.5 * WAVE_START_TOLERANCE_RAD, 0.2, 1, 2.0)
        .expect("wave from the held pose");
    assert_eq!(ctrl.control_mode(), ControlMode::Position);
    assert!(ctrl.position_wave_active());
}

#[test]
fn retargeting_another_joint_keeps_the_wave() {
    let (_fixture, mut ctrl) = gravity_comp_loop("ber09");
    ctrl.start_position_wave(PITCH, 0.0, 0.2, 2, 2.0)
        .expect("pitch wave");
    tick(&mut ctrl, 0.0);
    ctrl.set_joint_position_setpoint(ELBOW, 0.1)
        .expect("elbow retarget");
    assert!(
        ctrl.position_wave_active(),
        "an elbow retarget must not cancel the pitch wave"
    );
    ctrl.set_joint_position_setpoint(PITCH, 0.1)
        .expect("pitch retarget");
    assert!(
        !ctrl.position_wave_active(),
        "retargeting the waving joint ends its wave"
    );
}

/// RT-01: a fresh loop never enables the drives because of a motion command;
/// only an explicit enable (which runs the gravity preflight) does.
#[test]
fn motion_command_on_a_new_loop_requires_explicit_enable() {
    let fixture = support::fixture_tree_without_diagnostics("rt01", &source());
    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("sim");
    let err = ctrl
        .enter_position_hold()
        .expect_err("hold-on before enable");
    assert!(
        matches!(err, LoopError::ExplicitEnableRequired),
        "unexpected error: {err}"
    );
    assert_eq!(ctrl.supervisor().mode(), davout::OperationalMode::Disabled);
    let err = ctrl
        .enter_position_hold_at(Some(PITCH), 0.2)
        .expect_err("hold-at before enable");
    assert!(matches!(err, LoopError::ExplicitEnableRequired), "{err}");
    let err = ctrl
        .start_position_wave(PITCH, 0.0, 0.2, 1, 2.0)
        .expect_err("wave before enable");
    assert!(matches!(err, LoopError::ExplicitEnableRequired), "{err}");
    assert_eq!(ctrl.supervisor().mode(), davout::OperationalMode::Disabled);
}
