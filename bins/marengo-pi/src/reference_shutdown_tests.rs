//! Installed Pi shutdown composition with the real virtual owner and writer.
//! Candidate-interface conformance; no original-binary or physical stop claim.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{ActionEvent, Envelope, PersistStatus};
use berthier::{ControlLoop, ControlMode, GainOverride};
use chappe::Bus;
use davout::simulation::{
    InitialVirtualReference, ReferenceProofMode, ReferenceReplyRule, SimulationBus,
    SimulationReceive, SimulationTransmission, TxMatcher, TxOccurrence, TxRule,
};
use davout::{
    DavoutError, OperationalMode, ReferenceCancelReason, ReferenceCause, ReferenceCommit,
    ReferenceHandle, ReferencePhase, ReferenceRequest, ReferenceSnapshot, StopAction, StopReport,
};
use marengo_config::{load_command_joint_allowlist_from, load_control_config_from};
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};

use crate::limit_persist::{
    ConfigPersistQueue, PersistDrainStatus, PersistRequest, PersistTestHooks,
};
use crate::overlay::{ActuatorOverlay, TOPIC_AUDIT_ACTION};
use crate::{finish_owner_shutdown, ExitStopOutcome, ShutdownOutcome};

const TARGET: &str = "right_elbow_pitch";
const CLEANUP_BOUND: Duration = Duration::from_secs(2);
const SESSION: &str = "reference-shutdown-real-write";

