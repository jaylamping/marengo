//! Owner stop paths: Chappe disable, tick-error stop, Chappe set-zero admission.
//! No hardware: a recording bus that can refuse Disable frames.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use armee_proto::{EnableRequest, SetZeroRequest};
use berthier::{ControlLoop, ControlMode, LoopError};
use davout::DavoutError;
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorBus, ReceiveAttempt};

use crate::enable_gate::EnableGate;
use crate::{
    handle_chappe_enable, handle_chappe_set_zero, handle_command, parse_command,
    run_with_panic_stop, stop_after_tick_error, PiReferenceQueue, TickStopPacer,
};

const JOINT: &str = "right_elbow_pitch";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Records frames; optionally refuses every type-4 Disable.
#[derive(Default)]
struct FlakyBus {
    inner: MemoryBus,
    refuse_disable: bool,
    /// When set and true, refuses Disable like `refuse_disable`; the test can
    /// clear it after the bus moved into the controller.
    refuse_disable_switch: Option<Arc<AtomicBool>>,
}

impl CanBus for FlakyBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        let refusing = self.refuse_disable
            || self
                .refuse_disable_switch
                .as_ref()
                .is_some_and(|switch| switch.load(Ordering::SeqCst));
        if refusing && (frame.id >> 24) & 0x1f == 4 {
            return Err(BusError::Send {
                message: "injected disable refusal".into(),
            });
        }
        self.inner.send_frame(frame)
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        self.inner.recv_one_nonblocking()
    }
}

impl MotorBus for FlakyBus {}

fn loop_with(bus: FlakyBus) -> ControlLoop<FlakyBus> {
    ControlLoop::from_repo(repo_root(), bus, 200, 50).expect("loop")
}

fn queue() -> PiReferenceQueue {
    PiReferenceQueue::new("stop-path-test".into())
}

fn disables_to(loop_ctrl: &ControlLoop<FlakyBus>, device_id: u8) -> usize {
    loop_ctrl
        .supervisor()
        .bus()
        .inner
        .tx
        .iter()
        .filter(|frame| (frame.id >> 24) & 0x1f == 4 && (frame.id & 0xff) as u8 == device_id)
        .count()
}

fn disable_request() -> EnableRequest {
    EnableRequest {
        timestamp_ms: 0,
        operator_id: "test".into(),
        enable: false,
    }
}

fn comm_watchdog() -> LoopError {
    LoopError::Safety(DavoutError::CommWatchdog {
        ms: 100,
        joint: JOINT.into(),
        interface: "can0".into(),
        device_id: 4,
        age_ms: None,
    })
}

#[test]
fn chappe_disable_sets_mode_disabled_even_when_the_stop_was_not_delivered() {
    let mut loop_ctrl = loop_with(FlakyBus {
        refuse_disable: true,
        ..FlakyBus::default()
    });
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    let mut queue = queue();

    let result = handle_chappe_enable(
        &mut loop_ctrl,
        &mut queue,
        &mut EnableGate::default(),
        &disable_request(),
    );

    let error = result.expect_err("a refused Disable write is reported");
    assert!(error.contains("stop"), "{error}");
    assert_eq!(
        loop_ctrl.control_mode(),
        ControlMode::Disabled,
        "a failed Chappe disable left motion intent armed"
    );
    assert!(loop_ctrl.supervisor().has_latched_fault(), "StopDelivery");
}

#[test]
fn chappe_disable_stops_every_drive_and_clears_intent() {
    let mut loop_ctrl = loop_with(FlakyBus::default());
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    let mut queue = queue();

    handle_chappe_enable(
        &mut loop_ctrl,
        &mut queue,
        &mut EnableGate::default(),
        &disable_request(),
    )
    .expect("disable");

    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    for motor in loop_ctrl.supervisor().motors.motors.clone() {
        assert!(
            disables_to(&loop_ctrl, motor.device_id) >= 1,
            "{} not disabled",
            motor.joint
        );
    }
}

