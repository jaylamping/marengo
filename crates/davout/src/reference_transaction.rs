//! Bounded owner-local reference acquisition: the sealed virtual backend and the
//! qualified physical Robstride backend (ADR 0026, ADR 0036). No grant here;
//! permission is selected only by the durable commit owner.

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use robstride::{BusError, DeviceUid, MotorAddress, MotorBus, ParameterId};

use crate::burst::BurstPacer;
use crate::feedback_consumer::{
    AdvanceReceiveBudget, ReceiveContext, ReferenceReceiveContext, ReferenceReceivePhase,
};
use crate::reference_model::InstalledModelStamp;
use crate::reference_physical::{PhysicalBackend, PhysicalDevices};
use crate::{
    ControlMode, OperationalMode, Supervisor, POST_SET_ZERO_BLACKOUT_FROM, POST_SET_ZERO_QUIET,
};

use crate::{DavoutError, StopReport};

const TERMINAL_CAPACITY: usize = 8;
const POLICY_CAPACITY: usize = 262_144;
const PHASE_TIMEOUT: Duration = Duration::from_secs(2);

/// Issued by one owner. Cloning retries a request; it cannot grant motion.
#[derive(Debug, Clone)]
pub struct ReferenceStamp {
    owner: Arc<()>,
    sequence: u64,
    stop_generation: u64,
    reference_generation: u64,
    installed_model: InstalledModelStamp,
    policy: serde_json::Value,
}

/// Opaque identity for the same owner reservation and retained outcome.
#[derive(Debug, Clone)]
pub struct ReferenceHandle {
    owner: Arc<()>,
    sequence: u64,
}

impl PartialEq for ReferenceHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.owner, &other.owner) && self.sequence == other.sequence
    }
}
impl Eq for ReferenceHandle {}

