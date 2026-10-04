//! # marengo-config — runtime YAML configuration
//!
//! Typed loaders for repo [`config/`](../../config/). Single place for **declarative**
//! robot/motor/control parameters; no realtime logic.
//!
//! ## Files
//!
//! | File | Struct | Consumers |
//! |------|--------|-----------|
//! | `robot.yaml` | [`RobotConfigFile`] | Davout, Berthier, armee-dynamics |
//! | `motors.yaml` | [`MotorsConfigFile`] | Davout, robstride (`device_id`, [`MotorType`]) |
//! | `control.yaml` | [`ControlConfigFile`] | Berthier (gains, loop Hz), Davout (caps, danger zones) |
//! | `homing.yaml` | [`HomingConfigFile`] | Homing methods, offsets, sensor inputs |
//!
//! ## Responsibilities
//!
//! - Parse and validate (e.g. motor joints ⊆ `robot.joints`).
//! - [`resolve_urdf_path`]: fail fast if URDF missing.
//!
//! ## Does not
//!
//! - Enforce limits at runtime (Davout applies caps from loaded values).
//! - Encode CAN or run control loops.
//!
//! Change joint names, motor types, or bench caps here — then update URDF and
//! `hardware/docs/kinematics.md` together.

mod atomic_file;
mod bench_joints;
mod commissioning_scope;
mod completeness;
mod config_revision;
mod limit_patch;
mod profile_txn;
mod safety_validation;
mod urdf_expand;
mod urdf_merge;

pub use atomic_file::{write_profile_file_atomic, ProfileWriteLock};
pub use bench_joints::{
    apply_joint_subset, joint_subset_from_env, load_command_joint_allowlist,
    load_command_joint_allowlist_from, resolve_command_joint, CommandJointAllowlist,
};
pub use commissioning_scope::{
    clear_commissioning_scope, default_commissioning_scope_path, effective_commissioning_scope,
    load_commissioning_scope, save_commissioning_scope, scope_widens,
    validate_commissioning_scope_joints, CommissioningScopeFile, COMMISSIONING_SCOPE_VERSION,
};
pub use completeness::{completeness_report, CompletenessReport, CompletenessWarning};
pub use config_revision::profile_content_revision;
pub use limit_patch::{
    apply_limit_patch_to_control, apply_limit_patch_to_motor, ensure_soft_inset,
    soft_limits_with_inset, validate_limit_patch, LimitPatch, DEFAULT_SOFT_INSET_RAD,
};
pub use profile_txn::{limit_patch_from_motor, write_motors_and_control};
pub use safety_validation::validate_safety_config;
pub(crate) use safety_validation::{
    validate_homing_config, validate_motors_config, validate_robot_config,
};
pub use urdf_expand::{
    apply_local_limit_patch, expand_urdf_file_to_cover_motors, write_motors_control_and_urdf,
};
pub use urdf_merge::{
    merge_preview_from_paths, simulate_merge_xml, unresolved_critical_fields, FieldDiff,
    FieldResolution, MergePreview, ResolutionChoice,
};

use std::path::{Path, PathBuf};

