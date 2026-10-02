//! Real controller ticks over the explicit closed journal owner.
#![allow(clippy::expect_used)]
use crate::test_support as support;
use crate::{ControlLoop, ControlMode};
use davout::simulation::{
    JournalPausePoint, JournalTestPause, ReferenceProofMode, ReferenceReplyRule, SimulationBus,
    SimulationReceive, TxMatcher, TxOccurrence, TxRule,
};
use davout::{
    ReferenceAudit, ReferenceCancelReason, ReferenceCause, ReferenceCommitHandle,
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
        self.ctrl.supervisor_mut().cancel_reference_for_shutdown();
        self.ctrl
            .supervisor_mut()
            .drain_reference_journal_until(Instant::now() + WAIT);
    }
}
fn setup<'a>(
    tree: &support::FixtureTree,
    pause: &'a JournalTestPause,
) -> (Owner<'a>, ReferenceCommitHandle) {
    let mut ctrl = ControlLoop::from_simulation_with_paused_reference_journal(
        tree.path(),
        SimulationBus::default(),
        tree.path().join("history.yaml"),
        tree.path().join("reference.sqlite3"),
        pause,
        200,
        50,
    )
    .expect("matching explicit factory");
    ctrl.set_torque_cmd(TARGET, 0.25)
        .expect("old controller intent without drive permission");
    assert_eq!(ctrl.torque_cmd(TARGET), 0.25);
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
        .expect("Enable raw reply");
    ctrl.supervisor_mut()
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
        .expect("actual stamp");
    let acquired = ctrl
        .supervisor_mut()
        .begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("actual reservation");
    for _ in 0..6 {
        ctrl.tick(None).expect("actual tick owns each phase");
    }
    assert_eq!(
        ctrl.supervisor()
            .reference_snapshot()
            .terminal
            .expect("actual terminal")
            .cause,
        ReferenceCause::EvidenceStaged
    );
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    assert_eq!(ctrl.torque_cmd(TARGET), 0.0);
    let handle = ctrl
        .supervisor_mut()
        .begin_reference_commit(
            &acquired,
            ReferenceAudit {
                operator: "controller-fixture".into(),
                session: "controller-session".into(),
            },
        )
        .expect("actual journal job");
    (Owner { ctrl, pause }, handle)
}
fn resource() -> support::FixtureTree {
    let tree = support::FixtureTree::new(
        "journal-owner",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    );
    let path = tree.path().join("config/control.yaml");
    let text = std::fs::read_to_string(&path).expect("copied policy");
    std::fs::write(
        path,
        text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("isolated diagnostics");
    tree
}

#[test]
fn tick_uses_one_bounded_fresh_report_and_keeps_a_late_fault_above_real_completion() {
    let tree = resource();
    let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1)
        .expect("real SQL/readback gate");
    let (mut ctrl, handle) = setup(&tree, &pause);
    assert!(pause.wait_paused(WAIT));
    assert!(ctrl.set_torque_cmd(TARGET, 0.5).is_err());
    let writes = ctrl.supervisor().bus().transmissions().len();
    for order in 0..65 {
        ctrl.supervisor_mut()
            .bus_mut()
            .queue_frame(CanFrame {
                id: if order == 1 { 0x0281_02fd } else { 0x1f00_04fd },
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                extended: true,
            })
            .expect("ordered actual raw queue");
    }
    ctrl.tick(None)
        .expect("owner retains whole fault outcome without duplicate runtime fallback");
    let snapshot = ctrl
        .supervisor()
        .reference_commit_snapshot(&handle)
        .expect("retained commit");
    let receive = snapshot.receive.expect("actual sole tick report");
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
        "no second operational drain"
    );
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), writes + 15);
    assert_eq!(
        snapshot.phase,
        ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::SafetyHazard)
    );
    pause.release();
    let start = Instant::now();
    while ctrl.supervisor().reference_work_pending() {
        ctrl.tick(None).expect("one owner advance");
        assert!(start.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(1));
    }
    let completed = ctrl
        .supervisor()
        .reference_commit_snapshot(&handle)
        .expect("actual completion retained");
    assert_eq!(completed.phase, snapshot.phase);
    assert!(matches!(
        completed.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), writes + 15);
}

#[test]
fn cancelled_accepted_disk_work_still_advances_and_discards_new_unpermitted_intent() {
    let tree = resource();
    let pause = JournalTestPause::new(JournalPausePoint::BeforePublication, 1).expect("gate");
    let (mut ctrl, handle) = setup(&tree, &pause);
    assert!(pause.wait_paused(WAIT));
    ctrl.supervisor_mut()
        .cancel_reference_commit(&handle, ReferenceCancelReason::Operator)
        .expect("cancel eligibility only");
    ctrl.set_torque_cmd(TARGET, 0.5)
        .expect("ordinary disabled intent remains separate from output permission");
    assert_eq!(ctrl.torque_cmd(TARGET), 0.5);
    let writes = ctrl.supervisor().bus().transmissions().len();
    ctrl.tick(None)
        .expect("pending disk work still belongs to owner tick");
    assert_eq!(ctrl.torque_cmd(TARGET), 0.0);
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    assert_eq!(ctrl.supervisor().bus().transmissions().len(), writes);
    pause.release();
    let start = Instant::now();
    while ctrl.supervisor().reference_work_pending() {
        ctrl.tick(None).expect("real completion advance");
        assert!(start.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        ctrl.supervisor()
            .reference_commit_snapshot(&handle)
            .expect("late retained disk outcome")
            .phase,
        ReferenceCommitPhase::Cancelled(ReferenceCancelReason::Operator)
    );
    assert!(ctrl
        .supervisor_mut()
        .enable_targets(&[TARGET.into()])
        .is_err());
}
