//! # Davout — safety gateway (sole motor command path)
//!
//! Davout is the **only** crate that may send motion to [`robstride`]. Every MIT or legacy
//! command is filtered here before CAN encode. Berthier and bins must not call `robstride`
//! directly.
//!
//! ## Responsibilities
//!
//! - [`Supervisor`]: operational state machine (Disabled → Ready → Active).
//! - Filter [`MitJointCommand`] / [`JointCommand`]: URDF ∩ bench limits, per-`motor_type`
//!   `kp`/`kd`/`tau_ff` caps and [`tau_ff` rate limiting](Supervisor::filter_mit_core).
//! - Own joint↔motor coordinate conversion from `config/motors.yaml` (`direction`, `gear_ratio`):
//!   Berthier and dynamics stay in joint space; robstride stays in raw motor/CAN space.
//! - [`danger_zones`](marengo_config::DangerZoneRule) from `config/control.yaml` (clamp or fault on rule hit).
//! - Comm watchdog: stale feedback → [`DavoutError::CommWatchdog`].
//! - Persistent [`SafetySnapshot`]: device/runtime faults survive pose replacement, disable and replay.
//! - Private current-reference authority: retired history, cached pose and the
//!   removed scalar verifier cannot authorize Ready, scoped Enable or motion.
//! - Closed [`simulation::SimulationBus`] INITIAL virtual fixtures share admission/output logic;
//!   they do not qualify reference acquisition, SetZero correlation or persistence ordering.
//! - [`Supervisor::begin_reference`] / [`Supervisor::advance_reference`]: bounded owner
//!   acquisition with mandatory cleanup. The sealed virtual backend stages correlated
//!   virtual evidence; the explicit physical owner
//!   ([`Supervisor::from_repo_with_physical_reference`], ADR 0036) stages Robstride
//!   evidence: type-0 MCU identity, a type-2 ack popped after the addressed SetZero and a
//!   requested type-17 `mechPos` readback within tolerance. Neither grants motion.
//! - Retained matched evidence and [`ReferenceStageStatus`]: live inspection of
//!   owner/device/model/policy/stop continuity after cleanup, distinct from an
//!   immutable unusable acquisition terminal. No stage diagnostic grants output.
//! - Explicit unreferenced virtual journal owners commit immutable typed history on a
//!   dedicated bounded SQLite worker. Owner-consumed completion remains distinct from
//!   eligibility, cancellation and output permission; recovery is inspection only.
//! - Explicit current-consuming virtual owners may select the acquired joint after real
//!   durable completion and fresh continuity checks. Its private lifetime binds actual
//!   model/device continuity independently of transaction deadlines and diagnostic caches.
//! - Physical owners accumulate per-joint grants. Each binds the acquisition's MCU
//!   identifier and coordinate epoch; identity change or absence at Enable, coordinate
//!   discontinuity, Calibration mode, or silence beyond `comm_watchdog_ms` outside owner
//!   reference work revokes that joint. Fault/E-stop/uncertain stop revokes all.
//! - [`Supervisor::disable_all`]: all-address best-effort stop with honest delivery evidence.
//! - [`Supervisor::inspect_drive_protocol`] (ADR 0037): standalone, blocking read of
//!   firmware version, MCU identity and registers (including 0x7028 CAN timeout)
//!   from stopped drives through [`Supervisor::from_repo_for_protocol_inspection`].
//!   It writes only Disable, reporting Off and read queries, and never grants.
//! - [`drain_feedback`](Supervisor::drain_feedback): non-blocking RX queue drain
//!   (Berthier control loop; per-tick frame accounting via `begin_tick_feedback`).
//!
//! ## Does not
//!
//! - Compute gravity, impedance targets, or trajectories (Berthier / Talleyrand).
//! - Encode MIT CAN bytes (robstride).
//! - Plan paths or run vision (Talleyrand / Fouché).
//!
//! ## Data flow
//!
//! ```text
//! Berthier MitJointCommand batch
//!        │
//!        ▼
//!   Supervisor::send_mit_batch
//!        │
//!        ├─ filter in joint space
//!        ├─ apply direction/gear_ratio to motor space
//!        ▼
//!   mit_control_all_at (addressed Robstride batch)
//!        ▲
//!   motor_states (joint space) ◄── direction/gear_ratio ◄── lossless feedback report ◄── CAN
//!        │
//!        ▼
//!   Supervisor::joint_feedback (JointFeedback) — control / publish seam
//! ```
//!
//! Limits are built from [`armee_kinematics`] + [`marengo_config`] at startup.
//! See [safety.md](../../docs/safety.md), [ADR 0004](../../docs/decisions/0004-control-modes-and-mit.md),
//! and [ADR 0009](../../docs/decisions/0009-dynamic-position-limit-envelope.md).
//! Fault evidence/authority and unqualified recovery are defined by ADR 0020.
//! Reference admission and the isolated virtual realm are defined by ADR 0023.

pub use armee_kinematics::JointLimitPolicy;
#[cfg(test)]
extern crate self as davout;

mod active_reporting;
#[cfg(test)]
mod active_reporting_pacing_tests;
mod burst;
mod drive_loss;
mod faults;
mod feedback_consumer;
pub(crate) mod homing_facets;
#[cfg(test)]
mod limit_build_tests;
mod limit_envelope;
mod protocol_inspection;
mod reference;
mod reference_codec;
mod reference_commit;
mod reference_journal;
mod reference_journal_event;
#[cfg(test)]
mod reference_journal_tests;
mod reference_model;
mod reference_physical;
mod reference_transaction;
mod reference_urdf_codec;
pub mod simulation;

pub use drive_loss::{
    DegradedEnd, DegradedEpisode, DegradedOutcome, DriveLossPlan, SHED_REPLY_GRACE,
};
pub use protocol_inspection::{
    DriveProtocolInspection, INSPECTED_PARAMETERS, INSPECTION_QUERY_TIMEOUT, INSPECTION_SETTLE,
};
pub use reference_commit::{
    ReferenceAudit, ReferenceCommitError, ReferenceCommitHandle, ReferenceCommitPhase,
    ReferenceCommitSnapshot, ReferenceOutcome,
};
pub use reference_journal::{ReferenceJournalDrain, ReferenceJournalError, ReferenceJournalResult};
pub use reference_journal_event::ReferenceHistoryRecord;
/// Host timing bound of the liveness rule (ADR 0036, *Owed On*).
pub use reference_physical::OWED_ON_WRITE_BOUND;
use reference_physical::{Lapse, OwedOn, SilenceFrom};
/// Firmware timing bounds, exported for the measured-profile conformance test.
pub use reference_physical::{
    IDENTITY_ADMISSION_RETRY, IDENTITY_ADMISSION_SPACING, IDENTITY_ADMISSION_TIMEOUT,
};

pub use reference_transaction::{
    ReferenceCancelReason, ReferenceCause, ReferenceCommit, ReferenceError, ReferenceFailureKind,
    ReferenceHandle, ReferencePhase, ReferenceReceiveSummary, ReferenceReportingAttempt,
    ReferenceRequest, ReferenceSnapshot, ReferenceStageInvalidation, ReferenceStageStatus,
    ReferenceStamp, ReferenceTerminal,
};

pub use active_reporting::{ActiveReportingLeaseError, ActiveReportingState, DEFAULT_LEASE_TTL};
use burst::BurstPacer;
pub use burst::BURST_GROUP_SPACING;
use faults::{bounded_message, FaultAuthority};
pub use faults::{
    DeviceFaultEvidence, DeviceWarning, FaultClass, FaultRecord, ReceiveDrainEvidence,
    ReceiveFaultEvidence, ReceiveFrameEvidence, SafetySnapshot, StopAction, StopAttempt,
    StopReport,
};
use homing_facets::select_enable_targets;
pub use homing_facets::{JointFacetInput, JointHomingState};

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustc_hash::{FxHashMap, FxHashSet};

/// Free-drive / Hardware-page sensing TTL for `RobotState` presence.
///
/// While not [`OperationalMode::Active`], `joint_feedback` omits samples older than this so
/// Consul Online/Offline tracks recent RX (status poll or type-24), not sticky cache membership.
/// Sized at ~2× Consul `MOTOR_STATUS_POLL_MS` (2.5 s) so one missed poll does not flap Offline.
pub const FREE_DRIVE_FEEDBACK_TTL: Duration = Duration::from_secs(5);

/// Minimum time from a SetZero on the wire to the next Enable (or the type-24
/// Off that precedes it) written to the same address on an echoing bus.
///
/// Bench candumps of the right-arm drives (eight captures 2026-10-03, 128
/// SetZeros on all five; drives 1-2 run firmware 0.3.1.42, 3-4 0.2.3.34 and
/// 5 0.0.3.32; profile
/// `docs/commissioning/firmware/robstride-timing-profile.json`): after
/// receiving a SetZero (type 6) every Robstride drive transmits nothing for
/// 45-61 ms, starting 511-543 ms later in 127 cases and 614 ms once
/// (right_elbow_pitch, 15:34:08), and a frame it receives in that window is
/// never acted on. An Enable written there leaves the drive in Reset, which
/// latches a persistent `DriveState` fault after the Enable's echo. 800 ms is
/// the latest observed blackout end (667 ms) plus 100 ms, rounded up to 50 ms;
/// `tests/firmware_profile.rs` keeps that margin over the committed profile.
/// The SetZero time is its host echo's read time (its write time until the
/// echo is read), which bounds the wire time from above.
pub const POST_SET_ZERO_QUIET: Duration = Duration::from_millis(800);

/// Earliest a drive's post-SetZero blackout can begin, counted from the
/// SetZero's host echo: the measured minimum is 511 ms
/// (`docs/commissioning/firmware/robstride-firmware-behavior.md`), less 61 ms
/// for tick and pacing jitter. From here to [`POST_SET_ZERO_QUIET`] no type-24
/// On or Off is written to the drive: it drops every frame it receives in the
/// blackout, so an On written there leaves the stream Off with the host
/// believing otherwise. Before this point the drive acts on type-24 writes.
/// `tests/firmware_profile.rs` keeps the margin over the committed profile.
pub const POST_SET_ZERO_BLACKOUT_FROM: Duration = Duration::from_millis(450);

use armee_kinematics::{
    clamp_position_in_envelope, joint_limit_bounds, joint_limits, load_urdf, LimitMarginConfig,
};
use marengo_config::{
    default_commissioning_scope_path, effective_commissioning_scope, joint_subset_from_env,
    load_commissioning_scope, load_control_config_from, load_homing_config_from,
    load_motors_config_from, load_robot_config, load_robot_config_from, motor_for_joint,
    motor_type_key, resolve_config_dir, resolve_joint_velocity_cap, resolve_urdf_path,
    validate_control_against_limits, validate_safety_config, ControlConfigFile, HomingConfigFile,
    MotorEntry, MotorType, MotorsConfigFile, RobotConfigFile,
};
use reference::ReferenceAuthority;
use robstride::AddressedMitCommand;
use robstride::{DriveMode, MitCommand, MotorState, RunMode};
use thiserror::Error;
use tracing::{debug, info, trace, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalMode {
    Disabled,
    Ready,
    Active,
}

/// Berthier control mode (maps to proto `ControlMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    Disabled,
    GravityComp,
    Impedance,
    Position,
    TorqueOnly,
}

/// Joint-space feedback sample for one actuated joint.
///
/// Cache values are already joint-space after the installed-policy [`Supervisor`] poll.
/// Callers read this facade; they do not address the bus or re-apply direction/gear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointFeedback {
    pub position_rad: f64,
    pub velocity_rad_s: f64,
    pub torque_nm: f64,
    pub temperature_c: f32,
    pub fault: u16,
    /// Age of the underlying RX sample at read time. Presence in
    /// `RobotState` means "heard within the free-drive TTL", not "grant
    /// live": consumers must use this age, not mere presence, for liveness.
    pub sample_age: Duration,
}

/// MIT command for one joint (after filtering).
#[derive(Debug, Clone, PartialEq)]
pub struct MitJointCommand {
    pub joint: String,
    pub kp: f64,
    pub kd: f64,
    pub position_rad: f64,
    pub velocity_rad_s: f64,
    pub torque_ff_nm: f64,
}

