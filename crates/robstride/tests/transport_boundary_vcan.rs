//! Baseline-compatible contracts through the actual RuntimeBus/SocketCAN boundary.

#![cfg(all(feature = "socketcan", target_os = "linux"))]
#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::time::Duration;

use marengo_config::MotorType;
use robstride::{CanBus, FeedbackEvent, MotorAddress, MotorBus, RuntimeBus};
use socketcan::{CanFrame, CanSocket, EmbeddedFrame, ExtendedId, Socket};

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];

fn data(id: u32, bytes: &[u8]) -> CanFrame {
    CanFrame::new(ExtendedId::new(id).expect("literal extended ID"), bytes)
        .expect("classic CAN payload")
}

fn motor_types(interface: &str) -> HashMap<MotorAddress, MotorType> {
    HashMap::from([(MotorAddress::new(interface, 1), MotorType::Rs03)])
}

fn assert_only_marker_is_complete(bus: &mut RuntimeBus, context: &str) {
    let report = bus.recv_feedback_report(
        &motor_types("vcan0"),
        Duration::from_millis(50),
        Duration::from_millis(1),
    );
    assert!(report.terminal_error.is_none(), "{context}: receive failed");
    assert_eq!(
        report
            .observations
            .iter()
            .filter(|observation| matches!(observation.event, FeedbackEvent::Status(_)))
            .count(),
        1,
        "{context}: only the literal full-length marker may refresh pose"
    );
    assert!(
        report
            .observations
            .iter()
            .all(|observation| !matches!(observation.event, FeedbackEvent::DetailedFault(_))),
        "{context}: a partial payload is not a complete detailed report"
    );
}

#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
fn short_socketcan_data_never_becomes_complete_feedback() {
    // Type 2/24 status and type 21 detail IDs are literal vendor wire examples.
    // A separate full-length marker proves that the receiver is working.
    for id in [0x0280_01fd, 0x1880_01fd, 0x1500_01fd] {
        for length in 0..8 {
            let mut bus = RuntimeBus::socketcan("vcan0").expect("virtual receiver");
            let sender = CanSocket::open("vcan0").expect("virtual producer");
            sender
                .write_frame(&data(id, &POSE[..length]))
                .expect("short literal data frame");
            sender
                .write_frame(&data(0x0280_01fd, &POSE))
                .expect("full-length positive marker");
            assert_only_marker_is_complete(&mut bus, &format!("ID {id:#x}, DLC {length}"));
        }
    }
}

#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
fn remote_socketcan_requests_never_become_pose_or_device_faults() {
    for requested_length in [0, 8] {
        let mut bus = RuntimeBus::socketcan("vcan0").expect("virtual receiver");
        let sender = CanSocket::open("vcan0").expect("virtual producer");
        // The identifier resembles a Run status with all six device flags. It
        // is still only a remote request and carries no measured payload.
        let remote = CanFrame::new_remote(
            ExtendedId::new(0x02bf_01fd).expect("literal ID"),
            requested_length,
        )
        .expect("remote request");
        sender.write_frame(&remote).expect("remote injection");
        sender
            .write_frame(&data(0x0280_01fd, &POSE))
            .expect("full-length positive marker");
        assert_only_marker_is_complete(&mut bus, "remote frame");
        let complete =
            bus.recv_feedback_report(&motor_types("vcan0"), Duration::ZERO, Duration::ZERO);
        assert!(complete.observations.is_empty(), "marker consumed once");
    }
}

#[test]
#[ignore = "requires virtual vcan0/vcan1; no physical robot; run tests serially"]
fn routed_socketcan_visits_queued_peer_fault_before_hot_port_backlog() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut motors = marengo_config::load_motors_config(root).expect("fixture motor entries");
    motors.motors.truncate(2);
    for (motor, interface) in motors.motors.iter_mut().zip(["vcan0", "vcan1"]) {
        motor.can_interface = interface.into();
        motor.device_id = 1;
    }
    let mut bus = RuntimeBus::socketcan_from_motors(&motors).expect("virtual routed receiver");
    let mut types = motor_types("vcan0");
    types.extend(motor_types("vcan1"));
    for interface in ["vcan0", "vcan1"] {
        let sender = CanSocket::open(interface).expect("virtual producer");
        sender
            .write_frame(&data(0x1500_01fd, &[1, 0, 0, 0, 0, 0, 0, 0]))
            .expect("queued peer fault");
        for _ in 0..40 {
            sender
                .write_frame(&data(0x0280_01fd, &POSE))
                .expect("hot-port backlog");
        }
    }
    let report =
        bus.recv_feedback_report(&types, Duration::from_millis(50), Duration::from_millis(1));
    assert!(report.terminal_error.is_none());
    assert!(report.observations.len() >= 2, "both peer faults observed");
    let first = &report.observations[..2];
    assert!(first
        .iter()
        .all(|observation| matches!(observation.event, FeedbackEvent::DetailedFault(_))));
    assert_ne!(first[0].address.interface, first[1].address.interface);
}

