//! Real acquisition/worker ordering and refusal before admission.
#![allow(clippy::expect_used)]

use super::*;
use crate::reference_journal_tests::support;
use crate::simulation::{
    ReferenceProofMode, ReferenceReplyRule, SimulationBus, SimulationReceive, TxMatcher,
    TxOccurrence, TxRule,
};
use crate::{ReferenceCause, ReferenceRequest};
use robstride::{CanFrame, MotorAddress, ReceiveCompletion, ReceivedCanFrame};
use std::path::PathBuf;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(5);

struct Owner(Supervisor<SimulationBus>);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.cancel_reference_for_shutdown();
        self.0.drain_reference_journal_until(Instant::now() + WAIT);
    }
}

fn actual_owner() -> (support::FixtureTree, Owner, ReferenceHandle) {
    let tree = support::FixtureTree::new(
        "commit-ordering",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    );
    let control = tree.path().join("config/control.yaml");
    let text = std::fs::read_to_string(&control).expect("owned policy");
    std::fs::write(
        control,
        text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("isolated diagnostics policy");
    let mut owner = Owner(
        Supervisor::from_simulation_with_reference_journal(
            tree.path(),
            SimulationBus::default(),
            tree.path().join("history.yaml"),
            tree.path().join("journal.sqlite3"),
        )
        .expect("explicit unreferenced owner"),
    );
    let raw = CanFrame {
        id: 0x0280_04fd,
        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
        extended: true,
    };
    let received = ReceivedCanFrame::full_data(Some("can0".into()), raw);
    owner
        .0
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                device_id: Some(4),
                interface: Some("can0".into()),
            },
            occurrence: TxOccurrence::Every,
            receive: vec![SimulationReceive::Received(received.clone())],
            send_error: None,
        })
        .expect("literal Enable status");
    owner
        .0
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress {
                interface: "can0".into(),
                device_id: 4,
            },
            occurrence: TxOccurrence::Every,
            frame: received,
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("closed actual SetZero/pop causality");
    let stamp = owner
        .0
        .reference_snapshot()
        .next_stamp
        .expect("current stamp");
    let acquisition = owner
        .0
        .begin_reference(ReferenceRequest {
            stamp,
            joint: "right_elbow_pitch".into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("actual acquisition");
    for _ in 0..6 {
        owner
            .0
            .advance_reference(&acquisition)
            .expect("actual owner advance");
    }
    assert_eq!(
        owner
            .0
            .reference_snapshot()
            .terminal
            .expect("terminal")
            .cause,
        ReferenceCause::EvidenceStaged
    );
    (tree, owner, acquisition)
}

fn audit() -> ReferenceAudit {
    ReferenceAudit {
        operator: "fixture".into(),
        session: "ordering".into(),
    }
}

fn join_actual_worker(owner: &mut Owner) -> ReferenceJournalDrain {
    // Join only the actual worker: retain owner eligibility and its unconsumed
    // completion so the next advance must order fresh hazards before collection.
    owner
        .0
        .reference_commits
        .journal
        .as_mut()
        .expect("journal installed")
        .wait_until(Instant::now() + WAIT)
}

#[test]
fn completed_unconsumed_write_still_loses_to_one_fresh_bounded_fault_report() {
    let (_tree, mut owner, acquisition) = actual_owner();
    let handle = owner
        .0
        .begin_reference_commit(&acquisition, audit())
        .expect("accepted job");
    let queued = join_actual_worker(&mut owner);
    assert!(queued.worker_terminated);
    assert_eq!(
        (
            queued.completed_unconsumed,
            queued.accepted_credits,
            queued.durable_writes
        ),
        (1, 1, 1)
    );
    assert_eq!(
        owner
            .0
            .reference_commit_snapshot(&handle)
            .expect("pending owner")
            .phase,
        ReferenceCommitPhase::Pending
    );
    let writes = owner.0.bus().transmissions().len();
    for ordinal in 0..65 {
        owner
            .0
            .bus_mut()
            .queue_frame(CanFrame {
                id: if ordinal == 1 {
                    0x0281_02fd
                } else {
                    0x1f00_04fd
                },
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                extended: true,
            })
            .expect("literal fresh ordered peer fault/noise");
    }
    let result = owner
        .0
        .advance_reference_commit(&handle)
        .expect("real owner collection");
    assert_eq!(
        result.phase,
        ReferenceCommitPhase::Invalidated(ReferenceStageInvalidation::SafetyHazard)
    );
    assert!(matches!(
        result.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    let receive = result.receive.expect("actual fresh report");
    assert_eq!(
        (
            receive.raw_frames,
            receive.read_attempts,
            receive.completion
        ),
        (64, 64, ReceiveCompletion::WorkLimit)
    );
    assert_eq!(owner.0.bus().pending_receive_count(), 1);
    assert_eq!(owner.0.bus().transmissions().len(), writes + 15);
    assert!(!result.usable_reference);
}

#[test]
fn exhausted_commit_identity_refuses_actual_stage_before_disk_admission() {
    let (tree, mut owner, acquisition) = actual_owner();
    owner.0.reference_commits.next_sequence = u64::MAX;
    let refused = owner.0.begin_reference_commit(&acquisition, audit());
    let queued = join_actual_worker(&mut owner);
    assert!(queued.is_complete());
    assert!(matches!(
        refused,
        Err(ReferenceCommitError::CounterExhausted)
    ));
    assert_eq!(
        owner.0.reference_snapshot().staged_evidence,
        ReferenceStageStatus::CurrentVirtualEvidence
    );
    assert!(!owner.0.reference_work_pending());
    assert!(!tree.path().join("journal.sqlite3").exists());
}

#[test]
fn foreign_commit_handle_cannot_consume_feedback_or_admit_neighbor_work() {
    let (_first_tree, mut first, acquired) = actual_owner();
    let foreign = first
        .0
        .begin_reference_commit(&acquired, audit())
        .expect("first actual job");
    assert!(join_actual_worker(&mut first).worker_terminated);
    let (tree, mut second, acquired) = actual_owner();
    second
        .0
        .bus_mut()
        .queue_frame(CanFrame {
            id: 0x1f00_04fd,
            data: [0; 8],
            extended: true,
        })
        .expect("actual neighboring raw input");
    let writes = second.0.bus().transmissions().len();
    let refused = second.0.advance_reference_commit(&foreign);
    assert!(matches!(
        refused,
        Err(ReferenceCommitError::ForeignIdentity)
    ));
    assert_eq!(second.0.bus().pending_receive_count(), 1);
    assert_eq!(second.0.bus().transmissions().len(), writes);
    assert!(!tree.path().join("journal.sqlite3").exists());
    let genuine = second
        .0
        .begin_reference_commit(&acquired, audit())
        .expect("healthy neighboring stage");
    assert!(join_actual_worker(&mut second).worker_terminated);
    let completed = second
        .0
        .advance_reference_commit(&genuine)
        .expect("genuine matching completion");
    assert_eq!(completed.phase, ReferenceCommitPhase::Complete);
    assert_eq!(completed.receive.expect("own report").raw_frames, 1);
    assert!(matches!(
        completed.journal,
        ReferenceJournalResult::DurableHistory { .. }
    ));
    assert!(!completed.usable_reference);
}
