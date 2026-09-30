//! Public persistent fault/evidence contracts; all transports are local recordings.
#![allow(clippy::expect_used)]

use std::time::Instant;

use davout::{FaultClass, MitJointCommand, StopAction, Supervisor};
use marengo_config::MotorEntry;
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorBus, ReceiveAttempt};

#[derive(Default)]
struct EqualTimeBus {
    inner: MemoryBus,
    received_at: Option<Instant>,
}

impl CanBus for EqualTimeBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.inner.send_frame(frame)
    }

    fn begin_receive(&mut self) {
        self.received_at = Some(Instant::now());
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let mut attempt = self.inner.recv_one_nonblocking()?;
        if let ReceiveAttempt::Frame(frame) = &mut attempt {
            frame.received_at = self.received_at.unwrap_or_else(Instant::now);
        }
        Ok(attempt)
    }
}

impl MotorBus for EqualTimeBus {}

fn supervisor<B: MotorBus + Default>() -> Supervisor<B> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_repo(root, B::default()).expect("repo fixture");
    supervisor.control.control.comm_watchdog_ms = 1000;
    supervisor
}

fn motor<B: MotorBus>(supervisor: &Supervisor<B>, name: &str) -> MotorEntry {
    supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == name)
        .expect("joint")
        .clone()
}

fn status(motor: &MotorEntry) -> CanFrame {
    CanFrame {
        id: (2 << 24) | (2 << 22) | (u32::from(motor.device_id) << 8) | 0xfd,
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        extended: true,
    }
}

fn activate<B: MotorBus>(supervisor: &mut Supervisor<B>, motors: &[MotorEntry]) {
    supervisor
        .homing_registry_mut()
        .bench_mark_all_verified(motors)
        .expect("reference");
    let names: Vec<_> = motors.iter().map(|motor| motor.joint.clone()).collect();
    supervisor.enable_targets(&names).expect("enable");
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

#[test]
fn equal_timestamp_unsafe_pose_cannot_disappear_between_healthy_statuses() {
    let mut supervisor = supervisor::<EqualTimeBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().inner.rx_queue.push(status(&pitch));
    supervisor.drain_feedback().expect("initial Run status");

    let mut unsafe_pose = status(&pitch);
    unsafe_pose.data[0..2].copy_from_slice(&[0xff, 0xff]); // Vendor +12.5 rad, outside hard bounds.
    supervisor
        .bus_mut()
        .inner
        .rx_queue
        .extend([status(&pitch), unsafe_pose, status(&pitch)]);
    assert!(
        supervisor.drain_feedback().is_err(),
        "equal-time unsafe pose was skipped"
    );
    assert!(supervisor.safety_snapshot().is_latched());
    assert_eq!(
        supervisor
            .safety_snapshot()
            .first_fault()
            .expect("fault")
            .class,
        FaultClass::Feedback
    );
    supervisor.bus_mut().inner.tx.clear();
    assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
    assert!(supervisor.bus_mut().inner.tx.is_empty());
}

#[test]
fn safe_queued_pose_burst_does_not_treat_host_dequeue_spacing_as_motion() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_repo(root, MemoryBus::default()).expect("repo fixture");
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    let frame = |position: u16| {
        let mut frame = status(&pitch);
        frame.data[0..2].copy_from_slice(&position.to_be_bytes());
        frame.data[2..4].copy_from_slice(&[0x83, 0x11]); // Vendor RS03 velocity ~= 1.2 rad/s.
        frame
    };
    supervisor.bus_mut().rx_queue.push(frame(0x7fff));
    supervisor.drain_feedback().expect("initial Run pose");

    // Twelve 16-count position steps represent ~0.074 rad over 60 ms, within the 2.5 rad/s
    // envelope. A queued burst can arrive at the host in microseconds; those dequeue intervals
    // cannot represent the physical spacing of the motor's samples.
    std::thread::sleep(std::time::Duration::from_millis(60));
    supervisor
        .bus_mut()
        .rx_queue
        .extend((1..=12).map(|step| frame(0x7fff + step * 16)));
    assert_eq!(supervisor.drain_feedback().expect("safe backlog"), 12);
    assert!(!supervisor.safety_snapshot().is_latched());
    let feedback = supervisor
        .joint_feedback(&pitch.joint)
        .expect("latest pose");
    assert!(feedback.velocity_rad_s.is_finite());
    assert!(feedback.velocity_rad_s.abs() <= 2.5);
    supervisor.bus_mut().tx.clear();
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("safe backlog preserves motion admission");
    assert_eq!(supervisor.bus_mut().tx.len(), 1);
}

fn detailed(motor: &MotorEntry, raw: [u8; 8]) -> CanFrame {
    CanFrame {
        id: (21 << 24) | (u32::from(motor.device_id) << 8) | 0xfd,
        data: raw,
        extended: true,
    }
}

#[derive(Default)]
struct EvidenceBus {
    inner: MemoryBus,
    fail_receive: bool,
    fail_disables: bool,
}

