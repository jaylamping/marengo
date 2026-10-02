//! The real controller consumes current virtual permission without stale intent.
#![allow(clippy::expect_used)]

use crate::test_support as support;
use crate::{ControlLoop, ControlMode};
use davout::simulation::{
    JournalPausePoint, JournalTestPause, ReferenceProofMode, ReferenceReplyRule, SimulationBus,
    SimulationReceive, TxMatcher, TxOccurrence, TxRule,
};
use davout::{
    JointHomingState, OperationalMode, ReferenceAudit, ReferenceCause, ReferenceCommitHandle,
    ReferenceCommitPhase, ReferenceJournalResult, ReferenceRequest, ReferenceStageInvalidation,
};
use robstride::{CanFrame, MotorAddress, ReceiveCompletion, ReceivedCanFrame};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const TARGET: &str = "right_elbow_pitch";
const WAIT: Duration = Duration::from_secs(5);

struct Owner<'a> {
    ctrl: ControlLoop<SimulationBus>,
    pause: &'a JournalTestPause,
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
        self.pause.release();
        self.ctrl.inhibit_motion_for_shutdown();
        self.ctrl.supervisor_mut().cancel_reference_for_shutdown();
        self.ctrl
            .supervisor_mut()
            .disable_all()
            .expect("closed test cleanup");
        self.ctrl
            .supervisor_mut()
            .drain_reference_journal_until(Instant::now() + WAIT);
    }
}
fn raw_zero() -> CanFrame {
    CanFrame {
        id: 0x0280_04fd,
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        extended: true,
    }
}
fn tree() -> support::FixtureTree {
    let tree = support::FixtureTree::new(
        "current-grant-controller",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    );
    let path = tree.path().join("config/control.yaml");
    let text = std::fs::read_to_string(&path).expect("owned policy");
    std::fs::write(
        path,
        text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("isolated policy");
    tree
}
fn setup<'a>(
    tree: &support::FixtureTree,
    pause: &'a JournalTestPause,
) -> (Owner<'a>, ReferenceCommitHandle) {
    let mut ctrl = ControlLoop::from_simulation_with_paused_current_reference_journal(
        tree.path(),
        SimulationBus::default(),
        tree.path().join("history.yaml"),
        tree.path().join("reference.sqlite3"),
        pause,
        200,
        50,
    )
    .expect("closed Unreferenced consuming controller");
    assert_eq!(
        ctrl.supervisor().joint_homing_state(TARGET),
        JointHomingState::Unhomed
    );
    ctrl.set_torque_cmd(TARGET, 0.25)
        .expect("old disabled intent");
    let frame = ReceivedCanFrame::full_data(Some("can0".into()), raw_zero());
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
        .expect("literal Enable reply");
    let proof = ctrl
        .supervisor_mut()
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress::new("can0", 4),
            occurrence: TxOccurrence::Every,
            frame,
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("real SetZero/pop rule");
    let stamp = ctrl
        .supervisor()
        .reference_snapshot()
        .next_stamp
        .expect("fresh stamp");
    let acquired = ctrl
        .supervisor_mut()
        .begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("real acquisition");
    for _ in 0..6 {
        ctrl.tick(None).expect("real owner phase tick");
    }
    let terminal = ctrl
        .supervisor()
        .reference_snapshot()
        .terminal
        .expect("actual terminal");
    assert_eq!(terminal.cause, ReferenceCause::EvidenceStaged);
    assert_eq!(terminal.stop.attempts.len(), 15);
    assert_eq!(terminal.stop.failed_writes(), 0);
    assert_eq!(
        ctrl.supervisor().bus().reference_rule_trigger_count(proof),
        1
    );
    assert_eq!(ctrl.supervisor().bus().reference_rule_pop_count(proof), 1);
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), 32);
    assert_eq!(ctrl.torque_cmd(TARGET), 0.0);
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    let handle = ctrl
        .supervisor_mut()
        .begin_reference_commit(
            &acquired,
            ReferenceAudit {
                operator: "controller-grant-fixture".into(),
                session: "current-grant-tick".into(),
            },
        )
        .expect("accepted real worker job");
    (Owner { ctrl, pause }, handle)
}

