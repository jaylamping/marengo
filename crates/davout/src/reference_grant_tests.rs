//! Real closed acquisition, durable consumption and selected output/lifetime.
//! Counter/reset/cache controls create no pose, raw proof or permission.
#![allow(clippy::expect_used)]

use super::*;
use crate::reference_journal_tests::support;
use crate::simulation::{
    JournalPausePoint, JournalTestPause, ReferenceProofMode, ReferenceReplyRule, SimulationBus,
    SimulationReceive, TxMatcher, TxOccurrence, TxRule,
};
use crate::{JointHomingState, MitJointCommand, OperationalMode, ReferenceCause, ReferenceRequest};
use robstride::{CanFrame, MotorAddress, ReceiveCompletion, ReceivedCanFrame};
use std::path::PathBuf;
use std::time::Duration;

const TARGET: &str = "right_elbow_pitch";
const PEER: &str = "right_shoulder_roll";
const WAIT: Duration = Duration::from_secs(5);

struct Owner<'a> {
    inner: Supervisor<SimulationBus>,
    pause: Option<&'a JournalTestPause>,
    proof: crate::simulation::RuleId,
}
impl std::ops::Deref for Owner<'_> {
    type Target = Supervisor<SimulationBus>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Owner<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl Drop for Owner<'_> {
    fn drop(&mut self) {
        if let Some(pause) = self.pause {
            pause.release();
        }
        self.inner.cancel_reference_for_shutdown();
        self.inner
            .drain_reference_journal_until(Instant::now() + WAIT);
    }
}

