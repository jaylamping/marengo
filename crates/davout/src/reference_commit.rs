//! Owner-bound virtual history lifecycle. Disk history never grants output.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use robstride::MotorBus;
use serde::{Deserialize, Serialize};

use crate::feedback_consumer::{
    AdvanceReceiveBudget, ReceiveContext, ReferenceReceiveContext, ReferenceReceivePhase,
};
use crate::reference_journal::{
    JobIdentity, Journal, ReferenceJournalDrain, ReferenceJournalError, ReferenceJournalResult,
};
use crate::reference_transaction::RetainedOutcome;
use crate::{
    DavoutError, ReferenceCancelReason, ReferenceError, ReferenceHandle, ReferenceReceiveSummary,
    ReferenceStageInvalidation, ReferenceStageStatus, Supervisor,
};

const OUTCOME_CAPACITY: usize = 8;

/// Bounded operator/session labels for inspection, never a credential or permit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceAudit {
    pub operator: String,
    pub session: String,
}
impl ReferenceAudit {
    pub(super) fn validate(&self) -> bool {
        [&self.operator, &self.session].into_iter().all(|label| {
            !label.is_empty() && label.len() <= 128 && !label.chars().any(char::is_control)
        })
    }
}

#[derive(Debug, Clone)]
pub struct ReferenceCommitHandle {
    owner: Arc<()>,
    sequence: u64,
}
impl PartialEq for ReferenceCommitHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner) && self.sequence == other.sequence
    }
}
impl Eq for ReferenceCommitHandle {}

/// Lifecycle and current eligibility are separate from the actual disk outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCommitPhase {
    Pending,
    Complete,
    Cancelled(ReferenceCancelReason),
    Invalidated(ReferenceStageInvalidation),
}

#[derive(Debug, Clone)]
pub struct ReferenceCommitSnapshot {
    pub handle: ReferenceCommitHandle,
    pub acquisition: ReferenceHandle,
    pub joint: String,
    pub phase: ReferenceCommitPhase,
    pub eligibility: ReferenceStageStatus,
    pub journal: ReferenceJournalResult,
    pub receive: Option<ReferenceReceiveSummary>,
    pub usable_reference: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ReferenceCommitError {
    #[error("virtual reference journal is unsupported by this owner")]
    Unsupported,
    #[error("reference owner is busy")]
    Busy,
    #[error("reference journal completion credits full; admission may be retried")]
    QueueFull,
    #[error("commit identity belongs to another owner")]
    ForeignIdentity,
    #[error("commit outcome has expired from the bounded cache")]
    OutcomeExpired,
    #[error("acquisition already has a commit with a different audit body")]
    ConflictingRetry,
    #[error(
        "operator and session labels must be nonempty, bounded and contain no control characters"
    )]
    Audit,
    #[error("commit identity counter exhausted")]
    CounterExhausted,
    #[error("retained evidence is ineligible: {0:?}")]
    Ineligible(ReferenceStageStatus),
    #[error(transparent)]
    Acquisition(#[from] ReferenceError),
    #[error(transparent)]
    Journal(#[from] ReferenceJournalError),
}

struct Entry {
    handle: ReferenceCommitHandle,
    job: JobIdentity,
    stage: Rc<RetainedOutcome>,
    audit: ReferenceAudit,
    phase: Cell<ReferenceCommitPhase>,
    result: ReferenceJournalResult,
    receive: Option<ReferenceReceiveSummary>,
}
pub(super) struct CommitOwner {
    identity: Arc<()>,
    next_sequence: u64,
    journal: Option<Journal>,
    pending: VecDeque<Entry>,
    outcomes: VecDeque<Entry>,
}
impl Default for CommitOwner {
    fn default() -> Self {
        Self {
            identity: Arc::new(()),
            next_sequence: 1,
            journal: None,
            pending: VecDeque::new(),
            outcomes: VecDeque::new(),
        }
    }
}
impl CommitOwner {
    pub(super) fn install(&mut self, journal: Journal) {
        self.journal = Some(journal);
    }
    pub(super) fn busy(&self) -> bool {
        self.pending
            .iter()
            .any(|entry| entry.phase.get() == ReferenceCommitPhase::Pending)
    }
    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.pending.iter().chain(&self.outcomes)
    }
    fn get(&self, handle: &ReferenceCommitHandle) -> Result<&Entry, ReferenceCommitError> {
        if !Arc::ptr_eq(&handle.owner, &self.identity) {
            return Err(ReferenceCommitError::ForeignIdentity);
        }
        self.entries()
            .find(|entry| entry.handle == *handle)
            .ok_or(ReferenceCommitError::OutcomeExpired)
    }
    fn collect(&mut self, handle: &ReferenceCommitHandle) {
        let Some(index) = self
            .pending
            .iter()
            .position(|entry| entry.handle == *handle)
        else {
            return;
        };
        let Some(completion) = self
            .journal
            .as_ref()
            .and_then(|journal| journal.take_matching(&self.pending[index].job))
        else {
            return;
        };
        let Some(mut entry) = self.pending.remove(index) else {
            return;
        };
        entry.result = completion.result;
        if entry.phase.get() == ReferenceCommitPhase::Pending {
            entry.phase.set(ReferenceCommitPhase::Complete);
        }
        if !matches!(entry.result, ReferenceJournalResult::DurableHistory { .. }) {
            entry
                .stage
                .retire(ReferenceStageInvalidation::CommitRetired);
        }
        if self.outcomes.len() == OUTCOME_CAPACITY {
            self.outcomes.pop_front();
        }
        self.outcomes.push_back(entry);
    }
}

