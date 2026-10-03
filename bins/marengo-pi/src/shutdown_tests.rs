//! Graceful-exit regressions against the installed lifecycle and real writer.
//!
//! The first probe is replayed unchanged after a separately reviewed parity
//! extraction. Its archive/manifest binding is recorded outside this source;
//! this is not an unchanged-original-binary shutdown fixture.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{ActionEvent, Envelope, PersistStatus};
use berthier::{ControlLoop, ControlMode};
use chappe::Bus;
use davout::{JointHomingState, OperationalMode, StopAction, StopReport};
use marengo_config::{load_command_joint_allowlist_from, load_control_config_from};
use robstride::{BusError, CanBus, CanFrame, MotorAddress, MotorBus, ReceiveAttempt};

use crate::limit_persist::{ConfigPersistQueue, PersistRequest, PersistTestHooks};
use crate::overlay::{ActuatorOverlay, TOPIC_AUDIT_ACTION};
use crate::{finish_owner_shutdown, ExitStopOutcome};

const CLEANUP_BOUND: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttemptedFrame {
    address: Option<MotorAddress>,
    frame: CanFrame,
}

struct RecordingBus {
    witness: Arc<Mutex<Vec<AttemptedFrame>>>,
}

impl CanBus for RecordingBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.witness
            .lock()
            .expect("external transmit witness")
            .push(AttemptedFrame {
                address: None,
                frame: frame.clone(),
            });
        Ok(())
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.witness
            .lock()
            .expect("external addressed transmit witness")
            .push(AttemptedFrame {
                address: Some(address.clone()),
                frame: frame.clone(),
            });
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        Ok(ReceiveAttempt::Idle)
    }
}

impl MotorBus for RecordingBus {}

/// A failed assertion or fixture error must release a paused real worker.
struct ReleaseOnDrop(Sender<()>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn copied_owner_fixture() -> (tempfile::TempDir, PathBuf) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(expected) = std::env::var_os("BATCH07_EXPECTED_MANIFEST") {
        assert_eq!(
            manifest.canonicalize().expect("compiled manifest"),
            PathBuf::from(expected)
                .canonicalize()
                .expect("expected frozen manifest"),
            "probe must compile against the explicitly bound frozen owner"
        );
    }
    let root = manifest.join("../..");
    let temp = tempfile::tempdir().expect("exclusive copied owner");
    let config = temp.path().join("config");
    std::fs::create_dir_all(&config).expect("copied config directory");
    for name in ["robot.yaml", "control.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(root.join("config").join(name), config.join(name))
            .expect("copy existing master fixture");
    }
    let urdf = temp.path().join("assets/urdf");
    std::fs::create_dir_all(&urdf).expect("copied model directory");
    std::fs::copy(
        root.join("assets/urdf/marengo.urdf"),
        urdf.join("marengo.urdf"),
    )
    .expect("copy existing master model");
    (temp, config)
}