fn tree() -> support::FixtureTree {
    let tree = support::FixtureTree::new(
        "selected-reference",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    );
    let path = tree.path().join("config/control.yaml");
    let text = std::fs::read_to_string(&path).expect("owned policy");
    assert_eq!(
        text.matches("active_reporting_diagnostics: true").count(),
        1
    );
    std::fs::write(
        path,
        text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("isolated reporting policy");
    tree
}
fn raw_zero() -> CanFrame {
    CanFrame {
        id: 0x0280_04fd,
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        extended: true,
    }
}
fn owner<'a>(tree: &support::FixtureTree, pause: Option<&'a JournalTestPause>) -> Owner<'a> {
    let mut inner = if let Some(pause) = pause {
        Supervisor::from_simulation_with_paused_current_reference_journal(
            tree.path(),
            SimulationBus::default(),
            tree.path().join("history.yaml"),
            tree.path().join("reference.sqlite3"),
            pause,
        )
    } else {
        Supervisor::from_simulation_with_current_reference_journal(
            tree.path(),
            SimulationBus::default(),
            tree.path().join("history.yaml"),
            tree.path().join("reference.sqlite3"),
        )
    }
    .expect("explicit current-consuming Unreferenced factory");
    assert_eq!(inner.mode(), OperationalMode::Disabled);
    for motor in &inner.motors.motors {
        assert_eq!(
            inner.joint_homing_state(&motor.joint),
            JointHomingState::Unhomed
        );
    }
    let reply = ReceivedCanFrame::full_data(Some("can0".into()), raw_zero());
    inner
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                device_id: Some(4),
                interface: Some("can0".into()),
            },
            occurrence: TxOccurrence::Every,
            receive: vec![SimulationReceive::Received(reply.clone())],
            send_error: None,
        })
        .expect("literal Enable reply");
    let proof = inner
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress::new("can0", 4),
            occurrence: TxOccurrence::Every,
            frame: reply,
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("real correlated SetZero/pop rule");
    Owner {
        inner,
        pause,
        proof,
    }
}
fn acquire(owner: &mut Owner<'_>) -> ReferenceHandle {
    let before = owner.bus().transmissions().len();
    let pops = owner.bus().reference_rule_pop_count(owner.proof);
    let triggers = owner.bus().reference_rule_trigger_count(owner.proof);
    let stamp = owner.reference_snapshot().next_stamp.expect("fresh stamp");
    let handle = owner
        .begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("real reservation");
    assert_eq!(owner.bus().transmissions().len(), before);
    for _ in 0..6 {
        owner
            .advance_reference(&handle)
            .expect("real phase advance");
    }
    let terminal = owner
        .reference_snapshot()
        .terminal
        .expect("actual terminal");
    assert_eq!(terminal.cause, ReferenceCause::EvidenceStaged);
    assert!(!terminal.usable_reference);
    assert_eq!(terminal.stop.attempts.len(), 15);
    assert_eq!(terminal.stop.failed_writes(), 0);
    assert_eq!(
        owner.bus().reference_rule_trigger_count(owner.proof),
        triggers + 1
    );
    assert_eq!(owner.bus().reference_rule_pop_count(owner.proof), pops + 1);
    let writes = &owner.bus().transmissions()[before..];
    assert_eq!(writes.len(), 32);
    assert!(writes.iter().all(|write| write.delivered));
    assert_eq!(writes[15].frame.id, 0x0300_fd04);
    assert_eq!(writes[16].frame.id, 0x0600_fd04);
    for (slot, device) in (1u8..=5).enumerate() {
        let stop = &writes[17 + slot * 3..20 + slot * 3];
        assert!(stop
            .iter()
            .all(|write| { write.address.as_ref() == Some(&MotorAddress::new("can0", device)) }));
        assert_eq!(stop[2].frame.id, 0x0400_fd00 | u32::from(device));
    }
    handle
}
fn begin_commit(owner: &mut Owner<'_>, acquisition: &ReferenceHandle) -> ReferenceCommitHandle {
    owner
        .begin_reference_commit(
            acquisition,
            ReferenceAudit {
                operator: "grant-fixture".into(),
                session: "selected-lifetime".into(),
            },
        )
        .expect("actual accepted immutable worker job")
}
fn finish(owner: &mut Owner<'_>, handle: &ReferenceCommitHandle) -> ReferenceCommitSnapshot {
    let until = Instant::now() + WAIT;
    loop {
        let snapshot = owner
            .advance_reference_commit(handle)
            .expect("real consumption");
        if snapshot.journal != ReferenceJournalResult::Pending {
            return snapshot;
        }
        assert!(Instant::now() < until, "actual completion timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn queued_durable(owner: &mut Owner<'_>, handle: &ReferenceCommitHandle) {
    let result = owner
        .reference_commits
        .journal
        .as_mut()
        .expect("actual worker")
        .wait_until(Instant::now() + WAIT);
    assert!(result.worker_terminated);
    assert_eq!(
        (
            result.durable_writes,
            result.completed_unconsumed,
            result.accepted_credits
        ),
        (1, 1, 1)
    );
    assert_eq!(
        owner
            .reference_commit_snapshot(handle)
            .expect("pending observation")
            .phase,
        ReferenceCommitPhase::Pending
    );
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
}
fn select(owner: &mut Owner<'_>) -> ReferenceCommitHandle {
    let acquired = acquire(owner);
    let handle = begin_commit(owner, &acquired);
    let selected = finish(owner, &handle);
    assert!(matches!(
        selected.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert!(selected.usable_reference);
    assert_eq!(selected.phase, ReferenceCommitPhase::Complete);
    assert_eq!(
        selected.eligibility,
        ReferenceStageStatus::Invalidated(ReferenceStageInvalidation::Consumed)
    );
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Verified);
    handle
}
fn command(joint: &str) -> MitJointCommand {
    MitJointCommand {
        joint: joint.into(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.1,
    }
}
fn actual_output(owner: &mut Owner<'_>) {
    owner
        .enable_targets(&[TARGET.into()])
        .expect("real selected Enable");
    owner
        .bus_mut()
        .queue_frame(raw_zero())
        .expect("post-enable raw pose");
    owner.drain_feedback().expect("actual post-enable receive");
    let before = owner.bus().frames().len();
    owner
        .send_mit_batch(vec![command(TARGET)])
        .expect("selected nonzero output");
    let frame = &owner.bus().frames()[before];
    assert_eq!(frame.id >> 24, 1);
    assert_eq!(frame.id & 0xff, 4);
    let torque = (frame.id >> 8) & 0xffff;
    assert_ne!(
        torque, 32767,
        "actual output was neutral rather than selected torque"
    );
}

#[test]
fn current_grant_requires_real_durable_fresh_consumption_and_covers_only_its_joint() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    let acquired = acquire(&mut owner);
    let handle = begin_commit(&mut owner, &acquired);
    queued_durable(&mut owner, &handle);
    let history = Supervisor::<SimulationBus>::inspect_reference_journal(
        tree.path().join("reference.sqlite3"),
        8,
    )
    .expect("actual committed/readback history");
    assert_eq!(history.len(), 1);
    let before = owner.bus().transmissions().len();
    let selected = owner
        .advance_reference_commit(&handle)
        .expect("fresh consumption");
    assert!(selected.usable_reference);
    assert_eq!(selected.receive.expect("fresh report").raw_frames, 0);
    assert_eq!(owner.bus().transmissions().len(), before);
    assert_eq!(owner.joint_homing_state(PEER), JointHomingState::Unhomed);
    assert!(owner.set_homing_complete().is_err());
    assert!(owner.request_enable(true).is_err());
    assert!(owner.enable_targets(&[PEER.into()]).is_err());
    assert_eq!(owner.bus().transmissions().len(), before);
    actual_output(&mut owner);
    owner
        .request_enable(true)
        .expect("same selected Active shortcut");
    let before = owner.bus().transmissions().len();
    assert!(owner.send_mit_batch(vec![command(PEER)]).is_err());
    assert_eq!(owner.bus().transmissions().len(), before);
}

#[test]
fn current_grant_lifetime_survives_old_deadline_and_successful_ordinary_disable() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    let handle = select(&mut owner);
    let generation = owner.reference_generation();
    owner
        .bus_mut()
        .elapse_reference_clock(Duration::from_secs(30))
        .expect("old transaction deadline equality");
    assert!(
        owner
            .reference_commit_snapshot(&handle)
            .expect("current lifetime")
            .usable_reference
    );
    actual_output(&mut owner);
    let stop = owner.stop_generation();
    owner.disable_all().expect("ordinary successful stop");
    assert!(owner.stop_generation() > stop);
    assert_eq!(owner.reference_generation(), generation);
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Verified);
    actual_output(&mut owner);
}

#[test]
fn current_grant_is_independent_of_diagnostic_caches_and_copied_history() {
    let tree = tree();
    let mut original = owner(&tree, None);
    let acquired = acquire(&mut original);
    let handle = begin_commit(&mut original, &acquired);
    queued_durable(&mut original, &handle);
    let snapshot = original
        .advance_reference_commit(&handle)
        .expect("actual grant");
    assert!(snapshot.usable_reference);
    original.reference_owner.evict_outcomes_for_test();
    original.reference_commits.outcomes.clear();
    assert!(matches!(
        original.reference_commit_snapshot(&handle),
        Err(ReferenceCommitError::OutcomeExpired)
    ));
    assert_eq!(
        original.joint_homing_state(TARGET),
        JointHomingState::Verified
    );
    actual_output(&mut original);
    let mut fresh = owner(&tree, None);
    let before = fresh.bus().transmissions().len();
    assert!(fresh.enable_targets(&[snapshot.joint.clone()]).is_err());
    assert_eq!(fresh.bus().transmissions().len(), before);
    assert_eq!(fresh.joint_homing_state(TARGET), JointHomingState::Unhomed);
    assert!(matches!(
        fresh.advance_reference_commit(&handle),
        Err(ReferenceCommitError::ForeignIdentity)
    ));
}

#[test]
fn current_grant_completed_unconsumed_history_loses_to_fresh_fault_or_work_limit() {
    for fault in [true, false] {
        let tree = tree();
        let mut owner = owner(&tree, None);
        let acquired = acquire(&mut owner);
        let handle = begin_commit(&mut owner, &acquired);
        queued_durable(&mut owner, &handle);
        let before = owner.bus().transmissions().len();
        for ordinal in 0..65 {
            owner
                .bus_mut()
                .queue_frame(CanFrame {
                    id: if fault && ordinal == 1 {
                        0x0281_02fd
                    } else {
                        0x1f00_04fd
                    },
                    ..raw_zero()
                })
                .expect("fresh whole-report stream");
        }
        let result = owner
            .advance_reference_commit(&handle)
            .expect("truthful durable result");
        assert!(matches!(
            result.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        assert!(!result.usable_reference);
        assert_eq!(
            result.phase,
            ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::SafetyHazard)
        );
        let report = result.receive.expect("fresh bounded actual receive");
        assert_eq!(
            (report.raw_frames, report.read_attempts, report.completion),
            (64, 64, ReceiveCompletion::WorkLimit)
        );
        assert_eq!(owner.bus().pending_receive_count(), 1);
        assert_eq!(owner.bus().transmissions().len(), before + 15);
        assert_ne!(owner.joint_homing_state(TARGET), JointHomingState::Verified);
    }
}

#[test]
fn current_grant_completed_unconsumed_history_loses_at_original_deadline_equality() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    let acquired = acquire(&mut owner);
    let handle = begin_commit(&mut owner, &acquired);
    queued_durable(&mut owner, &handle);
    owner
        .bus_mut()
        .elapse_reference_clock(Duration::from_secs(30))
        .expect("exact captured deadline");
    let result = owner
        .advance_reference_commit(&handle)
        .expect("late durable consumption");
    assert!(
        matches!(
            result.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ),
        "{result:?}"
    );
    assert_eq!(
        result.phase,
        ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::DeadlineExpired)
    );
    assert!(!result.usable_reference);
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
}