impl<B: MotorBus> Supervisor<B> {
    pub(super) fn observe_reference_commits(&self) {
        for entry in self.reference_commits.entries() {
            if entry.phase.get() == ReferenceCommitPhase::Pending {
                if let ReferenceStageStatus::Invalidated(reason) =
                    self.retained_stage_status(&entry.stage)
                {
                    entry.phase.set(ReferenceCommitPhase::Invalidated(reason));
                }
            }
        }
    }
    pub(super) fn invalidate_reference_commits(&self, reason: ReferenceStageInvalidation) {
        for entry in self.reference_commits.entries() {
            entry.stage.retire(reason);
            if entry.phase.get() == ReferenceCommitPhase::Pending {
                entry.phase.set(ReferenceCommitPhase::Invalidated(reason));
            }
        }
    }
    pub(super) fn cancel_reference_commits(&self, reason: ReferenceCancelReason) {
        for entry in self.reference_commits.entries() {
            entry
                .stage
                .retire(if reason == ReferenceCancelReason::Shutdown {
                    ReferenceStageInvalidation::Shutdown
                } else {
                    ReferenceStageInvalidation::CommitRetired
                });
            if entry.phase.get() == ReferenceCommitPhase::Pending {
                entry.phase.set(ReferenceCommitPhase::Cancelled(reason));
            }
        }
    }

    /// Accept one immutable worker job from this owner's actual retained evidence.
    /// This performs no filesystem work and creates no usable reference.
    pub fn begin_reference_commit(
        &mut self,
        acquisition: &ReferenceHandle,
        audit: ReferenceAudit,
    ) -> Result<ReferenceCommitHandle, ReferenceCommitError> {
        let journal = self
            .reference_commits
            .journal
            .as_ref()
            .ok_or(ReferenceCommitError::Unsupported)?;
        if !audit.validate() {
            return Err(ReferenceCommitError::Audit);
        }
        if let Some(entry) = self
            .reference_commits
            .entries()
            .find(|entry| entry.stage.handle() == acquisition)
        {
            return if entry.audit == audit {
                Ok(entry.handle.clone())
            } else {
                Err(ReferenceCommitError::ConflictingRetry)
            };
        }
        self.observe_reference_commits();
        if self.acquisition_busy() || self.reference_commits.busy() {
            return Err(ReferenceCommitError::Busy);
        }
        let stage = self.stage_for_commit(acquisition)?;
        let status = self.retained_stage_status(&stage);
        if status != ReferenceStageStatus::CurrentVirtualEvidence {
            return Err(ReferenceCommitError::Ineligible(status));
        }
        if !journal.has_credit()? {
            return Err(ReferenceCommitError::QueueFull);
        }
        let next = self
            .reference_commits
            .next_sequence
            .checked_add(1)
            .ok_or(ReferenceCommitError::CounterExhausted)?;
        let input = stage.journal_input(audit.clone())?;
        let handle = ReferenceCommitHandle {
            owner: Arc::clone(&self.reference_commits.identity),
            sequence: self.reference_commits.next_sequence,
        };
        let job = JobIdentity {
            nonce: Arc::new(()),
            sequence: handle.sequence,
        };
        journal.submit(job.clone(), input)?;
        self.reference_commits.next_sequence = next;
        self.reference_commits.pending.push_back(Entry {
            handle: handle.clone(),
            job,
            stage,
            audit,
            phase: Cell::new(ReferenceCommitPhase::Pending),
            result: ReferenceJournalResult::Pending,
            receive: None,
        });
        Ok(handle)
    }

    pub fn reference_commit_snapshot(
        &self,
        handle: &ReferenceCommitHandle,
    ) -> Result<ReferenceCommitSnapshot, ReferenceCommitError> {
        self.observe_reference_commits();
        let entry = self.reference_commits.get(handle)?;
        Ok(ReferenceCommitSnapshot {
            handle: entry.handle.clone(),
            acquisition: entry.stage.handle().clone(),
            joint: entry.stage.joint().into(),
            phase: entry.phase.get(),
            eligibility: self.retained_stage_status(&entry.stage),
            journal: entry.result.clone(),
            receive: entry.receive,
            usable_reference: false,
        })
    }

