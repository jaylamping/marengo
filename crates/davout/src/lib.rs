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
//!   `kp`/`kd`/`tau_ff` caps, [`tau_ff` rate limiting](Supervisor::filter_mit_command).
//! - Own joint↔motor coordinate conversion from `config/motors.yaml` (`direction`, `gear_ratio`):
//!   Berthier and dynamics stay in joint space; robstride stays in raw motor/CAN space.
//! - [`danger_zones`](marengo_config::DangerZoneRule) from `config/control.yaml` (clamp or fault on rule hit).
//! - Comm watchdog: stale feedback → [`DavoutError::CommWatchdog`].
//! - Persistent [`SafetySnapshot`]: device/runtime faults survive pose replacement, disable and replay.
//! - Private current-reference authority: history, cached pose and public scalar verification
//!   cannot authorize Ready, scoped Enable or motion. Physical acquisition is unsupported.
//! - Closed [`simulation::SimulationBus`] INITIAL virtual fixtures share admission/output logic;
//!   they do not qualify reference acquisition, SetZero correlation or persistence ordering.
//! - [`Supervisor::begin_reference`] / [`Supervisor::advance_reference`]: a separately
//!   qualified closed virtual transaction stages correlated evidence and owns bounded
//!   cleanup. It cannot grant motion; physical acquisition remains unavailable.
//! - Retained matched evidence and [`ReferenceStageStatus`]: live inspection of
//!   owner/device/model/policy/stop continuity after cleanup, distinct from an
//!   immutable unusable acquisition terminal. No stage diagnostic grants output.
//! - Explicit unreferenced virtual journal owners commit immutable typed history on a
//!   dedicated bounded SQLite worker. Owner-consumed completion remains distinct from
//!   eligibility, cancellation and output permission; recovery is inspection only.
//! - Explicit current-consuming virtual owners may select the acquired joint after real
//!   durable completion and fresh continuity checks. Its private lifetime binds actual
//!   model/device continuity independently of transaction deadlines and diagnostic caches.
//! - [`Supervisor::disable_all`]: all-address best-effort stop with honest delivery evidence.
//! - [`Supervisor::inspect_drive_protocol`]: blocking standalone disabled-drive queries;
//!   shared hazard consumption and final stop, without reference or enable permission.
//! - [`refresh_feedback`]: blocking poll up to `feedback_poll_budget_us` (REPL / set-zero).
//! - [`drain_feedback`]: non-blocking RX queue drain (Berthier control loop).
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
//!   robstride::send_mit / mit_control_all
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
mod bench_home_qualification;
pub use bench_home_qualification::{BenchHomeQualification, BENCH_TIMEOUT_COUNTS};
#[cfg(test)]
mod active_reporting_pacing_tests;
mod faults;
mod feedback_consumer;
mod limit_envelope;
mod protocol_inspection;
pub use protocol_inspection::{MotorProtocolInspection, ProtocolReadReceipt};
mod reference;
mod reference_codec;
mod reference_commit;
mod reference_journal;
mod reference_journal_event;
#[cfg(test)]
mod reference_journal_tests;
mod reference_model;
mod reference_transaction;
mod reference_urdf_codec;
pub mod simulation;

pub use reference_commit::{
    ReferenceAudit, ReferenceCommitError, ReferenceCommitHandle, ReferenceCommitPhase,
    ReferenceCommitSnapshot,
};
pub use reference_journal::{ReferenceJournalDrain, ReferenceJournalError, ReferenceJournalResult};
pub use reference_journal_event::ReferenceHistoryRecord;

pub use reference_transaction::{
    ReferenceCancelReason, ReferenceCause, ReferenceCommit, ReferenceError, ReferenceFailureKind,
    ReferenceHandle, ReferencePhase, ReferenceReceiveSummary, ReferenceReportingAttempt,
    ReferenceRequest, ReferenceSnapshot, ReferenceStageInvalidation, ReferenceStageStatus,
    ReferenceStamp, ReferenceTerminal,
};

pub use active_reporting::{ActiveReportingLeaseError, ActiveReportingState, DEFAULT_LEASE_TTL};
use faults::{bounded_message, FaultAuthority};
pub use faults::{
    DeviceFaultEvidence, DeviceWarning, FaultClass, FaultRecord, ReceiveDrainEvidence,
    ReceiveFaultEvidence, ReceiveFrameEvidence, SafetySnapshot, StopAction, StopAttempt,
    StopReport,
};

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Free-drive / Hardware-page sensing TTL for `RobotState` presence.
///
/// While not [`OperationalMode::Active`], `joint_feedback` omits samples older than this so
/// Consul Online/Offline tracks recent RX (status poll or type-24), not sticky cache membership.
/// Sized at ~2× Consul `MOTOR_STATUS_POLL_MS` (2.5 s) so one missed poll does not flap Offline.
pub const FREE_DRIVE_FEEDBACK_TTL: Duration = Duration::from_secs(5);

use armee_kinematics::{
    clamp_position_in_envelope, joint_limit_bounds, joint_limits, load_urdf, LimitMarginConfig,
};
use marengo_config::{
    default_commissioning_scope_path, effective_commissioning_scope, joint_subset_from_env,
    load_commissioning_scope, load_control_config_from, load_homing_config_from,
    load_motors_config_from, load_robot_config, load_robot_config_from, motor_for_joint,
    motor_type_key, resolve_config_dir, resolve_joint_velocity_cap, resolve_urdf_path,
    validate_control_against_limits, validate_motors_against_robot,
    validate_robot_control_joint_coverage, validate_safety_config, ControlConfigFile,
    HomingConfigFile, MotorEntry, MotorType, MotorsConfigFile, RobotConfigFile,
};
use marengo_homing::{select_enable_targets, HomingRegistry, JointFacetInput};
use reference::ReferenceAuthority;
use robstride::AddressedMitCommand;
use robstride::{MitCommand, MotorState, ParameterId, ParameterValue, RunMode};
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

