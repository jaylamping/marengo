//! Persistent safety authority through the public Supervisor and recorded CAN bus.
//! No connection to physical hardware is made by these tests.
#![allow(clippy::expect_used)]
use davout::simulation::{SimulationBus, SimulationReceive, TxMatcher, TxOccurrence, TxRule};

use davout::{MitJointCommand, OperationalMode, Supervisor};
use marengo_config::MotorEntry;
use robstride::CanFrame;

fn supervisor() -> Supervisor<SimulationBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        davout::simulation::InitialVirtualReference::AllConfigured,
    )
    .expect("repo fixture");
    supervisor.control.control.comm_watchdog_ms = 1000;
    supervisor
}

fn motor(supervisor: &Supervisor<SimulationBus>, joint: &str) -> MotorEntry {
    supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == joint)
        .expect("configured joint")
        .clone()
}

fn status(motor: &MotorEntry) -> CanFrame {
    // Independent vendor fixture: type 2, Run mode, motor ID in bits 8..15;
    // near-zero position/velocity/torque and 20 C in the big-endian payload.
    CanFrame {
        id: (2 << 24) | (2 << 22) | (u32::from(motor.device_id) << 8) | 0xfd,
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8],
        extended: true,
    }
}

fn fault(motor: &MotorEntry) -> CanFrame {
    // Bit zero is overtemperature in either byte order when the word is all ones;
    // this initial persistence test deliberately does not qualify word endianness.
    CanFrame {
        id: (21 << 24) | (u32::from(motor.device_id) << 8) | 0xfd,
        data: [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0],
        extended: true,
    }
}

fn command(motor: &MotorEntry) -> MitJointCommand {
    MitJointCommand {
        joint: motor.joint.clone(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.1,
    }
}

fn activate(supervisor: &mut Supervisor<SimulationBus>, motor: &MotorEntry) {
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("recorded enable");
    supervisor
        .bus_mut()
        .queue_frame(status(motor))
        .expect("finite closed script");
    supervisor.drain_feedback().expect("post-enable pose");
}

#[test]
fn later_healthy_pose_cannot_resume_motion_after_a_device_fault() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor
        .bus_mut()
        .queue_frame(fault(&pitch))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    supervisor
        .bus_mut()
        .queue_frame(status(&pitch))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().clear_trace();

    let result = supervisor.send_mit_batch(vec![command(&pitch)]);
    assert!(
        result.is_err(),
        "healthy feedback erased the motion-blocking fault"
    );
    assert!(
        supervisor.bus().frames().is_empty(),
        "motion emitted after fault"
    );
}

fn assert_unexpected_drive_mode_blocks_motion(mode: u32) {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    let mut unexpected = status(&pitch);
    unexpected.id = (unexpected.id & !(3 << 22)) | (mode << 22);
    supervisor
        .bus_mut()
        .queue_frame(unexpected)
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().clear_trace();
    let result = supervisor.send_mit_batch(vec![command(&pitch)]);
    assert!(
        result.is_err(),
        "drive mode {mode} authorized active torque"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn current_enable_reset_status_cannot_authorize_motion() {
    assert_unexpected_drive_mode_blocks_motion(0);
}

#[test]
fn current_enable_calibration_status_cannot_authorize_motion() {
    assert_unexpected_drive_mode_blocks_motion(1);
}

#[test]
fn reserved_status_mode_cannot_authorize_motion() {
    assert_unexpected_drive_mode_blocks_motion(3);
}

#[test]
fn invalid_finite_gain_request_does_not_latch_or_emit_a_stop() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().clear_trace();
    let mut invalid = command(&pitch);
    invalid.kp = 100_000.0;
    assert!(supervisor.send_mit_batch(vec![invalid]).is_err());
    assert!(
        supervisor.bus().frames().is_empty(),
        "rejected gain request emitted stop/motion"
    );
    let mut neutral = command(&pitch);
    neutral.torque_ff_nm = 0.0;
    supervisor
        .send_mit_batch(vec![neutral])
        .expect("valid neutral after rejection");
}

#[test]
fn invalid_neutral_position_request_does_not_latch_or_emit_a_stop() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().clear_trace();
    let mut invalid = command(&pitch);
    invalid.torque_ff_nm = 0.0;
    invalid.position_rad = 100_000.0;
    assert!(supervisor.send_mit_batch(vec![invalid]).is_err());
    assert!(
        supervisor.bus().frames().is_empty(),
        "rejected neutral position emitted stop/motion"
    );
    let mut neutral = command(&pitch);
    neutral.torque_ff_nm = 0.0;
    supervisor
        .send_mit_batch(vec![neutral])
        .expect("valid neutral after rejection");
}

#[test]
fn scoped_disabled_peer_reset_status_does_not_stop_active_joint() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let roll = motor(&supervisor, "right_shoulder_roll");
    activate(&mut supervisor, &pitch);
    let mut disabled_peer = status(&roll);
    disabled_peer.id &= !(3 << 22); // Disabled peer reports Reset, as expected.
    supervisor
        .bus_mut()
        .queue_frame(disabled_peer)
        .expect("finite closed script");
    supervisor.bus_mut().clear_trace();
    supervisor
        .drain_feedback()
        .expect("inactive peer Reset is diagnostic");
    assert!(supervisor.bus().frames().is_empty());
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("active pitch remains permitted");
}