/// Expected routes/payloads are literal fixture declarations, not production
/// stop reports or the same encoding helpers that generated the actual output.
fn literal_stop_trace() -> Vec<AttemptedFrame> {
    [
        (1, [0x1200fd01, 0x017fff01, 0x0400fd01]),
        (2, [0x1200fd02, 0x017fff02, 0x0400fd02]),
        (3, [0x1200fd03, 0x017fff03, 0x0400fd03]),
        (4, [0x1200fd04, 0x017fff04, 0x0400fd04]),
        (5, [0x1200fd05, 0x017fff05, 0x0400fd05]),
    ]
    .into_iter()
    .flat_map(|(device_id, ids)| {
        ids.into_iter()
            .zip([
                [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
                [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
                [0; 8],
            ])
            .map(move |(id, data)| AttemptedFrame {
                address: Some(MotorAddress::new("can0", device_id)),
                frame: CanFrame {
                    id,
                    data,
                    extended: true,
                },
            })
    })
    .collect()
}

fn stop_trace(frames: &[AttemptedFrame]) -> Vec<AttemptedFrame> {
    frames
        .iter()
        .filter(|tx| matches!(tx.frame.id >> 24, 0x12 | 0x01 | 0x04))
        .cloned()
        .collect()
}

#[derive(Debug)]
struct WaitObservation {
    frames: Vec<AttemptedFrame>,
    control_mode: ControlMode,
    stop_report: Option<StopReport>,
    writer_blocked: bool,
}

#[test]
fn all_original_stop_writes_precede_waiting_for_the_real_gated_writer() {
    let (temp, config_dir) = copied_owner_fixture();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("original config bytes");
    let witness = Arc::new(Mutex::new(Vec::new()));
    let mut controller = ControlLoop::from_repo(
        temp.path(),
        RecordingBus {
            witness: Arc::clone(&witness),
        },
        200,
        25,
    )
    .expect("actual controller with unqualified recording transport");
    assert_eq!(controller.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(controller.supervisor().motors.motors.len(), 5);
    for joint in controller.joint_names() {
        assert_eq!(
            controller.supervisor().joint_homing_state(joint),
            JointHomingState::Unhomed
        );
    }
    let startup = witness.lock().expect("startup witness").clone();
    // Reporting writes are spaced one per interface per control period, so
    // construction reaches only the first configured On.
    assert_eq!(
        startup.iter().map(|tx| tx.frame.id).collect::<Vec<_>>(),
        [0x1800fd01],
        "ordinary construction must reach the real configured diagnostic output"
    );
    witness.lock().expect("clear checked setup traffic").clear();
    controller.set_control_mode(ControlMode::GravityComp);

    let shutdown = Arc::new(AtomicBool::new(false));
    let chappe = Arc::new(Bus::new(16));
    let mut actions = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleaseOnDrop(release_tx.clone());
    let writer_blocked = Arc::new(AtomicBool::new(false));
    let gate_timed_out = Arc::new(AtomicBool::new(false));
    let gate_rx = Mutex::new(release_rx);
    let blocked_in_hook = Arc::clone(&writer_blocked);
    let timeout_in_hook = Arc::clone(&gate_timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        Arc::clone(&shutdown),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |_| {
                blocked_in_hook.store(true, Ordering::SeqCst);
                let _ = entered_tx.send(());
                if gate_rx
                    .lock()
                    .expect("one real writer gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    timeout_in_hook.store(true, Ordering::SeqCst);
                }
                blocked_in_hook.store(false, Ordering::SeqCst);
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let mut draft = load_control_config_from(&config_dir).expect("real persisted draft");
    draft
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("fixture elbow")
        .impedance
        .kp = 37.0;
    queue
        .enqueue(PersistRequest {
            config_dir: config_dir.clone(),
            motors: None,
            control: draft,
            timestamp_ms: 707,
            session_id: "batch07-stop-before-wait".into(),
            operator_id: "shutdown-regression".into(),
            joint: "right_elbow_pitch".into(),
            param: "impedance.kp".into(),
        })
        .expect("accept a real retained write");
    let overlay = ActuatorOverlay::new(
        load_command_joint_allowlist_from(&config_dir).expect("fixture allowlist"),
        queue,
    );
    entered_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual writer reached its gated filesystem boundary");
    shutdown.store(true, Ordering::SeqCst);
    let observation = Mutex::new(None);
    let before_wait = |controller: &ControlLoop<RecordingBus>| {
        *observation.lock().expect("wait boundary observation") = Some(WaitObservation {
            frames: witness.lock().expect("immutable external witness").clone(),
            control_mode: controller.control_mode(),
            stop_report: controller.supervisor().safety_snapshot().last_stop,
            writer_blocked: writer_blocked.load(Ordering::SeqCst),
        });
        // Release even when the trace proves the old order is wrong. Assertions
        // happen after cleanup, so this regression fails rather than hangs.
        let _ = release_tx.send(());
    };
    let outcome = finish_owner_shutdown(
        &mut controller,
        &overlay,
        true,
        CLEANUP_BOUND,
        Some(&before_wait),
    );
    drop(overlay);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("real worker exited after finishing its write and audit publication");

    // Reachability controls: actual I/O, matching publication and actual stop
    // all complete even against the unfixed extracted path.
    assert!(
        !gate_timed_out.load(Ordering::SeqCst),
        "gate was explicitly released"
    );
    assert!(outcome.persist_idle, "real writer completed: {outcome:?}");
    let loaded = load_control_config_from(&config_dir).expect("reload real durable YAML");
    assert_eq!(
        loaded.control.joints["right_elbow_pitch"].impedance.kp,
        37.0
    );
    assert_ne!(
        std::fs::read(config_dir.join("control.yaml")).expect("actual new bytes"),
        original
    );
    let event_bytes = actions
        .try_recv()
        .expect("matching actual worker publication");
    let envelope = Envelope::decode(event_bytes.as_slice()).expect("actual audit envelope");
    let event = ActionEvent::decode(envelope.payload.as_slice()).expect("actual audit action");
    assert_eq!(event.timestamp_ms, 707);
    assert_eq!(event.session_id, "batch07-stop-before-wait");
    assert_eq!(event.operator_id, "shutdown-regression");
    assert_eq!(event.joint, "right_elbow_pitch");
    assert_eq!(event.action, "config_persist");
    assert!(event.accepted);
    assert_eq!(event.persist_status, PersistStatus::Durable as i32);
    assert!(!event.config_revision.is_empty());
    let final_frames = witness.lock().expect("final actual stop trace").clone();
    assert_eq!(stop_trace(&final_frames), literal_stop_trace());
    assert!(
        final_frames
            .iter()
            .all(|tx| matches!(tx.frame.id >> 24, 0x12 | 0x01 | 0x04 | 0x18)),
        "only the stop sequence and free-drive reporting maintenance may transmit"
    );
    let ExitStopOutcome::Attempted { result, report } = outcome.stop else {
        panic!("configured exit policy must attempt actual stop");
    };
    assert!(result.is_ok(), "actual stop result: {result:?}");
    let report = report.expect("actual supervisor stop report");
    assert_eq!(report.attempts.len(), 15);
    assert_eq!(report.failed_writes(), 0);
    let report_routes = report
        .attempts
        .iter()
        .map(|attempt| (attempt.address.clone(), attempt.action))
        .collect::<Vec<_>>();
    let expected_routes = [1, 2, 3, 4, 5]
        .into_iter()
        .flat_map(|id| {
            [
                StopAction::ZeroSpeed,
                StopAction::NeutralMit,
                StopAction::Disable,
            ]
            .into_iter()
            .map(move |action| (MotorAddress::new("can0", id), action))
        })
        .collect::<Vec<_>>();
    assert_eq!(report_routes, expected_routes);

    let at_wait = observation
        .into_inner()
        .expect("captured wait observation")
        .expect("installed lifecycle reached its storage wait boundary");
    println!("actual WaitEntered={at_wait:?}, final stop={report:?}, durable action={event:?}");
    assert!(
        at_wait.writer_blocked,
        "wait boundary observed while real I/O was gated"
    );
    assert_eq!(
        stop_trace(&at_wait.frames),
        literal_stop_trace(),
        "all fifteen original addressed stop writes must precede persistence waiting"
    );
    assert_eq!(at_wait.control_mode, ControlMode::Disabled);
    assert_eq!(at_wait.stop_report.as_ref(), Some(&report));
}

#[test]
fn stdin_quit_prevents_a_later_actuator_command_and_motion_tick() {
    use armee_proto::{
        actuator_command::Payload, ActuatorCommand, OperatorCommand, TuningChange,
        TuningChangeEvent, TuningTier,
    };
    use davout::simulation::{
        InitialVirtualReference, SimulationBus, TxMatcher, TxOccurrence, TxRule,
    };

    use crate::overlay::{TOPIC_ACTUATOR_COMMAND, TOPIC_AUDIT_TUNING};
    use crate::{run_control_loop, ControlLoopRuntime, PiCommand};

    // The neighbor exercises the actual subsystem dispatch and tick. Only the
    // Quit fixture invokes the installed outer loop; neither copies its logic.
    for quit_before_dispatch in [false, true] {
        let (temp, config_dir) = copied_owner_fixture();
        let original = std::fs::read(config_dir.join("control.yaml")).expect("original fixture");
        let mut bus = SimulationBus::default();
        let enabled_status = bus
            .add_tx_rule(TxRule {
                matcher: TxMatcher {
                    communication_type: Some(3),
                    device_id: Some(4),
                    interface: Some("can0".into()),
                },
                occurrence: TxOccurrence::Nth(1),
                // Literal vendor Run status: elbow, centered pose/velocity/
                // torque, 20 C, no fault. Delivered after actual scoped Enable.
                receive: vec![CanFrame {
                    id: 0x028004fd,
                    data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                    extended: true,
                }
                .into()],
                send_error: None,
            })
            .expect("finite raw status after the actual Enable attempt");
        let mut controller = ControlLoop::from_simulation(
            temp.path(),
            bus,
            InitialVirtualReference::Joints(vec!["right_elbow_pitch".into()]),
            200,
            25,
        )
        .expect("closed INITIAL virtual reference fixture, no arbitrary bus grant");
        controller.set_control_mode(ControlMode::Impedance);
        controller
            .supervisor_mut()
            .enable_targets(&["right_elbow_pitch".into()])
            .expect("actual scoped Enable admission");
        assert_eq!(controller.supervisor().mode(), OperationalMode::Active);
        assert_eq!(
            controller
                .supervisor_mut()
                .bus_mut()
                .rule_trigger_count(enabled_status),
            1,
            "actual elbow Enable reached the raw status fixture"
        );
        assert!(controller.gain_override("right_elbow_pitch").is_none());
        assert_eq!(controller.tick_count(), 0);
        controller.supervisor_mut().bus_mut().clear_trace();
        // Enable deliberately consumes traffic received before active_since.
        // A separate raw observation handed off after Enable is required for
        // current-session motion; no cache or reference setter substitutes it.
        controller
            .supervisor_mut()
            .bus_mut()
            .queue_frame(CanFrame {
                id: 0x028004fd,
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                extended: true,
            })
            .expect("finite current-session raw status");
        controller
            .supervisor_mut()
            .drain_feedback()
            .expect("actual decoding of post-Enable raw status");
        assert!(
            controller
                .supervisor()
                .joint_feedback("right_elbow_pitch")
                .expect("actual current-session elbow pose")
                .position_rad
                .abs()
                < 0.001
        );

        let chappe = Arc::new(Bus::new(16));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (exited_tx, exited_rx) = mpsc::channel();
        let queue = ConfigPersistQueue::spawn_with_test_hooks(
            Arc::clone(&chappe),
            Arc::clone(&shutdown),
            temp.path().to_path_buf(),
            PersistTestHooks {
                on_worker_exit: Some(Arc::new(move || {
                    let _ = exited_tx.send(());
                })),
                ..PersistTestHooks::default()
            },
        );
        let mut overlay = ActuatorOverlay::new(
            load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
            queue,
        );
        let mut enable_rx = chappe.subscribe("robot/enable");
        let mut homing_rx = chappe.subscribe("robot/homing");
        let mut set_zero_rx = chappe.subscribe("robot/set_zero");
        let mut lease_rx = chappe.subscribe("robot/active_reporting_lease");
        let mut status_poll_rx = chappe.subscribe("robot/motor_status_poll");
        let mut testing_cmd_rx = chappe.subscribe("robot/testing/mit_command_batch");
        let mut actuator_rx = chappe.subscribe(TOPIC_ACTUATOR_COMMAND);
        let mut tuning_rx = chappe.subscribe(TOPIC_AUDIT_TUNING);
        let op = OperatorCommand {
            timestamp_ms: 7083,
            session_id: "batch07-quit-followup".into(),
            operator_id: "quit-regression".into(),
            seq: 1,
            command: Some(ActuatorCommand {
                joint: "right_elbow_pitch".into(),
                payload: Some(Payload::Tuning(TuningChange {
                    tier: TuningTier::RuntimeMit as i32,
                    param: "kp".into(),
                    value: 83.0,
                    persist: false,
                })),
            }),
        };
        chappe
            .publish(
                TOPIC_ACTUATOR_COMMAND,
                "quit-regression",
                "marengo.v1.OperatorCommand",
                &op,
            )
            .expect("actual queued operator envelope");
        let (cmd_tx, cmd_rx) = mpsc::channel();

        if quit_before_dispatch {
            cmd_tx
                .send(PiCommand::Quit)
                .expect("Quit queued before entry");
            let mut runtime = ControlLoopRuntime {
                config_dir: &config_dir,
                chappe_state_hz: 25,
                chappe: &chappe,
                cmd_rx: &cmd_rx,
                enable_rx: &mut enable_rx,
                homing_rx: &mut homing_rx,
                set_zero_rx: &mut set_zero_rx,
                lease_rx: &mut lease_rx,
                status_poll_rx: &mut status_poll_rx,
                testing_cmd_rx: &mut testing_cmd_rx,
                actuator_rx: &mut actuator_rx,
                actuator_overlay: &mut overlay,
                shutdown: &shutdown,
                motion: crate::motion_owner::MotionLease::new(
                    crate::motion_owner::CommandSource::Stdin,
                ),
            };
            run_control_loop(&mut controller, &mut runtime);
        } else {
            overlay.drain_commands(&mut controller, &config_dir, &chappe, &mut actuator_rx);
            controller
                .tick(Some(chappe.as_ref()))
                .expect("actual Active neighbor tick, not a mocked output");
        }

        // Snapshot before the real shutdown cleanup clears gain intent and
        // emits stop writes. Cleanup cannot erase evidence of Quit fallthrough.
        let gain_after_dispatch = controller.gain_override("right_elbow_pitch").cloned();
        let ticks_after_dispatch = controller.tick_count();
        let frames_after_dispatch = controller
            .supervisor()
            .bus()
            .transmissions()
            .iter()
            .map(|tx| AttemptedFrame {
                address: tx.address.clone(),
                frame: tx.frame.clone(),
            })
            .collect::<Vec<_>>();
        let queued_after_dispatch = actuator_rx.try_recv().ok();
        let tuning_after_dispatch = tuning_rx.try_recv().ok();
        shutdown.store(true, Ordering::SeqCst);
        let outcome = finish_owner_shutdown(&mut controller, &overlay, true, CLEANUP_BOUND, None);
        drop(overlay);
        exited_rx
            .recv_timeout(CLEANUP_BOUND)
            .expect("actual persist worker returned after owner cleanup");

        assert!(outcome.persist_idle);
        assert!(outcome.persist.worker_terminated);
        assert!(matches!(
            outcome.stop,
            ExitStopOutcome::Attempted { result: Ok(()), .. }
        ));
        assert_eq!(
            std::fs::read(config_dir.join("control.yaml")).expect("nonpersistent command bytes"),
            original
        );
        assert_eq!(controller.control_mode(), ControlMode::Disabled);
        assert!(controller.gain_override("right_elbow_pitch").is_none());
        println!(
            "quit_before_dispatch={quit_before_dispatch}, gain={gain_after_dispatch:?}, ticks={ticks_after_dispatch}, raw={frames_after_dispatch:?}, queued={}, tuning={}",
            queued_after_dispatch.is_some(),
            tuning_after_dispatch.is_some()
        );

        if !quit_before_dispatch {
            assert_eq!(
                gain_after_dispatch.expect("actual gain application").kp,
                83.0
            );
            assert_eq!(ticks_after_dispatch, 1);
            assert!(queued_after_dispatch.is_none());
            let bytes = tuning_after_dispatch.expect("actual matching tuning publication");
            let envelope = Envelope::decode(bytes.as_slice()).expect("actual tuning envelope");
            let event = TuningChangeEvent::decode(envelope.payload.as_slice())
                .expect("actual tuning event");
            assert_eq!(event.session_id, "batch07-quit-followup");
            assert_eq!(event.operator_id, "quit-regression");
            assert_eq!(event.joint, "right_elbow_pitch");
            assert_eq!(event.param, "kp");
            assert_eq!(event.after, 83.0);
            assert!(
                frames_after_dispatch.iter().any(|tx| {
                    tx.address == Some(MotorAddress::new("can0", 4))
                        && tx.frame.id >> 24 == 1
                        && tx.frame.id & 0xff == 4
                        && tx.frame.data[4] != 0
                }),
                "the actual Active neighbor emits non-neutral elbow MIT gains"
            );
            continue;
        }

        // Decisive actual-loop regression assertion follows all cleanup and
        // the neighboring command/gain/motion reachability controls.
        assert!(
            gain_after_dispatch.is_none(),
            "Quit must stop before applying a later actuator command"
        );
        assert_eq!(
            ticks_after_dispatch, 0,
            "Quit must prevent a later motion tick"
        );
        assert!(
            frames_after_dispatch.is_empty(),
            "Quit dispatch cannot send later motion"
        );
        assert!(
            tuning_after_dispatch.is_none(),
            "unprocessed command has no tuning ACK"
        );
        let retained = queued_after_dispatch.expect("Quit leaves the later command unprocessed");
        let envelope = Envelope::decode(retained.as_slice()).expect("retained actual envelope");
        assert_eq!(
            OperatorCommand::decode(envelope.payload.as_slice()).expect("retained operator"),
            op
        );
    }
}

/// The injected uncertainty is at the transport boundary, after recording the
/// actual write. The real Davout stop/report path receives the actual BusError.
struct FailedStopBus(RecordingBus);

impl CanBus for FailedStopBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.0.send_frame(frame)
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.0.send_frame_to(address, frame)?;
        if address == &MotorAddress::new("can0", 3) && frame.id == 0x1200fd03 {
            Err(BusError::Driver(
                "batch07 zero-speed delivery uncertainty".into(),
            ))
        } else {
            Ok(())
        }
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        self.0.recv_one_nonblocking()
    }
}

impl MotorBus for FailedStopBus {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorageQualification {
    Durable,
    Failed,
    Unfinished,
}

fn qualify_failed_stop_with_actual_storage(storage: StorageQualification) {
    use crate::limit_persist::PersistDrainStatus;

    let (temp, config_dir) = copied_owner_fixture();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("original copied bytes");
    let witness = Arc::new(Mutex::new(Vec::new()));
    let mut controller = ControlLoop::from_repo(
        temp.path(),
        FailedStopBus(RecordingBus {
            witness: Arc::clone(&witness),
        }),
        200,
        25,
    )
    .expect("actual unreferenced owner with fallible recording transport");
    controller.set_control_mode(ControlMode::Impedance);
    let generation_before = controller.supervisor().stop_generation();
    witness
        .lock()
        .expect("clear verified startup output")
        .clear();
    let bus = Arc::new(Bus::new(16));
    let mut audit = bus.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleaseOnDrop(release_tx.clone());
    let gate = Mutex::new(release_rx);
    let blocked = Arc::new(AtomicBool::new(false));
    let hook_blocked = Arc::clone(&blocked);
    let timed_out = Arc::new(AtomicBool::new(false));
    let hook_timed_out = Arc::clone(&timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&bus),
        Arc::new(AtomicBool::new(true)),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |_| {
                hook_blocked.store(true, Ordering::SeqCst);
                let _ = entered_tx.send(());
                if gate
                    .lock()
                    .expect("real fallible-stop writer gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    hook_timed_out.store(true, Ordering::SeqCst);
                }
                hook_blocked.store(false, Ordering::SeqCst);
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let mut draft = load_control_config_from(&config_dir).expect("actual valid source draft");
    draft
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("fixture elbow")
        .impedance
        .kp = 57.0;
    let destination = if storage == StorageQualification::Failed {
        let blocker = temp.path().join("write-parent-blocker");
        std::fs::write(&blocker, b"ordinary parent file")
            .expect("real root-independent failure fixture");
        blocker.join("config")
    } else {
        config_dir.clone()
    };
    queue
        .enqueue(PersistRequest {
            config_dir: destination,
            motors: None,
            control: draft,
            timestamp_ms: 7057,
            session_id: "batch07-failed-stop".into(),
            operator_id: "shutdown-regression".into(),
            joint: "right_elbow_pitch".into(),
            param: "impedance.kp".into(),
        })
        .expect("actual accepted storage request");
    let overlay = ActuatorOverlay::new(
        load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
        queue,
    );
    entered_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual writer entered before the exit");
    let observation = Mutex::new(None);
    let before_wait = |controller: &ControlLoop<FailedStopBus>| {
        *observation.lock().expect("failed-stop wait observation") = Some(WaitObservation {
            frames: witness
                .lock()
                .expect("external failed-stop witness")
                .clone(),
            control_mode: controller.control_mode(),
            stop_report: controller.supervisor().safety_snapshot().last_stop,
            writer_blocked: blocked.load(Ordering::SeqCst),
        });
        if storage != StorageQualification::Unfinished {
            let _ = release_tx.send(());
        }
    };
    let timeout = if storage == StorageQualification::Unfinished {
        Duration::ZERO
    } else {
        CLEANUP_BOUND
    };
    let outcome =
        finish_owner_shutdown(&mut controller, &overlay, true, timeout, Some(&before_wait));
    // A zero-timeout outcome is retained verbatim; later successful cleanup
    // cannot turn either the original unfinished drain or failed stop into Ok.
    let _ = release_tx.send(());
    let final_drain = overlay.close_persist_and_drain(CLEANUP_BOUND);
    drop(overlay);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual storage worker returned after cleanup");
    let bytes = audit
        .try_recv()
        .expect("actual matching terminal after cleanup");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual storage envelope");
    let event = ActionEvent::decode(envelope.payload.as_slice()).expect("actual terminal event");
    let at_wait = observation
        .into_inner()
        .expect("captured actual wait")
        .expect("wait observer reached");
    let final_frames = witness
        .lock()
        .expect("final actual attempted writes")
        .clone();
    println!("storage={storage:?}, before_wait={at_wait:?}, original={outcome:?}, final_drain={final_drain:?}, actual_event={event:?}");
    assert!(!timed_out.load(Ordering::SeqCst));
    assert!(at_wait.writer_blocked);
    assert_eq!(at_wait.control_mode, ControlMode::Disabled);
    assert_eq!(stop_trace(&at_wait.frames), literal_stop_trace());
    assert_eq!(
        stop_trace(&final_frames),
        literal_stop_trace(),
        "storage must not retry/mask the initiating stop"
    );
    let ExitStopOutcome::Attempted { result, report } = outcome.stop else {
        panic!("stop policy must remain explicit");
    };
    assert!(matches!(
        result,
        Err(davout::DavoutError::StopDelivery { failed_writes: 1 })
    ));
    let report = report.expect("actual failed stop report");
    assert_eq!(report.generation, generation_before + 1);
    assert_eq!(report.attempts.len(), 15);
    assert_eq!(report.failed_writes(), 1);
    assert_eq!(report.attempts[6].address, MotorAddress::new("can0", 3));
    assert_eq!(report.attempts[6].action, StopAction::ZeroSpeed);
    assert!(report.attempts[6]
        .error
        .as_ref()
        .is_some_and(|error| error.contains("batch07 zero-speed delivery uncertainty")));
    assert!(report
        .attempts
        .iter()
        .enumerate()
        .all(|(index, attempt)| (index == 6) == attempt.error.is_some()));
    assert_eq!(at_wait.stop_report.as_ref(), Some(&report));
    let safety = controller.supervisor().safety_snapshot();
    assert_eq!(safety.last_stop.as_ref(), Some(&report));
    assert_eq!(safety.first_failed_stop.as_ref(), Some(&report));
    assert_eq!(controller.control_mode(), ControlMode::Disabled);
    assert!(final_drain.worker_terminated);
    assert!(final_drain.is_idle());
    assert_eq!(final_drain.publication_failures, 0);
    assert_eq!(event.timestamp_ms, 7057);
    assert_eq!(event.session_id, "batch07-failed-stop");
    assert_eq!(event.operator_id, "shutdown-regression");
    assert_eq!(event.joint, "right_elbow_pitch");
    assert_eq!(event.action, "config_persist");
    assert!(matches!(
        audit.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    if storage == StorageQualification::Failed {
        assert_eq!(
            outcome.persist.status,
            PersistDrainStatus::CompletedWithFailures
        );
        assert_eq!(
            final_drain.status,
            PersistDrainStatus::CompletedWithFailures
        );
        assert_eq!(final_drain.failed_writes, 1);
        assert_eq!(final_drain.successful_writes, 0);
        assert!(!event.accepted);
        assert_eq!(event.persist_status, PersistStatus::Failed as i32);
        assert!(!event.reject_reason.is_empty());
        assert!(event.config_revision.is_empty());
        assert_eq!(
            std::fs::read(config_dir.join("control.yaml")).expect("preserved original config"),
            original
        );
    } else {
        assert_eq!(final_drain.status, PersistDrainStatus::Complete);
        assert_eq!(final_drain.successful_writes, 1);
        assert_eq!(final_drain.failed_writes, 0);
        assert!(event.accepted);
        assert_eq!(event.persist_status, PersistStatus::Durable as i32);
        assert!(!event.config_revision.is_empty());
        assert_eq!(
            load_control_config_from(&config_dir)
                .expect("actual installed draft")
                .control
                .joints["right_elbow_pitch"]
                .impedance
                .kp,
            57.0
        );
        assert_ne!(
            std::fs::read(config_dir.join("control.yaml")).expect("actual installed bytes"),
            original
        );
        if storage == StorageQualification::Unfinished {
            assert_eq!(outcome.persist.status, PersistDrainStatus::TimedOut);
            assert!(outcome.persist.in_flight);
            assert_eq!(outcome.persist.pending_requests, 0);
            assert!(!outcome.persist.worker_terminated);
            assert!(!outcome.persist_idle);
        } else {
            assert_eq!(outcome.persist.status, PersistDrainStatus::Complete);
            assert!(outcome.persist_idle);
        }
    }
}

#[test]
fn failed_stop_report_survives_a_successful_real_write() {
    qualify_failed_stop_with_actual_storage(StorageQualification::Durable);
}

#[test]
fn failed_stop_report_survives_a_failed_real_write() {
    qualify_failed_stop_with_actual_storage(StorageQualification::Failed);
}

#[test]
fn failed_stop_report_survives_an_unfinished_real_write() {
    qualify_failed_stop_with_actual_storage(StorageQualification::Unfinished);
}

#[test]
fn explicit_no_disable_exit_policy_inhibits_intent_and_drains_without_stop_writes() {
    use crate::limit_persist::PersistDrainStatus;

    let (temp, config_dir) = copied_owner_fixture();
    let witness = Arc::new(Mutex::new(Vec::new()));
    let mut controller = ControlLoop::from_repo(
        temp.path(),
        RecordingBus {
            witness: Arc::clone(&witness),
        },
        200,
        25,
    )
    .expect("ordinary unreferenced owner");
    // Retained local intent does not grant drive admission or send a write.
    controller
        .set_torque_cmd("right_elbow_pitch", 0.05)
        .expect("public local torque intent");
    assert_eq!(controller.torque_cmd("right_elbow_pitch"), 0.05);
    assert_eq!(controller.control_mode(), ControlMode::TorqueOnly);
    let generation = controller.supervisor().stop_generation();
    witness.lock().expect("clear startup output").clear();
    let bus = Arc::new(Bus::new(16));
    let mut audit = bus.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleaseOnDrop(release_tx.clone());
    let gate = Mutex::new(release_rx);
    let timed_out = Arc::new(AtomicBool::new(false));
    let hook_timed_out = Arc::clone(&timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&bus),
        Arc::new(AtomicBool::new(true)),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |_| {
                let _ = entered_tx.send(());
                if gate
                    .lock()
                    .expect("no-disable real writer gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    hook_timed_out.store(true, Ordering::SeqCst);
                }
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let mut control = load_control_config_from(&config_dir).expect("actual source draft");
    control
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("elbow")
        .impedance
        .kp = 59.0;
    queue
        .enqueue(PersistRequest {
            config_dir: config_dir.clone(),
            motors: None,
            control,
            timestamp_ms: 7059,
            session_id: "batch07-no-disable".into(),
            operator_id: "shutdown-regression".into(),
            joint: "right_elbow_pitch".into(),
            param: "impedance.kp".into(),
        })
        .expect("actual accepted write");
    let overlay = ActuatorOverlay::new(
        load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
        queue,
    );
    entered_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual writer is gated");
    let at_wait = Mutex::new(None);
    let before_wait = |controller: &ControlLoop<RecordingBus>| {
        *at_wait.lock().expect("no-disable wait snapshot") = Some((
            controller.control_mode(),
            controller.torque_cmd("right_elbow_pitch"),
            controller.supervisor().stop_generation(),
            witness.lock().expect("no-disable external trace").clone(),
        ));
        let _ = release_tx.send(());
    };
    let outcome = finish_owner_shutdown(
        &mut controller,
        &overlay,
        false,
        CLEANUP_BOUND,
        Some(&before_wait),
    );
    drop(overlay);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual no-disable worker returned");
    let bytes = audit
        .try_recv()
        .expect("actual matching durable completion");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual no-disable envelope");
    let event =
        ActionEvent::decode(envelope.payload.as_slice()).expect("actual no-disable terminal");
    let (mode_at_wait, torque_at_wait, generation_at_wait, trace_at_wait) = at_wait
        .into_inner()
        .expect("captured no-disable wait")
        .expect("actual pre-drain boundary reached");
    println!("explicit no-disable original outcome={outcome:?}, actual completion={event:?}, wait_mode={mode_at_wait:?}, wait_torque={torque_at_wait}, raw={trace_at_wait:?}");
    assert!(!timed_out.load(Ordering::SeqCst));
    assert!(
        matches!(outcome.stop, ExitStopOutcome::Skipped),
        "no-disable policy is explicit skipped stop, not accepted delivery"
    );
    assert_eq!(mode_at_wait, ControlMode::Disabled);
    assert_eq!(torque_at_wait, 0.0);
    assert_eq!(generation_at_wait, generation);
    assert!(trace_at_wait.is_empty());
    assert!(witness.lock().expect("final no-disable trace").is_empty());
    assert_eq!(controller.supervisor().stop_generation(), generation);
    assert!(controller
        .supervisor()
        .safety_snapshot()
        .last_stop
        .is_none());
    assert_eq!(outcome.persist.status, PersistDrainStatus::Complete);
    assert!(outcome.persist.worker_terminated);
    assert_eq!(outcome.persist.successful_writes, 1);
    assert_eq!(outcome.persist.failed_writes, 0);
    assert_eq!(event.timestamp_ms, 7059);
    assert_eq!(event.session_id, "batch07-no-disable");
    assert_eq!(event.operator_id, "shutdown-regression");
    assert_eq!(event.joint, "right_elbow_pitch");
    assert_eq!(event.action, "config_persist");
    assert!(event.accepted);
    assert_eq!(event.persist_status, PersistStatus::Durable as i32);
    assert!(!event.config_revision.is_empty());
    assert_eq!(
        load_control_config_from(&config_dir)
            .expect("actual installed no-disable draft")
            .control
            .joints["right_elbow_pitch"]
            .impedance
            .kp,
        59.0
    );
}

#[test]
fn active_shutdown_clears_gain_torque_and_wave_intent_before_storage_and_reenable() {
    use berthier::GainOverride;
    use davout::simulation::{InitialVirtualReference, SimulationBus};

    // Independent closed INITIAL virtual fixtures cover each retained intent.
    // This is software output qualification, never reference acquisition or a
    // Wave hardware sign-off. The master limits and sign-off stay untouched.
    for intent in ["gain", "torque", "wave"] {
        let (temp, config_dir) = copied_owner_fixture();
        let mut controller = ControlLoop::from_simulation(
            temp.path(),
            SimulationBus::default(),
            InitialVirtualReference::Joints(vec!["right_elbow_pitch".into()]),
            200,
            25,
        )
        .expect("closed virtual owner");
        controller
            .supervisor_mut()
            .enable_targets(&["right_elbow_pitch".into()])
            .expect("actual scoped Enable");
        controller
            .supervisor_mut()
            .bus_mut()
            .queue_frame(CanFrame {
                id: 0x028004fd,
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                extended: true,
            })
            .expect("raw current-session pose handed off after Enable");
        controller
            .supervisor_mut()
            .drain_feedback()
            .expect("actual current-session decoding");
        match intent {
            "gain" => {
                controller.set_control_mode(ControlMode::Impedance);
                controller
                    .apply_gain_override(
                        "right_elbow_pitch",
                        GainOverride {
                            kp: 83.0,
                            kd: 1.5,
                            ki: 0.0,
                            fc: 0.0,
                        },
                    )
                    .expect("actual runtime gains");
                assert_eq!(
                    controller
                        .gain_override("right_elbow_pitch")
                        .expect("retained gains")
                        .kp,
                    83.0
                );
            }
            "torque" => {
                controller
                    .set_torque_cmd("right_elbow_pitch", 0.05)
                    .expect("actual retained torque");
                assert_eq!(controller.torque_cmd("right_elbow_pitch"), 0.05);
            }
            "wave" => {
                controller
                    .start_position_wave("right_elbow_pitch", 0.02, 0.04, 1, 0.5)
                    .expect("actual small virtual planner intent within unchanged limits");
                assert!(controller.position_wave_active());
                assert!(controller.position_setpoints().is_some());
            }
            _ => unreachable!("literal intent cases"),
        }
        controller.supervisor_mut().bus_mut().clear_trace();
        controller.tick(None).expect("actual Active intent tick");
        assert_eq!(controller.supervisor().mode(), OperationalMode::Active);
        let before_shutdown = controller.supervisor().bus().transmissions();
        assert!(
            before_shutdown
                .iter()
                .any(|tx| tx.address == Some(MotorAddress::new("can0", 4))
                    && tx.frame.id >> 24 == 1
                    && (tx.frame.data[4..8] != [0; 4] || (tx.frame.id >> 8) & 0xffff != 0x7fff)),
            "each real retained intent has reachable non-neutral MIT output"
        );
        controller.supervisor_mut().bus_mut().clear_trace();
        let bus = Arc::new(Bus::new(16));
        let (exited_tx, exited_rx) = mpsc::channel();
        let queue = ConfigPersistQueue::spawn_with_test_hooks(
            Arc::clone(&bus),
            Arc::new(AtomicBool::new(true)),
            temp.path().to_path_buf(),
            PersistTestHooks {
                on_worker_exit: Some(Arc::new(move || {
                    let _ = exited_tx.send(());
                })),
                ..PersistTestHooks::default()
            },
        );
        let overlay = ActuatorOverlay::new(
            load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
            queue,
        );
        let at_wait = Mutex::new(None);
        let before_wait = |controller: &ControlLoop<SimulationBus>| {
            *at_wait
                .lock()
                .expect("Active shutdown pre-storage snapshot") = Some((
                controller.control_mode(),
                controller.gain_override("right_elbow_pitch").cloned(),
                controller.torque_cmd("right_elbow_pitch"),
                controller.position_wave_active(),
                controller
                    .position_setpoints()
                    .map(|setpoints| setpoints.to_vec()),
                controller
                    .supervisor()
                    .bus()
                    .transmissions()
                    .iter()
                    .map(|tx| AttemptedFrame {
                        address: tx.address.clone(),
                        frame: tx.frame.clone(),
                    })
                    .collect::<Vec<_>>(),
            ));
        };
        let outcome = finish_owner_shutdown(
            &mut controller,
            &overlay,
            true,
            CLEANUP_BOUND,
            Some(&before_wait),
        );
        drop(overlay);
        exited_rx
            .recv_timeout(CLEANUP_BOUND)
            .expect("actual Active owner's worker returned");
        let (mode, gains, torque, wave, setpoints, raw) = at_wait
            .into_inner()
            .expect("captured Active pre-storage observation")
            .expect("actual boundary reached");
        println!("actual Active intent={intent}, pre-storage=({mode:?},{gains:?},{torque},{wave},{setpoints:?}), stops={raw:?}, outcome={outcome:?}");
        assert!(outcome.persist.worker_terminated);
        assert!(matches!(
            outcome.stop,
            ExitStopOutcome::Attempted { result: Ok(()), .. }
        ));
        assert_eq!(mode, ControlMode::Disabled);
        assert!(gains.is_none());
        assert_eq!(torque, 0.0);
        assert!(!wave);
        assert!(setpoints.is_none());
        assert_eq!(stop_trace(&raw), literal_stop_trace());
        controller.supervisor_mut().bus_mut().clear_trace();
        // A direct library tick tests cleared intent, separately from the
        // installed outer-loop Quit proof that prevents later runtime ticks.
        controller.tick(None).expect("direct stopped-library tick");
        assert!(controller
            .supervisor()
            .bus()
            .frames()
            .iter()
            .all(|frame| frame.id >> 24 != 1));
        controller
            .supervisor_mut()
            .enable_targets(&["right_elbow_pitch".into()])
            .expect("new explicit virtual Enable must not restore old intent");
        controller
            .supervisor_mut()
            .bus_mut()
            .queue_frame(CanFrame {
                id: 0x028004fd,
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                extended: true,
            })
            .expect("real raw pose for new virtual enable session");
        controller
            .tick(None)
            .expect("explicit reenable with Disabled control intent");
        let neutral = controller
            .supervisor()
            .bus()
            .frames()
            .iter()
            .filter(|frame| frame.id >> 24 == 1)
            .collect::<Vec<_>>();
        assert_eq!(neutral.len(), 1);
        assert_eq!(neutral[0].id, 0x017fff04);
        // Disabled keepalive may preserve the decoded, quantized position.
        // The independent inert contract is zero velocity/gains/torque.
        assert_eq!(&neutral[0].data[2..4], &[0x7f, 0xff]);
        assert_eq!(&neutral[0].data[4..8], &[0, 0, 0, 0]);
        assert_eq!(controller.control_mode(), ControlMode::Disabled);
        assert!(!controller.position_wave_active());
        assert_eq!(controller.torque_cmd("right_elbow_pitch"), 0.0);
    }
}

struct OwnerFlagBus {
    recording: RecordingBus,
    shutdown: Arc<AtomicBool>,
    trigger_on_stop: usize,
    stop_starts: usize,
    flag_triggers: usize,
}

impl CanBus for OwnerFlagBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.recording.send_frame(frame)
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.recording.send_frame_to(address, frame)?;
        if address == &MotorAddress::new("can0", 1) && frame.id == 0x1200fd01 {
            self.stop_starts += 1;
            if self.stop_starts == self.trigger_on_stop {
                self.flag_triggers += 1;
                self.shutdown.store(true, Ordering::SeqCst);
            }
        }
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        self.recording.recv_one_nonblocking()
    }
}

impl MotorBus for OwnerFlagBus {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnerFlagCase {
    ChappeSecondStopNeighbor,
    StdinFirstStop,
    ChappeFirstStop,
}

#[derive(Debug)]
struct OwnerFlagObservation {
    mode: ControlMode,
    ticks: u64,
    stop_generation: u64,
    flag_triggers: usize,
    stop_starts: usize,
    report: Option<StopReport>,
    frames: Vec<AttemptedFrame>,
    retained_stdin: Vec<&'static str>,
    retained_enable: Vec<armee_proto::EnableRequest>,
}

fn run_actual_owner_flag_case(case: OwnerFlagCase) -> OwnerFlagObservation {
    use crate::{run_control_loop, ControlLoopRuntime, PiCommand};
    use armee_proto::EnableRequest;

    let (temp, config_dir) = copied_owner_fixture();
    let shutdown = Arc::new(AtomicBool::new(false));
    let witness = Arc::new(Mutex::new(Vec::new()));
    let mut controller = ControlLoop::from_repo(
        temp.path(),
        OwnerFlagBus {
            recording: RecordingBus {
                witness: Arc::clone(&witness),
            },
            shutdown: Arc::clone(&shutdown),
            trigger_on_stop: if case == OwnerFlagCase::ChappeSecondStopNeighbor {
                2
            } else {
                1
            },
            stop_starts: 0,
            flag_triggers: 0,
        },
        200,
        25,
    )
    .expect("ordinary unreferenced owner with data-recording flag fixture");
    for joint in controller.joint_names() {
        assert_eq!(
            controller.supervisor().joint_homing_state(joint),
            JointHomingState::Unhomed
        );
    }
    assert_eq!(controller.supervisor().mode(), OperationalMode::Disabled);
    controller.set_control_mode(ControlMode::GravityComp);
    witness
        .lock()
        .expect("clear checked startup output")
        .clear();
    let chappe = Arc::new(Bus::new(16));
    let (exited_tx, exited_rx) = mpsc::channel();
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        Arc::clone(&shutdown),
        temp.path().to_path_buf(),
        PersistTestHooks {
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let mut overlay = ActuatorOverlay::new(
        load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
        queue,
    );
    let mut enable_rx = chappe.subscribe("robot/enable");
    let mut homing_rx = chappe.subscribe("robot/homing");
    let mut set_zero_rx = chappe.subscribe("robot/set_zero");
    let mut lease_rx = chappe.subscribe("robot/active_reporting_lease");
    let mut status_poll_rx = chappe.subscribe("robot/motor_status_poll");
    let mut testing_cmd_rx = chappe.subscribe("robot/testing/mit_command_batch");
    let mut actuator_rx = chappe.subscribe(crate::overlay::TOPIC_ACTUATOR_COMMAND);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    if case == OwnerFlagCase::StdinFirstStop {
        cmd_tx
            .send(PiCommand::Disable)
            .expect("real admitted stdin stop");
        cmd_tx
            .send(PiCommand::ImpedanceOn)
            .expect("later local intent command");
        cmd_tx
            .send(PiCommand::Quit)
            .expect("later Quit remains a separate command");
    } else {
        for (timestamp_ms, operator_id) in
            [(7101, "batch07-flag-first"), (7102, "batch07-flag-second")]
        {
            chappe
                .publish(
                    "robot/enable",
                    "owner-flag-regression",
                    "marengo.v1.EnableRequest",
                    &EnableRequest {
                        timestamp_ms,
                        operator_id: operator_id.into(),
                        enable: false,
                    },
                )
                .expect("actual valid Chappe disable request");
        }
    }

    // A timeout is cleanup only, never the regression oracle. If a changed
    // encoder/dispatch misses the literal trigger, terminate rather than hang;
    // require this fallback NOT to fire after actual runtime/worker cleanup.
    let (finished_tx, finished_rx) = mpsc::channel();
    let fallback = Arc::new(AtomicBool::new(false));
    let watchdog_fallback = Arc::clone(&fallback);
    let watchdog_shutdown = Arc::clone(&shutdown);
    let watchdog = std::thread::spawn(move || {
        if finished_rx.recv_timeout(CLEANUP_BOUND).is_err() {
            watchdog_fallback.store(true, Ordering::SeqCst);
            watchdog_shutdown.store(true, Ordering::SeqCst);
        }
    });
    let mut runtime = ControlLoopRuntime {
        config_dir: &config_dir,
        chappe_state_hz: 25,
        chappe: &chappe,
        cmd_rx: &cmd_rx,
        enable_rx: &mut enable_rx,
        homing_rx: &mut homing_rx,
        set_zero_rx: &mut set_zero_rx,
        lease_rx: &mut lease_rx,
        status_poll_rx: &mut status_poll_rx,
        testing_cmd_rx: &mut testing_cmd_rx,
        actuator_rx: &mut actuator_rx,
        actuator_overlay: &mut overlay,
        shutdown: &shutdown,
        motion: crate::motion_owner::MotionLease::new(crate::motion_owner::CommandSource::Stdin),
    };
    run_control_loop(&mut controller, &mut runtime);
    let _ = finished_tx.send(());
    watchdog
        .join()
        .expect("bounded actual-return watchdog cleanup");

    let retained_stdin = cmd_rx
        .try_iter()
        .map(|command| match command {
            PiCommand::ImpedanceOn => "ImpedanceOn",
            PiCommand::Quit => "Quit",
            PiCommand::Disable => "Disable",
            _ => "unexpected command",
        })
        .collect();
    let mut retained_enable = Vec::new();
    while let Ok(bytes) = enable_rx.try_recv() {
        let envelope = Envelope::decode(bytes.as_slice()).expect("retained actual Chappe envelope");
        retained_enable.push(
            EnableRequest::decode(envelope.payload.as_slice())
                .expect("retained actual enable request"),
        );
    }
    let observation = OwnerFlagObservation {
        mode: controller.control_mode(),
        ticks: controller.tick_count(),
        stop_generation: controller.supervisor().stop_generation(),
        flag_triggers: controller.supervisor().bus().flag_triggers,
        stop_starts: controller.supervisor().bus().stop_starts,
        report: controller.supervisor().safety_snapshot().last_stop,
        frames: witness
            .lock()
            .expect("complete pre-cleanup raw attempts")
            .clone(),
        retained_stdin,
        retained_enable,
    };
    // A skipped additional stop avoids rewriting the admitted stop generation;
    // the actual installed lifecycle still inhibits intent and closes/joins the
    // real worker. All initiating dispatch evidence was captured beforehand.
    let cleanup = finish_owner_shutdown(&mut controller, &overlay, false, CLEANUP_BOUND, None);
    drop(overlay);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual owner-flag worker returned");
    println!(
        "actual owner-flag case={case:?}, before cleanup={observation:?}, cleanup={cleanup:?}"
    );
    assert!(
        !fallback.load(Ordering::SeqCst),
        "literal transport flag, not timeout fallback, ended the actual loop"
    );
    assert!(shutdown.load(Ordering::SeqCst));
    assert_eq!(observation.flag_triggers, 1);
    assert!(cleanup.persist.worker_terminated);
    assert!(cleanup.persist.is_idle());
    assert!(matches!(cleanup.stop, ExitStopOutcome::Skipped));
    assert_eq!(controller.control_mode(), ControlMode::Disabled);
    assert_eq!(
        controller.supervisor().stop_generation(),
        observation.stop_generation
    );
    let report = observation
        .report
        .as_ref()
        .expect("actual admitted stop finished");
    assert_eq!(report.attempts.len(), 15);
    assert_eq!(report.failed_writes(), 0);
    assert!(observation
        .frames
        .iter()
        .all(|tx| matches!(tx.frame.id >> 24, 0x12 | 0x01 | 0x04 | 0x18)));
    observation
}

#[test]
fn observed_owner_shutdown_preserves_later_stdin_and_chappe_commands() {
    use armee_proto::EnableRequest;

    let neighbor = run_actual_owner_flag_case(OwnerFlagCase::ChappeSecondStopNeighbor);
    assert_eq!(neighbor.mode, ControlMode::Disabled);
    assert_eq!(neighbor.stop_generation, 2);
    assert_eq!(neighbor.stop_starts, 2);
    assert!(neighbor.retained_enable.is_empty());
    assert!(neighbor.retained_stdin.is_empty());
    assert!(
        neighbor.ticks <= 1,
        "the old neighbor may finish its incidental tick"
    );
    let two_complete_stops = [literal_stop_trace(), literal_stop_trace()].concat();
    assert_eq!(
        stop_trace(&neighbor.frames),
        two_complete_stops,
        "both actual valid Chappe requests reach all thirty addressed attempts"
    );

    // Execute and clean up both critical variants before the one decisive
    // assertion, so an old stdin failure cannot hide the Chappe observation.
    let stdin = run_actual_owner_flag_case(OwnerFlagCase::StdinFirstStop);
    let chappe = run_actual_owner_flag_case(OwnerFlagCase::ChappeFirstStop);
    assert_eq!(
        stop_trace(&stdin.frames),
        literal_stop_trace(),
        "the already admitted stdin stop must finish every address despite the flag"
    );
    let expected_second = EnableRequest {
        timestamp_ms: 7102,
        operator_id: "batch07-flag-second".into(),
        enable: false,
    };
    assert!(
        stdin.mode == ControlMode::Disabled
            && stdin.ticks == 0
            && stdin.stop_generation == 1
            && stdin.stop_starts == 1
            && stdin.retained_stdin == ["ImpedanceOn", "Quit"]
            && chappe.mode == ControlMode::Disabled
            && chappe.ticks == 0
            && chappe.stop_generation == 1
            && chappe.stop_starts == 1
            && chappe.retained_enable == [expected_second],
        "observed owner shutdown must stop before any later queued command or tick"
    );
    assert_eq!(
        stop_trace(&chappe.frames),
        literal_stop_trace(),
        "only the first admitted Chappe stop completes after shutdown is observed"
    );
}