/// Command from control stack before safety filtering.
#[derive(Debug, Clone, PartialEq)]
pub struct JointCommand {
    pub joint: String,
    pub position_rad: f64,
    pub velocity_rad_s: f64,
    pub torque_nm: f64,
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

/// Firmware speed-mode command for bench diagnostics only.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedCommand {
    pub joint: String,
    pub velocity_rad_s: f64,
}

#[derive(Debug, Error)]
pub enum DavoutError {
    #[error("drive protocol inspection on {joint}: {message}")]
    ProtocolInspection { joint: String, message: String },
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
    #[error("firmware speed mode is disabled in control.bench.allow_firmware_speed_mode")]
    FirmwareSpeedModeDisabled,
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
}

pub use marengo_homing::JointHomingState;
pub use robstride::bus::{BusError, MemoryBus, MotorAddress, MotorBus};

fn map_lease_error(err: ActiveReportingLeaseError) -> DavoutError {
    match err {
        ActiveReportingLeaseError::UnknownJoint { joint } => DavoutError::UnknownJoint { joint },
        ActiveReportingLeaseError::InvalidLeaseId
        | ActiveReportingLeaseError::InvalidClientId
        | ActiveReportingLeaseError::TooManyLeases { .. }
        | ActiveReportingLeaseError::MissingLease { .. } => DavoutError::Homing {
            message: format!("active reporting lease: {err:?}"),
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

/// Safety supervisor — the only gateway to the motor bus.
pub struct Supervisor<B: MotorBus> {
    mode: OperationalMode,
    control_mode: ControlMode,
    hardware_estop: bool,
    fault_authority: FaultAuthority,
    limits: HashMap<String, JointLimitPolicy>,
    robot: RobotConfigFile,
    urdf_robot: urdf_rs::Robot,
    installed_model: reference_model::InstalledReferenceModel,
    pub motors: MotorsConfigFile,
    pub control: ControlConfigFile,
    pub homing_config: HomingConfigFile,
    homing: HomingRegistry,
    reference_authority: ReferenceAuthority,
    reference_owner: reference_transaction::ReferenceOwner<B>,
    reference_commits: reference_commit::CommitOwner,
    reference_realm_matches: Option<fn(&B, &Arc<()>) -> bool>,
    /// Installed routes remain stop targets even if a caller corrupts public policy.
    stop_motors: Vec<MotorEntry>,
    motor_types: HashMap<MotorAddress, MotorType>,
    bus: B,
    motor_states: HashMap<MotorAddress, MotorState>,
    active_since: Option<Instant>,
    invalid_feedback: HashSet<MotorAddress>,
    last_tau_ff: HashMap<String, f64>,
    feedback_velocity_trips: HashMap<String, u8>,
    last_feedback_samples: HashMap<String, FeedbackSample>,
    wrong_sign_state: HashMap<String, WrongSignState>,
    last_tick: Option<Instant>,
    active_reporting: ActiveReportingState,
    /// Last time each joint produced a feedback frame (for type-24 silence retry).
    last_feedback_rx: HashMap<String, Instant>,
    /// Status frames decoded in the most recent [`Self::refresh_feedback`] poll.
    last_refresh_frames: usize,
    /// Joints whose drives were successfully enabled for the current Active session.
    active_joints: HashSet<String>,
}

impl<B: MotorBus> Supervisor<B> {
    /// Build supervisor from repo `config/` and URDF limits.
    ///
    /// `MARENGO_CALIBRATION_RECORD` overrides the configured history path, with
    /// relative overrides interpreted from the process working directory.
    pub fn from_repo(repo_root: impl AsRef<Path>, bus: B) -> Result<Self, DavoutError> {
        let record_path = std::env::var_os("MARENGO_CALIBRATION_RECORD").map(PathBuf::from);
        Self::from_repo_inner(repo_root.as_ref(), bus, record_path)
    }

    /// Build with an explicit calibration history path, ignoring the environment override.
    ///
    /// The path is used as supplied. History never grants current reference;
    /// resource errors return before startup diagnostic traffic is transmitted.
    pub fn from_repo_with_calibration_record_path(
        repo_root: impl AsRef<Path>,
        bus: B,
        record_path: impl AsRef<Path>,
    ) -> Result<Self, DavoutError> {
        Self::from_repo_inner(
            repo_root.as_ref(),
            bus,
            Some(record_path.as_ref().to_owned()),
        )
    }

    fn from_repo_inner(
        root: &Path,
        bus: B,
        record_path: Option<PathBuf>,
    ) -> Result<Self, DavoutError> {
        Self::from_config_dir_inner(root, &resolve_config_dir(root), bus, record_path)
    }

    fn from_config_dir_inner(
        root: &Path,
        config_dir: &Path,
        bus: B,
        record_path: Option<PathBuf>,
    ) -> Result<Self, DavoutError> {
        Self::from_config_dir_with_reporting(root, config_dir, bus, record_path, true)
    }

    /// Standalone diagnostic owner: validate normal installed policy/history,
    /// but transmit no automatic reporting traffic during construction.
    pub fn from_repo_for_protocol_inspection(
        root: impl AsRef<Path>,
        bus: B,
    ) -> Result<Self, DavoutError> {
        let root = root.as_ref();
        let record_path = std::env::var_os("MARENGO_CALIBRATION_RECORD").map(PathBuf::from);
        Self::from_config_dir_with_reporting(
            root,
            &resolve_config_dir(root),
            bus,
            record_path,
            false,
        )
    }

    fn from_config_dir_with_reporting(
        root: &Path,
        config_dir: &Path,
        bus: B,
        record_path: Option<PathBuf>,
        reporting: bool,
    ) -> Result<Self, DavoutError> {
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
        validate_motors_against_robot(&robot, &motors)?;
        validate_robot_control_joint_coverage(&robot, &control)?;
        let homing_joints: Vec<String> = robot.robot.joints.clone();
        let record_path =
            record_path.unwrap_or_else(|| root.join(&homing_config.homing.calibration_record_path));
        let homing = HomingRegistry::with_record_path(
            record_path,
            homing_joints,
            homing_config.homing.zero_verify_tolerance_rad,
        )
        .map_err(|e| DavoutError::Homing {
            message: e.to_string(),
        })?;
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
        let mut supervisor = Self {
            mode: OperationalMode::Disabled,
            control_mode: ControlMode::Disabled,
            hardware_estop: false,
            fault_authority: FaultAuthority::default(),
            limits,
            robot,
            urdf_robot,
            installed_model,
            stop_motors: motors.motors.clone(),
            motors,
            control,
            homing_config,
            homing,
            reference_authority: ReferenceAuthority::default(),
            reference_owner: reference_transaction::ReferenceOwner::default(),
            reference_commits: reference_commit::CommitOwner::default(),
            reference_realm_matches: None,
            motor_types,
            bus,
            motor_states: HashMap::new(),
            active_since: None,
            invalid_feedback: HashSet::new(),
            last_tau_ff: HashMap::new(),
            feedback_velocity_trips: HashMap::new(),
            last_feedback_samples: HashMap::new(),
            wrong_sign_state: HashMap::new(),
            last_tick: None,
            active_reporting: ActiveReportingState::default(),
            last_feedback_rx: HashMap::new(),
            last_refresh_frames: 0,
            active_joints: HashSet::new(),
        };
        // Arm type-24 when configured so free-drive Set Limits can see motion while
        // limp (Disabled/Ready). MIT Active still turns reporting off in sync below.
        if reporting {
            supervisor.sync_active_reporting();
        }
        Ok(supervisor)
    }

    /// Frames received in the last [`Self::refresh_feedback`] call (0 if none or timeout).
    pub fn last_refresh_frame_count(&self) -> usize {
        self.last_refresh_frames
    }

    pub fn homing_registry(&self) -> &HomingRegistry {
        &self.homing
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
        self.homing.is_out_of_limits(joint)
    }

    /// Wire facets for one joint: proto homing ordinal, drive_active, out_of_limits.
    pub fn joint_commissioning_wire(&self, joint: &str) -> (i32, bool, bool) {
        let homing = marengo_homing::to_proto_homing_state(self.joint_homing_state(joint)) as i32;
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

    /// Rebuild runtime limit policies from the supervisor's in-memory configuration.
    ///
    /// The existing policies remain installed if validation or rebuilding fails.
    pub fn rebuild_limits(&mut self) -> Result<(), DavoutError> {
        self.refuse_reference_interference("rebuild_limits")?;
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::LimitPatchActive);
        }
        // A refused install must not erase an observed change to live fields.
        self.reference_binding_valid();
        validate_control_against_limits(&self.robot, &self.motors, &self.control)?;
        let limits = build_limits(&self.robot, &self.motors, &self.control, &self.urdf_robot)?;
        let installed_model = self
            .installed_model
            .replacement(&self.robot, &self.urdf_robot)?;
        self.reference_authority.revoke();
        self.installed_model = installed_model;
        self.limits = limits;
        Ok(())
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
        let motor = motor_for_joint(&self.motors, joint)?;
        let address = MotorAddress::from(motor);
        let state = self.motor_states.get(&address)?;
        if state.updated.is_none() || self.invalid_feedback.contains(&address) {
            return None;
        }
        if self.mode == OperationalMode::Active && !self.pose_is_current(state, Instant::now()) {
            return None;
        }
        if self.mode != OperationalMode::Active && state.is_stale(FREE_DRIVE_FEEDBACK_TTL) {
            return None;
        }
        Some(JointFeedback {
            position_rad: f64::from(state.position_rad),
            velocity_rad_s: f64::from(state.velocity_rad_s),
            torque_nm: f64::from(state.torque_nm),
            temperature_c: state.temperature_c,
            // Compatibility indication only. Full independent vendor domains
            // remain available in safety_snapshot, including fault-only evidence.
            fault: state.fault | u16::from(self.fault_authority.joint_is_faulted(joint)),
        })
    }

    /// Joint-space feedback position (rad) if available.
    pub fn joint_position_rad(&self, joint: &str) -> Option<f64> {
        self.joint_feedback(joint).map(|s| s.position_rad)
    }

    /// Joint-space feedback velocity (rad/s) if available.
    pub fn joint_velocity_rad(&self, joint: &str) -> Option<f64> {
        self.joint_feedback(joint).map(|s| s.velocity_rad_s)
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

    /// Cached pose cannot qualify SetZero; legacy verification refuses before persistence.
    pub fn verify_zero_after_set(
        &mut self,
        joint: &str,
        _operator: &str,
        sign_test_passed: bool,
    ) -> Result<f64, DavoutError> {
        self.reference_request_preflight(joint, Some(sign_test_passed))?;
        Err(DavoutError::ReferenceUnsupported {
            operation: "cached set-zero verification",
        })
    }

    /// Legacy calibration validates target/method/sign, then refuses unqualified
    /// acquisition before any arming, SetZero or persistence.
    ///
    /// Refuses when already [`OperationalMode::Active`] so success cannot
    /// `disable_all` out from under GravityComp / hold.
    pub fn calibrate_joint_zero(
        &mut self,
        joint: &str,
        _operator: &str,
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
        Err(DavoutError::ReferenceUnsupported {
            operation: "reference calibration",
        })
    }

    /// Current reference generation, distinct from ordinary motion-stop generation.
    pub fn reference_generation(&self) -> u64 {
        self.reference_binding_valid();
        self.reference_authority.generation()
    }

    fn reference_binding_valid(&self) -> bool {
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
        let realm_matches = match (
            self.reference_authority.realm(),
            self.reference_realm_matches,
        ) {
            (Some(realm), Some(matches)) => matches(&self.bus, realm),
            _ => false,
        };
        if !realm_matches {
            if self.reference_authority.realm().is_some() {
                self.reference_authority.revoke();
            }
            return false;
        }
        if !self
            .reference_authority
            .validate_consumed_model(&self.installed_model)
            || self
                .reference_authority
                .consumed_binding()
                .is_some_and(|binding| {
                    self.reference_owner
                        .current_device_epoch(&self.bus, &binding.address)
                        != Some(binding.device_epoch)
                })
        {
            self.reference_authority.revoke();
            return false;
        }
        self.reference_authority
            .validate_binding(&self.motors, &self.homing_config, &self.control)
    }

    fn ensure_reference_for(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        self.refuse_reference_interference("ordinary reference admission")?;
        self.require_fault_clear()?;
        if !self.reference_binding_valid() {
            if self.mode == OperationalMode::Active {
                let _ = self.disable_all();
            }
            return Err(DavoutError::Homing { message: "current reference is unavailable or permanently revoked; qualified acquisition is unsupported".into() });
        }
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
                if self.active_since.is_some_and(|enabled| {
                    enabled.elapsed()
                        <= Duration::from_millis(self.control.control.comm_watchdog_ms)
                }) {
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

    pub fn clear_motor_states(&mut self) {
        if self.reference_busy() {
            return;
        }
        self.motor_states.clear();
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

    #[tracing::instrument(skip(self))]
    pub fn request_enable(&mut self, enable: bool) -> Result<(), DavoutError> {
        if !enable {
            return self.disable_all();
        }
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        let targets = if self.mode == OperationalMode::Active {
            self.active_joints.iter().cloned().collect::<Vec<_>>()
        } else {
            self.robot.robot.joints.clone()
        };
        self.ensure_reference_for(&targets)?;
        if self.mode == OperationalMode::Active {
            return Ok(());
        }
        if self.mode != OperationalMode::Ready {
            return Err(DavoutError::NotActive { mode: self.mode });
        }
        self.enable_targets_inner(&targets)
    }

    /// Legacy calibration arming is unqualified and refuses before transmitting.
    pub fn request_enable_for_calibration(&mut self) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::Homing {
                message: "calibration enable refused while ACTIVE; disable first".into(),
            });
        }
        Err(DavoutError::ReferenceUnsupported {
            operation: "legacy calibration enable",
        })
    }

    /// Enable only the listed joints. On any enable/run-mode failure, [`disable_all`].
    ///
    /// Every requested joint needs private current-reference authority. Empty targets
    /// are rejected. While Active, a different joint set is refused
    /// ([`DavoutError::ActiveSetChangeRefused`]); operators must Disable then Enable.
    pub fn enable_targets(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode == OperationalMode::Active {
            let active: Vec<_> = self.active_joints.iter().cloned().collect();
            self.ensure_reference_for(&active)?;
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
                drive_active: self.joint_drive_active(name),
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
    pub fn resolve_enable_targets(
        &self,
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
        let (master, loaded) = self.commissioning_facets(&master_names);
        select_enable_targets(&master, &loaded, effective.as_deref())
            .map_err(|message| DavoutError::Homing { message })
    }

    fn enable_targets_inner(&mut self, joints: &[String]) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        self.ensure_reference_for(joints)?;
        // Status has no command-generation field. Drain already queued traffic
        // before activation; only later received poses may authorize this session.
        self.poll_feedback(Duration::ZERO)?;
        let result = (|| {
            for joint in joints {
                let motor = motor_for_joint(&self.motors, joint)
                    .ok_or_else(|| DavoutError::UnknownJoint {
                        joint: joint.clone(),
                    })?
                    .clone();
                let address = MotorAddress::from(&motor);
                info!(
                    joint = %motor.joint,
                    interface = %address.interface,
                    device_id = address.device_id,
                    "enabling motor"
                );
                self.bus.enable_drive_at(&address)?;
                self.bus.set_run_mode_at(&address, RunMode::Mit)?;
            }
            // Drives can queue status while enable/run-mode writes are still
            // in progress. Decode that traffic before establishing this session
            // so its later decode time cannot fabricate post-enable pose.
            // A receive failure here uses the same rollback as a write failure.
            self.poll_feedback(Duration::ZERO)?;
            self.ensure_reference_for(joints)?;
            self.active_joints = joints.iter().cloned().collect();
            self.mode = OperationalMode::Active;
            self.active_since = Some(Instant::now());
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

    /// Poll CAN feedback up to [`feedback_poll_budget_us`](marengo_config::ControlSection::feedback_poll_budget_us).
    pub fn refresh_feedback(&mut self) -> Result<usize, DavoutError> {
        self.refuse_reference_interference("ordinary feedback refresh")?;
        self.poll_feedback(self.feedback_poll_timeout())
    }

    fn poll_feedback(&mut self, budget: Duration) -> Result<usize, DavoutError> {
        // Observe policy changes before projecting raw pose data. Restoring a
        // public field after this drain must not restore a revoked reference.
        let lost_active_reference =
            self.mode == OperationalMode::Active && !self.reference_binding_valid();
        if self.mode != OperationalMode::Active {
            let _ = self.reference_binding_valid();
        }
        let quiet = self.feedback_drain_quiet();
        let report = self
            .bus
            .recv_feedback_report(&self.motor_types, budget, quiet);
        let count = report.observations.len();
        self.last_refresh_frames = self.last_refresh_frames.saturating_add(count);
        let consumption = self.consume_feedback_report(report);
        let mut first_error = consumption.first_error;
        let first_transition = consumption.first_transition;
        if first_transition {
            let _ = self.disable_all();
        }
        if lost_active_reference {
            if self.mode == OperationalMode::Active {
                let _ = self.disable_all();
            }
            if first_error.is_none() {
                first_error = Some(DavoutError::Homing {
                    message: "current reference was revoked before feedback projection".into(),
                });
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(count),
        }
    }

    fn pose_is_current(&self, state: &MotorState, now: Instant) -> bool {
        let Some(received_at) = state.updated else {
            return false;
        };
        let Some(active_since) = self.active_since else {
            return false;
        };
        received_at > active_since
            && received_at <= now
            && now.duration_since(received_at)
                <= Duration::from_millis(self.control.control.comm_watchdog_ms)
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
        for motor in &self.motors.motors {
            let address = MotorAddress::from(motor);
            let Some(state) = self.motor_states.get(&address) else {
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
        for motor in &self.motors.motors {
            if !self.active_joints.contains(&motor.joint) {
                continue;
            }
            let address = MotorAddress::from(motor);
            let state = self.motor_states.get(&address);
            if !self.invalid_feedback.contains(&address)
                && state.is_some_and(|state| self.pose_is_current(state, now))
            {
                continue;
            }
            // Bootstrap is only an inert MIT status solicit during the enable
            // deadline; never permit servo/FF motion on absent or stale pose.
            if neutral_bootstrap
                && !self.invalid_feedback.contains(&address)
                && self.active_since.is_some_and(|enabled| {
                    now.duration_since(enabled) <= Duration::from_millis(max_ms)
                })
            {
                continue;
            }
            let age_ms = state
                .and_then(|state| state.updated)
                .and_then(|received| now.checked_duration_since(received))
                .map(|age| age.as_millis().min(u128::from(u64::MAX)) as u64);
            return Err(DavoutError::CommWatchdog {
                ms: max_ms,
                joint: motor.joint.clone(),
                interface: address.interface,
                device_id: address.device_id,
                age_ms,
            });
        }
        Ok(())
    }

    /// Filter and send a joint command. **Legacy position path.**
    pub fn send_joint_command(&mut self, cmd: JointCommand) -> Result<(), DavoutError> {
        let joint = cmd.joint.clone();
        let motor = motor_for_joint(&self.motors, &joint)
            .ok_or(DavoutError::UnknownJoint {
                joint: joint.clone(),
            })?
            .clone();
        let mit = MitJointCommand {
            joint,
            kp: 0.0,
            kd: 0.0,
            position_rad: cmd.position_rad,
            velocity_rad_s: cmd.velocity_rad_s,
            torque_ff_nm: cmd.torque_nm,
        };
        self.send_mit_joint(mit, &motor)
    }

    /// Filter and send one MIT command.
    pub fn send_mit_joint(
        &mut self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
    ) -> Result<(), DavoutError> {
        self.admit_and_send_mit(vec![(cmd, motor.clone())])
    }

    /// Filter and send a batch of MIT commands (one per joint).
    pub fn send_mit_batch(&mut self, cmds: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        let mut addressed = Vec::with_capacity(cmds.len());
        for cmd in cmds {
            let joint = cmd.joint.clone();
            let motor = motor_for_joint(&self.motors, &joint)
                .ok_or(DavoutError::UnknownJoint { joint })?
                .clone();
            addressed.push((cmd, motor));
        }
        self.admit_and_send_mit(addressed)
    }

    fn admit_and_send_mit(
        &mut self,
        cmds: Vec<(MitJointCommand, MotorEntry)>,
    ) -> Result<(), DavoutError> {
        self.require_fault_clear()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        for (cmd, _) in &cmds {
            validate_mit_command(cmd)?;
        }
        let active: Vec<_> = self.active_joints.iter().cloned().collect();
        self.ensure_reference_for(&active)?;
        let mut unique = HashSet::new();
        for (cmd, motor) in &cmds {
            validate_mit_command(cmd)?;
            self.require_active_joint(&cmd.joint)?;
            let configured = motor_for_joint(&self.motors, &cmd.joint).ok_or_else(|| {
                DavoutError::UnknownJoint {
                    joint: cmd.joint.clone(),
                }
            })?;
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
            if cmd.joint != motor.joint || !unique.insert(cmd.joint.clone()) {
                return Err(DavoutError::InvalidCommand {
                    joint: cmd.joint.clone(),
                    message: "joint/address mismatch or repeated joint in batch".into(),
                });
            }
        }
        let batch_tick = Instant::now();
        let previous_tau = self.last_tau_ff.clone();
        let previous_sign = self.wrong_sign_state.clone();
        let prepared = (|| {
            let mut wires = Vec::with_capacity(cmds.len());
            let mut neutral = true;
            for (cmd, motor) in cmds {
                let filtered = self.filter_mit_command_at_tick(cmd, &motor, self.last_tick)?;
                validate_mit_command(&filtered)?;
                neutral &= filtered.kp == 0.0
                    && filtered.kd == 0.0
                    && filtered.torque_ff_nm == 0.0
                    && filtered.velocity_rad_s == 0.0;
                wires.push(AddressedMitCommand {
                    address: MotorAddress::from(&motor),
                    command: joint_to_motor_command(&motor, &filtered)?,
                });
            }
            self.check_comm_watchdog(neutral)?;
            Ok(wires)
        })();
        let wires = match prepared {
            Ok(wires) => wires,
            Err(error) => {
                self.last_tau_ff = previous_tau;
                self.wrong_sign_state = previous_sign;
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
        };
        // Admission is atomic; delivery can still fail partway through a physical
        // bus write and is reported as an error for the owner's stop path.
        if let Err(error) = self.bus.mit_control_all_at(&wires) {
            let error = DavoutError::Bus(error);
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        self.last_tick = Some(batch_tick);
        Ok(())
    }

    /// Send a firmware speed-mode command for bench diagnostics.
    ///
    /// This is not a Berthier control mode. It switches the drive to Robstride
    /// `run_mode=2`, caps the target velocity, writes `limit_spd`, then writes
    /// `spd_ref`.
    pub fn send_speed_command(&mut self, cmd: SpeedCommand) -> Result<f64, DavoutError> {
        self.require_fault_clear()?;
        validate_finite(&cmd.joint, "velocity", cmd.velocity_rad_s)?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        let active: Vec<_> = self.active_joints.iter().cloned().collect();
        self.ensure_reference_for(&active)?;
        self.require_active_joint(&cmd.joint)?;
        if !self.control.control.bench.allow_firmware_speed_mode {
            return Err(DavoutError::FirmwareSpeedModeDisabled);
        }
        let motor = motor_for_joint(&self.motors, &cmd.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: cmd.joint.clone(),
            })?
            .clone();
        let capped = self.filter_speed_command(cmd, &motor)?;
        let cap = self.speed_cap_for_joint(&capped.joint)?;
        let scale = motor_position_scale(&motor)?;
        let wire_cap = checked_wire_float(&motor.joint, "speed cap", cap * scale.abs())?;
        let wire_velocity =
            checked_wire_float(&motor.joint, "speed", capped.velocity_rad_s * scale)?;
        let address = MotorAddress::from(&motor);
        if let Err(error) = self.check_comm_watchdog(false) {
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        let result = (|| {
            self.bus.set_run_mode_at(&address, RunMode::Speed)?;
            self.bus.write_parameter_at(
                &address,
                ParameterId::LimitSpeed,
                ParameterValue::F32(wire_cap),
            )?;
            self.bus.speed_control_at(&address, wire_velocity)?;
            Ok::<(), BusError>(())
        })();
        if let Err(error) = result {
            let error = DavoutError::Bus(error);
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        Ok(capped.velocity_rad_s)
    }

    /// Best-effort speed reference zero for bench firmware speed mode.
    pub fn stop_speed_command(&mut self, joint: &str) -> Result<(), DavoutError> {
        self.refuse_reference_interference("individual speed stop")?;
        let motor = motor_for_joint(&self.motors, joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?
            .clone();
        let address = MotorAddress::from(&motor);
        if let Err(error) = self.bus.speed_control_at(&address, 0.0) {
            let error = DavoutError::Bus(error);
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        Ok(())
    }

    /// Raw firmware SetZero is unavailable without a qualified owner transaction.
    pub fn set_zero_position(&mut self, joint: &str) -> Result<(), DavoutError> {
        self.reference_request_preflight(joint, None)?;
        Err(DavoutError::ReferenceUnsupported {
            operation: "raw firmware SetZero",
        })
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
        if self.has_latched_fault() {
            self.reference_authority.revoke();
        }
        let generation = self.fault_authority.begin_stop();
        let mut report = StopReport {
            generation,
            attempts: Vec::with_capacity(self.stop_motors.len() * 3),
        };
        for motor in &self.stop_motors {
            let address = MotorAddress::from(motor);
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
        self.active_joints.clear();
        self.feedback_velocity_trips.clear();
        self.last_feedback_samples.clear();
        self.last_feedback_rx.clear();
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
        self.fault_authority.set_stop_report(report.clone());
        report
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
        for (joint, address) in targets {
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
    /// Re-asserts enable on a 1 s heartbeat and when a joint's feedback goes stale
    /// while sensing is still desired (motors can drop Active Reporting mid-sweep).
    pub fn sync_active_reporting(&mut self) {
        if self.reference_busy() {
            return;
        }
        let mode_active = self.mode == OperationalMode::Active;
        let global = self.control.control.bench.active_reporting_diagnostics;
        let now = Instant::now();
        self.active_reporting.sync(
            &mut self.bus,
            &self.motors,
            mode_active,
            global,
            now,
            &self.last_feedback_rx,
        );
    }

    /// Expire TTLs and resync type-24 (call each control-loop iteration).
    pub fn tick_active_reporting_leases(&mut self) {
        self.sync_active_reporting();
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

    /// True when type-24 is applied on for `joint` after last successful sync.
    pub fn active_reporting_applied(&self, joint: &str) -> bool {
        self.active_reporting.applied_on(joint)
    }

    fn feedback_poll_timeout(&self) -> Duration {
        Duration::from_micros(self.control.control.feedback_poll_budget_us)
    }

    fn feedback_drain_quiet(&self) -> Duration {
        Duration::from_micros(self.control.control.feedback_drain_quiet_us)
    }

    pub fn filter_mit_command(
        &mut self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
    ) -> Result<MitJointCommand, DavoutError> {
        let tick = Instant::now();
        let out = self.filter_mit_command_at_tick(cmd, motor, self.last_tick)?;
        self.last_tick = Some(tick);
        Ok(out)
    }

    fn filter_mit_command_at_tick(
        &mut self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
        previous_tick: Option<Instant>,
    ) -> Result<MitJointCommand, DavoutError> {
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
        if out.kp > defaults.kp_max || out.kd > defaults.kd_max {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: "kp/kd exceed motor type max".to_string(),
            });
        }
        let q_meas = self
            .joint_position_rad(&out.joint)
            .unwrap_or(out.position_rad);
        let dq_meas = self.joint_velocity_rad(&out.joint).unwrap_or(0.0);
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
            &mut self.last_tau_ff,
            &out.joint,
            out.torque_ff_nm,
            self.control.control.tau_ff_rate_limit_nm_per_s,
            previous_tick,
            torque_cap,
        );

        self.check_wrong_sign_watchdog(&out, q_meas, dq_meas)?;

        Ok(out)
    }

    fn check_wrong_sign_watchdog(
        &mut self,
        out: &MitJointCommand,
        q_meas: f64,
        dq_meas: f64,
    ) -> Result<(), DavoutError> {
        if self.control_mode != ControlMode::GravityComp {
            return Ok(());
        }
        let cfg = &self.control.control.wrong_sign_watchdog;
        if !cfg.enabled {
            return Ok(());
        }
        let state = self.wrong_sign_state.entry(out.joint.clone()).or_default();
        state.ticks_since_enable = state.ticks_since_enable.saturating_add(1);

        if state.ticks_since_enable <= cfg.grace_period_ticks {
            return Ok(());
        }
        if dq_meas.abs() <= cfg.min_velocity_rad_s {
            return Ok(());
        }

        let expected_sign = if q_meas >= 0.0 {
            cfg.expected_sign_at_positive_q as f64
        } else {
            -(cfg.expected_sign_at_positive_q as f64)
        };
        let actual_sign = out.torque_ff_nm.signum();

        if actual_sign == 0.0 || actual_sign == expected_sign {
            state.opposition_ticks = 0;
            return Ok(());
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
            return Err(DavoutError::WrongSignWatchdog {
                joint: out.joint.clone(),
                ticks: state.opposition_ticks,
            });
        }
        Ok(())
    }

    fn speed_cap_for_joint(&self, joint: &str) -> Result<f64, DavoutError> {
        let lim = self
            .limits
            .get(joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        Ok(lim.velocity)
    }

    pub fn filter_speed_command(
        &self,
        cmd: SpeedCommand,
        _motor: &MotorEntry,
    ) -> Result<SpeedCommand, DavoutError> {
        validate_finite(&cmd.joint, "velocity", cmd.velocity_rad_s)?;
        let cap = self.speed_cap_for_joint(&cmd.joint)?;
        let mut out = cmd;
        out.velocity_rad_s = out.velocity_rad_s.clamp(-cap, cap);
        Ok(out)
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

    /// Apply URDF + bench limits without sending (for tests and planners).
    pub fn filter_command(&self, cmd: JointCommand) -> Result<JointCommand, DavoutError> {
        validate_finite(&cmd.joint, "position", cmd.position_rad)?;
        validate_finite(&cmd.joint, "velocity", cmd.velocity_rad_s)?;
        validate_finite(&cmd.joint, "torque", cmd.torque_nm)?;
        let lim = self
            .limits
            .get(&cmd.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: cmd.joint.clone(),
            })?;
        let mut out = cmd;
        out.position_rad =
            clamp_position_in_envelope(lim, out.position_rad, out.velocity_rad_s, out.position_rad);
        if out.position_rad < lim.hard_lower() || out.position_rad > lim.hard_upper() {
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
        if out.velocity_rad_s.abs() > lim.velocity {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!("|velocity| {} > {}", out.velocity_rad_s, lim.velocity),
            });
        }
        if out.torque_nm.abs() > lim.effort {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!("|torque| {} > {}", out.torque_nm, lim.effort),
            });
        }
        Ok(out)
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

fn rate_limit_tau_ff(
    last: &mut HashMap<String, f64>,
    joint: &str,
    target: f64,
    rate_nm_s: f64,
    last_tick: Option<Instant>,
    torque_cap_nm: f64,
) -> f64 {
    // A hard cap takes priority over slew continuity if measured torque or a
    // policy change leaves the previous output outside the current envelope.
    let prev = last
        .get(joint)
        .copied()
        .unwrap_or(0.0)
        .clamp(-torque_cap_nm, torque_cap_nm);
    let target = target.clamp(-torque_cap_nm, torque_cap_nm);
    let dt = last_tick
        .map(|t| t.elapsed().as_secs_f64())
        .unwrap_or(0.01)
        // Late ticks must not bank torque-step credit while control was paused.
        .clamp(1e-4, 0.01);
    let max_step = rate_nm_s * dt;
    let delta = (target - prev).clamp(-max_step, max_step);
    let out = (prev + delta).clamp(-torque_cap_nm, torque_cap_nm);
    last.insert(joint.to_string(), out);
    out
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
) -> Result<HashMap<String, JointLimitPolicy>, DavoutError> {
    let mut map = HashMap::new();
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
        if let Some(joint_cfg) = control.control.joints.get(joint_name) {
            if let Some(lo) = joint_cfg.position_soft_lower_rad {
                bounds.soft_lower = lo.clamp(bounds.hard_lower, bounds.hard_upper);
            }
            if let Some(hi) = joint_cfg.position_soft_upper_rad {
                bounds.soft_upper = hi.clamp(bounds.hard_lower, bounds.hard_upper);
            }
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
        let joint_cfg = control.control.joints.get(joint_name);
        let margin = limit_margin_from_config(joint_cfg);
        let velocity = resolve_joint_velocity_cap(joint_name, motor.motor_type, &control.control)?;
        let effort = urdf_lim
            .effort
            .min(motor.bench.torque_limit_nm)
            .min(robot.robot.bench.max_joint_torque_nm);
        let tau_ff_max = effort
            .min(motor.bench.torque_limit_nm)
            .min(defaults.tau_ff_max_nm);
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

fn limit_margin_from_config(
    joint_cfg: Option<&marengo_config::JointControlEntry>,
) -> LimitMarginConfig {
    match joint_cfg {
        Some(c) => LimitMarginConfig {
            min_rad: c.position_limit_margin_min_rad,
            k_v_s: c.position_limit_margin_k_v_s,
            k_stop: c.position_limit_margin_k_stop,
            velocity_deadband_rad_s: c.position_trajectory_velocity_deadband_rad,
            measured_fault_slack_rad: c.position_limit_measured_fault_slack_rad,
            decel_rad_s2: c.position_trajectory_accel_rad_s2,
        },
        None => LimitMarginConfig::default(),
    }
}

#[cfg(test)]
#[path = "../../marengo-homing/tests/support/mod.rs"]
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
        sup.request_enable(true).expect("enable");
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
        Supervisor::from_simulation_with_calibration_record_path(
            fixture.path(),
            SimulationBus::default(),
            fixture.path().join("history.yaml"),
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
            marengo_homing::to_proto_homing_state(marengo_homing::JointHomingState::Unhomed) as i32
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
            marengo_homing::to_proto_homing_state(marengo_homing::JointHomingState::Verified)
                as i32
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
    fn rebuild_limits_uses_in_memory_config() {
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

        sup.rebuild_limits().expect("rebuild");

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
        assert!((urdf_after.lower - (-0.5)).abs() < 1e-9);
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
        sup.last_feedback_samples.insert(
            "right_elbow_pitch".to_string(),
            FeedbackSample {
                position_rad: 1.0,
                received_at: Instant::now(),
            },
        );

        let err = sup
            .apply_limit_patch(&elbow_limit_patch(0.1, 0.9))
            .expect_err("measured q must remain inside hard bounds");

        assert!(matches!(err, DavoutError::Limit { .. }));
    }

    #[test]
    fn rate_limit_uses_shared_previous_tick() {
        use std::collections::HashMap;
        use std::time::{Duration, Instant};

        let mut last = HashMap::new();
        last.insert("j1".to_string(), 0.0);
        last.insert("j2".to_string(), 0.0);
        let prev = Instant::now() - Duration::from_millis(10);
        let out1 = rate_limit_tau_ff(&mut last, "j1", 10.0, 100.0, Some(prev), 20.0);
        let out2 = rate_limit_tau_ff(&mut last, "j2", 10.0, 100.0, Some(prev), 20.0);
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
    fn rejects_velocity_outside_limits() {
        let bus = SimulationBus::default();
        let sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let err = sup
            .filter_command(JointCommand {
                joint: "right_shoulder_roll".to_string(),
                position_rad: 0.0,
                velocity_rad_s: 99.0,
                torque_nm: 0.0,
            })
            .expect_err("limit");
        assert!(matches!(err, DavoutError::Limit { .. }));
    }

    #[test]
    fn active_mode_required_to_send() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_verify_all_joints(&mut sup);
        sup.set_homing_complete().expect("ready");
        let err = sup
            .send_joint_command(JointCommand {
                joint: "right_shoulder_roll".to_string(),
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            })
            .expect_err("not active");
        assert!(matches!(err, DavoutError::NotActive { .. }));
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
        sup.request_enable(true).expect("enable");
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
    fn firmware_speed_mode_requires_config_flag() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        bench_ready_active(&mut sup);
        let err = sup
            .send_speed_command(SpeedCommand {
                joint: "right_elbow_pitch".to_string(),
                velocity_rad_s: 0.1,
            })
            .expect_err("speed mode disabled");
        assert!(matches!(err, DavoutError::FirmwareSpeedModeDisabled));
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
            ) | (2 << 22),
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

        assert_eq!(sup.refresh_feedback().expect("refresh"), 1);
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

        let count = sup.refresh_feedback().expect("feedback");

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
        sup.refresh_feedback().expect("first spike ignored");
        sup.bus
            .queue_received(stationary_overspeed)
            .expect("finite raw fixture");
        sup.refresh_feedback()
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
        sup.request_enable(true).expect("enable");
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

    #[test]
    fn active_reporting_default_false_sends_no_type24() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Repo bench yaml may enable the flag; force on then off to emit disables.
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        sup.bus.clear_trace();
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.sync_active_reporting();
        let off = type24_tx_frames(sup.bus.frames());
        assert_eq!(off.len(), sup.motors.motors.len());
        for frame in off {
            assert_eq!(frame.data[6], 0x00, "disable F_CMD when flag false");
        }
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| !sup.active_reporting_applied(&m.joint)));
    }

    #[test]
    fn active_reporting_sends_type24_when_non_active_and_flag_true() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        // Normalize to off, then enable.
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.sync_active_reporting();
        for m in &sup.motors.motors {
            assert!(!sup.active_reporting_applied(&m.joint));
        }
        sup.bus.clear_trace();
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        let type24 = type24_tx_frames(sup.bus.frames());
        assert_eq!(type24.len(), sup.motors.motors.len());
        for frame in type24 {
            assert_eq!(frame.data[6], 0x01, "enable F_CMD");
        }
        let tx_before = sup.bus.frames().len();
        bench_ready_active(&mut sup);
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| !sup.active_reporting_applied(&m.joint)));
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
        sup.sync_active_reporting();
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
        sup.sync_active_reporting();
        for m in &sup.motors.motors {
            assert!(!sup.active_reporting_applied(&m.joint));
        }
        sup.bus.clear_trace();
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| sup.active_reporting_applied(&m.joint)));
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
            .all(|m| sup.active_reporting_applied(&m.joint)));
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

    #[test]
    fn feedback_poll_timeout_honors_budget_not_watchdog_cap() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.comm_watchdog_ms = 50;
        sup.control.control.feedback_poll_budget_us = 3000;
        sup.control.control.feedback_drain_quiet_us = 300;
        assert_eq!(
            sup.feedback_poll_timeout(),
            Duration::from_micros(3000),
            "poll budget must not be capped at 1 ms by comm_watchdog_ms"
        );
    }

    #[test]
    fn comm_watchdog_unchanged_despite_larger_poll_budget() {
        let bus = SimulationBus::default();
        let mut sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        sup.control.control.comm_watchdog_ms = 50;
        sup.control.control.feedback_poll_budget_us = 3000;
        sup.control.control.feedback_drain_quiet_us = 300;
        bench_ready_active(&mut sup);
        std::thread::sleep(Duration::from_millis(10));
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        sup.send_mit_joint(
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
        .expect("watchdog should not fire at 10 ms silence");
        std::thread::sleep(Duration::from_millis(45));
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
        assert!(matches!(err, DavoutError::CommWatchdog { ms: 50, .. }));
    }

    #[test]
    fn filter_command_clamps_position_into_envelope() {
        let bus = SimulationBus::default();
        let sup =
            Supervisor::from_simulation(repo_root(), bus, InitialVirtualReference::AllConfigured)
                .expect("supervisor");
        let out = sup
            .filter_command(JointCommand {
                joint: "right_elbow_pitch".to_string(),
                position_rad: 99.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            })
            .expect("clamp");
        let policy = sup.joint_limit_policy("right_elbow_pitch").expect("policy");
        assert!(out.position_rad <= policy.hard_upper());
        assert!(out.position_rad < 99.0);
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
        sup.request_enable(true).expect("enable");
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
        sup.request_enable(true).expect("enable");
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