// Config maps are hashed on every control tick by safety validation. Keys are
// operator-authored names, so the fast non-DoS-resistant hasher is appropriate.
pub use rustc_hash::FxHashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("failed to parse {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("URDF path does not exist: {path}")]
    UrdfMissing { path: PathBuf },
    #[error("unknown joint {joint} in motors.yaml (not listed in robot.yaml)")]
    UnknownMotorJoint { joint: String },
    #[error("duplicate motor CAN address {interface}:{device_id} in motors.yaml")]
    DuplicateMotorAddress { interface: String, device_id: u8 },
    #[error("invalid safety configuration {field}: {message}")]
    InvalidSafetyConfig { field: String, message: String },
    #[error("invalid limit margin on joint {joint}: {message}")]
    InvalidLimitMargin { joint: String, message: String },
    #[error("invalid actuator group {group}: {message}")]
    InvalidActuatorGroup { group: String, message: String },
    #[error("invalid velocity on joint {joint}: {message}")]
    InvalidVelocity { joint: String, message: String },
    #[error("invalid gravity_comp gains on joint {joint}: {message}")]
    InvalidGravityCompGains { joint: String, message: String },
    #[error("joint {joint} in robot.yaml has no entry in control.yaml")]
    MissingControlJoint { joint: String },
    #[error("MARENGO_JOINT_SUBSET names unknown joint {joint} (not in robot.joints)")]
    UnknownJointSubset { joint: String },
    #[error("MARENGO_JOINT_SUBSET intersects to zero joints")]
    EmptyJointSubset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RobotConfigFile {
    pub robot: RobotSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RobotSection {
    pub urdf: String,
    pub bench: BenchSection,
    pub joints: Vec<String>,
    /// Anatomical limb → joint membership for commissioning aggregation.
    /// Members may include unbuilt Offline inventory not listed in `joints`.
    #[serde(default)]
    pub limbs: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchSection {
    pub max_joint_torque_nm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorsConfigFile {
    pub motors: Vec<MotorEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MotorType {
    Rs00,
    Rs02,
    Rs03,
    Rs04,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorEntry {
    pub joint: String,
    pub driver: String,
    pub motor_type: MotorType,
    pub can_interface: String,
    pub device_id: u8,
    pub direction: i8,
    #[serde(default = "default_gear_ratio")]
    pub gear_ratio: f64,
    pub recv_can_id: u32,
    pub firmware_version: String,
    pub bench: MotorBenchLimits,
}

fn default_gear_ratio() -> f64 {
    1.0
}

fn default_feedback_drain_quiet_us() -> u64 {
    300
}

fn default_active_reporting_diagnostics() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorBenchLimits {
    pub position_lower_rad: f64,
    pub position_upper_rad: f64,
    pub velocity_limit_rad_s: f64,
    pub torque_limit_nm: f64,
}

fn read_yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    serde_yaml::from_str(&text).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Repository root: `MARENGO_ROOT` env, else compile-time workspace root from this crate.
pub fn resolve_repo_root() -> PathBuf {
    std::env::var("MARENGO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// Pi install default; dev falls back to `<repo_root>/config` when unset and path missing.
pub(crate) const DEFAULT_PI_CONFIG_DIR: &str = "/opt/marengo/config";

/// Config directory: `MARENGO_CONFIG_DIR`, else `/opt/marengo/config` when present, else `<repo_root>/config`.
pub fn resolve_config_dir(repo_root: impl AsRef<Path>) -> PathBuf {
    if let Ok(dir) = std::env::var("MARENGO_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let pi_default = PathBuf::from(DEFAULT_PI_CONFIG_DIR);
    if pi_default.is_dir() {
        return pi_default;
    }
    repo_root.as_ref().join("config")
}

/// File name of the physical reference journal beside the calibration history.
pub(crate) const REFERENCE_JOURNAL_FILE: &str = "reference-journal.sqlite3";

/// Physical reference journal: `MARENGO_REFERENCE_JOURNAL`, else
/// [`REFERENCE_JOURNAL_FILE`] beside the calibration history
/// (`MARENGO_CALIBRATION_RECORD`, else `homing.yaml` `calibration_record_path`
/// under `repo_root`). Relative results are anchored at the working directory
/// and `.`/`..` are collapsed lexically, because the journal requires an
/// absolute normal path.
pub fn resolve_reference_journal_path(
    repo_root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
) -> Result<PathBuf, ConfigError> {
    let path = if let Some(path) = std::env::var_os("MARENGO_REFERENCE_JOURNAL") {
        PathBuf::from(path)
    } else {
        let record = match std::env::var_os("MARENGO_CALIBRATION_RECORD") {
            Some(record) => PathBuf::from(record),
            None => repo_root.as_ref().join(
                load_homing_config_from(config_dir)?
                    .homing
                    .calibration_record_path,
            ),
        };
        record
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(REFERENCE_JOURNAL_FILE)
    };
    let absolute = std::path::absolute(&path).map_err(|e| ConfigError::Io {
        path: path.clone(),
        message: e.to_string(),
    })?;
    let mut normal = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    Ok(normal)
}

/// Load `robot.yaml` from `config_dir`.
pub fn load_robot_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<RobotConfigFile, ConfigError> {
    let cfg = read_yaml(&config_dir.as_ref().join("robot.yaml"))?;
    validate_robot_config(&cfg)?;
    Ok(cfg)
}

/// Load `motors.yaml` from `config_dir`.
pub fn load_motors_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<MotorsConfigFile, ConfigError> {
    let cfg = read_yaml(&config_dir.as_ref().join("motors.yaml"))?;
    validate_motors_config(&cfg)?;
    Ok(cfg)
}

/// Load `control.yaml` from `config_dir`.
pub fn load_control_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<ControlConfigFile, ConfigError> {
    let cfg: ControlConfigFile = read_yaml(&config_dir.as_ref().join("control.yaml"))?;
    validate_control_config(&cfg)?;
    Ok(cfg)
}

/// Load `homing.yaml` from `config_dir`.
pub fn load_homing_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<HomingConfigFile, ConfigError> {
    let cfg = read_yaml(&config_dir.as_ref().join("homing.yaml"))?;
    validate_homing_config(&cfg)?;
    Ok(cfg)
}

/// Load `config/robot.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_robot_config(repo_root: impl AsRef<Path>) -> Result<RobotConfigFile, ConfigError> {
    load_robot_config_from(resolve_config_dir(repo_root))
}

/// Load `config/motors.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_motors_config(repo_root: impl AsRef<Path>) -> Result<MotorsConfigFile, ConfigError> {
    load_motors_config_from(resolve_config_dir(repo_root))
}

/// CAN address of one configured drive, enough to send it a stop frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MotorStopTarget {
    pub can_interface: String,
    pub device_id: u8,
}

#[derive(Deserialize)]
struct MotorStopTargetsFile {
    motors: Vec<MotorStopTargetRow>,
}

#[derive(Deserialize)]
struct MotorStopTargetRow {
    can_interface: String,
    device_id: u8,
}

/// Every configured drive address from `motors.yaml` in `config_dir`, without
/// semantic validation, so a stop can still reach the drives when limits,
/// URDF, control or homing configuration are broken. Only `can_interface` and
/// `device_id` of each row are read; duplicates collapse to one target.
pub fn load_motor_stop_targets_from(
    config_dir: impl AsRef<Path>,
) -> Result<Vec<MotorStopTarget>, ConfigError> {
    let file: MotorStopTargetsFile = read_yaml(&config_dir.as_ref().join("motors.yaml"))?;
    let mut targets: Vec<MotorStopTarget> = Vec::with_capacity(file.motors.len());
    for row in file.motors {
        let target = MotorStopTarget {
            can_interface: row.can_interface,
            device_id: row.device_id,
        };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Ok(targets)
}

/// [`load_motor_stop_targets_from`] for the resolved config dir (honours `MARENGO_CONFIG_DIR`).
pub fn load_motor_stop_targets(
    repo_root: impl AsRef<Path>,
) -> Result<Vec<MotorStopTarget>, ConfigError> {
    load_motor_stop_targets_from(resolve_config_dir(repo_root))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlConfigFile {
    pub control: ControlSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlSection {
    pub loop_hz: u32,
    pub chappe_state_hz: u32,
    pub comm_watchdog_ms: u64,
    #[serde(default = "default_feedback_drain_quiet_us")]
    pub feedback_drain_quiet_us: u64,
    pub tau_ff_rate_limit_nm_per_s: f64,
    pub disable_on_exit: bool,
    #[serde(default)]
    pub bench: ControlBenchSection,
    pub motor_type_defaults: FxHashMap<String, MotorTypeDefaults>,
    #[serde(default)]
    pub actuator_groups: FxHashMap<String, ActuatorGroupEntry>,
    pub joints: FxHashMap<String, JointControlEntry>,
    pub danger_zones: Vec<DangerZoneRule>,
    /// GravityComp wrong-sign watchdog (ADR 0015). Trips when sign(torque_ff)
    /// opposes the expected sign sustained over `min_opposition_ticks`.
    #[serde(default)]
    pub wrong_sign_watchdog: WrongSignWatchdogConfig,
    /// Single-drive-loss degraded hold (ADR 0038). Required when any joint
    /// sets `on_drive_loss: shed_subtree`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive_loss: Option<DriveLossConfig>,
}

/// What Davout does when a joint's drive goes silent while Active (ADR 0038).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnDriveLoss {
    /// Stop every drive at once.
    #[default]
    DisableAll,
    /// Disable this joint and every joint distal to it; the proximal joints
    /// hold, then lower to rest, then everything is disabled. Admitted only
    /// when the offline gravity bound passes.
    ShedSubtree,
}

impl OnDriveLoss {
    pub fn is_disable_all(&self) -> bool {
        *self == Self::DisableAll
    }
}

/// Position-mode control law for one joint (ADR 0039 Phase 3 selector).
///
/// The key exists only while the scaled-PD law is qualified on the bench; the Phase 4
/// cutover removes it together with the legacy law.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionLaw {
    /// ADR 0007 law: host damping, planner repair heuristics, gated drive kd.
    #[default]
    Legacy,
    /// ADR 0039 law: drive-side PD on a feedback time-scaled reference.
    ScaledPd,
}

impl PositionLaw {
    pub fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }

    /// Trace/log spelling (matches the YAML value).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::ScaledPd => "scaled_pd",
        }
    }
}

/// Scaled-PD full-rate band `e0` as a fraction of `e1` when unset (ADR 0039: "start at e1/4").
pub const DEFAULT_POSITION_TIME_SCALE_E0_FRACTION: f64 = 0.25;

/// Scaled-PD integral band (rad) when unset: the legacy integral window (`|e| < 0.1`).
pub const DEFAULT_POSITION_INTEGRAL_BAND_RAD: f64 = 0.1;

/// Scaled-PD integral leak time constant (s) when unset. 0.5 s bounds the decay slope of the
/// 0.5 Nm integral to 1 Nm/s (0.005 Nm per 200 Hz tick) and drops a stale term within ~1.5 s.
pub const DEFAULT_POSITION_INTEGRAL_LEAK_S: f64 = 0.5;

/// Stribeck velocity `v_b` (rad/s) when unset. Inert while `fs` defaults to `fc`.
pub const DEFAULT_FRICTION_STRIBECK_VELOCITY_RAD_S: f64 = 0.05;

/// Timing and bounds of a degraded episode after a single drive loss (ADR 0038).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriveLossConfig {
    /// Healthy joints hold in place this long, then lower unless the operator acted (s).
    pub hold_window_s: f64,
    /// Speed cap of the lower to rest for every holding joint (rad/s).
    pub lower_velocity_rad_s: f64,
    /// Time allowed beyond `distance / lower_velocity_rad_s` to reach and settle at rest (s).
    pub lower_settle_s: f64,
    /// Cap on the lower phase of the Davout deadline (s).
    pub lower_max_s: f64,
    /// Admission margin: on every holding joint, `max |τ_g| + max |Δτ_g| + margin`
    /// must stay within its feed-forward torque cap (Nm).
    pub tau_margin_nm: f64,
}

/// Config-driven sign table for the GravityComp wrong-sign watchdog (ADR 0015).
///
/// Davout does not recompute `tau_g` (crate boundary). Instead, the expected sign
/// of `torque_ff` is config-driven: `expected_sign_at_positive_q` tells the
/// watchdog what sign the motor torque should have when `q > 0`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WrongSignWatchdogConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Expected sign of `torque_ff` when `q > 0` (depends on URDF joint axis + motor direction).
    #[serde(default = "default_expected_sign")]
    pub expected_sign_at_positive_q: i8,
    /// Minimum `|dq|` (rad/s) to consider sign (below this, sign is undefined).
    #[serde(default = "default_min_velocity")]
    pub min_velocity_rad_s: f64,
    /// Consecutive opposition ticks required to trip.
    #[serde(default = "default_min_opposition_ticks")]
    pub min_opposition_ticks: u32,
    /// Grace period ticks after enable (no trip during this window).
    #[serde(default = "default_grace_period_ticks")]
    pub grace_period_ticks: u32,
}

impl Default for WrongSignWatchdogConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            expected_sign_at_positive_q: default_expected_sign(),
            min_velocity_rad_s: default_min_velocity(),
            min_opposition_ticks: default_min_opposition_ticks(),
            grace_period_ticks: default_grace_period_ticks(),
        }
    }
}

fn default_expected_sign() -> i8 {
    -1
}

fn default_min_velocity() -> f64 {
    0.05
}

fn default_min_opposition_ticks() -> u32 {
    10
}

fn default_grace_period_ticks() -> u32 {
    20
}