#[derive(Debug, Error)]
pub enum DavoutError {
    #[error("config: {0}")]
    Config(#[from] marengo_config::ConfigError),
    #[error("urdf: {0}")]
    Urdf(#[from] armee_kinematics::UrdfError),
    #[error("bus: {0}")]
    Bus(#[from] BusError),
    #[error("joint {joint}: {message}")]
    Limit { joint: String, message: String },
    #[error("supervisor in mode {mode:?}, motion not allowed")]
    NotActive { mode: OperationalMode },
    #[error("hardware E-stop asserted")]
    Estop,
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("joint {joint} is not in the active enable set")]
    InactiveJoint { joint: String },
    #[error("comm watchdog on {joint} ({interface}:{device_id}): no current-enable pose within {ms} ms (sample age {age_ms:?} ms)")]
    CommWatchdog {
        ms: u64,
        joint: String,
        interface: String,
        device_id: u8,
        age_ms: Option<u64>,
    },
    #[error("invalid command for {joint}: {message}")]
    InvalidCommand { joint: String, message: String },
    #[error("invalid feedback for {joint}: {message}")]
    InvalidFeedback { joint: String, message: String },
    #[error("danger zone {name} triggered on {joint}")]
    DangerZone { name: String, joint: String },
    #[error("invalid motor config for {joint}: {message}")]
    InvalidMotorConfig { joint: String, message: String },
    #[error("motor fault on {joint}: 0x{fault:04x}")]
    MotorFault { joint: String, fault: u16 },
    #[error("persistent safety fault {id} ({class:?}) on {joint:?}: {message}; qualified recovery unavailable")]
    FaultLatched {
        id: u64,
        class: FaultClass,
        joint: Option<String>,
        message: String,
    },
    #[error("stop delivery failed for {failed_writes} writes; physical stop unconfirmed")]
    StopDelivery { failed_writes: usize },
    #[error("homing: {message}")]
    Homing { message: String },
    #[error("homing verify on {joint}: {message}")]
    HomingVerify { joint: String, message: String },
    #[error("{operation}: qualified current-reference capability is unavailable")]
    ReferenceUnsupported { operation: &'static str },
    #[error("reference transaction is busy; {operation} is refused")]
    ReferenceBusy { operation: &'static str },
    #[error("installed model/policy generation is exhausted")]
    InstalledGenerationExhausted,
    #[error("installed reference model: {message}")]
    ReferenceModel { message: String },
    #[error("wrong-sign watchdog: gravity-comp torque opposes motion for {ticks} ticks on joint {joint}")]
    WrongSignWatchdog { joint: String, ticks: u32 },
    #[error("runtime limit changes are refused while supervisor is ACTIVE")]
    LimitPatchActive,
    #[error("active enable set cannot change while ACTIVE; disable first")]
    ActiveSetChangeRefused,
    #[error("drive protocol inspection: {message}")]
    ProtocolInspection { message: String },
    #[error("degraded hold after losing {joint}: {message}; only disable or lower is accepted")]
    DegradedEpisode {
        joint: String,
        message: &'static str,
    },
    #[error("drive-loss plan for {joint}: {message}")]
    DriveLossPlan { joint: String, message: String },
}

pub use robstride::bus::{BusError, MemoryBus, MotorAddress, MotorBus};

fn map_lease_error(err: ActiveReportingLeaseError) -> DavoutError {
    match err {
        ActiveReportingLeaseError::UnknownJoint { joint } => DavoutError::UnknownJoint { joint },
        ActiveReportingLeaseError::InvalidLeaseId
        | ActiveReportingLeaseError::InvalidClientId
        | ActiveReportingLeaseError::InvalidTtl
        | ActiveReportingLeaseError::TooManyLeases { .. }
        | ActiveReportingLeaseError::MissingLease { .. } => DavoutError::Homing {
            message: format!("active reporting lease: {err:?}"),
        },
    }
}

/// Synchronous calibration waits beyond the acquisition deadline only for the
/// durable commit and its fresh consumption.
const CALIBRATION_COMMIT_GRACE: Duration = Duration::from_secs(10);
/// Synchronous calibration yields between bounded owner advances.
const CALIBRATION_POLL: Duration = Duration::from_millis(2);

fn reference_error(joint: &str, error: ReferenceError) -> DavoutError {
    match error {
        ReferenceError::Admission(error) => error,
        ReferenceError::Unsupported => DavoutError::ReferenceUnsupported {
            operation: "reference calibration",
        },
        other => DavoutError::HomingVerify {
            joint: joint.to_owned(),
            message: other.to_string(),
        },
    }
}

const FEEDBACK_VELOCITY_LIMIT_TRIPS: u8 = 3;
/// Measured (position-derived) speed must exceed `limit + margin` before tripping.
/// Absorbs encoder quantization and planner cruise at the nominal cap without disabling.
const FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S: f64 = 0.50;

#[derive(Debug, Clone, Copy, Default)]
struct WrongSignState {
    opposition_ticks: u32,
    ticks_since_enable: u32,
}

#[derive(Debug, Clone, Copy)]
struct FeedbackSample {
    position_rad: f64,
    received_at: Instant,
}

/// Whether a reference-validity check judges physical grant liveness now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Liveness {
    Judge,
    /// Left to the next judged check: the receive queue has not been read
    /// yet. An address whose Enable echo is pending is still judged: its
    /// traffic is not pose, so reading cannot renew it, and a staggered
    /// Enable may be written to it before the read.
    Defer,
}

/// Limiter/watchdog advance for one admitted command, committed only with its batch.
#[derive(Debug, Clone, Copy)]
struct StagedMitState {
    /// Index of the command's configured motor in `motors.motors`.
    motor: usize,
    tau_ff_nm: f64,
    wrong_sign: Option<WrongSignState>,
}

/// Reused MIT admission buffers; contents never outlive one batch.
#[derive(Debug, Default)]
struct MitBatchScratch {
    /// Configured motor index per command, in command order.
    configured: Vec<usize>,
    staged: Vec<StagedMitState>,
    wires: Vec<AddressedMitCommand>,
}

/// Latest observed drive frame per address (a status or version reply's mode
/// plus its host receive time). Tracking only: never permission, liveness,
/// or fault evidence.
#[derive(Debug, Clone, Copy)]
struct DriveFrameTrack {
    mode: DriveMode,
    at: Instant,
}

/// Active session per-target solicit state: the write instant of the earliest
/// host frame the drive answers that no admitted pose has followed, plus how
/// many such writes went unanswered since the last admitted pose (ADR 0036,
/// *Solicited silence while Active*).
#[derive(Debug, Clone, Copy, Default)]
struct SolicitedState {
    earliest: Option<Instant>,
    unanswered_writes: u64,
}

impl SolicitedState {
    /// Another host frame the drive answers went out at `written_at` with no
    /// admitted pose since: keep the earliest instant and count the write.
    fn note_asked(&mut self, written_at: Instant) {
        self.earliest.get_or_insert(written_at);
        self.unanswered_writes = self.unanswered_writes.saturating_add(1);
    }
}

/// A likely drive power-interruption reboot observed on the wire: a drive the
/// host expected to run reported Reset after running, or reappeared in Reset
/// after longer than the comm watchdog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveRebootObservation {
    pub joint: String,
    pub address: MotorAddress,
    pub previous_mode: DriveMode,
    pub new_mode: DriveMode,
    /// Host time between the drive's previous frame and this one.
    pub silence: Duration,
}

/// Safety supervisor — the only gateway to the motor bus.
pub struct Supervisor<B: MotorBus> {
    mode: OperationalMode,
    control_mode: ControlMode,
    hardware_estop: bool,
    fault_authority: FaultAuthority,
    limits: FxHashMap<String, JointLimitPolicy>,
    robot: RobotConfigFile,
    urdf_robot: urdf_rs::Robot,
    installed_model: reference_model::InstalledReferenceModel,
    pub motors: MotorsConfigFile,
    pub control: ControlConfigFile,
    pub homing_config: HomingConfigFile,
    out_of_limits: homing_facets::OutOfLimitsFlags,
    reference_authority: ReferenceAuthority,
    reference_owner: reference_transaction::ReferenceOwner<B>,
    reference_commits: reference_commit::CommitOwner,
    /// Installed routes remain stop targets even if a caller corrupts public policy.
    stop_motors: Arc<[MotorEntry]>,
    /// Routing keys for `motors.motors`; each use revalidates against the public entry.
    motor_addresses: Vec<MotorAddress>,
    mit_scratch: MitBatchScratch,
    feedback_scratch: feedback_consumer::ConsumeScratch,
    motor_types: HashMap<MotorAddress, MotorType>,
    bus: B,
    motor_states: FxHashMap<MotorAddress, MotorState>,
    active_since: Option<Instant>,
    /// Echoing buses only: addresses whose latest Enable write has not yet been
    /// read back from the wire. A write only queues the frame, so drive traffic
    /// read after it may predate the Enable on the bus; the strict Run
    /// expectation and session pose start at the echo instead.
    enable_echo_pending: FxHashSet<MotorAddress>,
    /// Echoing buses only: this session's targets whose Enable + RunMode are not
    /// yet written, in target order. Every host frame solicits a drive reply and
    /// the mcp251x keeps only two received frames, so activation writes one
    /// target per interface per control period (see [`Self::issue_due_enable_writes`]),
    /// each only once its type-24 Off has settled on the wire.
    /// They stay in `enable_echo_pending` from activation until their own echo.
    enable_writes_pending: Vec<MotorAddress>,
    /// Earliest time the next staggered Enable wave may be written.
    enable_write_due: Option<Instant>,
    /// Echoing buses only: start of the current enable session or physical
    /// reference arm. Only type-24 Offs written since then can release an Enable.
    reporting_off_since: Option<Instant>,
    /// Echoing buses only: when the echo of each address's type-24 Off written
    /// since `reporting_off_since` was read (see [`Self::reporting_off_settled`]).
    reporting_off_echoed: FxHashMap<MotorAddress, Instant>,
    /// Echoing buses only: group starts of this process's enable-bootstrap
    /// writes (Enable waves, gate Offs, MIT solicits) per interface; see
    /// [`Self::write_mit_wires`].
    session_pacer: BurstPacer,
    /// Echoing buses only: each address's latest SetZero, at its host echo's
    /// read time (its write time until then). No Enable goes to that address
    /// before [`POST_SET_ZERO_QUIET`] has elapsed since.
    set_zero_on_wire: FxHashMap<MotorAddress, Instant>,
    invalid_feedback: FxHashSet<MotorAddress>,
    last_tau_ff: FxHashMap<String, f64>,
    feedback_velocity_trips: FxHashMap<String, u8>,
    last_feedback_samples: FxHashMap<String, FeedbackSample>,
    wrong_sign_state: FxHashMap<String, WrongSignState>,
    last_tick: Option<Instant>,
    active_reporting: ActiveReportingState,
    /// Last time each joint produced a feedback frame (for type-24 silence retry).
    last_feedback_rx: FxHashMap<String, Instant>,
    /// Latest observed status/version frame per address, whether or not it
    /// became pose (frames withheld from pose still prove the drive alive).
    /// Tracking only: never permission, liveness, or fault evidence.
    drive_frames: FxHashMap<MotorAddress, DriveFrameTrack>,
    /// Latest observed likely drive reboot, retained as inspection evidence.
    last_drive_reboot: Option<DriveRebootObservation>,
    /// Status frames decoded since the last [`Self::begin_tick_feedback`] call.
    last_refresh_frames: usize,
    /// Joints whose drives were successfully enabled for the current Active session.
    active_joints: HashSet<String>,
    /// Active session: per target address, the write instant of the earliest
    /// host frame the drive answers that no admitted pose has followed since
    /// (ADR 0036, *Solicited silence while Active*): its Enable, and every MIT
    /// batch, whether or not the batch commands that target. Keys are the
    /// session's targets, set at activation. `unanswered_writes` counts how
    /// many such writes went unanswered since the last admitted pose.
    unanswered_solicits: FxHashMap<MotorAddress, SolicitedState>,
    /// Single-drive-loss plans, episode and shed addresses (ADR 0038).
    drive_loss: drive_loss::DriveLossState,
}

impl<B: MotorBus> Supervisor<B> {
    /// Build supervisor from repo `config/` and URDF limits.
    ///
    /// Legacy calibration history is never read: every startup is Unreferenced
    /// and only a qualified reference workflow grants current reference.
    /// `homing.yaml calibration_record_path` is retained as the reserved
    /// history location (it locates the reference journal beside it).
    pub fn from_repo(repo_root: impl AsRef<Path>, bus: B) -> Result<Self, DavoutError> {
        Self::from_repo_inner(repo_root.as_ref(), bus)
    }

    /// Physical owner with the qualified Robstride reference workflow (ADR 0036):
    /// type-0 identity, target-only arming, SetZero, type-2 ack, type-17 `mechPos`
    /// readback, stop before storage, durable journal and per-joint selection.
    /// Every startup is Unreferenced; the journal never grants.
    ///
    /// `journal_path` must be absolute and distinct from the reserved history
    /// location; resource errors return before startup diagnostic traffic is
    /// transmitted.
    pub fn from_repo_with_physical_reference(
        repo_root: impl AsRef<Path>,
        bus: B,
        journal_path: impl AsRef<Path>,
    ) -> Result<Self, DavoutError> {
        Self::from_repo_physical_inner(repo_root.as_ref(), bus, journal_path.as_ref())
    }

    fn from_repo_physical_inner(root: &Path, bus: B, journal: &Path) -> Result<Self, DavoutError> {
        let (mut owner, record) = Self::build_unsynced(root, &resolve_config_dir(root), bus)?;
        reference_journal::distinct_history_path(&record, journal).map_err(|error| {
            DavoutError::Homing {
                message: error.to_string(),
            }
        })?;
        let journal = reference_journal::Journal::spawn(
            journal.to_owned(),
            #[cfg(any(test, feature = "reference-journal-test-support"))]
            None,
        )
        .map_err(|error| DavoutError::Homing {
            message: error.to_string(),
        })?;
        let mut installed = Vec::with_capacity(owner.stop_motors.len());
        for motor in owner.stop_motors.iter() {
            let gear = motor_position_scale(motor)?.abs();
            let ranges = robstride::motor_type::MitRanges::for_motor_type(motor.motor_type);
            installed.push((
                MotorAddress::from(motor),
                reference_physical::ContinuityBounds {
                    max_rate_rad_s: f64::from(ranges.velocity_scale) / gear,
                    quantization_rad: 2.0 * ranges.feedback_position_step() / gear,
                },
            ));
        }
        owner
            .reference_commits
            .install(journal, reference_commit::CommitSelection::CurrentPhysical);
        owner.reference_owner.install_physical(
            reference_physical::PhysicalBackend::new(),
            reference_physical::PhysicalDevices::new(installed),
        );
        owner.sync_active_reporting();
        Ok(owner)
    }

    fn from_repo_inner(root: &Path, bus: B) -> Result<Self, DavoutError> {
        Self::from_config_dir_inner(root, &resolve_config_dir(root), bus)
    }

    fn from_config_dir_inner(root: &Path, config_dir: &Path, bus: B) -> Result<Self, DavoutError> {
        let (mut supervisor, _) = Self::build_unsynced(root, config_dir, bus)?;
        // Arm type-24 when configured so free-drive Set Limits can see motion while
        // limp (Disabled/Ready). MIT Active still turns reporting off in sync below.
        supervisor.sync_active_reporting();
        Ok(supervisor)
    }

    /// Load and validate every resource without transmitting. Returns the owner
    /// and the reserved calibration history path (never read; the reference
    /// journal must occupy a distinct location).
    fn build_unsynced(
        root: &Path,
        config_dir: &Path,
        bus: B,
    ) -> Result<(Self, PathBuf), DavoutError> {
        let mut robot = load_robot_config_from(config_dir)?;
        let mut motors = load_motors_config_from(config_dir)?;
        let mut control = load_control_config_from(config_dir)?;
        if let Some(subset) = marengo_config::joint_subset_from_env() {
            marengo_config::apply_joint_subset(&mut robot, &mut motors, &mut control, &subset)?;
            info!(
                joint_count = robot.robot.joints.len(),
                "applied MARENGO_JOINT_SUBSET to Davout robot/motors/control"
            );
        }
        let homing_config = load_homing_config_from(config_dir)?;
        validate_safety_config(&robot, &motors, &control, &homing_config)?;
        let record_path = root.join(&homing_config.homing.calibration_record_path);
        let urdf_path = resolve_urdf_path(root, &robot)?;
        let urdf_robot = load_urdf(&urdf_path)?;
        validate_control_against_limits(&robot, &motors, &control)?;
        let limits = build_limits(&robot, &motors, &control, &urdf_robot)?;
        let installed_model = reference_model::InstalledReferenceModel::new(&robot, &urdf_robot)?;
        let motor_types = motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();
        let motor_addresses = motors.motors.iter().map(MotorAddress::from).collect();
        let supervisor = Self {
            mode: OperationalMode::Disabled,
            control_mode: ControlMode::Disabled,
            hardware_estop: false,
            fault_authority: FaultAuthority::default(),
            limits,
            robot,
            urdf_robot,
            installed_model,
            stop_motors: motors.motors.as_slice().into(),
            motor_addresses,
            mit_scratch: MitBatchScratch::default(),
            feedback_scratch: feedback_consumer::ConsumeScratch::default(),
            motors,
            control,
            homing_config,
            out_of_limits: homing_facets::OutOfLimitsFlags::default(),
            reference_authority: ReferenceAuthority::default(),
            reference_owner: reference_transaction::ReferenceOwner::default(),
            reference_commits: reference_commit::CommitOwner::default(),
            motor_types,
            bus,
            motor_states: FxHashMap::default(),
            active_since: None,
            enable_echo_pending: FxHashSet::default(),
            enable_writes_pending: Vec::new(),
            enable_write_due: None,
            reporting_off_since: None,
            reporting_off_echoed: FxHashMap::default(),
            session_pacer: BurstPacer::default(),
            set_zero_on_wire: FxHashMap::default(),
            invalid_feedback: FxHashSet::default(),
            last_tau_ff: FxHashMap::default(),
            feedback_velocity_trips: FxHashMap::default(),
            last_feedback_samples: FxHashMap::default(),
            wrong_sign_state: FxHashMap::default(),
            last_tick: None,
            active_reporting: ActiveReportingState::default(),
            last_feedback_rx: FxHashMap::default(),
            drive_frames: FxHashMap::default(),
            last_drive_reboot: None,
            last_refresh_frames: 0,
            active_joints: HashSet::new(),
            unanswered_solicits: FxHashMap::default(),
            drive_loss: drive_loss::DriveLossState::default(),
        };
        Ok((supervisor, record_path))
    }

    pub fn joint_homing_state(&self, joint: &str) -> JointHomingState {
        if self.has_latched_fault() {
            self.reference_authority.revoke();
            return JointHomingState::Faulted;
        }
        if self.reference_binding_valid() && self.reference_authority.contains(joint) {
            JointHomingState::Verified
        } else {
            JointHomingState::Unhomed
        }
    }

    /// Whether Davout currently has this joint's drive enabled (ACTIVE + in set).
    pub fn joint_drive_active(&self, joint: &str) -> bool {
        self.mode == OperationalMode::Active
            && self.active_joints.contains(joint)
            && self.joint_homing_state(joint) == JointHomingState::Verified
    }

    /// Joints currently in the Active enable set (empty when not Active).
    pub fn active_joints(&self) -> &HashSet<String> {
        &self.active_joints
    }

    pub fn joint_out_of_limits(&self, joint: &str) -> bool {
        self.out_of_limits.is_set(joint)
    }

    /// Wire facets for one joint: proto homing ordinal, drive_active, out_of_limits.
    pub fn joint_commissioning_wire(&self, joint: &str) -> (i32, bool, bool) {
        let homing = homing_facets::to_proto_homing_state(self.joint_homing_state(joint)) as i32;
        (
            homing,
            self.joint_drive_active(joint),
            self.joint_out_of_limits(joint),
        )
    }

    /// Per-joint limit policy (hard/soft bounds + margin config). ADR 0009.
    pub fn joint_limit_policy(&self, joint: &str) -> Option<&JointLimitPolicy> {
        self.limits.get(joint)
    }

    /// Command velocity cap (rad/s) from control.yaml resolution. ADR 0010.
    pub fn joint_velocity_cap(&self, joint: &str) -> Option<f64> {
        self.limits.get(joint).map(|lim| lim.velocity)
    }

    /// Encoder progress threshold in joint space, from the frozen installed receive mapping.
    ///
    /// Half a nominal feedback step plus a conservative decode/transform rounding allowance
    /// admits every new adjacent normal f32 level. It does not establish physical noise or
    /// reference validity. See ADR0025 for the numeric domain and rounding bound.
    pub fn joint_position_progress_threshold(&self, joint: &str) -> Result<f64, DavoutError> {
        let motor = self
            .stop_motors
            .iter()
            .find(|motor| motor.joint == joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        let gear = motor_position_scale(motor)?.abs();
        let ranges = robstride::motor_type::MitRanges::for_motor_type(motor.motor_type);
        let span = f64::from(ranges.position_scale) / gear;
        let step = ranges.feedback_position_step() / gear;
        let roundoff = 16.0 * f64::from(f32::EPSILON) * span;
        let minimum_adjacent = step - 2.0 * roundoff;
        let threshold = step / 2.0 + roundoff;
        // The decoder's highest code is slightly above +span. Keep all nonzero levels
        // normal and the whole decoded range finite after the actual joint-space f32 cast.
        if !span.is_finite()
            || !threshold.is_finite()
            || threshold <= 0.0
            || minimum_adjacent < f64::from(f32::MIN_POSITIVE)
            || threshold >= minimum_adjacent
            || span + 2.0 * step > f64::from(f32::MAX)
        {
            return Err(DavoutError::InvalidMotorConfig {
                joint: joint.to_string(),
                message: "position feedback grid is outside the qualified finite normal f32 range"
                    .to_string(),
            });
        }
        Ok(threshold)
    }

    /// Validate a proposed control policy against the installed robot, motor,
    /// and homing configuration before an overlay is installed or persisted.
    /// This reads policy only; callers still own installation and limit rebuilds.
    pub fn validate_control_candidate(
        &self,
        candidate: &ControlConfigFile,
    ) -> Result<(), marengo_config::ConfigError> {
        validate_safety_config(&self.robot, &self.motors, candidate, &self.homing_config)
    }

    /// Joint-space feedback sample if available (cache is already joint-space).
    ///
    /// While not [`OperationalMode::Active`], returns `None` when the sample is older than
    /// [`FREE_DRIVE_FEEDBACK_TTL`] so Berthier omits the joint from `RobotState` (Consul Offline).
    /// ACTIVE returns only a fresh, valid pose received in the current enable session.
    pub fn joint_feedback(&self, joint: &str) -> Option<JointFeedback> {
        let state = self.published_state(self.motor_index(joint)?)?;
        Some(JointFeedback {
            position_rad: f64::from(state.position_rad),
            velocity_rad_s: f64::from(state.velocity_rad_s),
            torque_nm: f64::from(state.torque_nm),
            temperature_c: state.temperature_c,
            // Compatibility indication only. Full independent vendor domains
            // remain available in safety_snapshot, including fault-only evidence.
            fault: state.fault | u16::from(self.fault_authority.joint_is_faulted(joint)),
            // `published_state` already refused samples without RX time,
            // so this age is always a real measurement, never a default.
            sample_age: state.updated.map(|t| t.elapsed()).unwrap_or(Duration::MAX),
        })
    }

    /// [`Self::joint_feedback`] gating for configured motor `index` (from [`Self::motor_index`]).
    fn published_state(&self, index: usize) -> Option<&MotorState> {
        let address = self.configured_address(index)?;
        let state = self.motor_states.get(&*address)?;
        if state.updated.is_none()
            || self.invalid_feedback.contains(&*address)
            || self.drive_loss.is_shed(&address)
        {
            return None;
        }
        if self.mode == OperationalMode::Active
            && !self.pose_is_current(&address, state, Instant::now())
        {
            return None;
        }
        if self.mode != OperationalMode::Active && state.is_stale(FREE_DRIVE_FEEDBACK_TTL) {
            return None;
        }
        Some(state)
    }

    /// Index of the configured motor for `joint`; same first match as [`motor_for_joint`].
    fn motor_index(&self, joint: &str) -> Option<usize> {
        self.motors
            .motors
            .iter()
            .position(|motor| motor.joint == joint)
    }

    /// Routing key of configured motor `index`, equal to `MotorAddress::from` that entry.
    ///
    /// `motors` is public, so the cached key is used only while it still matches the
    /// entry's interface and device id; otherwise an owned key is built from the entry.
    fn configured_address(&self, index: usize) -> Option<Cow<'_, MotorAddress>> {
        let motor = self.motors.motors.get(index)?;
        Some(self.address_for(index, motor))
    }

    /// `MotorAddress::from(motor)`, borrowed from the cache slot `index` when it matches.
    fn address_for(&self, index: usize, motor: &MotorEntry) -> Cow<'_, MotorAddress> {
        match self.motor_addresses.get(index) {
            Some(address) if is_motor_address(address, motor) => Cow::Borrowed(address),
            _ => Cow::Owned(MotorAddress::from(motor)),
        }
    }

    /// Rebuild cached routing keys that no longer match `motors` (public policy edits).
    fn sync_motor_addresses(&mut self) {
        let motors = &self.motors.motors;
        self.motor_addresses.truncate(motors.len());
        for (index, motor) in motors.iter().enumerate() {
            match self.motor_addresses.get_mut(index) {
                Some(address) if is_motor_address(address, motor) => {}
                Some(address) => *address = MotorAddress::from(motor),
                None => self.motor_addresses.push(MotorAddress::from(motor)),
            }
        }
    }

    /// Joint-space feedback position (rad) if available.
    pub fn joint_position_rad(&self, joint: &str) -> Option<f64> {
        self.joint_feedback(joint).map(|s| s.position_rad)
    }

    /// Joint-space feedback torque (Nm) if available.
    pub fn joint_torque_rad(&self, joint: &str) -> Option<f64> {
        self.joint_feedback(joint).map(|s| s.torque_nm)
    }

    /// Mark supervisor Ready when every configured joint is Verified.
    pub fn set_homing_complete(&mut self) -> Result<(), DavoutError> {
        self.refuse_reference_interference("set_homing_complete")?;
        self.require_fault_clear()?;
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::Homing {
                message: "Ready transition refused while ACTIVE; disable first".into(),
            });
        }
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        let joints = self.robot.robot.joints.clone();
        self.ensure_reference_for(&joints)?;
        self.mode = OperationalMode::Ready;
        self.sync_active_reporting();
        Ok(())
    }

    /// Synchronous qualified calibration for one-shot callers (`motor-repl set-zero`).
    /// Validates target/method/sign first, then drives the owner's own acquisition,
    /// durable commit and consumption to a terminal outcome. Owners without a
    /// current-selecting backend refuse before arming, SetZero or persistence.
    /// The resulting permission belongs to this owner and ends with it.
    ///
    /// Refuses when already [`OperationalMode::Active`] so success cannot
    /// `disable_all` out from under GravityComp / hold.
    pub fn calibrate_joint_zero(
        &mut self,
        joint: &str,
        operator: &str,
        sign_test_passed: bool,
    ) -> Result<f64, DavoutError> {
        self.require_fault_clear()?;
        let joint = joint.trim();
        self.reference_request_preflight(joint, Some(sign_test_passed))?;
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::Homing {
                message: "set-zero refused while ACTIVE; disable motors first".into(),
            });
        }
        if !self.reference_commits.selects_current() || !self.reference_owner.has_backend() {
            return Err(DavoutError::ReferenceUnsupported {
                operation: "reference calibration",
            });
        }
        let audit = ReferenceAudit {
            operator: operator.to_owned(),
            session: format!("calibrate-{}", std::process::id()),
        };
        let homing_timeout = self
            .homing_config
            .homing
            .effective_joint(joint)
            .and_then(|policy| Duration::try_from_secs_f64(policy.search_timeout_s).ok())
            .unwrap_or(Duration::ZERO);
        let handle = self
            .request_reference(joint, sign_test_passed, audit)
            .map_err(|error| reference_error(joint, error))?;
        let deadline = Instant::now() + homing_timeout + CALIBRATION_COMMIT_GRACE;
        loop {
            self.advance_reference_work()
                .map_err(|error| DavoutError::HomingVerify {
                    joint: joint.to_owned(),
                    message: error.to_string(),
                })?;
            match self
                .reference_outcome(&handle)
                .map_err(|error| reference_error(joint, error))?
            {
                ReferenceOutcome::Current { position_rad } => return Ok(f64::from(position_rad)),
                ReferenceOutcome::Failed { message } => {
                    return Err(DavoutError::HomingVerify {
                        joint: joint.to_owned(),
                        message,
                    })
                }
                ReferenceOutcome::InProgress if Instant::now() >= deadline => {
                    let _ = self.disable_all();
                    return Err(DavoutError::HomingVerify {
                        joint: joint.to_owned(),
                        message: "reference workflow did not finish before its deadline".into(),
                    });
                }
                ReferenceOutcome::InProgress => std::thread::sleep(CALIBRATION_POLL),
            }
        }
    }

    /// Current reference generation, distinct from ordinary motion-stop generation.
    pub fn reference_generation(&self) -> u64 {
        self.reference_binding_valid();
        self.reference_authority.generation()
    }

    fn reference_binding_valid(&self) -> bool {
        self.reference_binding_valid_with(Liveness::Judge)
    }

    /// [`Self::reference_binding_valid`] with each physical grant's liveness
    /// judged now or deferred to the next judged check. Only a feedback drain
    /// defers, for the observation it makes before reading the receive queue:
    /// receive times can be host read times, and an Active target's queued
    /// replies answer what the host asked, so judging liveness there counts
    /// the host's own read gap (e.g. a synchronous host stall; marengo-pi's
    /// gravity preflight is now swept across ticks) as drive silence. The
    /// drain judges liveness once it has read the queue.
    fn reference_binding_valid_with(&self, liveness: Liveness) -> bool {
        self.observe_staged_reference();
        self.observe_reference_commits();
        if self.has_latched_fault() || self.hardware_estop {
            self.reference_authority.revoke();
            return false;
        }
        if validate_safety_config(
            &self.robot,
            &self.motors,
            &self.control,
            &self.homing_config,
        )
        .is_err()
        {
            self.reference_authority.revoke();
            return false;
        }
        let realm_matches = self
            .reference_authority
            .realm()
            .is_some_and(|realm| self.reference_owner.realm_matches(&self.bus, realm));
        if !realm_matches {
            if self.reference_authority.realm().is_some() {
                self.reference_authority.revoke();
            }
            return false;
        }
        if !self
            .reference_authority
            .validate_consumed_model(&self.installed_model)
        {
            self.reference_authority.revoke();
            return false;
        }
        // Physical grants are per joint: a device that changed identity,
        // coordinate continuity or went silent beyond the watchdog loses only
        // its own grant (ADR 0036). A virtual device epoch change revokes all.
        let physical = self.reference_owner.is_physical();
        let window = Duration::from_millis(self.control.control.comm_watchdog_ms);
        let owner_busy = self.acquisition_busy()
            || self.reference_commits.busy()
            || self.reference_owner.pending_commit.is_some();
        for binding in self.reference_authority.consumed_bindings() {
            if binding.is_revoked() {
                continue;
            }
            let from = self.silence_from(&binding.joint, &binding.address);
            let judge = match liveness {
                Liveness::Judge => true,
                Liveness::Defer => matches!(from, SilenceFrom::Withheld(_)),
            };
            let current = if judge {
                self.reference_owner.live_device_epoch(
                    &self.bus,
                    &binding.address,
                    window,
                    owner_busy,
                    from,
                )
            } else {
                self.reference_owner
                    .current_device_epoch(&self.bus, &binding.address)
                    .ok_or(Lapse::Silent)
            };
            let identity_holds = !physical
                || binding.uid.is_some()
                    && self.reference_owner.physical.uid(&binding.address) == binding.uid;
            if current != Ok(binding.device_epoch) || !identity_holds {
                if physical {
                    let cause = if !identity_holds {
                        "device identity changed"
                    } else {
                        match current {
                            Err(Lapse::Silent) => "no feedback within comm_watchdog_ms",
                            Err(Lapse::OwedOnUnwritten) => {
                                "owed type-24 On not written within OWED_ON_WRITE_BOUND"
                            }
                            Ok(_) => "coordinate epoch changed",
                        }
                    };
                    warn!(
                        joint = %binding.joint,
                        interface = %binding.address.interface,
                        device_id = binding.address.device_id,
                        cause,
                        silence = ?self.reference_owner.physical.counted_silence(
                            &binding.address,
                            Instant::now(),
                            from,
                        ),
                        comm_watchdog_ms = self.control.control.comm_watchdog_ms,
                        peers_replying_during_silence =
                            self.peers_replying_during_silence(&binding.address),
                        unanswered_writes = self
                            .unanswered_solicits
                            .get(&binding.address)
                            .map_or(0, |slot| slot.unanswered_writes),
                        "physical reference grant revoked"
                    );
                    if identity_holds && current == Err(Lapse::Silent) {
                        self.reference_authority.revoke_binding_silent(binding);
                    } else {
                        self.reference_authority.revoke_binding(binding);
                    }
                } else {
                    self.reference_authority.revoke();
                    return false;
                }
            }
        }
        self.reference_authority
            .validate_binding(&self.motors, &self.homing_config, &self.control)
    }

    fn ensure_reference_for(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        self.ensure_reference_binding()?;
        self.require_reference_permission(joints)
    }

    /// [`Self::ensure_reference_for`] over the current active set, without copying it.
    /// A physical joint that lost its own grant stops the whole active session,
    /// unless the loss qualifies for a subtree shed (ADR 0038). Returns whether
    /// this call shed a subtree.
    fn ensure_reference_for_active(&mut self) -> Result<bool, DavoutError> {
        self.ensure_reference_binding()?;
        let shed = self.mode == OperationalMode::Active
            && !self.active_references_held()
            && self.shed_for_drive_loss()?;
        let result = self.require_reference_permission(&self.active_joints);
        if result.is_err() && self.mode == OperationalMode::Active {
            let _ = self.disable_all();
        }
        result.map(|()| shed)
    }

    /// Judge every Active joint's grant now. A qualifying single loss sheds its
    /// subtree (ADR 0038); any other lapse stops every drive and errors. The
    /// controller calls this when an Active pose reads stale before the grant
    /// that counts the same unanswered solicit was judged lapsed.
    pub fn check_active_references(&mut self) -> Result<(), DavoutError> {
        if self.mode != OperationalMode::Active {
            return Ok(());
        }
        self.ensure_reference_for_active().map(|_| ())
    }

    fn ensure_reference_binding(&mut self) -> Result<(), DavoutError> {
        self.refuse_reference_interference("ordinary reference admission")?;
        self.require_fault_clear()?;
        if !self.reference_binding_valid() {
            if self.mode == OperationalMode::Active {
                let _ = self.disable_all();
            }
            let message = if self.reference_owner.has_backend() {
                "current reference is unavailable or permanently revoked; acquire a qualified reference for each joint first"
            } else {
                "current reference is unavailable or permanently revoked; this owner has no qualified acquisition capability"
            };
            return Err(DavoutError::Homing {
                message: message.into(),
            });
        }
        Ok(())
    }

    fn require_reference_permission<'a>(
        &self,
        joints: impl IntoIterator<Item = &'a String>,
    ) -> Result<(), DavoutError> {
        for joint in joints {
            if !self.reference_authority.contains(joint) {
                return Err(DavoutError::Homing {
                    message: format!("joint {joint}: no private current-reference permission"),
                });
            }
        }
        Ok(())
    }

    fn reference_request_preflight(
        &self,
        joint: &str,
        sign_test: Option<bool>,
    ) -> Result<(), DavoutError> {
        let _ = self.reference_binding_valid();
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        let _motor =
            motor_for_joint(&self.motors, joint).ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.into(),
            })?;
        validate_safety_config(
            &self.robot,
            &self.motors,
            &self.control,
            &self.homing_config,
        )?;
        let policy = self
            .homing_config
            .homing
            .effective_joint(joint)
            .ok_or_else(|| DavoutError::Homing {
                message: format!("joint {joint} missing homing policy"),
            })?;
        if policy.method != marengo_config::HomingMethod::ManualReference {
            return Err(DavoutError::ReferenceUnsupported {
                operation: "configured reference method",
            });
        }
        if policy.sign_test_required && sign_test == Some(false) {
            return Err(DavoutError::HomingVerify {
                joint: joint.into(),
                message: "sign test required before reference request".into(),
            });
        }
        Ok(())
    }

    pub fn mode(&self) -> OperationalMode {
        self.mode
    }

    /// Persistent faults and last stop delivery, independent of pose caches.
    /// This slice has no qualified reset/recovery API.
    pub fn safety_snapshot(&self) -> SafetySnapshot {
        self.fault_authority.snapshot(self.hardware_estop)
    }

    pub fn has_latched_fault(&self) -> bool {
        self.fault_authority.is_latched()
    }

    /// Motion owners use this barrier to discard intent after stop/re-enable.
    pub fn stop_generation(&self) -> u64 {
        self.fault_authority.stop_generation()
    }

    fn require_fault_clear(&self) -> Result<(), DavoutError> {
        if let Some(fault) = self.fault_authority.first_fault() {
            return Err(DavoutError::FaultLatched {
                id: fault.id,
                class: fault.class,
                joint: fault.joint.clone(),
                message: fault.message.clone(),
            });
        }
        Ok(())
    }

    /// Read-only admission gate for control-owner mode entry after diagnostic RX.
    pub fn check_fault_authority(&self) -> Result<(), DavoutError> {
        self.require_fault_clear()
    }

    /// Trusted control-owner failure hook; never an operator reset or enable.
    pub fn latch_control_fault(&mut self, message: &str, joint: Option<&str>) {
        let address = joint
            .and_then(|name| motor_for_joint(&self.motors, name))
            .map(MotorAddress::from);
        let first = self.fault_authority.record(
            FaultClass::Controller,
            joint.filter(|_| address.is_some()).map(str::to_owned),
            address,
            message,
            DeviceFaultEvidence::default(),
        );
        if first {
            if self.reference_busy() {
                self.abort_reference_for_hazard(message);
            } else {
                let _ = self.disable_all();
            }
        }
    }

    fn record_runtime_error(&mut self, error: &DavoutError) -> bool {
        let (class, joint) = match error {
            DavoutError::InvalidFeedback { joint, .. } | DavoutError::Limit { joint, .. } => {
                (FaultClass::Feedback, Some(joint.as_str()))
            }
            DavoutError::CommWatchdog { joint, .. } => {
                // An early nonneutral request without initial pose is rejected,
                // but the bounded neutral bootstrap may still obtain feedback.
                let window = Duration::from_millis(self.control.control.comm_watchdog_ms);
                if motor_for_joint(&self.motors, joint)
                    .and_then(|motor| self.enable_bounds_start(&MotorAddress::from(motor)))
                    .is_some_and(|start| start.elapsed() <= window)
                {
                    return false;
                }
                (FaultClass::Communication, Some(joint.as_str()))
            }
            DavoutError::MotorFault { joint, .. } => (FaultClass::Device, Some(joint.as_str())),
            DavoutError::WrongSignWatchdog { joint, .. } => {
                (FaultClass::WrongSign, Some(joint.as_str()))
            }
            DavoutError::DangerZone { joint, .. } => (FaultClass::DangerZone, Some(joint.as_str())),
            DavoutError::Bus(
                BusError::Send { .. } | BusError::Driver(_) | BusError::ReceiveIncomplete { .. },
            ) => (FaultClass::Transport, None),
            DavoutError::Bus(BusError::MalformedFeedback { address, .. }) => {
                let joint = self
                    .motors
                    .motors
                    .iter()
                    .find(|motor| MotorAddress::from(*motor) == *address)
                    .map(|motor| motor.joint.as_str());
                (FaultClass::Feedback, joint)
            }
            _ => return false,
        };
        let address = joint
            .and_then(|name| motor_for_joint(&self.motors, name))
            .map(MotorAddress::from);
        self.fault_authority.record(
            class,
            joint.map(str::to_owned),
            address,
            &error.to_string(),
            DeviceFaultEvidence::default(),
        )
    }

    fn stop_after_runtime_error(&mut self, error: &DavoutError) {
        if self.record_runtime_error(error) {
            let _ = self.disable_all();
        }
    }

    /// Read-only marker for the current successful drive-enable session.
    /// Controllers can compare it across ticks to detect disable/re-enable
    /// cycles that happen between observations of the operational mode.
    /// Pose freshness remains enforced inside the supervisor.
    pub fn enable_session_started_at(&self) -> Option<Instant> {
        self.active_since
    }

    pub fn control_mode(&self) -> ControlMode {
        self.control_mode
    }

    pub fn set_control_mode(&mut self, mode: ControlMode) {
        if self.reference_busy() {
            return;
        }
        self.control_mode = mode;
        debug!(?mode, "control mode set");
    }

    /// Seed the tau_ff rate limiter with current measured torque for each joint.
    ///
    /// Called on mode transitions to slew from measured torque, bounded by the
    /// current feedforward policy. Without a seed, the limiter starts from zero.
    pub fn seed_tau_ff_rate_limiter(&mut self) {
        for motor in &self.motors.motors {
            let joint = &motor.joint;
            let measured = self.joint_torque_rad(joint).unwrap_or(0.0);
            let cap = self
                .limits
                .get(joint)
                .map(|limit| limit.tau_ff_max)
                .unwrap_or(0.0);
            let type_cap = self
                .control
                .control
                .motor_type_defaults
                .get(motor_type_key(motor.motor_type))
                .map(|defaults| defaults.tau_ff_max_nm)
                .unwrap_or(0.0);
            self.last_tau_ff.insert(
                joint.clone(),
                measured.clamp(-cap.min(type_cap), cap.min(type_cap)),
            );
        }
    }

    /// Read-only transport diagnostics; physical transmit/replacement is private.
    pub fn bus(&self) -> &B {
        &self.bus
    }

    pub fn set_hardware_estop(&mut self, asserted: bool) {
        // Runtime GPIO/input wiring is not yet integrated on the Pi bench — see
        // docs/position-hold-control-review.md (hardware E-stop gap).
        let was_asserted = self.hardware_estop;
        self.hardware_estop = asserted;
        if asserted {
            self.fault_authority.record(
                FaultClass::HardwareEstop,
                None,
                None,
                "hardware E-stop input asserted; releasing input is not qualified recovery",
                DeviceFaultEvidence::default(),
            );
            if !was_asserted {
                if self.reference_busy() {
                    self.abort_reference_for_hazard("hardware E-stop input asserted");
                } else {
                    let _ = self.disable_all();
                }
            }
            warn!("hardware E-stop input asserted — persistent fault");
        }
    }

    /// Enable only the listed joints. On any enable/run-mode failure, [`disable_all`].
    ///
    /// Every requested joint needs private current-reference authority. Empty targets
    /// are rejected. While Active, a different joint set is refused
    /// ([`DavoutError::ActiveSetChangeRefused`]); operators must Disable then Enable.
    pub fn enable_targets(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        self.refuse_during_episode("enable refused")?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode == OperationalMode::Active {
            self.ensure_reference_for_active()?;
            self.refuse_during_episode("enable refused")?;
        }
        if joints.is_empty() {
            return Err(DavoutError::Homing {
                message: "enable targets empty".into(),
            });
        }
        for joint in joints {
            if motor_for_joint(&self.motors, joint).is_none() {
                return Err(DavoutError::UnknownJoint {
                    joint: joint.clone(),
                });
            }
        }
        self.ensure_reference_for(joints)?;
        if self.mode == OperationalMode::Active {
            let want: HashSet<&str> = joints.iter().map(String::as_str).collect();
            let have: HashSet<&str> = self.active_joints.iter().map(String::as_str).collect();
            if want == have {
                return Ok(());
            }
            return Err(DavoutError::ActiveSetChangeRefused);
        }
        self.enable_targets_inner(joints)
    }

    /// Master + loaded joint facets for commissioning / Enable eligibility.
    ///
    /// `online` requires live CAN feedback. Master names come from the caller (full
    /// `robot.yaml` joints, not the loaded subset).
    pub fn commissioning_facets(
        &self,
        master_joint_names: &[String],
    ) -> (Vec<JointFacetInput>, Vec<JointFacetInput>) {
        let loaded_names: HashSet<&str> = self
            .motors
            .motors
            .iter()
            .map(|m| m.joint.as_str())
            .collect();
        let mut master = Vec::with_capacity(master_joint_names.len());
        for name in master_joint_names {
            let motor_mapped = loaded_names.contains(name.as_str());
            let feedback = self.joint_feedback(name);
            let online = feedback.is_some();
            let fault = self.fault_authority.joint_is_faulted(name)
                || feedback.map(|f| f.fault != 0).unwrap_or(false)
                || self.joint_homing_state(name) == JointHomingState::Faulted;
            master.push(JointFacetInput {
                name: name.clone(),
                homing_state: if motor_mapped {
                    self.joint_homing_state(name)
                } else {
                    JointHomingState::Unhomed
                },
                online,
                motor_mapped,
                fault,
                out_of_limits: self.joint_out_of_limits(name),
            });
        }
        let loaded: Vec<JointFacetInput> =
            master.iter().filter(|j| j.motor_mapped).cloned().collect();
        (master, loaded)
    }

    /// Resolve scoped Enable targets from persisted commissioning scope + facets.
    ///
    /// No scope file → full-master Robot Ready required. Persisted scope → Verified
    /// in-scope joints only. Never calls [`Self::set_homing_complete`].
    ///
    /// Runs the reporting sync, then drains pending feedback
    /// ([`Self::drain_feedback`]), so the facets judge what the drives sent,
    /// not how long the caller went without reading. A caller's synchronous
    /// work (marengo-pi's gravity preflight is now swept across ticks, so it
    /// no longer stalls reads) would otherwise revoke the grant of a drive
    /// whose reports are queued. A type-24 On that fell due meanwhile goes
    /// out first.
    pub fn resolve_enable_targets(
        &mut self,
        repo_root: impl AsRef<Path>,
    ) -> Result<Vec<String>, DavoutError> {
        self.require_fault_clear()?;
        let scope_path = default_commissioning_scope_path();
        let persisted = load_commissioning_scope(&scope_path)?;
        let ceiling = joint_subset_from_env();
        let effective = persisted
            .as_ref()
            .map(|scope| effective_commissioning_scope(&scope.joints, ceiling.as_ref()));
        let master_names = load_robot_config(repo_root.as_ref())?.robot.joints;
        self.sync_active_reporting();
        self.drain_feedback()?;
        let (master, loaded) = self.commissioning_facets(&master_names);
        select_enable_targets(&master, &loaded, effective.as_deref())
            .map_err(|message| DavoutError::Homing { message })
    }

    fn enable_targets_inner(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        self.ensure_reference_for(joints)?;
        // Physical admission re-reads every target's MCU identifier before any
        // enable write (ADR 0036); a changed or missing identity revokes it.
        if let Err(error) = self.verify_physical_identities(joints) {
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        // Status has no command-generation field. Drain already queued traffic
        // before activation; only later received poses may authorize this session.
        self.poll_feedback(Duration::ZERO)?;
        let result = (|| {
            let mut addresses = Vec::with_capacity(joints.len());
            for joint in joints {
                let motor = motor_for_joint(&self.motors, joint).ok_or_else(|| {
                    DavoutError::UnknownJoint {
                        joint: joint.clone(),
                    }
                })?;
                addresses.push(MotorAddress::from(motor));
            }
            if self.bus.echoes_transmissions() {
                // Every target stays pending from here until its own echo, so
                // traffic from a target not yet written is never held to Run
                // and never becomes session pose. Each target's type-24 Off goes
                // first, whatever this process applied: a physical drive keeps
                // reporting across host processes, and a report it built before
                // acting on an Enable follows that Enable's echo still in Reset.
                // Enables follow one per interface per control period, each only
                // once its Off has settled on the wire. A target zeroed less
                // than POST_SET_ZERO_QUIET ago gets neither its Off nor its
                // Enable until that quiet has elapsed: a drive drops frames it
                // receives in its post-SetZero blackout.
                let now = Instant::now();
                for (joint, address) in joints.iter().zip(&addresses) {
                    if let Some(until) = self
                        .set_zero_quiet_until(address)
                        .filter(|until| *until > now)
                    {
                        info!(
                            joint = %joint,
                            interface = %address.interface,
                            device_id = address.device_id,
                            wait_ms = until.duration_since(now).as_millis() as u64,
                            "Enable held until the post-SetZero quiet elapses"
                        );
                    }
                }
                self.begin_reporting_off_gate(now);
                self.enable_echo_pending.extend(addresses.iter().cloned());
                self.enable_writes_pending = addresses;
                self.issue_target_reporting_offs(now)?;
            } else {
                // Without echo, post-enable follows pop time: every target must
                // be written before activation.
                for address in &addresses {
                    self.write_enable(address)?;
                }
            }
            // Drives can queue status while enable/run-mode writes are still
            // in progress. Decode that traffic before establishing this session
            // so its later decode time cannot fabricate post-enable pose.
            // A receive failure here uses the same rollback as a write failure.
            self.poll_feedback(Duration::ZERO)?;
            self.ensure_reference_for(joints)?;
            self.active_joints = joints.iter().cloned().collect();
            // A new session has asked its targets nothing yet.
            self.unanswered_solicits.clear();
            for motor in &self.motors.motors {
                if self.active_joints.contains(&motor.joint) {
                    self.unanswered_solicits
                        .insert(MotorAddress::from(motor), SolicitedState::default());
                }
            }
            self.mode = OperationalMode::Active;
            let activated_at = Instant::now();
            self.active_since = Some(activated_at);
            self.enable_write_due = None;
            self.wrong_sign_state.clear();
            self.last_tau_ff.clear();
            self.last_tick = None;
            self.sync_active_reporting();
            info!(motor_count = joints.len(), "supervisor ACTIVE (targeted)");
            Ok(())
        })();
        if let Err(err) = &result {
            warn!(error = %err, "targeted enable failed — disable_all");
            let already_stopped = self.has_latched_fault();
            self.record_runtime_error(err);
            if !already_stopped {
                let _ = self.disable_all();
            }
        }
        result
    }

    fn loop_period(&self) -> Duration {
        Duration::from_micros(1_000_000 / u64::from(self.control.control.loop_hz.max(1)))
    }

    /// Remove the next Enable wave from `enable_writes_pending`: among targets
    /// whose post-SetZero quiet has elapsed and whose type-24 Off has settled on
    /// the wire, the first of each interface once the stagger period is due,
    /// plus every such target past its catch-up point.
    fn take_enable_wave(&mut self, now: Instant) -> Vec<MotorAddress> {
        let stagger_due = self.enable_write_due.is_none_or(|due| now >= due);
        let mut pending = std::mem::take(&mut self.enable_writes_pending);
        let mut wave: Vec<MotorAddress> = Vec::new();
        pending.retain(|address| {
            if !self.set_zero_quiet_elapsed(address, now)
                || !self.reporting_off_settled(address, now)
            {
                return true;
            }
            if !self.enable_catch_up(address, now)
                && (!stagger_due
                    || wave
                        .iter()
                        .any(|taken| taken.interface == address.interface))
            {
                return true;
            }
            wave.push(address.clone());
            false
        });
        self.enable_writes_pending = pending;
        wave
    }

    /// Echoing buses: open the window whose type-24 Offs can release Enables.
    fn begin_reporting_off_gate(&mut self, now: Instant) {
        self.reporting_off_since = Some(now);
        self.reporting_off_echoed.clear();
    }

    /// When this gate window last wrote `joint`'s type-24 Off, by any writer.
    fn reporting_off_written(&self, joint: &str) -> Option<Instant> {
        let since = self.reporting_off_since?;
        self.active_reporting
            .off_written_at(joint)
            .filter(|written| *written >= since)
    }

    /// The echo of `address`'s type-24 Off written in this gate window was read
    /// at least one control period before `now`. Read time bounds wire time
    /// from above, so any report the drive built before acting on that Off has
    /// left the wire before an Enable written now.
    fn reporting_off_settled(&self, address: &MotorAddress, now: Instant) -> bool {
        let Some(motor) = self
            .motors
            .motors
            .iter()
            .find(|motor| is_motor_address(address, motor))
        else {
            return false;
        };
        let Some(written) = self.reporting_off_written(&motor.joint) else {
            return false;
        };
        self.reporting_off_echoed
            .get(address)
            .is_some_and(|echoed| {
                *echoed >= written && now.saturating_duration_since(*echoed) >= self.loop_period()
            })
    }

    /// Wire-order marker: an Off echo counts only after this window wrote one.
    fn observe_reporting_off_echo(&mut self, address: &MotorAddress, received_at: Instant) {
        let written = self
            .motors
            .motors
            .iter()
            .find(|motor| is_motor_address(address, motor))
            .and_then(|motor| self.reporting_off_written(&motor.joint));
        if written.is_some() {
            self.reporting_off_echoed
                .insert(address.clone(), received_at);
        }
    }

    /// Record a SetZero to `address` on an echoing bus: its write time, then
    /// its host echo's read time. The latest SetZero wins.
    fn note_set_zero(&mut self, address: &MotorAddress, at: Instant) {
        self.set_zero_on_wire
            .entry(address.clone())
            .and_modify(|last| *last = (*last).max(at))
            .or_insert(at);
    }

    /// When `address`'s post-SetZero quiet ends, if a SetZero was recorded.
    fn set_zero_quiet_until(&self, address: &MotorAddress) -> Option<Instant> {
        self.set_zero_on_wire
            .get(address)
            .map(|at| *at + POST_SET_ZERO_QUIET)
    }

    /// No SetZero to `address` within [`POST_SET_ZERO_QUIET`] before `now`.
    fn set_zero_quiet_elapsed(&self, address: &MotorAddress, now: Instant) -> bool {
        self.set_zero_quiet_until(address)
            .is_none_or(|until| now >= until)
    }

    /// `address` may be inside its post-SetZero blackout: its SetZero is
    /// between [`POST_SET_ZERO_BLACKOUT_FROM`] and [`POST_SET_ZERO_QUIET`] old.
    /// Only echoing buses record SetZeros.
    fn set_zero_blackout_possible(&self, address: &MotorAddress, now: Instant) -> bool {
        self.set_zero_on_wire.get(address).is_some_and(|at| {
            let since = now.saturating_duration_since(*at);
            (POST_SET_ZERO_BLACKOUT_FROM..POST_SET_ZERO_QUIET).contains(&since)
        })
    }

    /// Start of `address`'s enable bounds in the current Active session:
    /// activation, or the end of its post-SetZero quiet when that is later.
    /// Catch-up, missing-echo, first-pose and liveness bounds count from here,
    /// so an Enable held for the quiet is not failed for being held.
    fn enable_bounds_start(&self, address: &MotorAddress) -> Option<Instant> {
        let since = self.active_since?;
        Some(
            self.set_zero_quiet_until(address)
                .map_or(since, |until| since.max(until)),
        )
    }

    /// Where grant liveness counts `joint`'s silence from (ADR 0036). While
    /// Active a target speaks only when the host writes to it, so its silence
    /// counts from the earliest write it has not answered; until its Enable
    /// echo, from its enable bounds start. Everything else counts from its
    /// last frame.
    fn silence_from(&self, joint: &str, address: &MotorAddress) -> SilenceFrom {
        if self.mode != OperationalMode::Active || !self.active_joints.contains(joint) {
            return SilenceFrom::LastFrame;
        }
        if self.enable_echo_pending.contains(address) {
            if let Some(since) = self.enable_bounds_start(address) {
                return SilenceFrom::Withheld(since);
            }
        }
        SilenceFrom::Solicited(self.unanswered_solicit(address))
    }

    /// Write instant of the earliest host frame `address` answers that no
    /// admitted pose has followed, in the current Active session.
    fn unanswered_solicit(&self, address: &MotorAddress) -> Option<Instant> {
        self.unanswered_solicits
            .get(address)
            .and_then(|slot| slot.earliest)
    }

    /// An Active target was written a frame it answers with a status, at
    /// `written_at` (sampled before the write, so never after the frame
    /// reached the wire). Only the earliest unanswered one counts for
    /// silence; every unanswered write counts for diagnostics.
    fn note_solicit(&mut self, address: &MotorAddress, written_at: Instant) {
        if self.mode != OperationalMode::Active {
            return;
        }
        match self.unanswered_solicits.get_mut(address) {
            Some(slot) => {
                slot.note_asked(written_at);
            }
            None => {
                let mut slot = SolicitedState::default();
                slot.note_asked(written_at);
                self.unanswered_solicits.insert(address.clone(), slot);
            }
        }
    }

    /// Latest observed likely drive reboot, if any frame since startup looked
    /// like a power-interruption return (Run, then Reset while run was
    /// expected, or Reset after longer than the comm watchdog).
    pub fn last_drive_reboot(&self) -> Option<DriveRebootObservation> {
        self.last_drive_reboot.clone()
    }

    /// Other drives on the silent drive's interface that replied after its
    /// last observed frame: nonzero proves the bus and host path were alive
    /// during the silence window, pointing at the drive rather than CAN.
    /// Revoke path only; the hot tick never calls this.
    fn peers_replying_during_silence(&self, silent: &MotorAddress) -> usize {
        let Some(silent_last) = self
            .drive_frames
            .get(silent)
            .map(|track| track.at)
            .or_else(|| self.reference_owner.physical.last_seen(silent))
        else {
            return 0;
        };
        self.motors
            .motors
            .iter()
            .filter(|motor| {
                motor.can_interface == silent.interface && motor.device_id != silent.device_id
            })
            .filter(|motor| {
                self.drive_frames
                    .get(&MotorAddress::from(*motor))
                    .is_some_and(|track| track.at > silent_last)
            })
            .count()
    }

    /// Half of `comm_watchdog_ms` past `address`'s enable bounds start: its Off
    /// and Enable are written regardless of the stagger, so slow ticks never
    /// stretch the stagger into the missing-echo bound.
    fn enable_catch_up(&self, address: &MotorAddress, now: Instant) -> bool {
        self.enable_bounds_start(address).is_some_and(|start| {
            now.saturating_duration_since(start)
                >= Duration::from_millis(self.control.control.comm_watchdog_ms) / 2
        })
    }

    /// Echoing buses: write the type-24 Off of each target whose Enable is
    /// pending, whose post-SetZero quiet has elapsed and whose stream this
    /// window has not turned Off yet, in reporting's one-write-per-interface-
    /// per-period slot unless the target is past its catch-up point.
    fn issue_target_reporting_offs(&mut self, now: Instant) -> Result<(), DavoutError> {
        for address in &self.enable_writes_pending {
            let Some(motor) = self
                .motors
                .motors
                .iter()
                .find(|motor| is_motor_address(address, motor))
            else {
                continue;
            };
            if self.reporting_off_written(&motor.joint).is_some()
                || !self.set_zero_quiet_elapsed(address, now)
            {
                continue;
            }
            let force = self.enable_catch_up(address, now);
            if let Some(result) = self
                .active_reporting
                .write_off(&mut self.bus, motor, now, force)
            {
                self.session_pacer.record_group(&motor.can_interface);
                result?;
            }
        }
        Ok(())
    }

    fn write_enable(&mut self, address: &MotorAddress) -> Result<(), DavoutError> {
        let joint = self
            .motors
            .motors
            .iter()
            .find(|motor| is_motor_address(address, motor))
            .map_or("", |motor| motor.joint.as_str());
        info!(
            joint,
            interface = %address.interface,
            device_id = address.device_id,
            "enabling motor"
        );
        let written_at = Instant::now();
        self.bus.enable_drive_at(address)?;
        self.bus.set_run_mode_at(address, RunMode::Mit)?;
        self.note_solicit(address, written_at);
        Ok(())
    }

    /// Write type-24 Offs, then the next staggered Enable wave once its control
    /// period is due. A target's Off and Enable wait for its post-SetZero quiet
    /// ([`POST_SET_ZERO_QUIET`]). Half of `comm_watchdog_ms` past a target's
    /// enable bounds start ([`Self::enable_bounds_start`]) its remaining Off,
    /// and its Enable once the Off has settled, are written regardless of the
    /// stagger; the missing-echo bound still fails closed for any target whose
    /// Off or Enable echo does not follow.
    fn issue_due_enable_writes(&mut self, now: Instant) -> Result<(), DavoutError> {
        if self.mode != OperationalMode::Active
            || self.enable_writes_pending.is_empty()
            || self.active_since.is_none()
        {
            return Ok(());
        }
        self.issue_target_reporting_offs(now)?;
        let wave = self.take_enable_wave(now);
        if wave.is_empty() {
            return Ok(());
        }
        for address in &wave {
            self.write_enable(address)?;
            self.session_pacer.record_group(&address.interface);
        }
        self.enable_write_due = Some(now + self.loop_period());
        Ok(())
    }

    /// True while some target of the current enable session has not had its
    /// staggered Enable written yet. Controllers keep their first-feedback
    /// grace open until this clears.
    pub fn enable_writes_pending(&self) -> bool {
        self.mode == OperationalMode::Active && !self.enable_writes_pending.is_empty()
    }

    /// Type-0 round trip for each physical target, bounded by
    /// [`reference_physical::IDENTITY_ADMISSION_TIMEOUT`] from the first request,
    /// plus two [`reference_physical::IDENTITY_ADMISSION_SPACING`] per further target.
    /// No enable frame is sent unless every target answers with the identifier
    /// its grant bound. Requests leave one per spacing, so the replies do not
    /// overrun the controller's receive buffers. A target still silent is asked
    /// again every [`reference_physical::IDENTITY_ADMISSION_RETRY`]; only replies
    /// popped after the first request count, and a missing reply still revokes.
    fn verify_physical_identities(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        if !self.reference_owner.is_physical() {
            return Ok(());
        }
        let mut targets = Vec::with_capacity(joints.len());
        for joint in joints {
            let binding =
                self.reference_authority
                    .binding_for(joint)
                    .ok_or_else(|| DavoutError::Homing {
                        message: format!("joint {joint}: no private current-reference permission"),
                    })?;
            let uid = binding.uid.ok_or_else(|| DavoutError::HomingVerify {
                joint: joint.clone(),
                message: "physical grant has no bound device identity".into(),
            })?;
            targets.push((joint.clone(), binding.address.clone(), uid));
        }
        let watermark = self.reference_owner.physical.watermark();
        let start = Instant::now();
        let spacing = reference_physical::IDENTITY_ADMISSION_SPACING;
        // The last target's first request leaves (n - 1) spacings late, and a
        // lost request is retried after at most n spacings.
        let deadline = start
            + reference_physical::IDENTITY_ADMISSION_TIMEOUT
            + spacing * u32::try_from(2 * targets.len().saturating_sub(1)).unwrap_or(u32::MAX);
        let mut asked: Vec<Option<Instant>> = vec![None; targets.len()];
        let mut next_request = start;
        let mut missing = Vec::with_capacity(targets.len());
        loop {
            missing.clear();
            for (index, (joint, address, bound)) in targets.iter().enumerate() {
                match self
                    .reference_owner
                    .physical
                    .identity_after(address, watermark)
                {
                    Some(observed) if observed == *bound => {}
                    Some(observed) => {
                        self.reference_authority.revoke_joint(joint);
                        return Err(DavoutError::HomingVerify {
                            joint: joint.clone(),
                            message: format!(
                                "device identity changed: bound {bound}, observed {observed}"
                            ),
                        });
                    }
                    None => missing.push(index),
                }
            }
            if missing.is_empty() {
                return Ok(());
            }
            let now = Instant::now();
            if now >= deadline {
                for index in &missing {
                    self.reference_authority.revoke_joint(&targets[*index].0);
                }
                return Err(DavoutError::HomingVerify {
                    joint: missing
                        .iter()
                        .map(|index| targets[*index].0.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                    message: "device identity reply missing at admission".into(),
                });
            }
            // A target is due when it was never asked or its last request is a
            // retry period old; at most one request leaves per spacing.
            let due_at = |index: usize| {
                asked[index].map_or(next_request, |at| {
                    (at + reference_physical::IDENTITY_ADMISSION_RETRY).max(next_request)
                })
            };
            if let Some(index) = missing
                .iter()
                .copied()
                .filter(|index| due_at(*index) <= now)
                .min_by_key(|index| asked[*index])
            {
                self.bus.get_device_id_at(&targets[index].1)?;
                asked[index] = Some(now);
                next_request = now + spacing;
                continue;
            }
            let next_due = missing
                .iter()
                .map(|index| due_at(*index))
                .min()
                .unwrap_or(deadline);
            let wait = deadline
                .min(next_due)
                .saturating_duration_since(Instant::now());
            // Admission can outlast a control period many times over (a target
            // in its post-SetZero blackout is asked again for up to the
            // deadline). Keep writing type-24 that falls due meanwhile, such as
            // an On held back until a peer's quiet ends: an Off stream is
            // silence the host causes, and liveness does not excuse it past
            // that quiet's end.
            self.sync_active_reporting();
            self.poll_feedback(wait.min(reference_physical::IDENTITY_ADMISSION_POLL))?;
        }
    }

    fn require_active_joint(&self, joint: &str) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        if self.mode != OperationalMode::Active {
            return Err(DavoutError::NotActive { mode: self.mode });
        }
        if !self.active_joints.contains(joint) {
            return Err(DavoutError::InactiveJoint {
                joint: joint.to_string(),
            });
        }
        Ok(())
    }

    /// Frames received in the last control tick (pre- and post-send drains combined).
    pub fn begin_tick_feedback(&mut self) {
        self.last_refresh_frames = 0;
    }

    /// Non-blocking drain of pending CAN status frames (control-loop path).
    ///
    /// Does not wait for the next MIT response; uses whatever is already in the
    /// SocketCAN RX queue from the previous tick's transmit.
    pub fn drain_feedback(&mut self) -> Result<usize, DavoutError> {
        self.refuse_reference_interference("ordinary feedback drain")?;
        self.poll_feedback(Duration::ZERO)
    }

    fn poll_feedback(&mut self, budget: Duration) -> Result<usize, DavoutError> {
        // Observe policy, realm, model, identity and coordinate changes before
        // projecting raw pose data. Restoring a public field after this drain
        // must not restore a revoked reference. Grant liveness is judged once
        // this drain has read the queue (below): judged here, a host stall
        // would count as drive silence, both the read gap (receive times can
        // be read times) and an Active target's replies still queued.
        self.enforce_degraded_deadline()?;
        let valid_before_read = self.reference_binding_valid_with(Liveness::Defer);
        let mut lost_active_reference = self.mode == OperationalMode::Active
            && !(valid_before_read && self.active_references_held());
        // A grant judged lapsed since the last drain (e.g. by a facet read)
        // may still be answered by a subtree shed (ADR 0038).
        if lost_active_reference && valid_before_read && self.shed_for_drive_loss()? {
            lost_active_reference = false;
        }
        if !lost_active_reference {
            if let Err(error) = self.issue_due_enable_writes(Instant::now()) {
                warn!(error = %error, "staggered enable write failed — disable_all");
                let already_stopped = self.has_latched_fault();
                self.record_runtime_error(&error);
                if !already_stopped {
                    let _ = self.disable_all();
                }
                return Err(error);
            }
        }
        let quiet = self.feedback_drain_quiet();
        let report = self
            .bus
            .recv_feedback_report(&self.motor_types, budget, quiet);
        let count = report.observations.len();
        self.last_refresh_frames = self.last_refresh_frames.saturating_add(count);
        let consumption = self.consume_feedback_report(report);
        let valid_after_read = self.reference_binding_valid();
        let mut lapsed_active_reference = !lost_active_reference
            && self.mode == OperationalMode::Active
            && !(valid_after_read && self.active_references_held());
        let mut first_error = consumption.first_error;
        let mut first_transition = consumption.first_transition;
        if let Some((error, transition)) = self.enable_echo_overdue(Instant::now()) {
            first_transition |= transition;
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        if lapsed_active_reference && valid_after_read && !first_transition && first_error.is_none()
        {
            match self.shed_for_drive_loss() {
                Ok(shed) => lapsed_active_reference = !shed,
                Err(error) => first_error = Some(error),
            }
        }
        if first_transition {
            let _ = self.disable_all();
        }
        if lost_active_reference || lapsed_active_reference {
            if self.mode == OperationalMode::Active {
                let _ = self.disable_all();
            }
            if first_error.is_none() {
                let message = if lost_active_reference {
                    "current reference was revoked before feedback projection"
                } else {
                    "current reference was revoked after feedback projection"
                };
                first_error = Some(DavoutError::Homing {
                    message: message.into(),
                });
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(count),
        }
    }

    /// Fail closed when an Active address's Enable has not been read back from
    /// the wire within `comm_watchdog_ms` of its enable bounds start (activation,
    /// or the end of its post-SetZero quiet when later): without the echo the
    /// strict Run expectation would never arm for that address. A target whose
    /// type-24 Off echo never arrived had its Enable withheld; it fails here too.
    fn enable_echo_overdue(&mut self, now: Instant) -> Option<(DavoutError, bool)> {
        if self.mode != OperationalMode::Active || self.enable_echo_pending.is_empty() {
            return None;
        }
        let bound_ms = self.control.control.comm_watchdog_ms;
        let bound = Duration::from_millis(bound_ms);
        let stop_motors = Arc::clone(&self.stop_motors);
        let (motor, address) = stop_motors.iter().find_map(|motor| {
            self.enable_echo_pending
                .iter()
                .find(|address| {
                    is_motor_address(address, motor)
                        && self
                            .enable_bounds_start(address)
                            .and_then(|start| now.checked_duration_since(start))
                            .is_some_and(|since| since > bound)
                })
                .map(|address| (motor, address))
        })?;
        let message = if self.enable_writes_pending.contains(address)
            && !self.reporting_off_echoed.contains_key(address)
        {
            format!(
                "type-24 Off not observed on the bus within {bound_ms} ms; Enable withheld, drive state unconfirmed"
            )
        } else {
            format!("Enable not observed on the bus within {bound_ms} ms; drive state unconfirmed")
        };
        let error = DavoutError::InvalidFeedback {
            joint: motor.joint.clone(),
            message,
        };
        let transition = self.fault_authority.record(
            FaultClass::DriveState,
            Some(motor.joint.clone()),
            Some(MotorAddress::from(motor)),
            &error.to_string(),
            DeviceFaultEvidence::default(),
        );
        Some((error, transition))
    }

    /// Session pose of `address` is current: received after activation, and
    /// every host frame the drive answers was answered or is younger than
    /// `comm_watchdog_ms` (ADR 0036, *Solicited silence while Active*). A
    /// target speaks only when written to, so the age of a pose the host asked
    /// nothing since is the host's silence, not the drive's: after a host
    /// stall the first batch is computed from it, and its replies renew it.
    fn pose_is_current(&self, address: &MotorAddress, state: &MotorState, now: Instant) -> bool {
        let Some(received_at) = state.updated else {
            return false;
        };
        let Some(active_since) = self.active_since else {
            return false;
        };
        received_at > active_since
            && received_at <= now
            && self.unanswered_solicit(address).is_none_or(|solicited| {
                now.saturating_duration_since(solicited)
                    <= Duration::from_millis(self.control.control.comm_watchdog_ms)
            })
    }

    /// Every joint of the Active set still holds its reference permission.
    fn active_references_held(&self) -> bool {
        self.active_joints
            .iter()
            .all(|joint| self.reference_authority.contains(joint))
    }

    fn check_comm_watchdog(&self, neutral_bootstrap: bool) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        let max_ms = self.control.control.comm_watchdog_ms;
        if self.mode != OperationalMode::Active {
            return Ok(());
        }
        if max_ms == 0 {
            return Err(DavoutError::InvalidCommand {
                joint: "*".into(),
                message: "communication watchdog must be positive".into(),
            });
        }
        for (index, motor) in self.motors.motors.iter().enumerate() {
            if !self.active_joints.contains(&motor.joint) {
                continue;
            }
            let address = self.address_for(index, motor);
            let Some(state) = self.motor_states.get(&*address) else {
                continue;
            };
            if state.fault != 0 {
                warn!(
                    joint = %motor.joint,
                    interface = %address.interface,
                    device_id = address.device_id,
                    fault = format_args!("{:#06x}", state.fault),
                    "motor fault latched"
                );
                return Err(DavoutError::MotorFault {
                    joint: motor.joint.clone(),
                    fault: state.fault,
                });
            }
        }
        let now = Instant::now();
        for (index, motor) in self.motors.motors.iter().enumerate() {
            if !self.active_joints.contains(&motor.joint) {
                continue;
            }
            let address = self.address_for(index, motor);
            let state = self.motor_states.get(&*address);
            if !self.invalid_feedback.contains(&*address)
                && state.is_some_and(|state| self.pose_is_current(&address, state, now))
            {
                continue;
            }
            // Bootstrap is only an inert MIT status solicit during the enable
            // deadline (from the address's enable bounds start, so an Enable
            // held for its post-SetZero quiet keeps it); never permit servo/FF
            // motion on absent or stale pose.
            if neutral_bootstrap
                && !self.invalid_feedback.contains(&*address)
                && self.enable_bounds_start(&address).is_some_and(|start| {
                    now.saturating_duration_since(start) <= Duration::from_millis(max_ms)
                })
            {
                continue;
            }
            let age_ms = state
                .and_then(|state| state.updated)
                .and_then(|received| now.checked_duration_since(received))
                .map(|age| age.as_millis().min(u128::from(u64::MAX)) as u64);
            let MotorAddress {
                interface,
                device_id,
            } = address.into_owned();
            return Err(DavoutError::CommWatchdog {
                ms: max_ms,
                joint: motor.joint.clone(),
                interface,
                device_id,
                age_ms,
            });
        }
        Ok(())
    }

    /// Filter and send one MIT command.
    pub fn send_mit_joint(
        &mut self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
    ) -> Result<(), DavoutError> {
        self.admit_and_send_mit(vec![cmd], Some(motor))
    }

    /// Filter and send a batch of MIT commands (one per joint).
    pub fn send_mit_batch(&mut self, cmds: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        if let Some(cmd) = cmds
            .iter()
            .find(|cmd| self.motor_index(&cmd.joint).is_none())
        {
            return Err(DavoutError::UnknownJoint {
                joint: cmd.joint.clone(),
            });
        }
        self.admit_and_send_mit(cmds, None)
    }

    /// `supplied` is the caller's motor for every command; `None` uses each
    /// command's configured motor.
    fn admit_and_send_mit(
        &mut self,
        cmds: Vec<MitJointCommand>,
        supplied: Option<&MotorEntry>,
    ) -> Result<(), DavoutError> {
        let mut scratch = std::mem::take(&mut self.mit_scratch);
        let result = self.admit_and_send_mit_with(cmds, supplied, &mut scratch);
        self.mit_scratch = scratch;
        result
    }

    fn admit_and_send_mit_with(
        &mut self,
        mut cmds: Vec<MitJointCommand>,
        supplied: Option<&MotorEntry>,
        scratch: &mut MitBatchScratch,
    ) -> Result<(), DavoutError> {
        self.enforce_degraded_deadline()?;
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        for cmd in &cmds {
            validate_mit_command(cmd)?;
        }
        if self.ensure_reference_for_active()? {
            // This call shed a subtree: the batch was composed before the
            // controller could know. Later batches to a shed joint are refused.
            if let Some(episode) = &self.drive_loss.episode {
                cmds.retain(|cmd| !episode.shed_joints.contains(&cmd.joint));
            }
        }
        self.sync_motor_addresses();
        scratch.configured.clear();
        for cmd in &cmds {
            self.require_active_joint(&cmd.joint)?;
            let index = self
                .motor_index(&cmd.joint)
                .ok_or_else(|| DavoutError::UnknownJoint {
                    joint: cmd.joint.clone(),
                })?;
            let configured = &self.motors.motors[index];
            let motor = supplied.unwrap_or(configured);
            if motor.can_interface != configured.can_interface
                || motor.device_id != configured.device_id
                || motor.motor_type != configured.motor_type
                || motor.direction != configured.direction
                || motor.gear_ratio != configured.gear_ratio
            {
                return Err(DavoutError::InvalidMotorConfig {
                    joint: cmd.joint.clone(),
                    message: "command motor differs from configured joint mapping".into(),
                });
            }
            // Same joint name <=> same first-match configured index.
            if cmd.joint != motor.joint || scratch.configured.contains(&index) {
                return Err(DavoutError::InvalidCommand {
                    joint: cmd.joint.clone(),
                    message: "joint/address mismatch or repeated joint in batch".into(),
                });
            }
            scratch.configured.push(index);
        }
        let batch_tick = Instant::now();
        if let Err(error) = self.prepare_mit_batch(cmds, supplied, scratch) {
            let shed = if matches!(error, DavoutError::CommWatchdog { .. }) {
                self.shed_after_stale_pose(scratch)?
            } else {
                false
            };
            if !shed {
                // Staged limiter/watchdog state is discarded: a rejected batch leaves
                // `last_tau_ff` and `wrong_sign_state` exactly as before admission.
                // Limit here is a rejected request (gains/neutral position),
                // unlike Limit produced while validating received feedback.
                if matches!(
                    error,
                    DavoutError::CommWatchdog { .. }
                        | DavoutError::MotorFault { .. }
                        | DavoutError::WrongSignWatchdog { .. }
                        | DavoutError::DangerZone { .. }
                ) {
                    self.stop_after_runtime_error(&error);
                }
                return Err(error);
            }
        }
        // Commit before transmit: a delivery failure's stop path clears this state.
        for stage in &scratch.staged {
            let joint = &self.motors.motors[stage.motor].joint;
            store_by_joint(&mut self.last_tau_ff, joint, stage.tau_ff_nm);
            if let Some(state) = stage.wrong_sign {
                store_by_joint(&mut self.wrong_sign_state, joint, state);
            }
        }
        // Admission is atomic; delivery can still fail partway through a physical
        // bus write and is reported as an error for the owner's stop path.
        let written_at = Instant::now();
        if let Err(error) = self.write_mit_wires(&scratch.wires) {
            let error = DavoutError::Bus(error);
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        // Each commanded target answers with a status (ADR 0036, *Solicited
        // silence while Active*). The controller commands every Active joint
        // each tick, so a target a batch leaves out ages as if asked: an
        // omission fails closed through the pose watchdog.
        for solicit in self.unanswered_solicits.values_mut() {
            solicit.note_asked(written_at);
        }
        self.last_tick = Some(batch_tick);
        Ok(())
    }

    /// Write an admitted batch. Outside the enable bootstrap it goes out back
    /// to back as before. While Enables of this session are unwritten (echoing
    /// buses only), a target whose Enable is not yet written gets no frame:
    /// its drive is in Reset, and its replies are neither session pose nor
    /// liveness until its own Enable echo. While a drive on an interface may
    /// still stream type-24 ([`Self::reporting_may_stream`]), each frame there
    /// starts a [`BurstPacer`] group after the previous Enable, gate Off or
    /// solicit. Type-24 has the lowest CAN priority, so reports falling due
    /// while a back-to-back batch holds the bus leave behind it with its
    /// replies and overrun the mcp251x's two receive buffers (2026-10-04 soak,
    /// cycle 5: rx_over_errors 9 to 10, Transport latched).
    fn write_mit_wires(&mut self, wires: &[AddressedMitCommand]) -> Result<(), BusError> {
        if self.mode != OperationalMode::Active || self.enable_writes_pending.is_empty() {
            return self.bus.mit_control_all_at(wires);
        }
        let now = Instant::now();
        for wire in wires {
            if self.enable_writes_pending.contains(&wire.address) {
                continue;
            }
            if self.reporting_may_stream(&wire.address.interface, now) {
                self.session_pacer.begin_group(&wire.address.interface);
            }
            self.bus.mit_control_all_at(std::slice::from_ref(wire))?;
        }
        Ok(())
    }

    /// A drive on `interface` may still stream type-24 in this enable
    /// session: a target whose Enable is unwritten and whose gate Off has not
    /// settled (a drive keeps reporting across processes, and the Off waits
    /// for the post-SetZero quiet), or a drive this process turned On and has
    /// not turned Off yet.
    fn reporting_may_stream(&self, interface: &str, now: Instant) -> bool {
        self.enable_writes_pending.iter().any(|address| {
            address.interface == interface && !self.reporting_off_settled(address, now)
        }) || self.motors.motors.iter().any(|motor| {
            motor.can_interface == interface && self.active_reporting.applied_on(&motor.joint)
        })
    }

    /// Filter every admitted command into `scratch.wires`, staging limiter/watchdog
    /// advances in `scratch.staged`; nothing is committed here.
    ///
    /// Joints are unique per batch, so each command reads only its own pre-batch
    /// limiter/watchdog entry, exactly as sequential in-place updates would.
    fn prepare_mit_batch(
        &self,
        cmds: Vec<MitJointCommand>,
        supplied: Option<&MotorEntry>,
        scratch: &mut MitBatchScratch,
    ) -> Result<(), DavoutError> {
        scratch.staged.clear();
        let mut neutral = true;
        for (slot, (cmd, &index)) in cmds.into_iter().zip(&scratch.configured).enumerate() {
            let motor = supplied.unwrap_or(&self.motors.motors[index]);
            let (filtered, q_meas, dq_meas) =
                self.filter_mit_core(cmd, motor, Some(index), self.last_tick)?;
            let wrong_sign = self
                .wrong_sign_step(&filtered, q_meas, dq_meas)
                .map(|(state, admitted)| admitted.map(|()| state))
                .transpose()?;
            validate_mit_command(&filtered)?;
            neutral &= filtered.kp == 0.0
                && filtered.kd == 0.0
                && filtered.torque_ff_nm == 0.0
                && filtered.velocity_rad_s == 0.0;
            let command = joint_to_motor_command(motor, &filtered)?;
            match scratch.wires.get_mut(slot) {
                Some(wire) => {
                    wire.address.interface.clone_from(&motor.can_interface);
                    wire.address.device_id = motor.device_id;
                    wire.command = command;
                }
                None => scratch.wires.push(AddressedMitCommand {
                    address: MotorAddress::from(motor),
                    command,
                }),
            }
            scratch.staged.push(StagedMitState {
                motor: index,
                tau_ff_nm: filtered.torque_ff_nm,
                wrong_sign,
            });
        }
        scratch.wires.truncate(scratch.staged.len());
        self.check_comm_watchdog(neutral)
    }

    /// Disable all drives (best-effort zero speed, zero-torque MIT, then DISABLE).
    #[tracing::instrument(skip(self))]
    pub fn disable_all(&mut self) -> Result<(), DavoutError> {
        self.cancel_reference_commits(ReferenceCancelReason::Disable);
        if self.acquisition_busy() {
            let terminal =
                self.cancel_live_reference_for_disable()
                    .map_err(|error| DavoutError::Homing {
                        message: error.to_string(),
                    })?;
            let failed_writes = terminal.stop.failed_writes();
            return if failed_writes > 0 {
                Err(DavoutError::StopDelivery { failed_writes })
            } else {
                Ok(())
            };
        }
        let report = self.perform_stop(true);
        let failed_writes = report.failed_writes();
        if failed_writes > 0 {
            Err(DavoutError::StopDelivery { failed_writes })
        } else {
            Ok(())
        }
    }

    /// One all-address best-effort burst. Reference cleanup suppresses reporting
    /// synchronization so a sensing lease cannot re-enable a stopped target.
    fn perform_stop(&mut self, synchronize_reporting: bool) -> StopReport {
        self.perform_stop_paced(synchronize_reporting, None)
    }

    /// [`Self::perform_stop`] with each address's three frames starting a
    /// [`BurstPacer`] group on its interface, so the replies do not overrun the
    /// controller's receive buffers. Only reference work paces; every other stop
    /// goes out at once.
    fn perform_stop_paced(
        &mut self,
        synchronize_reporting: bool,
        mut pacer: Option<&mut BurstPacer>,
    ) -> StopReport {
        if self.has_latched_fault() {
            self.reference_authority.revoke();
        }
        if self.mode == OperationalMode::Active {
            self.carry_session_silence(Instant::now());
        }
        let generation = self.fault_authority.begin_stop();
        let mut report = StopReport {
            generation,
            attempts: Vec::with_capacity(self.stop_motors.len() * 3),
        };
        for motor in self.stop_motors.iter() {
            if let Some(pacer) = pacer.as_mut() {
                pacer.begin_group(&motor.can_interface);
            }
            let address = MotorAddress::from(motor);
            if self.drive_loss.is_shed(&address) {
                // A shed drive is never commanded again except Disable (ADR 0038).
                let result = self.bus.disable_drive_at(&address);
                report.attempts.push(StopAttempt {
                    address,
                    action: StopAction::Disable,
                    error: result
                        .err()
                        .map(|error| bounded_message(&error.to_string())),
                });
                continue;
            }
            let result = self.bus.speed_control_at(&address, 0.0);
            report.attempts.push(StopAttempt {
                address: address.clone(),
                action: StopAction::ZeroSpeed,
                error: result
                    .err()
                    .map(|error| bounded_message(&error.to_string())),
            });
            let wire = MitCommand {
                device_id: motor.device_id,
                motor_type: motor.motor_type,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                kp: 0.0,
                kd: 0.0,
                torque_ff_nm: 0.0,
            };
            let result = self.bus.mit_control_all_at(&[AddressedMitCommand {
                address: address.clone(),
                command: wire,
            }]);
            report.attempts.push(StopAttempt {
                address: address.clone(),
                action: StopAction::NeutralMit,
                error: result
                    .err()
                    .map(|error| bounded_message(&error.to_string())),
            });
            let result = self.bus.disable_drive_at(&address);
            report.attempts.push(StopAttempt {
                address,
                action: StopAction::Disable,
                error: result
                    .err()
                    .map(|error| bounded_message(&error.to_string())),
            });
        }
        self.mode = OperationalMode::Disabled;
        self.control_mode = ControlMode::Disabled;
        self.active_since = None;
        self.enable_echo_pending.clear();
        self.enable_writes_pending.clear();
        self.enable_write_due = None;
        self.reporting_off_since = None;
        self.reporting_off_echoed.clear();
        self.active_joints.clear();
        self.feedback_velocity_trips.clear();
        self.last_feedback_samples.clear();
        self.last_feedback_rx.clear();
        self.drive_frames.clear();
        self.wrong_sign_state.clear();
        self.last_tau_ff.clear();
        self.last_tick = None;
        if synchronize_reporting {
            self.sync_active_reporting();
        }
        debug!("supervisor DISABLED");
        let failed_writes = report.failed_writes();
        if failed_writes > 0 {
            self.reference_authority.revoke();
            self.fault_authority.record(
                FaultClass::StopDelivery,
                None,
                None,
                "one or more stop writes failed; physical stop unconfirmed",
                DeviceFaultEvidence::default(),
            );
        }
        self.end_degraded_episode_on_stop();
        self.fault_authority.set_stop_report(report.clone());
        report
    }

    /// An Active session ends with a stop written at `stop_at`. Each target's
    /// silence keeps counting from where the session counted it, and a target
    /// the host had asked nothing since its last pose counts from the stop,
    /// which every drive answers: a host stall just before a Disable is not the
    /// drive's silence (ADR 0036, *Solicited silence while Active*).
    fn carry_session_silence(&mut self, stop_at: Instant) {
        let stop_motors = Arc::clone(&self.stop_motors);
        for motor in stop_motors.iter() {
            if !self.active_joints.contains(&motor.joint) {
                continue;
            }
            let address = MotorAddress::from(motor);
            let counts_from = match self.silence_from(&motor.joint, &address) {
                SilenceFrom::LastFrame => continue,
                SilenceFrom::Withheld(since) => since,
                SilenceFrom::Solicited(solicited) => solicited.unwrap_or(stop_at),
            };
            self.reference_owner
                .physical
                .count_silence_from(&address, counts_from);
        }
    }

    /// Light status solicit for Hardware-page sensing (no continuous type-24).
    ///
    /// When not [`OperationalMode::Active`], re-TX RobStride Disable (type-4) once per
    /// loaded motor that does **not** already desire Active Reporting (global diagnostics
    /// or an unexpired lease). Motors reply with OperationStatus (type-2); the normal
    /// control-loop drain picks those up on the next tick.
    ///
    /// No-op while Active (MIT owns feedback). Skips motors already covered by type-24 so
    /// the poll does not fight Set Limits / bench diagnostics. Best-effort per motor —
    /// one TX failure does not abort the rest of the bus (same honesty as [`Self::disable_all`]).
    pub fn solicit_status_feedback(&mut self) -> Result<(), DavoutError> {
        if self.reference_busy() {
            return Ok(());
        }
        if self.mode == OperationalMode::Active {
            return Ok(());
        }
        let global = self.control.control.bench.active_reporting_diagnostics;
        let now = Instant::now();
        let targets: Vec<(String, MotorAddress)> = self
            .motors
            .motors
            .iter()
            .filter(|motor| {
                !self
                    .active_reporting
                    .desired(&motor.joint, false, global, now)
            })
            .map(|motor| (motor.joint.clone(), MotorAddress::from(motor)))
            .collect();
        // Each Disable solicits a reply: on a real bus the writes start spaced
        // groups so five replies cannot overrun the controller.
        let mut pacer = self.bus.echoes_transmissions().then(BurstPacer::default);
        for (joint, address) in targets {
            if let Some(pacer) = pacer.as_mut() {
                pacer.begin_group(&address.interface);
            }
            if let Err(e) = self.bus.disable_drive_at(&address) {
                warn!(
                    joint = %joint,
                    error = %e,
                    "status solicit Disable TX failed; continuing remaining motors"
                );
            }
        }
        Ok(())
    }

    /// Free-drive sensing: type-24 per joint when diagnostics/leases say so and not ACTIVE.
    /// ACTIVE uses MIT status replies instead; type-24 must stay off then.
    ///
    /// Expires lease TTLs and resyncs type-24; call each control-loop iteration.
    /// Re-asserts enable on a 1 s heartbeat and when a joint's feedback goes stale
    /// while sensing is still desired (motors can drop Active Reporting mid-sweep).
    pub fn sync_active_reporting(&mut self) {
        if self.reference_busy() {
            return;
        }
        let mode_active = self.mode == OperationalMode::Active;
        let global = self.control.control.bench.active_reporting_diagnostics;
        let now = Instant::now();
        // A drive in its possible blackout drops a type-24 On, and the stream
        // would stay Off until the stale retry, longer than the grant liveness
        // bound. Hold those writes to the quiet's end; the drive's silence is
        // the host's until the held On is actually written (bounded by
        // OWED_ON_WRITE_BOUND), then counts from that write.
        let mut held: Vec<String> = if self.set_zero_on_wire.is_empty() {
            Vec::new()
        } else {
            self.motors
                .motors
                .iter()
                .filter(|motor| self.set_zero_blackout_possible(&MotorAddress::from(*motor), now))
                .map(|motor| motor.joint.clone())
                .collect()
        };
        // A shed drive gets no type-24 write ever again (ADR 0038).
        if !self.drive_loss.shed.is_empty() {
            held.extend(
                self.motors
                    .motors
                    .iter()
                    .filter(|motor| self.drive_loss.is_shed(&MotorAddress::from(*motor)))
                    .map(|motor| motor.joint.clone()),
            );
        }
        let deferred = self.active_reporting.sync_holding(
            &mut self.bus,
            &self.motors,
            mode_active,
            global,
            now,
            &self.last_feedback_rx,
            &held,
        );
        for joint in deferred {
            let Some(motor) = self.motors.motors.iter().find(|motor| motor.joint == joint) else {
                continue;
            };
            let address = MotorAddress::from(motor);
            if self.drive_loss.is_shed(&address) {
                continue;
            }
            if let Some(until) = self.set_zero_quiet_until(&address) {
                self.reference_owner
                    .physical
                    .owe_reporting_on(&address, until);
            }
        }
        let motors = &self.motors.motors;
        let reporting = &self.active_reporting;
        self.reference_owner
            .physical
            .settle_owed_reporting_ons(|address| {
                let Some(motor) = motors.iter().find(|motor| is_motor_address(address, motor))
                else {
                    return OwedOn::Released;
                };
                if reporting.applied_on(&motor.joint) {
                    OwedOn::Written(reporting.last_on_written(&motor.joint).unwrap_or(now))
                } else if reporting.desired(&motor.joint, mode_active, global, now) {
                    OwedOn::Owed
                } else {
                    OwedOn::Released
                }
            });
    }

    /// Acquire or upsert a client-minted lease for `joint`.
    pub fn acquire_active_reporting_lease(
        &mut self,
        joint: &str,
        client_id: &str,
        lease_id: &str,
        ttl: Duration,
    ) -> Result<(), DavoutError> {
        let now = Instant::now();
        self.active_reporting
            .acquire(joint, client_id, lease_id, ttl, now, &self.motors)
            .map_err(map_lease_error)?;
        self.sync_active_reporting();
        Ok(())
    }

    /// Renew an existing lease by `lease_id`.
    pub fn renew_active_reporting_lease(
        &mut self,
        joint: &str,
        client_id: &str,
        lease_id: &str,
        ttl: Duration,
    ) -> Result<(), DavoutError> {
        let now = Instant::now();
        self.active_reporting
            .renew(joint, client_id, lease_id, ttl, now, &self.motors)
            .map_err(map_lease_error)?;
        self.sync_active_reporting();
        Ok(())
    }

    /// Release a lease by `lease_id` (no-op if already gone).
    pub fn release_active_reporting_lease(
        &mut self,
        joint: &str,
        lease_id: &str,
    ) -> Result<(), DavoutError> {
        self.active_reporting
            .release(joint, lease_id, &self.motors)
            .map_err(map_lease_error)?;
        self.sync_active_reporting();
        Ok(())
    }

    fn feedback_drain_quiet(&self) -> Duration {
        Duration::from_micros(self.control.control.feedback_drain_quiet_us)
    }

    /// Step one joint command through the live filter, keeping limiter/watchdog
    /// advances on error (unit-test entry; production batches use [`Self::filter_mit_core`]
    /// through [`Self::send_mit_batch`]).
    #[cfg(test)]
    fn filter_mit_command(
        &mut self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
    ) -> Result<MitJointCommand, DavoutError> {
        let index = self
            .motor_index(&cmd.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: cmd.joint.clone(),
            })?;
        let configured = &self.motors.motors[index];
        if motor.joint != configured.joint
            || motor.can_interface != configured.can_interface
            || motor.device_id != configured.device_id
            || motor.motor_type != configured.motor_type
            || motor.direction != configured.direction
            || motor.gear_ratio != configured.gear_ratio
        {
            return Err(DavoutError::InvalidMotorConfig {
                joint: cmd.joint.clone(),
                message: "filter motor differs from configured joint mapping".into(),
            });
        }
        validate_safety_config(
            &self.robot,
            &self.motors,
            &self.control,
            &self.homing_config,
        )?;
        let tick = Instant::now();
        let (out, q_meas, dq_meas) =
            self.filter_mit_core(cmd, configured, Some(index), self.last_tick)?;
        // Unlike a batch, a single filter keeps limiter/watchdog advances on error.
        store_by_joint(&mut self.last_tau_ff, &out.joint, out.torque_ff_nm);
        if let Some((state, admitted)) = self.wrong_sign_step(&out, q_meas, dq_meas) {
            store_by_joint(&mut self.wrong_sign_state, &out.joint, state);
            admitted?;
        }
        self.last_tick = Some(tick);
        Ok(out)
    }

    /// Limits, envelope, danger zones and tau_ff rate limiting without mutating state.
    ///
    /// `feedback` is the configured motor index for `cmd.joint` (measured pose source).
    /// Returns the filtered command plus `(q_meas, dq_meas)`; the caller stores
    /// `torque_ff_nm` as the joint's new `last_tau_ff`.
    fn filter_mit_core(
        &self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
        feedback: Option<usize>,
        previous_tick: Option<Instant>,
    ) -> Result<(MitJointCommand, f64, f64), DavoutError> {
        validate_mit_command(&cmd)?;
        let lim = self
            .limits
            .get(&cmd.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: cmd.joint.clone(),
            })?;
        let type_key = motor_type_key(motor.motor_type);
        let defaults = self
            .control
            .control
            .motor_type_defaults
            .get(type_key)
            .ok_or_else(|| DavoutError::Limit {
                joint: cmd.joint.clone(),
                message: format!("no defaults for motor type {type_key}"),
            })?;

        let mut out = cmd;
        let gain_scale = motor_position_scale(motor)?.powi(2);
        if !gain_scale.is_finite()
            || gain_scale <= 0.0
            || out.kp / gain_scale > defaults.kp_max
            || out.kd / gain_scale > defaults.kd_max
        {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: "motor-space kp/kd exceed motor type max".to_string(),
            });
        }
        // Separate gated reads, as before: each applies its own freshness instant.
        let q_meas = feedback
            .and_then(|index| self.published_state(index))
            .map_or(out.position_rad, |state| f64::from(state.position_rad));
        let dq_meas = feedback
            .and_then(|index| self.published_state(index))
            .map_or(0.0, |state| f64::from(state.velocity_rad_s));
        let dq_envelope = if out.velocity_rad_s.abs() >= dq_meas.abs() {
            out.velocity_rad_s
        } else if dq_meas.abs() > 1e-9 {
            dq_meas
        } else {
            out.velocity_rad_s
        };
        let keepalive = out.kp == 0.0 && out.kd == 0.0 && out.torque_ff_nm == 0.0;
        if !keepalive {
            let clamped = clamp_position_in_envelope(lim, q_meas, dq_envelope, out.position_rad);
            if (clamped - out.position_rad).abs() > 1e-9 {
                trace!(
                    joint = %out.joint,
                    requested = out.position_rad,
                    clamped,
                    q_meas,
                    dq_cmd = out.velocity_rad_s,
                    "MIT position clamped to limit envelope"
                );
            }
            out.position_rad = clamped;
            if out.position_rad < lim.hard_lower() || out.position_rad > lim.hard_upper() {
                return Err(DavoutError::Limit {
                    joint: out.joint.clone(),
                    message: format!(
                        "position {} outside hard [{}, {}] after envelope clamp",
                        out.position_rad,
                        lim.hard_lower(),
                        lim.hard_upper()
                    ),
                });
            }
        } else if out.position_rad < lim.hard_lower() || out.position_rad > lim.hard_upper() {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!(
                    "position {} outside [{}, {}]",
                    out.position_rad,
                    lim.hard_lower(),
                    lim.hard_upper()
                ),
            });
        }
        let vel_cap = lim.velocity;
        let danger_zone_torque_cap = self.apply_danger_zone_clamps(&mut out, q_meas, dq_meas);
        if out.velocity_rad_s.abs() > vel_cap {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!("|velocity| {} > {}", out.velocity_rad_s, vel_cap),
            });
        }
        let torque_cap = lim
            .tau_ff_max
            .min(motor.bench.torque_limit_nm)
            .min(defaults.tau_ff_max_nm)
            .min(danger_zone_torque_cap);
        out.torque_ff_nm = rate_limit_tau_ff(
            self.last_tau_ff.get(&out.joint).copied(),
            out.torque_ff_nm,
            self.control.control.tau_ff_rate_limit_nm_per_s,
            previous_tick,
            torque_cap,
        );
        Ok((out, q_meas, dq_meas))
    }

    /// Next wrong-sign watchdog state for `out.joint` and whether it admits `out`.
    ///
    /// `None` when the watchdog is inactive (state untouched). Otherwise the caller
    /// stores the returned state, which also advances on a trip.
    fn wrong_sign_step(
        &self,
        out: &MitJointCommand,
        q_meas: f64,
        dq_meas: f64,
    ) -> Option<(WrongSignState, Result<(), DavoutError>)> {
        if self.control_mode != ControlMode::GravityComp {
            return None;
        }
        let cfg = &self.control.control.wrong_sign_watchdog;
        if !cfg.enabled {
            return None;
        }
        let mut state = self
            .wrong_sign_state
            .get(&out.joint)
            .copied()
            .unwrap_or_default();
        state.ticks_since_enable = state.ticks_since_enable.saturating_add(1);

        if state.ticks_since_enable <= cfg.grace_period_ticks {
            return Some((state, Ok(())));
        }
        if dq_meas.abs() <= cfg.min_velocity_rad_s {
            return Some((state, Ok(())));
        }

        let expected_sign = if q_meas >= 0.0 {
            cfg.expected_sign_at_positive_q as f64
        } else {
            -(cfg.expected_sign_at_positive_q as f64)
        };
        let actual_sign = out.torque_ff_nm.signum();

        if actual_sign == 0.0 || actual_sign == expected_sign {
            state.opposition_ticks = 0;
            return Some((state, Ok(())));
        }

        state.opposition_ticks = state.opposition_ticks.saturating_add(1);
        if state.opposition_ticks >= cfg.min_opposition_ticks {
            warn!(
                joint = %out.joint,
                ticks = state.opposition_ticks,
                torque_ff_nm = out.torque_ff_nm,
                q_meas,
                dq_meas,
                expected_sign,
                "wrong-sign watchdog tripped"
            );
            return Some((
                state,
                Err(DavoutError::WrongSignWatchdog {
                    joint: out.joint.clone(),
                    ticks: state.opposition_ticks,
                }),
            ));
        }
        Some((state, Ok(())))
    }

    fn apply_danger_zone_clamps(
        &self,
        cmd: &mut MitJointCommand,
        q_meas: f64,
        dq_meas: f64,
    ) -> f64 {
        let mut torque_cap = f64::INFINITY;
        for rule in &self.control.control.danger_zones {
            if rule.joint != cmd.joint {
                continue;
            }
            if q_meas <= rule.position_above_rad || dq_meas >= rule.velocity_below_rad_s {
                continue;
            }
            match rule.action.as_str() {
                "clamp_velocity" => {
                    if cmd.velocity_rad_s < 0.0 {
                        cmd.velocity_rad_s = cmd.velocity_rad_s.max(-rule.max_velocity_rad_s);
                    } else {
                        cmd.velocity_rad_s = cmd.velocity_rad_s.min(rule.max_velocity_rad_s);
                    }
                }
                "clamp_torque" => {
                    let cap = rule
                        .max_torque_nm
                        .unwrap_or(rule.max_velocity_rad_s)
                        .max(0.0);
                    cmd.torque_ff_nm = cmd.torque_ff_nm.clamp(-cap, cap);
                    torque_cap = torque_cap.min(cap);
                }
                _ => {}
            }
        }
        torque_cap
    }
}

