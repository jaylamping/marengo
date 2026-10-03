//! Owner stop paths: Chappe disable, tick-error stop, Chappe set-zero admission.
//! No hardware: a recording bus that can refuse Disable frames.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use armee_proto::{EnableRequest, SetZeroRequest};
use berthier::{ControlLoop, ControlMode, LoopError};
use davout::DavoutError;
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorBus, ReceiveAttempt};

use crate::{
    handle_chappe_enable, handle_chappe_set_zero, stop_after_tick_error, PiReferenceQueue,
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
}

impl CanBus for FlakyBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        if self.refuse_disable && (frame.id >> 24) & 0x1f == 4 {
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

    let result = handle_chappe_enable(&mut loop_ctrl, &mut queue, &disable_request());

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

    handle_chappe_enable(&mut loop_ctrl, &mut queue, &disable_request()).expect("disable");

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

    let fault = stop_after_tick_error(&mut loop_ctrl, &comm_watchdog());

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

    let fault = stop_after_tick_error(&mut loop_ctrl, &comm_watchdog());

    assert!(
        fault.contains("stop after tick failure not delivered"),
        "the published fault hid the failed stop: {fault}"
    );
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    assert!(loop_ctrl.supervisor().has_latched_fault());
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

    let error = handle_chappe_enable(&mut loop_ctrl, &mut queue, &enable)
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