/// Shared tuning for a named actuator grouping (e.g. shoulder pitch L/R, hips).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActuatorGroupEntry {
    pub joints: Vec<String>,
    pub velocity_max_rad_s: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlBenchSection {
    #[serde(default = "default_active_reporting_diagnostics")]
    pub active_reporting_diagnostics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorTypeDefaults {
    pub kp_max: f64,
    pub kd_max: f64,
    pub tau_ff_max_nm: f64,
    pub velocity_max_rad_s: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointControlEntry {
    pub motor_type: MotorType,
    pub gravity_comp: ModeGains,
    pub impedance: ModeGains,
    pub friction: FrictionGains,
    /// Operator desired MIT / planner velocity cap (rad/s). Overrides group and motor-type default.
    #[serde(default)]
    pub velocity_max_rad_s: Option<f64>,
    /// Max rate for ramping position-hold setpoints (`hold-at` / retarget). Default 0.25 rad/s.
    #[serde(default = "default_position_slew_rad_s")]
    pub position_slew_rad_s: f64,
    /// Max rad the ramped MIT setpoint may lead measured `q` (prevents racing ahead at high slew).
    #[serde(default = "default_position_slew_max_lead_rad")]
    pub position_slew_max_lead_rad: f64,
    /// `|target − q|` above this selects trapezoidal trajectory instead of slew (ADR 0007).
    #[serde(default = "default_position_trajectory_threshold_rad")]
    pub position_trajectory_threshold_rad: f64,
    /// Trajectory cruise speed cap (rad/s); must stay below Davout bench velocity limit.
    #[serde(default = "default_position_trajectory_velocity_rad_s")]
    pub position_trajectory_velocity_rad_s: f64,
    /// Trajectory acceleration cap (rad/s²).
    #[serde(default = "default_position_trajectory_accel_rad_s2")]
    pub position_trajectory_accel_rad_s2: f64,
    /// Friction follows `dq_des` when |dq_des| exceeds this (rad/s).
    #[serde(default = "default_position_trajectory_velocity_deadband_rad")]
    pub position_trajectory_velocity_deadband_rad: f64,
    /// Added to every position-hold target (rad). Bench trim when mechanical zero ≠ encoder zero.
    #[serde(default)]
    pub position_hold_trim_rad: f64,
    /// Response to this joint's drive going silent while Active (ADR 0038).
    #[serde(default, skip_serializing_if = "OnDriveLoss::is_disable_all")]
    pub on_drive_loss: OnDriveLoss,
    /// Position-mode law (ADR 0039 Phase 3). Default `legacy`.
    #[serde(default, skip_serializing_if = "PositionLaw::is_legacy")]
    pub position_law: PositionLaw,
    /// Scaled-PD full-rate band `e0` (rad): the reference runs at full rate while
    /// `|q_ref − q| ≤ e0`. Must be below `position_slew_max_lead_rad` (`e1`). Default `e1/4`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_time_scale_e0_rad: Option<f64>,
    /// Scaled-PD integral band (rad): the integral accumulates while `|target − q|` is below
    /// it and leaks otherwise. Default [`DEFAULT_POSITION_INTEGRAL_BAND_RAD`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_integral_band_rad: Option<f64>,
    /// Scaled-PD integral leak time constant (s) outside the band. Default
    /// [`DEFAULT_POSITION_INTEGRAL_LEAK_S`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_integral_leak_s: Option<f64>,
    /// Minimum position envelope margin at rest (rad). ADR 0009.
    #[serde(default = "default_position_limit_margin_min_rad")]
    pub position_limit_margin_min_rad: f64,
    /// Velocity-scaled envelope term: margin += k_v_s * |dq_cmd| (s).
    #[serde(default = "default_position_limit_margin_k_v_s")]
    pub position_limit_margin_k_v_s: f64,
    /// Stopping-distance scale on v²/(2a) envelope term.
    #[serde(default = "default_position_limit_margin_k_stop")]
    pub position_limit_margin_k_stop: f64,
    /// Measured-q fault slack beyond hard limits before disable (rad).
    #[serde(default = "default_position_limit_measured_fault_slack_rad")]
    pub position_limit_measured_fault_slack_rad: f64,
    /// Override URDF soft lower when `safety_controller` absent (rad).
    #[serde(default)]
    pub position_soft_lower_rad: Option<f64>,
    /// Override URDF soft upper when `safety_controller` absent (rad).
    #[serde(default)]
    pub position_soft_upper_rad: Option<f64>,
}

impl JointControlEntry {
    /// Scaled-PD time-scale stop band `e1` (rad): `position_slew_max_lead_rad`.
    pub fn time_scale_e1_rad(&self) -> f64 {
        self.position_slew_max_lead_rad
    }

    /// Scaled-PD full-rate band `e0` (rad), defaulting to `e1/4`.
    pub fn time_scale_e0_rad(&self) -> f64 {
        self.position_time_scale_e0_rad
            .unwrap_or(self.position_slew_max_lead_rad * DEFAULT_POSITION_TIME_SCALE_E0_FRACTION)
    }

    /// Scaled-PD integral band (rad).
    pub fn integral_band_rad(&self) -> f64 {
        self.position_integral_band_rad
            .unwrap_or(DEFAULT_POSITION_INTEGRAL_BAND_RAD)
    }

    /// Scaled-PD integral leak time constant (s).
    pub fn integral_leak_s(&self) -> f64 {
        self.position_integral_leak_s
            .unwrap_or(DEFAULT_POSITION_INTEGRAL_LEAK_S)
    }

    pub(crate) fn limit_margin_fields_valid(&self, joint: &str) -> Result<(), ConfigError> {
        if !self.position_limit_margin_min_rad.is_finite()
            || self.position_limit_margin_min_rad < 0.0
        {
            return Err(ConfigError::InvalidLimitMargin {
                joint: joint.to_string(),
                message: "position_limit_margin_min_rad must be finite and >= 0".to_string(),
            });
        }
        if !self.position_limit_margin_k_v_s.is_finite() || self.position_limit_margin_k_v_s < 0.0 {
            return Err(ConfigError::InvalidLimitMargin {
                joint: joint.to_string(),
                message: "position_limit_margin_k_v_s must be finite and >= 0".to_string(),
            });
        }
        if !self.position_limit_margin_k_stop.is_finite() || self.position_limit_margin_k_stop < 0.0
        {
            return Err(ConfigError::InvalidLimitMargin {
                joint: joint.to_string(),
                message: "position_limit_margin_k_stop must be finite and >= 0".to_string(),
            });
        }
        if !self.position_limit_measured_fault_slack_rad.is_finite()
            || self.position_limit_measured_fault_slack_rad < 0.0
        {
            return Err(ConfigError::InvalidLimitMargin {
                joint: joint.to_string(),
                message: "position_limit_measured_fault_slack_rad must be finite and >= 0"
                    .to_string(),
            });
        }
        if self.position_soft_lower_rad.is_some_and(|v| !v.is_finite())
            || self.position_soft_upper_rad.is_some_and(|v| !v.is_finite())
        {
            return Err(ConfigError::InvalidLimitMargin {
                joint: joint.to_string(),
                message: "soft bounds must be finite".to_string(),
            });
        }
        if let (Some(lo), Some(hi)) = (self.position_soft_lower_rad, self.position_soft_upper_rad) {
            if lo > hi {
                return Err(ConfigError::InvalidLimitMargin {
                    joint: joint.to_string(),
                    message: format!("position_soft_lower_rad {lo} > position_soft_upper_rad {hi}"),
                });
            }
        }
        Ok(())
    }
}

fn default_position_slew_rad_s() -> f64 {
    0.25
}

fn default_position_slew_max_lead_rad() -> f64 {
    0.15
}

fn default_position_trajectory_threshold_rad() -> f64 {
    0.15
}

fn default_position_trajectory_velocity_rad_s() -> f64 {
    0.30
}

fn default_position_trajectory_accel_rad_s2() -> f64 {
    0.20
}

fn default_position_trajectory_velocity_deadband_rad() -> f64 {
    0.02
}

fn default_position_limit_margin_min_rad() -> f64 {
    0.01
}

fn default_position_limit_margin_k_v_s() -> f64 {
    0.02
}

fn default_position_limit_margin_k_stop() -> f64 {
    0.5
}

fn default_position_limit_measured_fault_slack_rad() -> f64 {
    // Bench Set Limits stores taught hard; enable/settle jitter uses this slack
    // instead of silently widening motors.yaml (was Consul ±30 mrad pad).
    0.03
}

/// YAML key for [`MotorType`] in `motor_type_defaults`.
pub fn motor_type_key(motor_type: MotorType) -> &'static str {
    match motor_type {
        MotorType::Rs00 => "rs00",
        MotorType::Rs02 => "rs02",
        MotorType::Rs03 => "rs03",
        MotorType::Rs04 => "rs04",
    }
}

/// Actuator group containing `joint`, if any.
pub(crate) fn actuator_group_for_joint<'a>(
    joint: &str,
    control: &'a ControlSection,
) -> Option<(&'a str, &'a ActuatorGroupEntry)> {
    control
        .actuator_groups
        .iter()
        .find(|(_, group)| group.joints.iter().any(|j| j == joint))
        .map(|(name, group)| (name.as_str(), group))
}