fn validate_finite(joint: &str, field: &str, value: f64) -> Result<(), DavoutError> {
    if !value.is_finite() {
        return Err(DavoutError::InvalidCommand {
            joint: joint.to_string(),
            message: format!("{field} must be finite"),
        });
    }
    Ok(())
}

fn validate_mit_command(cmd: &MitJointCommand) -> Result<(), DavoutError> {
    for (field, value) in [
        ("position", cmd.position_rad),
        ("velocity", cmd.velocity_rad_s),
        ("kp", cmd.kp),
        ("kd", cmd.kd),
        ("torque_ff", cmd.torque_ff_nm),
    ] {
        validate_finite(&cmd.joint, field, value)?;
    }
    if cmd.kp < 0.0 || cmd.kd < 0.0 {
        return Err(DavoutError::InvalidCommand {
            joint: cmd.joint.clone(),
            message: "kp/kd must be nonnegative".into(),
        });
    }
    Ok(())
}

fn checked_wire_float(joint: &str, field: &str, value: f64) -> Result<f32, DavoutError> {
    validate_finite(joint, field, value)?;
    let wire = value as f32;
    if !wire.is_finite() {
        return Err(DavoutError::InvalidCommand {
            joint: joint.to_string(),
            message: format!("{field} overflows motor-space f32"),
        });
    }
    Ok(wire)
}