#[test]
fn current_grant_fresh_read_attempt_limit_refuses_completed_unconsumed_permission() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    let acquired = acquire(&mut owner);
    let handle = begin_commit(&mut owner, &acquired);
    queued_durable(&mut owner, &handle);
    owner
        .bus_mut()
        .queue_attempts((0..257).map(|_| SimulationReceive::Interrupted))
        .expect("bounded interrupted reads");
    let before = owner.bus().transmissions().len();
    let snapshot = owner
        .advance_reference_commit(&handle)
        .expect("actual bounded consumer");
    let receive = snapshot.receive.expect("fresh read report");
    assert_eq!(
        (
            receive.raw_frames,
            receive.read_attempts,
            receive.completion
        ),
        (0, 256, ReceiveCompletion::WorkLimit)
    );
    assert_eq!(owner.bus().pending_receive_count(), 1);
    assert_eq!(owner.bus().transmissions().len(), before + 15);
    assert!(matches!(
        snapshot.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert!(!snapshot.usable_reference);
}

#[test]
fn current_grant_cancel_and_shutdown_win_before_late_durable_publication() {
    for reason in [
        ReferenceCancelReason::Operator,
        ReferenceCancelReason::Shutdown,
    ] {
        let tree = tree();
        let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1)
            .expect("real committed/readback pause");
        let mut owner = owner(&tree, Some(&pause));
        let acquired = acquire(&mut owner);
        let handle = begin_commit(&mut owner, &acquired);
        assert!(pause.wait_paused(WAIT));
        let before = owner.bus().transmissions().len();
        if reason == ReferenceCancelReason::Shutdown {
            assert!(owner.cancel_reference_for_shutdown().is_none());
        } else {
            owner
                .cancel_reference_commit(&handle, reason)
                .expect("explicit cancel");
        }
        pause.release();
        let result = finish(&mut owner, &handle);
        assert!(matches!(
            result.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        assert_eq!(result.phase, ReferenceCommitPhase::Cancelled(reason));
        assert!(!result.usable_reference);
        assert_eq!(owner.bus().transmissions().len(), before);
    }
}

#[test]
fn current_grant_failed_and_uncertain_real_workers_cannot_select_permission() {
    {
        let tree = tree();
        std::fs::create_dir(tree.path().join("reference.sqlite3"))
            .expect("real incompatible resource");
        let mut owner = owner(&tree, None);
        let acquired = acquire(&mut owner);
        let handle = begin_commit(&mut owner, &acquired);
        let before = owner.bus().transmissions().len();
        let result = finish(&mut owner, &handle);
        assert!(matches!(
            result.journal,
            ReferenceJournalResult::Failed { .. }
        ));
        assert!(!result.usable_reference);
        assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
        assert_eq!(owner.bus().transmissions().len(), before);
    }
    {
        let tree = tree();
        let pause = JournalTestPause::with_worker_unwind(JournalPausePoint::AfterCommit)
            .expect("actual after-COMMIT unwind");
        let mut owner = owner(&tree, Some(&pause));
        let acquired = acquire(&mut owner);
        let handle = begin_commit(&mut owner, &acquired);
        assert!(pause.wait_paused(WAIT));
        pause.release();
        let result = finish(&mut owner, &handle);
        assert!(matches!(
            result.journal,
            ReferenceJournalResult::Uncertain { .. }
        ));
        assert!(!result.usable_reference);
        assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
    }
}

#[test]
fn current_grant_counter_exhaustion_refuses_commit_before_acceptance_or_permission_change() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    owner
        .reference_authority
        .set_generation_for_test(u64::MAX - 1);
    let acquired = acquire(&mut owner);
    assert_eq!(owner.reference_generation(), u64::MAX);
    let before = owner.bus().transmissions().len();
    assert!(matches!(
        owner.begin_reference_commit(
            &acquired,
            ReferenceAudit {
                operator: "counter-fixture".into(),
                session: "exhaustion".into()
            }
        ),
        Err(ReferenceCommitError::CounterExhausted)
    ));
    assert!(!tree.path().join("reference.sqlite3").exists());
    assert_eq!(owner.reference_generation(), u64::MAX);
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
    assert_eq!(owner.bus().transmissions().len(), before);
}

#[test]
fn current_grant_final_generation_blocks_new_acquisition_without_revoking_current_permission() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    owner
        .reference_authority
        .set_generation_for_test(u64::MAX - 2);
    let handle = select(&mut owner);
    assert_eq!(owner.reference_generation(), u64::MAX);
    let stamp = owner
        .reference_snapshot()
        .next_stamp
        .expect("last current stamp");
    let before = owner.bus().transmissions().len();
    assert!(matches!(
        owner.begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true
        }),
        Err(crate::ReferenceError::CounterExhausted)
    ));
    assert!(
        owner
            .reference_commit_snapshot(&handle)
            .expect("intact final generation")
            .usable_reference
    );
    assert_eq!(owner.reference_generation(), u64::MAX);
    assert_eq!(owner.bus().transmissions().len(), before);
}