/// Velocity cap from control.yaml (joint > group > motor type). ADR 0010.
pub fn resolve_joint_velocity_cap(
    joint: &str,
    motor_type: MotorType,
    control: &ControlSection,
) -> Result<f64, ConfigError> {
    if let Some(v) = control
        .joints
        .get(joint)
        .and_then(|entry| entry.velocity_max_rad_s)
    {
        if !v.is_finite() || v <= 0.0 {
            return Err(ConfigError::InvalidVelocity {
                joint: joint.to_string(),
                message: "velocity_max_rad_s must be finite and > 0".to_string(),
            });
        }
        return Ok(v);
    }
    if let Some((_group, entry)) = actuator_group_for_joint(joint, control) {
        if !entry.velocity_max_rad_s.is_finite() || entry.velocity_max_rad_s <= 0.0 {
            return Err(ConfigError::InvalidVelocity {
                joint: joint.to_string(),
                message: "actuator group velocity_max_rad_s must be finite and > 0".to_string(),
            });
        }
        return Ok(entry.velocity_max_rad_s);
    }
    let type_key = motor_type_key(motor_type);
    let defaults =
        control
            .motor_type_defaults
            .get(type_key)
            .ok_or_else(|| ConfigError::InvalidVelocity {
                joint: joint.to_string(),
                message: format!("missing motor_type_defaults.{type_key}"),
            })?;
    if !defaults.velocity_max_rad_s.is_finite() || defaults.velocity_max_rad_s <= 0.0 {
        return Err(ConfigError::InvalidVelocity {
            joint: joint.to_string(),
            message: format!(
                "motor_type_defaults.{type_key}.velocity_max_rad_s must be finite and > 0"
            ),
        });
    }
    Ok(defaults.velocity_max_rad_s)
}

fn validate_actuator_groups(control: &ControlSection) -> Result<(), ConfigError> {
    for (index, (group, entry)) in control.actuator_groups.iter().enumerate() {
        if !entry.velocity_max_rad_s.is_finite() || entry.velocity_max_rad_s <= 0.0 {
            return Err(ConfigError::InvalidActuatorGroup {
                group: group.clone(),
                message: "velocity_max_rad_s must be finite and > 0".to_string(),
            });
        }
        if entry.joints.is_empty() {
            return Err(ConfigError::InvalidActuatorGroup {
                group: group.clone(),
                message: "joints must not be empty".to_string(),
            });
        }
        for (position, joint) in entry.joints.iter().enumerate() {
            if !control.joints.contains_key(joint) {
                return Err(ConfigError::InvalidActuatorGroup {
                    group: group.clone(),
                    message: format!("joint {joint} not in control.joints"),
                });
            }
            // Unmodified map iteration order is stable, so `take(index)` revisits
            // exactly the groups that claimed joints earlier in this pass.
            let other = control
                .actuator_groups
                .iter()
                .take(index)
                .find(|(_, prior)| prior.joints.contains(joint))
                .map(|(name, _)| name)
                .or_else(|| entry.joints[..position].contains(joint).then_some(group));
            if let Some(other) = other {
                return Err(ConfigError::InvalidActuatorGroup {
                    group: group.clone(),
                    message: format!("joint {joint} already in group {other}"),
                });
            }
        }
    }
    Ok(())
}

/// Validate numeric control policy, gain caps, margins, groups, and danger rules.
pub(crate) fn validate_control_config(control: &ControlConfigFile) -> Result<(), ConfigError> {
    validate_actuator_groups(&control.control)?;
    for (joint, entry) in &control.control.joints {
        entry.limit_margin_fields_valid(joint)?;
        if let Some(v) = entry.velocity_max_rad_s {
            if !v.is_finite() || v <= 0.0 {
                return Err(ConfigError::InvalidVelocity {
                    joint: joint.clone(),
                    message: "velocity_max_rad_s must be finite and > 0".to_string(),
                });
            }
        }
        // ADR 0004: GravityComp wire gains must be zero (YAML is source of truth).
        const EPS: f64 = 1e-9;
        let g = &entry.gravity_comp;
        if !g.kp.is_finite() || !g.kd.is_finite() || !g.ki.is_finite() {
            return Err(ConfigError::InvalidGravityCompGains {
                joint: joint.clone(),
                message: "gravity_comp kp/kd/ki must be finite".to_string(),
            });
        }
        if g.kp.abs() > EPS || g.kd.abs() > EPS || g.ki.abs() > EPS {
            return Err(ConfigError::InvalidGravityCompGains {
                joint: joint.clone(),
                message: "gravity_comp kp/kd/ki must be 0.0 (ADR 0004)".to_string(),
            });
        }
    }
    safety_validation::validate_control_numbers(control)
}

/// Every `robot.joints` entry must have a matching `control.joints` entry.
pub fn validate_robot_control_joint_coverage(
    robot: &RobotConfigFile,
    control: &ControlConfigFile,
) -> Result<(), ConfigError> {
    for joint in &robot.robot.joints {
        if !control.control.joints.contains_key(joint) {
            return Err(ConfigError::MissingControlJoint {
                joint: joint.clone(),
            });
        }
    }
    Ok(())
}

/// Validate robot/motor/control identities, numeric policy, soft bounds, and planner caps.
pub fn validate_control_against_limits(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
) -> Result<(), ConfigError> {
    validate_motors_against_robot(robot, motors)?;
    validate_control_config(control)?;
    validate_robot_control_joint_coverage(robot, control)?;
    for joint in control.control.joints.keys() {
        if !robot.robot.joints.contains(joint) {
            return Err(ConfigError::InvalidSafetyConfig {
                field: "control.joints".to_string(),
                message: format!("joint {joint} not in robot.joints"),
            });
        }
    }
    for (group, entry) in &control.control.actuator_groups {
        for joint in &entry.joints {
            if !robot.robot.joints.contains(joint) {
                return Err(ConfigError::InvalidActuatorGroup {
                    group: group.clone(),
                    message: format!("joint {joint} not in robot.joints"),
                });
            }
        }
    }
    for joint in &robot.robot.joints {
        let joint_cfg =
            control
                .control
                .joints
                .get(joint)
                .ok_or_else(|| ConfigError::MissingControlJoint {
                    joint: joint.clone(),
                })?;
        let motor =
            motor_for_joint(motors, joint).ok_or_else(|| ConfigError::InvalidSafetyConfig {
                field: "robot.joints".to_string(),
                message: format!("joint {joint} has no matching motors.yaml entry"),
            })?;
        if joint_cfg.motor_type != motor.motor_type {
            return Err(ConfigError::InvalidSafetyConfig {
                field: format!("control.joints.{joint}.motor_type"),
                message: "must match motors.yaml".to_string(),
            });
        }
        for bound in [
            joint_cfg.position_soft_lower_rad,
            joint_cfg.position_soft_upper_rad,
        ]
        .into_iter()
        .flatten()
        {
            if bound < motor.bench.position_lower_rad || bound > motor.bench.position_upper_rad {
                return Err(ConfigError::InvalidLimitMargin {
                    joint: joint.clone(),
                    message: "soft bounds must lie within motor hard bounds".to_string(),
                });
            }
        }
        let cap = resolve_joint_velocity_cap(joint, motor.motor_type, &control.control)?;
        if joint_cfg.position_trajectory_velocity_rad_s > cap + 1e-9 {
            return Err(ConfigError::InvalidVelocity {
                joint: joint.clone(),
                message: format!(
                    "position_trajectory_velocity_rad_s {} > velocity cap {}",
                    joint_cfg.position_trajectory_velocity_rad_s, cap
                ),
            });
        }
    }
    Ok(())
}