fn joint_to_motor_command(
    motor: &MotorEntry,
    cmd: &MitJointCommand,
) -> Result<MitCommand, DavoutError> {
    let scale = motor_position_scale(motor)?;
    let gain_scale = scale.powi(2);
    if !gain_scale.is_finite() || gain_scale <= 0.0 {
        return Err(DavoutError::InvalidMotorConfig {
            joint: motor.joint.clone(),
            message: "gear ratio squared must be finite and nonzero".into(),
        });
    }
    Ok(MitCommand {
        device_id: motor.device_id,
        motor_type: motor.motor_type,
        position_rad: checked_wire_float(&cmd.joint, "position", cmd.position_rad * scale)?,
        velocity_rad_s: checked_wire_float(&cmd.joint, "velocity", cmd.velocity_rad_s * scale)?,
        kp: checked_wire_float(&cmd.joint, "kp", cmd.kp / gain_scale)?,
        kd: checked_wire_float(&cmd.joint, "kd", cmd.kd / gain_scale)?,
        torque_ff_nm: checked_wire_float(&cmd.joint, "torque_ff", cmd.torque_ff_nm / scale)?,
    })
}

fn validate_motor_feedback(joint: &str, state: &MotorState) -> Result<(), DavoutError> {
    for (field, value) in [
        ("position", state.position_rad),
        ("velocity", state.velocity_rad_s),
        ("torque", state.torque_nm),
        ("temperature", state.temperature_c),
    ] {
        if !value.is_finite() {
            return Err(DavoutError::InvalidFeedback {
                joint: joint.to_string(),
                message: format!("{field} must be finite"),
            });
        }
    }
    Ok(())
}