#[test]
fn homing_ready_transition_cannot_hide_physically_active_drives() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.set_homing_complete().is_err(),
        "Active changed to Ready"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Active);
    assert!(supervisor.joint_drive_active(&pitch.joint));
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn calibration_enable_cannot_be_requested_over_active_motion() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.request_enable_for_calibration().is_err(),
        "calibration accepted over Active"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Active);
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn hardware_estop_release_cannot_rearm_without_qualified_recovery() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.set_hardware_estop(true);
    supervisor.set_hardware_estop(false);
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor
            .enable_targets(std::slice::from_ref(&pitch.joint))
            .is_err(),
        "released input cleared safety authority"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn hardware_input_assertion_attempts_disable_for_every_configured_motor() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    let motors = supervisor.motors.motors.clone();
    supervisor.bus_mut().clear_trace();
    supervisor.set_hardware_estop(true);
    for motor in motors {
        assert!(
            supervisor
                .bus()
                .frames()
                .iter()
                .any(|frame| frame.id >> 24 == 4 && frame.id & 0xff == u32::from(motor.device_id)),
            "no disable attempt for {}",
            motor.joint
        );
    }
}

#[test]
fn every_supported_motion_enable_and_calibration_route_remains_blocked_after_fault() {
    let mut accepted = Vec::new();
    for route in [0, 1, 4, 5, 6, 7, 8] {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        activate(&mut supervisor, &pitch);
        supervisor
            .bus_mut()
            .queue_frame(fault(&pitch))
            .expect("finite closed script");
        let _ = supervisor.drain_feedback();
        supervisor
            .bus_mut()
            .queue_frame(status(&pitch))
            .expect("finite closed script");
        let _ = supervisor.drain_feedback();
        if route >= 7 {
            let _ = supervisor.disable_all();
        }
        supervisor.bus_mut().clear_trace();
        let result = match route {
            0 => supervisor.send_mit_joint(command(&pitch), &pitch),
            1 => supervisor.send_mit_batch(vec![command(&pitch)]),
            4 => supervisor.set_zero_position(&pitch.joint),
            5 => supervisor.request_enable(true),
            6 => supervisor.enable_targets(std::slice::from_ref(&pitch.joint)),
            7 => supervisor.request_enable_for_calibration(),
            _ => supervisor
                .calibrate_joint_zero(&pitch.joint, "virtual-test", false)
                .map(|_| ()),
        };
        if result.is_ok() || !supervisor.bus().frames().is_empty() {
            accepted.push(route);
        }
    }
    assert!(
        accepted.is_empty(),
        "faulted routes accepted or emitted: {accepted:?}"
    );
}

#[test]
fn fault_only_without_pose_blocks_targeted_enable() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    supervisor
        .bus_mut()
        .queue_frame(fault(&pitch))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    assert!(supervisor.joint_feedback(&pitch.joint).is_none());
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor
            .enable_targets(std::slice::from_ref(&pitch.joint))
            .is_err(),
        "fault-only evidence did not block enable"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn same_drain_fault_cannot_disappear_before_healthy_status() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor
        .bus_mut()
        .queue_frames([fault(&pitch), status(&pitch)])
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "same-drain healthy status erased fault"
    );
    assert!(supervisor.bus().frames().is_empty());
}

fn failing_supervisor() -> (Supervisor<SimulationBus>, MotorEntry) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        davout::simulation::InitialVirtualReference::AllConfigured,
    )
    .expect("repo fixture");
    supervisor.control.control.comm_watchdog_ms = 1000;
    let pitch = supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_shoulder_pitch")
        .expect("pitch")
        .clone();
    supervisor
        .enable_targets(std::slice::from_ref(&pitch.joint))
        .expect("enable");
    supervisor
        .bus_mut()
        .queue_frame(status(&pitch))
        .expect("finite closed script");
    supervisor.drain_feedback().expect("new Run status");
    (supervisor, pitch)
}

#[test]
fn received_fault_prefix_survives_terminal_rx_error_and_healthy_retry() {
    let (mut supervisor, pitch) = failing_supervisor();
    supervisor
        .bus_mut()
        .queue_frame(fault(&pitch))
        .expect("finite closed script");
    supervisor
        .bus_mut()
        .queue_attempts([SimulationReceive::Error(
            "injected receive failure after prefix".into(),
        )])
        .expect("terminal error script");
    assert!(supervisor.drain_feedback().is_err());
    supervisor
        .bus_mut()
        .queue_frame(status(&pitch))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "terminal read error discarded fault prefix"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn failed_disable_attempts_all_addresses_and_reports_failure() {
    let (mut supervisor, _) = failing_supervisor();
    let motors = supervisor.motors.motors.clone();
    let rule = supervisor
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(4),
                ..TxMatcher::default()
            },
            occurrence: TxOccurrence::Every,
            receive: vec![],
            send_error: Some("injected delivery failure".into()),
        })
        .expect("all-disable failure rule");
    supervisor.bus_mut().clear_trace();
    let result = supervisor.disable_all();
    for motor in &motors {
        assert!(
            supervisor
                .bus()
                .frames()
                .iter()
                .any(|frame| frame.id >> 24 == 4 && frame.id & 0xff == u32::from(motor.device_id)),
            "no later disable attempt for {}",
            motor.joint
        );
    }
    assert!(
        result.is_err(),
        "failed physical stop was reported as success"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(supervisor.bus().rule_trigger_count(rule), motors.len());
}

#[test]
fn unqualified_set_zero_refuses_before_delivery_without_latching() {
    let (mut supervisor, pitch) = failing_supervisor();
    supervisor.bus_mut().clear_trace();
    assert!(matches!(
        supervisor.set_zero_position(&pitch.joint),
        Err(davout::DavoutError::ReferenceUnsupported { .. })
    ));
    assert!(supervisor.bus().frames().is_empty());
    assert!(!supervisor.has_latched_fault());
    supervisor
        .bus_mut()
        .queue_frame(status(&pitch))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().clear_trace();
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("refused request did not mutate reference");
    assert_eq!(supervisor.bus().frames().len(), 1);
}
