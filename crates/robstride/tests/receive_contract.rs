#![allow(clippy::expect_used, clippy::panic)]

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use robstride::{
    BusError, CanBus, CanFrame, DriveMode, FeedbackEvent, MalformedReason, MemoryBus, MotorAddress,
    MotorBus, MotorState, ReceiveAttempt, ReceiveCompletion, ReceiveLimits, ReceivedCanFrame,
    RxFrameKind, TimedCanFrame,
};

const POSE: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8];

#[derive(Debug)]
enum Step {
    Frame(ReceivedCanFrame),
    Idle,
    Interrupted,
    Error,
}

struct ScriptedBus {
    steps: VecDeque<Step>,
    stamp: Instant,
    calls: usize,
    sources: usize,
}

impl ScriptedBus {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            stamp: Instant::now(),
            calls: 0,
            sources: 1,
        }
    }
}

impl CanBus for ScriptedBus {
    fn send_frame(&mut self, _: &CanFrame) -> Result<(), BusError> {
        Ok(())
    }

    fn receive_source_count(&self) -> usize {
        self.sources
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        self.calls += 1;
        match self.steps.pop_front().unwrap_or(Step::Idle) {
            Step::Frame(received) => Ok(ReceiveAttempt::Frame(TimedCanFrame {
                received_at: self.stamp,
                received,
            })),
            Step::Idle => Ok(ReceiveAttempt::Idle),
            Step::Interrupted => Ok(ReceiveAttempt::Interrupted),
            Step::Error => Err(BusError::Driver("injected one-read failure".into())),
        }
    }
}

impl MotorBus for ScriptedBus {}

fn types() -> HashMap<MotorAddress, MotorType> {
    HashMap::from([(MotorAddress::new("can0", 1), MotorType::Rs03)])
}

fn data(id: u32, payload: &[u8]) -> ReceivedCanFrame {
    ReceivedCanFrame::new_data(Some("can0".into()), id, true, payload).expect("valid fixture")
}

#[test]
fn every_short_status_and_fault_payload_remains_partial_evidence() {
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
    for id in [0x02a5_01fd, 0x18a5_01fd, 0x1500_01fd] {
        for length in 0..8 {
            let mut bus = ScriptedBus::new([Step::Frame(data(id, &bytes[..length]))]);
            let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
            assert_eq!(report.completion, ReceiveCompletion::Idle);
            assert_eq!(report.observations.len(), 1);
            let observed = &report.observations[0];
            assert_eq!(observed.can_id, id);
            assert_eq!(observed.address, MotorAddress::new("can0", 1));
            let FeedbackEvent::Malformed(partial) = observed.event else {
                panic!("partial data cannot become a pose or a complete fault word");
            };
            assert_eq!(
                partial.reason,
                MalformedReason::PayloadLength {
                    received: length as u8
                }
            );
            assert_eq!(partial.payload_len, length as u8);
            assert_eq!(partial.kind, RxFrameKind::Data);
            assert_eq!(&partial.raw[..length], &bytes[..length]);
            assert!(partial.raw[length..].iter().all(|byte| *byte == 0));
            let status_header = (id != 0x1500_01fd).then_some((37, DriveMode::Run));
            assert_eq!(partial.status_flags, status_header.map(|header| header.0));
            assert_eq!(partial.drive_mode, status_header.map(|header| header.1));
            let mut projection = MotorState::default();
            observed.update_state(&mut projection);
            assert_eq!(projection.updated, None);
            assert_eq!(projection.fault, 0);
        }
    }
}

#[test]
fn remote_requests_never_supply_vendor_status_or_fault_header_proof() {
    for id in [0x02ff_01fd, 0x18ff_01fd, 0x1500_01fd] {
        for requested_len in 0..=8 {
            let remote = ReceivedCanFrame::new_remote(Some("can0".into()), id, true, requested_len)
                .expect("classic request");
            assert_eq!(remote.payload(), Some([].as_slice()));
            let mut bus = ScriptedBus::new([Step::Frame(remote)]);
            let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
            let FeedbackEvent::Malformed(observed) = report.observations[0].event else {
                panic!("remote cannot become status or detailed faults");
            };
            assert_eq!(observed.reason, MalformedReason::NonDataFrame);
            assert_eq!(observed.kind, RxFrameKind::Remote { requested_len });
            assert_eq!(observed.payload_len, 0);
            assert_eq!(observed.status_flags, None);
            assert_eq!(observed.drive_mode, None);
        }
    }
}

