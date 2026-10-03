//! Bounded receive work and incomplete-view admission through the public supervisor.
//! All transport traffic is a local recording; no hardware is opened.
#![allow(clippy::expect_used)]
use davout::simulation::{SimulationBus, SimulationReceive, TxMatcher, TxOccurrence, TxRule};
use std::time::Instant;

use davout::{
    FaultClass, JointHomingState, MitJointCommand, OperationalMode, StopAction, Supervisor,
};
use marengo_config::MotorEntry;
use robstride::{
    BusError, CanFrame, FeedbackReport, MalformedReason, MotorAddress, MotorBus, ReceiveCompletion,
    ReceivedCanFrame, RxFrameKind, TimedCanFrame,
};

// The bounded receive contract permits at most 64 raw frames per poll. Unsupported and
// unconfigured traffic must consume that work quota before vendor decoding.
const FLOOD_FRAMES: usize = 257;

fn noise() -> CanFrame {
    CanFrame {
        id: 0x1f00_fefd,
        data: [0; 8],
        extended: true,
    }
}

fn supervisor() -> Supervisor<SimulationBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        davout::simulation::InitialVirtualReference::AllConfigured,
    )
    .expect("repo fixture")
}

fn pitch<B: MotorBus>(supervisor: &Supervisor<B>) -> MotorEntry {
    supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_shoulder_pitch")
        .expect("pitch fixture")
        .clone()
}

fn verify<B: MotorBus>(supervisor: &mut Supervisor<B>, motor: &MotorEntry) {
    assert_eq!(
        supervisor.joint_homing_state(&motor.joint),
        davout::JointHomingState::Verified,
        "declared INITIAL virtual fixture"
    );
}