#[test]
fn tick_error_stops_every_drive_and_disables_even_for_comm_watchdog() {
    let mut loop_ctrl = loop_with(FlakyBus::default());
    loop_ctrl.set_control_mode(ControlMode::Position);

    let fault = stop_after_tick_error(
        &mut loop_ctrl,
        &mut TickStopPacer::default(),
        &comm_watchdog(),
        Instant::now(),
    );

    assert!(fault.contains("comm watchdog"), "{fault}");
    assert!(!fault.contains("not delivered"), "{fault}");
    assert_eq!(
        loop_ctrl.control_mode(),
        ControlMode::Disabled,
        "no Position hold is preserved across a comm watchdog"
    );
    for motor in loop_ctrl.supervisor().motors.motors.clone() {
        assert!(disables_to(&loop_ctrl, motor.device_id) >= 1);
    }
}

#[test]
fn tick_error_reports_a_stop_that_was_not_delivered() {
    let mut loop_ctrl = loop_with(FlakyBus {
        refuse_disable: true,
        ..FlakyBus::default()
    });
    loop_ctrl.set_control_mode(ControlMode::GravityComp);

    let fault = stop_after_tick_error(
        &mut loop_ctrl,
        &mut TickStopPacer::default(),
        &comm_watchdog(),
        Instant::now(),
    );

    assert!(
        fault.contains("stop after tick failure not delivered"),
        "the published fault hid the failed stop: {fault}"
    );
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    assert!(loop_ctrl.supervisor().has_latched_fault());
}

/// DAV-01: a sticky drive fault fails every drain while latched. Once the
/// drives are stopped, the tick-error path re-stops at most once a second
/// instead of every tick, but always retries a stop that was not delivered.
#[test]
fn latched_tick_errors_restop_at_most_once_a_second() {
    let refusing = Arc::new(AtomicBool::new(true));
    let mut loop_ctrl = loop_with(FlakyBus {
        refuse_disable_switch: Some(Arc::clone(&refusing)),
        ..FlakyBus::default()
    });
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    let mut pacer = TickStopPacer::default();
    let t0 = Instant::now();
    let device = loop_ctrl.supervisor().motors.motors[0].device_id;

    stop_after_tick_error(&mut loop_ctrl, &mut pacer, &comm_watchdog(), t0);
    assert!(loop_ctrl.supervisor().has_latched_fault());
    refusing.store(false, Ordering::SeqCst);

    let tick = Duration::from_millis(5);
    stop_after_tick_error(&mut loop_ctrl, &mut pacer, &comm_watchdog(), t0 + tick);
    let after_retry = disables_to(&loop_ctrl, device);
    assert!(
        after_retry >= 1,
        "an undelivered stop is retried on the next tick"
    );

    for k in 2..=100 {
        let fault =
            stop_after_tick_error(&mut loop_ctrl, &mut pacer, &comm_watchdog(), t0 + tick * k);
        assert!(fault.contains("comm watchdog"), "{fault}");
    }
    assert_eq!(
        disables_to(&loop_ctrl, device),
        after_retry,
        "no stop storm while latched and already stopped"
    );

    stop_after_tick_error(
        &mut loop_ctrl,
        &mut pacer,
        &comm_watchdog(),
        t0 + tick + TickStopPacer::RESTOP_INTERVAL,
    );
    assert!(
        disables_to(&loop_ctrl, device) > after_retry,
        "the stop is still repeated once a second"
    );
}

fn set_zero(confirm: bool, sign_test_passed: bool, joint: &str) -> SetZeroRequest {
    SetZeroRequest {
        timestamp_ms: 0,
        operator_id: String::new(),
        joint: joint.into(),
        confirm,
        sign_test_passed,
    }
}

