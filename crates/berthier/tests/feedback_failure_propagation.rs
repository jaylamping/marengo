//! Receive hazards must be returned by the actual tick and mode-entry paths.

#![allow(clippy::expect_used)]

use berthier::{ControlLoop, ControlMode, LoopError};
use davout::simulation::{
    InitialVirtualReference, RuleId, SimulationBus, SimulationReceive, TxMatcher, TxOccurrence,
    TxRule,
};
use davout::DavoutError;
use robstride::{CanFrame, ReceivedCanFrame};

fn controller() -> ControlLoop<SimulationBus> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut controller = ControlLoop::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("virtual initial reference fixture");
    controller
        .supervisor_mut()
        .set_homing_complete()
        .expect("ready");
    controller
}

fn enabled_controller() -> ControlLoop<SimulationBus> {
    let mut controller = controller();
    controller
        .supervisor_mut()
        .enable_targets(&["right_upper_arm_yaw".into()])
        .expect("recording enable");
    controller.supervisor_mut().bus_mut().clear_trace();
    controller
}

fn after_next_mit(
    controller: &mut ControlLoop<SimulationBus>,
    receive: SimulationReceive,
) -> RuleId {
    controller
        .supervisor_mut()
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(1),
                device_id: Some(3),
                interface: None,
            },
            occurrence: TxOccurrence::Nth(1),
            receive: vec![receive],
            send_error: None,
        })
        .expect("finite post-send script")
}

fn receive_failure(controller: &mut ControlLoop<SimulationBus>) {
    controller
        .supervisor_mut()
        .bus_mut()
        .queue_attempts([SimulationReceive::Error("injected receive failure".into())])
        .expect("finite receive failure");
}

fn status() -> ReceivedCanFrame {
    // Manual status: type 2, Run mode 2, motor 3, host 0; centered pose/speed/torque.
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0280_0300,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8],
            extended: true,
        },
    )
}

#[test]
fn bootstrap_tick_returns_post_send_receive_failure() {
    let mut controller = enabled_controller();
    controller.set_control_mode(ControlMode::Impedance);
    let rule = after_next_mit(
        &mut controller,
        SimulationReceive::Error("injected receive failure".into()),
    );
    let result = controller.tick(None);
    assert_eq!(controller.supervisor().bus().rule_trigger_count(rule), 1);
    assert!(
        result.is_err(),
        "post-send receive failure was hidden: {result:?}"
    );
    assert!(
        controller
            .supervisor()
            .bus()
            .frames()
            .iter()
            .any(|frame| (frame.id >> 24) & 0x1f == 1),
        "probe must reach real MIT send"
    );
}

#[test]
fn active_tick_returns_post_send_receive_failure_in_each_control_mode() {
    let mut hidden = Vec::new();
    for mode in [
        ControlMode::GravityComp,
        ControlMode::Impedance,
        ControlMode::Position,
        ControlMode::TorqueOnly,
        ControlMode::Disabled,
    ] {
        let mut controller = enabled_controller();
        controller
            .supervisor_mut()
            .bus_mut()
            .queue_received(status())
            .expect("raw status");
        if mode == ControlMode::Position {
            controller.enter_position_hold().expect("valid hold entry");
        } else {
            controller.set_control_mode(mode);
        }
        let rule = after_next_mit(
            &mut controller,
            SimulationReceive::Error("injected receive failure".into()),
        );
        let result = controller.tick(None);
        assert_eq!(
            controller.supervisor().bus().rule_trigger_count(rule),
            1,
            "{mode:?}: post-send script must fire"
        );
        if result.is_ok() {
            hidden.push(mode);
        }
        assert!(
            controller
                .supervisor()
                .bus()
                .frames()
                .iter()
                .any(|frame| (frame.id >> 24) & 0x1f == 1),
            "{mode:?}: actual MIT send required"
        );
    }
    assert!(
        hidden.is_empty(),
        "post-send receive failure was hidden in {hidden:?}"
    );
}