#[test]
fn current_grant_pending_policy_observation_and_device_reset_prevent_restored_selection() {
    for reset in [true, false] {
        let tree = tree();
        let mut owner = owner(&tree, None);
        let acquired = acquire(&mut owner);
        let handle = begin_commit(&mut owner, &acquired);
        queued_durable(&mut owner, &handle);
        let original = owner.control.control.comm_watchdog_ms;
        if reset {
            owner
                .inner
                .bus
                .reset_reference_device_for_test(&MotorAddress::new("can0", 4))
                .expect("modeled reset after real write/readback");
        } else {
            owner.control.control.comm_watchdog_ms = original + 1;
        }
        let observed = owner
            .reference_commit_snapshot(&handle)
            .expect("sticky current observation");
        assert_eq!(
            observed.phase,
            ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::BindingChanged)
        );
        owner.control.control.comm_watchdog_ms = original;
        let before = owner.bus().transmissions().len();
        let consumed = owner
            .advance_reference_commit(&handle)
            .expect("late truthful consumption");
        assert!(matches!(
            consumed.journal,
            ReferenceJournalResult::DurableHistory { .. }
        ));
        assert_eq!(consumed.phase, observed.phase);
        assert!(!consumed.usable_reference);
        assert_eq!(owner.bus().transmissions().len(), before);
    }
}