/// Window an operator position command may target for `joint`: `control.yaml` soft bounds
/// intersected with `motors.yaml` bench hard bounds. `None` when the joint lacks either soft
/// bound or a motors entry, or the intersection is empty.
pub fn commanded_position_window(
    control: &ControlConfigFile,
    motors: &MotorsConfigFile,
    joint: &str,
) -> Option<(f64, f64)> {
    let soft = control.control.joints.get(joint)?;
    let hard = &motors.motors.iter().find(|m| m.joint == joint)?.bench;
    let lower = soft.position_soft_lower_rad?.max(hard.position_lower_rad);
    let upper = soft.position_soft_upper_rad?.min(hard.position_upper_rad);
    (lower <= upper).then_some((lower, upper))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeGains {
    pub kp: f64,
    pub kd: f64,
    pub ki: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrictionGains {
    pub fc: f64,
    pub fv: f64,
    pub fo: f64,
    pub k: f64,
    /// Static (breakaway) friction `fs` (Nm) for the scaled-PD law's Stribeck term
    /// (ADR 0039). Must be `>= fc`. Default `fc` (no Stribeck term).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fs: Option<f64>,
    /// Stribeck velocity `v_b` (rad/s): the breakaway excess decays as `exp(−|v|/v_b)`.
    /// Default [`DEFAULT_FRICTION_STRIBECK_VELOCITY_RAD_S`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_b: Option<f64>,
}

impl FrictionGains {
    /// Resolved static friction `fs` (defaults to `fc`).
    pub fn static_nm(&self) -> f64 {
        self.fs.unwrap_or(self.fc)
    }

    /// Resolved Stribeck velocity `v_b`.
    pub fn stribeck_velocity_rad_s(&self) -> f64 {
        self.v_b.unwrap_or(DEFAULT_FRICTION_STRIBECK_VELOCITY_RAD_S)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DangerZoneRule {
    pub name: String,
    pub joint: String,
    pub position_above_rad: f64,
    pub velocity_below_rad_s: f64,
    pub action: String,
    pub max_velocity_rad_s: f64,
    /// Optional torque cap (Nm) when `action` is `clamp_torque`.
    #[serde(default)]
    pub max_torque_nm: Option<f64>,
}

/// Load `config/homing.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_homing_config(repo_root: impl AsRef<Path>) -> Result<HomingConfigFile, ConfigError> {
    load_homing_config_from(resolve_config_dir(repo_root))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomingConfigFile {
    pub homing: HomingSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomingSection {
    pub zero_verify_tolerance_rad: f64,
    pub calibration_record_path: String,
    #[serde(default)]
    pub defaults: HomingJointDefaults,
    pub joints: FxHashMap<String, HomingJointEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomingMethod {
    None,
    ManualReference,
    HallThreeSensor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomingJointDefaults {
    #[serde(default)]
    pub home_offset_rad: f64,
    #[serde(default = "default_search_direction")]
    pub search_direction: SearchDirection,
    #[serde(default = "default_search_velocity")]
    pub search_velocity_rad_s: f64,
    #[serde(default = "default_search_torque")]
    pub search_torque_nm: f64,
    #[serde(default = "default_search_timeout")]
    pub search_timeout_s: f64,
    #[serde(default = "default_backoff")]
    pub backoff_rad: f64,
    #[serde(default = "default_true")]
    pub sign_test_required: bool,
    #[serde(default)]
    pub allow_sensor_overlap: bool,
}

impl Default for HomingJointDefaults {
    fn default() -> Self {
        Self {
            home_offset_rad: 0.0,
            search_direction: SearchDirection::Positive,
            search_velocity_rad_s: default_search_velocity(),
            search_torque_nm: default_search_torque(),
            search_timeout_s: default_search_timeout(),
            backoff_rad: default_backoff(),
            sign_test_required: true,
            allow_sensor_overlap: false,
        }
    }
}

fn default_homing_method() -> HomingMethod {
    HomingMethod::ManualReference
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchDirection {
    Positive,
    Negative,
}

fn default_search_direction() -> SearchDirection {
    SearchDirection::Positive
}

fn default_search_velocity() -> f64 {
    0.15
}

fn default_search_torque() -> f64 {
    0.5
}

fn default_search_timeout() -> f64 {
    30.0
}

fn default_backoff() -> f64 {
    0.05
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomingJointEntry {
    #[serde(default = "default_homing_method")]
    pub method: HomingMethod,
    #[serde(flatten)]
    pub overrides: HomingJointOverrides,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomingJointOverrides {
    #[serde(default)]
    pub home_offset_rad: Option<f64>,
    #[serde(default)]
    pub search_direction: Option<SearchDirection>,
    #[serde(default)]
    pub search_velocity_rad_s: Option<f64>,
    #[serde(default)]
    pub search_torque_nm: Option<f64>,
    #[serde(default)]
    pub search_timeout_s: Option<f64>,
    #[serde(default)]
    pub backoff_rad: Option<f64>,
    #[serde(default)]
    pub sign_test_required: Option<bool>,
    #[serde(default)]
    pub allow_sensor_overlap: Option<bool>,
    #[serde(default)]
    pub sensors: Option<HomingSensors>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomingSensors {
    pub home: SensorInput,
    pub min_limit: SensorInput,
    pub max_limit: SensorInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorInput {
    pub gpio: u8,
    #[serde(default = "default_true")]
    pub active_high: bool,
}

/// Effective homing parameters for one joint (defaults merged with overrides).
///
/// `J` is the joint-name type: owned by default, or `&str` borrowed from the
/// config via [`HomingSection::effective_joint_ref`] on per-tick paths.
#[derive(Debug, Clone)]
pub struct EffectiveHomingJoint<J = String> {
    pub joint: J,
    pub method: HomingMethod,
    pub home_offset_rad: f64,
    pub search_direction: SearchDirection,
    pub search_velocity_rad_s: f64,
    pub search_torque_nm: f64,
    pub search_timeout_s: f64,
    pub backoff_rad: f64,
    pub sign_test_required: bool,
    pub allow_sensor_overlap: bool,
    pub sensors: Option<HomingSensors>,
}

impl HomingSection {
    pub fn effective_joint(&self, joint: &str) -> Option<EffectiveHomingJoint> {
        let effective = self.effective_joint_ref(joint)?;
        Some(EffectiveHomingJoint {
            joint: effective.joint.to_string(),
            method: effective.method,
            home_offset_rad: effective.home_offset_rad,
            search_direction: effective.search_direction,
            search_velocity_rad_s: effective.search_velocity_rad_s,
            search_torque_nm: effective.search_torque_nm,
            search_timeout_s: effective.search_timeout_s,
            backoff_rad: effective.backoff_rad,
            sign_test_required: effective.sign_test_required,
            allow_sensor_overlap: effective.allow_sensor_overlap,
            sensors: effective.sensors,
        })
    }

    /// Same resolution as [`Self::effective_joint`] without allocating the name.
    pub fn effective_joint_ref(&self, joint: &str) -> Option<EffectiveHomingJoint<&str>> {
        let (joint, entry) = self.joints.get_key_value(joint)?;
        let d = &self.defaults;
        let o = &entry.overrides;
        Some(EffectiveHomingJoint {
            joint: joint.as_str(),
            method: entry.method,
            home_offset_rad: o.home_offset_rad.unwrap_or(d.home_offset_rad),
            search_direction: o.search_direction.unwrap_or(d.search_direction),
            search_velocity_rad_s: o.search_velocity_rad_s.unwrap_or(d.search_velocity_rad_s),
            search_torque_nm: o.search_torque_nm.unwrap_or(d.search_torque_nm),
            search_timeout_s: o.search_timeout_s.unwrap_or(d.search_timeout_s),
            backoff_rad: o.backoff_rad.unwrap_or(d.backoff_rad),
            sign_test_required: o.sign_test_required.unwrap_or(d.sign_test_required),
            allow_sensor_overlap: o.allow_sensor_overlap.unwrap_or(d.allow_sensor_overlap),
            sensors: o.sensors.clone(),
        })
    }

    pub fn configured_joints(&self) -> impl Iterator<Item = &String> {
        self.joints.keys()
    }
}

/// Load `config/control.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_control_config(repo_root: impl AsRef<Path>) -> Result<ControlConfigFile, ConfigError> {
    load_control_config_from(resolve_config_dir(repo_root))
}

/// Path to `control.yaml` under `config_dir`.
pub(crate) fn control_config_path(config_dir: impl AsRef<Path>) -> PathBuf {
    config_dir.as_ref().join("control.yaml")
}

/// Persist `control.yaml` under `config_dir` (validates, then atomic temp+rename).
pub fn write_control_config_from(
    config_dir: impl AsRef<Path>,
    cfg: &ControlConfigFile,
) -> Result<(), ConfigError> {
    let config_dir = config_dir.as_ref();
    let _lock = ProfileWriteLock::acquire(config_dir)?;
    validate_control_config(cfg)?;
    let path = control_config_path(config_dir);
    let text = serde_yaml::to_string(cfg).map_err(|e| ConfigError::Parse {
        path: path.clone(),
        message: e.to_string(),
    })?;
    atomic_file::write_atomic(&path, text.as_bytes())
}

/// Apply a config-tier tuning parameter; returns the previous value.
///
/// Rejects `velocity_max_rad_s` — that requires rebuilding Davout limits after reload.
pub fn apply_joint_config_param(
    entry: &mut JointControlEntry,
    param: &str,
    value: f64,
) -> Result<f64, ConfigError> {
    if !value.is_finite() {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!("non-finite value for {param}"),
        });
    }
    if value < 0.0
        && matches!(
            param,
            "impedance.kp"
                | "impedance.kd"
                | "impedance.ki"
                | "friction.fc"
                | "friction.fv"
                | "friction.k"
        )
    {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!("{param} must be >= 0"),
        });
    }
    match param {
        "impedance.kp" => {
            let before = entry.impedance.kp;
            entry.impedance.kp = value;
            Ok(before)
        }
        "impedance.kd" => {
            let before = entry.impedance.kd;
            entry.impedance.kd = value;
            Ok(before)
        }
        "impedance.ki" => {
            let before = entry.impedance.ki;
            entry.impedance.ki = value;
            Ok(before)
        }
        "friction.fc" => {
            let before = entry.friction.fc;
            entry.friction.fc = value;
            Ok(before)
        }
        "friction.fv" => {
            let before = entry.friction.fv;
            entry.friction.fv = value;
            Ok(before)
        }
        "friction.fo" => {
            let before = entry.friction.fo;
            entry.friction.fo = value;
            Ok(before)
        }
        "friction.k" => {
            let before = entry.friction.k;
            entry.friction.k = value;
            Ok(before)
        }
        "velocity_max_rad_s" => Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: "velocity_max_rad_s overlay gated until Davout limits rebuild is wired"
                .to_string(),
        }),
        other => Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!("unsupported config overlay param: {other}"),
        }),
    }
}

/// Reject impedance/friction gains above motor-type maxima (fail closed before persist).
/// Validator surface (P-marengo-config-07): no internal production caller today —
/// `apply_joint_config_param` does not invoke it — but validators are never
/// pruned, so this stays `pub` for operators and future patch gates.
pub fn validate_joint_gains_against_motor_type(
    cfg: &ControlConfigFile,
    joint: &str,
) -> Result<(), ConfigError> {
    let entry = cfg
        .control
        .joints
        .get(joint)
        .ok_or_else(|| ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!("unknown joint {joint}"),
        })?;
    validate_entry_gains_against_motor_type(cfg, joint, entry)
}

/// [`validate_joint_gains_against_motor_type`] for an entry the caller already holds.
pub(crate) fn validate_entry_gains_against_motor_type(
    cfg: &ControlConfigFile,
    joint: &str,
    entry: &JointControlEntry,
) -> Result<(), ConfigError> {
    let type_key = motor_type_key(entry.motor_type);
    let defaults = cfg
        .control
        .motor_type_defaults
        .get(type_key)
        .ok_or_else(|| ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!("missing motor_type_defaults.{type_key}"),
        })?;
    let nonnegative_gains = [
        entry.impedance.kp,
        entry.impedance.kd,
        entry.impedance.ki,
        entry.friction.fc,
        entry.friction.fv,
        entry.friction.k,
        defaults.kp_max,
        defaults.kd_max,
        defaults.tau_ff_max_nm,
    ];
    if nonnegative_gains
        .into_iter()
        .any(|value| !value.is_finite() || value < 0.0)
        || !entry.friction.fo.is_finite()
    {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!("joint {joint} gains and type maxima must be finite and >= 0; friction.fo must be finite"),
        });
    }
    if entry.impedance.kp > defaults.kp_max {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!(
                "impedance.kp {} exceeds kp_max {} for {type_key}",
                entry.impedance.kp, defaults.kp_max
            ),
        });
    }
    if entry.impedance.kd > defaults.kd_max {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!(
                "impedance.kd {} exceeds kd_max {} for {type_key}",
                entry.impedance.kd, defaults.kd_max
            ),
        });
    }
    // Match Berthier's conservative integral ceiling: the schema has no ki_max.
    if entry.impedance.ki > defaults.kp_max {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!(
                "impedance.ki {} exceeds kp_max {} for {type_key}",
                entry.impedance.ki, defaults.kp_max
            ),
        });
    }
    if entry.friction.fc > defaults.tau_ff_max_nm {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!(
                "friction.fc {} exceeds tau_ff_max_nm {} for {type_key}",
                entry.friction.fc, defaults.tau_ff_max_nm
            ),
        });
    }
    if entry.friction.static_nm() > defaults.tau_ff_max_nm {
        return Err(ConfigError::Parse {
            path: PathBuf::from("control.yaml"),
            message: format!(
                "friction.fs {} exceeds tau_ff_max_nm {} for {type_key}",
                entry.friction.static_nm(),
                defaults.tau_ff_max_nm
            ),
        });
    }
    Ok(())
}