#[test]
fn post_send_unsafe_pose_cannot_be_reported_as_a_successful_tick() {
    let mut failures = Vec::new();
    for fresh_pose in [false, true] {
        let mut controller = enabled_controller();
        controller.set_control_mode(ControlMode::Impedance);
        if fresh_pose {
            controller
                .supervisor_mut()
                .bus_mut()
                .queue_received(status())
                .expect("raw status");
        }
        let mut unsafe_status = status();
        unsafe_status.frame.data[0..2].copy_from_slice(&[0xff, 0xff]);
        let rule = after_next_mit(&mut controller, SimulationReceive::Received(unsafe_status));
        let result = controller.tick(None);
        assert!(
            controller.supervisor().bus().rule_trigger_count(rule) == 1,
            "probe must reach actual MIT transmission before injecting feedback"
        );
        if !matches!(
            result,
            Err(LoopError::Safety(
                DavoutError::Limit { .. } | DavoutError::InvalidFeedback { .. }
            ))
        ) {
            failures.push(format!("fresh={fresh_pose}: {result:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "unsafe post-send pose was hidden: {failures:?}"
    );
}

#[test]
fn hold_entry_rejects_receive_failure_before_installing_motion_intent() {
    let mut controller = controller();
    receive_failure(&mut controller);
    let result = controller.enter_position_hold();
    assert!(
        matches!(result, Err(LoopError::Safety(DavoutError::Bus(_)))),
        "mode-entry receive failure was hidden: {result:?}"
    );
    assert!(controller.position_setpoints().is_none());
    assert!(
        controller
            .supervisor()
            .bus()
            .frames()
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1f != 3),
        "rejected refresh must not enable"
    );
}

#[test]
fn target_entry_rejects_receive_failure_before_installing_motion_intent() {
    let mut controller = controller();
    receive_failure(&mut controller);
    let result = controller.enter_position_hold_at(Some("right_upper_arm_yaw"), 0.1);
    assert!(
        matches!(result, Err(LoopError::Safety(DavoutError::Bus(_)))),
        "target refresh failure was hidden: {result:?}"
    );
    assert!(controller.position_setpoints().is_none());
    assert!(
        controller
            .supervisor()
            .bus()
            .frames()
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1f != 3),
        "rejected refresh must not enable"
    );
}

#[test]
fn wave_entry_rejects_receive_failure_before_installing_motion_intent() {
    let mut controller = controller();
    receive_failure(&mut controller);
    let result = controller.start_position_wave("right_upper_arm_yaw", -0.1, 0.1, 1, 1.0);
    assert!(
        matches!(result, Err(LoopError::Safety(DavoutError::Bus(_)))),
        "wave refresh failure was hidden: {result:?}"
    );
    assert!(controller.position_setpoints().is_none());
    assert!(!controller.position_wave_active());
    assert!(
        controller
            .supervisor()
            .bus()
            .frames()
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1f != 3),
        "rejected refresh must not enable"
    );
}

#[test]
fn already_observed_device_fault_rejects_hold_entry_without_installing_intent() {
    let mut controller = enabled_controller();
    controller
        .supervisor_mut()
        .bus_mut()
        .queue_received(ReceivedCanFrame::full_data(
            Some("can0".into()),
            CanFrame {
                id: 0x1500_0300,
                data: [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0],
                extended: true,
            },
        ))
        .expect("literal device fault");
    let _ = controller.supervisor_mut().drain_feedback();
    controller.supervisor_mut().bus_mut().clear_trace();
    let result = controller.enter_position_hold();
    assert!(
        result.is_err(),
        "quiet diagnostic refresh must not grant fault recovery: {result:?}"
    );
    assert!(controller.position_setpoints().is_none());
    assert!(controller.supervisor().bus().frames().is_empty());
}

#[test]
fn already_observed_device_fault_rejects_new_torque_intent() {
    let mut controller = enabled_controller();
    controller
        .supervisor_mut()
        .bus_mut()
        .queue_received(ReceivedCanFrame::full_data(
            Some("can0".into()),
            CanFrame {
                id: 0x1500_0300,
                data: [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0],
                extended: true,
            },
        ))
        .expect("literal device fault");
    let _ = controller.supervisor_mut().drain_feedback();
    controller.supervisor_mut().bus_mut().clear_trace();
    let result = controller.set_torque_cmd("right_upper_arm_yaw", 0.5);
    assert!(
        result.is_err(),
        "a fault must refuse new torque intent: {result:?}"
    );
    assert_eq!(controller.torque_cmd("right_upper_arm_yaw"), 0.0);
    assert_eq!(controller.control_mode(), ControlMode::Disabled);
    assert!(controller.supervisor().bus().frames().is_empty());
}

#[test]
fn exhausted_feedback_bootstrap_cannot_be_rearmed_by_disable_and_enable() {
    let mut controller = enabled_controller();
    controller.set_control_mode(ControlMode::Impedance);
    for _ in 0..2 {
        controller.tick(None).expect("bounded neutral bootstrap");
    }
    assert!(
        controller.tick(None).is_err(),
        "missing feedback must expire bootstrap"
    );
    controller
        .supervisor_mut()
        .disable_all()
        .expect("recording stop");
    controller.supervisor_mut().bus_mut().clear_trace();
    let result = controller
        .supervisor_mut()
        .enable_targets(&["right_upper_arm_yaw".into()]);
    assert!(
        result.is_err(),
        "Disable must not clear missing-feedback authority: {result:?}"
    );
    assert!(controller.supervisor().bus().frames().is_empty());
}

#[test]
fn disable_and_enable_between_ticks_cancels_previous_torque_intent() {
    let mut controller = enabled_controller();
    controller
        .set_torque_cmd("right_upper_arm_yaw", 0.5)
        .expect("torque intent");
    assert_eq!(controller.torque_cmd("right_upper_arm_yaw"), 0.5);
    controller
        .supervisor_mut()
        .disable_all()
        .expect("recording stop");
    controller
        .supervisor_mut()
        .enable_targets(&["right_upper_arm_yaw".into()])
        .expect("new explicit enable");
    controller.tick(None).expect("neutral fresh enable tick");
    assert_eq!(
        controller.torque_cmd("right_upper_arm_yaw"),
        0.0,
        "a stopped torque request must not survive a new enable"
    );
}