#[test]
fn current_grant_observed_policy_mutation_and_model_replacement_stay_revoked() {
    for kind in 0..3 {
        let tree = tree();
        let mut owner = owner(&tree, None);
        let handle = select(&mut owner);
        let motors = owner.motors.clone();
        let control = owner.control.clone();
        match kind {
            0 => owner.motors.motors[0].direction *= -1,
            1 => {
                owner
                    .control
                    .control
                    .joints
                    .get_mut(TARGET)
                    .expect("bound policy")
                    .position_hold_trim_rad += 0.01
            }
            _ => owner
                .rebuild_limits()
                .expect("real equal-value model installation"),
        }
        assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
        owner.motors = motors;
        owner.control = control;
        assert!(
            !owner
                .reference_commit_snapshot(&handle)
                .expect("restored diagnostics")
                .usable_reference
        );
        let before = owner.bus().transmissions().len();
        assert!(owner.enable_targets(&[TARGET.into()]).is_err());
        assert_eq!(owner.bus().transmissions().len(), before);
    }
}

#[test]
fn current_grant_output_only_cap_change_retains_reference_and_clamps_real_output() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    let handle = select(&mut owner);
    owner
        .motors
        .motors
        .iter_mut()
        .find(|m| m.joint == TARGET)
        .expect("bound motor")
        .bench
        .torque_limit_nm = 0.05;
    assert!(
        owner
            .reference_commit_snapshot(&handle)
            .expect("valid output cap")
            .usable_reference
    );
    actual_output(&mut owner);
    let wire = owner.bus().frames().last().expect("actual MIT");
    assert_eq!(
        owner
            .motors
            .motors
            .iter()
            .find(|m| m.joint == TARGET)
            .expect("target motor")
            .motor_type,
        marengo_config::MotorType::Rs02
    );
    let torque = (f64::from((wire.id >> 8) & 0xffff) / 32767.0 - 1.0) * 17.0;
    assert!(
        torque.abs() <= 0.052,
        "literal wire torque {torque} exceeds cap"
    );
    owner.control.control.comm_watchdog_ms = 0;
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
}