/// Slewed and capped tau_ff from the joint's `previous` output (`None` = never sent).
fn rate_limit_tau_ff(
    previous: Option<f64>,
    target: f64,
    rate_nm_s: f64,
    last_tick: Option<Instant>,
    torque_cap_nm: f64,
) -> f64 {
    // A hard cap takes priority over slew continuity if measured torque or a
    // policy change leaves the previous output outside the current envelope.
    let prev = previous.unwrap_or(0.0).clamp(-torque_cap_nm, torque_cap_nm);
    let target = target.clamp(-torque_cap_nm, torque_cap_nm);
    let dt = last_tick
        .map(|t| t.elapsed().as_secs_f64())
        .unwrap_or(0.01)
        // Late ticks must not bank torque-step credit while control was paused.
        .clamp(1e-4, 0.01);
    let max_step = rate_nm_s * dt;
    let delta = (target - prev).clamp(-max_step, max_step);
    (prev + delta).clamp(-torque_cap_nm, torque_cap_nm)
}

/// `map.insert(joint.to_owned(), value)` that allocates only for a new joint.
fn store_by_joint<V>(map: &mut FxHashMap<String, V>, joint: &str, value: V) {
    if let Some(slot) = map.get_mut(joint) {
        *slot = value;
    } else {
        map.insert(joint.to_owned(), value);
    }
}