#[derive(Debug, Clone)]
pub struct ReferenceRequest {
    pub stamp: ReferenceStamp,
    pub joint: String,
    pub confirmed: bool,
    pub sign_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferencePhase {
    Idle,
    BaselineStop,
    DrainOld,
    /// Physical: request the target's type-0 MCU identifier before arming.
    RequestIdentity,
    AwaitIdentity,
    /// Physical, echoing bus: the target's type-24 Off read back from the wire
    /// at least one control period ago, so no pre-Enable report can follow the
    /// Enable's echo.
    AwaitReportingOff,
    ArmTarget,
    DrainPostArm,
    SetZero,
    AwaitEvidence,
    /// Physical: a type-2 status reply popped after the addressed SetZero.
    AwaitAck,
    /// Physical: request the type-17 `mechPos` (0x7019) readback after the ack.
    RequestReadback,
    AwaitReadback,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCancelReason {
    Operator,
    Disable,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceFailureKind {
    Delivery,
    Reporting,
    Hazard,
    BindingChanged,
    IncompleteReceive,
    Backend,
    /// Physical identity was absent, ambiguous or changed.
    Identity,
    /// The requested post-command readback refused or was outside tolerance.
    Readback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceCause {
    EvidenceStaged,
    Cancelled(ReferenceCancelReason),
    TimedOut,
    Failed {
        kind: ReferenceFailureKind,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCommit {
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceReportingAttempt {
    pub joint: String,
    pub address: MotorAddress,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceReceiveSummary {
    pub completion: robstride::ReceiveCompletion,
    pub raw_frames: usize,
    pub read_attempts: usize,
}

/// Immutable acquisition result. Accepted writes are not physical acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTerminal {
    pub handle: ReferenceHandle,
    pub joint: String,
    pub cause: ReferenceCause,
    pub stop: StopReport,
    pub reporting: Vec<ReferenceReportingAttempt>,
    pub receive: Option<ReferenceReceiveSummary>,
    pub commit: ReferenceCommit,
    pub usable_reference: bool,
}

/// Inspection of retained evidence, never readiness or output permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceStageStatus {
    NoEvidence,
    CurrentEvidence,
    Invalidated(ReferenceStageInvalidation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceStageInvalidation {
    BindingChanged,
    StopChanged,
    DeadlineExpired,
    SafetyHazard,
    StopUncertain,
    Superseded,
    Shutdown,
    CounterExhausted,
    CommitRetired,
    Consumed,
}

#[derive(Debug, Clone)]
pub struct ReferenceSnapshot {
    pub phase: ReferencePhase,
    pub joint: Option<String>,
    pub handle: Option<ReferenceHandle>,
    pub next_stamp: Option<ReferenceStamp>,
    pub remaining: Option<Duration>,
    pub reference_armed: bool,
    pub usable_reference: bool,
    pub terminal: Option<ReferenceTerminal>,
    pub receive: Option<ReferenceReceiveSummary>,
    pub staged_evidence: ReferenceStageStatus,
}

#[derive(Debug, thiserror::Error)]
pub enum ReferenceError {
    #[error("qualified reference acquisition is unsupported by this owner")]
    Unsupported,
    #[error("invalid reference request: {message}")]
    InvalidRequest { message: String },
    #[error("reference owner already has a reservation")]
    Busy,
    #[error("reference identity belongs to another owner")]
    ForeignIdentity,
    #[error("reference request stamp is stale")]
    StaleStamp,
    #[error("reference request identity was reused with a different body")]
    ConflictingRetry,
    #[error("reference outcome has expired from the bounded cache")]
    OutcomeExpired,
    #[error("reference identity counter is exhausted")]
    CounterExhausted,
    #[error(transparent)]
    Admission(#[from] DavoutError),
}

/// Private correlation for accepted evidence. The closed virtual device attaches
/// it to an actual raw pop; the physical backend builds it from the requested
/// readback reply it popped. Public raw/typed queue injection never creates it.
pub(crate) struct ReferenceCorrelation {
    pub(crate) owner: Arc<()>,
    pub(crate) realm: Arc<()>,
    pub(crate) transaction: u64,
    pub(crate) device_epoch: u64,
    pub(crate) address: MotorAddress,
    pub(crate) order: usize,
    pub(crate) can_id: u32,
    pub(crate) received_at: Instant,
}

/// Only the concrete SimulationBus specialization installs these fixed private
/// functions. No public MotorBus capability hook or caller callback is accepted.
pub(crate) struct VirtualBackend<B: MotorBus> {
    pub(crate) realm: Arc<()>,
    pub(crate) matches: fn(&B, &Arc<()>) -> bool,
    pub(crate) now: fn(&B) -> Duration,
    pub(crate) begin: fn(&mut B, Arc<()>, u64, MotorAddress) -> Result<(), BusError>,
    pub(crate) end: fn(&mut B),
    pub(crate) prepare_report: fn(&mut B),
    pub(crate) take_proofs: fn(&mut B) -> Vec<ReferenceCorrelation>,
    pub(crate) device_epoch: fn(&B) -> Option<u64>,
    pub(crate) current_device_epoch: fn(&B, &MotorAddress) -> Option<u64>,
}

impl<B: MotorBus> Clone for VirtualBackend<B> {
    fn clone(&self) -> Self {
        Self {
            realm: Arc::clone(&self.realm),
            matches: self.matches,
            now: self.now,
            begin: self.begin,
            end: self.end,
            prepare_report: self.prepare_report,
            take_proofs: self.take_proofs,
            device_epoch: self.device_epoch,
            current_device_epoch: self.current_device_epoch,
        }
    }
}

/// Acquisition capability installed only by explicit factories: the sealed
/// simulation realm or the physical Robstride protocol owner (ADR 0036).
pub(crate) enum ReferenceBackend<B: MotorBus> {
    Virtual(VirtualBackend<B>),
    Physical(PhysicalBackend),
}

impl<B: MotorBus> Clone for ReferenceBackend<B> {
    fn clone(&self) -> Self {
        match self {
            Self::Virtual(backend) => Self::Virtual(backend.clone()),
            Self::Physical(backend) => Self::Physical(backend.clone()),
        }
    }
}

impl<B: MotorBus> ReferenceBackend<B> {
    pub(crate) fn realm(&self) -> &Arc<()> {
        match self {
            Self::Virtual(backend) => &backend.realm,
            Self::Physical(backend) => &backend.realm,
        }
    }

    /// The physical owner's bus cannot be replaced, so its realm always matches.
    fn matches(&self, bus: &B) -> bool {
        match self {
            Self::Virtual(backend) => (backend.matches)(bus, &backend.realm),
            Self::Physical(_) => true,
        }
    }

    fn now(&self, bus: &B) -> Duration {
        match self {
            Self::Virtual(backend) => (backend.now)(bus),
            Self::Physical(backend) => backend.now(),
        }
    }

    fn is_physical(&self) -> bool {
        matches!(self, Self::Physical(_))
    }
}

/// Physical ack accepted from a type-2 status popped after the addressed SetZero.
#[derive(Debug, Clone, Copy)]
struct PhysicalAck {
    order: usize,
    can_id: u32,
    position_rad: f32,
}

#[derive(Clone)]
struct Reservation {
    request: ReferenceRequest,
    handle: ReferenceHandle,
    address: MotorAddress,
    policy: serde_json::Value,
    typed_policy: Arc<crate::reference_journal_event::TypedPolicy>,
    installed_model: InstalledModelStamp,
    reference_generation: u64,
    stop_generation: u64,
    binding_invalidated: Cell<bool>,
    phase: ReferencePhase,
    overall_deadline: Duration,
    phase_deadline: Duration,
    armed: bool,
    device_epoch: Option<u64>,
    reporting: Vec<ReferenceReportingAttempt>,
    receive: Option<ReferenceReceiveSummary>,
    /// Physical: only replies popped after this watermark answer the last request.
    request_watermark: u64,
    uid: Option<DeviceUid>,
    ack: Option<PhysicalAck>,
}

/// Created only from an admitted pose/reply and its exact private correlation.
struct AcceptedReferenceEvidence {
    proof: ReferenceCorrelation,
    address: MotorAddress,
    order: usize,
    can_id: u32,
    received_at: Instant,
    position_rad: f32,
}

/// Physical evidence beside the readback correlation, retained for history.
#[derive(Debug, Clone, Copy)]
pub(super) struct PhysicalEvidence {
    pub(super) uid: DeviceUid,
    pub(super) ack_order: usize,
    pub(super) ack_can_id: u32,
    pub(super) ack_position_rad: f32,
}

struct RetainedStage {
    evidence: AcceptedReferenceEvidence,
    physical: Option<PhysicalEvidence>,
    installed_model: InstalledModelStamp,
    overall_deadline: Duration,
    reference_generation: u64,
    invalidated: Cell<Option<ReferenceStageInvalidation>>,
    typed_policy: Arc<crate::reference_journal_event::TypedPolicy>,
}

pub(super) struct RetainedOutcome {
    request: ReferenceRequest,
    terminal: ReferenceTerminal,
    stage: Option<RetainedStage>,
}

/// One owner request driven to a durable commit by the controller tick.
pub(super) struct PendingCommit {
    pub(super) acquisition: ReferenceHandle,
    pub(super) audit: crate::ReferenceAudit,
}

const REQUEST_FAILURE_CAPACITY: usize = 8;

pub(crate) struct ReferenceOwner<B: MotorBus> {
    identity: Arc<()>,
    next_sequence: u64,
    backend: Option<ReferenceBackend<B>>,
    reservation: Option<Reservation>,
    outcomes: VecDeque<Rc<RetainedOutcome>>,
    pub(super) physical: PhysicalDevices,
    pub(super) pending_commit: Option<PendingCommit>,
    request_failures: VecDeque<(ReferenceHandle, String)>,
}

impl<B: MotorBus> Default for ReferenceOwner<B> {
    fn default() -> Self {
        Self {
            identity: Arc::new(()),
            next_sequence: 1,
            backend: None,
            reservation: None,
            outcomes: VecDeque::new(),
            physical: PhysicalDevices::default(),
            pending_commit: None,
            request_failures: VecDeque::new(),
        }
    }
}

impl<B: MotorBus> ReferenceOwner<B> {
    pub(crate) fn install_backend(&mut self, backend: ReferenceBackend<B>) {
        self.backend = Some(backend);
    }

    pub(crate) fn install_physical(&mut self, backend: PhysicalBackend, devices: PhysicalDevices) {
        self.backend = Some(ReferenceBackend::Physical(backend));
        self.physical = devices;
    }

    pub(super) fn has_backend(&self) -> bool {
        self.backend.is_some()
    }

    pub(super) fn is_physical(&self) -> bool {
        self.backend
            .as_ref()
            .is_some_and(ReferenceBackend::is_physical)
    }

    /// The authority realm must be this owner's installed backend realm.
    pub(super) fn realm_matches(&self, bus: &B, realm: &Arc<()>) -> bool {
        self.backend
            .as_ref()
            .is_some_and(|backend| Arc::ptr_eq(backend.realm(), realm) && backend.matches(bus))
    }

    /// Coordinate epoch without liveness: virtual device epoch or physical continuity.
    pub(super) fn current_device_epoch(&self, bus: &B, address: &MotorAddress) -> Option<u64> {
        match self.backend.as_ref()? {
            ReferenceBackend::Virtual(backend) => (backend.current_device_epoch)(bus, address),
            ReferenceBackend::Physical(_) => self.physical.epoch(address),
        }
    }

    /// Epoch of a selected grant. Physical grants also require liveness (ADR 0036).
    /// `withheld_since`: start of an Active session that still withholds this
    /// address's traffic from session pose (its Enable echo is pending).
    pub(super) fn live_device_epoch(
        &self,
        bus: &B,
        address: &MotorAddress,
        window: Duration,
        owner_busy: bool,
        withheld_since: Option<Instant>,
    ) -> Option<u64> {
        match self.backend.as_ref()? {
            ReferenceBackend::Virtual(backend) => (backend.current_device_epoch)(bus, address),
            ReferenceBackend::Physical(_) => self.physical.live_epoch(
                address,
                Instant::now(),
                window,
                owner_busy,
                withheld_since,
            ),
        }
    }

    pub(super) fn mark_owner_work(&mut self) {
        if self.is_physical() {
            self.physical.mark_owner_work(Instant::now());
        }
    }

    pub(super) fn record_request_failure(&mut self, handle: ReferenceHandle, message: String) {
        if self.request_failures.len() == REQUEST_FAILURE_CAPACITY {
            self.request_failures.pop_front();
        }
        self.request_failures.push_back((handle, message));
    }

    pub(super) fn request_failure(&self, handle: &ReferenceHandle) -> Option<&str> {
        self.request_failures
            .iter()
            .rev()
            .find(|(failed, _)| failed == handle)
            .map(|(_, message)| message.as_str())
    }

    #[cfg(test)]
    pub(super) fn evict_outcomes_for_test(&mut self) {
        self.outcomes.clear();
    }
}

impl<B: MotorBus> Supervisor<B> {
    pub fn reference_busy(&self) -> bool {
        self.observe_reference_commits();
        self.acquisition_busy() || self.reference_commits.busy()
    }

    pub(super) fn acquisition_busy(&self) -> bool {
        self.reference_owner.reservation.is_some()
    }

    pub(super) fn refuse_reference_interference(
        &self,
        operation: &'static str,
    ) -> Result<(), DavoutError> {
        if self.reference_busy() {
            Err(DavoutError::ReferenceBusy { operation })
        } else {
            Ok(())
        }
    }

    /// Observe the latest retained stage without transmitting or granting permission.
    /// This deliberately avoids the public reference-generation/binding accessors.
    pub(super) fn observe_staged_reference(&self) -> ReferenceStageStatus {
        self.reference_owner
            .outcomes
            .back()
            .map_or(ReferenceStageStatus::NoEvidence, |outcome| {
                self.reference_stage_status_for(&outcome.terminal.handle)
            })
    }

    fn reference_stage_status_for(&self, handle: &ReferenceHandle) -> ReferenceStageStatus {
        let Some(outcome) = self
            .reference_owner
            .outcomes
            .iter()
            .find(|outcome| outcome.terminal.handle == *handle)
        else {
            return ReferenceStageStatus::NoEvidence;
        };
        self.retained_stage_status(outcome)
    }

    pub(super) fn retained_stage_status(&self, outcome: &RetainedOutcome) -> ReferenceStageStatus {
        let Some(stage) = &outcome.stage else {
            return ReferenceStageStatus::NoEvidence;
        };
        if let Some(reason) = stage.invalidated.get() {
            return ReferenceStageStatus::Invalidated(reason);
        }
        if let Some(reason) = self.staged_reference_invalidation(outcome, stage) {
            stage.invalidated.set(Some(reason));
            ReferenceStageStatus::Invalidated(reason)
        } else {
            ReferenceStageStatus::CurrentEvidence
        }
    }

    fn staged_reference_invalidation(
        &self,
        outcome: &RetainedOutcome,
        stage: &RetainedStage,
    ) -> Option<ReferenceStageInvalidation> {
        use ReferenceStageInvalidation as Reason;

        if outcome.terminal.stop.failed_writes() > 0 {
            return Some(Reason::StopUncertain);
        }
        if self.stop_generation() == u64::MAX || outcome.terminal.stop.generation == u64::MAX {
            return Some(Reason::CounterExhausted);
        }
        if self.reference_owner.reservation.is_some()
            || self
                .reference_owner
                .outcomes
                .back()
                .is_none_or(|latest| latest.terminal.handle != outcome.terminal.handle)
            || outcome.terminal.handle.sequence.checked_add(1)
                != Some(self.reference_owner.next_sequence)
        {
            return Some(Reason::Superseded);
        }
        let Some(backend) = &self.reference_owner.backend else {
            return Some(Reason::BindingChanged);
        };
        if backend.now(&self.bus) >= stage.overall_deadline {
            return Some(Reason::DeadlineExpired);
        }
        if self.has_latched_fault() || self.hardware_estop {
            return Some(Reason::SafetyHazard);
        }
        if self.stop_generation() != outcome.terminal.stop.generation {
            return Some(Reason::StopChanged);
        }
        let evidence = &stage.evidence;
        let proof = &evidence.proof;
        let identity_matches = Arc::ptr_eq(&proof.owner, &self.reference_owner.identity)
            && Arc::ptr_eq(&proof.owner, &outcome.terminal.handle.owner)
            && Arc::ptr_eq(&proof.realm, backend.realm())
            && proof.transaction == outcome.terminal.handle.sequence;
        let pop_matches = proof.address == evidence.address
            && proof.order == evidence.order
            && proof.can_id == evidence.can_id
            && proof.received_at == evidence.received_at;
        let target_matches = self.stop_motors.iter().any(|motor| {
            motor.joint == outcome.request.joint && MotorAddress::from(motor) == evidence.address
        });
        let receive_matches = outcome.terminal.receive.is_some_and(|receive| {
            receive.completion.is_complete()
                && receive.raw_frames <= robstride::MAX_RX_FRAMES_PER_POLL
                && receive.read_attempts <= robstride::MAX_RX_ATTEMPTS_PER_POLL
                && evidence.order < receive.raw_frames
        });
        if outcome.terminal.cause != ReferenceCause::EvidenceStaged
            || !identity_matches
            || !pop_matches
            || !target_matches
            || !receive_matches
            || !evidence.position_rad.is_finite()
            || !backend.matches(&self.bus)
            || self
                .reference_owner
                .current_device_epoch(&self.bus, &evidence.address)
                != Some(proof.device_epoch)
            || stage.physical.is_some_and(|physical| {
                self.reference_owner.physical.uid(&evidence.address) != Some(physical.uid)
            })
            || self.reference_authority.generation() != stage.reference_generation
            || !self.installed_model.matches(&stage.installed_model)
            || self
                .reference_policy()
                .map_or(true, |policy| policy != outcome.request.stamp.policy)
            || !stage
                .typed_policy
                .matches(&self.motors, &self.control, &self.homing_config)
            || self.mode != OperationalMode::Disabled
            || self.control_mode != ControlMode::Disabled
            || !outcome.request.confirmed
            || self
                .homing_config
                .homing
                .effective_joint(&outcome.request.joint)
                .is_none_or(|policy| policy.sign_test_required && !outcome.request.sign_verified)
            || f64::from(evidence.position_rad).abs()
                > self.homing_config.homing.zero_verify_tolerance_rad
        {
            return Some(Reason::BindingChanged);
        }
        None
    }

    fn invalidate_retained_stages(&self, reason: ReferenceStageInvalidation) {
        for outcome in &self.reference_owner.outcomes {
            if let Some(stage) = &outcome.stage {
                if stage.invalidated.get().is_none() {
                    stage.invalidated.set(Some(reason));
                }
            }
        }
    }

    /// Inspection only: neither a stamp nor a terminal can authorize output.
    pub fn reference_snapshot(&self) -> ReferenceSnapshot {
        // Observing a mismatched installed binding permanently revokes INITIAL
        // permission even when policy validation cannot issue a new stamp.
        self.reference_binding_valid();
        let owner = &self.reference_owner;
        let policy = self.reference_policy().ok();
        if let Some(reservation) = &owner.reservation {
            if policy.as_ref() != Some(&reservation.policy)
                || !self.installed_model.matches(&reservation.installed_model)
            {
                // Inspection cannot deliver cleanup, but restoring public fields
                // must not erase an already observed live policy mismatch.
                reservation.binding_invalidated.set(true);
            }
        }
        let next_stamp = if owner.next_sequence.checked_add(1).is_some() {
            policy.map(|policy| ReferenceStamp {
                owner: Arc::clone(&owner.identity),
                sequence: owner.next_sequence,
                stop_generation: self.stop_generation(),
                reference_generation: self.reference_authority.generation(),
                installed_model: self.installed_model.stamp(),
                policy,
            })
        } else {
            None
        };
        if let Some(reservation) = &owner.reservation {
            let now = owner
                .backend
                .as_ref()
                .map_or(Duration::ZERO, |backend| backend.now(&self.bus));
            ReferenceSnapshot {
                phase: reservation.phase,
                joint: Some(reservation.request.joint.clone()),
                handle: Some(reservation.handle.clone()),
                next_stamp,
                remaining: Some(
                    reservation
                        .overall_deadline
                        .min(reservation.phase_deadline)
                        .saturating_sub(now),
                ),
                reference_armed: reservation.armed,
                usable_reference: false,
                terminal: None,
                receive: reservation.receive,
                staged_evidence: ReferenceStageStatus::NoEvidence,
            }
        } else {
            let terminal = owner
                .outcomes
                .back()
                .map(|outcome| outcome.terminal.clone());
            ReferenceSnapshot {
                phase: if terminal.is_some() {
                    ReferencePhase::Terminal
                } else {
                    ReferencePhase::Idle
                },
                joint: terminal.as_ref().map(|outcome| outcome.joint.clone()),
                handle: terminal.as_ref().map(|outcome| outcome.handle.clone()),
                next_stamp,
                remaining: None,
                reference_armed: false,
                usable_reference: false,
                terminal,
                receive: owner
                    .outcomes
                    .back()
                    .and_then(|outcome| outcome.terminal.receive),
                staged_evidence: self.observe_staged_reference(),
            }
        }
    }

    /// Reserve without transmitting. Only the closed concrete virtual backend and
    /// the explicit physical owner factory have this capability (ADR 0036);
    /// ordinary constructors always refuse.
    pub fn begin_reference(
        &mut self,
        request: ReferenceRequest,
    ) -> Result<ReferenceHandle, ReferenceError> {
        let backend = self
            .reference_owner
            .backend
            .clone()
            .ok_or(ReferenceError::Unsupported)?;
        if !Arc::ptr_eq(&request.stamp.owner, &self.reference_owner.identity) {
            return Err(ReferenceError::ForeignIdentity);
        }
        if let Some(outcome) = self
            .reference_owner
            .outcomes
            .iter()
            .find(|outcome| outcome.request.stamp.sequence == request.stamp.sequence)
        {
            return if same_request(&outcome.request, &request) {
                Ok(outcome.terminal.handle.clone())
            } else {
                Err(ReferenceError::ConflictingRetry)
            };
        }
        if let Some(reservation) = &self.reference_owner.reservation {
            if reservation.request.stamp.sequence == request.stamp.sequence {
                return if same_request(&reservation.request, &request) {
                    Ok(reservation.handle.clone())
                } else {
                    Err(ReferenceError::ConflictingRetry)
                };
            }
            return Err(ReferenceError::Busy);
        }
        self.observe_reference_commits();
        if self.reference_commits.busy() || self.reference_owner.pending_commit.is_some() {
            return Err(ReferenceError::Busy);
        }
        if request.stamp.sequence < self.reference_owner.next_sequence {
            return Err(ReferenceError::OutcomeExpired);
        }
        self.reference_binding_valid();
        let next = self
            .reference_owner
            .next_sequence
            .checked_add(1)
            .ok_or(ReferenceError::CounterExhausted)?;
        let policy = self.reference_policy()?;
        if request.stamp.sequence != self.reference_owner.next_sequence
            || request.stamp.stop_generation != self.stop_generation()
            || request.stamp.reference_generation != self.reference_authority.generation()
            || !self.installed_model.matches(&request.stamp.installed_model)
            || request.stamp.policy != policy
        {
            return Err(ReferenceError::StaleStamp);
        }
        if !backend.matches(&self.bus) {
            return Err(ReferenceError::Unsupported);
        }
        if !request.confirmed {
            return Err(ReferenceError::InvalidRequest {
                message: "operator confirmation is required".into(),
            });
        }
        self.reference_request_preflight(&request.joint, Some(request.sign_verified))?;
        if self.mode == OperationalMode::Active {
            return Err(ReferenceError::InvalidRequest {
                message: "disable ordinary motion before acquisition".into(),
            });
        }
        let homing = self
            .homing_config
            .homing
            .effective_joint(&request.joint)
            .ok_or_else(|| ReferenceError::InvalidRequest {
                message: "selected homing policy is absent".into(),
            })?;
        if homing.home_offset_rad != 0.0 || homing.sensors.is_some() {
            return Err(ReferenceError::Unsupported);
        }
        let timeout = Duration::try_from_secs_f64(homing.search_timeout_s).map_err(|_| {
            ReferenceError::InvalidRequest {
                message: "reference timeout is not representable".into(),
            }
        })?;
        if timeout.is_zero() {
            return Err(ReferenceError::InvalidRequest {
                message: "reference timeout rounds to zero".into(),
            });
        }
        let now = backend.now(&self.bus);
        let overall_deadline = now
            .checked_add(timeout)
            .ok_or(ReferenceError::CounterExhausted)?;
        let phase_deadline = phase_deadline(now, overall_deadline, ReferencePhase::BaselineStop)?;
        let address = self
            .stop_motors
            .iter()
            .find(|motor| motor.joint == request.joint)
            .map(MotorAddress::from)
            .ok_or_else(|| ReferenceError::InvalidRequest {
                message: "target is outside installed routes".into(),
            })?;
        let typed_policy = Arc::new(crate::reference_journal_event::TypedPolicy {
            motors: self.motors.clone(),
            control: self.control.clone(),
            homing: self.homing_config.clone(),
        });
        crate::reference_codec::encode(typed_policy.as_ref()).map_err(|error| {
            ReferenceError::InvalidRequest {
                message: crate::bounded_message(&error.to_string()),
            }
        })?;
        // Preflight is complete. No history or replacement permission is created
        // here. Virtual acquisition conservatively revokes all INITIAL coverage;
        // physical acquisition revokes only the target's coordinate binding.
        let reference_generation = if backend.is_physical() {
            self.reference_authority
                .invalidate_joint_for_acquisition(&request.joint)
        } else {
            self.reference_authority.invalidate_for_acquisition()
        }
        .ok_or(ReferenceError::CounterExhausted)?;
        match &backend {
            ReferenceBackend::Virtual(backend) => (backend.begin)(
                &mut self.bus,
                Arc::clone(&self.reference_owner.identity),
                request.stamp.sequence,
                address.clone(),
            )
            .map_err(DavoutError::from)?,
            ReferenceBackend::Physical(_) => self.reference_owner.mark_owner_work(),
        }
        let handle = ReferenceHandle {
            owner: Arc::clone(&self.reference_owner.identity),
            sequence: request.stamp.sequence,
        };
        let installed_model = request.stamp.installed_model.clone();
        self.invalidate_retained_stages(ReferenceStageInvalidation::Superseded);
        self.reference_owner.next_sequence = next;
        self.mode = OperationalMode::Disabled;
        self.control_mode = ControlMode::Disabled;
        self.enable_writes_pending.clear();
        self.enable_write_due = None;
        self.reference_owner.reservation = Some(Reservation {
            request,
            handle: handle.clone(),
            address,
            policy,
            typed_policy,
            installed_model,
            reference_generation,
            stop_generation: self.stop_generation(),
            binding_invalidated: Cell::new(false),
            phase: ReferencePhase::BaselineStop,
            overall_deadline,
            phase_deadline,
            armed: false,
            device_epoch: None,
            reporting: Vec::new(),
            receive: None,
            request_watermark: 0,
            uid: None,
            ack: None,
        });
        Ok(handle)
    }

    /// Exactly one phase and at most one bounded real report per call. A tied
    /// deadline expires before a queued proof can be acquired or accepted.
    ///
    /// Every exit that leaves this call with its reservation still live is
    /// cleaned up here (ADR 0026:62-66): an `Err` from a phase (`?`) ends the
    /// reference with the same all-address stop as any other terminal, and the
    /// original error is still returned. A later `?` added to a phase cannot
    /// leave an armed target behind.
    pub fn advance_reference(
        &mut self,
        handle: &ReferenceHandle,
    ) -> Result<ReferenceSnapshot, ReferenceError> {
        let result = self.advance_reference_phase(handle);
        if let Err(error) = &result {
            let live = self
                .reference_owner
                .reservation
                .as_ref()
                .is_some_and(|reservation| reservation.handle == *handle);
            if live {
                let _ = self.finish_reference(
                    failure(ReferenceFailureKind::Backend, &error.to_string()),
                    None,
                    None,
                );
            }
        }
        result
    }

    fn advance_reference_phase(
        &mut self,
        handle: &ReferenceHandle,
    ) -> Result<ReferenceSnapshot, ReferenceError> {
        if let Some(terminal) = self.retained_reference(handle)? {
            return Ok(self.snapshot_for_terminal(terminal));
        }
        let reservation = self.live_reference(handle)?.clone();
        let backend = self
            .reference_owner
            .backend
            .clone()
            .ok_or(ReferenceError::Unsupported)?;
        self.reference_owner.mark_owner_work();
        let now = backend.now(&self.bus);
        if now >= reservation.overall_deadline || now >= reservation.phase_deadline {
            let terminal = self.finish_reference(ReferenceCause::TimedOut, None, None)?;
            return Ok(self.snapshot_for_terminal(terminal));
        }
        if self.has_latched_fault() || self.hardware_estop {
            return self.fail_reference(ReferenceFailureKind::Hazard, "persistent safety hazard");
        }
        if reservation.binding_invalidated.get()
            || !backend.matches(&self.bus)
            || !self.installed_model.matches(&reservation.installed_model)
            || self.reference_authority.generation() != reservation.reference_generation
            || self.stop_generation() != reservation.stop_generation
            || self
                .reference_policy()
                .map_or(true, |policy| policy != reservation.policy)
            || self.mode != OperationalMode::Disabled
            || self.control_mode != ControlMode::Disabled
        {
            return self.fail_reference(
                ReferenceFailureKind::BindingChanged,
                "installed policy or owner continuity changed",
            );
        }
        let next_phase = match reservation.phase {
            ReferencePhase::BaselineStop => {
                // Stop and Off groups are spaced on a real (echoing) bus: their
                // replies must not overrun the controller's receive buffers.
                let mut pacer = (backend.is_physical() && self.bus.echoes_transmissions())
                    .then(BurstPacer::default);
                let stop = self.perform_stop_paced(false, pacer.as_mut());
                if stop.failed_writes() > 0 {
                    let terminal = self.finish_reference(
                        failure(
                            ReferenceFailureKind::Delivery,
                            "baseline stop was uncertain",
                        ),
                        Some(stop),
                        None,
                    )?;
                    return Ok(self.snapshot_for_terminal(terminal));
                }
                if let Some(live) = &mut self.reference_owner.reservation {
                    live.stop_generation = stop.generation;
                }
                // A physical drive keeps type-24 reporting across host
                // processes, so the target may stream though this process never
                // turned it On. A report the drive built before acting on the
                // Enable would follow the Enable's echo still in Reset and fail
                // the strict Run check (bench 2026-10-03 14:55), so the target
                // is turned Off here and, on an echoing bus, armed only once
                // that Off has settled on the wire (`AwaitReportingOff`).
                let now = Instant::now();
                let gated = backend.is_physical() && self.bus.echoes_transmissions();
                if gated {
                    self.begin_reporting_off_gate(now);
                }
                let unrecorded = backend.is_physical().then_some(&reservation.address);
                // A peer in its possible post-SetZero blackout keeps its stream:
                // an Off it acts on needs an On that the drive would then drop.
                let set_zero_on_wire = &self.set_zero_on_wire;
                let reporting = self.active_reporting.suspend(
                    &mut self.bus,
                    &self.stop_motors,
                    unrecorded,
                    |address| {
                        set_zero_on_wire.get(address).is_some_and(|at| {
                            (POST_SET_ZERO_BLACKOUT_FROM..POST_SET_ZERO_QUIET)
                                .contains(&now.saturating_duration_since(*at))
                        })
                    },
                    pacer.as_mut(),
                    now,
                );
                let attempts: Vec<_> = reporting
                    .attempts
                    .into_iter()
                    .map(|attempt| ReferenceReportingAttempt {
                        joint: attempt.joint,
                        address: attempt.address,
                        error: attempt
                            .error
                            .map(|error| crate::bounded_message(&error.to_string())),
                    })
                    .collect();
                let reporting_failed = attempts.iter().any(|attempt| attempt.error.is_some());
                if let Some(live) = &mut self.reference_owner.reservation {
                    live.reporting = attempts;
                }
                if reporting_failed {
                    self.fault_authority.record(
                        crate::FaultClass::Transport,
                        None,
                        None,
                        "reference reporting suspension failed",
                        crate::DeviceFaultEvidence::default(),
                    );
                    return self.fail_reference(
                        ReferenceFailureKind::Reporting,
                        "reporting Off write failed",
                    );
                }
                ReferencePhase::DrainOld
            }
            ReferencePhase::RequestIdentity | ReferencePhase::RequestReadback => {
                let watermark = self.reference_owner.physical.watermark();
                if let Some(live) = &mut self.reference_owner.reservation {
                    live.request_watermark = watermark;
                }
                let (result, next) = if reservation.phase == ReferencePhase::RequestIdentity {
                    (
                        self.bus.get_device_id_at(&reservation.address),
                        ReferencePhase::AwaitIdentity,
                    )
                } else {
                    (
                        self.bus
                            .read_parameter_at(&reservation.address, ParameterId::MechPos),
                        ReferencePhase::AwaitReadback,
                    )
                };
                if let Err(error) = result {
                    return self.fail_reference(ReferenceFailureKind::Delivery, &error.to_string());
                }
                next
            }
            ReferencePhase::ArmTarget => {
                if backend.is_physical() && self.bus.echoes_transmissions() {
                    let now = Instant::now();
                    if !self.reporting_off_settled(&reservation.address, now) {
                        return self.fail_reference(
                            ReferenceFailureKind::Backend,
                            "target reporting Off not settled on the wire before Enable",
                        );
                    }
                    if !self.set_zero_quiet_elapsed(&reservation.address, now) {
                        return self.fail_reference(
                            ReferenceFailureKind::Backend,
                            "target post-SetZero quiet not elapsed before Enable",
                        );
                    }
                }
                // Uncertain delivery may have armed the target; cleanup is
                // mandatory even when the bus returns an error.
                if let Some(live) = &mut self.reference_owner.reservation {
                    live.armed = true;
                }
                if let Err(error) = self.bus.enable_drive_at(&reservation.address) {
                    return self.fail_reference(ReferenceFailureKind::Delivery, &error.to_string());
                }
                if self.bus.echoes_transmissions() {
                    self.enable_echo_pending.insert(reservation.address.clone());
                }
                ReferencePhase::DrainPostArm
            }
            ReferencePhase::SetZero => {
                let motor = self
                    .stop_motors
                    .iter()
                    .find(|motor| MotorAddress::from(*motor) == reservation.address)
                    .cloned()
                    .ok_or_else(|| ReferenceError::InvalidRequest {
                        message: "installed target disappeared".into(),
                    })?;
                self.invalidate_reference_target_pose_for_zero_attempt(&motor);
                // Even an uncertain write may reach the wire; its echo, when
                // read, supersedes this write time.
                if self.bus.echoes_transmissions() {
                    self.note_set_zero(&reservation.address, Instant::now());
                }
                let result = self.bus.set_zero_position_at(&reservation.address);
                let epoch = match &backend {
                    ReferenceBackend::Virtual(backend) => (backend.device_epoch)(&self.bus),
                    // Any attempt, including uncertain delivery, starts a new coordinate.
                    ReferenceBackend::Physical(_) => self
                        .reference_owner
                        .physical
                        .begin_new_coordinate(&reservation.address),
                };
                if let Some(live) = &mut self.reference_owner.reservation {
                    live.device_epoch = epoch;
                }
                if let Err(error) = result {
                    return self.fail_reference(ReferenceFailureKind::Delivery, &error.to_string());
                }
                if epoch.is_none() {
                    return self.fail_reference(
                        ReferenceFailureKind::Backend,
                        "addressed SetZero did not advance a device epoch",
                    );
                }
                if backend.is_physical() {
                    ReferencePhase::AwaitAck
                } else {
                    ReferencePhase::AwaitEvidence
                }
            }
            ReferencePhase::DrainOld
            | ReferencePhase::DrainPostArm
            | ReferencePhase::AwaitEvidence
            | ReferencePhase::AwaitIdentity
            | ReferencePhase::AwaitReportingOff
            | ReferencePhase::AwaitAck
            | ReferencePhase::AwaitReadback => {
                if let ReferenceBackend::Virtual(backend) = &backend {
                    (backend.prepare_report)(&mut self.bus);
                }
                let mut allowance = AdvanceReceiveBudget::new(None);
                let report = allowance
                    .acquire(&mut self.bus, &self.motor_types)
                    .map_err(DavoutError::from)?;
                let motor = self
                    .stop_motors
                    .iter()
                    .find(|motor| MotorAddress::from(*motor) == reservation.address)
                    .cloned()
                    .ok_or_else(|| ReferenceError::InvalidRequest {
                        message: "installed target disappeared".into(),
                    })?;
                let phase = match (reservation.armed, reservation.device_epoch.is_some()) {
                    (false, _) => ReferenceReceivePhase::BeforeEnable,
                    (true, false) => ReferenceReceivePhase::EnabledBeforeZero,
                    (true, true) => ReferenceReceivePhase::Enabled,
                };
                let context = ReceiveContext::Reference(
                    ReferenceReceiveContext::from_installed_target(&motor, phase),
                );
                let consumed = self.consume_report_in_context(report, &context);
                if let Some(live) = &mut self.reference_owner.reservation {
                    live.receive = Some(ReferenceReceiveSummary {
                        completion: consumed.completion,
                        raw_frames: consumed.raw_frames,
                        read_attempts: consumed.read_attempts,
                    });
                }
                let mut proofs = match &backend {
                    ReferenceBackend::Virtual(backend) => (backend.take_proofs)(&mut self.bus),
                    ReferenceBackend::Physical(_) => Vec::new(),
                };
                if let Some(error) = consumed.first_error {
                    let kind =
                        if matches!(error, DavoutError::Bus(BusError::ReceiveIncomplete { .. })) {
                            ReferenceFailureKind::IncompleteReceive
                        } else {
                            ReferenceFailureKind::Hazard
                        };
                    return self.fail_reference(kind, &error.to_string());
                }
                if consumed.first_transition || self.has_latched_fault() {
                    return self
                        .fail_reference(ReferenceFailureKind::Hazard, "ordered feedback hazard");
                }
                if !consumed.completion.is_complete() {
                    return self.fail_reference(
                        ReferenceFailureKind::IncompleteReceive,
                        "reference drain is incomplete",
                    );
                }
                let tolerance = self.homing_config.homing.zero_verify_tolerance_rad;
                match reservation.phase {
                    ReferencePhase::AwaitEvidence => {
                        let accepted = consumed.reference_poses.into_iter().find_map(|pose| {
                            if pose.address != reservation.address
                                || !pose.state.position_rad.is_finite()
                                || f64::from(pose.state.position_rad).abs() > tolerance
                            {
                                return None;
                            }
                            let index = proofs.iter().position(|proof| {
                                Arc::ptr_eq(&proof.owner, &self.reference_owner.identity)
                                    && Arc::ptr_eq(&proof.realm, backend.realm())
                                    && proof.transaction == reservation.handle.sequence
                                    && Some(proof.device_epoch) == reservation.device_epoch
                                    && proof.address == pose.address
                                    && proof.order == pose.order
                                    && proof.can_id == pose.can_id
                                    && proof.received_at == pose.received_at
                            })?;
                            Some(AcceptedReferenceEvidence {
                                proof: proofs.swap_remove(index),
                                address: pose.address,
                                order: pose.order,
                                can_id: pose.can_id,
                                received_at: pose.received_at,
                                position_rad: pose.state.position_rad,
                            })
                        });
                        if let Some(evidence) = accepted {
                            let terminal = self.finish_reference(
                                ReferenceCause::EvidenceStaged,
                                None,
                                Some((evidence, None)),
                            )?;
                            return Ok(self.snapshot_for_terminal(terminal));
                        }
                        ReferencePhase::AwaitEvidence
                    }
                    ReferencePhase::DrainOld if backend.is_physical() => {
                        ReferencePhase::RequestIdentity
                    }
                    ReferencePhase::DrainOld => ReferencePhase::ArmTarget,
                    // On an echoing bus the target is armed for the strict Run
                    // check only once its Enable is read back from the wire; the
                    // unrenewed phase deadline fails a missing echo closed.
                    ReferencePhase::DrainPostArm
                        if self.enable_echo_pending.contains(&reservation.address) =>
                    {
                        ReferencePhase::DrainPostArm
                    }
                    ReferencePhase::DrainPostArm => ReferencePhase::SetZero,
                    ReferencePhase::AwaitIdentity => {
                        let physical = &self.reference_owner.physical;
                        match physical
                            .identity_after(&reservation.address, reservation.request_watermark)
                        {
                            None => ReferencePhase::AwaitIdentity,
                            Some(uid)
                                if physical.uid_claimed_elsewhere(&reservation.address, uid) =>
                            {
                                return self.fail_reference(
                                    ReferenceFailureKind::Identity,
                                    &format!("device identifier {uid} also answers at another installed address"),
                                );
                            }
                            Some(uid) => {
                                if let Some(live) = &mut self.reference_owner.reservation {
                                    live.uid = Some(uid);
                                }
                                if self.bus.echoes_transmissions() {
                                    if let Some(until) = self
                                        .set_zero_quiet_until(&reservation.address)
                                        .filter(|until| *until > Instant::now())
                                    {
                                        tracing::info!(
                                            interface = %reservation.address.interface,
                                            device_id = reservation.address.device_id,
                                            wait_ms = until
                                                .saturating_duration_since(Instant::now())
                                                .as_millis()
                                                as u64,
                                            "reference Enable held until the post-SetZero quiet elapses"
                                        );
                                    }
                                    ReferencePhase::AwaitReportingOff
                                } else {
                                    ReferencePhase::ArmTarget
                                }
                            }
                        }
                    }
                    // The unrenewed phase deadline fails a missing Off echo
                    // closed, as DrainPostArm does a missing Enable echo. A
                    // target zeroed less than POST_SET_ZERO_QUIET ago also waits
                    // here: its drive drops frames received in its blackout.
                    ReferencePhase::AwaitReportingOff => {
                        let now = Instant::now();
                        if self.reporting_off_settled(&reservation.address, now)
                            && self.set_zero_quiet_elapsed(&reservation.address, now)
                        {
                            ReferencePhase::ArmTarget
                        } else {
                            ReferencePhase::AwaitReportingOff
                        }
                    }
                    ReferencePhase::AwaitAck => {
                        // Only a type-2 status reply popped after the addressed
                        // SetZero can acknowledge it; type-24 reports cannot.
                        let ack = consumed.reference_poses.iter().find(|pose| {
                            pose.address == reservation.address
                                && pose.can_id >> 24
                                    == u32::from(
                                        robstride::CommunicationType::OperationStatus.as_u8(),
                                    )
                                && pose.state.position_rad.is_finite()
                                && f64::from(pose.state.position_rad).abs() <= tolerance
                        });
                        match ack {
                            Some(pose) => {
                                let ack = PhysicalAck {
                                    order: pose.order,
                                    can_id: pose.can_id,
                                    position_rad: pose.state.position_rad,
                                };
                                if let Some(live) = &mut self.reference_owner.reservation {
                                    live.ack = Some(ack);
                                }
                                ReferencePhase::RequestReadback
                            }
                            None => ReferencePhase::AwaitAck,
                        }
                    }
                    ReferencePhase::AwaitReadback => {
                        return self.accept_physical_readback(&reservation, &backend, &motor);
                    }
                    _ => {
                        return Err(ReferenceError::InvalidRequest {
                            message: "invalid receive phase".into(),
                        });
                    }
                }
            }
            ReferencePhase::Idle | ReferencePhase::Terminal => {
                return Err(ReferenceError::InvalidRequest {
                    message: "invalid reserved phase".into(),
                });
            }
        };
        // Repeated await phases do not renew their deadline.
        if next_phase != reservation.phase {
            let deadline = phase_deadline(now, reservation.overall_deadline, next_phase)?;
            if let Some(live) = &mut self.reference_owner.reservation {
                live.phase = next_phase;
                live.phase_deadline = deadline;
            }
        }
        Ok(self.reference_snapshot())
    }

    /// Physical evidence: the `mechPos` reply popped after its request, which was
    /// issued only after a type-2 ack popped after the addressed SetZero.
    fn accept_physical_readback(
        &mut self,
        reservation: &Reservation,
        backend: &ReferenceBackend<B>,
        motor: &marengo_config::MotorEntry,
    ) -> Result<ReferenceSnapshot, ReferenceError> {
        let tolerance = self.homing_config.homing.zero_verify_tolerance_rad;
        let Some(read) = self
            .reference_owner
            .physical
            .read_after(
                &reservation.address,
                ParameterId::MechPos,
                reservation.request_watermark,
            )
            .cloned()
        else {
            return Ok(self.reference_snapshot());
        };
        if !read.reply.succeeded() {
            return self.fail_reference(
                ReferenceFailureKind::Readback,
                &format!("drive refused mechPos read (status {})", read.reply.status),
            );
        }
        let scale = crate::motor_position_scale(motor)?;
        let position_rad = f64::from(read.reply.as_f32()) / scale;
        if !position_rad.is_finite() || position_rad.abs() > tolerance {
            return self.fail_reference(
                ReferenceFailureKind::Readback,
                &format!(
                    "mechPos readback {position_rad} rad is outside tolerance {tolerance} rad"
                ),
            );
        }
        let (Some(uid), Some(ack), Some(device_epoch)) =
            (reservation.uid, reservation.ack, reservation.device_epoch)
        else {
            return self.fail_reference(
                ReferenceFailureKind::Backend,
                "physical readback without identity, ack and coordinate epoch",
            );
        };
        let continuity = self.reference_owner.physical.observe_position(
            &reservation.address,
            position_rad,
            read.received_at,
            tolerance,
        );
        if continuity.is_some()
            || self.reference_owner.physical.uid(&reservation.address) != Some(uid)
            || self.reference_owner.physical.epoch(&reservation.address) != Some(device_epoch)
        {
            return self.fail_reference(
                ReferenceFailureKind::Identity,
                "device identity or coordinate continuity changed during acquisition",
            );
        }
        let evidence = AcceptedReferenceEvidence {
            proof: ReferenceCorrelation {
                owner: Arc::clone(&self.reference_owner.identity),
                realm: Arc::clone(backend.realm()),
                transaction: reservation.handle.sequence,
                device_epoch,
                address: reservation.address.clone(),
                order: read.order,
                can_id: read.can_id,
                received_at: read.received_at,
            },
            address: read.address.clone(),
            order: read.order,
            can_id: read.can_id,
            received_at: read.received_at,
            position_rad: position_rad as f32,
        };
        let physical = PhysicalEvidence {
            uid,
            ack_order: ack.order,
            ack_can_id: ack.can_id,
            ack_position_rad: ack.position_rad,
        };
        let terminal = self.finish_reference(
            ReferenceCause::EvidenceStaged,
            None,
            Some((evidence, Some(physical))),
        )?;
        Ok(self.snapshot_for_terminal(terminal))
    }

    pub fn cancel_reference(
        &mut self,
        handle: &ReferenceHandle,
        reason: ReferenceCancelReason,
    ) -> Result<ReferenceTerminal, ReferenceError> {
        if let Some(terminal) = self.retained_reference(handle)? {
            return Ok(terminal);
        }
        self.live_reference(handle)?;
        self.finish_reference(ReferenceCause::Cancelled(reason), None, None)
    }

    /// Mandatory cleanup is separate from optional ordinary exit Disable.
    pub fn cancel_reference_for_shutdown(&mut self) -> Option<ReferenceTerminal> {
        if !self.reference_authority.consumed_bindings().is_empty() {
            self.reference_authority.revoke();
        }
        self.reference_owner.pending_commit = None;
        self.invalidate_retained_stages(ReferenceStageInvalidation::Shutdown);
        self.cancel_reference_commits(ReferenceCancelReason::Shutdown);
        if !self.acquisition_busy() {
            return None;
        }
        self.finish_reference(
            ReferenceCause::Cancelled(ReferenceCancelReason::Shutdown),
            None,
            None,
        )
        .ok()
    }

    pub(super) fn cancel_live_reference_for_disable(
        &mut self,
    ) -> Result<ReferenceTerminal, ReferenceError> {
        self.finish_reference(
            ReferenceCause::Cancelled(ReferenceCancelReason::Disable),
            None,
            None,
        )
    }

    pub(super) fn abort_reference_for_hazard(&mut self, message: &str) {
        self.invalidate_reference_commits(ReferenceStageInvalidation::SafetyHazard);
        if self.acquisition_busy() {
            let _ =
                self.finish_reference(failure(ReferenceFailureKind::Hazard, message), None, None);
        } else {
            let _ = self.perform_stop(false);
        }
    }

    fn reference_policy(&self) -> Result<serde_json::Value, ReferenceError> {
        marengo_config::validate_safety_config(
            &self.robot,
            &self.motors,
            &self.control,
            &self.homing_config,
        )
        .map_err(DavoutError::from)?;
        // Compatibility fields may be edited before a stamp is requested. A
        // fresh stamp cannot reinstall a different drive/frame into this owner.
        let installed = self
            .stop_motors
            .iter()
            .zip(&self.motors.motors)
            .all(|(old, current)| {
                old.joint == current.joint
                    && old.driver == current.driver
                    && old.motor_type == current.motor_type
                    && old.can_interface == current.can_interface
                    && old.device_id == current.device_id
                    && old.direction == current.direction
                    && old.gear_ratio == current.gear_ratio
                    && old.recv_can_id == current.recv_can_id
                    && old.firmware_version == current.firmware_version
            });
        if self.stop_motors.len() != self.motors.motors.len() || !installed {
            return Err(ReferenceError::InvalidRequest {
                message: "public mapping differs from the installed drive/frame policy".into(),
            });
        }
        let value = serde_json::to_value((&self.motors, &self.control, &self.homing_config))
            .map_err(|error| ReferenceError::InvalidRequest {
                message: crate::bounded_message(&error.to_string()),
            })?;
        let bytes = serde_json::to_vec(&value).map_err(|error| ReferenceError::InvalidRequest {
            message: crate::bounded_message(&error.to_string()),
        })?;
        if bytes.len() > POLICY_CAPACITY {
            return Err(ReferenceError::InvalidRequest {
                message: "installed reference policy exceeds the bounded transaction capacity"
                    .into(),
            });
        }
        Ok(value)
    }

    fn retained_reference(
        &self,
        handle: &ReferenceHandle,
    ) -> Result<Option<ReferenceTerminal>, ReferenceError> {
        if !Arc::ptr_eq(&handle.owner, &self.reference_owner.identity) {
            return Err(ReferenceError::ForeignIdentity);
        }
        Ok(self
            .reference_owner
            .outcomes
            .iter()
            .find(|outcome| outcome.terminal.handle == *handle)
            .map(|outcome| outcome.terminal.clone()))
    }

    fn live_reference(&self, handle: &ReferenceHandle) -> Result<&Reservation, ReferenceError> {
        self.reference_owner
            .reservation
            .as_ref()
            .filter(|reservation| reservation.handle == *handle)
            .ok_or(ReferenceError::OutcomeExpired)
    }

    fn finish_reference(
        &mut self,
        cause: ReferenceCause,
        existing_stop: Option<StopReport>,
        accepted: Option<(AcceptedReferenceEvidence, Option<PhysicalEvidence>)>,
    ) -> Result<ReferenceTerminal, ReferenceError> {
        let reservation = self
            .reference_owner
            .reservation
            .take()
            .ok_or(ReferenceError::OutcomeExpired)?;
        // A completed physical reference stops the drives group by group; any
        // failure or cancellation stops them at once.
        let stop = existing_stop.unwrap_or_else(|| {
            let mut pacer = (accepted.is_some()
                && self.reference_owner.is_physical()
                && self.bus.echoes_transmissions())
            .then(BurstPacer::default);
            self.perform_stop_paced(false, pacer.as_mut())
        });
        if let Some(ReferenceBackend::Virtual(backend)) = &self.reference_owner.backend {
            (backend.end)(&mut self.bus);
        }
        self.reference_owner.mark_owner_work();
        let stage = accepted.map(|(evidence, physical)| RetainedStage {
            evidence,
            physical,
            installed_model: reservation.installed_model,
            overall_deadline: reservation.overall_deadline,
            reference_generation: self.reference_authority.generation(),
            invalidated: Cell::new(None),
            typed_policy: reservation.typed_policy,
        });
        let terminal = ReferenceTerminal {
            handle: reservation.handle,
            joint: reservation.request.joint.clone(),
            cause,
            stop,
            reporting: reservation.reporting,
            receive: reservation.receive,
            commit: ReferenceCommit::Unavailable,
            usable_reference: false,
        };
        if self.reference_owner.outcomes.len() == TERMINAL_CAPACITY {
            self.reference_owner.outcomes.pop_front();
        }
        self.reference_owner
            .outcomes
            .push_back(Rc::new(RetainedOutcome {
                request: reservation.request,
                terminal: terminal.clone(),
                stage,
            }));
        // The real cleanup and backend end precede the first stage observation.
        // Failure here changes only its inspection status, never the terminal.
        let _ = self.observe_staged_reference();
        Ok(terminal)
    }

    fn fail_reference(
        &mut self,
        kind: ReferenceFailureKind,
        message: &str,
    ) -> Result<ReferenceSnapshot, ReferenceError> {
        if matches!(
            kind,
            ReferenceFailureKind::Delivery | ReferenceFailureKind::Backend
        ) {
            self.fault_authority.record(
                crate::FaultClass::Transport,
                None,
                None,
                message,
                crate::DeviceFaultEvidence::default(),
            );
        }
        let terminal = self.finish_reference(failure(kind, message), None, None)?;
        Ok(self.snapshot_for_terminal(terminal))
    }

    fn snapshot_for_terminal(&self, terminal: ReferenceTerminal) -> ReferenceSnapshot {
        let mut snapshot = self.reference_snapshot();
        snapshot.phase = ReferencePhase::Terminal;
        snapshot.joint = Some(terminal.joint.clone());
        snapshot.handle = Some(terminal.handle.clone());
        snapshot.remaining = None;
        snapshot.reference_armed = false;
        snapshot.receive = terminal.receive;
        snapshot.staged_evidence = self.reference_stage_status_for(&terminal.handle);
        snapshot.terminal = Some(terminal);
        snapshot
    }

    pub(super) fn stage_for_commit(
        &self,
        handle: &ReferenceHandle,
    ) -> Result<Rc<RetainedOutcome>, ReferenceError> {
        if !Arc::ptr_eq(&handle.owner, &self.reference_owner.identity) {
            return Err(ReferenceError::ForeignIdentity);
        }
        self.reference_owner
            .outcomes
            .iter()
            .find(|outcome| outcome.terminal.handle == *handle)
            .cloned()
            .ok_or(ReferenceError::OutcomeExpired)
    }
}

impl RetainedOutcome {
    pub(super) fn handle(&self) -> &ReferenceHandle {
        &self.terminal.handle
    }
    pub(super) fn terminal_ref(&self) -> &ReferenceTerminal {
        &self.terminal
    }
    pub(super) fn joint(&self) -> &str {
        &self.terminal.joint
    }
    pub(super) fn retire(&self, reason: ReferenceStageInvalidation) {
        if let Some(stage) = &self.stage {
            if stage.invalidated.get().is_none() {
                stage.invalidated.set(Some(reason));
            }
        }
    }
    /// Evidence position (joint space) retained for operator receipts.
    pub(super) fn evidence_position_rad(&self) -> Option<f32> {
        self.stage.as_ref().map(|stage| stage.evidence.position_rad)
    }
    pub(super) fn is_physical(&self) -> bool {
        self.stage
            .as_ref()
            .is_some_and(|stage| stage.physical.is_some())
    }
    pub(super) fn select_current(
        &self,
        authority: &mut crate::reference::ReferenceAuthority,
        job: Arc<()>,
    ) -> Option<()> {
        let stage = self.stage.as_ref()?;
        let proof = &stage.evidence.proof;
        let scope = if stage.physical.is_some() {
            crate::reference::SelectionScope::Accumulate
        } else {
            crate::reference::SelectionScope::Replace
        };
        authority.select_consumed(
            Arc::clone(&proof.realm),
            crate::reference::ConsumedReferenceBinding::new(
                job,
                stage.installed_model.clone(),
                self.joint().to_owned(),
                stage.evidence.address.clone(),
                proof.device_epoch,
                stage.physical.map(|physical| physical.uid),
            ),
            &stage.typed_policy,
            scope,
        )
    }
    pub(super) fn journal_input(
        &self,
        audit: crate::ReferenceAudit,
    ) -> Result<crate::reference_journal_event::Input, ReferenceError> {
        use crate::reference_journal_event::{Capture, Input};
        let stage = self.stage.as_ref().ok_or(ReferenceError::OutcomeExpired)?;
        let evidence = &stage.evidence;
        let receive = self
            .terminal
            .receive
            .ok_or(ReferenceError::OutcomeExpired)?;
        Ok(Input {
            model: stage.installed_model.clone(),
            policy: Arc::clone(&stage.typed_policy),
            capture: Capture {
                acquisition_sequence: self.terminal.handle.sequence,
                joint: self.request.joint.clone(),
                address: (&evidence.address).into(),
                confirmed: self.request.confirmed,
                sign_verified: self.request.sign_verified,
                position_rad: evidence.position_rad,
                raw_pop_order: u32::try_from(evidence.order)
                    .map_err(|_| ReferenceError::CounterExhausted)?,
                can_id: evidence.can_id,
                device_epoch: evidence.proof.device_epoch,
                original_deadline_secs: stage.overall_deadline.as_secs(),
                original_deadline_nanos: stage.overall_deadline.subsec_nanos(),
                stamp_reference_generation: self.request.stamp.reference_generation,
                stamp_stop_generation: self.request.stamp.stop_generation,
                accepted_reference_generation: stage.reference_generation,
                cleanup: (&self.terminal.stop).into(),
                reporting: self.terminal.reporting.iter().map(Into::into).collect(),
                receive: crate::reference_journal_event::Receive::try_from(receive).map_err(
                    |message| ReferenceError::InvalidRequest {
                        message: message.into(),
                    },
                )?,
                audit,
                physical: stage.physical.map(|physical| {
                    crate::reference_journal_event::PhysicalCapture {
                        device_uid: physical.uid.as_u64(),
                        ack_pop_order: u32::try_from(physical.ack_order).unwrap_or(u32::MAX),
                        ack_can_id: physical.ack_can_id,
                        ack_position_rad: physical.ack_position_rad,
                    }
                }),
            },
        })
    }
}

fn same_request(old: &ReferenceRequest, new: &ReferenceRequest) -> bool {
    old.joint == new.joint
        && old.confirmed == new.confirmed
        && old.sign_verified == new.sign_verified
        && Arc::ptr_eq(&old.stamp.owner, &new.stamp.owner)
        && old.stamp.sequence == new.stamp.sequence
        && old.stamp.stop_generation == new.stamp.stop_generation
        && old.stamp.reference_generation == new.stamp.reference_generation
        && old.stamp.installed_model == new.stamp.installed_model
        && old.stamp.policy == new.stamp.policy
}

fn failure(kind: ReferenceFailureKind, message: &str) -> ReferenceCause {
    ReferenceCause::Failed {
        kind,
        message: crate::bounded_message(message),
    }
}

fn phase_deadline(
    now: Duration,
    overall: Duration,
    phase: ReferencePhase,
) -> Result<Duration, ReferenceError> {
    if phase == ReferencePhase::AwaitEvidence {
        return Ok(overall);
    }
    now.checked_add(PHASE_TIMEOUT.min(overall.saturating_sub(now)))
        .ok_or(ReferenceError::CounterExhausted)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::simulation::{InitialVirtualReference, SimulationBus};

    #[test]
    fn exhausted_identity_and_virtual_time_refuse_before_revocation_or_tx() {
        for exhaust_identity in [true, false] {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let mut owner = Supervisor::from_simulation(
                root,
                SimulationBus::default(),
                InitialVirtualReference::Unreferenced,
            )
            .expect("actual closed owner");
            let mut request = ReferenceRequest {
                stamp: owner
                    .reference_snapshot()
                    .next_stamp
                    .expect("issued before exhaustion"),
                joint: "right_elbow_pitch".into(),
                confirmed: true,
                sign_verified: true,
            };
            let before = owner.bus().frames().to_vec();
            let generation = owner.reference_generation();
            if exhaust_identity {
                // Counter injection exercises an otherwise unreachable lifetime
                // boundary; it creates no reference grant or device evidence.
                owner.reference_owner.next_sequence = u64::MAX;
                request.stamp.sequence = u64::MAX;
            } else {
                owner
                    .bus_mut()
                    .elapse_reference_clock(Duration::MAX)
                    .expect("finite maximum owner clock");
                assert!(owner
                    .bus_mut()
                    .elapse_reference_clock(Duration::from_nanos(1))
                    .is_err());
            }
            assert!(matches!(
                owner.begin_reference(request),
                Err(ReferenceError::CounterExhausted)
            ));
            assert_eq!(owner.bus().frames(), before);
            assert_eq!(owner.reference_generation(), generation);
            assert!(!owner.reference_busy());
        }
    }

    /// ADR 0026:62-66: every exit of an armed reference ends with the
    /// all-address stop. The `?` exits of `advance_reference` are unreachable
    /// through the public API today (the receive budget is fresh on every call
    /// and the target is resolved from the immutable installed set), so this
    /// forces one white-box: the reservation's target is not an installed drive after arming.
    #[test]
    fn error_exit_after_arming_runs_the_all_address_stop() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut owner = Supervisor::from_simulation(
            root,
            SimulationBus::default(),
            InitialVirtualReference::Unreferenced,
        )
        .expect("actual closed owner");
        let request = ReferenceRequest {
            stamp: owner
                .reference_snapshot()
                .next_stamp
                .expect("stamp before reservation"),
            joint: "right_elbow_pitch".into(),
            confirmed: true,
            sign_verified: true,
        };
        let handle = owner.begin_reference(request).expect("reservation");
        let mut advances = 0;
        while !owner
            .reference_owner
            .reservation
            .as_ref()
            .is_some_and(|reservation| reservation.armed)
        {
            owner.advance_reference(&handle).expect("phase advance");
            advances += 1;
            assert!(advances < 64, "target never armed");
        }
        let installed = owner.stop_motors.len();
        // The binding checks never look at the reservation's own address, so
        // pointing it at a drive that is not installed reaches the lookup.
        if let Some(reservation) = &mut owner.reference_owner.reservation {
            reservation.address = MotorAddress::new("vanished", 99);
        }
        let frames_before = owner.bus().frames().len();

        let error = owner
            .advance_reference(&handle)
            .expect_err("an uninstalled reservation target is an error exit");
        assert!(
            matches!(error, ReferenceError::InvalidRequest { .. }),
            "unexpected {error}"
        );
        assert!(
            !owner.reference_busy(),
            "the armed reservation must not outlive the error exit"
        );
        let disables = owner.bus().frames()[frames_before..]
            .iter()
            .filter(|frame| {
                (frame.id >> 24) & 0x1f == u32::from(robstride::CommunicationType::Disable.as_u8())
            })
            .count();
        assert!(
            disables >= installed,
            "the cleanup stop must disable every installed drive ({disables} of {installed})"
        );
    }
}