#[test]
fn chappe_set_zero_requires_confirm_and_sign_test_and_a_configured_joint() {
    let loop_ctrl = loop_with(FlakyBus::default());
    let mut queue = queue();
    let cases = [
        (set_zero(false, true, JOINT), "confirm"),
        (set_zero(true, false, JOINT), "sign-tested required"),
        (set_zero(true, true, "no_such_joint"), "unknown joint"),
        (set_zero(true, true, "  "), "unknown joint"),
    ];
    for (request, expected) in cases {
        let error =
            handle_chappe_set_zero(&loop_ctrl, &mut queue, &request).expect_err("refused request");
        assert!(error.contains(expected), "{request:?}: {error}");
        assert!(!queue.is_busy(), "a refusal queued {request:?}");
    }
}

#[test]
fn chappe_set_zero_queues_one_joint_and_trims_the_name() {
    let loop_ctrl = loop_with(FlakyBus::default());
    let mut queue = queue();

    handle_chappe_set_zero(
        &loop_ctrl,
        &mut queue,
        &set_zero(true, true, " right_elbow_pitch "),
    )
    .expect("admitted");

    assert!(queue.is_busy());
    let second = handle_chappe_set_zero(&loop_ctrl, &mut queue, &set_zero(true, true, JOINT))
        .expect_err("queue is busy");
    assert!(second.contains("busy"), "{second}");
}

#[test]
fn chappe_enable_is_refused_while_a_reference_is_queued() {
    let mut loop_ctrl = loop_with(FlakyBus::default());
    let mut queue = queue();
    handle_chappe_set_zero(&loop_ctrl, &mut queue, &set_zero(true, true, JOINT)).expect("queued");
    let enable = EnableRequest {
        timestamp_ms: 0,
        operator_id: "test".into(),
        enable: true,
    };

    let error = handle_chappe_enable(
        &mut loop_ctrl,
        &mut queue,
        &mut EnableGate::default(),
        &enable,
    )
    .expect_err("enable under a queued reference");

    assert!(error.contains("reference queue busy"), "{error}");
    assert!(loop_ctrl
        .supervisor()
        .bus()
        .inner
        .tx
        .iter()
        .all(|frame| (frame.id >> 24) & 0x1f != 3));
}

#[test]
fn both_enable_owners_refuse_gravity_preflight_without_feedback() {
    let mut loop_ctrl = loop_with(FlakyBus::default());
    let mut queue = queue();
    let request = EnableRequest {
        timestamp_ms: 0,
        operator_id: "test".into(),
        enable: true,
    };
    let mut gate = EnableGate::default();
    let error = handle_chappe_enable(&mut loop_ctrl, &mut queue, &mut gate, &request)
        .expect_err("Chappe enable must require measured gravity pose");
    assert!(error.contains("gravity saturation"), "{error}");
    assert!(!gate.preflight_pending());

    let command = parse_command("enable test force").expect("forced enable parses");
    handle_command(
        &mut loop_ctrl,
        &mut queue,
        &mut gate,
        command,
        &repo_root().join("config"),
    );

    assert!(
        loop_ctrl
            .supervisor()
            .bus()
            .inner
            .tx
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1f != 3),
        "no Enable frame may be sent without the gravity preflight evidence"
    );
}

/// RT-04: a panic in the control loop stops every drive before unwinding on.
#[test]
fn panic_in_the_control_loop_stops_every_drive() {
    let mut loop_ctrl = loop_with(FlakyBus::default());
    loop_ctrl.allow_implicit_enable();
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    let panicked = run_with_panic_stop(&mut loop_ctrl, |_| panic!("injected control-loop panic"));
    assert!(panicked);
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    assert!(loop_ctrl.implicit_enable_forbidden());
    for motor in loop_ctrl.supervisor().motors.motors.clone() {
        assert!(
            disables_to(&loop_ctrl, motor.device_id) >= 1,
            "{} not disabled after a panic",
            motor.joint
        );
    }
}

#[test]
fn a_loop_that_returns_normally_is_not_stopped_by_the_guard() {
    let mut loop_ctrl = loop_with(FlakyBus::default());
    let panicked = run_with_panic_stop(&mut loop_ctrl, |_| {});
    assert!(!panicked);
    assert_eq!(disables_to(&loop_ctrl, 4), 0);
}