/// `*address == MotorAddress::from(motor)` without allocating.
fn is_motor_address(address: &MotorAddress, motor: &MotorEntry) -> bool {
    address.device_id == motor.device_id && address.interface == motor.can_interface
}

fn motor_position_scale(motor: &MotorEntry) -> Result<f64, DavoutError> {
    if !matches!(motor.direction, -1 | 1) {
        return Err(DavoutError::InvalidMotorConfig {
            joint: motor.joint.clone(),
            message: "direction must be -1 or 1".to_string(),
        });
    }
    if !motor.gear_ratio.is_finite() || motor.gear_ratio <= 0.0 {
        return Err(DavoutError::InvalidMotorConfig {
            joint: motor.joint.clone(),
            message: "gear_ratio must be finite and positive".to_string(),
        });
    }
    let direction = if motor.direction < 0 { -1.0 } else { 1.0 };
    Ok(direction * motor.gear_ratio)
}

fn motor_to_joint_state(motor: &MotorEntry, raw: MotorState) -> Result<MotorState, DavoutError> {
    let scale = motor_position_scale(motor)?;
    Ok(MotorState {
        position_rad: (f64::from(raw.position_rad) / scale) as f32,
        velocity_rad_s: (f64::from(raw.velocity_rad_s) / scale) as f32,
        torque_nm: (f64::from(raw.torque_nm) * scale) as f32,
        temperature_c: raw.temperature_c,
        fault: raw.fault,
        updated: raw.updated,
    })
}

pub(crate) fn build_limits(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
    urdf_robot: &urdf_rs::Robot,
) -> Result<FxHashMap<String, JointLimitPolicy>, DavoutError> {
    let mut map = FxHashMap::default();
    for joint_name in &robot.robot.joints {
        let urdf_lim = joint_limits(urdf_robot, joint_name)?;
        let mut bounds = joint_limit_bounds(urdf_robot, joint_name)?;
        let motor =
            motor_for_joint(motors, joint_name).ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint_name.clone(),
            })?;
        bounds.hard_lower = urdf_lim.lower.max(motor.bench.position_lower_rad);
        bounds.hard_upper = urdf_lim.upper.min(motor.bench.position_upper_rad);
        if bounds.hard_lower >= bounds.hard_upper {
            return Err(DavoutError::Limit {
                joint: joint_name.clone(),
                message: format!(
                    "URDF [{}, {}] and bench [{}, {}] hard bounds do not overlap",
                    urdf_lim.lower,
                    urdf_lim.upper,
                    motor.bench.position_lower_rad,
                    motor.bench.position_upper_rad
                ),
            });
        }
        let joint_cfg =
            control
                .control
                .joints
                .get(joint_name)
                .ok_or_else(|| DavoutError::Limit {
                    joint: joint_name.clone(),
                    message: format!("missing control.joints.{joint_name}"),
                })?;
        if let Some(lo) = joint_cfg.position_soft_lower_rad {
            bounds.soft_lower = lo.clamp(bounds.hard_lower, bounds.hard_upper);
        }
        if let Some(hi) = joint_cfg.position_soft_upper_rad {
            bounds.soft_upper = hi.clamp(bounds.hard_lower, bounds.hard_upper);
        }
        bounds.soft_lower = bounds
            .soft_lower
            .clamp(bounds.hard_lower, bounds.hard_upper);
        bounds.soft_upper = bounds
            .soft_upper
            .clamp(bounds.hard_lower, bounds.hard_upper);
        if bounds.soft_lower < bounds.hard_lower || bounds.soft_upper > bounds.hard_upper {
            return Err(DavoutError::Limit {
                joint: joint_name.clone(),
                message: format!(
                    "soft [{}, {}] not within hard [{}, {}]",
                    bounds.soft_lower, bounds.soft_upper, bounds.hard_lower, bounds.hard_upper
                ),
            });
        }
        let type_key = motor_type_key(motor.motor_type);
        let defaults = control
            .control
            .motor_type_defaults
            .get(type_key)
            .ok_or_else(|| DavoutError::Limit {
                joint: joint_name.clone(),
                message: format!("missing motor_type_defaults.{type_key}"),
            })?;
        let margin = limit_margin_from_config(joint_cfg);
        let velocity = resolve_joint_velocity_cap(joint_name, motor.motor_type, &control.control)?;
        let effort = urdf_lim
            .effort
            .min(motor.bench.torque_limit_nm)
            .min(robot.robot.bench.max_joint_torque_nm);
        let tau_ff_max = effort.min(defaults.tau_ff_max_nm);
        map.insert(
            joint_name.clone(),
            JointLimitPolicy {
                bounds,
                margin,
                velocity,
                effort,
                tau_ff_max,
            },
        );
    }
    Ok(map)
}