struct ReleaseOnDrop(Sender<()>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Wire {
    address: Option<MotorAddress>,
    frame: CanFrame,
    delivered: bool,
}

fn captured(trace: &[SimulationTransmission]) -> Vec<Wire> {
    trace
        .iter()
        .map(|tx| Wire {
            address: tx.address.clone(),
            frame: tx.frame.clone(),
            delivered: tx.delivered,
        })
        .collect()
}

fn wire(device: u8, id: u32, data: [u8; 8]) -> Wire {
    Wire {
        address: Some(MotorAddress::new("can0", device)),
        frame: CanFrame {
            id,
            data,
            extended: true,
        },
        delivered: true,
    }
}

/// Literal installed routes and payloads, independent of stop/encoder helpers.
fn all_stop(failed_target_write: bool) -> Vec<Wire> {
    let mut trace: Vec<_> = [
        (1, [0x1200fd01, 0x017fff01, 0x0400fd01]),
        (2, [0x1200fd02, 0x017fff02, 0x0400fd02]),
        (3, [0x1200fd03, 0x017fff03, 0x0400fd03]),
        (4, [0x1200fd04, 0x017fff04, 0x0400fd04]),
        (5, [0x1200fd05, 0x017fff05, 0x0400fd05]),
    ]
    .into_iter()
    .flat_map(|(device, ids)| {
        ids.into_iter()
            .zip([
                [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
                [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
                [0; 8],
            ])
            .map(move |(id, data)| wire(device, id, data))
    })
    .collect();
    if failed_target_write {
        trace[9].delivered = false;
    }
    trace
}

fn target_run() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x028004fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

fn copied_fixture() -> (tempfile::TempDir, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().expect("exclusive copied shutdown resource");
    let config = temp.path().join("config");
    std::fs::create_dir(&config).expect("copied config directory");
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        std::fs::copy(root.join("config").join(name), config.join(name))
            .expect("copy immutable master input");
    }
    let urdf = temp.path().join("assets/urdf");
    std::fs::create_dir_all(&urdf).expect("copied model directory");
    std::fs::copy(
        root.join("assets/urdf/marengo.urdf"),
        urdf.join("marengo.urdf"),
    )
    .expect("copy immutable master model");
    // Exact acquisition trace is separate from the applied-reporting qualification.
    crate::test_support::disable_copied_diagnostics(temp.path());
    (temp, config)
}

#[derive(Debug)]
struct WaitObservation {
    trace: Vec<Wire>,
    reference: ReferenceSnapshot,
    stop: Option<StopReport>,
    controller_mode: ControlMode,
    supervisor_mode: OperationalMode,
    gains: Option<GainOverride>,
    torque: f64,
    wave: bool,
    has_position_intent: bool,
    busy: bool,
    writer_blocked: bool,
    disk: Vec<u8>,
}

struct Case {
    disable_on_exit: bool,
    failed_target_write: bool,
    handle: ReferenceHandle,
    advances: Vec<ReferenceSnapshot>,
    before_trace: Vec<Wire>,
    before_gains: Option<GainOverride>,
    before_generation: u64,
    rule_counts: (usize, usize, usize),
    at_wait: WaitObservation,
    outcome: ShutdownOutcome,
    final_trace: Vec<Wire>,
    final_reference: ReferenceSnapshot,
    original: Vec<u8>,
    written: Vec<u8>,
    written_kp: f64,
    action: ActionEvent,
    entered_session: String,
    gate_timed_out: bool,
}

fn run_case(disable_on_exit: bool, failed_target_write: bool) -> Case {
    let (temp, config_dir) = copied_fixture();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("pre-write bytes");
    let mut controller = ControlLoop::from_simulation_with_calibration_record_path(
        temp.path(),
        SimulationBus::default(),
        temp.path().join("history.yaml"),
        InitialVirtualReference::Unreferenced,
        200,
        25,
    )
    .expect("real controller with isolated history and closed acquisition backend");
    // Public inactive controller intent survives direct Supervisor acquisition;
    // no Active grant, motion tick or arbitrary transport is used to seed it.
    controller.set_control_mode(ControlMode::Impedance);
    controller
        .apply_gain_override(
            TARGET,
            GainOverride {
                kp: 41.0,
                kd: 1.0,
                ki: 0.0,
                fc: 0.0,
            },
        )
        .expect("real finite retained controller gains");
    let owner = controller.supervisor_mut();
    let enable = owner
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                device_id: Some(4),
                interface: Some("can0".into()),
            },
            occurrence: TxOccurrence::Nth(1),
            receive: vec![SimulationReceive::Received(target_run())],
            send_error: None,
        })
        .expect("actual addressed Enable triggers raw Run input");
    let proof = owner
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress::new("can0", 4),
            occurrence: TxOccurrence::Nth(1),
            frame: target_run(),
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("actual SetZero queues sealed raw reply");
    owner.bus_mut().clear_trace();
    let handle = owner
        .begin_reference(ReferenceRequest {
            stamp: owner
                .reference_snapshot()
                .next_stamp
                .expect("owner-issued stamp"),
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("real reservation without normal motion permission");
    let advances = (0..5)
        .map(|_| {
            owner
                .advance_reference(&handle)
                .expect("bounded public owner advance")
        })
        .collect();
    let before_trace = captured(owner.bus().transmissions());
    let before_generation = owner.stop_generation();
    if failed_target_write {
        owner
            .bus_mut()
            .add_tx_rule(TxRule {
                matcher: TxMatcher {
                    communication_type: Some(0x12),
                    device_id: Some(4),
                    interface: Some("can0".into()),
                },
                occurrence: TxOccurrence::Nth(1),
                receive: Vec::new(),
                send_error: Some("uncertain mandatory target zero-speed delivery".into()),
            })
            .expect("finite real cleanup failure installed after baseline stop");
    }
    let rule_counts = (
        owner.bus().rule_trigger_count(enable),
        owner.bus_mut().reference_rule_trigger_count(proof),
        owner.bus_mut().reference_rule_pop_count(proof),
    );
    owner.bus_mut().clear_trace();
    let before_gains = controller.gain_override(TARGET).cloned();

    let chappe = Arc::new(Bus::new(16));
    let mut actions = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let shutdown = Arc::new(AtomicBool::new(false));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleaseOnDrop(release_tx.clone());
    let gate_rx = Mutex::new(release_rx);
    let blocked = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let blocked_in_hook = Arc::clone(&blocked);
    let timed_out_in_hook = Arc::clone(&timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        Arc::clone(&shutdown),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |request| {
                blocked_in_hook.store(true, Ordering::SeqCst);
                let _ = entered_tx.send(request.session_id.clone());
                if gate_rx
                    .lock()
                    .expect("single real I/O gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    timed_out_in_hook.store(true, Ordering::SeqCst);
                }
                blocked_in_hook.store(false, Ordering::SeqCst);
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let mut draft = load_control_config_from(&config_dir).expect("actual source draft");
    draft
        .control
        .joints
        .get_mut(TARGET)
        .expect("configured elbow")
        .impedance
        .kp = 51.0;
    queue
        .enqueue(PersistRequest {
            config_dir: config_dir.clone(),
            motors: None,
            control: draft,
            timestamp_ms: 10042,
            session_id: SESSION.into(),
            operator_id: "reference-shutdown-conformance".into(),
            joint: TARGET.into(),
            param: "impedance.kp".into(),
        })
        .expect("accept real write behind installed owner");
    let overlay = ActuatorOverlay::new(
        load_command_joint_allowlist_from(&config_dir).expect("copied allowlist"),
        queue,
    );
    let entered_session = entered_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual writer reached I/O boundary");
    shutdown.store(true, Ordering::SeqCst);
    let observation = Mutex::new(None);
    let before_wait = |controller: &ControlLoop<SimulationBus>| {
        *observation.lock().expect("wait capture") = Some(WaitObservation {
            trace: captured(controller.supervisor().bus().transmissions()),
            reference: controller.supervisor().reference_snapshot(),
            stop: controller.supervisor().safety_snapshot().last_stop,
            controller_mode: controller.control_mode(),
            supervisor_mode: controller.supervisor().mode(),
            gains: controller.gain_override(TARGET).cloned(),
            torque: controller.torque_cmd(TARGET),
            wave: controller.position_wave_active(),
            has_position_intent: controller.position_setpoints().is_some(),
            busy: controller.supervisor().reference_busy(),
            writer_blocked: blocked.load(Ordering::SeqCst),
            disk: std::fs::read(config_dir.join("control.yaml"))
                .expect("actual disk before released write"),
        });
        // Unconditional release precedes all decisive assertions, even on failure.
        let _ = release_tx.send(());
    };
    let outcome = finish_owner_shutdown(
        &mut controller,
        &overlay,
        disable_on_exit,
        CLEANUP_BOUND,
        Some(&before_wait),
    );
    drop(overlay);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual worker returned after write/publication");
    let at_wait = observation
        .into_inner()
        .expect("wait observation")
        .expect("actual installed pre-persistence seam was reached");
    let final_trace = captured(controller.supervisor().bus().transmissions());
    let final_reference = controller.supervisor().reference_snapshot();
    let written = std::fs::read(config_dir.join("control.yaml")).expect("actual installed bytes");
    let written_kp = load_control_config_from(&config_dir)
        .expect("reload actual write")
        .control
        .joints[TARGET]
        .impedance
        .kp;
    let bytes = actions.try_recv().expect("matching actual terminal audit");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual audit envelope");
    let action = ActionEvent::decode(envelope.payload.as_slice()).expect("actual audit action");
    let gate_timed_out = timed_out.load(Ordering::SeqCst);
    drop(controller);
    temp.close()
        .expect("exclusive fixture removed after real worker/controller cleanup");
    Case {
        disable_on_exit,
        failed_target_write,
        handle,
        advances,
        before_trace,
        before_gains,
        before_generation,
        rule_counts,
        at_wait,
        outcome,
        final_trace,
        final_reference,
        original,
        written,
        written_kp,
        action,
        entered_session,
        gate_timed_out,
    }
}

#[test]
fn armed_reference_shutdown_stops_before_real_storage_under_both_exit_policies() {
    // Run and clean every neighbor/failure fixture before classifying outcomes.
    let cases: Vec<_> = [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .map(|(exit, failure)| run_case(exit, failure))
        .collect();
    for case in cases {
        assert_eq!(
            case.advances
                .iter()
                .map(|step| step.phase)
                .collect::<Vec<_>>(),
            [
                ReferencePhase::DrainOld,
                ReferencePhase::ArmTarget,
                ReferencePhase::DrainPostArm,
                ReferencePhase::SetZero,
                ReferencePhase::AwaitEvidence,
            ]
        );
        assert!(case.advances[4].reference_armed);
        assert!(!case.advances[4].usable_reference);
        let mut expected_before = all_stop(false);
        expected_before.push(wire(4, 0x0300fd04, [0; 8]));
        expected_before.push(wire(4, 0x0600fd04, [1, 0, 0, 0, 0, 0, 0, 0]));
        assert_eq!(
            case.before_trace, expected_before,
            "actual armed/SetZero reachability"
        );
        assert_eq!(case.before_generation, 1);
        assert_eq!(
            case.rule_counts,
            (1, 1, 0),
            "real Enable/SetZero, pending unpopped proof"
        );
        assert_eq!(
            case.before_gains.as_ref().expect("retained real gains").kp,
            41.0
        );
        assert!(
            !case.gate_timed_out,
            "real writer gate was explicitly released"
        );
        assert_eq!(case.entered_session, SESSION);
        assert!(case.outcome.persist_idle);
        assert_eq!(case.outcome.persist.status, PersistDrainStatus::Complete);
        assert_eq!(case.outcome.persist.successful_writes, 1);
        assert_eq!(case.outcome.persist.failed_writes, 0);
        assert_eq!(case.outcome.persist.publication_failures, 0);
        assert_eq!(case.outcome.persist.pending_requests, 0);
        assert!(!case.outcome.persist.in_flight);
        assert!(case.outcome.persist.worker_terminated);
        assert_eq!(case.written_kp, 51.0);
        assert_ne!(case.written, case.original);
        assert_eq!(case.action.timestamp_ms, 10042);
        assert_eq!(case.action.session_id, SESSION);
        assert_eq!(case.action.operator_id, "reference-shutdown-conformance");
        assert_eq!(case.action.joint, TARGET);
        assert_eq!(case.action.action, "config_persist");
        assert!(case.action.accepted);
        assert_eq!(case.action.persist_status, PersistStatus::Durable as i32);
        assert!(!case.action.config_revision.is_empty());

        let mandatory = case
            .outcome
            .mandatory_reference
            .as_ref()
            .expect("mandatory reference receipt");
        assert_eq!(mandatory.handle, case.handle);
        assert_eq!(mandatory.joint, TARGET);
        assert_eq!(
            mandatory.cause,
            ReferenceCause::Cancelled(ReferenceCancelReason::Shutdown)
        );
        assert_eq!(mandatory.commit, ReferenceCommit::Unavailable);
        assert!(!mandatory.usable_reference);
        assert!(mandatory.reporting.is_empty());
        assert_eq!(mandatory.stop.generation, 2);
        assert_eq!(mandatory.stop.attempts.len(), 15);
        assert_eq!(
            mandatory.stop.failed_writes(),
            usize::from(case.failed_target_write)
        );
        let routes = [1, 2, 3, 4, 5].into_iter().flat_map(|device| {
            [
                StopAction::ZeroSpeed,
                StopAction::NeutralMit,
                StopAction::Disable,
            ]
            .into_iter()
            .map(move |action| (MotorAddress::new("can0", device), action))
        });
        for (index, (attempt, (address, action))) in
            mandatory.stop.attempts.iter().zip(routes).enumerate()
        {
            assert_eq!(attempt.address, address);
            assert_eq!(attempt.action, action);
            assert_eq!(
                attempt.error.is_some(),
                case.failed_target_write && index == 9
            );
        }
        assert_eq!(case.final_reference.terminal.as_ref(), Some(mandatory));
        match &case.outcome.stop {
            ExitStopOutcome::Skipped => assert!(!case.disable_on_exit),
            ExitStopOutcome::Attempted { result, report } => {
                assert!(case.disable_on_exit);
                assert_eq!(report.as_ref(), Some(&mandatory.stop));
                if case.failed_target_write {
                    assert!(matches!(
                        result,
                        Err(DavoutError::StopDelivery { failed_writes: 1 })
                    ));
                } else {
                    assert!(result.is_ok());
                }
            }
        }
        println!(
            "actual armed reference exit={} failure={} wait={:?} outcome={:?}",
            case.disable_on_exit, case.failed_target_write, case.at_wait, case.outcome
        );
        assert!(case.at_wait.writer_blocked);
        assert_eq!(case.at_wait.disk, case.original);
        assert_eq!(case.at_wait.controller_mode, ControlMode::Disabled);
        assert_eq!(case.at_wait.supervisor_mode, OperationalMode::Disabled);
        assert!(case.at_wait.gains.is_none());
        assert_eq!(case.at_wait.torque, 0.0);
        assert!(!case.at_wait.wave);
        assert!(!case.at_wait.has_position_intent);
        assert!(!case.at_wait.busy);
        assert!(!case.at_wait.reference.reference_armed);
        assert!(!case.at_wait.reference.usable_reference);
        assert_eq!(case.at_wait.reference.terminal.as_ref(), Some(mandatory));
        assert_eq!(case.at_wait.stop.as_ref(), Some(&mandatory.stop));
        assert_eq!(case.at_wait.trace, all_stop(case.failed_target_write),
            "mandatory fifteen reference cleanup attempts must precede persistence waiting under either exit policy");
        assert_eq!(
            case.final_trace, case.at_wait.trace,
            "ordinary exit policy must reuse mandatory cleanup without a second stop sequence"
        );
    }
}
