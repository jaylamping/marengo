#![allow(clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use robstride::{
    BusError, CanBus, CanFrame, DriveMode, FeedbackEvent, MemoryBus, MotorAddress, MotorBus,
    MotorState, ReceivedCanFrame, TimedCanFrame,
};

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];

fn frame(id: u32, data: [u8; 8]) -> CanFrame {
    CanFrame {
        id,
        data,
        extended: true,
    }
}

fn types() -> HashMap<MotorAddress, MotorType> {
    HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)])
}

#[test]
fn literal_status_flags_and_all_drive_modes_remain_separate() {
    // Each literal exercises one documented CAN-ID flag, then all four modes.
    for (id, flags, mode) in [
        (0x0201_01fd, 1, DriveMode::Reset),
        (0x0202_01fd, 2, DriveMode::Reset),
        (0x0204_01fd, 4, DriveMode::Reset),
        (0x0208_01fd, 8, DriveMode::Reset),
        (0x0210_01fd, 16, DriveMode::Reset),
        (0x0220_01fd, 32, DriveMode::Reset),
        (0x0200_01fd, 0, DriveMode::Reset),
        (0x0240_01fd, 0, DriveMode::Calibration),
        (0x0280_01fd, 0, DriveMode::Run),
        (0x02c0_01fd, 0, DriveMode::Reserved),
        (0x18ff_01fd, 63, DriveMode::Reserved),
    ] {
        let mut bus = MemoryBus::default();
        bus.rx_queue.push(frame(id, POSE));
        let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
        assert!(report.terminal_error.is_none());
        assert_eq!(report.observations.len(), 1);
        let observed = &report.observations[0];
        assert_eq!(observed.address, MotorAddress::new("can0", 1));
        assert_eq!(observed.can_id, id);
        let FeedbackEvent::Status(status) = observed.event else {
            panic!("status observation required");
        };
        assert_eq!(status.status_flags, flags, "literal {id:#x}");
        assert_eq!(status.drive_mode, mode, "literal {id:#x}");
        assert_eq!(status.device_id, 1);
        assert_eq!(status.position_rad, 0.0);
        assert_eq!(status.velocity_rad_s, 0.0);
        assert_eq!(status.torque_nm, 0.0);
        assert_eq!(status.temperature_c, 20.0);
    }
}

#[test]
fn detailed_faults_and_warnings_preserve_full_asymmetric_raw_domains() {
    let mut bus = MemoryBus::default();
    let raw = [0x12, 0x34, 0x56, 0x80, 0x9a, 0xbc, 0xde, 0xf0];
    bus.rx_queue.push(frame(0x1500_01fd, raw));
    let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert!(report.terminal_error.is_none());
    assert_eq!(report.observations.len(), 1);
    let FeedbackEvent::DetailedFault(fault) = report.observations[0].event else {
        panic!("detailed-fault observation required");
    };
    assert_eq!(fault.raw, raw);
    assert_eq!(fault.fault_bytes(), [0x12, 0x34, 0x56, 0x80]);
    assert_eq!(fault.warning_bytes(), [0x9a, 0xbc, 0xde, 0xf0]);
    assert!(fault.has_fault());
    assert!(fault.has_warning());
    let mut projection = MotorState::default();
    report.observations[0].update_state(&mut projection);
    assert_eq!(
        projection.fault, 1,
        "indication does not invent a bit identity"
    );
    assert_eq!(projection.updated, None);
}

#[test]
fn every_raw_fault_and_status_survives_all_same_batch_orders() {
    let first = frame(0x1500_01fd, [1, 0, 0, 0, 0, 0, 0, 0]);
    let second = frame(0x1500_01fd, [0, 0, 0, 0x80, 0, 0, 0, 0]);
    let healthy = frame(0x0280_01fd, POSE);
    for sequence in [
        vec![first.clone(), healthy.clone()],
        vec![healthy.clone(), first.clone()],
        vec![first.clone(), second.clone(), healthy.clone()],
    ] {
        let mut bus = MemoryBus {
            rx_queue: sequence.clone(),
            ..MemoryBus::default()
        };
        let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
        assert!(report.terminal_error.is_none());
        assert_eq!(report.observations.len(), sequence.len());
        for (observed, raw) in report.observations.iter().zip(&sequence) {
            assert_eq!(observed.can_id, raw.id);
            match observed.event {
                FeedbackEvent::DetailedFault(fault) => assert_eq!(fault.raw, raw.data),
                FeedbackEvent::Status(status) => {
                    assert_eq!(raw.id, 0x0280_01fd);
                    assert_eq!(status.status_flags, 0);
                    assert_eq!(status.drive_mode, DriveMode::Run);
                }
            }
        }
    }
}

