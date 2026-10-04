//! The gravity saturation preflight runs one bounded slice per control tick
//! while the loop keeps draining CAN, and a sweep that refuses or is voided
//! never grants an Enable. Closed simulation owner, no hardware.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{
    ActionEvent, ControlMode as ProtoControlMode, EnableRequest, Envelope, MitCommandBatch,
    MitJointCommand,
};
use berthier::{ControlLoop, ControlMode};
use chappe::Bus;
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::OperationalMode;
use marengo_config::LimitPatch;

use crate::enable_gate::EnableGate;
use crate::gravity_preflight::{SliceBudget, SweepStats};
use crate::motion_owner::{CommandSource, MotionLease, MOTION_REFUSED_ACTION};
use crate::test_support::queue_all_status;
use crate::{
    advance_gravity_preflight, dispatch_stdin_command, drain_testing_commands,
    handle_chappe_enable, PiCommand, PiReferenceQueue,
};

const PITCH: &str = "right_shoulder_pitch";
/// Five joints × five grid points.
const FULL_GRID: usize = 3125;
/// The per-tick sample cap with no wall-time cut, so slicing does not depend
/// on how fast this (debug) build evaluates the model.
const TEST_BUDGET: SliceBudget = SliceBudget {
    max_samples: SliceBudget::PER_TICK.max_samples,
    max_time: Duration::from_secs(60),
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Disabled, every joint referenced (virtual fixture) and measured once.
fn measured_loop() -> ControlLoop<SimulationBus> {
    let mut loop_ctrl = ControlLoop::from_simulation(
        repo_root(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        25,
    )
    .expect("closed virtual owner");
    queue_all_status(loop_ctrl.supervisor_mut(), None);
    loop_ctrl
        .tick(None)
        .expect("disabled tick drains the status");
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    loop_ctrl
}

struct Runtime {
    queue: PiReferenceQueue,
    gate: EnableGate,
    chappe: Bus,
}

impl Runtime {
    fn new() -> Self {
        Self {
            queue: PiReferenceQueue::new("preflight-test".into()),
            gate: EnableGate::default(),
            chappe: Bus::new(32),
        }
    }

    fn stdin(&mut self, loop_ctrl: &mut ControlLoop<SimulationBus>, cmd: PiCommand) {
        assert!(dispatch_stdin_command(
            loop_ctrl,
            &mut self.queue,
            &mut self.gate,
            cmd,
            &repo_root().join("config"),
        ));
    }

    /// One `run_control_loop` iteration's preflight slice and tick, with a
    /// fresh report from every drive queued first.
    fn iterate(&mut self, loop_ctrl: &mut ControlLoop<SimulationBus>) -> Option<SweepStats> {
        queue_all_status(loop_ctrl.supervisor_mut(), None);
        let stats = advance_gravity_preflight(
            loop_ctrl,
            &mut self.gate,
            &self.queue,
            &self.chappe,
            &AtomicBool::new(false),
            TEST_BUDGET,
        );
        if let Err(error) = loop_ctrl.tick(Some(&self.chappe)) {
            panic!("tick: {error}");
        }
        stats
    }
}

fn enable() -> PiCommand {
    PiCommand::Enable {
        operator_id: "bench".into(),
    }
}

fn enable_frames(loop_ctrl: &mut ControlLoop<SimulationBus>) -> usize {
    loop_ctrl
        .supervisor_mut()
        .bus_mut()
        .frames()
        .iter()
        .filter(|frame| (frame.id >> 24) & 0x1f == 3)
        .count()
}

fn pending_reports(loop_ctrl: &mut ControlLoop<SimulationBus>) -> usize {
    loop_ctrl.supervisor_mut().bus_mut().pending_receive_count()
}

fn pitch_patch(loop_ctrl: &ControlLoop<SimulationBus>, torque_limit_nm: f64) -> LimitPatch {
    let policy = loop_ctrl
        .supervisor()
        .joint_limit_policy(PITCH)
        .expect("pitch policy");
    LimitPatch {
        joint: PITCH.into(),
        position_lower_rad: policy.hard_lower(),
        position_upper_rad: policy.hard_upper(),
        torque_limit_nm: Some(torque_limit_nm),
        position_soft_lower_rad: None,
        position_soft_upper_rad: None,
        velocity_max_rad_s: None,
    }
}

/// Run iterations until the sweep reaches a verdict (bounded).
fn run_to_verdict(
    loop_ctrl: &mut ControlLoop<SimulationBus>,
    runtime: &mut Runtime,
) -> (SweepStats, u32) {
    for ticks in 1..=FULL_GRID as u32 {
        if let Some(stats) = runtime.iterate(loop_ctrl) {
            return (stats, ticks);
        }
        assert_eq!(
            pending_reports(loop_ctrl),
            0,
            "the tick after every slice drains the drives' reports"
        );
    }
    panic!("sweep did not finish within one tick per sample");
}

#[test]
fn stdin_enable_returns_to_the_loop_before_the_sweep_completes() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    queue_all_status(loop_ctrl.supervisor_mut(), None);
    let queued = pending_reports(&mut loop_ctrl);

    runtime.stdin(&mut loop_ctrl, enable());

    // Before: the whole 5^5 grid ran here, then Enable resolution read the
    // queue and the drives went Active, all inside one stdin dispatch.
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(
        pending_reports(&mut loop_ctrl),
        queued,
        "the reports are left for the next tick"
    );
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
    assert!(runtime.gate.preflight_pending());
}

#[test]
fn the_sweep_completes_across_ticks_that_keep_draining_then_enables() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    runtime.stdin(&mut loop_ctrl, enable());
    // A command after `enable` still runs after it.
    runtime.stdin(&mut loop_ctrl, PiCommand::GravityOn);
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);

    let (stats, ticks) = run_to_verdict(&mut loop_ctrl, &mut runtime);
    eprintln!(
        "sweep: samples={} slices={} ticks={ticks} max_slice={:?} elapsed={:?}",
        stats.samples, stats.slices, stats.max_slice, stats.elapsed
    );

    assert_eq!(stats.samples, FULL_GRID);
    assert!(
        stats.slices as usize >= FULL_GRID.div_ceil(TEST_BUDGET.max_samples),
        "{stats:?}"
    );
    assert!(ticks >= 2);
    assert!(!runtime.gate.preflight_pending());
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Active);
    assert!(enable_frames(&mut loop_ctrl) > 0);
    assert!(matches!(
        runtime.gate.take_ready_stdin(),
        Some(PiCommand::GravityOn)
    ));
}

