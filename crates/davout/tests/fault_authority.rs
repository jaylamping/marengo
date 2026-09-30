//! Persistent safety authority through the public Supervisor and recorded CAN bus.
//! No connection to physical hardware is made by these tests.
#![allow(clippy::expect_used)]

use davout::{JointCommand, MitJointCommand, OperationalMode, SpeedCommand, Supervisor};
use marengo_config::MotorEntry;
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorBus};

fn supervisor() -> Supervisor<MemoryBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_repo(root, MemoryBus::default()).expect("repo fixture");
    supervisor.control.control.comm_watchdog_ms = 1000;
    supervisor
}

fn motor(supervisor: &Supervisor<MemoryBus>, joint: &str) -> MotorEntry {
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

fn activate(supervisor: &mut Supervisor<MemoryBus>, motor: &MotorEntry) {
    supervisor
        .homing_registry_mut()
        .bench_mark_all_verified(std::slice::from_ref(motor))
        .expect("recorded reference");
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("recorded enable");
    supervisor.bus_mut().rx_queue.push(status(motor));
    supervisor.drain_feedback().expect("post-enable pose");
}

#[test]
fn later_healthy_pose_cannot_resume_motion_after_a_device_fault() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().rx_queue.push(fault(&pitch));
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().rx_queue.push(status(&pitch));
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().tx.clear();

    let result = supervisor.send_mit_batch(vec![command(&pitch)]);
    assert!(
        result.is_err(),
        "healthy feedback erased the motion-blocking fault"
    );
    assert!(
        supervisor.bus_mut().tx.is_empty(),
        "motion emitted after fault"
    );
}

fn assert_unexpected_drive_mode_blocks_motion(mode: u32) {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    let mut unexpected = status(&pitch);
    unexpected.id = (unexpected.id & !(3 << 22)) | (mode << 22);
    supervisor.bus_mut().rx_queue.push(unexpected);
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().tx.clear();
    let result = supervisor.send_mit_batch(vec![command(&pitch)]);
    assert!(
        result.is_err(),
        "drive mode {mode} authorized active torque"
    );
    assert!(supervisor.bus_mut().tx.is_empty());
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
    supervisor.bus_mut().tx.clear();
    let mut invalid = command(&pitch);
    invalid.kp = 100_000.0;
    assert!(supervisor.send_mit_batch(vec![invalid]).is_err());
    assert!(
        supervisor.bus_mut().tx.is_empty(),
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
    supervisor.bus_mut().tx.clear();
    let mut invalid = command(&pitch);
    invalid.torque_ff_nm = 0.0;
    invalid.position_rad = 100_000.0;
    assert!(supervisor.send_mit_batch(vec![invalid]).is_err());
    assert!(
        supervisor.bus_mut().tx.is_empty(),
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
    supervisor.bus_mut().rx_queue.push(disabled_peer);
    supervisor.bus_mut().tx.clear();
    supervisor
        .drain_feedback()
        .expect("inactive peer Reset is diagnostic");
    assert!(supervisor.bus_mut().tx.is_empty());
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("active pitch remains permitted");
}

#[test]
fn homing_ready_transition_cannot_hide_physically_active_drives() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let all = supervisor.motors.motors.clone();
    supervisor
        .homing_registry_mut()
        .bench_mark_all_verified(&all)
        .expect("all reference");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor.set_homing_complete().is_err(),
        "Active changed to Ready"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Active);
    assert!(supervisor.joint_drive_active(&pitch.joint));
    assert!(supervisor.bus_mut().tx.is_empty());
}

#[test]
fn unchecked_ready_helper_cannot_hide_physically_active_drives() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.set_homing_complete_unchecked();
    assert_eq!(supervisor.mode(), OperationalMode::Active);
    assert!(supervisor.joint_drive_active(&pitch.joint));
}

#[test]
fn calibration_enable_cannot_be_requested_over_active_motion() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor.request_enable_for_calibration().is_err(),
        "calibration accepted over Active"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Active);
    assert!(supervisor.bus_mut().tx.is_empty());
}

#[test]
fn hardware_estop_release_cannot_rearm_without_qualified_recovery() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor.set_hardware_estop(true);
    supervisor.set_hardware_estop(false);
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor
            .enable_targets(std::slice::from_ref(&pitch.joint))
            .is_err(),
        "released input cleared safety authority"
    );
    assert!(supervisor.bus_mut().tx.is_empty());
}

#[test]
fn hardware_input_assertion_attempts_disable_for_every_configured_motor() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    let motors = supervisor.motors.motors.clone();
    supervisor.bus_mut().tx.clear();
    supervisor.set_hardware_estop(true);
    for motor in motors {
        assert!(
            supervisor
                .bus_mut()
                .tx
                .iter()
                .any(|frame| frame.id >> 24 == 4 && frame.id & 0xff == u32::from(motor.device_id)),
            "no disable attempt for {}",
            motor.joint
        );
    }
}