impl CanBus for EvidenceBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.inner.send_frame(frame)?;
        if self.fail_disables && frame.id >> 24 == 4 {
            return Err(BusError::Send {
                message: "virtual disable delivery unknown".into(),
            });
        }
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let attempt = self.inner.recv_one_nonblocking()?;
        if matches!(attempt, ReceiveAttempt::Idle) && self.fail_receive {
            self.fail_receive = false;
            return Err(BusError::Driver(
                "terminal error after observed prefix".into(),
            ));
        }
        Ok(attempt)
    }
}

impl MotorBus for EvidenceBus {}

#[test]
fn invalid_pose_peer_fault_and_terminal_error_all_survive_one_drain() {
    let mut supervisor = supervisor::<EvidenceBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let roll = motor(&supervisor, "right_shoulder_roll");
    activate(&mut supervisor, &[pitch.clone(), roll.clone()]);
    supervisor
        .bus_mut()
        .inner
        .rx_queue
        .extend([status(&pitch), status(&roll)]);
    supervisor.drain_feedback().expect("initial poses");
    let mut invalid_pose = status(&pitch);
    invalid_pose.data[0..2].copy_from_slice(&[0xff, 0xff]);
    let peer_raw = [0x00, 0x00, 0x01, 0x80, 0x02, 0x00, 0x00, 0x40];
    supervisor.bus_mut().inner.rx_queue.extend([
        invalid_pose,
        detailed(&roll, peer_raw),
        status(&roll),
    ]);
    supervisor.bus_mut().fail_receive = true;
    assert!(supervisor.drain_feedback().is_err());

    let snapshot = supervisor.safety_snapshot();
    assert_eq!(
        snapshot.first_fault().expect("first cause").class,
        FaultClass::Feedback
    );
    let peer = snapshot
        .faults
        .iter()
        .find(|fault| {
            fault.class == FaultClass::Device && fault.joint.as_deref() == Some(roll.joint.as_str())
        })
        .expect("peer device fault retained");
    assert_eq!(peer.device.detailed_fault_bytes, [0, 0, 1, 0x80]);
    assert_eq!(peer.device.warning_bytes, [2, 0, 0, 0x40]);
    assert!(snapshot
        .faults
        .iter()
        .any(|fault| fault.class == FaultClass::Transport));
    assert!(snapshot.last_stop.is_some());
    assert!(!snapshot.recovery_available);
}

#[test]
fn full_raw_domains_and_fault_identity_survive_healthy_pose_cache_and_replay() {
    let mut supervisor = supervisor::<MemoryBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().rx_queue.push(status(&pitch));
    supervisor.drain_feedback().expect("initial pose");
    let mut flagged = status(&pitch);
    flagged.id |= 1 << 16;
    supervisor.bus_mut().rx_queue.extend([
        detailed(&pitch, [1, 2, 0x80, 0x40, 2, 0x80, 4, 0x20]),
        detailed(&pitch, [0x80, 1, 0, 4, 1, 0x40, 0x80, 0]),
        flagged,
        status(&pitch),
    ]);
    assert!(supervisor.drain_feedback().is_err());
    let first = supervisor.safety_snapshot();
    let fault = first.first_fault().expect("first fault");
    assert_eq!(fault.id, 1);
    assert_eq!(fault.class, FaultClass::Device);
    assert_eq!(fault.device.status_flags, 1);
    assert_eq!(fault.device.detailed_fault_bytes, [0x81, 3, 0x80, 0x44]);
    assert_eq!(fault.device.warning_bytes, [3, 0xc0, 0x84, 0x20]);
    assert_eq!(fault.device.drive_mode, Some(2));

    supervisor.bus_mut().rx_queue.push(status(&pitch));
    supervisor
        .drain_feedback()
        .expect("healthy diagnostics are still available");
    let later = supervisor.safety_snapshot();
    assert_eq!(later.first_fault(), first.first_fault());
    assert_eq!(later.stop_generation, first.stop_generation);
    supervisor.clear_motor_states();
    supervisor.seed_synthetic_feedback();
    supervisor
        .set_synthetic_joint_feedback(&pitch.joint, 0.0, 0.0)
        .expect("replay pose");
    assert_eq!(supervisor.safety_snapshot(), later);
    assert_ne!(
        supervisor
            .joint_feedback(&pitch.joint)
            .expect("diagnostic pose")
            .fault,
        0
    );
    assert!(supervisor.check_fault_authority().is_err());
}