#[test]
fn the_per_tick_time_budget_splits_the_sweep() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    runtime.stdin(&mut loop_ctrl, enable());
    let budget = SliceBudget {
        max_samples: usize::MAX,
        max_time: SliceBudget::PER_TICK.max_time,
    };
    let stats = loop {
        queue_all_status(loop_ctrl.supervisor_mut(), None);
        if let Some(stats) = advance_gravity_preflight(
            &mut loop_ctrl,
            &mut runtime.gate,
            &runtime.queue,
            &runtime.chappe,
            &AtomicBool::new(false),
            budget,
        ) {
            break stats;
        }
        loop_ctrl.tick(None).expect("tick");
    };
    eprintln!(
        "time-budgeted sweep: samples={} slices={} max_slice={:?} elapsed={:?}",
        stats.samples, stats.slices, stats.max_slice, stats.elapsed
    );
    assert_eq!(stats.samples, FULL_GRID);
    assert!(stats.slices > 1, "{stats:?}");
}

#[test]
fn a_saturating_limit_still_refuses_and_nothing_is_enabled() {
    let mut loop_ctrl = measured_loop();
    let patch = pitch_patch(&loop_ctrl, 0.01);
    loop_ctrl
        .supervisor_mut()
        .apply_limit_patch(&patch)
        .expect("Set Limits while Disabled");
    let mut runtime = Runtime::new();
    runtime.stdin(&mut loop_ctrl, enable());
    runtime.stdin(&mut loop_ctrl, PiCommand::GravityOn);

    let (stats, _) = run_to_verdict(&mut loop_ctrl, &mut runtime);

    assert_eq!(stats.samples, FULL_GRID);
    assert!(!runtime.gate.preflight_pending());
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
    assert!(
        runtime.gate.take_ready_stdin().is_none(),
        "commands held behind a refused Enable are discarded"
    );
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
}

#[test]
fn disable_mid_sweep_voids_it_and_no_enable_follows() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    runtime.stdin(&mut loop_ctrl, enable());
    runtime.stdin(&mut loop_ctrl, PiCommand::HoldOn);
    assert!(runtime.iterate(&mut loop_ctrl).is_none());
    assert!(runtime.gate.preflight_pending());

    runtime.stdin(&mut loop_ctrl, PiCommand::Disable);

    assert!(!runtime.gate.preflight_pending());
    for _ in 0..3 {
        assert!(runtime.iterate(&mut loop_ctrl).is_none());
    }
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
    assert!(runtime.gate.take_ready_stdin().is_none());
    assert!(loop_ctrl.position_setpoints().is_none());
}

