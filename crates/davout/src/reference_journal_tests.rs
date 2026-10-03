//! Actual closed acquisitions and concrete SQLite files; never physical acceptance.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::simulation::{
    InitialVirtualReference, JournalPausePoint, JournalTestPause, ReferenceProofMode,
    ReferenceReplyRule, SimulationBus, SimulationReceive, TxMatcher, TxOccurrence, TxRule,
};
use crate::{
    ReferenceAudit, ReferenceCancelReason, ReferenceCause, ReferenceCommit, ReferenceCommitError,
    ReferenceCommitHandle, ReferenceCommitPhase, ReferenceHandle, ReferenceJournalResult,
    ReferenceRequest, ReferenceStageInvalidation, ReferenceStageStatus, Supervisor,
};
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};
use rusqlite::{Connection, TransactionBehavior};

#[path = "../../berthier/tests/support/mod.rs"]
pub(super) mod support;
const TARGET: &str = "right_elbow_pitch";
const WAIT: Duration = Duration::from_secs(5);

fn tree() -> support::FixtureTree {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    support::fixture_tree_without_diagnostics("reference-journal", &source)
}
fn zero() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0280_04fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}
struct TestOwner<'a> {
    owner: Supervisor<SimulationBus>,
    pause: Option<&'a JournalTestPause>,
}
impl std::ops::Deref for TestOwner<'_> {
    type Target = Supervisor<SimulationBus>;
    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}
