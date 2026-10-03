//! Exercise periodic SafetyState publication through the actual installed loop.
//! All transport and resource inputs are isolated; no physical device is used.
#![allow(clippy::expect_used)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use armee_proto::prost::Message;
use armee_proto::{Envelope, FaultSeverity, OperationalMode as ProtoMode, SafetyState};
use berthier::ControlLoop;
use chappe::Bus;
use davout::{FaultClass, OperationalMode};
use robstride::{
    BusError, CanBus, CanFrame, MotorAddress, MotorBus, ReceiveAttempt, ReceivedCanFrame,
    RxFrameKind, TimedCanFrame,
};

use crate::limit_persist::{ConfigPersistQueue, PersistDrainStatus};
use crate::overlay::{ActuatorOverlay, TOPIC_ACTUATOR_COMMAND};
use crate::{run_control_loop, ControlLoopRuntime, PiCommand};

const BOUND: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Witness {
    writes: Vec<CanFrame>,
    delivered: usize,
    empty_reads: usize,
}

struct ScriptedBus {
    frames: VecDeque<ReceivedCanFrame>,
    witness: Arc<Mutex<Witness>>,
}

impl CanBus for ScriptedBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.witness
            .lock()
            .expect("write witness")
            .writes
            .push(frame.clone());
        Ok(())
    }

    fn send_frame_to(&mut self, _: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.send_frame(frame)
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let mut witness = self.witness.lock().expect("receive witness");
        if let Some(received) = self.frames.pop_front() {
            witness.delivered += 1;
            Ok(ReceiveAttempt::Frame(TimedCanFrame {
                received,
                received_at: Instant::now(),
            }))
        } else {
            witness.empty_reads += 1;
            Ok(ReceiveAttempt::Idle)
        }
    }
}

impl MotorBus for ScriptedBus {}

struct ShutdownOnDrop(Arc<AtomicBool>);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn status(id: u32) -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id,
            extended: true,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        },
    )
}