fn limit_margin_from_config(c: &marengo_config::JointControlEntry) -> LimitMarginConfig {
    LimitMarginConfig {
        min_rad: c.position_limit_margin_min_rad,
        k_v_s: c.position_limit_margin_k_v_s,
        k_stop: c.position_limit_margin_k_stop,
        velocity_deadband_rad_s: c.position_trajectory_velocity_deadband_rad,
        measured_fault_slack_rad: c.position_limit_measured_fault_slack_rad,
        decel_rad_s2: c.position_trajectory_accel_rad_s2,
    }
}

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_directory;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use crate::simulation::{
        InitialVirtualReference, SimulationBus, TxMatcher, TxOccurrence, TxRule,
    };
    use marengo_config::LimitPatch;
    use robstride::{CanFrame, CommunicationType, ReceivedCanFrame};

    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    use crate::test_directory as directory;

    fn bench_verify_all_joints(sup: &mut Supervisor<SimulationBus>) {
        for motor in &sup.motors.motors {
            assert_eq!(
                sup.joint_homing_state(&motor.joint),
                JointHomingState::Verified,
                "declared INITIAL virtual fixture"
            );
        }
    }

    fn bench_active(sup: &mut Supervisor<SimulationBus>) {
        bench_verify_all_joints(sup);
        sup.set_homing_complete().expect("ready");
        let joints = sup.robot.robot.joints.clone();
        sup.enable_targets(&joints).expect("enable");
    }

    fn receive_pose(sup: &mut Supervisor<SimulationBus>, joint: &str, q: f32, dq: f32) {
        let motor = motor_for_joint(&sup.motors, joint).expect("motor").clone();
        let scale = f32::from(motor.direction) * motor.gear_ratio as f32;
        let frame = status_frame_motor_space(
            motor.device_id,
            motor.motor_type,
            q * scale,
            dq * scale,
            0.0,
            25.0,
        );
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some(motor.can_interface),
                frame,
            ))
            .expect("finite raw fixture");
        sup.drain_feedback().expect("real pose receive");
    }

    fn initial_poses(sup: &mut Supervisor<SimulationBus>, target: &str, q: f32, dq: f32) {
        for motor in sup.motors.motors.clone() {
            let (q, dq) = if motor.joint == target {
                (q, dq)
            } else {
                (0.0, 0.0)
            };
            let scale = f32::from(motor.direction) * motor.gear_ratio as f32;
            sup.bus
                .queue_received(ReceivedCanFrame::full_data(
                    Some(motor.can_interface),
                    status_frame_motor_space(
                        motor.device_id,
                        motor.motor_type,
                        q * scale,
                        dq * scale,
                        0.0,
                        25.0,
                    ),
                ))
                .expect("finite raw fixture");
        }
        sup.drain_feedback().expect("real initial poses");
    }

    fn bench_ready_active(sup: &mut Supervisor<SimulationBus>) {
        bench_active(sup);
        initial_poses(sup, "", 0.0, 0.0);
    }

    fn installed_policy(
        motors: &MotorsConfigFile,
        control: &ControlConfigFile,
    ) -> Supervisor<SimulationBus> {
        let fixture = directory::TestDirectory::new("davout-unit-policy");
        std::fs::create_dir_all(fixture.path().join("config")).expect("config");
        for name in ["robot.yaml", "homing.yaml"] {
            std::fs::copy(
                repo_root().join("config").join(name),
                fixture.path().join("config").join(name),
            )
            .expect("policy");
        }
        std::fs::write(
            fixture.path().join("config/motors.yaml"),
            serde_yaml::to_string(motors).expect("motors YAML"),
        )
        .expect("motors");
        std::fs::write(
            fixture.path().join("config/control.yaml"),
            serde_yaml::to_string(control).expect("control YAML"),
        )
        .expect("control");
        std::fs::create_dir_all(fixture.path().join("assets/urdf")).expect("assets");
        std::fs::copy(
            repo_root().join("assets/urdf/marengo.urdf"),
            fixture.path().join("assets/urdf/marengo.urdf"),
        )
        .expect("URDF");
        Supervisor::from_simulation(
            fixture.path(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("installed initial policy")
    }

    #[test]
    fn commissioning_wire_publishes_drive_active_and_homing() {
        let bus = SimulationBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let joint = "right_elbow_pitch".to_string();
        let (homing, drive, ool) = sup.joint_commissioning_wire(&joint);
        assert_eq!(
            homing,
            crate::homing_facets::to_proto_homing_state(crate::JointHomingState::Unhomed) as i32
        );
        assert!(!drive);
        assert!(!ool);

        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("virtual initial fixture");
        bench_ready_active(&mut sup);
        let (homing, drive, ool) = sup.joint_commissioning_wire(&joint);
        assert_eq!(
            homing,
            crate::homing_facets::to_proto_homing_state(crate::JointHomingState::Verified) as i32
        );
        assert!(drive);
        assert!(!ool);

        sup.disable_all().expect("disable");
        let (_, drive, _) = sup.joint_commissioning_wire(&joint);
        assert!(!drive);
    }

    #[test]
    fn enable_targets_sets_active_joints_and_drive_active() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let a = "right_shoulder_roll".to_string();
        let b = "right_shoulder_pitch".to_string();
        sup.enable_targets(&[a.clone()]).expect("enable one");
        assert_eq!(sup.mode(), OperationalMode::Active);
        assert!(sup.joint_drive_active(&a));
        assert!(!sup.joint_drive_active(&b));
        assert_eq!(sup.active_joints().len(), 1);
        // Idempotent same-set Enable while Active.
        sup.enable_targets(&[a.clone()]).expect("same set");
        assert_eq!(sup.mode(), OperationalMode::Active);
    }

    #[test]
    fn enable_targets_refuses_different_set_while_active() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let a = "right_shoulder_roll".to_string();
        let b = "right_shoulder_pitch".to_string();
        sup.enable_targets(&[a.clone()]).expect("enable one");
        let err = sup
            .enable_targets(&[a.clone(), b.clone()])
            .expect_err("set change");
        assert!(
            matches!(err, DavoutError::ActiveSetChangeRefused),
            "got {err}"
        );
        assert_eq!(sup.mode(), OperationalMode::Active);
        assert!(sup.joint_drive_active(&a));
        assert!(!sup.joint_drive_active(&b));
    }

    #[test]
    fn enable_targets_partial_failure_disables_all() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("fixture");
        sup.bus.clear_trace();
        let rule = sup
            .bus
            .add_tx_rule(TxRule {
                matcher: TxMatcher {
                    communication_type: Some(3),
                    ..Default::default()
                },
                occurrence: TxOccurrence::Nth(2),
                receive: vec![],
                send_error: Some("second Enable delivery uncertain".into()),
            })
            .expect("rule");
        let joints: Vec<_> = sup
            .motors
            .motors
            .iter()
            .take(2)
            .map(|m| m.joint.clone())
            .collect();
        assert!(matches!(
            sup.enable_targets(&joints),
            Err(DavoutError::Bus(_))
        ));
        assert_eq!(sup.bus.rule_trigger_count(rule), 1, "reached second Enable");
        assert_eq!(sup.mode(), OperationalMode::Disabled);
        assert!(sup.active_joints().is_empty());
        let stopped: Vec<_> = sup
            .bus
            .frames()
            .iter()
            .filter(|f| f.id >> 24 == 4)
            .map(|f| f.id as u8)
            .collect();
        assert_eq!(
            stopped,
            sup.motors
                .motors
                .iter()
                .map(|m| m.device_id)
                .collect::<Vec<_>>(),
            "every installed motor stop attempted"
        );
    }

    #[test]
    fn mit_command_rejected_outside_active_joints() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let a = "right_shoulder_roll".to_string();
        let b = "right_shoulder_pitch".to_string();
        sup.enable_targets(&[a.clone()]).expect("enable");
        let motor = marengo_config::motor_for_joint(&sup.motors, &b)
            .expect("motor")
            .clone();
        let err = sup
            .send_mit_joint(
                MitJointCommand {
                    joint: b.clone(),
                    kp: 0.0,
                    kd: 0.0,
                    position_rad: 0.0,
                    velocity_rad_s: 0.0,
                    torque_ff_nm: 0.0,
                },
                &motor,
            )
            .expect_err("inactive");
        assert!(
            matches!(err, DavoutError::InactiveJoint { .. }),
            "got {err}"
        );
    }

    #[test]
    fn enable_targets_rejects_empty() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let err = sup.enable_targets(&[]).expect_err("empty");
        assert!(matches!(err, DavoutError::Homing { .. }));
    }

    #[test]
    fn measured_position_fault_marks_out_of_limits() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_ready_active(&mut sup);
        let joint = "right_elbow_pitch".to_string();
        let lim = *sup.joint_limit_policy(&joint).expect("policy");
        let outside = lim.hard_upper() + lim.margin.measured_fault_slack_rad + 0.5;
        let motor = marengo_config::motor_for_joint(&sup.motors, &joint)
            .expect("motor")
            .clone();
        let state = MotorState {
            position_rad: outside as f32,
            velocity_rad_s: 0.0,
            torque_nm: 0.0,
            temperature_c: 0.0,
            fault: 0,
            updated: Some(Instant::now()),
        };
        let err = sup
            .check_feedback_position(&motor, &state)
            .expect_err("out of limits");
        assert!(matches!(err, DavoutError::Limit { .. }));
        assert!(sup.joint_out_of_limits(&joint));
        let (_, _, ool) = sup.joint_commissioning_wire(&joint);
        assert!(ool);
    }
    #[test]
    fn measured_position_fault_latches_and_needs_restart_for_recovery() {
        // L-marengo-homing-01 recovery story: through the production consume
        // path, an OutOfLimits facet is always accompanied by a permanent
        // Feedback-class fault latch recorded in the same call, so nothing the
        // flag gates can pass without the fault gating it too. Recovery is a
        // fresh process: a rebuilt supervisor starts with no flags and no
        // latched fault.
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_ready_active(&mut sup);
        let joint = "right_elbow_pitch".to_string();
        let lim = *sup.joint_limit_policy(&joint).expect("policy");
        let outside = (lim.hard_upper() + lim.margin.measured_fault_slack_rad + 0.5) as f32;
        // Same framing as receive_pose, but the out-of-limits drain refuses by
        // design, so the drain result is observed, not unwrapped.
        let motor = marengo_config::motor_for_joint(&sup.motors, &joint)
            .expect("motor")
            .clone();
        let scale = f32::from(motor.direction) * motor.gear_ratio as f32;
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some(motor.can_interface),
                status_frame_motor_space(
                    motor.device_id,
                    motor.motor_type,
                    outside * scale,
                    0.0,
                    0.0,
                    25.0,
                ),
            ))
            .expect("finite raw fixture");
        let _ = sup.drain_feedback();
        assert!(sup.joint_out_of_limits(&joint));
        assert!(sup.has_latched_fault());
        assert!(sup.check_fault_authority().is_err());
        // Restart equivalence: fresh construction clears both the facet and the
        // latch, since neither is persisted.
        let fresh = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        assert!(!fresh.joint_out_of_limits(&joint));
        assert!(!fresh.has_latched_fault());
    }

    #[test]
    fn scoped_watchdog_ignores_cached_motor_fault_on_inactive_peer() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        bench_active(&mut sup);
        initial_poses(&mut sup, "right_elbow_pitch", 0.0, 0.0);
        sup.active_joints.clear();
        sup.active_joints.insert("right_elbow_pitch".to_string());

        let peer = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("inactive peer")
            .clone();
        sup.motor_states.insert(
            MotorAddress::from(&peer),
            MotorState {
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
                temperature_c: 25.0,
                fault: 1,
                updated: Some(Instant::now()),
            },
        );

        assert!(sup.check_comm_watchdog(false).is_ok());
    }

    #[test]
    fn calibrate_joint_zero_refuses_while_active() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_ready_active(&mut sup);
        let joint = sup.motors.motors[0].joint.clone();
        let err = sup
            .calibrate_joint_zero(&joint, "test", true)
            .expect_err("must refuse ACTIVE");
        assert!(
            matches!(err, DavoutError::Homing { ref message } if message.contains("ACTIVE")),
            "got {err}"
        );
        assert_eq!(sup.mode(), OperationalMode::Active);
    }

    #[test]
    fn calibrate_joint_zero_unknown_joint_before_enable() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        assert_eq!(sup.mode(), OperationalMode::Disabled);
        let err = sup
            .calibrate_joint_zero("not_a_real_joint", "test", true)
            .expect_err("unknown");
        assert!(matches!(err, DavoutError::UnknownJoint { .. }));
        assert_eq!(
            sup.mode(),
            OperationalMode::Disabled,
            "must not enable before joint resolve"
        );
    }

    fn elbow_limit_patch(lower: f64, upper: f64) -> LimitPatch {
        LimitPatch {
            joint: "right_elbow_pitch".to_string(),
            position_lower_rad: lower,
            position_upper_rad: upper,
            torque_limit_nm: None,
            position_soft_lower_rad: None,
            position_soft_upper_rad: None,
            velocity_max_rad_s: None,
        }
    }

    #[test]
    fn limit_patch_refuses_while_active() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        bench_ready_active(&mut sup);
        let before = *sup.joint_limit_policy("right_elbow_pitch").expect("policy");

        let err = sup
            .apply_limit_patch(&elbow_limit_patch(0.1, 1.5))
            .expect_err("must refuse ACTIVE");

        assert!(matches!(err, DavoutError::LimitPatchActive));
        assert_eq!(
            *sup.joint_limit_policy("right_elbow_pitch").expect("policy"),
            before
        );
    }

    #[test]
    fn limit_patch_rebuilds_runtime_policy() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");

        sup.apply_limit_patch(&elbow_limit_patch(0.1, 1.5))
            .expect("apply patch");

        let policy = sup.joint_limit_policy("right_elbow_pitch").expect("policy");
        assert!((policy.hard_lower() - 0.1).abs() < 1e-9);
        assert!((policy.hard_upper() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn restored_snapshot_uses_in_memory_config() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        let motor = sup
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == "right_elbow_pitch")
            .expect("motor");
        // Master URDF elbow hard upper is 1.2 — set bench below that so rebuild
        // uses the in-memory bench cap (URDF ∩ bench).
        motor.bench.position_upper_rad = 1.0;
        sup.control
            .control
            .joints
            .get_mut("right_elbow_pitch")
            .expect("control")
            .position_soft_upper_rad = Some(1.0);

        let motors = sup.motors.clone();
        let control = sup.control.clone();
        let urdf_robot = sup.urdf_robot.clone();
        sup.restore_limit_snapshot(motors, control, urdf_robot)
            .expect("snapshot install");

        let policy = sup.joint_limit_policy("right_elbow_pitch").expect("policy");
        assert!((policy.hard_upper() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn limit_patch_expands_urdf_when_past_current_hard() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        let urdf_before = joint_limits(sup.urdf_robot(), "right_elbow_pitch").expect("urdf");

        sup.apply_limit_patch(&elbow_limit_patch(-0.5, 3.0))
            .expect("expand past URDF hard");

        let policy = sup.joint_limit_policy("right_elbow_pitch").expect("policy");
        assert!((policy.hard_lower() - (-0.5)).abs() < 1e-9);
        assert!((policy.hard_upper() - 3.0).abs() < 1e-9);
        let urdf_after = joint_limits(sup.urdf_robot(), "right_elbow_pitch").expect("urdf");
        assert!(urdf_after.lower <= urdf_before.lower);
        assert!(urdf_after.upper >= urdf_before.upper);
        // Expand-only: a bound moves only when the patch is past the current URDF hard.
        assert!((urdf_after.lower - urdf_before.lower.min(-0.5)).abs() < 1e-9);
        assert!((urdf_after.upper - 3.0).abs() < 1e-9);
    }

    #[test]
    fn limit_patch_rejects_inverted_bounds_without_mutation() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        let before = *sup.joint_limit_policy("right_elbow_pitch").expect("policy");

        let err = sup
            .apply_limit_patch(&elbow_limit_patch(1.5, 0.1))
            .expect_err("inverted bounds must fail");

        assert!(matches!(err, DavoutError::Config(_)));
        assert_eq!(
            *sup.joint_limit_policy("right_elbow_pitch").expect("policy"),
            before
        );
    }

    #[test]
    fn limit_patch_rejects_measured_position_outside_new_bounds() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        receive_pose(&mut sup, "right_elbow_pitch", 1.0, 0.0);

        let err = sup
            .apply_limit_patch(&elbow_limit_patch(0.1, 0.9))
            .expect_err("measured q must remain inside hard bounds");

        assert!(matches!(err, DavoutError::Limit { .. }));
    }

    #[test]
    fn rate_limit_uses_shared_previous_tick() {
        use std::time::{Duration, Instant};

        let prev = Instant::now() - Duration::from_millis(10);
        let out1 = rate_limit_tau_ff(Some(0.0), 10.0, 100.0, Some(prev), 20.0);
        let out2 = rate_limit_tau_ff(Some(0.0), 10.0, 100.0, Some(prev), 20.0);
        assert!(
            (out1 - 1.0).abs() < 0.05,
            "j1 slew expected ~1.0, got {out1}"
        );
        assert!(
            (out2 - 1.0).abs() < 0.05,
            "j2 slew expected ~1.0, got {out2}"
        );
    }

    #[test]
    fn rate_limiter_seeds_on_mode_transition() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_active(&mut sup);
        initial_poses(&mut sup, "right_shoulder_pitch", 1.0, 0.5);

        // Set last_tau_ff to a known value different from measured torque (0.0).
        sup.last_tau_ff
            .insert("right_shoulder_pitch".to_string(), 5.0);

        // Seed — should update last_tau_ff to measured torque (0.0), not clear or keep 5.0.
        sup.seed_tau_ff_rate_limiter();

        let seeded = sup.last_tau_ff.get("right_shoulder_pitch").copied();
        assert!(
            seeded.is_some(),
            "shoulder_pitch should still have an entry (not cleared)"
        );
        assert!(
            (seeded.expect("shoulder_pitch should still have an entry") - 0.0).abs() < 1e-9,
            "expected measured torque 0.0, got {}",
            seeded.expect("shoulder_pitch should still have an entry")
        );
    }

    #[test]
    fn clamp_velocity_danger_zone_limits_downward_speed() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        receive_pose(&mut sup, "right_shoulder_pitch", 1.0, -0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let filtered = sup
            .filter_mit_command(
                MitJointCommand {
                    joint: "right_shoulder_pitch".to_string(),
                    kp: 10.0,
                    kd: 0.0,
                    position_rad: 1.0,
                    velocity_rad_s: -0.5,
                    torque_ff_nm: 2.0,
                },
                &motor,
            )
            .expect("filter");
        assert!(
            filtered.velocity_rad_s >= -0.45 - 1e-9,
            "expected clamp to max_velocity_rad_s 0.45, got {}",
            filtered.velocity_rad_s
        );
    }

    #[test]
    fn danger_zone_skips_when_measured_q_below_threshold() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        receive_pose(&mut sup, "right_shoulder_pitch", 0.2, -0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let filtered = sup
            .filter_mit_command(
                MitJointCommand {
                    joint: "right_shoulder_pitch".to_string(),
                    kp: 10.0,
                    kd: 0.0,
                    position_rad: 0.2,
                    velocity_rad_s: -0.5,
                    torque_ff_nm: 0.0,
                },
                &motor,
            )
            .expect("filter");
        assert!(
            (filtered.velocity_rad_s + 0.5).abs() < 1e-9,
            "rule requires measured q > 0.5, got velocity {}",
            filtered.velocity_rad_s
        );
    }

    #[test]
    fn send_mit_records_extended_frame_when_active() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_ready_active(&mut sup);
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        sup.send_mit_joint(
            MitJointCommand {
                joint: "right_elbow_pitch".to_string(),
                kp: 0.0,
                kd: 0.0,
                position_rad: 0.5,
                velocity_rad_s: 0.0,
                torque_ff_nm: 1.0,
            },
            &motor,
        )
        .expect("send");
        assert!(!sup.bus.frames().is_empty());
        assert!(sup.bus.frames()[0].extended);
    }

    #[test]
    fn solicit_status_feedback_sends_one_disable_per_motor_when_not_active() {
        use robstride::comm::{unpack_ext_id, CommunicationType};

        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Ignore init/sync side effects; status poll itself must not start type-24.
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.bus.clear_trace();
        let n = sup.motors.motors.len();
        assert!(n >= 1);
        sup.solicit_status_feedback().expect("solicit");
        let disable_tx: Vec<_> = sup
            .bus
            .frames()
            .iter()
            .filter(|frame| {
                unpack_ext_id(frame.id)
                    .map(|u| u.comm_type == CommunicationType::Disable.as_u8())
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(disable_tx.len(), n);
        assert!(
            !sup.bus.frames().iter().any(|frame| {
                unpack_ext_id(frame.id)
                    .map(|u| u.comm_type == CommunicationType::ActiveReporting.as_u8())
                    .unwrap_or(false)
            }),
            "status poll must not enable type-24"
        );
    }

    #[test]
    fn solicit_status_feedback_is_noop_when_active() {
        use robstride::comm::{unpack_ext_id, CommunicationType};

        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_ready_active(&mut sup);
        sup.bus.clear_trace();
        sup.solicit_status_feedback().expect("solicit");
        assert!(
            !sup.bus.frames().iter().any(|frame| {
                unpack_ext_id(frame.id)
                    .map(|u| u.comm_type == CommunicationType::Disable.as_u8())
                    .unwrap_or(false)
            }),
            "must not solicit while Active"
        );
    }

    #[test]
    fn solicit_status_feedback_skips_motors_when_global_ar_desired() {
        use robstride::comm::{unpack_ext_id, CommunicationType};

        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Master bench config: diagnostics already desires type-24 for every joint.
        assert!(sup.control.control.bench.active_reporting_diagnostics);
        sup.bus.clear_trace();
        sup.solicit_status_feedback().expect("solicit");
        assert!(
            !sup.bus.frames().iter().any(|frame| {
                unpack_ext_id(frame.id)
                    .map(|u| u.comm_type == CommunicationType::Disable.as_u8())
                    .unwrap_or(false)
            }),
            "must not Disable-solicit motors already covered by type-24 diagnostics"
        );
    }

    #[test]
    fn solicit_status_feedback_skips_leased_joint_only() {
        use robstride::comm::{unpack_ext_id, CommunicationType};

        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        assert!(sup.motors.motors.len() >= 2);
        sup.control.control.bench.active_reporting_diagnostics = false;
        let leased = sup.motors.motors[0].joint.clone();
        sup.acquire_active_reporting_lease(&leased, "consul", "lease-1", DEFAULT_LEASE_TTL)
            .expect("lease");
        sup.bus.clear_trace();
        sup.solicit_status_feedback().expect("solicit");
        let disable_device_ids: Vec<u8> = sup
            .bus
            .frames()
            .iter()
            .filter_map(|frame| {
                unpack_ext_id(frame.id).and_then(|u| {
                    (u.comm_type == CommunicationType::Disable.as_u8()).then_some(u.device_id)
                })
            })
            .collect();
        let leased_id = MotorAddress::from(&sup.motors.motors[0]).device_id;
        assert!(
            !disable_device_ids.contains(&leased_id),
            "leased joint must not receive Disable solicit"
        );
        assert_eq!(
            disable_device_ids.len(),
            sup.motors.motors.len() - 1,
            "all non-leased motors still solicited"
        );
    }

    #[test]
    fn solicit_status_feedback_continues_after_partial_tx_failure() {
        let mut sup = Supervisor::from_repo(repo_root(), SimulationBus::default())
            .expect("ordinary diagnostics");
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.bus.clear_trace();
        let rule = sup
            .bus
            .add_tx_rule(TxRule {
                matcher: TxMatcher {
                    communication_type: Some(4),
                    ..Default::default()
                },
                occurrence: TxOccurrence::Nth(1),
                receive: vec![],
                send_error: Some("first diagnostic solicit write failed".into()),
            })
            .expect("rule");
        sup.solicit_status_feedback()
            .expect("diagnostic best effort");
        assert_eq!(sup.bus.rule_trigger_count(rule), 1);
        let attempts: Vec<_> = sup
            .bus
            .transmissions()
            .iter()
            .filter(|tx| tx.frame.id >> 24 == 4)
            .collect();
        assert_eq!(attempts.len(), sup.motors.motors.len());
        assert!(!attempts[0].delivered);
        assert!(attempts[1..].iter().all(|tx| tx.delivered));
    }

    #[test]
    fn joint_feedback_omits_stale_samples_when_not_active() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        assert_ne!(sup.mode(), OperationalMode::Active);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let address = MotorAddress::from(&motor);
        let stale_at = Instant::now()
            .checked_sub(FREE_DRIVE_FEEDBACK_TTL + Duration::from_millis(1))
            .expect("clock");
        sup.motor_states.insert(
            address.clone(),
            MotorState {
                position_rad: 0.5,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
                temperature_c: 25.0,
                fault: 0,
                updated: Some(stale_at),
            },
        );
        assert!(
            sup.joint_feedback("right_shoulder_pitch").is_none(),
            "stale free-drive sample must not publish Online"
        );

        // Fresh sample is visible again.
        sup.motor_states.insert(
            address,
            MotorState {
                position_rad: 0.5,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
                temperature_c: 25.0,
                fault: 0,
                updated: Some(Instant::now()),
            },
        );
        assert!(sup.joint_feedback("right_shoulder_pitch").is_some());
    }

    #[test]
    fn joint_feedback_omits_stale_samples_while_active() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        bench_ready_active(&mut sup);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let address = MotorAddress::from(&motor);
        let stale_at = Instant::now()
            .checked_sub(FREE_DRIVE_FEEDBACK_TTL + Duration::from_secs(1))
            .expect("clock");
        sup.motor_states.insert(
            address,
            MotorState {
                position_rad: 0.5,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
                temperature_c: 25.0,
                fault: 0,
                updated: Some(stale_at),
            },
        );
        assert!(
            sup.joint_feedback("right_shoulder_pitch").is_none(),
            "control must not use an expired pose while ACTIVE"
        );
    }

    #[test]
    fn enable_sends_lifecycle_and_mit_run_mode() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Free-drive type-24 frames are covered elsewhere; this test asserts enable + MIT run mode.
        sup.control.control.bench.active_reporting_diagnostics = false;
        bench_verify_all_joints(&mut sup);
        sup.set_homing_complete().expect("ready");
        sup.bus.clear_trace();
        let joints = sup.robot.robot.joints.clone();
        sup.enable_targets(&joints).expect("enable");
        assert_eq!(sup.mode(), OperationalMode::Active);
        assert_eq!(sup.bus.frames().len(), sup.motors.motors.len() * 2);
        let first = robstride::unpack_ext_id(sup.bus.frames()[0].id).expect("enable id");
        assert_eq!(
            first.comm_type,
            robstride::CommunicationType::Enable.as_u8()
        );
        let second = robstride::unpack_ext_id(sup.bus.frames()[1].id).expect("run_mode id");
        assert_eq!(
            second.comm_type,
            robstride::CommunicationType::WriteParameter.as_u8()
        );
    }

    #[test]
    fn motor_transform_converts_feedback_to_joint_space() {
        let mut motor = motor_for_joint(
            &Supervisor::from_simulation(
                repo_root(),
                SimulationBus::default(),
                InitialVirtualReference::AllConfigured,
            )
            .expect("supervisor")
            .motors,
            "right_elbow_pitch",
        )
        .expect("motor")
        .clone();
        motor.direction = -1;
        motor.gear_ratio = 2.0;
        let joint = motor_to_joint_state(
            &motor,
            MotorState {
                position_rad: -1.0,
                velocity_rad_s: -0.5,
                torque_nm: -2.0,
                temperature_c: 42.0,
                fault: 7,
                updated: None,
            },
        )
        .expect("transform");
        assert!((joint.position_rad - 0.5).abs() < 1e-6);
        assert!((joint.velocity_rad_s - 0.25).abs() < 1e-6);
        assert!((joint.torque_nm - 4.0).abs() < 1e-6);
        assert_eq!(joint.temperature_c, 42.0);
        assert_eq!(joint.fault, 7);
    }

    #[test]
    fn joint_feedback_exposes_temperature_and_fault() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let address = MotorAddress::from(&motor);
        let now = Instant::now();
        sup.motor_states.insert(
            address,
            MotorState {
                position_rad: 0.5,
                velocity_rad_s: 0.1,
                torque_nm: 1.2,
                temperature_c: 42.0,
                fault: 7,
                updated: Some(now),
            },
        );
        let fb = sup
            .joint_feedback("right_shoulder_pitch")
            .expect("feedback");
        assert!((fb.position_rad - 0.5).abs() < 1e-6);
        assert!((fb.velocity_rad_s - 0.1).abs() < 1e-6);
        assert!((fb.torque_nm - 1.2).abs() < 1e-6);
        assert_eq!(fb.temperature_c, 42.0);
        assert_eq!(fb.fault, 7);
    }

    /// Status RX → `motor_to_joint_state` → cache → facade (not synthetic / not direct insert).
    fn status_frame_motor_space(
        device_id: u8,
        motor_type: MotorType,
        position_rad: f32,
        velocity_rad_s: f32,
        torque_nm: f32,
        temperature_c: f32,
    ) -> CanFrame {
        status_frame_motor_space_mode(
            device_id,
            motor_type,
            position_rad,
            velocity_rad_s,
            torque_nm,
            temperature_c,
            2,
        )
    }

    /// [`status_frame_motor_space`] with explicit status CAN-ID mode bits
    /// 22..23 (2 = Run, 0 = Reset) for reboot-transition fixtures.
    fn status_frame_motor_space_mode(
        device_id: u8,
        motor_type: MotorType,
        position_rad: f32,
        velocity_rad_s: f32,
        torque_nm: f32,
        temperature_c: f32,
        mode_bits: u32,
    ) -> CanFrame {
        let ranges = robstride::motor_type::MitRanges::for_motor_type(motor_type);
        let to_u16 = |value: f32, scale: f32| -> u16 {
            let clamped = value.clamp(-scale, scale);
            ((clamped / scale + 1.0) * 0x7FFF as f32)
                .round()
                .clamp(0.0, u16::MAX as f32) as u16
        };
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&to_u16(position_rad, ranges.position_scale).to_be_bytes());
        data[2..4].copy_from_slice(&to_u16(velocity_rad_s, ranges.velocity_scale).to_be_bytes());
        data[4..6].copy_from_slice(&to_u16(torque_nm, ranges.torque_scale).to_be_bytes());
        let temp_raw = ((temperature_c / 0.1).round() as u16).to_be_bytes();
        data[6..8].copy_from_slice(&temp_raw);
        CanFrame {
            id: robstride::pack_ext_id(
                CommunicationType::OperationStatus.as_u8(),
                u16::from(device_id),
                robstride::DEFAULT_HOST_ID,
            ) | (mode_bits << 22),
            data,
            extended: true,
        }
    }

    #[test]
    fn joint_feedback_transforms_once_on_refresh_with_inverted_scale() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let joint = "right_elbow_pitch".to_string();
        for m in &mut sup.motors.motors {
            if m.joint == joint {
                m.direction = -1;
                m.gear_ratio = 2.0;
                m.can_interface = "can0".to_string();
            }
        }
        sup = installed_policy(&sup.motors, &sup.control);
        let motor = motor_for_joint(&sup.motors, &joint)
            .expect("right_elbow_pitch")
            .clone();
        assert_eq!(motor.direction, -1);
        assert!((motor.gear_ratio - 2.0).abs() < 1e-9);
        sup.motor_types = sup
            .motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();

        // Motor-space sample; scale = direction * gear = -2 → joint (0.5, 0.25, 4.0).
        let motor_pos = -1.0_f32;
        let motor_vel = -0.5_f32;
        let motor_tau = -2.0_f32;
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some("can0".to_string()),
                status_frame_motor_space(
                    motor.device_id,
                    motor.motor_type,
                    motor_pos,
                    motor_vel,
                    motor_tau,
                    25.0,
                ),
            ))
            .expect("finite raw fixture");

        assert_eq!(sup.drain_feedback().expect("drain"), 1);
        let fb = sup.joint_feedback(&joint).expect("feedback");
        assert!(
            (fb.position_rad - 0.5).abs() < 1e-3,
            "expected joint-space 0.5 after one transform, got {}",
            fb.position_rad
        );
        assert!(
            (fb.velocity_rad_s - 0.25).abs() < 1e-3,
            "expected joint-space 0.25, got {}",
            fb.velocity_rad_s
        );
        assert!(
            (fb.torque_nm - 4.0).abs() < 1e-2,
            "expected joint-space 4.0 Nm, got {}",
            fb.torque_nm
        );
        // Double-transform would yield position motor/scale² = -1/4 = -0.25.
        assert!(
            (fb.position_rad + 0.25).abs() > 0.1,
            "must not re-transform on read"
        );
        assert!(
            (sup.joint_position_rad(&joint).expect("pos") - 0.5).abs() < 1e-3,
            "scalar accessor must match facade"
        );
    }

    #[test]
    fn feedback_state_is_keyed_by_bus_address() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.motors.motors[0].can_interface = "can0".to_string();
        sup.motors.motors[0].device_id = 1;
        sup.motors.motors[1].can_interface = "can1".to_string();
        sup.motors.motors[1].device_id = 1;
        sup = installed_policy(&sup.motors, &sup.control);
        sup.motor_types = sup
            .motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();
        let status = [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x00, 0xC8];
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some("can0".to_string()),
                CanFrame {
                    id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID) | (2 << 22),
                    data: status,
                    extended: true,
                },
            ))
            .expect("finite raw fixture");
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some("can1".to_string()),
                CanFrame {
                    id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                    data: status,
                    extended: true,
                },
            ))
            .expect("finite raw fixture");

        let count = sup.drain_feedback().expect("feedback");

        assert_eq!(count, 2);
        let j0 = sup.motors.motors[0].joint.clone();
        let j1 = sup.motors.motors[1].joint.clone();
        assert!(sup.joint_feedback(&j0).is_some());
        assert!(sup.joint_feedback(&j1).is_some());
    }

    #[test]
    fn active_feedback_velocity_above_limit_faults() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_active(&mut sup);
        let motor = sup.motors.motors[0].clone();
        let limit = sup.limits.get(&motor.joint).expect("limits").velocity;
        let overspeed = limit + FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S + 0.15;
        let dt = 0.005;
        let t0 = Instant::now();
        let mut pos = 0.30_f32;
        let mut state = MotorState {
            position_rad: pos,
            velocity_rad_s: 0.0,
            torque_nm: 0.0,
            temperature_c: 25.0,
            fault: 0,
            updated: Some(t0),
        };
        sup.check_feedback_velocity(&motor, &mut state, t0)
            .expect("seed sample");

        for step in 1..=u64::from(FEEDBACK_VELOCITY_LIMIT_TRIPS) {
            pos += (overspeed * dt) as f32;
            let t = t0 + Duration::from_secs_f64(dt * step as f64);
            let mut sample = MotorState {
                position_rad: pos,
                velocity_rad_s: overspeed as f32,
                torque_nm: 0.0,
                temperature_c: 25.0,
                fault: 0,
                updated: Some(t),
            };
            let result = sup.check_feedback_velocity(&motor, &mut sample, t);
            if step < u64::from(FEEDBACK_VELOCITY_LIMIT_TRIPS) {
                result.expect("warn-only overspeed sample");
            } else {
                let err = result.expect_err("sustained overspeed feedback");
                assert!(matches!(err, DavoutError::Limit { .. }));
                assert!(err.to_string().contains("feedback |velocity|"));
            }
        }
    }

    #[test]
    fn stationary_feedback_velocity_spike_is_ignored() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.motors.motors[0].can_interface = "can0".to_string();
        sup.motors.motors[0].device_id = 1;
        sup = installed_policy(&sup.motors, &sup.control);
        sup.motor_types = sup
            .motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();
        bench_ready_active(&mut sup);
        let stationary_overspeed = ReceivedCanFrame::full_data(
            Some("can0".to_string()),
            CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID) | (2 << 22),
                data: [0x7f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0x00, 0xc8],
                extended: true,
            },
        );

        sup.bus
            .queue_received(stationary_overspeed.clone())
            .expect("finite raw fixture");
        sup.drain_feedback().expect("first spike ignored");
        sup.bus
            .queue_received(stationary_overspeed)
            .expect("finite raw fixture");
        sup.drain_feedback()
            .expect("stationary repeated spike remains ignored");

        assert!(sup.feedback_velocity_trips.is_empty());
        let joint = sup.motors.motors[0].joint.clone();
        let state = sup.joint_feedback(&joint).expect("sanitized state cached");
        assert_eq!(state.velocity_rad_s, 0.0);
    }

    #[test]
    fn cruise_near_limit_measured_velocity_does_not_fault() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_active(&mut sup);
        let motor = sup.motors.motors[0].clone();
        let limit = sup.limits.get(&motor.joint).expect("limits").velocity;
        let t0 = Instant::now();
        let mut first = MotorState {
            position_rad: 0.30,
            velocity_rad_s: 5.0,
            torque_nm: 0.0,
            temperature_c: 25.0,
            fault: 0,
            updated: Some(t0),
        };
        sup.check_feedback_velocity(&motor, &mut first, t0)
            .expect("seed sample");

        let near_limit = limit + 0.10;
        let dt = 0.005;
        let mut pos = 0.30_f32;
        for i in 0..5 {
            pos += (near_limit * dt) as f32;
            let t = t0 + Duration::from_secs_f64(dt * f64::from(i + 1));
            let mut state = MotorState {
                position_rad: pos,
                velocity_rad_s: 5.0,
                torque_nm: 0.0,
                temperature_c: 25.0,
                fault: 0,
                updated: Some(t),
            };
            sup.check_feedback_velocity(&motor, &mut state, t)
                .expect("near-limit cruise should stay enabled");
        }
        assert!(sup.feedback_velocity_trips.is_empty());
    }

    #[test]
    fn active_feedback_velocity_cache_uses_position_delta() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_active(&mut sup);
        let motor = sup.motors.motors[0].clone();
        let t0 = Instant::now();
        let mut first = MotorState {
            position_rad: 0.2,
            velocity_rad_s: 0.0,
            torque_nm: 0.0,
            temperature_c: 25.0,
            fault: 0,
            updated: Some(t0),
        };
        sup.check_feedback_velocity(&motor, &mut first, t0)
            .expect("first sample");

        let mut noisy_but_stationary = MotorState {
            position_rad: 0.2,
            velocity_rad_s: 0.4,
            torque_nm: 0.0,
            temperature_c: 25.0,
            fault: 0,
            updated: Some(t0 + Duration::from_millis(20)),
        };
        sup.check_feedback_velocity(
            &motor,
            &mut noisy_but_stationary,
            t0 + Duration::from_millis(20),
        )
        .expect("stationary sample");

        assert_eq!(noisy_but_stationary.velocity_rad_s, 0.0);
    }

    #[test]
    fn send_mit_converts_joint_command_to_motor_space() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let mut motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        motor.direction = -1;
        motor.gear_ratio = 2.0;
        let configured = sup
            .motors
            .motors
            .iter_mut()
            .find(|configured| configured.joint == motor.joint)
            .expect("configured elbow");
        configured.direction = motor.direction;
        configured.gear_ratio = motor.gear_ratio;
        sup = installed_policy(&sup.motors, &sup.control);
        bench_ready_active(&mut sup);
        sup.bus.clear_trace();
        sup.send_mit_joint(
            MitJointCommand {
                joint: "right_elbow_pitch".to_string(),
                kp: 8.0,
                kd: 4.0,
                position_rad: 0.5,
                velocity_rad_s: 0.25,
                // Keep this coordinate-conversion request below the initial
                // slew bound; torque lifecycle has separate boundary tests.
                torque_ff_nm: 0.4,
            },
            &motor,
        )
        .expect("send");
        // Independent RS02 vendor wire oracle: direction -1, gearing 2 gives
        // q=-1, dq=-.5, kp=2, kd=1, tau=-.2. Signed ranges are ±4π/44/17;
        // unsigned gain ranges are 500/5. No production encoder as oracle.
        assert_eq!(sup.bus.frames().len(), 1);
        assert_eq!(sup.bus.frames()[0].id, 0x017e_7e04);
        assert_eq!(
            sup.bus.frames()[0].data,
            [0x75, 0xcf, 0x7e, 0x8b, 0x01, 0x06, 0x33, 0x33]
        );
    }

    #[test]
    fn watchdog_fires_when_active_without_first_feedback() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.comm_watchdog_ms = 1;
        bench_verify_all_joints(&mut sup);
        sup.set_homing_complete().expect("ready");
        let joints = sup.robot.robot.joints.clone();
        sup.enable_targets(&joints).expect("enable");
        std::thread::sleep(Duration::from_millis(2));
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        let err = sup
            .send_mit_joint(
                MitJointCommand {
                    joint: "right_elbow_pitch".to_string(),
                    kp: 0.0,
                    kd: 0.0,
                    position_rad: 0.0,
                    velocity_rad_s: 0.0,
                    torque_ff_nm: 0.0,
                },
                &motor,
            )
            .expect_err("watchdog");
        assert!(matches!(err, DavoutError::CommWatchdog { ms: 1, .. }));
    }

    fn type24_tx_frames(tx: &[CanFrame]) -> Vec<&CanFrame> {
        tx.iter()
            .filter(|f| {
                robstride::unpack_ext_id(f.id)
                    .map(|u| u.comm_type == robstride::CommunicationType::ActiveReporting.as_u8())
                    .unwrap_or(false)
            })
            .collect()
    }

    /// `sync` writes one type-24 frame per interface per control period;
    /// call it once per period until every joint's change is written.
    fn settle_active_reporting<B: MotorBus>(sup: &mut Supervisor<B>) {
        for _ in 0..=sup.motors.motors.len() {
            sup.sync_active_reporting();
            std::thread::sleep(active_reporting::ACTIVE_REPORTING_WRITE_SPACING);
        }
    }

    #[test]
    fn active_reporting_default_false_sends_no_type24() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Repo bench yaml may enable the flag; force on then off to emit disables.
        sup.control.control.bench.active_reporting_diagnostics = true;
        settle_active_reporting(&mut sup);
        sup.bus.clear_trace();
        sup.control.control.bench.active_reporting_diagnostics = false;
        settle_active_reporting(&mut sup);
        let off = type24_tx_frames(sup.bus.frames());
        assert_eq!(off.len(), sup.motors.motors.len());
        for frame in off {
            assert_eq!(frame.data[6], 0x00, "disable F_CMD when flag false");
        }
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| !sup.active_reporting.applied_on(&m.joint)));
    }

    #[test]
    fn active_reporting_sends_type24_when_non_active_and_flag_true() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Normalize to off, then enable.
        sup.control.control.bench.active_reporting_diagnostics = false;
        settle_active_reporting(&mut sup);
        for m in &sup.motors.motors {
            assert!(!sup.active_reporting.applied_on(&m.joint));
        }
        sup.bus.clear_trace();
        sup.control.control.bench.active_reporting_diagnostics = true;
        settle_active_reporting(&mut sup);
        let type24 = type24_tx_frames(sup.bus.frames());
        assert_eq!(type24.len(), sup.motors.motors.len());
        for frame in type24 {
            assert_eq!(frame.data[6], 0x01, "enable F_CMD");
        }
        let tx_before = sup.bus.frames().len();
        bench_ready_active(&mut sup);
        // Activation writes the first Off; the rest follow one per period.
        settle_active_reporting(&mut sup);
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| !sup.active_reporting.applied_on(&m.joint)));
        let new_frames = &sup.bus.frames()[tx_before..];
        for frame in type24_tx_frames(new_frames) {
            assert_eq!(
                frame.data[6], 0x00,
                "disable before Active (MIT owns status)"
            );
        }
    }

    #[test]
    fn active_reporting_zero_tx_during_active_mit_batch() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.bench.active_reporting_diagnostics = true;
        settle_active_reporting(&mut sup);
        bench_ready_active(&mut sup);
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        let tx_before = sup.bus.frames().len();
        sup.send_mit_joint(
            MitJointCommand {
                joint: "right_elbow_pitch".to_string(),
                kp: 0.0,
                kd: 0.0,
                position_rad: 0.5,
                velocity_rad_s: 0.0,
                torque_ff_nm: 1.0,
            },
            &motor,
        )
        .expect("send");
        assert!(!sup.bus.frames().is_empty());
        let new_frames = &sup.bus.frames()[tx_before..];
        assert!(
            type24_tx_frames(new_frames).is_empty(),
            "MIT batch must not include type-24 frames"
        );
    }

    #[test]
    fn active_reporting_stays_armed_through_ready_for_free_drive() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Normalize off then on so enable TX is observed.
        sup.control.control.bench.active_reporting_diagnostics = false;
        settle_active_reporting(&mut sup);
        for m in &sup.motors.motors {
            assert!(!sup.active_reporting.applied_on(&m.joint));
        }
        sup.bus.clear_trace();
        sup.control.control.bench.active_reporting_diagnostics = true;
        settle_active_reporting(&mut sup);
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| sup.active_reporting.applied_on(&m.joint)));
        assert_eq!(
            type24_tx_frames(sup.bus.frames()).len(),
            sup.motors.motors.len()
        );
        bench_verify_all_joints(&mut sup);
        let tx_before = sup.bus.frames().len();
        sup.set_homing_complete().expect("ready");
        assert_eq!(sup.mode(), OperationalMode::Ready);
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| sup.active_reporting.applied_on(&m.joint)));
        let off_frames = type24_tx_frames(&sup.bus.frames()[tx_before..]);
        assert!(
            off_frames.is_empty(),
            "Ready must keep type-24 for Set Limits free-drive sensing"
        );
    }

    #[test]
    fn no_comm_type_22_in_robstride_surface() {
        assert!(robstride::CommunicationType::from_u8(22).is_none());
    }

    fn neutral_elbow() -> MitJointCommand {
        MitJointCommand {
            joint: "right_elbow_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: 0.0,
        }
    }

    #[test]
    fn comm_watchdog_fires_on_silence() {
        // The host keeps sending and the drive never answers: the watchdog
        // fires comm_watchdog_ms after the first unanswered command.
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.comm_watchdog_ms = 50;
        sup.control.control.feedback_drain_quiet_us = 300;
        let window = Duration::from_millis(50);
        bench_ready_active(&mut sup);
        std::thread::sleep(Duration::from_millis(10));
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        let first = Instant::now();
        sup.send_mit_joint(neutral_elbow(), &motor)
            .expect("watchdog should not fire at 10 ms silence");
        let err = loop {
            std::thread::sleep(Duration::from_millis(5));
            match sup.send_mit_joint(neutral_elbow(), &motor) {
                Ok(()) => assert!(
                    first.elapsed() <= window + Duration::from_millis(5),
                    "admitted {:?} after the first unanswered command",
                    first.elapsed()
                ),
                Err(err) => break err,
            }
        };
        assert!(
            matches!(err, DavoutError::CommWatchdog { ms: 50, .. }),
            "{err}"
        );
        assert!(first.elapsed() > window);
    }

    #[test]
    fn comm_watchdog_does_not_count_the_hosts_own_silence() {
        // Nothing was asked of the drive while the host sent nothing (ADR
        // 0036: host-caused silence is not counted). Once asked, it must
        // answer within comm_watchdog_ms.
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.comm_watchdog_ms = 50;
        sup.control.control.feedback_drain_quiet_us = 300;
        bench_ready_active(&mut sup);
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        std::thread::sleep(Duration::from_millis(80));
        sup.send_mit_joint(neutral_elbow(), &motor)
            .expect("the host's own 80 ms silence is not the drive's");
        std::thread::sleep(Duration::from_millis(60));
        let err = sup
            .send_mit_joint(neutral_elbow(), &motor)
            .expect_err("the command 60 ms ago is unanswered");
        assert!(
            matches!(err, DavoutError::CommWatchdog { ms: 50, .. }),
            "{err}"
        );
    }

    #[test]
    fn drive_reboot_detected_after_silence_then_reset() {
        // 2026-10-04 gravity-calibration shape: a drive running in Motor
        // mode goes silent past the comm watchdog, then replies in Reset
        // (firmware boots into Reset after a power interruption).
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.comm_watchdog_ms = 50;
        sup.control.control.feedback_drain_quiet_us = 300;
        bench_ready_active(&mut sup);
        assert!(sup.last_drive_reboot().is_none());
        std::thread::sleep(Duration::from_millis(60));
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some(motor.can_interface.clone()),
                status_frame_motor_space_mode(
                    motor.device_id,
                    motor.motor_type,
                    0.0,
                    0.0,
                    0.0,
                    25.0,
                    0,
                ),
            ))
            .expect("finite raw fixture");
        sup.drain_feedback()
            .expect_err("Reset while Active still latches DriveState");
        let reboot = sup
            .last_drive_reboot()
            .expect("Run, then silence, then Reset is a reboot");
        assert_eq!(reboot.joint, "right_elbow_pitch");
        assert_eq!(reboot.address, MotorAddress::from(&motor));
        assert_eq!(reboot.previous_mode, DriveMode::Run);
        assert_eq!(reboot.new_mode, DriveMode::Reset);
        assert!(
            reboot.silence >= Duration::from_millis(50),
            "silence covers the quiet window: {:?}",
            reboot.silence
        );
    }

    #[test]
    fn drive_reboot_ignored_while_run_not_expected() {
        // A Reset frame on a Disabled supervisor is ordinary (stopped
        // drives report Reset), not a reboot: run is not expected there.
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        receive_pose(&mut sup, "right_elbow_pitch", 0.0, 0.0);
        sup.bus
            .queue_received(ReceivedCanFrame::full_data(
                Some(motor.can_interface.clone()),
                status_frame_motor_space_mode(
                    motor.device_id,
                    motor.motor_type,
                    0.0,
                    0.0,
                    0.0,
                    25.0,
                    0,
                ),
            ))
            .expect("finite raw fixture");
        sup.drain_feedback()
            .expect("Reset while Disabled is ordinary");
        assert!(sup.last_drive_reboot().is_none());
    }

    #[test]
    fn peers_replying_during_silence_counts_same_interface() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let base = Instant::now();
        let elbow =
            MotorAddress::from(motor_for_joint(&sup.motors, "right_elbow_pitch").expect("motor"));
        let pitch = MotorAddress::from(
            motor_for_joint(&sup.motors, "right_shoulder_pitch").expect("motor"),
        );
        sup.drive_frames.insert(
            elbow.clone(),
            DriveFrameTrack {
                mode: DriveMode::Run,
                at: base,
            },
        );
        // Same-interface peer replied after the silent drive's last frame.
        sup.drive_frames.insert(
            pitch.clone(),
            DriveFrameTrack {
                mode: DriveMode::Run,
                at: base + Duration::from_millis(10),
            },
        );
        // Another interface never counts, even when fresh.
        sup.drive_frames.insert(
            MotorAddress::new("can9", 7),
            DriveFrameTrack {
                mode: DriveMode::Run,
                at: base + Duration::from_millis(10),
            },
        );
        let expected = usize::from(pitch.interface == elbow.interface);
        assert_eq!(sup.peers_replying_during_silence(&elbow), expected);
    }

    #[test]
    fn mit_filter_clamps_position_into_the_live_envelope() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        receive_pose(&mut sup, "right_elbow_pitch", 0.5, 0.0);
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        let out = sup
            .filter_mit_command(
                MitJointCommand {
                    joint: motor.joint.clone(),
                    kp: 10.0,
                    kd: 0.0,
                    position_rad: 99.0,
                    velocity_rad_s: 0.0,
                    torque_ff_nm: 0.0,
                },
                &motor,
            )
            .expect("valid MIT command clamps");
        let policy = sup.joint_limit_policy(&motor.joint).expect("policy");
        assert!(out.position_rad <= policy.hard_upper());
        assert!(out.position_rad < 99.0);
    }

    #[test]
    fn mit_gain_caps_are_checked_after_joint_to_motor_gear_scaling() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        let motor = sup
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == "right_elbow_pitch")
            .expect("elbow motor");
        motor.gear_ratio = 2.0;
        let motor = motor.clone();
        let out = sup
            .filter_mit_command(
                MitJointCommand {
                    joint: motor.joint.clone(),
                    kp: 1500.0,
                    kd: 15.0,
                    position_rad: 0.0,
                    velocity_rad_s: 0.0,
                    torque_ff_nm: 0.0,
                },
                &motor,
            )
            .expect("wire gains are under type caps");
        assert_eq!(out.kp, 1500.0);
        assert_eq!(out.kd, 15.0);
    }

    #[test]
    fn public_mit_filter_refuses_mutated_invalid_policy_before_clamping() {
        let mut sup = Supervisor::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("supervisor");
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        sup.control.control.tau_ff_rate_limit_nm_per_s = f64::NAN;
        let result = sup.filter_mit_command(
            MitJointCommand {
                joint: motor.joint.clone(),
                kp: 10.0,
                kd: 0.0,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_ff_nm: 0.0,
            },
            &motor,
        );
        assert!(matches!(result, Err(DavoutError::Config(_))));
    }

    fn wrong_sign_sup_at(velocity: f32) -> Supervisor<SimulationBus> {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Master control.yaml disables the watchdog during elbow commissioning;
        // force-enable for unit coverage of the trip path.
        sup.control.control.wrong_sign_watchdog.enabled = true;
        bench_active(&mut sup);
        initial_poses(&mut sup, "right_shoulder_pitch", 1.0, velocity);
        sup.set_control_mode(ControlMode::GravityComp);
        sup
    }

    fn wrong_sign_cmd() -> MitJointCommand {
        MitJointCommand {
            joint: "right_shoulder_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 1.0,
            velocity_rad_s: 0.0,
            // q > 0 → expected_sign = -1; +1.0 opposes it.
            torque_ff_nm: 1.0,
        }
    }

    #[test]
    fn wrong_sign_watchdog_trips_on_sustained_opposition() {
        let mut sup = wrong_sign_sup_at(0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let cmd = wrong_sign_cmd();
        let grace = sup.control.control.wrong_sign_watchdog.grace_period_ticks;
        let min_opp = sup.control.control.wrong_sign_watchdog.min_opposition_ticks;
        for _ in 0..grace + min_opp - 1 {
            sup.filter_mit_command(cmd.clone(), &motor)
                .expect("no trip before threshold");
        }
        let err = sup
            .filter_mit_command(cmd, &motor)
            .expect_err("should trip on sustained opposition");
        assert!(matches!(
            err,
            DavoutError::WrongSignWatchdog {
                ticks, ..
            } if ticks >= min_opp
        ));
    }

    #[test]
    fn wrong_sign_watchdog_no_trip_in_impedance() {
        let mut sup = wrong_sign_sup_at(0.5);
        sup.set_control_mode(ControlMode::Impedance);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let cmd = wrong_sign_cmd();
        let grace = sup.control.control.wrong_sign_watchdog.grace_period_ticks;
        let min_opp = sup.control.control.wrong_sign_watchdog.min_opposition_ticks;
        for _ in 0..grace + min_opp + 5 {
            sup.filter_mit_command(cmd.clone(), &motor)
                .expect("Impedance mode must not trip wrong-sign watchdog");
        }
    }

    #[test]
    fn wrong_sign_watchdog_grace_period() {
        let mut sup = wrong_sign_sup_at(0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let cmd = wrong_sign_cmd();
        let grace = sup.control.control.wrong_sign_watchdog.grace_period_ticks;
        // Opposition during the entire grace window must not trip.
        for _ in 0..grace {
            sup.filter_mit_command(cmd.clone(), &motor)
                .expect("grace period must not trip");
        }
        // State should show opposition has not accumulated past grace.
        let state = sup
            .wrong_sign_state
            .get("right_shoulder_pitch")
            .expect("state populated");
        assert_eq!(state.opposition_ticks, 0);
    }

    #[test]
    fn wrong_sign_watchdog_resets_on_enable() {
        let mut sup = wrong_sign_sup_at(0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let cmd = wrong_sign_cmd();
        let grace = sup.control.control.wrong_sign_watchdog.grace_period_ticks;
        // Accumulate a few opposition ticks past grace.
        for _ in 0..grace + 3 {
            let _ = sup.filter_mit_command(cmd.clone(), &motor);
        }
        assert!(sup.wrong_sign_state.contains_key("right_shoulder_pitch"));
        sup.disable_all().expect("disable");
        assert!(sup.wrong_sign_state.is_empty());
        // Re-enable clears state on the enable path too.
        sup.set_homing_complete().expect("ready");
        let joints = sup.robot.robot.joints.clone();
        sup.enable_targets(&joints).expect("enable");
        assert!(sup.wrong_sign_state.is_empty());
    }

    #[test]
    fn wrong_sign_watchdog_no_trip_when_velocity_below_threshold() {
        let mut sup = wrong_sign_sup_at(0.01);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let cmd = wrong_sign_cmd();
        let grace = sup.control.control.wrong_sign_watchdog.grace_period_ticks;
        let min_opp = sup.control.control.wrong_sign_watchdog.min_opposition_ticks;
        for _ in 0..grace + min_opp + 5 {
            sup.filter_mit_command(cmd.clone(), &motor)
                .expect("near-zero velocity must not trip");
        }
    }

    #[test]
    fn wrong_sign_watchdog_no_trip_when_sign_matches() {
        let mut sup = wrong_sign_sup_at(0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        // q > 0 → expected_sign = -1; torque_ff = -1.0 matches → no trip.
        let cmd = MitJointCommand {
            joint: "right_shoulder_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 1.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: -1.0,
        };
        let grace = sup.control.control.wrong_sign_watchdog.grace_period_ticks;
        let min_opp = sup.control.control.wrong_sign_watchdog.min_opposition_ticks;
        for _ in 0..grace + min_opp + 5 {
            sup.filter_mit_command(cmd.clone(), &motor)
                .expect("matching sign must not trip");
        }
    }

    #[test]
    fn disable_all_clears_last_tick() {
        let mut sup = wrong_sign_sup_at(0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        let cmd = MitJointCommand {
            joint: "right_shoulder_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 1.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: -1.0, // matching sign → no watchdog trip
        };
        let _ = sup.filter_mit_command(cmd, &motor);
        assert!(sup.last_tick.is_some(), "last_tick set after a tick");
        sup.disable_all().expect("disable");
        assert!(
            sup.last_tick.is_none(),
            "disable_all must clear last_tick to prevent stale dt on re-enable"
        );
    }

    #[test]
    fn tau_ff_step_rate_limited_after_disable_gap() {
        let mut sup = wrong_sign_sup_at(0.5);
        let motor = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("motor")
            .clone();
        sup.seed_tau_ff_rate_limiter();
        // Tick once at -0.5 Nm to establish a rate-limiter baseline.
        let cmd_small = MitJointCommand {
            joint: "right_shoulder_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 1.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: -0.5,
        };
        let out1 = sup.filter_mit_command(cmd_small, &motor).expect("tick 1");
        // Disable — simulates a safety trip / comm-watchdog gap.
        sup.disable_all().expect("disable");
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Re-enable (mirrors the recovery path the Pi would take).
        sup.set_homing_complete().expect("ready");
        let joints = sup.robot.robot.joints.clone();
        sup.enable_targets(&joints).expect("enable");
        // Large τ_ff step — must be rate-limited, NOT passed through.
        let cmd_big = MitJointCommand {
            joint: "right_shoulder_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 1.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: -5.0,
        };
        let out2 = sup
            .filter_mit_command(cmd_big, &motor)
            .expect("tick after re-enable");
        let rate = sup.control.control.tau_ff_rate_limit_nm_per_s;
        let max_step = rate * 0.01; // default dt fallback (0.01 s)
        let actual_step = (out2.torque_ff_nm - out1.torque_ff_nm).abs();
        assert!(
            actual_step <= max_step + 1e-6,
            "τ_ff step {actual_step} must be <= rate-limited max_step {max_step} after disable gap \
             (rate={rate} Nm/s, out1={}, out2={})",
            out1.torque_ff_nm,
            out2.torque_ff_nm,
        );
    }
}
