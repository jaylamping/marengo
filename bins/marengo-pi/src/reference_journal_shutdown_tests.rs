//! Both actual storage workers behind the real Pi owner shutdown composition.
#![allow(clippy::expect_used)]
use crate::limit_persist::{
    ConfigPersistQueue, PersistDrainStatus, PersistRequest, PersistTestHooks,
};
use crate::overlay::ActuatorOverlay;
use crate::{finish_owner_shutdown, ExitStopOutcome};
use berthier::ControlLoop;
use chappe::Bus;
use davout::simulation::{
    JournalPausePoint, JournalTestPause, ReferenceProofMode, ReferenceReplyRule, SimulationBus,
    SimulationReceive, TxMatcher, TxOccurrence, TxRule,
};
use davout::{
    ReferenceAudit, ReferenceCancelReason, ReferenceCause, ReferenceCommitHandle,
    ReferenceCommitPhase, ReferenceJournalResult, ReferenceRequest,
};
use marengo_config::{load_command_joint_allowlist_from, load_control_config_from};
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
mod support {
    use std::path::{Path, PathBuf};
    pub struct FixtureTree {
        temp: tempfile::TempDir,
    }
    impl FixtureTree {
        pub fn new(_label: &str, root: &Path) -> Self {
            let temp = tempfile::tempdir().expect("exclusive copied owner fixture");
            std::fs::create_dir(temp.path().join("config")).expect("copied config directory");
            for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
                std::fs::copy(
                    root.join("config").join(name),
                    temp.path().join("config").join(name),
                )
                .expect("immutable master input");
            }
            let model = temp.path().join("assets/urdf");
            std::fs::create_dir_all(&model).expect("copied model directory");
            std::fs::copy(
                root.join("assets/urdf/marengo.urdf"),
                model.join("marengo.urdf"),
            )
            .expect("immutable model input");
            Self { temp }
        }
        pub fn path(&self) -> PathBuf {
            self.temp.path().to_owned()
        }
    }
}
const TARGET: &str = "right_elbow_pitch";
const WAIT: Duration = Duration::from_secs(5);

struct Owner<'a> {
    ctrl: ControlLoop<SimulationBus>,
    pause: &'a JournalTestPause,
    overlay: &'a ActuatorOverlay,
    release: Sender<()>,
}
impl std::ops::Deref for Owner<'_> {
    type Target = ControlLoop<SimulationBus>;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}
impl std::ops::DerefMut for Owner<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}
impl Drop for Owner<'_> {
    fn drop(&mut self) {
        let _ = self.release.send(());
        self.pause.release();
        self.ctrl.inhibit_motion_for_shutdown();
        self.ctrl.supervisor_mut().cancel_reference_for_shutdown();
        self.overlay.close_persist_and_drain(WAIT);
        self.ctrl
            .supervisor_mut()
            .drain_reference_journal_until(Instant::now() + WAIT);
    }
}
fn request(ctrl: &mut ControlLoop<SimulationBus>) -> davout::ReferenceHandle {
    let stamp = ctrl
        .supervisor()
        .reference_snapshot()
        .next_stamp
        .expect("actual owner stamp");
    ctrl.supervisor_mut()
        .begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("actual acquisition")
}
fn install(ctrl: &mut ControlLoop<SimulationBus>) -> ReferenceCommitHandle {
    let frame = ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0280_04fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    );
    ctrl.supervisor_mut()
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                device_id: Some(4),
                interface: Some("can0".into()),
            },
            occurrence: TxOccurrence::Every,
            receive: vec![SimulationReceive::Received(frame.clone())],
            send_error: None,
        })
        .expect("Enable reply");
    ctrl.supervisor_mut()
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress::new("can0", 4),
            occurrence: TxOccurrence::Every,
            frame,
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("actual SetZero/pop rule");
    let acquisition = request(ctrl);
    for _ in 0..6 {
        ctrl.tick(None).expect("real reference owner tick");
    }
    assert_eq!(
        ctrl.supervisor()
            .reference_snapshot()
            .terminal
            .expect("real terminal")
            .cause,
        ReferenceCause::EvidenceStaged
    );
    ctrl.supervisor_mut()
        .begin_reference_commit(
            &acquisition,
            ReferenceAudit {
                operator: "shutdown-fixture".into(),
                session: "shutdown-history".into(),
            },
        )
        .expect("actual job")
}