#[test]
fn first_cause_and_failed_stop_evidence_survive_later_successful_stop() {
    let mut supervisor = supervisor::<EvidenceBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let motor_count = supervisor.motors.motors.len();
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().inner.rx_queue.push(status(&pitch));
    supervisor.drain_feedback().expect("pose");
    supervisor.bus_mut().fail_disables = true;
    supervisor
        .bus_mut()
        .inner
        .rx_queue
        .push(detailed(&pitch, [0, 0, 0, 0x80, 0, 0, 0, 0]));
    assert!(supervisor.drain_feedback().is_err());
    let failed = supervisor.safety_snapshot();
    assert_eq!(
        failed.first_fault().expect("cause").class,
        FaultClass::Device
    );
    assert!(failed
        .faults
        .iter()
        .any(|fault| fault.class == FaultClass::StopDelivery));
    let report = failed.last_stop.as_ref().expect("all attempts");
    assert_eq!(report.attempts.len(), motor_count * 3);
    assert_eq!(report.failed_writes(), motor_count);
    assert_eq!(
        report
            .attempts
            .iter()
            .filter(|attempt| attempt.action == StopAction::Disable)
            .count(),
        motor_count
    );

    supervisor.bus_mut().fail_disables = false;
    supervisor.disable_all().expect("later writes accepted");
    let later = supervisor.safety_snapshot();
    assert_eq!(later.first_fault(), failed.first_fault());
    assert_eq!(later.first_failed_stop, failed.last_stop);
    assert_eq!(later.last_stop.expect("later stop").failed_writes(), 0);
    assert!(later.stop_generation > failed.stop_generation);
    assert!(supervisor.check_fault_authority().is_err());
}

#[test]
fn fault_only_without_pose_is_visible_in_commissioning_facets() {
    let mut supervisor = supervisor::<MemoryBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    supervisor
        .bus_mut()
        .rx_queue
        .push(detailed(&pitch, [0, 0, 0, 0x80, 0, 0, 0, 0]));
    assert!(supervisor.drain_feedback().is_err());
    let (master, _) = supervisor.commissioning_facets(std::slice::from_ref(&pitch.joint));
    assert!(master[0].fault);
    assert!(!master[0].online);
    assert!(!master[0].drive_active);
    assert!(supervisor.joint_feedback(&pitch.joint).is_none());
}

#[test]
fn warning_only_evidence_is_retained_without_latching_motion() {
    let mut supervisor = supervisor::<MemoryBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().rx_queue.push(status(&pitch));
    supervisor.drain_feedback().expect("pose");
    supervisor
        .bus_mut()
        .rx_queue
        .push(detailed(&pitch, [0, 0, 0, 0, 1, 0x80, 0, 0]));
    supervisor.drain_feedback().expect("warning diagnostics");
    let snapshot = supervisor.safety_snapshot();
    assert!(!snapshot.is_latched());
    assert_eq!(snapshot.observed_warnings.len(), 1);
    assert_eq!(snapshot.observed_warnings[0].warning_bytes, [1, 0x80, 0, 0]);
    supervisor.bus_mut().tx.clear();
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("warning does not grant or revoke permission");
    assert_eq!(supervisor.bus_mut().tx.len(), 1);
}

#[test]
fn prefix_free_receive_failure_is_a_persistent_transport_fault() {
    let mut supervisor = supervisor::<EvidenceBus>();
    supervisor.bus_mut().fail_receive = true;
    assert!(supervisor.drain_feedback().is_err());
    let snapshot = supervisor.safety_snapshot();
    assert_eq!(
        snapshot.first_fault().expect("transport fault").class,
        FaultClass::Transport
    );
    assert!(snapshot.last_stop.is_some());
    supervisor
        .drain_feedback()
        .expect("later empty diagnostics");
    assert_eq!(supervisor.safety_snapshot(), snapshot);
}

#[test]
fn expired_watchdog_remains_a_communication_fault_after_healthy_diagnostics() {
    let mut supervisor = supervisor::<MemoryBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    supervisor.control.control.comm_watchdog_ms = 1;
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().rx_queue.push(status(&pitch));
    supervisor.drain_feedback().expect("pose");
    std::thread::sleep(std::time::Duration::from_millis(3));
    assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
    let snapshot = supervisor.safety_snapshot();
    assert_eq!(
        snapshot.first_fault().expect("watchdog").class,
        FaultClass::Communication
    );
    supervisor.bus_mut().rx_queue.push(status(&pitch));
    supervisor
        .drain_feedback()
        .expect("healthy free-drive diagnostics");
    assert_eq!(
        supervisor.safety_snapshot().first_fault(),
        snapshot.first_fault()
    );
    assert!(supervisor.check_fault_authority().is_err());
}

#[test]
fn newly_asserted_hardware_input_attempts_stop_even_after_an_existing_fault() {
    let mut supervisor = supervisor::<MemoryBus>();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    supervisor
        .bus_mut()
        .rx_queue
        .push(detailed(&pitch, [1, 0, 0, 0, 0, 0, 0, 0]));
    assert!(supervisor.drain_feedback().is_err());
    let first = supervisor.safety_snapshot();
    supervisor.bus_mut().tx.clear();
    supervisor.set_hardware_estop(true);
    let asserted = supervisor.safety_snapshot();
    assert_eq!(asserted.first_fault(), first.first_fault());
    assert!(asserted.stop_generation > first.stop_generation);
    assert!(asserted
        .faults
        .iter()
        .any(|fault| fault.class == FaultClass::HardwareEstop));
    assert_eq!(
        asserted.last_stop.expect("hardware stop").attempts.len(),
        supervisor.motors.motors.len() * 3
    );
    supervisor.set_hardware_estop(false);
    assert!(supervisor.check_fault_authority().is_err());
}