    /// One fresh bounded disabled report precedes consuming a matching completion.
    pub fn advance_reference_commit(
        &mut self,
        handle: &ReferenceCommitHandle,
    ) -> Result<ReferenceCommitSnapshot, ReferenceCommitError> {
        self.reference_commits.get(handle)?;
        if self.acquisition_busy() {
            return Err(ReferenceCommitError::Busy);
        }
        if !self
            .reference_commits
            .pending
            .iter()
            .any(|entry| entry.handle == *handle)
        {
            return self.reference_commit_snapshot(handle);
        }
        let joint = self.reference_commits.get(handle)?.stage.joint();
        let motor = self
            .stop_motors
            .iter()
            .find(|motor| motor.joint == joint)
            .ok_or(ReferenceError::OutcomeExpired)?;
        let context = ReceiveContext::Reference(ReferenceReceiveContext::from_installed_target(
            motor,
            ReferenceReceivePhase::BeforeEnable,
        ));
        let report = AdvanceReceiveBudget::new(None)
            .acquire(&mut self.bus, &self.motor_types)
            .map_err(DavoutError::from)
            .map_err(ReferenceError::from)?;
        let consumption = self.consume_report_in_context(report, &context);
        if let Some(entry) = self
            .reference_commits
            .pending
            .iter_mut()
            .find(|entry| entry.handle == *handle)
        {
            entry.receive = Some(ReferenceReceiveSummary {
                completion: consumption.completion,
                raw_frames: consumption.raw_frames,
                read_attempts: consumption.read_attempts,
            });
        }
        if consumption.first_transition {
            let _ = self.perform_stop(false);
        }
        if consumption.first_error.is_some()
            || !consumption.completion.is_complete()
            || self.has_latched_fault()
            || self.hardware_estop
        {
            self.invalidate_reference_commits(ReferenceStageInvalidation::SafetyHazard);
        }
        // Deadline equality and every captured continuity check run before collect.
        self.observe_reference_commits();
        self.reference_commits.collect(handle);
        self.reference_commit_snapshot(handle)
    }

    pub fn cancel_reference_commit(
        &mut self,
        handle: &ReferenceCommitHandle,
        reason: ReferenceCancelReason,
    ) -> Result<ReferenceCommitSnapshot, ReferenceCommitError> {
        let entry = self.reference_commits.get(handle)?;
        entry
            .stage
            .retire(if reason == ReferenceCancelReason::Shutdown {
                ReferenceStageInvalidation::Shutdown
            } else {
                ReferenceStageInvalidation::CommitRetired
            });
        if entry.phase.get() == ReferenceCommitPhase::Pending {
            entry.phase.set(ReferenceCommitPhase::Cancelled(reason));
        }
        self.reference_commit_snapshot(handle)
    }

    pub fn reference_work_pending(&self) -> bool {
        self.acquisition_busy() || !self.reference_commits.pending.is_empty()
    }
    /// The controller advances the owner once, never a second operational drain.
    pub fn advance_reference_work(&mut self) -> Result<(), ReferenceCommitError> {
        if let Some(handle) = self
            .reference_snapshot()
            .handle
            .filter(|_| self.acquisition_busy())
        {
            self.advance_reference(&handle)?;
        } else if let Some(handle) = self
            .reference_commits
            .pending
            .front()
            .map(|entry| entry.handle.clone())
        {
            self.advance_reference_commit(&handle)?;
        }
        Ok(())
    }
    pub fn close_reference_journal_admission(&self) {
        if let Some(journal) = &self.reference_commits.journal {
            journal.close();
        }
    }
    /// Shutdown-only collection: eligibility is retired before storage waits.
    pub fn drain_reference_journal_until(&mut self, deadline: Instant) -> ReferenceJournalDrain {
        self.cancel_reference_commits(ReferenceCancelReason::Shutdown);
        let Some(journal) = self.reference_commits.journal.as_mut() else {
            return ReferenceJournalDrain::absent();
        };
        journal.wait_until(deadline);
        let handles: Vec<_> = self
            .reference_commits
            .pending
            .iter()
            .map(|entry| entry.handle.clone())
            .collect();
        for handle in handles {
            self.reference_commits.collect(&handle);
        }
        self.reference_commits
            .journal
            .as_mut()
            .map_or_else(ReferenceJournalDrain::absent, |journal| {
                journal.wait_until(Instant::now())
            })
    }
}

#[cfg(test)]
#[path = "reference_commit_tests.rs"]
mod tests;