#[test]
fn a_limits_change_mid_sweep_voids_it() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    runtime.stdin(&mut loop_ctrl, enable());
    assert!(runtime.iterate(&mut loop_ctrl).is_none());

    let current = loop_ctrl
        .supervisor()
        .joint_limit_policy(PITCH)
        .expect("pitch policy")
        .effort;
    let patch = pitch_patch(&loop_ctrl, current * 0.9);
    loop_ctrl
        .supervisor_mut()
        .apply_limit_patch(&patch)
        .expect("Set Limits while Disabled");

    for _ in 0..3 {
        assert!(runtime.iterate(&mut loop_ctrl).is_none());
    }
    assert!(!runtime.gate.preflight_pending());
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
}

#[test]
fn a_drive_stop_mid_sweep_voids_it() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    runtime.stdin(&mut loop_ctrl, enable());
    assert!(runtime.iterate(&mut loop_ctrl).is_none());

    // Not an operator Disable: e.g. stop_after_tick_error.
    loop_ctrl.supervisor_mut().disable_all().expect("stop");

    assert!(runtime.iterate(&mut loop_ctrl).is_none());
    assert!(!runtime.gate.preflight_pending());
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
}

#[test]
fn chappe_disable_mid_sweep_voids_a_chappe_enable() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    let request = |enable| EnableRequest {
        timestamp_ms: 1,
        operator_id: "consul-tab".into(),
        enable,
    };
    handle_chappe_enable(
        &mut loop_ctrl,
        &mut runtime.queue,
        &mut runtime.gate,
        &request(true),
    )
    .expect("sweep started");
    assert!(runtime.gate.preflight_pending());
    assert!(runtime.iterate(&mut loop_ctrl).is_none());
    // A second enable waits behind the first sweep instead of sweeping again.
    handle_chappe_enable(
        &mut loop_ctrl,
        &mut runtime.queue,
        &mut runtime.gate,
        &request(true),
    )
    .expect("held");

    handle_chappe_enable(
        &mut loop_ctrl,
        &mut runtime.queue,
        &mut runtime.gate,
        &request(false),
    )
    .expect("disable");

    assert!(!runtime.gate.preflight_pending());
    assert!(runtime.gate.take_ready_chappe_enable().is_none());
    for _ in 0..3 {
        assert!(runtime.iterate(&mut loop_ctrl).is_none());
    }
    assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
}

/// B13 (P-armee-proto-04): `timestamp_ms` is deprecated (written, never read).
#[allow(deprecated)]
fn position_batch() -> MitCommandBatch {
    MitCommandBatch {
        timestamp_ms: 1,
        mode: ProtoControlMode::Position as i32,
        joints: vec![MitJointCommand {
            name: PITCH.into(),
            kd: 1.0,
            position: 0.0,
            ..MitJointCommand::default()
        }],
    }
}

#[test]
fn reference_work_mid_sweep_refuses_the_waiting_testing_batch() {
    let mut loop_ctrl = measured_loop();
    let mut runtime = Runtime::new();
    let mut testing = runtime.chappe.subscribe("robot/testing/mit_command_batch");
    let mut audit = runtime.chappe.subscribe("robot/audit/action");
    runtime
        .chappe
        .publish(
            "robot/testing/mit_command_batch",
            "test",
            "marengo.v1.MitCommandBatch",
            &position_batch(),
        )
        .expect("publish batch");
    drain_testing_commands(
        &mut loop_ctrl,
        &runtime.queue,
        &mut runtime.gate,
        MotionLease::new(CommandSource::Chappe),
        &runtime.chappe,
        &mut testing,
        &AtomicBool::new(false),
    );
    assert!(runtime.gate.preflight_pending());
    assert!(runtime.iterate(&mut loop_ctrl).is_none());

    runtime
        .queue
        .admit(&[PITCH.into()], true, "consul", |_| true)
        .expect("Set Zero admitted");
    assert!(runtime.iterate(&mut loop_ctrl).is_none());

    assert!(!runtime.gate.preflight_pending());
    assert_eq!(enable_frames(&mut loop_ctrl), 0);
    assert_ne!(loop_ctrl.control_mode(), ControlMode::Position);
    let mut refusals = Vec::new();
    while let Ok(bytes) = audit.try_recv() {
        let envelope = Envelope::decode(bytes.as_slice()).expect("audit envelope");
        let event = ActionEvent::decode(envelope.payload.as_slice()).expect("ActionEvent");
        if event.action == MOTION_REFUSED_ACTION {
            refusals.push(event);
        }
    }
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    assert!(!refusals[0].accepted);
}