struct TimedBus {
    frames: Vec<TimedCanFrame>,
    error: Option<BusError>,
}

impl CanBus for TimedBus {
    fn send_frame(&mut self, _frame: &CanFrame) -> Result<(), BusError> {
        Ok(())
    }

    fn recv_timed_frames_from(&mut self, out: &mut Vec<TimedCanFrame>) -> Result<(), BusError> {
        out.append(&mut self.frames);
        match self.error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn recv_timed_frames_from_nonblocking(
        &mut self,
        out: &mut Vec<TimedCanFrame>,
    ) -> Result<(), BusError> {
        self.recv_timed_frames_from(out)
    }
}

impl MotorBus for TimedBus {}

fn timed(interface: Option<&str>, id: u32, data: [u8; 8], received_at: Instant) -> TimedCanFrame {
    TimedCanFrame {
        received_at,
        received: ReceivedCanFrame {
            interface: interface.map(str::to_string),
            frame: frame(id, data),
        },
    }
}

#[test]
fn separate_fault_rx_time_never_refreshes_the_last_pose() {
    let pose_at = Instant::now() - Duration::from_millis(10);
    let fault_at = Instant::now();
    let mut bus = TimedBus {
        frames: vec![
            timed(Some("can0"), 0x0280_01fd, POSE, pose_at),
            timed(
                Some("can0"),
                0x1500_01fd,
                [0, 0, 1, 0, 0, 0, 0, 0],
                fault_at,
            ),
        ],
        error: None,
    };
    let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert_eq!(report.observations.len(), 2);
    assert_eq!(report.observations[0].received_at, pose_at);
    assert_eq!(report.observations[1].received_at, fault_at);
    let mut projection = MotorState::default();
    for observation in report.observations {
        observation.update_state(&mut projection);
    }
    assert_eq!(projection.updated, Some(pose_at));
    assert_eq!(projection.fault, 1);
    assert!(projection.is_stale(Duration::from_millis(5)));
}

#[test]
fn warning_only_report_is_visible_without_becoming_pose_or_detailed_fault() {
    let mut bus = MemoryBus::default();
    bus.rx_queue
        .push(frame(0x1500_01fd, [0, 0, 0, 0, 1, 2, 4, 0x80]));
    let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert_eq!(report.observations.len(), 1);
    let FeedbackEvent::DetailedFault(warning) = report.observations[0].event else {
        panic!("warning evidence retained in detailed report");
    };
    assert!(!warning.has_fault());
    assert!(warning.has_warning());
    let mut projection = MotorState::default();
    report.observations[0].update_state(&mut projection);
    assert_eq!(projection.fault, 0);
    assert_eq!(projection.updated, None);
}

#[test]
fn report_retains_prefix_and_terminal_error_for_both_budget_modes() {
    for budget in [Duration::ZERO, Duration::from_millis(2)] {
        let received_at = Instant::now();
        let mut bus = TimedBus {
            frames: vec![timed(
                Some("can0"),
                0x1500_01fd,
                [1, 0, 0, 0, 0, 0, 0, 0],
                received_at,
            )],
            error: Some(BusError::Driver("fault prefix then RX failure".into())),
        };
        let report = bus.recv_feedback_report(&types(), budget, Duration::ZERO);
        assert_eq!(report.observations.len(), 1);
        assert_eq!(report.observations[0].received_at, received_at);
        assert!(matches!(report.terminal_error, Some(BusError::Driver(_))));
        let FeedbackEvent::DetailedFault(fault) = report.observations[0].event else {
            panic!("delivered fault survives receive error");
        };
        assert_eq!(fault.raw, [1, 0, 0, 0, 0, 0, 0, 0]);
    }
}

#[test]
fn same_device_id_on_two_interfaces_has_independent_ordered_evidence() {
    let received_at = Instant::now();
    let mut bus = TimedBus {
        frames: vec![
            timed(
                Some("can1"),
                0x1500_01fd,
                [0, 0, 0x40, 0, 0, 0, 0, 0],
                received_at,
            ),
            timed(Some("can0"), 0x0280_01fd, POSE, received_at),
            timed(None, 0x1500_01fd, [1, 0, 0, 0, 0, 0, 0, 0], received_at),
            timed(
                Some("unconfigured"),
                0x1500_01fd,
                [1, 0, 0, 0, 0, 0, 0, 0],
                received_at,
            ),
        ],
        error: None,
    };
    let types = HashMap::from([
        (MotorAddress::new("can0", 1), MotorType::Rs03),
        (MotorAddress::new("can1", 1), MotorType::Rs02),
    ]);
    let report = bus.recv_feedback_report(&types, Duration::ZERO, Duration::ZERO);
    assert_eq!(
        report.observations.len(),
        2,
        "ambiguous/unknown sources must not alias"
    );
    assert_eq!(report.observations[0].address, MotorAddress::new("can1", 1));
    assert_eq!(report.observations[1].address, MotorAddress::new("can0", 1));
    assert!(matches!(
        report.observations[0].event,
        FeedbackEvent::DetailedFault(_)
    ));
    assert!(matches!(
        report.observations[1].event,
        FeedbackEvent::Status(_)
    ));
}

#[test]
fn empty_nonblocking_report_differs_from_a_blocking_timeout() {
    let mut bus = MemoryBus::default();
    let empty = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert!(empty.observations.is_empty());
    assert!(empty.terminal_error.is_none());
    let timeout = bus.recv_feedback_report(&types(), Duration::from_millis(1), Duration::ZERO);
    assert!(timeout.observations.is_empty());
    assert!(matches!(
        timeout.terminal_error,
        Some(BusError::RecvTimeout)
    ));
}

#[test]
fn ordinary_disable_never_clears_firmware_faults() {
    let mut bus = MemoryBus::default();
    bus.disable_drive_at(&MotorAddress::new("can0", 1))
        .expect("ordinary disable");
    assert_eq!(bus.tx.len(), 1);
    assert_eq!(bus.tx[0].id, 0x0400_fd01);
    assert_eq!(bus.tx[0].data, [0; 8]);
}

#[cfg(all(feature = "socketcan", target_os = "linux"))]
#[test]
#[ignore = "requires virtual vcan0; no physical robot"]
fn socketcan_runtime_report_preserves_order_raw_evidence_and_receive_times() {
    use robstride::RuntimeBus;

    for budget in [Duration::ZERO, Duration::from_millis(100)] {
        let mut bus = RuntimeBus::socketcan("vcan0").expect("open virtual SocketCAN");
        let motor_types = HashMap::from([(MotorAddress::new("vcan0", 1), MotorType::Rs03)]);
        let sequence = [
            frame(0x1500_01fd, [0x12, 0x34, 0x56, 0x80, 0, 0, 0, 0]),
            frame(0x0280_01fd, POSE),
            frame(0x1500_01fd, [0, 0, 0, 0, 1, 2, 4, 0x80]),
        ];
        let started = Instant::now();
        for raw in &sequence {
            bus.send_frame(raw)
                .expect("inject literal virtual CAN frame");
        }
        let deadline = started + Duration::from_millis(100);
        let mut observations = Vec::new();
        while observations.len() < sequence.len() && Instant::now() < deadline {
            let report = bus.recv_feedback_report(&motor_types, budget, Duration::from_millis(2));
            assert!(report.terminal_error.is_none());
            observations.extend(report.observations);
            if observations.len() < sequence.len() {
                std::thread::sleep(Duration::from_micros(100));
            }
        }
        let finished = Instant::now();
        assert_eq!(observations.len(), 3, "budget {budget:?}");
        for (observed, expected) in observations.iter().zip(sequence) {
            assert_eq!(observed.address, MotorAddress::new("vcan0", 1));
            assert_eq!(observed.can_id, expected.id);
            assert!((started..=finished).contains(&observed.received_at));
            if let FeedbackEvent::DetailedFault(fault) = observed.event {
                assert_eq!(fault.raw, expected.data);
            }
        }
        assert!(observations
            .windows(2)
            .all(|pair| pair[0].received_at <= pair[1].received_at));
        assert!(matches!(observations[1].event, FeedbackEvent::Status(_)));
        let mut projection = MotorState::default();
        for observation in &observations {
            observation.update_state(&mut projection);
        }
        assert_eq!(projection.updated, Some(observations[1].received_at));
    }
}