fn run_status(motor: &MotorEntry) -> CanFrame {
    CanFrame {
        id: (2 << 24) | (2 << 22) | (u32::from(motor.device_id) << 8) | 0xfd,
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
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

#[test]
fn finite_unknown_flood_cannot_be_fully_consumed_by_one_supervisor_poll() {
    let mut supervisor = supervisor();
    supervisor
        .bus_mut()
        .queue_frames(std::iter::repeat_with(noise).take(FLOOD_FRAMES))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback();

    // Observe the suffix through the public raw receive API rather than assuming how MemoryBus
    // stores pending data. A bounded implementation may move rx_queue into a private deque.
    let suffix = supervisor.bus_mut().drain_raw().frames;
    assert!(
        !suffix.is_empty(),
        "one poll consumed every unsupported frame instead of bounding raw work"
    );
}

#[test]
fn ignored_incomplete_drain_cannot_authorize_motion_or_reenable() {
    let mut supervisor = supervisor();
    let motor = pitch(&supervisor);
    verify(&mut supervisor, &motor);
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("enable");
    supervisor
        .bus_mut()
        .queue_frame(run_status(&motor))
        .expect("finite closed script");
    supervisor.drain_feedback().expect("current-session pose");
    supervisor
        .bus_mut()
        .queue_frames(std::iter::repeat_with(noise).take(FLOOD_FRAMES))
        .expect("finite closed script");
    let _ = supervisor.drain_feedback(); // Deliberately ignore the receive error.
    supervisor.bus_mut().clear_trace();
    let motion = supervisor.send_mit_batch(vec![command(&motor)]);
    let reenable = supervisor.enable_targets(std::slice::from_ref(&motor.joint));
    assert!(
        motion.is_err() && reenable.is_err(),
        "incomplete safety view admitted motion={motion:?}, reenable={reenable:?}"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn incomplete_pre_enable_flush_refuses_activation() {
    let mut supervisor = supervisor();
    let motor = pitch(&supervisor);
    verify(&mut supervisor, &motor);
    supervisor
        .bus_mut()
        .queue_frames(std::iter::repeat_with(noise).take(FLOOD_FRAMES))
        .expect("finite closed script");
    assert!(
        supervisor
            .enable_targets(std::slice::from_ref(&motor.joint))
            .is_err(),
        "a prefix was treated as a complete pre-enable flush"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(supervisor.enable_session_started_at(), None);
    assert!(supervisor
        .bus()
        .frames()
        .iter()
        .all(|frame| frame.id >> 24 != 3));
}

#[test]
fn incomplete_post_enable_flush_rolls_back_before_session_marker() {
    let mut supervisor = supervisor();
    let motor = pitch(&supervisor);
    verify(&mut supervisor, &motor);
    let rule = supervisor
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                ..TxMatcher::default()
            },
            occurrence: TxOccurrence::Nth(1),
            receive: std::iter::repeat_with(|| SimulationReceive::from(noise()))
                .take(FLOOD_FRAMES)
                .collect(),
            send_error: None,
        })
        .expect("enable-triggered finite flood");
    assert!(
        supervisor
            .enable_targets(std::slice::from_ref(&motor.joint))
            .is_err(),
        "enable-write flood was treated as a complete final flush"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(supervisor.enable_session_started_at(), None);
    assert_eq!(supervisor.bus().rule_trigger_count(rule), 1);
    for configured in &supervisor.motors.motors.clone() {
        assert!(supervisor.bus().frames().iter().any(|frame| {
            frame.id >> 24 == 4
                && frame.id & 0xff == u32::from(configured.device_id)
                && frame.data[0] == 0
        }));
    }
}

fn data(id: u32, payload: &[u8]) -> ReceivedCanFrame {
    ReceivedCanFrame::new_data(Some("can0".into()), id, true, payload).expect("classic CAN fixture")
}

fn assert_all_stop_attempts<B: MotorBus>(supervisor: &Supervisor<B>) {
    let snapshot = supervisor.safety_snapshot();
    let stop = snapshot.last_stop.expect("receive hazard attempts a stop");
    assert_eq!(stop.attempts.len(), supervisor.motors.motors.len() * 3);
    for motor in &supervisor.motors.motors {
        let address = MotorAddress::from(motor);
        for action in [
            StopAction::ZeroSpeed,
            StopAction::NeutralMit,
            StopAction::Disable,
        ] {
            assert_eq!(
                stop.attempts
                    .iter()
                    .filter(|attempt| attempt.address == address && attempt.action == action)
                    .count(),
                1
            );
        }
    }
}

#[test]
fn every_short_status_shape_latches_without_installing_pose() {
    let payload = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8];
    for id in [0x0280_01fd, 0x1880_01fd] {
        for length in 0..8 {
            let mut supervisor = supervisor();
            let motor = pitch(&supervisor);
            supervisor
                .bus_mut()
                .queue_received(data(id, &payload[..length]))
                .expect("finite closed script");
            assert!(supervisor.drain_feedback().is_err());
            assert!(supervisor.joint_feedback(&motor.joint).is_none());
            let snapshot = supervisor.safety_snapshot();
            let fault = snapshot.first_fault().expect("malformed status");
            assert_eq!(fault.class, FaultClass::Feedback);
            assert_eq!(fault.receive.frame_count, 1);
            let frame = fault.receive.first_frame.as_ref().expect("raw envelope");
            assert_eq!(frame.can_id, id);
            assert_eq!(frame.payload_len, length as u8);
            assert_eq!(frame.kind, RxFrameKind::Data);
            assert_eq!(
                frame.reason,
                Some(MalformedReason::PayloadLength {
                    received: length as u8
                })
            );
            assert_eq!(&frame.raw[..length], &payload[..length]);
            assert_all_stop_attempts(&supervisor);
            assert_eq!(
                supervisor.joint_homing_state(&motor.joint),
                JointHomingState::Faulted
            );
            supervisor.bus_mut().clear_trace();
            assert!(supervisor
                .enable_targets(std::slice::from_ref(&motor.joint))
                .is_err());
            assert!(supervisor.bus().frames().is_empty());
        }
    }
}

#[test]
fn exact_eight_byte_status_data_remains_valid_for_both_status_types() {
    for id in [0x0280_01fd, 0x1880_01fd] {
        let mut supervisor = supervisor();
        let motor = pitch(&supervisor);
        supervisor
            .bus_mut()
            .queue_received(data(id, &[0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8]))
            .expect("finite closed script");
        assert_eq!(supervisor.drain_feedback().expect("complete status"), 1);
        assert!(supervisor.joint_feedback(&motor.joint).is_some());
        assert!(!supervisor.has_latched_fault());
    }
}

#[test]
fn remote_fault_like_identifier_cannot_supply_pose_or_device_header_proof() {
    let mut supervisor = supervisor();
    let motor = pitch(&supervisor);
    supervisor
        .bus_mut()
        .queue_received(
            ReceivedCanFrame::new_remote(Some("can0".into()), 0x02ff_01fd, true, 8)
                .expect("RTR fixture"),
        )
        .expect("finite closed script");
    assert!(supervisor.drain_feedback().is_err());
    assert!(supervisor.joint_feedback(&motor.joint).is_none());
    let snapshot = supervisor.safety_snapshot();
    assert!(snapshot
        .faults
        .iter()
        .all(|fault| fault.class != FaultClass::Device && fault.class != FaultClass::DriveState));
    let fault = snapshot.first_fault().expect("protocol mismatch");
    assert_eq!(fault.class, FaultClass::Feedback);
    assert_eq!(fault.device.status_flags, 0);
    assert_eq!(fault.device.drive_mode, None);
    let frame = fault.receive.first_frame.as_ref().expect("RTR envelope");
    assert_eq!(frame.payload_len, 0);
    assert_eq!(frame.kind, RxFrameKind::Remote { requested_len: 8 });
    assert_eq!(frame.reason, Some(MalformedReason::NonDataFrame));
    assert_all_stop_attempts(&supervisor);
}

#[test]
fn malformed_data_headers_partial_peer_fault_and_terminal_error_all_survive() {
    let mut supervisor = supervisor();
    let motor = pitch(&supervisor);
    verify(&mut supervisor, &motor);
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("enable");
    supervisor
        .bus_mut()
        .queue_attempts(
            [
                data(0x0291_01fd, &[0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff]),
                data(0x1500_02fd, &[0, 0, 0x80, 0x40, 2, 0x80]),
                data(0x0280_01fd, &[0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8]),
                data(0x0280_02fd, &[0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8]),
            ]
            .map(SimulationReceive::Received),
        )
        .expect("finite envelope prefix");
    supervisor
        .bus_mut()
        .queue_attempts([SimulationReceive::Error(
            "terminal receive failure after prefix".into(),
        )])
        .expect("terminal error");
    assert!(supervisor.drain_feedback().is_err());
    let snapshot = supervisor.safety_snapshot();
    let first = snapshot.first_fault().expect("first header cause");
    assert_eq!(first.class, FaultClass::Device);
    assert_eq!(first.device.status_flags, 0x11);
    let peer = snapshot
        .faults
        .iter()
        .find(|fault| {
            fault.class == FaultClass::Device
                && fault
                    .address
                    .as_ref()
                    .is_some_and(|address| address.device_id == 2)
        })
        .expect("partial peer fault");
    assert_eq!(peer.device.detailed_fault_bytes, [0; 4]);
    assert_eq!(peer.device.warning_bytes, [0; 4]);
    assert_eq!(peer.device.partial_fault_bytes, [0, 0, 0x80, 0x40]);
    assert_eq!(peer.device.partial_fault_available, 0b1111);
    assert_eq!(peer.device.partial_warning_bytes, [2, 0x80, 0, 0]);
    assert_eq!(peer.device.partial_warning_available, 0b0011);
    let transport = snapshot
        .faults
        .iter()
        .find(|fault| fault.class == FaultClass::Transport)
        .expect("terminal error");
    assert!(transport
        .message
        .contains("terminal receive failure after prefix"));
    assert_eq!(
        transport
            .receive
            .first_incomplete
            .as_ref()
            .expect("failed completion")
            .completion,
        ReceiveCompletion::Failed
    );
    assert_all_stop_attempts(&supervisor);
    supervisor
        .bus_mut()
        .queue_received(data(
            0x0280_01fd,
            &[0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        ))
        .expect("finite closed script");
    supervisor.drain_feedback().expect("healthy diagnostics");
    assert_eq!(
        supervisor.safety_snapshot().first_fault(),
        snapshot.first_fault()
    );
    supervisor.bus_mut().clear_trace();
    assert!(supervisor.send_mit_batch(vec![command(&motor)]).is_err());
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn partial_warning_is_a_protocol_fault_without_becoming_a_complete_device_fault() {
    let mut supervisor = supervisor();
    supervisor
        .bus_mut()
        .queue_received(data(0x1500_01fd, &[0, 0, 0, 0, 0x80]))
        .expect("finite closed script");
    assert!(supervisor.drain_feedback().is_err());
    let snapshot = supervisor.safety_snapshot();
    assert!(snapshot.observed_warnings.is_empty());
    assert!(snapshot
        .faults
        .iter()
        .all(|fault| fault.class != FaultClass::Device));
    let fault = snapshot.first_fault().expect("protocol fault");
    assert_eq!(fault.device.warning_bytes, [0; 4]);
    assert_eq!(fault.device.partial_warning_bytes, [0x80, 0, 0, 0]);
    assert_eq!(fault.device.partial_warning_available, 1);
    assert_eq!(
        fault
            .receive
            .first_frame
            .as_ref()
            .expect("short envelope")
            .payload_len,
        5
    );
}

#[test]
fn kernel_error_envelope_is_transport_evidence_without_vendor_fault_decoding() {
    let mut supervisor = supervisor();
    let motor = pitch(&supervisor);
    let mut error_frame = data(0x02ff_01fd, &[1, 2, 4, 8, 16, 32, 64, 128]);
    error_frame.kind = RxFrameKind::Error;
    supervisor
        .bus_mut()
        .queue_received(error_frame)
        .expect("finite closed script");
    assert!(supervisor.drain_feedback().is_err());
    assert!(supervisor.joint_feedback(&motor.joint).is_none());
    let snapshot = supervisor.safety_snapshot();
    let fault = snapshot.first_fault().expect("transport evidence");
    assert_eq!(fault.class, FaultClass::Transport);
    assert_eq!(fault.address, None);
    assert_eq!(fault.joint, None);
    assert_eq!(fault.device.status_flags, 0);
    let frame = fault
        .receive
        .first_frame
        .as_ref()
        .expect("raw Error envelope");
    assert_eq!(frame.kind, RxFrameKind::Error);
    assert_eq!(frame.payload_len, 8);
    assert_eq!(frame.raw, [1, 2, 4, 8, 16, 32, 64, 128]);
    assert_all_stop_attempts(&supervisor);
}

#[test]
fn every_incomplete_completion_blocks_even_without_a_terminal_error() {
    for completion in [
        ReceiveCompletion::WorkLimit,
        ReceiveCompletion::Deadline,
        ReceiveCompletion::Failed,
    ] {
        let mut supervisor = supervisor();
        let motor = pitch(&supervisor);
        verify(&mut supervisor, &motor);
        supervisor
            .enable_targets(std::slice::from_ref(&motor.joint))
            .expect("enable");
        supervisor
            .bus_mut()
            .queue_frame(run_status(&motor))
            .expect("finite closed script");
        supervisor.drain_feedback().expect("current pose");
        supervisor
            .bus_mut()
            .queue_feedback_report(FeedbackReport {
                completion,
                raw_frames: 64,
                read_attempts: 256,
                ..FeedbackReport::default()
            })
            .expect("bounded incomplete consumer report");
        let _ = supervisor.drain_feedback();
        let snapshot = supervisor.safety_snapshot();
        let fault = snapshot.first_fault().expect("incomplete authority");
        assert_eq!(fault.class, FaultClass::Transport);
        let incomplete = fault
            .receive
            .first_incomplete
            .as_ref()
            .expect("completion evidence");
        assert_eq!(incomplete.completion, completion);
        assert_eq!(incomplete.raw_frames, 64);
        assert_eq!(incomplete.read_attempts, 256);
        assert_all_stop_attempts(&supervisor);
        supervisor
            .bus_mut()
            .queue_frame(run_status(&motor))
            .expect("finite closed script");
        supervisor
            .drain_feedback()
            .expect("healthy complete diagnostics");
        assert_eq!(
            supervisor.safety_snapshot().first_fault(),
            snapshot.first_fault()
        );
        supervisor.bus_mut().clear_trace();
        assert!(supervisor.send_mit_batch(vec![command(&motor)]).is_err());
        assert!(supervisor
            .enable_targets(std::slice::from_ref(&motor.joint))
            .is_err());
        assert!(supervisor.bus().frames().is_empty());
    }
}

#[test]
fn new_typed_receive_errors_cannot_fall_through_runtime_authority() {
    for malformed in [false, true] {
        let mut supervisor = supervisor();
        let motor = pitch(&supervisor);
        let error = if malformed {
            BusError::MalformedFeedback {
                address: MotorAddress::from(&motor),
                reason: MalformedReason::NonDataFrame,
            }
        } else {
            BusError::ReceiveIncomplete {
                completion: ReceiveCompletion::WorkLimit,
            }
        };
        supervisor
            .bus_mut()
            .queue_feedback_report(FeedbackReport {
                terminal_error: Some(error),
                terminal_error_order: Some(0),
                ..FeedbackReport::default()
            })
            .expect("typed consumer error");
        assert!(supervisor.drain_feedback().is_err());
        let snapshot = supervisor.safety_snapshot();
        assert_eq!(
            snapshot.first_fault().expect("typed RX latch").class,
            if malformed {
                FaultClass::Feedback
            } else {
                FaultClass::Transport
            }
        );
        assert_all_stop_attempts(&supervisor);
    }
}

#[test]
fn unknown_flood_retains_work_limit_stats_and_attempts_every_stop() {
    let mut supervisor = supervisor();
    supervisor
        .bus_mut()
        .queue_frames(std::iter::repeat_with(noise).take(FLOOD_FRAMES))
        .expect("finite closed script");
    assert!(supervisor.drain_feedback().is_err());
    let snapshot = supervisor.safety_snapshot();
    let incomplete = snapshot
        .first_fault()
        .expect("raw capacity hazard")
        .receive
        .first_incomplete
        .as_ref()
        .expect("bounded report");
    assert_eq!(incomplete.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(incomplete.raw_frames, 64);
    assert_eq!(incomplete.read_attempts, 64);
    assert_all_stop_attempts(&supervisor);
    for motor in supervisor.motors.motors.clone() {
        assert!(supervisor
            .bus()
            .frames()
            .iter()
            .any(|frame| frame.id >> 24 == 4
                && frame.id & 0xff == u32::from(motor.device_id)
                && frame.data[0] == 0));
    }
    let suffix = supervisor.bus_mut().drain_raw().frames;
    assert_eq!(
        suffix.len(),
        64,
        "stop must not synchronously drain the unread backlog"
    );
}

#[test]
fn ordinary_empty_drain_and_timeout_remain_benign() {
    let mut supervisor = supervisor();
    // Startup may configure diagnostic Active Reporting; it is not a stop attempt.
    supervisor.bus_mut().clear_trace();
    assert_eq!(supervisor.drain_feedback().expect("empty drain"), 0);
    supervisor
        .bus_mut()
        .queue_feedback_report(FeedbackReport {
            terminal_error: Some(BusError::RecvTimeout),
            ..FeedbackReport::default()
        })
        .expect("benign complete timeout report");
    assert_eq!(
        supervisor
            .drain_feedback()
            .expect("complete timeout diagnostics"),
        0
    );
    let snapshot = supervisor.safety_snapshot();
    assert!(!snapshot.is_latched());
    assert_eq!(snapshot.stop_generation, 0);
    assert!(snapshot.last_stop.is_none());
    assert!(supervisor.bus().frames().is_empty());
}

fn assert_interleaved_first_cause(transport_first: bool) {
    let mut supervisor = supervisor();
    let mut error_frame = data(0x02ff_01fd, &[1, 2, 4, 8, 16, 32, 64, 128]);
    error_frame.kind = RxFrameKind::Error;
    let fault_frame = data(0x1500_01fd, &[1, 0, 0, 0, 0, 0, 0, 0]);
    let equal_time = Instant::now();
    supervisor
        .bus_mut()
        .queue_attempts(
            (if transport_first {
                [error_frame, fault_frame]
            } else {
                [fault_frame, error_frame]
            })
            .map(|received| {
                SimulationReceive::Timed(TimedCanFrame {
                    received_at: equal_time,
                    received,
                })
            }),
        )
        .expect("equal-time raw order");
    assert!(supervisor.drain_feedback().is_err());
    let snapshot = supervisor.safety_snapshot();
    assert_eq!(
        snapshot.first_fault().expect("initiating cause").class,
        if transport_first {
            FaultClass::Transport
        } else {
            FaultClass::Device
        }
    );
    assert!(snapshot
        .faults
        .iter()
        .any(|fault| fault.class == FaultClass::Device));
    assert!(snapshot
        .faults
        .iter()
        .any(|fault| fault.class == FaultClass::Transport));
    assert_all_stop_attempts(&supervisor);
}

#[test]
fn raw_error_before_vendor_fault_preserves_transport_as_first_cause_on_timestamp_tie() {
    assert_interleaved_first_cause(true);
}

#[test]
fn vendor_fault_before_raw_error_preserves_device_as_first_cause_on_timestamp_tie() {
    assert_interleaved_first_cause(false);
}

#[test]
fn backend_error_before_peer_fault_preserves_first_transport_cause_and_peer_evidence() {
    let mut supervisor = supervisor();
    supervisor
        .bus_mut()
        .set_source_count(2)
        .expect("two distinct local queues");
    supervisor
        .bus_mut()
        .queue_to_source(
            0,
            [SimulationReceive::Error(
                "first source receive failed".into(),
            )],
        )
        .expect("source zero failure");
    supervisor
        .bus_mut()
        .queue_to_source(
            1,
            [SimulationReceive::Received(data(
                0x1500_02fd,
                &[0, 0, 0x80, 0x40, 0, 0, 0, 0],
            ))],
        )
        .expect("independent peer queue");
    assert!(supervisor.drain_feedback().is_err());
    let snapshot = supervisor.safety_snapshot();
    let first = snapshot.first_fault().expect("first backend cause");
    assert_eq!(first.class, FaultClass::Transport);
    assert!(first.message.contains("first source receive failed"));
    let peer = snapshot
        .faults
        .iter()
        .find(|fault| fault.class == FaultClass::Device)
        .expect("peer fault retained");
    assert_eq!(peer.address.as_ref().expect("peer address").device_id, 2);
    assert_eq!(peer.device.detailed_fault_bytes, [0, 0, 0x80, 0x40]);
    assert_all_stop_attempts(&supervisor);
}