fn exercise(
    input: Vec<ReceivedCanFrame>,
    expected_faults: &[(FaultClass, &str)],
    hardware_estop: bool,
) {
    // Bind each actual-loop child independently of ambient runtime configuration,
    // calibration history and trace paths. Never mutate the shared test process.
    let Some(root) = std::env::var_os("MARENGO_SAFETY_PUBLICATION_FIXTURE") else {
        let fixture = publication_fixture();
        let test_thread = thread::current();
        let test = test_thread.name().expect("actual test name");
        let output = Command::new(std::env::current_exe().expect("actual test executable"))
            .args(["--exact", test, "--nocapture"])
            .current_dir(fixture.path())
            .env("MARENGO_SAFETY_PUBLICATION_FIXTURE", fixture.path())
            .env("MARENGO_ROOT", fixture.path())
            .env("MARENGO_CONFIG_DIR", fixture.path().join("config"))
            .env(
                "MARENGO_CALIBRATION_RECORD",
                fixture.path().join("missing-calibration.yaml"),
            )
            .env_remove("MARENGO_POSITION_TRACE")
            .env_remove("MARENGO_POSITION_TRACE_HZ")
            .env_remove("MARENGO_JOINT_SUBSET")
            .output()
            .expect("isolated actual-loop child");
        assert!(
            output.status.success(),
            "isolated publication assertion failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    };
    let root = PathBuf::from(root);
    let config = root.join("config");
    assert_eq!(
        marengo_config::resolve_config_dir(&root),
        config,
        "actual config resolution stays inside the exclusive fixture"
    );
    assert_eq!(
        std::env::var_os("MARENGO_CALIBRATION_RECORD"),
        Some(root.join("missing-calibration.yaml").into_os_string())
    );
    assert!(std::env::var_os("MARENGO_POSITION_TRACE").is_none());
    exercise_bound(&root, input, expected_faults, hardware_estop);
}

fn publication_fixture() -> tempfile::TempDir {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = tempfile::tempdir().expect("exclusive publication fixture");
    let config = fixture.path().join("config");
    std::fs::create_dir(&config).expect("fixture config directory");
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        std::fs::copy(source.join("config").join(name), config.join(name))
            .expect("copy immutable master input");
    }
    let urdf = fixture.path().join("assets/urdf");
    std::fs::create_dir_all(&urdf).expect("fixture model directory");
    std::fs::copy(
        source.join("assets/urdf/marengo.urdf"),
        urdf.join("marengo.urdf"),
    )
    .expect("copy immutable master model");
    fixture
}

fn exercise_bound(
    root: &std::path::Path,
    input: Vec<ReceivedCanFrame>,
    expected_faults: &[(FaultClass, &str)],
    hardware_estop: bool,
) {
    let config = root.join("config");
    let original = std::fs::read(config.join("control.yaml")).expect("copied source policy");
    let delivered = input.len() + 1;
    let fault_input = input.clone();
    let mut frames = VecDeque::from(input);
    // An actual healthy status follows the hazards in their same drain.
    frames.push_back(status(0x0200_04fd));
    let witness = Arc::new(Mutex::new(Witness::default()));
    let mut controller = ControlLoop::from_repo(
        root,
        ScriptedBus {
            frames,
            witness: Arc::clone(&witness),
        },
        200,
        25,
    )
    .expect("actual unreferenced controller with isolated transport");
    assert_eq!(controller.supervisor().mode(), OperationalMode::Disabled);
    if hardware_estop {
        controller.supervisor_mut().set_hardware_estop(true);
    }
    let bus = Arc::new(Bus::new(32));
    let mut publications = bus.subscribe("robot/safety");
    let shutdown = Arc::new(AtomicBool::new(false));
    let queue = ConfigPersistQueue::spawn(Arc::clone(&bus), root.to_path_buf());
    let mut overlay =
        ActuatorOverlay::from_config_dir(&config, queue).expect("real installed overlay");
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let mut enable_rx = bus.subscribe("robot/enable");
    let mut set_zero_rx = bus.subscribe("robot/set_zero");
    let mut lease_rx = bus.subscribe("robot/active_reporting_lease");
    let mut status_poll_rx = bus.subscribe("robot/motor_status_poll");
    let mut testing_cmd_rx = bus.subscribe("robot/testing/mit_command_batch");
    let mut actuator_rx = bus.subscribe(TOPIC_ACTUATOR_COMMAND);
    let mut runtime = ControlLoopRuntime {
        config_dir: &config,
        chappe_state_hz: 25,
        chappe: &bus,
        cmd_rx: &cmd_rx,
        enable_rx: &mut enable_rx,
        set_zero_rx: &mut set_zero_rx,
        lease_rx: &mut lease_rx,
        status_poll_rx: &mut status_poll_rx,
        testing_cmd_rx: &mut testing_cmd_rx,
        actuator_rx: &mut actuator_rx,
        actuator_overlay: &mut overlay,
        shutdown: &shutdown,
        motion: crate::motion_owner::MotionLease::new(crate::motion_owner::CommandSource::Stdin),
    };
    let observer_shutdown = Arc::clone(&shutdown);
    let observer = thread::spawn(move || {
        let _shutdown_on_failure = ShutdownOnDrop(Arc::clone(&observer_shutdown));
        let deadline = Instant::now() + BOUND;
        let mut states = Vec::new();
        while states.len() < 3 {
            assert!(
                Instant::now() < deadline,
                "actual loop did not publish three states"
            );
            match publications.try_recv() {
                Ok(bytes) => {
                    let envelope =
                        Envelope::decode(bytes.as_slice()).expect("actual published envelope");
                    assert_eq!(envelope.source_node, "marengo-pi");
                    assert_eq!(envelope.message_type, "marengo.v1.SafetyState");
                    states.push(
                        SafetyState::decode(envelope.payload.as_slice())
                            .expect("actual safety wire payload"),
                    );
                    if states.len() == 1 {
                        cmd_tx
                            .send(PiCommand::Disable)
                            .expect("ordinary installed Disable after publication");
                    }
                }
                Err(error) => {
                    assert!(
                        matches!(error, tokio::sync::broadcast::error::TryRecvError::Empty),
                        "publication failed: {error}"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
        observer_shutdown.store(true, Ordering::SeqCst);
        states
    });
    // Current reference authority stays on its owning thread. Only the passive
    // wire observer and ordinary command channel cross this test thread seam.
    run_control_loop(&mut controller, &mut runtime);
    let states = observer
        .join()
        .expect("bounded publication observer returned");
    let drain = overlay.close_persist_and_drain(BOUND);
    assert_eq!(drain.status, PersistDrainStatus::Complete);
    assert!(drain.worker_terminated);
    assert_eq!(
        std::fs::read(config.join("control.yaml")).expect("fixture policy after run"),
        original
    );
    assert!(controller.tick_count() >= 3);
    assert_eq!(controller.supervisor().mode(), OperationalMode::Disabled);
    let observed = controller.supervisor().safety_snapshot();
    let transport = witness.lock().expect("final wire witness");
    assert_eq!(transport.delivered, delivered);
    assert!(transport.empty_reads >= 3);
    for id in [0x0400fd01, 0x0400fd02, 0x0400fd03, 0x0400fd04, 0x0400fd05] {
        assert!(
            transport
                .writes
                .iter()
                .any(|frame| frame.id == id && frame.data == [0; 8]),
            "ordinary Disable attempts every configured address"
        );
    }
    assert!(
        !transport
            .writes
            .iter()
            .any(|frame| matches!(frame.id >> 24, 3 | 6)),
        "no Enable or SetZero admitted"
    );
    assert!(transport
        .writes
        .iter()
        .filter(|frame| frame.id >> 24 == 1)
        .all(|frame| frame.data == [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0]));
    assert!(!observed.recovery_available);
    assert_eq!(observed.hardware_estop_asserted, hardware_estop);
    assert_eq!(observed.faults.len(), expected_faults.len());
    for (index, (record, (class, joint))) in observed.faults.iter().zip(expected_faults).enumerate()
    {
        assert_eq!(record.id, index as u64 + 1);
        assert_eq!(record.class, *class);
        assert_eq!(record.joint.as_deref().unwrap_or(""), *joint);
        if *class == FaultClass::Transport {
            let raw = record
                .receive
                .first_frame
                .as_ref()
                .expect("initiating actual envelope");
            let input = fault_input
                .iter()
                .find(|frame| frame.kind == RxFrameKind::Error)
                .expect("literal error input");
            assert_eq!(raw.can_id, input.frame.id);
            assert_eq!(raw.raw, input.frame.data);
        }
    }
    for state in states {
        assert_eq!(state.mode, ProtoMode::Disabled as i32);
        assert_eq!(
            state.active_faults.len(),
            expected_faults.len(),
            "latched fault must remain in periodic wire state after healthy/Disabled ticks"
        );
        assert_eq!(
            state.hardware_estop_asserted, hardware_estop,
            "publish actual observed authority; no physical wiring claim"
        );
        assert_eq!(
            state.software_estop_latched,
            !expected_faults.is_empty(),
            "persistent authority cannot be published clear or invented"
        );
        for (index, (fault, (class, joint))) in
            state.active_faults.iter().zip(expected_faults).enumerate()
        {
            assert_eq!(fault.code, "runtime");
            assert_eq!(fault.joint, *joint);
            assert_eq!(
                fault.severity,
                if *class == FaultClass::HardwareEstop {
                    FaultSeverity::Estop
                } else {
                    FaultSeverity::Fault
                } as i32
            );
            assert!(fault
                .message
                .contains(&format!("persistent safety fault {}", index + 1)));
            assert!(fault.message.contains(&format!("({class:?})")));
        }
    }
}

#[test]
fn periodic_wire_state_retains_single_transport_fault_across_healthy_ticks_and_disable() {
    exercise(
        vec![ReceivedCanFrame {
            interface: Some("can0".into()),
            kind: RxFrameKind::Error,
            payload_len: 8,
            frame: CanFrame {
                id: 0x0000_0004,
                extended: false,
                data: [0, 1, 0, 0, 0, 0, 0, 0],
            },
        }],
        &[(FaultClass::Transport, "")],
        false,
    );
}

#[test]
fn periodic_wire_state_retains_single_device_fault_with_following_healthy_status() {
    exercise(
        vec![status(0x0201_04fd)],
        &[(FaultClass::Device, "right_elbow_pitch")],
        false,
    );
}

#[test]
fn healthy_periodic_wire_state_and_ordinary_disable_remain_clear() {
    exercise(vec![], &[], false);
}

#[test]
fn periodic_wire_state_keeps_faults_on_both_loaded_peer_addresses() {
    exercise(
        vec![status(0x0201_04fd), status(0x0201_05fd)],
        &[
            (FaultClass::Device, "right_elbow_pitch"),
            (FaultClass::Device, "right_lower_arm_yaw"),
        ],
        false,
    );
}

#[test]
fn periodic_wire_state_reports_observed_hardware_estop_authority() {
    // Existing simulated owner input only: actual Pi GPIO wiring remains open.
    exercise(vec![], &[(FaultClass::HardwareEstop, "")], true);
}