#[test]
fn current_grant_controller_consuming_tick_has_one_report_and_no_old_intent_or_output() {
    let tree = tree();
    let pause = JournalTestPause::new(JournalPausePoint::AfterPublication, 1)
        .expect("real completion queue gate");
    let (mut ctrl, handle) = setup(&tree, &pause);
    assert!(pause.wait_paused(WAIT));
    let before = ctrl.supervisor().bus().transmissions().len();
    ctrl.supervisor_mut()
        .bus_mut()
        .queue_frame(raw_zero())
        .expect("one fresh bounded report");
    ctrl.tick(None).expect("sole consumer tick");
    let snapshot = ctrl
        .supervisor()
        .reference_commit_snapshot(&handle)
        .expect("actual selected commit");
    assert!(matches!(
        snapshot.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert!(snapshot.usable_reference);
    let receive = snapshot.receive.expect("actual report");
    assert_eq!(
        (receive.raw_frames, receive.completion),
        (1, ReceiveCompletion::Idle)
    );
    assert_eq!(ctrl.supervisor().bus().pending_receive_count(), 0);
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), before);
    assert_eq!(ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    assert_eq!(ctrl.torque_cmd(TARGET), 0.0);
    assert_eq!(
        ctrl.supervisor().joint_homing_state("right_shoulder_roll"),
        JointHomingState::Unhomed
    );
    pause.release();
    ctrl.supervisor_mut()
        .enable_targets(&[TARGET.into()])
        .expect("new selected Enable");
    ctrl.set_torque_cmd(TARGET, 0.1)
        .expect("new explicit torque intent");
    support::queue_joint_status(ctrl.supervisor_mut(), TARGET, 0.0, 0.0);
    let before = ctrl.supervisor().bus().transmissions().len();
    ctrl.tick(None).expect("real selected output tick");
    let writes = &ctrl.supervisor().bus().transmissions()[before..];
    let motion: Vec<_> = writes.iter().filter(|w| w.frame.id >> 24 == 1).collect();
    assert_eq!(motion.len(), 1);
    assert_eq!(
        motion[0].address.as_ref(),
        Some(&MotorAddress::new("can0", 4))
    );
    assert_ne!(
        (motion[0].frame.id >> 8) & 0xffff,
        32767,
        "selected output stayed neutral"
    );
}

#[test]
fn current_grant_controller_completed_result_still_loses_to_whole_bounded_fault_report() {
    let tree = tree();
    let pause = JournalTestPause::new(JournalPausePoint::AfterPublication, 1)
        .expect("actual queued completion");
    let (mut ctrl, handle) = setup(&tree, &pause);
    assert!(pause.wait_paused(WAIT));
    let before = ctrl.supervisor().bus().transmissions().len();
    for ordinal in 0..65 {
        ctrl.supervisor_mut()
            .bus_mut()
            .queue_frame(CanFrame {
                id: if ordinal == 1 {
                    0x0281_02fd
                } else {
                    0x1f00_04fd
                },
                ..raw_zero()
            })
            .expect("whole raw report");
    }
    ctrl.tick(None)
        .expect("actual owner tick preserves hazard priority");
    let snapshot = ctrl
        .supervisor()
        .reference_commit_snapshot(&handle)
        .expect("actual retained result");
    assert!(matches!(
        snapshot.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert_eq!(
        snapshot.phase,
        ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::SafetyHazard)
    );
    assert!(!snapshot.usable_reference);
    let receive = snapshot.receive.expect("sole report");
    assert_eq!(
        (
            receive.raw_frames,
            receive.read_attempts,
            receive.completion
        ),
        (64, 64, ReceiveCompletion::WorkLimit)
    );
    assert_eq!(
        ctrl.supervisor().bus().pending_receive_count(),
        1,
        "competing drain consumed suffix"
    );
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), before + 15);
    assert_eq!(ctrl.torque_cmd(TARGET), 0.0);
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    pause.release();
}

#[test]
fn current_grant_controller_active_motion_entry_rechecks_revoked_permission() {
    let tree = tree();
    let pause = JournalTestPause::new(JournalPausePoint::AfterPublication, 1)
        .expect("actual completion gate");
    let (mut ctrl, handle) = setup(&tree, &pause);
    assert!(pause.wait_paused(WAIT));
    ctrl.tick(None).expect("actual durable grant consumption");
    assert!(
        ctrl.supervisor()
            .reference_commit_snapshot(&handle)
            .expect("current selected job")
            .usable_reference
    );
    pause.release();
    ctrl.supervisor_mut()
        .enable_targets(&[TARGET.into()])
        .expect("real selected Enable");
    let before = ctrl.supervisor().bus().transmissions().len();
    ctrl.ensure_active_for_motion()
        .expect("intact selected Active shortcut");
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), before);
    let stop = ctrl.supervisor().stop_generation();
    ctrl.supervisor_mut()
        .cancel_reference_commit(&handle, davout::ReferenceCancelReason::Operator)
        .expect("revoke the actual owning job");
    assert_eq!(ctrl.supervisor().mode(), OperationalMode::Active);
    assert_eq!(ctrl.supervisor().stop_generation(), stop);
    assert_eq!(
        ctrl.supervisor().joint_homing_state(TARGET),
        JointHomingState::Unhomed
    );
    assert!(ctrl.ensure_active_for_motion().is_err());
    assert_eq!(ctrl.supervisor().mode(), OperationalMode::Disabled);
    assert!(ctrl.supervisor().stop_generation() > stop);
    let report = ctrl
        .supervisor()
        .safety_snapshot()
        .last_stop
        .expect("shared admission's complete stop report");
    assert_eq!(report.attempts.len(), 15);
    assert_eq!(report.failed_writes(), 0);
    let writes = &ctrl.supervisor().bus().transmissions()[before..];
    assert_eq!(writes.len(), 15);
    for (slot, device) in (1u8..=5).enumerate() {
        let stop = &writes[slot * 3..slot * 3 + 3];
        assert!(stop
            .iter()
            .all(|write| write.address.as_ref() == Some(&MotorAddress::new("can0", device))));
        assert_eq!(stop[2].frame.id, 0x0400_fd00 | u32::from(device));
    }
}
