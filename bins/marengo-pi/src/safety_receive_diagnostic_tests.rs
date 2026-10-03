//! Actual producer diagnostics from retained owner evidence, using isolated input.
#![allow(clippy::expect_used)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use armee_proto::prost::Message;
use armee_proto::{Envelope, FaultSeverity, OperationalMode as ProtoMode, SafetyState};
use davout::{FaultClass, OperationalMode, Supervisor};
use robstride::{
    BusError, CanBus, CanFrame, MotorBus, ReceiveAttempt, ReceivedCanFrame, RxFrameKind,
    TimedCanFrame,
};

struct InputBus {
    input: Arc<Mutex<VecDeque<ReceivedCanFrame>>>,
    writes: Vec<CanFrame>,
}

impl CanBus for InputBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.writes.push(frame.clone());
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        Ok(
            match self.input.lock().expect("isolated input").pop_front() {
                Some(received) => ReceiveAttempt::Frame(TimedCanFrame {
                    received,
                    received_at: Instant::now(),
                }),
                None => ReceiveAttempt::Idle,
            },
        )
    }
}

impl MotorBus for InputBus {}

fn healthy() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0200_04fd,
            extended: true,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        },
    )
}

fn error(data: [u8; 8], payload_len: u8) -> ReceivedCanFrame {
    ReceivedCanFrame {
        interface: Some("can0".into()),
        kind: RxFrameKind::Error,
        payload_len,
        frame: CanFrame {
            id: 4,
            extended: false,
            data,
        },
    }
}