#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
#[allow(clippy::panic)]
fn runtime_socketcan_preserves_malformed_evidence_and_remote_requested_length() {
    // Candidate conformance strengthens the unchanged baseline regressions:
    // rejecting fabricated pose must still retain the actual malformed evidence.
    for (id, length, remote, status_header) in [
        (0x02bf_01fd, 3, false, true),
        (0x18bf_01fd, 7, false, true),
        (0x1500_01fd, 5, false, false),
        (0x02bf_01fd, 0, true, true),
        (0x02bf_01fd, 8, true, true),
    ] {
        let mut bus = RuntimeBus::socketcan("vcan0").expect("virtual receiver");
        let sender = CanSocket::open("vcan0").expect("virtual producer");
        let frame = if remote {
            CanFrame::new_remote(ExtendedId::new(id).expect("literal ID"), length)
                .expect("remote request")
        } else {
            data(id, &POSE[..length])
        };
        sender.write_frame(&frame).expect("malformed injection");
        sender
            .write_frame(&data(0x0280_01fd, &POSE))
            .expect("full-length positive marker");
        let report = bus.recv_feedback_report(
            &motor_types("vcan0"),
            Duration::from_millis(50),
            Duration::from_millis(1),
        );
        assert!(report.terminal_error.is_none());
        assert!(matches!(
            report.completion,
            robstride::ReceiveCompletion::Idle | robstride::ReceiveCompletion::Quiet
        ));
        assert_eq!(
            report.observations.len(),
            2,
            "malformed evidence and marker"
        );
        let observed = &report.observations[0];
        assert_eq!(observed.order, 0);
        assert_eq!(observed.address, MotorAddress::new("vcan0", 1));
        assert_eq!(observed.can_id, id);
        let FeedbackEvent::Malformed(malformed) = observed.event else {
            panic!("actual short/RTR evidence must remain malformed");
        };
        let payload_len = if remote { 0 } else { length };
        assert_eq!(malformed.payload_len, payload_len as u8);
        assert_eq!(&malformed.raw[..payload_len], &POSE[..payload_len]);
        assert_eq!(
            malformed.kind,
            if remote {
                robstride::RxFrameKind::Remote {
                    requested_len: length as u8,
                }
            } else {
                robstride::RxFrameKind::Data
            }
        );
        assert_eq!(
            malformed.reason,
            if remote {
                robstride::MalformedReason::NonDataFrame
            } else {
                robstride::MalformedReason::PayloadLength {
                    received: length as u8,
                }
            }
        );
        assert_eq!(
            malformed.status_flags,
            (status_header && !remote).then_some(0x3f)
        );
        assert_eq!(
            malformed.drive_mode,
            (status_header && !remote).then_some(robstride::DriveMode::Run)
        );
        assert_eq!(report.observations[1].order, 1);
        assert!(matches!(
            report.observations[1].event,
            FeedbackEvent::Status(_)
        ));
    }
}
#[test]
#[ignore = "requires virtual vcan0; no physical robot; run tests serially"]
fn socketcan_own_message_echo_is_observable() {
    let mut bus = RuntimeBus::socketcan("vcan0").expect("virtual receiver");
    let (id, data) = robstride::encode_default_enable(1);
    bus.send_frame(&robstride::CanFrame {
        id,
        data,
        extended: true,
    })
    .expect("write on receiver socket");
    let report = bus.recv_feedback_report(
        &motor_types("vcan0"),
        Duration::from_millis(50),
        Duration::from_millis(1),
    );
    assert!(report.terminal_error.is_none());
    assert_eq!(report.host_echoes.len(), 1);
    assert_eq!(
        report.host_echoes[0].command,
        robstride::EchoedCommand::Enable
    );
}

#[test]
fn socketcan_router_refuses_duplicate_addresses_before_opening_interfaces() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut motors = marengo_config::load_motors_config(root).expect("fixture motor entries");
    let duplicate = motors.motors[0].clone();
    motors.motors.push(duplicate);
    assert!(matches!(
        RuntimeBus::socketcan_from_motors(&motors),
        Err(robstride::BusError::DuplicateMotorAddress { .. })
    ));
}