#[test]
fn every_motion_enable_and_calibration_route_remains_blocked_after_fault() {
    let mut accepted = Vec::new();
    for route in 0..9 {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        activate(&mut supervisor, &pitch);
        supervisor.bus_mut().rx_queue.push(fault(&pitch));
        let _ = supervisor.drain_feedback();
        supervisor.bus_mut().rx_queue.push(status(&pitch));
        let _ = supervisor.drain_feedback();
        supervisor.control.control.bench.allow_firmware_speed_mode = true;
        if route >= 7 {
            let _ = supervisor.disable_all();
        }
        supervisor.bus_mut().tx.clear();
        let result = match route {
            0 => supervisor.send_mit_joint(command(&pitch), &pitch),
            1 => supervisor.send_mit_batch(vec![command(&pitch)]),
            2 => supervisor.send_joint_command(JointCommand {
                joint: pitch.joint.clone(),
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.1,
            }),
            3 => supervisor
                .send_speed_command(SpeedCommand {
                    joint: pitch.joint.clone(),
                    velocity_rad_s: 0.1,
                })
                .map(|_| ()),
            4 => supervisor.set_zero_position(&pitch.joint),
            5 => supervisor.request_enable(true),
            6 => supervisor.enable_targets(std::slice::from_ref(&pitch.joint)),
            7 => supervisor.request_enable_for_calibration(),
            _ => supervisor
                .calibrate_joint_zero(&pitch.joint, "virtual-test", false)
                .map(|_| ()),
        };
        if result.is_ok() || !supervisor.bus_mut().tx.is_empty() {
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
    supervisor.bus_mut().rx_queue.push(fault(&pitch));
    let _ = supervisor.drain_feedback();
    assert!(supervisor.joint_feedback(&pitch.joint).is_none());
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor
            .enable_targets(std::slice::from_ref(&pitch.joint))
            .is_err(),
        "fault-only evidence did not block enable"
    );
    assert!(supervisor.bus_mut().tx.is_empty());
}

#[test]
fn same_drain_fault_cannot_disappear_before_healthy_status() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, &pitch);
    supervisor
        .bus_mut()
        .rx_queue
        .extend([fault(&pitch), status(&pitch)]);
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "same-drain healthy status erased fault"
    );
    assert!(supervisor.bus_mut().tx.is_empty());
}

#[derive(Default)]
struct FailingBus {
    tx: Vec<CanFrame>,
    rx: Vec<CanFrame>,
    fail_receive: bool,
    fail_send_type: Option<u32>,
}

impl CanBus for FailingBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.tx.push(frame.clone());
        if self.fail_send_type == Some(frame.id >> 24) {
            return Err(BusError::Send {
                message: "injected delivery failure".into(),
            });
        }
        Ok(())
    }

    fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        out.append(&mut self.rx);
        if self.fail_receive {
            self.fail_receive = false;
            return Err(BusError::Driver(
                "injected receive failure after prefix".into(),
            ));
        }
        Ok(())
    }
}

impl MotorBus for FailingBus {}

fn failing_supervisor() -> (Supervisor<FailingBus>, MotorEntry) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_repo(root, FailingBus::default()).expect("repo fixture");
    supervisor.control.control.comm_watchdog_ms = 1000;
    let pitch = supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_shoulder_pitch")
        .expect("pitch")
        .clone();
    supervisor
        .homing_registry_mut()
        .bench_mark_all_verified(std::slice::from_ref(&pitch))
        .expect("reference");
    supervisor
        .enable_targets(std::slice::from_ref(&pitch.joint))
        .expect("enable");
    supervisor.bus_mut().rx.push(status(&pitch));
    supervisor.drain_feedback().expect("new Run status");
    (supervisor, pitch)
}

#[test]
fn received_fault_prefix_survives_terminal_rx_error_and_healthy_retry() {
    let (mut supervisor, pitch) = failing_supervisor();
    supervisor.bus_mut().rx.push(fault(&pitch));
    supervisor.bus_mut().fail_receive = true;
    assert!(supervisor.drain_feedback().is_err());
    supervisor.bus_mut().rx.push(status(&pitch));
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "terminal read error discarded fault prefix"
    );
    assert!(supervisor.bus_mut().tx.is_empty());
}

#[test]
fn failed_disable_attempts_all_addresses_and_reports_failure() {
    let (mut supervisor, _) = failing_supervisor();
    let motors = supervisor.motors.motors.clone();
    supervisor.bus_mut().fail_send_type = Some(4);
    supervisor.bus_mut().tx.clear();
    let result = supervisor.disable_all();
    for motor in &motors {
        assert!(
            supervisor
                .bus_mut()
                .tx
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
}

#[test]
fn set_zero_transport_uncertainty_cannot_be_cleared_by_healthy_feedback() {
    let (mut supervisor, pitch) = failing_supervisor();
    supervisor.bus_mut().fail_send_type = Some(6);
    assert!(supervisor.set_zero_position(&pitch.joint).is_err());
    supervisor.bus_mut().fail_send_type = None;
    supervisor.bus_mut().rx.push(status(&pitch));
    let _ = supervisor.drain_feedback();
    supervisor.bus_mut().tx.clear();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "uncertain SetZero delivery restored motion"
    );
    assert!(supervisor.bus_mut().tx.is_empty());
}