impl std::ops::DerefMut for TestOwner<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.owner
    }
}
impl Drop for TestOwner<'_> {
    fn drop(&mut self) {
        if let Some(pause) = self.pause {
            pause.release();
        }
        self.owner.cancel_reference_for_shutdown();
        self.owner
            .drain_reference_journal_until(Instant::now() + WAIT);
    }
}
fn journal_owner<'a>(
    tree: &support::FixtureTree,
    pause: Option<&'a JournalTestPause>,
) -> TestOwner<'a> {
    let history = tree.path().join("history.yaml");
    let journal = tree.path().join("reference.sqlite3");
    let mut owner = if let Some(pause) = pause {
        Supervisor::from_simulation_with_paused_reference_journal(
            tree.path(),
            SimulationBus::default(),
            history,
            journal,
            pause,
        )
    } else {
        Supervisor::from_simulation_with_reference_journal(
            tree.path(),
            SimulationBus::default(),
            history,
            journal,
        )
    }
    .expect("explicit unreferenced journal owner");
    install_script(&mut owner);
    TestOwner { owner, pause }
}
fn install_script(owner: &mut Supervisor<SimulationBus>) {
    owner
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                device_id: Some(4),
                interface: Some("can0".into()),
            },
            occurrence: TxOccurrence::Every,
            receive: vec![SimulationReceive::Received(zero())],
            send_error: None,
        })
        .expect("Enable status input");
    owner
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress {
                interface: "can0".into(),
                device_id: 4,
            },
            occurrence: TxOccurrence::Every,
            frame: zero(),
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("real SetZero/pop correlation");
}
fn acquire(owner: &mut Supervisor<SimulationBus>) -> ReferenceHandle {
    let stamp = owner
        .reference_snapshot()
        .next_stamp
        .expect("fresh owner stamp");
    let handle = owner
        .begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("actual acquisition");
    for _ in 0..6 {
        owner
            .advance_reference(&handle)
            .expect("actual owner phase");
    }
    let snapshot = owner.reference_snapshot();
    let terminal = snapshot.terminal.expect("actual terminal");
    assert_eq!(terminal.cause, ReferenceCause::EvidenceStaged);
    assert_eq!(terminal.commit, ReferenceCommit::Unavailable);
    assert!(!terminal.usable_reference);
    assert_eq!(terminal.stop.attempts.len(), 15);
    assert_eq!(terminal.stop.failed_writes(), 0);
    assert_eq!(
        snapshot.staged_evidence,
        ReferenceStageStatus::CurrentEvidence
    );
    handle
}
fn audit() -> ReferenceAudit {
    ReferenceAudit {
        operator: "fixture-operator".into(),
        session: "fixture-session".into(),
    }
}
pub(super) fn actual_input() -> crate::reference_journal_event::Input {
    let tree = tree();
    let mut owner = journal_owner(&tree, None);
    let acquired = acquire(&mut owner);
    owner
        .stage_for_commit(&acquired)
        .expect("actual retained stage")
        .journal_input(audit())
        .expect("actual immutable input")
}
fn finish(
    owner: &mut Supervisor<SimulationBus>,
    handle: &ReferenceCommitHandle,
) -> crate::ReferenceCommitSnapshot {
    let start = Instant::now();
    loop {
        let snapshot = owner
            .advance_reference_commit(handle)
            .expect("actual ordered completion advance");
        if snapshot.journal != ReferenceJournalResult::Pending {
            return snapshot;
        }
        assert!(start.elapsed() < WAIT, "actual worker completion timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn close(owner: &mut Supervisor<SimulationBus>) {
    let report = owner.drain_reference_journal_until(Instant::now() + WAIT);
    assert!(
        report.is_complete(),
        "actual worker retained work: {report:?}"
    );
}

#[test]
fn durable_history_is_real_readback_and_never_a_current_reference() {
    let tree = tree();
    let path = tree.path().join("reference.sqlite3");
    let mut owner = journal_owner(&tree, None);
    assert!(!path.exists(), "construction performs no journal I/O");
    let acquisition = acquire(&mut owner);
    let terminal = owner.reference_snapshot().terminal;
    let input = owner
        .stage_for_commit(&acquisition)
        .expect("retained actual stage")
        .journal_input(audit())
        .expect("sealed worker input");
    let body: crate::reference_journal_event::Body =
        crate::reference_codec::decode(&input.encode(1, 1).expect("typed body"))
            .expect("canonical typed decode");
    assert!(
        body.validate(),
        "actual captured header: {:?}",
        body.capture
    );
    let handle = owner
        .begin_reference_commit(&acquisition, audit())
        .expect("one accepted job");
    assert_eq!(
        owner
            .begin_reference_commit(&acquisition, audit())
            .expect("identical retry"),
        handle
    );
    let mut changed = audit();
    changed.session.push('2');
    assert!(matches!(
        owner.begin_reference_commit(&acquisition, changed),
        Err(ReferenceCommitError::ConflictingRetry)
    ));
    assert!(owner.reference_busy());
    let snapshot = finish(&mut owner, &handle);
    assert_eq!(snapshot.phase, ReferenceCommitPhase::Complete);
    assert!(
        matches!(
            snapshot.journal,
            ReferenceJournalResult::DurableHistory {
                diagnostic_session: 1,
                job_sequence: 1,
                ..
            }
        ),
        "{snapshot:?}"
    );
    assert_eq!(snapshot.eligibility, ReferenceStageStatus::CurrentEvidence);
    assert!(!snapshot.usable_reference);
    assert_eq!(owner.reference_snapshot().terminal, terminal);
    assert!(owner.enable_targets(&[TARGET.into()]).is_err());
    close(&mut owner);
    let records = Supervisor::<SimulationBus>::inspect_reference_journal(&path, 8)
        .expect("reopen actual committed SQL");
    assert_eq!(records.len(), 1);
    assert!(!records[0].is_legacy_schema());
    assert_eq!(records[0].audit(), Some(&audit()));
    assert_eq!(records[0].joint(), Some(TARGET));
    assert_eq!(records[0].position_rad(), Some(f32::from_bits(0x8000_0000)));
    assert_eq!(records[0].motors().expect("typed motors").motors.len(), 5);
    assert_eq!(
        records[0].urdf().expect("typed urdf").name,
        owner.urdf_robot.name
    );
    drop(owner);
    let mut recovered = journal_owner(&tree, None);
    assert!(recovered.enable_targets(&[TARGET.into()]).is_err());
    assert_eq!(
        recovered.reference_snapshot().staged_evidence,
        ReferenceStageStatus::NoEvidence
    );
    let acquired = acquire(&mut recovered);
    let second = recovered
        .begin_reference_commit(&acquired, audit())
        .expect("new owner's actual job");
    assert!(matches!(
        finish(&mut recovered, &second).journal,
        ReferenceJournalResult::DurableHistory {
            diagnostic_session: 2,
            job_sequence: 1,
            ..
        }
    ));
    close(&mut recovered);
}

#[test]
fn late_real_write_does_not_rewrite_operator_cancellation_or_send_extra_stops() {
    let tree = tree();
    let pause =
        JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("declarative pause");
    let mut owner = journal_owner(&tree, Some(&pause));
    let acquisition = acquire(&mut owner);
    let handle = owner
        .begin_reference_commit(&acquisition, audit())
        .expect("accepted job");
    assert!(pause.wait_paused(WAIT));
    let writes = owner.bus().transmissions().len();
    let cancelled = owner
        .cancel_reference_commit(&handle, ReferenceCancelReason::Operator)
        .expect("cancel eligibility only");
    assert_eq!(
        cancelled.phase,
        ReferenceCommitPhase::Cancelled(ReferenceCancelReason::Operator)
    );
    assert_eq!(cancelled.journal, ReferenceJournalResult::Pending);
    assert!(!owner.reference_busy());
    assert!(owner.reference_work_pending());
    pause.release();
    let late = finish(&mut owner, &handle);
    assert_eq!(late.phase, cancelled.phase);
    assert!(matches!(
        late.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert!(matches!(
        late.eligibility,
        ReferenceStageStatus::Invalidated(_)
    ));
    assert_eq!(owner.bus().transmissions().len(), writes);
    close(&mut owner);
}

#[test]
fn queued_real_completion_loses_eligibility_at_deadline_equality() {
    let tree = tree();
    let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("pause");
    let mut owner = journal_owner(&tree, Some(&pause));
    let acquisition = acquire(&mut owner);
    let handle = owner
        .begin_reference_commit(&acquisition, audit())
        .expect("accepted");
    assert!(pause.wait_paused(WAIT));
    owner
        .bus_mut()
        .elapse_reference_clock(Duration::from_secs(30))
        .expect("exact original deadline");
    pause.release();
    let late = finish(&mut owner, &handle);
    assert_eq!(
        late.phase,
        ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::DeadlineExpired)
    );
    assert!(matches!(
        late.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    close(&mut owner);
}

#[test]
fn corruption_and_blocked_parent_fail_without_replacing_existing_resources() {
    for blocked in [false, true] {
        let tree = tree();
        let path = tree.path().join("reference.sqlite3");
        if blocked {
            std::fs::create_dir(&path).expect("blocked file resource");
        } else {
            std::fs::write(&path, b"preserve-this-corrupt-fixture").expect("owned malformed data");
        }
        let mut owner = journal_owner(&tree, None);
        let acquired = acquire(&mut owner);
        let handle = owner
            .begin_reference_commit(&acquired, audit())
            .expect("worker admission precedes filesystem validation");
        assert!(matches!(
            finish(&mut owner, &handle).journal,
            ReferenceJournalResult::Failed { .. }
        ));
        if blocked {
            assert!(path.is_dir());
        } else {
            assert_eq!(
                std::fs::read(&path).expect("preserved file"),
                b"preserve-this-corrupt-fixture"
            );
        }
        close(&mut owner);
    }
}

#[test]
fn actual_sql_lock_is_failure_and_does_not_replace_or_overwrite_history() {
    let tree = tree();
    let path = tree.path().join("reference.sqlite3");
    let mut first = journal_owner(&tree, None);
    let acquired = acquire(&mut first);
    let handle = first
        .begin_reference_commit(&acquired, audit())
        .expect("first event");
    assert!(matches!(
        finish(&mut first, &handle).journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    close(&mut first);
    let mut database = Connection::open(&path).expect("real competing connection");
    let transaction = database
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .expect("actual exclusive lock");
    let mut second = journal_owner(&tree, None);
    let acquired = acquire(&mut second);
    let handle = second
        .begin_reference_commit(&acquired, audit())
        .expect("admission");
    assert!(matches!(
        finish(&mut second, &handle).journal,
        ReferenceJournalResult::Failed { .. }
    ));
    close(&mut second);
    transaction.rollback().expect("release owned SQL lock");
    drop(database);
    assert_eq!(
        Supervisor::<SimulationBus>::inspect_reference_journal(&path, 8)
            .expect("preserved prior event")
            .len(),
        1
    );
}

#[test]
fn ordinary_constructors_and_foreign_handles_never_gain_journal_capability() {
    let tree = tree();
    let mut ordinary = Supervisor::from_simulation(
        tree.path(),
        SimulationBus::default(),
        InitialVirtualReference::Unreferenced,
    )
    .expect("ordinary virtual factory");
    install_script(&mut ordinary);
    let acquisition = acquire(&mut ordinary);
    assert!(matches!(
        ordinary.begin_reference_commit(&acquisition, audit()),
        Err(ReferenceCommitError::Unsupported)
    ));
    assert!(ordinary
        .drain_reference_journal_until(Instant::now())
        .is_complete());
    assert!(!tree.path().join("reference.sqlite3").exists());
    let mut journal = journal_owner(&tree, None);
    assert!(matches!(
        journal.begin_reference_commit(&acquisition, audit()),
        Err(ReferenceCommitError::Acquisition(
            crate::ReferenceError::ForeignIdentity
        ))
    ));
    close(&mut journal);
}

#[test]
fn all_eight_accepted_credits_survive_cancellation_and_cache_eviction() {
    let tree = tree();
    let pause = JournalTestPause::new(JournalPausePoint::BeforeOpen, 1).expect("actual I/O gate");
    let mut owner = journal_owner(&tree, Some(&pause));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let acquired = acquire(&mut owner);
        let handle = owner
            .begin_reference_commit(&acquired, audit())
            .expect("reserved completion credit");
        owner
            .cancel_reference_commit(&handle, ReferenceCancelReason::Operator)
            .expect("retire eligibility, retain disk work");
        handles.push(handle);
    }
    assert!(pause.wait_paused(WAIT));
    let ninth = acquire(&mut owner);
    assert!(matches!(
        owner.begin_reference_commit(&ninth, audit()),
        Err(ReferenceCommitError::QueueFull)
    ));
    for handle in &handles {
        assert_eq!(
            owner
                .reference_commit_snapshot(handle)
                .expect("pending correlations never evicted")
                .journal,
            ReferenceJournalResult::Pending
        );
    }
    pause.release();
    let first = finish(&mut owner, &handles[0]);
    assert!(matches!(
        first.journal,
        ReferenceJournalResult::DurableHistory {
            job_sequence: 1,
            ..
        }
    ));
    let admitted = owner
        .begin_reference_commit(&ninth, audit())
        .expect("identical rejected admission is retryable after credit return");
    for handle in &handles[1..] {
        let snapshot = finish(&mut owner, handle);
        assert_eq!(
            snapshot.phase,
            ReferenceCommitPhase::Cancelled(ReferenceCancelReason::Operator)
        );
        assert!(matches!(
            snapshot.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
    }
    assert!(matches!(
        finish(&mut owner, &admitted).journal,
        ReferenceJournalResult::DurableHistory {
            job_sequence: 9,
            ..
        }
    ));
    assert!(matches!(
        owner.reference_commit_snapshot(&handles[0]),
        Err(ReferenceCommitError::OutcomeExpired)
    ));
    close(&mut owner);
    let sql =
        Connection::open(tree.path().join("reference.sqlite3")).expect("actual reopened history");
    assert_eq!(
        sql.query_row::<i64, _, _>("SELECT count(*) FROM reference_events", [], |row| row
            .get(0))
            .expect("real rows"),
        9
    );
}

#[test]
fn fresh_report_fault_and_saturation_precede_consuming_a_real_completion() {
    for saturated in [false, true] {
        let tree = tree();
        let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1)
            .expect("after actual commit/readback");
        let mut owner = journal_owner(&tree, Some(&pause));
        let acquisition = acquire(&mut owner);
        let handle = owner
            .begin_reference_commit(&acquisition, audit())
            .expect("real job");
        assert!(pause.wait_paused(WAIT));
        let writes = owner.bus().transmissions().len();
        let raw = if saturated { 65 } else { 2 };
        for order in 0..raw {
            let id = if order == 1 { 0x0281_02fd } else { 0x1f00_04fd };
            owner
                .bus_mut()
                .queue_frame(CanFrame {
                    id,
                    data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                    extended: true,
                })
                .expect("literal ordered peer fault/noise");
        }
        pause.release();
        let observed = owner
            .advance_reference_commit(&handle)
            .expect("one bounded actual report");
        let receive = observed.receive.expect("whole ordered report");
        assert_eq!(receive.raw_frames, if saturated { 64 } else { 2 });
        assert_eq!(receive.read_attempts, if saturated { 64 } else { 3 });
        assert_eq!(
            receive.completion,
            if saturated {
                robstride::ReceiveCompletion::WorkLimit
            } else {
                robstride::ReceiveCompletion::Idle
            }
        );
        assert!(owner.has_latched_fault());
        assert_eq!(
            observed.phase,
            ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::SafetyHazard)
        );
        assert_eq!(
            owner.bus().transmissions().len(),
            writes + 15,
            "one real all-address stop"
        );
        let completed = finish(&mut owner, &handle);
        assert!(matches!(
            completed.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        assert_eq!(completed.phase, observed.phase);
        assert_eq!(
            owner.bus().transmissions().len(),
            writes + 15,
            "no duplicate stop on cached fault"
        );
        close(&mut owner);
    }
}

#[test]
fn restored_policy_bits_cannot_restore_observed_pending_eligibility() {
    let tree = tree();
    let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("pause");
    let mut owner = journal_owner(&tree, Some(&pause));
    let original = owner.control.control.comm_watchdog_ms;
    let acquisition = acquire(&mut owner);
    let handle = owner
        .begin_reference_commit(&acquisition, audit())
        .expect("real job");
    assert!(pause.wait_paused(WAIT));
    let motors = owner.motors.clone();
    let control = owner.control.clone();
    let urdf_robot = owner.urdf_robot.clone();
    assert!(
        owner
            .restore_limit_snapshot(motors, control, urdf_robot)
            .is_err(),
        "eligible pending commit excludes typed installs"
    );
    owner.control.control.comm_watchdog_ms = original + 1;
    let changed = owner
        .reference_commit_snapshot(&handle)
        .expect("observed mismatch");
    owner.control.control.comm_watchdog_ms = original;
    pause.release();
    let late = finish(&mut owner, &handle);
    assert_eq!(
        changed.phase,
        ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::BindingChanged)
    );
    assert_eq!(late.phase, changed.phase);
    assert!(matches!(
        late.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    close(&mut owner);
}

#[test]
fn worker_unwind_retains_uncertain_committed_job_and_failed_queued_neighbors() {
    let tree = tree();
    let pause = JournalTestPause::with_worker_unwind(JournalPausePoint::AfterCommit)
        .expect("fixed real worker unwind");
    let mut owner = journal_owner(&tree, Some(&pause));
    let first_acquisition = acquire(&mut owner);
    let first = owner
        .begin_reference_commit(&first_acquisition, audit())
        .expect("real job");
    assert!(pause.wait_paused(WAIT), "actual SQL COMMIT occurred");
    owner
        .cancel_reference_commit(&first, ReferenceCancelReason::Operator)
        .expect("release eligibility only");
    let second_acquisition = acquire(&mut owner);
    let second = owner
        .begin_reference_commit(&second_acquisition, audit())
        .expect("queued accepted job");
    pause.release();
    let committed = finish(&mut owner, &first);
    let queued = finish(&mut owner, &second);
    assert!(matches!(
        committed.journal,
        ReferenceJournalResult::Uncertain { .. }
    ));
    assert_eq!(
        committed.phase,
        ReferenceCommitPhase::Cancelled(ReferenceCancelReason::Operator)
    );
    assert!(matches!(
        queued.journal,
        ReferenceJournalResult::Failed { .. }
    ));
    let report = owner.drain_reference_journal_until(Instant::now() + WAIT);
    assert!(report.is_complete());
    assert_eq!(
        (
            report.uncertain_writes,
            report.failed_writes,
            report.accepted_credits
        ),
        (1, 1, 0)
    );
    assert!(report.worker_error.is_some());
    assert_eq!(
        Supervisor::<SimulationBus>::inspect_reference_journal(
            tree.path().join("reference.sqlite3"),
            8
        )
        .expect("actual post-unwind commit recovery")
        .len(),
        1
    );
}

#[test]
fn exact_urdf_optional_fields_and_policy_scalars_survive_real_sql_reopen() {
    let tree = tree();
    let mut owner = journal_owner(&tree, None);
    let material = urdf_rs::Material {
        name: "exact-material".into(),
        color: Some(urdf_rs::Color {
            rgba: urdf_rs::Vec4([-0.0, f64::from_bits(1), 1.25, 1.0]),
        }),
        texture: Some(urdf_rs::Texture {
            filename: "exact-texture.png".into(),
        }),
    };
    owner.urdf_robot.materials.push(material.clone());
    let link = &mut owner.urdf_robot.links[0];
    link.inertial.origin.xyz = urdf_rs::Vec3([-0.0, f64::from_bits(1), -1.125]);
    link.inertial.origin.rpy = urdf_rs::Vec3([0.125, 0.25, 0.375]);
    link.inertial.mass.value = 2.125;
    link.inertial.inertia = urdf_rs::Inertia {
        ixx: 1.25,
        ixy: -0.0,
        ixz: f64::from_bits(1),
        iyy: 2.25,
        iyz: -0.125,
        izz: 3.25,
    };
    for (index, geometry) in [
        urdf_rs::Geometry::Box {
            size: urdf_rs::Vec3([1.125, 2.25, 3.375]),
        },
        urdf_rs::Geometry::Cylinder {
            radius: 0.125,
            length: 1.25,
        },
        urdf_rs::Geometry::Capsule {
            radius: f64::from_bits(1),
            length: 2.25,
        },
        urdf_rs::Geometry::Sphere { radius: 3.25 },
        urdf_rs::Geometry::Mesh {
            filename: "exact-mesh.stl".into(),
            scale: Some(urdf_rs::Vec3([-0.0, f64::from_bits(1), 1.125])),
        },
        urdf_rs::Geometry::Mesh {
            filename: "unscaled-mesh.stl".into(),
            scale: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        link.visual.push(urdf_rs::Visual {
            name: Some(format!("exact-visual-{index}")),
            origin: link.inertial.origin.clone(),
            geometry: geometry.clone(),
            material: Some(material.clone()),
        });
        link.collision.push(urdf_rs::Collision {
            name: Some(format!("exact-collision-{index}")),
            origin: link.inertial.origin.clone(),
            geometry,
        });
    }
    let joint = &mut owner.urdf_robot.joints[0];
    joint.dynamics = Some(urdf_rs::Dynamics {
        damping: -0.0,
        friction: f64::from_bits(1),
    });
    joint.mimic = Some(urdf_rs::Mimic {
        joint: "exact-mimic".into(),
        multiplier: Some(1.125),
        offset: Some(-0.0),
    });
    joint.safety_controller = Some(urdf_rs::SafetyController {
        soft_lower_limit: -0.125,
        soft_upper_limit: 0.25,
        k_position: f64::from_bits(1),
        k_velocity: -0.0,
    });
    owner.control.control.comm_watchdog_ms = 9_007_199_254_740_993;
    let motors = owner.motors.clone();
    let control = owner.control.clone();
    let urdf_robot = owner.urdf_robot.clone();
    owner
        .restore_limit_snapshot(motors, control, urdf_robot)
        .expect("typed immutable model install before acquisition");
    let expected_urdf = owner.urdf_robot.clone();
    let acquired = acquire(&mut owner);
    let handle = owner
        .begin_reference_commit(&acquired, audit())
        .expect("real typed capture");
    assert!(matches!(
        finish(&mut owner, &handle).journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    close(&mut owner);
    let records = Supervisor::<SimulationBus>::inspect_reference_journal(
        tree.path().join("reference.sqlite3"),
        8,
    )
    .expect("real owned decoded URDF");
    let actual = records[0].urdf().expect("typed urdf");
    assert_eq!(format!("{actual:?}"), format!("{expected_urdf:?}"));
    assert_eq!(
        actual.links[0].inertial.origin.xyz.0.map(f64::to_bits),
        [0x8000_0000_0000_0000, 1, (-1.125f64).to_bits()]
    );
    assert_eq!(
        actual.links[0].inertial.inertia.ixy.to_bits(),
        0x8000_0000_0000_0000
    );
    assert_eq!(actual.links[0].inertial.inertia.ixz.to_bits(), 1);
    assert_eq!(
        actual
            .materials
            .last()
            .expect("optional material")
            .color
            .as_ref()
            .expect("color")
            .rgba
            .0
            .map(f64::to_bits),
        [0x8000_0000_0000_0000, 1, 1.25f64.to_bits(), 1f64.to_bits()]
    );
    assert_eq!(
        actual.joints[0]
            .dynamics
            .as_ref()
            .expect("dynamics")
            .damping
            .to_bits(),
        0x8000_0000_0000_0000
    );
    assert_eq!(
        actual.joints[0]
            .mimic
            .as_ref()
            .expect("mimic")
            .offset
            .expect("offset")
            .to_bits(),
        0x8000_0000_0000_0000
    );
    assert_eq!(
        actual.joints[0]
            .safety_controller
            .as_ref()
            .expect("safety controller")
            .k_position
            .to_bits(),
        1
    );
    assert_eq!(
        records[0]
            .control()
            .expect("typed control")
            .control
            .comm_watchdog_ms,
        9_007_199_254_740_993
    );
}

#[test]
fn corrupt_checksum_incompatible_schema_and_session_exhaustion_are_preserved() {
    for alteration in [
        "UPDATE reference_events SET checksum=zeroblob(32)",
        "CREATE TABLE unexpected(value INTEGER)",
        "PRAGMA user_version=2",
        "UPDATE reference_meta SET next_session=9223372036854775807",
    ] {
        let tree = tree();
        let path = tree.path().join("reference.sqlite3");
        let mut first = journal_owner(&tree, None);
        let acquired = acquire(&mut first);
        let handle = first
            .begin_reference_commit(&acquired, audit())
            .expect("real baseline event");
        assert!(matches!(
            finish(&mut first, &handle).journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        close(&mut first);
        Connection::open(&path)
            .expect("owned fixture SQL connection")
            .execute_batch(alteration)
            .expect("explicit fixture alteration");
        let original = std::fs::read(&path).expect("exact altered resource");
        let mut second = journal_owner(&tree, None);
        let acquired = acquire(&mut second);
        let handle = second
            .begin_reference_commit(&acquired, audit())
            .expect("worker admission");
        assert!(matches!(
            finish(&mut second, &handle).journal,
            ReferenceJournalResult::Failed { .. }
        ));
        close(&mut second);
        assert_eq!(
            std::fs::read(&path).expect("preserved evidence"),
            original,
            "{alteration}"
        );
    }
}

/// The bench regression: a row written by an older binary (here, with the
/// retired `allow_firmware_speed_mode` bench key restored inside the control
/// payload) must still open. History integrity runs on the identity view, so
/// the new commit appends in a fresh session without touching the old row.
#[test]
fn older_schema_row_with_retired_control_key_still_opens_and_appends() {
    let tree = tree();
    let path = tree.path().join("reference.sqlite3");
    let mut first = journal_owner(&tree, None);
    let acquired = acquire(&mut first);
    let handle = first
        .begin_reference_commit(&acquired, audit())
        .expect("real baseline event");
    assert!(matches!(
        finish(&mut first, &handle).journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    close(&mut first);
    drift_control_bench(&path, "allow_firmware_speed_mode", true);
    let mut second = journal_owner(&tree, None);
    let acquired = acquire(&mut second);
    let handle = second
        .begin_reference_commit(&acquired, audit())
        .expect("worker admission after schema drift");
    assert!(
        matches!(
            finish(&mut second, &handle).journal,
            ReferenceJournalResult::DurableHistory {
                diagnostic_session: 2,
                job_sequence: 1,
                ..
            }
        ),
        "old-schema history must not block the new commit"
    );
    close(&mut second);
    let records =
        Supervisor::<SimulationBus>::inspect_reference_journal(&path, 8).expect("mixed history");
    assert_eq!(records.len(), 2);
    assert!(!records[0].is_legacy_schema());
    assert_eq!(records[0].diagnostic_session(), 2);
    let legacy = &records[1];
    assert!(legacy.is_legacy_schema());
    assert_eq!(legacy.diagnostic_session(), 1);
    assert_eq!(legacy.job_sequence(), 1);
    assert_eq!(legacy.joint(), None);
    assert_eq!(legacy.audit(), None);
}

/// Fail-closed behaviour survives schema tolerance: a checksum mismatch or a
/// row-key identity mismatch inside an old-schema row is still refused on
/// open, and the stored bytes are preserved.
#[test]
fn corrupt_old_schema_row_is_refused_and_preserved() {
    for tamper_checksum in [false, true] {
        let tree = tree();
        let path = tree.path().join("reference.sqlite3");
        let mut first = journal_owner(&tree, None);
        let acquired = acquire(&mut first);
        let handle = first
            .begin_reference_commit(&acquired, audit())
            .expect("real baseline event");
        assert!(matches!(
            finish(&mut first, &handle).journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        close(&mut first);
        drift_control_bench(&path, "allow_firmware_speed_mode", true);
        if tamper_checksum {
            Connection::open(&path)
                .expect("owned fixture SQL connection")
                .execute_batch("UPDATE reference_events SET checksum=zeroblob(32)")
                .expect("explicit fixture corruption");
        } else {
            Connection::open(&path)
                .expect("owned fixture SQL connection")
                .execute_batch("UPDATE reference_events SET job=zeroblob(8)")
                .expect("explicit fixture identity mismatch");
        }
        let original = std::fs::read(&path).expect("exact altered resource");
        let mut second = journal_owner(&tree, None);
        let acquired = acquire(&mut second);
        let handle = second
            .begin_reference_commit(&acquired, audit())
            .expect("worker admission");
        assert!(
            matches!(
                finish(&mut second, &handle).journal,
                ReferenceJournalResult::Failed { .. }
            ),
            "tamper checksum={tamper_checksum}"
        );
        close(&mut second);
        assert_eq!(
            std::fs::read(&path).expect("preserved evidence"),
            original,
            "tamper checksum={tamper_checksum}"
        );
    }
}

/// Rewrite every stored body with one extra boolean control bench key and a
/// matching checksum, like rows written by an older binary. The envelope stays
/// valid, so the identity view still verifies; the typed body no longer
/// decodes under the current schema.
fn drift_control_bench(path: &std::path::Path, key: &str, value: bool) {
    let database = Connection::open(path).expect("owned fixture SQL connection");
    let mut query = database
        .prepare("SELECT session,job,body FROM reference_events")
        .expect("fixture row query");
    let mut rows = query.query([]).expect("fixture row scan");
    let mut drifted = Vec::new();
    while let Some(row) = rows.next().expect("fixture row") {
        let session: i64 = row.get(0).expect("fixture session");
        let job: Vec<u8> = row.get(1).expect("fixture job");
        let body: Vec<u8> = row.get(2).expect("fixture body");
        let spliced = crate::reference_codec::splice_bool_field(
            &body,
            &["policy", "control", "control", "bench"],
            key,
            value,
        )
        .expect("schema-drift fixture keeps a valid envelope");
        assert!(
            crate::reference_codec::decode::<crate::reference_journal_event::Body>(&spliced)
                .is_err(),
            "drifted body must fail the current typed decode"
        );
        use sha2::Digest;
        let checksum: [u8; 32] = sha2::Sha256::digest(&spliced).into();
        drifted.push((session, job, spliced, checksum.to_vec()));
    }
    drop(rows);
    drop(query);
    drop(database);
    let database = Connection::open(path).expect("owned fixture SQL connection");
    for (session, job, body, checksum) in drifted {
        database
            .execute(
                "UPDATE reference_events SET body=?1,checksum=?2 WHERE session=?3 AND job=?4",
                rusqlite::params![body, checksum, session, job],
            )
            .expect("fixture drift write");
    }
}

// Replayed only by the controlled child test; ordinary runs perform no child I/O.
#[test]
fn controlled_crash_child() {
    let Some(root) = std::env::var_os("MARENGO_JOURNAL_CHILD_FIXTURE") else {
        return;
    };
    let root = PathBuf::from(root);
    let point = if std::env::var_os("MARENGO_JOURNAL_AFTER_COMMIT").is_some() {
        JournalPausePoint::AfterCommit
    } else {
        JournalPausePoint::BeforeCommit
    };
    let pause = JournalTestPause::new(point, 1).expect("exact transaction pause");
    let mut owner = Supervisor::from_simulation_with_paused_reference_journal(
        &root,
        SimulationBus::default(),
        root.join("history.yaml"),
        root.join("reference.sqlite3"),
        &pause,
    )
    .expect("controlled child owner");
    install_script(&mut owner);
    let acquisition = acquire(&mut owner);
    owner
        .begin_reference_commit(&acquisition, audit())
        .expect("real child job");
    assert!(pause.wait_paused(WAIT), "reached named actual SQL phase");
    std::process::exit(91); // Controlled process death; no claim of power-loss/media acceptance.
}

#[test]
fn controlled_child_death_before_and_after_real_commit_recovers_history_only() {
    for after in [false, true] {
        let tree = tree();
        let mut child =
            std::process::Command::new(std::env::current_exe().expect("actual test executable"));
        child
            .args([
                "--exact",
                "reference_journal_tests::controlled_crash_child",
                "--nocapture",
            ])
            .env("MARENGO_JOURNAL_CHILD_FIXTURE", tree.path());
        if after {
            child.env("MARENGO_JOURNAL_AFTER_COMMIT", "1");
        }
        let status = child.status().expect("controlled child process");
        assert_eq!(status.code(), Some(91), "actual named SQL phase reached");
        let path = tree.path().join("reference.sqlite3");
        let records = Supervisor::<SimulationBus>::inspect_reference_journal(&path, 8)
            .expect("SQLite's own actual recovery");
        assert_eq!(records.len(), usize::from(after));
        let mut recovered = journal_owner(&tree, None);
        assert!(recovered.enable_targets(&[TARGET.into()]).is_err());
        assert_eq!(
            recovered.reference_snapshot().staged_evidence,
            ReferenceStageStatus::NoEvidence
        );
        close(&mut recovered);
    }
}