/// Ensure every motor entry references a joint declared in `robot.yaml`.
pub fn validate_motors_against_robot(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
) -> Result<(), ConfigError> {
    validate_robot_config(robot)?;
    validate_motors_config(motors)?;
    for m in &motors.motors {
        if !robot.robot.joints.iter().any(|j| j == &m.joint) {
            return Err(ConfigError::UnknownMotorJoint {
                joint: m.joint.clone(),
            });
        }
    }
    Ok(())
}

/// Resolve URDF path from robot config; errors if the file is missing.
pub fn resolve_urdf_path(
    repo_root: impl AsRef<Path>,
    robot: &RobotConfigFile,
) -> Result<PathBuf, ConfigError> {
    let path = repo_root.as_ref().join(&robot.robot.urdf);
    if !path.is_file() {
        return Err(ConfigError::UrdfMissing { path });
    }
    Ok(path)
}

/// Lookup motor config for a joint name.
pub fn motor_for_joint<'a>(motors: &'a MotorsConfigFile, joint: &str) -> Option<&'a MotorEntry> {
    motors.motors.iter().find(|m| m.joint == joint)
}

/// Bench-wired joints eligible for actuator commands (4-DOF left arm bring-up).
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn repo_root() -> PathBuf {
        resolve_repo_root()
    }

    #[test]
    fn stop_targets_read_master_motors_and_keep_every_row() {
        let targets = load_motor_stop_targets(repo_root()).expect("stop targets");
        let motors = load_motors_config(repo_root()).expect("motors.yaml");
        assert_eq!(targets.len(), motors.motors.len());
        for motor in &motors.motors {
            assert!(targets.contains(&MotorStopTarget {
                can_interface: motor.can_interface.clone(),
                device_id: motor.device_id,
            }));
        }
    }

    #[test]
    fn stop_targets_survive_config_that_fails_full_validation() {
        let dir = tempfile::tempdir().expect("tempdir");
        // No bench limits, firmware or recv id; a duplicate address; a second
        // interface. Full validation rejects this, a stop must still address it.
        std::fs::write(
            dir.path().join("motors.yaml"),
            "motors:\n  - {joint: a, can_interface: can0, device_id: 1}\n  - {joint: b, can_interface: can0, device_id: 1}\n  - {joint: c, can_interface: can1, device_id: 7, motor_type: nonsense}\n",
        )
        .expect("write");
        assert!(load_motors_config_from(dir.path()).is_err());
        let targets = load_motor_stop_targets_from(dir.path()).expect("lenient stop targets");
        assert_eq!(
            targets,
            vec![
                MotorStopTarget {
                    can_interface: "can0".into(),
                    device_id: 1
                },
                MotorStopTarget {
                    can_interface: "can1".into(),
                    device_id: 7
                },
            ]
        );
    }

    #[test]
    fn stop_targets_error_when_motors_yaml_is_unreadable() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(load_motor_stop_targets_from(dir.path()).is_err());
        std::fs::write(dir.path().join("motors.yaml"), "motors: [not, rows]").expect("write");
        assert!(load_motor_stop_targets_from(dir.path()).is_err());
    }

    #[test]
    fn robot_yaml_parses() {
        let cfg = load_robot_config(repo_root()).expect("robot.yaml");
        assert!(cfg.robot.urdf.contains("marengo.urdf"));
        assert_eq!(cfg.robot.joints.len(), 5);
        // Match URDF chain + motors.yaml list order (pitch before roll).
        assert_eq!(
            cfg.robot.joints,
            vec![
                "right_shoulder_pitch".to_string(),
                "right_shoulder_roll".to_string(),
                "right_upper_arm_yaw".to_string(),
                "right_elbow_pitch".to_string(),
                "right_lower_arm_yaw".to_string(),
            ]
        );
        let right = cfg.robot.limbs.get("right_arm").expect("right_arm limb");
        assert!(right.contains(&"right_elbow_pitch".to_string()));
        assert!(right.contains(&"right_lower_arm_yaw".to_string()));
        assert_eq!(
            right.first().map(String::as_str),
            Some("right_shoulder_pitch")
        );
        assert!(cfg.robot.limbs.contains_key("left_arm"));
    }

    #[test]
    fn motors_yaml_parses_and_matches_robot() {
        let robot = load_robot_config(repo_root()).expect("robot");
        let motors = load_motors_config(repo_root()).expect("motors");
        assert_eq!(motors.motors.len(), 5);
        validate_motors_against_robot(&robot, &motors).expect("joint names align");

        let expected: &[(&str, &str, u8, i8, u32, MotorType)] = &[
            (
                "right_shoulder_pitch",
                "can0",
                1,
                -1,
                0x241,
                MotorType::Rs03,
            ),
            ("right_shoulder_roll", "can0", 2, 1, 0x242, MotorType::Rs03),
            ("right_upper_arm_yaw", "can0", 3, -1, 0x243, MotorType::Rs02),
            ("right_elbow_pitch", "can0", 4, -1, 0x244, MotorType::Rs02),
            ("right_lower_arm_yaw", "can0", 5, -1, 0x245, MotorType::Rs00),
        ];
        for &(joint, iface, device_id, direction, recv, motor_type) in expected {
            let m = motor_for_joint(&motors, joint).expect("missing joint in motors.yaml");
            assert_eq!(m.can_interface, iface, "{joint} can_interface");
            assert_eq!(m.device_id, device_id, "{joint} device_id");
            assert_eq!(m.direction, direction, "{joint} direction");
            assert_eq!(m.recv_can_id, recv, "{joint} recv_can_id");
            assert_eq!(m.motor_type, motor_type, "{joint} motor_type");
        }
    }

    #[test]
    fn duplicate_motor_addresses_are_rejected() {
        let robot = load_robot_config(repo_root()).expect("robot");
        let mut motors = load_motors_config(repo_root()).expect("motors");
        motors.motors[1].can_interface = motors.motors[0].can_interface.clone();
        motors.motors[1].device_id = motors.motors[0].device_id;

        let err = validate_motors_against_robot(&robot, &motors).expect_err("duplicate address");

        assert!(matches!(err, ConfigError::DuplicateMotorAddress { .. }));
    }

    #[test]
    fn missing_motor_error_names_the_robot_joint_direction() {
        let root = repo_root();
        let config_dir = resolve_config_dir(&root);
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let mut motors = load_motors_config_from(&config_dir).expect("motors");
        let control = load_control_config_from(&config_dir).expect("control");
        motors
            .motors
            .retain(|motor| motor.joint != "right_elbow_pitch");

        let error = validate_control_against_limits(&robot, &motors, &control)
            .expect_err("robot joint without motor");

        assert!(matches!(
            error,
            ConfigError::InvalidSafetyConfig { field, message }
                if field == "robot.joints"
                    && message.contains("right_elbow_pitch")
                    && message.contains("no matching motors.yaml entry")
        ));
    }

    #[test]
    fn master_config_validates_five_dof_right_bench() {
        let root = repo_root();
        let config_dir = resolve_config_dir(&root);
        let robot = load_robot_config_from(&config_dir).expect("robot.yaml");
        let motors = load_motors_config_from(&config_dir).expect("motors.yaml");
        validate_motors_against_robot(&robot, &motors).expect("joints align");
        assert!(robot.robot.urdf.contains("marengo.urdf"));
        let elbow = motor_for_joint(&motors, "right_elbow_pitch").expect("elbow");
        assert_eq!(elbow.device_id, 4);
        assert_eq!(elbow.can_interface, "can0");
        let lower = motor_for_joint(&motors, "right_lower_arm_yaw").expect("lower_arm_yaw");
        assert_eq!(lower.device_id, 5);
        assert_eq!(lower.can_interface, "can0");
        load_control_config_from(&config_dir).expect("control.yaml");
        let urdf = resolve_urdf_path(&root, &robot).expect("urdf");
        assert!(urdf.ends_with("marengo.urdf"));
    }

    #[test]
    fn production_urdf_resolves() {
        let cfg = load_robot_config(repo_root()).expect("robot.yaml");
        let path = resolve_urdf_path(repo_root(), &cfg).expect("marengo.urdf");
        assert!(path.ends_with("marengo.urdf"));
    }

    #[test]
    fn homing_yaml_parses() {
        let cfg = load_homing_config(repo_root()).expect("homing.yaml");
        assert!((cfg.homing.zero_verify_tolerance_rad - 0.05).abs() < 1e-9);
        let roll = cfg
            .homing
            .effective_joint("right_shoulder_roll")
            .expect("right_shoulder_roll");
        assert!(matches!(roll.method, HomingMethod::ManualReference));
    }

    #[test]
    fn control_yaml_parses() {
        let cfg = load_control_config(repo_root()).expect("control.yaml");
        assert_eq!(cfg.control.loop_hz, 200);
        assert_eq!(cfg.control.feedback_drain_quiet_us, 300);
        assert!(cfg.control.bench.active_reporting_diagnostics);
        assert!(cfg.control.motor_type_defaults.contains_key("rs03"));
    }

    #[test]
    fn humanoid_config_templates_parse_and_align() {
        let root = repo_root();
        let robot_path = root.join("config/robot_humanoid.yaml");
        let motors_path = root.join("config/motors_humanoid.yaml");
        let robot: RobotConfigFile = read_yaml(&robot_path).expect("robot_humanoid.yaml");
        let motors: MotorsConfigFile = read_yaml(&motors_path).expect("motors_humanoid.yaml");
        assert_eq!(robot.robot.joints.len(), 23);
        assert_eq!(motors.motors.len(), 23);
        validate_motors_against_robot(&robot, &motors).expect("humanoid joint names align");
        let knee = motor_for_joint(&motors, "left_knee").expect("left_knee");
        assert_eq!(knee.motor_type, MotorType::Rs04);
        let hip = motor_for_joint(&motors, "left_hip_pitch").expect("left_hip_pitch");
        assert_eq!(hip.motor_type, MotorType::Rs04);
    }

    fn sample_control_section() -> ControlSection {
        let mut motor_type_defaults = FxHashMap::default();
        motor_type_defaults.insert(
            "rs03".to_string(),
            MotorTypeDefaults {
                kp_max: 5000.0,
                kd_max: 100.0,
                tau_ff_max_nm: 5.0,
                velocity_max_rad_s: 2.0,
            },
        );
        let mut joints = FxHashMap::default();
        joints.insert(
            "right_shoulder_pitch".to_string(),
            JointControlEntry {
                motor_type: MotorType::Rs03,
                gravity_comp: ModeGains {
                    kp: 0.0,
                    kd: 0.0,
                    ki: 0.0,
                },
                impedance: ModeGains {
                    kp: 8.0,
                    kd: 1.0,
                    ki: 0.0,
                },
                friction: FrictionGains {
                    fc: 0.0,
                    fv: 0.0,
                    fo: 0.0,
                    k: 10.0,
                    fs: None,
                    v_b: None,
                },
                velocity_max_rad_s: None,
                position_slew_rad_s: 0.15,
                position_slew_max_lead_rad: 0.10,
                position_trajectory_threshold_rad: 0.0,
                position_trajectory_velocity_rad_s: 2.0,
                position_trajectory_accel_rad_s2: 4.8,
                position_trajectory_velocity_deadband_rad: 0.02,
                position_hold_trim_rad: 0.0,
                on_drive_loss: OnDriveLoss::DisableAll,
                position_law: PositionLaw::Legacy,
                position_time_scale_e0_rad: None,
                position_integral_band_rad: None,
                position_integral_leak_s: None,
                position_limit_margin_min_rad: 0.01,
                position_limit_margin_k_v_s: 0.02,
                position_limit_margin_k_stop: 0.5,
                position_limit_measured_fault_slack_rad: 0.005,
                position_soft_lower_rad: None,
                position_soft_upper_rad: None,
            },
        );
        let mut actuator_groups = FxHashMap::default();
        actuator_groups.insert(
            "shoulder_pitch".to_string(),
            ActuatorGroupEntry {
                joints: vec!["right_shoulder_pitch".to_string()],
                velocity_max_rad_s: 2.0,
            },
        );
        ControlSection {
            loop_hz: 200,
            chappe_state_hz: 50,
            comm_watchdog_ms: 50,
            feedback_drain_quiet_us: 300,
            tau_ff_rate_limit_nm_per_s: 20.0,
            disable_on_exit: true,
            bench: ControlBenchSection::default(),
            motor_type_defaults,
            actuator_groups,
            joints,
            danger_zones: vec![],
            wrong_sign_watchdog: WrongSignWatchdogConfig::default(),
            drive_loss: None,
        }
    }

    fn sample_motor() -> MotorEntry {
        MotorEntry {
            joint: "right_shoulder_pitch".to_string(),
            driver: "robstride".to_string(),
            motor_type: MotorType::Rs03,
            can_interface: "can0".to_string(),
            device_id: 1,
            direction: -1,
            gear_ratio: 1.0,
            recv_can_id: 0x241,
            firmware_version: "test".to_string(),
            bench: MotorBenchLimits {
                position_lower_rad: -0.9,
                position_upper_rad: 3.17,
                velocity_limit_rad_s: 2.0,
                torque_limit_nm: 5.0,
            },
        }
    }

    fn drive_loss_timing() -> DriveLossConfig {
        DriveLossConfig {
            hold_window_s: 5.0,
            lower_velocity_rad_s: 0.25,
            lower_settle_s: 3.0,
            lower_max_s: 10.0,
            tau_margin_nm: 0.5,
        }
    }

    #[test]
    fn on_drive_loss_defaults_to_disable_all_and_refuses_unknown_values() {
        let yaml = |extra: &str| {
            format!(
                "motor_type: rs03\ngravity_comp: {{kp: 0, kd: 0, ki: 0}}\n\
                 impedance: {{kp: 8, kd: 1, ki: 0}}\nfriction: {{fc: 0, fv: 0, fo: 0, k: 10}}\n{extra}"
            )
        };
        let entry: JointControlEntry = serde_yaml::from_str(&yaml("")).expect("default");
        assert_eq!(entry.on_drive_loss, OnDriveLoss::DisableAll);
        let entry: JointControlEntry =
            serde_yaml::from_str(&yaml("on_drive_loss: shed_subtree\n")).expect("shed");
        assert_eq!(entry.on_drive_loss, OnDriveLoss::ShedSubtree);
        assert!(
            serde_yaml::from_str::<JointControlEntry>(&yaml("on_drive_loss: descend\n")).is_err()
        );
        // The default is not written back, so existing files keep their bytes.
        let mut control = sample_control_section();
        let text = serde_yaml::to_string(&control).expect("yaml");
        assert!(!text.contains("on_drive_loss") && !text.contains("drive_loss"));
        control.drive_loss = Some(drive_loss_timing());
        let text = serde_yaml::to_string(&control).expect("yaml");
        let back: ControlSection = serde_yaml::from_str(&text).expect("round trip");
        assert_eq!(back.drive_loss, Some(drive_loss_timing()));
        assert!(serde_yaml::from_str::<DriveLossConfig>("hold_window_s: 5.0\nextra: 1\n").is_err());
    }

    #[test]
    fn shed_subtree_requires_valid_drive_loss_timing() {
        let mut control = ControlConfigFile {
            control: sample_control_section(),
        };
        control
            .control
            .joints
            .get_mut("right_shoulder_pitch")
            .expect("joint")
            .on_drive_loss = OnDriveLoss::ShedSubtree;
        let error = safety_validation::validate_control_numbers(&control).expect_err("no timing");
        assert!(
            matches!(&error, ConfigError::InvalidSafetyConfig { field, .. }
                if field == "control.joints.right_shoulder_pitch.on_drive_loss"),
            "{error}"
        );
        control.control.drive_loss = Some(drive_loss_timing());
        safety_validation::validate_control_numbers(&control).expect("valid timing");
        for bad in [0.0, -1.0, f64::NAN] {
            let mut timing = drive_loss_timing();
            timing.lower_velocity_rad_s = bad;
            control.control.drive_loss = Some(timing);
            assert!(
                safety_validation::validate_control_numbers(&control).is_err(),
                "{bad}"
            );
        }
        let mut timing = drive_loss_timing();
        timing.tau_margin_nm = -0.1;
        control.control.drive_loss = Some(timing);
        assert!(safety_validation::validate_control_numbers(&control).is_err());
    }

    #[test]
    fn joint_velocity_override_beats_group_and_type_default() {
        let mut control = sample_control_section();
        control
            .joints
            .get_mut("right_shoulder_pitch")
            .expect("joint")
            .velocity_max_rad_s = Some(1.5);
        let desired = resolve_joint_velocity_cap("right_shoulder_pitch", MotorType::Rs03, &control)
            .expect("desired");
        assert!((desired - 1.5).abs() < 1e-9);
    }

    #[test]
    fn group_velocity_used_when_joint_override_absent() {
        let control = sample_control_section();
        let desired = resolve_joint_velocity_cap("right_shoulder_pitch", MotorType::Rs03, &control)
            .expect("desired");
        assert!((desired - 2.0).abs() < 1e-9);
    }

    #[test]
    fn resolve_joint_velocity_cap_ignores_bench_yaml() {
        let control = sample_control_section();
        let mut motor = sample_motor();
        motor.bench.velocity_limit_rad_s = 0.5;
        let cap = resolve_joint_velocity_cap("right_shoulder_pitch", motor.motor_type, &control)
            .expect("cap");
        assert!((cap - 2.0).abs() < 1e-9);
    }

    #[test]
    fn duplicate_actuator_group_membership_rejected() {
        let mut control = sample_control_section();
        control.actuator_groups.insert(
            "other".to_string(),
            ActuatorGroupEntry {
                joints: vec!["right_shoulder_pitch".to_string()],
                velocity_max_rad_s: 1.0,
            },
        );
        let cfg = ControlConfigFile { control };
        let err = validate_control_config(&cfg).expect_err("duplicate");
        assert!(matches!(err, ConfigError::InvalidActuatorGroup { .. }));
    }

    #[test]
    fn non_zero_gravity_comp_gains_rejected() {
        let mut control = sample_control_section();
        control
            .joints
            .get_mut("right_shoulder_pitch")
            .expect("joint")
            .gravity_comp
            .kp = 1.0;
        let cfg = ControlConfigFile { control };
        let err = validate_control_config(&cfg).expect_err("non-zero gravity_comp");
        assert!(matches!(err, ConfigError::InvalidGravityCompGains { .. }));
    }

    #[test]
    fn nan_gravity_comp_gains_rejected() {
        let mut control = sample_control_section();
        control
            .joints
            .get_mut("right_shoulder_pitch")
            .expect("joint")
            .gravity_comp
            .kd = f64::NAN;
        let cfg = ControlConfigFile { control };
        let err = validate_control_config(&cfg).expect_err("nan gravity_comp");
        assert!(matches!(err, ConfigError::InvalidGravityCompGains { .. }));
    }

    #[test]
    fn missing_control_joint_rejected() {
        let root = repo_root();
        let config_dir = resolve_config_dir(&root);
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let mut control = load_control_config_from(&config_dir).expect("control");
        control.control.joints.remove("right_shoulder_pitch");
        let err =
            validate_robot_control_joint_coverage(&robot, &control).expect_err("missing joint");
        assert!(matches!(err, ConfigError::MissingControlJoint { .. }));
    }

    #[test]
    fn trajectory_velocity_above_effective_cap_rejected() {
        let root = repo_root();
        let config_dir = resolve_config_dir(&root);
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let motors = load_motors_config_from(&config_dir).expect("motors");
        let mut control = load_control_config_from(&config_dir).expect("control");
        control
            .control
            .joints
            .get_mut("right_shoulder_pitch")
            .expect("joint")
            .position_trajectory_velocity_rad_s = 99.0;
        let err = validate_control_against_limits(&robot, &motors, &control)
            .expect_err("trajectory too high");
        assert!(matches!(err, ConfigError::InvalidVelocity { .. }));
    }

    #[test]
    fn write_control_config_roundtrip_in_temp_dir() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy config");
        }
        let mut cfg = load_control_config_from(tmp.path()).expect("load");
        let before = cfg.control.joints["right_elbow_pitch"].impedance.kp;
        cfg.control
            .joints
            .get_mut("right_elbow_pitch")
            .expect("elbow")
            .impedance
            .kp = before + 3.0;
        write_control_config_from(tmp.path(), &cfg).expect("write");
        let reloaded = load_control_config_from(tmp.path()).expect("reload");
        assert!(
            (reloaded.control.joints["right_elbow_pitch"].impedance.kp - (before + 3.0)).abs()
                < 1e-9
        );
    }

    #[test]
    fn validate_joint_gains_rejects_over_max_kp() {
        let cfg = load_control_config(repo_root()).expect("control");
        let mut bad = cfg.clone();
        bad.control
            .joints
            .get_mut("right_elbow_pitch")
            .expect("elbow")
            .impedance
            .kp = 9999.0;
        let err = validate_joint_gains_against_motor_type(&bad, "right_elbow_pitch")
            .expect_err("over max");
        assert!(matches!(err, ConfigError::Parse { .. }));
    }

    #[test]
    fn validate_joint_gains_rejects_negative_kp() {
        let cfg = load_control_config(repo_root()).expect("control");
        let mut bad = cfg.clone();
        bad.control
            .joints
            .get_mut("right_elbow_pitch")
            .expect("elbow")
            .impedance
            .kp = -1.0;
        let err = validate_joint_gains_against_motor_type(&bad, "right_elbow_pitch")
            .expect_err("negative");
        assert!(matches!(err, ConfigError::Parse { .. }));
    }
}