#[test]
fn both_writers_share_one_shutdown_budget_after_the_required_stop() {
    for (disable_on_exit, armed) in [(false, false), (true, false), (false, true)] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tree = support::FixtureTree::new("journal-shutdown", &root);
        let config = tree.path().join("config");
        crate::test_support::disable_copied_diagnostics(&tree.path());
        let pause = JournalTestPause::new(JournalPausePoint::AfterCommit, 1)
            .expect("actual committed history gate");
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let released = Mutex::new(release_rx);
        let config_gate_timeout = Arc::new(AtomicBool::new(false));
        let timeout_in_worker = Arc::clone(&config_gate_timeout);
        let queue = ConfigPersistQueue::spawn_with_test_hooks(
            Arc::new(Bus::new(16)),
            tree.path().to_owned(),
            PersistTestHooks {
                before_write: Some(Arc::new(move |_| {
                    let _ = entered_tx.send(());
                    if released
                        .lock()
                        .expect("one real I/O gate")
                        .recv_timeout(WAIT)
                        .is_err()
                    {
                        timeout_in_worker.store(true, Ordering::SeqCst);
                    }
                })),
                ..PersistTestHooks::default()
            },
        );
        let mut control = load_control_config_from(&config).expect("actual typed config draft");
        control
            .control
            .joints
            .get_mut(TARGET)
            .expect("configured target")
            .impedance
            .kp = 51.0;
        queue
            .enqueue(PersistRequest {
                config_dir: config.clone(),
                motors: None,
                control,
                timestamp_ms: 30032,
                session_id: "shutdown-config".into(),
                operator_id: "shutdown-fixture".into(),
                joint: TARGET.into(),
                param: "impedance.kp".into(),
            })
            .expect("actual config worker job");
        let overlay = ActuatorOverlay::new(
            load_command_joint_allowlist_from(&config).expect("copied allowlist"),
            queue,
        );
        let ctrl = ControlLoop::from_simulation_with_paused_reference_journal(
            tree.path(),
            SimulationBus::default(),
            tree.path().join("history.yaml"),
            tree.path().join("reference.sqlite3"),
            &pause,
            200,
            50,
        )
        .expect("matching real owner composition");
        let mut owner = Owner {
            ctrl,
            pause: &pause,
            overlay: &overlay,
            release: release_tx,
        };
        let handle = install(&mut owner);
        assert!(pause.wait_paused(WAIT));
        entered_rx
            .recv_timeout(WAIT)
            .expect("actual config write is also held");
        if armed {
            owner
                .supervisor_mut()
                .cancel_reference_commit(&handle, ReferenceCancelReason::Operator)
                .expect("release eligibility while keeping old I/O");
            let second = request(&mut owner);
            for _ in 0..5 {
                owner
                    .supervisor_mut()
                    .advance_reference(&second)
                    .expect("new live acquisition phase");
            }
            assert!(owner.supervisor().reference_snapshot().reference_armed);
        }
        let before_writes = owner.supervisor().bus().transmissions().len();
        let observed = Mutex::new(None);
        let before_wait = |ctrl: &ControlLoop<SimulationBus>| {
            *observed.lock().expect("shutdown boundary observation") = Some((
                ctrl.supervisor().bus().transmissions().len(),
                ctrl.supervisor().reference_snapshot().reference_armed,
            ));
        };
        let budget = Duration::from_millis(250);
        let start = Instant::now();
        let outcome = finish_owner_shutdown(
            &mut owner,
            &overlay,
            disable_on_exit,
            budget,
            Some(&before_wait),
        );
        let elapsed = start.elapsed();
        // Always release and observe actual joined workers before decisive oracles.
        let _ = owner.release.send(());
        pause.release();
        let config_complete = overlay.close_persist_and_drain(WAIT);
        let journal_complete = owner
            .supervisor_mut()
            .drain_reference_journal_until(Instant::now() + WAIT);
        assert!(!config_gate_timeout.load(Ordering::SeqCst));
        assert_eq!(config_complete.status, PersistDrainStatus::Complete);
        assert!(journal_complete.is_complete());
        assert_eq!(journal_complete.durable_writes, 1);
        assert!(elapsed >= Duration::from_millis(240));
        assert!(
            elapsed < Duration::from_millis(440),
            "independent waits exceeded shared budget: {elapsed:?}"
        );
        assert_eq!(outcome.persist.status, PersistDrainStatus::TimedOut);
        assert!(outcome.persist.in_flight && !outcome.persist.worker_terminated);
        assert!(
            outcome.reference_journal.in_flight && !outcome.reference_journal.worker_terminated
        );
        assert_eq!(outcome.reference_journal.accepted_credits, 1);
        assert!(outcome.reference_journal.admission_closed);
        // The stop when one runs, then one exit type-24 Off per drive.
        let writes = usize::from(disable_on_exit || armed) * 15 + 5;
        assert_eq!(
            *observed.lock().expect("before-wait evidence"),
            Some((before_writes + writes, false))
        );
        assert_eq!(
            owner.supervisor().bus().transmissions().len(),
            before_writes + writes,
            "late disk completion adds no stop"
        );
        assert_eq!(outcome.mandatory_reference.is_some(), armed);
        assert!(matches!(outcome.stop, ExitStopOutcome::Skipped) != disable_on_exit);
        let completion = owner
            .supervisor()
            .reference_commit_snapshot(&handle)
            .expect("retained final actual history result");
        assert_eq!(
            completion.phase,
            ReferenceCommitPhase::Cancelled(if armed {
                ReferenceCancelReason::Operator
            } else {
                ReferenceCancelReason::Shutdown
            })
        );
        assert!(matches!(
            completion.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        assert!(!completion.usable_reference);
        assert_eq!(
            load_control_config_from(&config)
                .expect("actual final config file")
                .control
                .joints[TARGET]
                .impedance
                .kp,
            51.0
        );
    }
}
