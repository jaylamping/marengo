#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Instant;

use berthier::{ControlLoop, ControlMode, ENABLE_COMPLETION_TIMEOUT};
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::OperationalMode;
use robstride::CanFrame;

use crate::enable_gate::{EnableGate, GateLine};
use crate::{dispatch_stdin_command, PiCommand, PiReferenceQueue};

const JOINT: &str = "right_elbow_pitch";
/// Type-2 Run status from can0 device 4 (right_elbow_pitch), off zero.
const RUN_STATUS_ID: u32 = 0x028004fd;
const RUN_STATUS_DATA: [u8; 8] = [0x80, 0x40, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Active on JOINT through the real Enable path, with no session pose yet.
fn enabled_without_pose() -> ControlLoop<SimulationBus> {
    let mut loop_ctrl = ControlLoop::from_simulation(
        repo_root(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![JOINT.into()]),
        200,
        25,
    )
    .expect("closed virtual owner");
    loop_ctrl
        .supervisor_mut()
        .enable_targets(&[JOINT.into()])
        .expect("scoped Enable");
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Active);
    loop_ctrl
}

fn first_session_status(loop_ctrl: &mut ControlLoop<SimulationBus>) -> f64 {
    loop_ctrl
        .supervisor_mut()
        .bus_mut()
        .queue_frame(CanFrame {
            id: RUN_STATUS_ID,
            data: RUN_STATUS_DATA,
            extended: true,
        })
        .expect("raw status after Enable");
    loop_ctrl
        .supervisor_mut()
        .drain_feedback()
        .expect("session status decoded");
    loop_ctrl
        .supervisor()
        .joint_feedback(JOINT)
        .expect("session pose")
        .position_rad
}

fn held_position(loop_ctrl: &ControlLoop<SimulationBus>) -> Option<f64> {
    let index = loop_ctrl
        .joint_names()
        .iter()
        .position(|name| name == JOINT)
        .expect("configured joint");
    loop_ctrl
        .position_setpoints()
        .map(|setpoints| setpoints[index])
}

fn out(text: &str) -> GateLine {
    GateLine::Out(text.into())
}

#[test]
fn enabled_is_reported_only_once_enable_completes() {
    let mut loop_ctrl = enabled_without_pose();
    let mut gate = EnableGate::default();
    let now = Instant::now();
    assert_eq!(
        gate.begin_enable("bench".into(), vec![JOINT.into()], now),
        [out(
            "waiting for enable to complete (operator=bench) targets=right_elbow_pitch"
        )]
    );
    assert!(
        gate.poll(&mut loop_ctrl, now).is_empty(),
        "no `enabled` before session feedback"
    );
    first_session_status(&mut loop_ctrl);
    assert_eq!(
        gate.poll(&mut loop_ctrl, Instant::now()),
        [out("enabled (operator=bench) targets=right_elbow_pitch")]
    );
    assert!(gate.poll(&mut loop_ctrl, Instant::now()).is_empty(), "once");
}

#[test]
fn hold_on_before_enable_completes_waits_then_latches_measured_q() {
    let mut loop_ctrl = enabled_without_pose();
    let mut gate = EnableGate::default();
    let now = Instant::now();
    gate.begin_enable("bench".into(), vec![JOINT.into()], now);
    assert_eq!(
        gate.submit(&mut loop_ctrl, PiCommand::HoldOn, now),
        [out("hold-on waiting for enable to complete")]
    );
    assert!(gate.poll(&mut loop_ctrl, now).is_empty());
    assert_eq!(
        held_position(&loop_ctrl),
        None,
        "no 0.0 placeholder latched"
    );
    assert_ne!(loop_ctrl.control_mode(), ControlMode::Position);

    let measured = first_session_status(&mut loop_ctrl);
    assert!(
        measured.abs() > 0.01,
        "fixture pose is off zero: {measured}"
    );
    let lines = gate.poll(&mut loop_ctrl, Instant::now());
    assert_eq!(
        lines.first(),
        Some(&out("enabled (operator=bench) targets=right_elbow_pitch")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| matches!(line, GateLine::Out(text) if text.starts_with("control mode → Position hold"))),
        "{lines:?}"
    );
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    let held = held_position(&loop_ctrl).expect("latched");
    assert!(
        (held - measured).abs() < 1e-9,
        "held {held}, measured {measured}"
    );
}