#[test]
fn current_grant_device_reset_and_owning_commit_cancel_revoke_exact_selection() {
    for reset in [true, false] {
        let tree = tree();
        let mut owner = owner(&tree, None);
        let handle = select(&mut owner);
        if reset {
            owner
                .inner
                .bus
                .reset_reference_device_for_test(&MotorAddress::new("can0", 4))
                .expect("checked modeled closed-device reset");
        } else {
            owner
                .cancel_reference_commit(&handle, ReferenceCancelReason::Operator)
                .expect("cancel selected job");
        }
        assert!(
            !owner
                .reference_commit_snapshot(&handle)
                .expect("actual current projection")
                .usable_reference
        );
        assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
        let before = owner.bus().transmissions().len();
        assert!(owner.enable_targets(&[TARGET.into()]).is_err());
        assert_eq!(owner.bus().transmissions().len(), before);
    }
}

#[test]
fn current_grant_new_acquisition_revokes_then_real_neighbor_can_select_again() {
    let tree = tree();
    let mut owner = owner(&tree, None);
    let first = select(&mut owner);
    let acquired = acquire(&mut owner);
    assert!(
        !owner
            .reference_commit_snapshot(&first)
            .expect("prior selection")
            .usable_reference
    );
    assert_eq!(owner.joint_homing_state(TARGET), JointHomingState::Unhomed);
    let second = begin_commit(&mut owner, &acquired);
    assert!(finish(&mut owner, &second).usable_reference);
    assert!(
        !owner
            .reference_commit_snapshot(&first)
            .expect("old job cannot supply new grant")
            .usable_reference
    );
    actual_output(&mut owner);
}

#[test]
fn current_grant_fault_estop_and_uncertain_stop_preserve_revocation() {
    for cause in 0..3 {
        let tree = tree();
        let mut owner = owner(&tree, None);
        let handle = select(&mut owner);
        match cause {
            0 => {
                owner
                    .bus_mut()
                    .queue_frame(CanFrame {
                        id: 0x0281_02fd,
                        ..raw_zero()
                    })
                    .expect("real raw peer fault");
                owner.drain_feedback().expect_err("latched peer fault");
            }
            1 => owner.set_hardware_estop(true),
            _ => {
                owner
                    .bus_mut()
                    .add_tx_rule(TxRule {
                        matcher: TxMatcher {
                            communication_type: Some(4),
                            device_id: Some(2),
                            interface: Some("can0".into()),
                        },
                        occurrence: TxOccurrence::Every,
                        receive: vec![],
                        send_error: Some("uncertain modeled stop write".into()),
                    })
                    .expect("one actual failed stop attempt");
                assert!(owner.disable_all().is_err());
                assert_eq!(
                    owner
                        .safety_snapshot()
                        .last_stop
                        .expect("all-address report")
                        .attempts
                        .len(),
                    15
                );
            }
        }
        assert!(
            !owner
                .reference_commit_snapshot(&handle)
                .expect("revoked projection")
                .usable_reference
        );
        assert_ne!(owner.joint_homing_state(TARGET), JointHomingState::Verified);
    }
}