fn exercise(input: Vec<ReceivedCanFrame>, expected: Option<(FaultClass, &str)>) {
    // Each child binds configuration/history independently of the operator process.
    let Some(root) = std::env::var_os("MARENGO_RECEIVE_DIAGNOSTIC_FIXTURE") else {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fixture = tempfile::tempdir().expect("exclusive diagnostic fixture");
        let config = fixture.path().join("config");
        std::fs::create_dir(&config).expect("fixture config");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(source.join("config").join(name), config.join(name))
                .expect("immutable master config");
        }
        let urdf = fixture.path().join("assets/urdf");
        std::fs::create_dir_all(&urdf).expect("fixture model directory");
        std::fs::copy(
            source.join("assets/urdf/marengo.urdf"),
            urdf.join("marengo.urdf"),
        )
        .expect("immutable master model");
        let test_thread = std::thread::current();
        let output = Command::new(std::env::current_exe().expect("actual test executable"))
            .args([
                "--exact",
                test_thread.name().expect("actual test name"),
                "--nocapture",
            ])
            .current_dir(fixture.path())
            .env("MARENGO_RECEIVE_DIAGNOSTIC_FIXTURE", fixture.path())
            .env("MARENGO_ROOT", fixture.path())
            .env("MARENGO_CONFIG_DIR", &config)
            .env_remove("MARENGO_JOINT_SUBSET")
            .env_remove("MARENGO_POSITION_TRACE")
            .env_remove("MARENGO_POSITION_TRACE_HZ")
            .output()
            .expect("isolated producer child");
        assert!(
            output.status.success(),
            "actual producer assertion failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    };
    let root = PathBuf::from(root);
    let first = input.first().cloned();
    let mut frames = VecDeque::from(input);
    frames.push_back(healthy());
    let input = Arc::new(Mutex::new(frames));
    let mut supervisor = Supervisor::from_repo(
        &root,
        InputBus {
            input: Arc::clone(&input),
            writes: Vec::new(),
        },
    )
    .expect("isolated unreferenced owner");
    let result = supervisor.drain_feedback();
    assert_eq!(result.is_err(), expected.is_some());
    let retained = supervisor.safety_snapshot();
    assert_eq!(retained.faults.len(), usize::from(expected.is_some()));
    if let Some((class, _)) = expected {
        let record = &retained.faults[0];
        assert_eq!(record.class, class);
        let evidence = record
            .receive
            .first_frame
            .as_ref()
            .expect("first owner envelope");
        let original = first.as_ref().expect("initiating input");
        assert_eq!(evidence.raw, original.frame.data);
        assert_eq!(evidence.payload_len, original.payload_len);
        assert_eq!(evidence.kind, original.kind);
        assert_eq!(evidence.can_id, original.frame.id);
    }
    let bus = chappe::Bus::new(8);
    let mut publications = bus.subscribe("robot/safety");
    let mut messages = Vec::new();
    for _ in 0..3 {
        supervisor.disable_all().expect("ordinary all-address stop");
        input
            .lock()
            .expect("new healthy input")
            .push_back(healthy());
        let _ = supervisor.drain_feedback();
        crate::publish_safety(&bus, &supervisor, None).expect("actual safety producer");
        let envelope = Envelope::decode(publications.try_recv().expect("publication").as_slice())
            .expect("actual envelope");
        assert_eq!(envelope.source_node, "marengo-pi");
        assert_eq!(envelope.message_type, "marengo.v1.SafetyState");
        let state = SafetyState::decode(envelope.payload.as_slice()).expect("actual wire state");
        assert_eq!(state.mode, ProtoMode::Disabled as i32);
        assert_eq!(state.software_estop_latched, expected.is_some());
        assert_eq!(state.active_faults.len(), usize::from(expected.is_some()));
        if let Some((_, suffix)) = expected {
            let fault = &state.active_faults[0];
            assert_eq!(fault.severity, FaultSeverity::Fault as i32);
            assert!(
                fault.message.ends_with(suffix),
                "missing original receive diagnostic: {}",
                fault.message
            );
            messages.push(fault.message.clone());
        }
    }
    assert!(
        messages.windows(2).all(|pair| pair[0] == pair[1]),
        "later healthy/Disable publications retain the initiating diagnostic"
    );
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert!(!supervisor.safety_snapshot().recovery_available);
    assert!(!supervisor
        .bus()
        .writes
        .iter()
        .any(|frame| matches!(frame.id >> 24, 3 | 6)));
    assert!(supervisor
        .bus()
        .writes
        .iter()
        .filter(|frame| frame.id >> 24 == 1)
        .all(|frame| frame.data == [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0]));
}

#[test]
fn original_error_envelope_survives_later_error_healthy_and_disable_publications() {
    let mut later = error([9; 8], 8);
    later.frame.id = 0x2000;
    exercise(
        vec![error([0, 1, 0, 0, 0, 0, 0, 0], 8), later],
        Some((FaultClass::Transport, "first receive: interface=Some(\"can0\"), kind=Error, can_id=0x00000004, extended=false, payload_len=8, data=0001000000000000")),
    );
}

#[test]
fn short_error_publishes_only_received_prefix() {
    exercise(
        vec![error([1, 2, 3, 0xee, 0xee, 0xee, 0xee, 0xee], 3)],
        Some((FaultClass::Transport, "first receive: interface=Some(\"can0\"), kind=Error, can_id=0x00000004, extended=false, payload_len=3, data=010203")),
    );
}

#[test]
fn remote_request_publishes_requested_length_without_payload() {
    let mut remote = healthy();
    remote.kind = RxFrameKind::Remote { requested_len: 8 };
    remote.payload_len = 0;
    remote.frame.data = [0xee; 8];
    exercise(
        vec![remote],
        Some((FaultClass::Feedback, "first receive: interface=Some(\"can0\"), kind=Remote { requested_len: 8 }, can_id=0x020004fd, extended=true, payload_len=0, data=, reason=feedback is not a data frame")),
    );
}

#[test]
fn absent_interface_is_published_as_unknown_without_losing_error_bits() {
    let mut unknown = error([1; 8], 8);
    unknown.interface = None;
    unknown.frame.id = 0x2000;
    exercise(
        vec![unknown],
        Some((FaultClass::Transport, "first receive: interface=None, kind=Error, can_id=0x00002000, extended=false, payload_len=8, data=0101010101010101")),
    );
}

#[test]
fn short_configured_status_retains_malformed_reason_and_received_prefix() {
    let mut short = healthy();
    short.payload_len = 2;
    short.frame.data = [0x7f, 0xff, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee];
    exercise(
        vec![short],
        Some((FaultClass::Feedback, "first receive: interface=Some(\"can0\"), kind=Data, can_id=0x020004fd, extended=true, payload_len=2, data=7fff, reason=expected eight data bytes, received 2")),
    );
}

#[test]
fn healthy_input_does_not_invent_receive_fault_diagnostics() {
    exercise(vec![], None);
}