#[test]
fn enable_not_completing_within_the_bound_is_refused_and_stops() {
    let mut loop_ctrl = enabled_without_pose();
    let mut gate = EnableGate::default();
    let now = Instant::now();
    gate.begin_enable("bench".into(), vec![JOINT.into()], now);
    gate.submit(
        &mut loop_ctrl,
        PiCommand::HoldAt {
            joint: Some(JOINT.into()),
            position_rad: 0.1,
        },
        now,
    );
    let lines = gate.poll(&mut loop_ctrl, now + ENABLE_COMPLETION_TIMEOUT);
    assert_eq!(
        lines,
        [
            GateLine::Err(
                "enable failed: not complete within 2000 ms: waiting for enable to complete: \
                 no session feedback from right_elbow_pitch"
                    .into()
            ),
            GateLine::Err("hold-at failed: enable did not complete".into()),
        ]
    );
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    assert_eq!(held_position(&loop_ctrl), None);
    assert!(gate.poll(&mut loop_ctrl, Instant::now()).is_empty());
}

#[test]
fn arm_deferred_by_its_own_re_enable_is_refused_after_the_bound() {
    let mut loop_ctrl = enabled_without_pose();
    let mut gate = EnableGate::default();
    let now = Instant::now();
    let lines = gate.submit(
        &mut loop_ctrl,
        PiCommand::Wave {
            joint: JOINT.into(),
            min_rad: 0.02,
            max_rad: 0.04,
            cycles: 1,
            half_period_sec: 0.5,
        },
        now,
    );
    assert_eq!(
        lines,
        [out(
            "wave waiting for enable to complete: no session feedback from right_elbow_pitch"
        )]
    );
    assert!(
        gate.poll(&mut loop_ctrl, now).is_empty(),
        "still within the bound"
    );
    let lines = gate.poll(&mut loop_ctrl, now + ENABLE_COMPLETION_TIMEOUT);
    assert_eq!(
        lines,
        [GateLine::Err(
            "wave failed: waiting for enable to complete: no session feedback from \
             right_elbow_pitch (no completion within 2000 ms)"
                .into()
        )]
    );
    assert!(!loop_ctrl.position_wave_active());
    assert_eq!(held_position(&loop_ctrl), None);
}

#[test]
fn disable_and_hold_off_cancel_waiting_commands() {
    let mut loop_ctrl = enabled_without_pose();
    let mut gate = EnableGate::default();
    let mut queue = PiReferenceQueue::new("marengo-pi-test".into());
    let config = repo_root().join("config");
    gate.begin_enable("bench".into(), vec![JOINT.into()], Instant::now());
    assert!(dispatch_stdin_command(
        &mut loop_ctrl,
        &mut queue,
        &mut gate,
        PiCommand::HoldOn,
        &config
    ));
    assert!(dispatch_stdin_command(
        &mut loop_ctrl,
        &mut queue,
        &mut gate,
        PiCommand::Disable,
        &config
    ));
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert!(gate.poll(&mut loop_ctrl, Instant::now()).is_empty());
    assert_eq!(held_position(&loop_ctrl), None);

    let mut loop_ctrl = enabled_without_pose();
    let mut gate = EnableGate::default();
    gate.submit(&mut loop_ctrl, PiCommand::HoldOn, Instant::now());
    assert_eq!(
        gate.cancel_arms("cancelled by hold-off"),
        [GateLine::Err(
            "hold-on failed: cancelled by hold-off".into()
        )]
    );
    first_session_status(&mut loop_ctrl);
    assert!(gate.poll(&mut loop_ctrl, Instant::now()).is_empty());
    assert_eq!(held_position(&loop_ctrl), None, "cancelled arm never runs");
}