#[test]
fn invalid_public_envelope_metadata_is_rejected_without_slicing_or_pose() {
    for (payload_len, kind) in [
        (9, RxFrameKind::Data),
        (1, RxFrameKind::Remote { requested_len: 8 }),
        (0, RxFrameKind::Remote { requested_len: 9 }),
    ] {
        let mut invalid = data(0x0280_01fd, &POSE);
        invalid.payload_len = payload_len;
        invalid.kind = kind;
        assert_eq!(invalid.payload(), None);
        let mut bus = ScriptedBus::new([Step::Frame(invalid)]);
        let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
        assert!(
            matches!(report.observations[0].event, FeedbackEvent::Malformed(partial)
            if partial.reason == MalformedReason::InvalidEnvelope)
        );
    }
    assert!(ReceivedCanFrame::new_data(None, 1, true, &[0; 9]).is_err());
    assert!(ReceivedCanFrame::new_remote(None, 1, true, 9).is_err());
}

#[test]
fn raw_order_connects_transport_vendor_and_ignored_frames_with_equal_timestamps() {
    let mut error = data(0x1000_0040, &[1, 2, 3, 4, 5, 6, 7, 8]);
    error.kind = RxFrameKind::Error;
    error.frame.extended = false;
    let mut bus = ScriptedBus::new([
        Step::Frame(error.clone()),
        Step::Frame(data(0x1f00_01fd, &[0; 8])),
        Step::Frame(data(0x1500_01fd, &[0, 0, 0, 0x80, 0, 0, 0, 0])),
        Step::Frame(data(0x0280_01fd, &POSE)),
    ]);
    let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert_eq!(report.raw_frames, 4);
    assert_eq!(
        report
            .observations
            .iter()
            .map(|event| event.order)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(report.transport_frames.len(), 1);
    assert_eq!(report.transport_frames[0].order, 0);
    assert_eq!(report.transport_frames[0].frame.received, error);
    assert_eq!(
        report.transport_frames[0].frame.received_at,
        report.observations[0].received_at
    );
    assert!(
        report.terminal_error.is_none(),
        "typed error evidence is not duplicated as a read error"
    );
    assert_eq!(report.terminal_error_order, None);
}

#[test]
fn first_read_error_order_is_retained_before_later_peer_evidence() {
    for prefix in [false, true] {
        let mut steps = Vec::new();
        if prefix {
            steps.push(Step::Frame(data(0x0280_01fd, &POSE)));
        }
        steps.push(Step::Error);
        steps.push(Step::Frame(data(0x1500_01fd, &[1, 0, 0, 0, 0, 0, 0, 0])));
        let mut bus = ScriptedBus::new(steps);
        bus.sources = if prefix { 3 } else { 2 };
        let report = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
        assert_eq!(report.completion, ReceiveCompletion::Failed);
        assert!(matches!(report.terminal_error, Some(BusError::Driver(_))));
        assert_eq!(report.terminal_error_order, Some(usize::from(prefix)));
        assert_eq!(
            report.observations.last().expect("peer evidence").order,
            usize::from(prefix)
        );
        assert_eq!(report.raw_frames, 1 + usize::from(prefix));
        assert_eq!(report.read_attempts, bus.sources);
    }
}

#[test]
fn interruptions_spend_tokens_without_proving_idle_or_consuming_the_next_frame() {
    let mut steps = (0..256).map(|_| Step::Interrupted).collect::<Vec<_>>();
    steps.push(Step::Frame(data(0x0280_01fd, &POSE)));
    let mut bus = ScriptedBus::new(steps);
    let first = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert_eq!(first.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(first.read_attempts, 256);
    assert_eq!(bus.calls, 256);
    assert_eq!(first.raw_frames, 0);
    assert!(first.observations.is_empty());
    let second = bus.recv_feedback_report(&types(), Duration::ZERO, Duration::ZERO);
    assert_eq!(second.observations.len(), 1);
    assert_eq!(second.completion, ReceiveCompletion::Idle);
}

#[test]
fn narrower_limits_and_partial_source_rounds_never_claim_quiescence() {
    let mut bus = ScriptedBus::new([Step::Idle, Step::Frame(data(0x1500_01fd, &[1; 8]))]);
    bus.sources = 2;
    let report = bus.recv_raw_report(
        Duration::ZERO,
        Duration::ZERO,
        ReceiveLimits {
            max_attempts: 1,
            ..ReceiveLimits::default()
        },
    );
    assert_eq!(report.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(report.read_attempts, 1);
    assert!(!report.completion.is_complete());
    assert_eq!(bus.steps.len(), 1, "no lookahead beyond the quota");

    let mut bus = MemoryBus::default();
    bus.rx_queue.extend((0..65).map(|_| CanFrame {
        id: 0x0280_01fd,
        data: POSE,
        extended: true,
    }));
    let first = bus.recv_raw_report(
        Duration::ZERO,
        Duration::ZERO,
        ReceiveLimits {
            max_frames: 2,
            ..ReceiveLimits::default()
        },
    );
    assert_eq!(first.frames.len(), 2);
    assert_eq!(first.completion, ReceiveCompletion::WorkLimit);
    let second = bus.recv_raw_report(
        Duration::ZERO,
        Duration::ZERO,
        ReceiveLimits {
            max_frames: usize::MAX,
            max_attempts: usize::MAX,
            deadline: None,
        },
    );
    assert_eq!(second.frames.len(), 63);
    assert_eq!(second.completion, ReceiveCompletion::Idle);

    let mut oversized = MemoryBus::default();
    oversized.rx_queue.extend((0..65).map(|_| CanFrame {
        id: 0x1f00_01fd,
        data: [0; 8],
        extended: true,
    }));
    let report = oversized.recv_raw_report(
        Duration::ZERO,
        Duration::ZERO,
        ReceiveLimits {
            max_frames: usize::MAX,
            max_attempts: usize::MAX,
            deadline: None,
        },
    );
    assert_eq!(
        report.frames.len(),
        64,
        "callers cannot increase the fixed raw frame quota"
    );
    assert_eq!(report.read_attempts, 64);
    assert_eq!(report.completion, ReceiveCompletion::WorkLimit);

    for limits in [
        ReceiveLimits {
            max_frames: 0,
            ..ReceiveLimits::default()
        },
        ReceiveLimits {
            max_attempts: 0,
            ..ReceiveLimits::default()
        },
    ] {
        let mut bus = ScriptedBus::new([Step::Frame(data(0x0280_01fd, &POSE))]);
        let report = bus.recv_raw_report(Duration::ZERO, Duration::ZERO, limits);
        assert_eq!(report.completion, ReceiveCompletion::WorkLimit);
        assert_eq!(bus.calls, 0, "zero capacity is not unlimited");
    }
}

#[test]
fn quota_exhaustion_retains_both_prefix_and_read_failure_without_visiting_more_sources() {
    let mut bus = ScriptedBus::new([
        Step::Frame(data(0x0280_01fd, &POSE)),
        Step::Error,
        Step::Frame(data(0x1500_01fd, &[1; 8])),
    ]);
    bus.sources = 3;
    let report = bus.recv_feedback_report_with_limits(
        &types(),
        Duration::ZERO,
        Duration::ZERO,
        ReceiveLimits {
            max_attempts: 2,
            ..ReceiveLimits::default()
        },
    );
    assert_eq!(report.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(report.observations.len(), 1);
    assert_eq!(report.terminal_error_order, Some(1));
    assert!(matches!(report.terminal_error, Some(BusError::Driver(_))));
    assert_eq!(bus.calls, 2);
    assert_eq!(bus.steps.len(), 1);
}

#[test]
fn explicit_expired_deadline_and_overflow_fail_before_io() {
    let mut bus = ScriptedBus::new([Step::Frame(data(0x0280_01fd, &POSE))]);
    let report = bus.recv_raw_report(
        Duration::ZERO,
        Duration::ZERO,
        ReceiveLimits {
            deadline: Some(Instant::now()),
            ..ReceiveLimits::default()
        },
    );
    assert_eq!(report.completion, ReceiveCompletion::Deadline);
    assert_eq!(bus.calls, 0);
    let report = bus.recv_raw_report(Duration::MAX, Duration::ZERO, ReceiveLimits::default());
    assert_eq!(report.completion, ReceiveCompletion::Failed);
    assert!(matches!(report.terminal_error, Some(BusError::Driver(_))));
    assert_eq!(bus.calls, 0);
}

#[test]
fn healthy_long_empty_poll_is_a_benign_timeout_with_bounded_attempts() {
    let mut bus = ScriptedBus::new([]);
    let report = bus.recv_feedback_report(
        &types(),
        Duration::from_millis(100),
        Duration::from_millis(1),
    );
    assert_eq!(report.completion, ReceiveCompletion::Idle);
    assert!(matches!(report.terminal_error, Some(BusError::RecvTimeout)));
    assert!((1..=256).contains(&report.read_attempts));
    assert_eq!(bus.calls, report.read_attempts);
}

#[test]
fn legacy_projections_preserve_valid_prefixes_and_report_unrepresentable_rx() {
    for malformed in [
        data(0x0280_01fd, &POSE[..7]),
        ReceivedCanFrame::new_remote(Some("can0".into()), 0x0280_01fd, true, 8).expect("remote"),
    ] {
        let mut bus = ScriptedBus::new([
            Step::Frame(data(0x0280_01fd, &POSE)),
            Step::Frame(malformed.clone()),
        ]);
        let mut raw = Vec::new();
        assert!(bus.recv_frames(&mut raw).is_err());
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].data, POSE);

        let mut bus = ScriptedBus::new([
            Step::Frame(data(0x0280_01fd, &POSE)),
            Step::Frame(malformed.clone()),
        ]);
        let mut states = HashMap::new();
        assert!(matches!(
            bus.recv_all_addressed(&types(), &mut states, Duration::ZERO, Duration::ZERO),
            Err(BusError::MalformedFeedback { .. })
        ));
        assert_eq!(
            states[&MotorAddress::new("can0", 1)].updated,
            Some(bus.stamp)
        );

        let mut bus = ScriptedBus::new([
            Step::Frame(data(0x0280_01fd, &POSE)),
            Step::Frame(malformed),
        ]);
        let mut states = HashMap::new();
        assert!(matches!(
            bus.recv_all(
                &HashMap::from([(1, MotorType::Rs03)]),
                &mut states,
                Duration::ZERO,
                Duration::ZERO
            ),
            Err(BusError::MalformedFeedback { .. })
        ));
        assert_eq!(states[&1].updated, Some(bus.stamp));
    }
    let mut error = data(0x40, &[0x80; 8]);
    error.kind = RxFrameKind::Error;
    let mut bus = ScriptedBus::new([Step::Frame(data(0x0280_01fd, &POSE)), Step::Frame(error)]);
    let mut states = HashMap::new();
    assert!(matches!(
        bus.recv_all_addressed(&types(), &mut states, Duration::ZERO, Duration::ZERO),
        Err(BusError::Driver(_))
    ));
    assert!(states[&MotorAddress::new("can0", 1)].updated.is_some());
}

#[test]
fn memory_backlog_suffix_retains_fifo_payloads_between_bounded_polls() {
    let mut bus = MemoryBus::default();
    for value in 0..130 {
        bus.rx_queue.push(CanFrame {
            id: 0x1f00_01fd,
            data: [value; 8],
            extended: true,
        });
    }
    let first = bus.recv_raw_report(Duration::ZERO, Duration::ZERO, ReceiveLimits::default());
    let second = bus.recv_raw_report(Duration::ZERO, Duration::ZERO, ReceiveLimits::default());
    let third = bus.recv_raw_report(Duration::ZERO, Duration::ZERO, ReceiveLimits::default());
    assert_eq!(
        [first.frames.len(), second.frames.len(), third.frames.len()],
        [64, 64, 2]
    );
    assert_eq!(first.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(second.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(third.completion, ReceiveCompletion::Idle);
    let payloads = first
        .frames
        .into_iter()
        .chain(second.frames)
        .chain(third.frames)
        .map(|frame| frame.received.frame.data[0])
        .collect::<Vec<_>>();
    assert_eq!(payloads, (0..130).collect::<Vec<_>>());
    assert!(bus.rx_queue.is_empty());
}
