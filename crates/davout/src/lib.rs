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
//! - [`disable_all`]: best-effort zero-torque MIT on shutdown.
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
//!   motor_states (joint space) ◄── direction/gear_ratio ◄── recv_all (motor space) ◄── CAN feedback
//!        │
//!        ▼
//!   Supervisor::joint_feedback (JointFeedback) — control / publish seam
//! ```
//!
//! Limits are built from [`armee_kinematics`] + [`marengo_config`] at startup.
//! See [safety.md](../../docs/safety.md), [ADR 0004](../../docs/decisions/0004-control-modes-and-mit.md),
//! and [ADR 0009](../../docs/decisions/0009-dynamic-position-limit-envelope.md).

pub use armee_kinematics::JointLimitPolicy;

mod active_reporting;
mod limit_envelope;

pub use active_reporting::{ActiveReportingLeaseError, ActiveReportingState, DEFAULT_LEASE_TTL};

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use armee_kinematics::{
    clamp_position_in_envelope, joint_limit_bounds, joint_limits, load_urdf,
    measured_position_fault, LimitMarginConfig,
};
use marengo_config::{
    load_control_config, load_homing_config, load_motors_config, load_robot_config,
    motor_for_joint, motor_type_key, resolve_joint_velocity_cap, resolve_urdf_path,
    validate_control_against_limits, validate_motors_against_robot,
    validate_robot_control_joint_coverage, ControlConfigFile, HomingConfigFile, MotorEntry,
    MotorType, MotorsConfigFile, RobotConfigFile,
};
use marengo_homing::{verify_manual_reference, HomingRegistry, VerifyError};
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
/// Cache values are already joint-space after [`Supervisor`] poll / synthetic insert.
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
    #[error("comm watchdog: no feedback for {ms} ms")]
    CommWatchdog { ms: u64 },
    #[error("danger zone {name} triggered on {joint}")]
    DangerZone { name: String, joint: String },
    #[error("firmware speed mode is disabled in control.bench.allow_firmware_speed_mode")]
    FirmwareSpeedModeDisabled,
    #[error("invalid motor config for {joint}: {message}")]
    InvalidMotorConfig { joint: String, message: String },
    #[error("motor fault on {joint}: 0x{fault:04x}")]
    MotorFault { joint: String, fault: u16 },
    #[error("homing: {message}")]
    Homing { message: String },
    #[error("homing verify on {joint}: {message}")]
    HomingVerify { joint: String, message: String },
    #[error("wrong-sign watchdog: gravity-comp torque opposes motion for {ticks} ticks on joint {joint}")]
    WrongSignWatchdog { joint: String, ticks: u32 },
    #[error("runtime limit changes are refused while supervisor is ACTIVE")]
    LimitPatchActive,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnablePolicy {
    Normal,
    ZeroCalibration,
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
    limits: HashMap<String, JointLimitPolicy>,
    robot: RobotConfigFile,
    urdf_robot: urdf_rs::Robot,
    pub motors: MotorsConfigFile,
    pub control: ControlConfigFile,
    pub homing_config: HomingConfigFile,
    homing: HomingRegistry,
    motor_types: HashMap<MotorAddress, MotorType>,
    bus: B,
    motor_states: HashMap<MotorAddress, MotorState>,
    last_recv: Option<Instant>,
    active_since: Option<Instant>,
    last_tau_ff: HashMap<String, f64>,
    feedback_velocity_trips: HashMap<String, u8>,
    last_feedback_samples: HashMap<String, FeedbackSample>,
    wrong_sign_state: HashMap<String, WrongSignState>,
    last_tick: Option<Instant>,
    active_reporting: ActiveReportingState,
    /// Status frames decoded in the most recent [`Self::refresh_feedback`] poll.
    last_refresh_frames: usize,
}

impl<B: MotorBus> Supervisor<B> {
    /// Build supervisor from repo `config/` and URDF limits.
    pub fn from_repo(repo_root: impl AsRef<Path>, bus: B) -> Result<Self, DavoutError> {
        let root = repo_root.as_ref();
        let mut robot = load_robot_config(root)?;
        let mut motors = load_motors_config(root)?;
        let mut control = load_control_config(root)?;
        if let Some(subset) = marengo_config::joint_subset_from_env() {
            marengo_config::apply_joint_subset(&mut robot, &mut motors, &mut control, &subset)?;
            info!(
                joint_count = robot.robot.joints.len(),
                "applied MARENGO_JOINT_SUBSET to Davout robot/motors/control"
            );
        }
        let homing_config = load_homing_config(root)?;
        validate_motors_against_robot(&robot, &motors)?;
        validate_robot_control_joint_coverage(&robot, &control)?;
        let homing_joints: Vec<String> = robot.robot.joints.clone();
        let homing = HomingRegistry::new(
            root,
            &homing_config.homing.calibration_record_path,
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
        let motor_types = motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();
        let mut supervisor = Self {
            mode: OperationalMode::Disabled,
            control_mode: ControlMode::Disabled,
            hardware_estop: false,
            limits,
            robot,
            urdf_robot,
            motors,
            control,
            homing_config,
            homing,
            motor_types,
            bus,
            motor_states: HashMap::new(),
            last_recv: None,
            active_since: None,
            last_tau_ff: HashMap::new(),
            feedback_velocity_trips: HashMap::new(),
            last_feedback_samples: HashMap::new(),
            wrong_sign_state: HashMap::new(),
            last_tick: None,
            active_reporting: ActiveReportingState::default(),
            last_refresh_frames: 0,
        };
        // Arm type-24 when configured so free-drive Set Limits can see motion while
        // limp (Disabled/Ready). MIT Active still turns reporting off in sync below.
        supervisor.sync_active_reporting();
        Ok(supervisor)
    }

    /// Frames received in the last [`Self::refresh_feedback`] call (0 if none or timeout).
    pub fn last_refresh_frame_count(&self) -> usize {
        self.last_refresh_frames
    }

    pub fn homing_registry(&self) -> &HomingRegistry {
        &self.homing
    }

    pub fn homing_registry_mut(&mut self) -> &mut HomingRegistry {
        &mut self.homing
    }

    pub fn joint_homing_state(&self, joint: &str) -> JointHomingState {
        self.homing.joint_state(joint)
    }

    /// Whether Davout currently has this joint's drive enabled (ACTIVE mode).
    ///
    /// Phase 5 refines this with `active_joints` for targeted enable; until then
    /// all configured motors share the supervisor ACTIVE/Disabled boundary.
    pub fn joint_drive_active(&self, joint: &str) -> bool {
        self.mode == OperationalMode::Active && self.motors.motors.iter().any(|m| m.joint == joint)
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

    /// Rebuild runtime limit policies from the supervisor's in-memory configuration.
    ///
    /// The existing policies remain installed if validation or rebuilding fails.
    pub fn rebuild_limits(&mut self) -> Result<(), DavoutError> {
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::LimitPatchActive);
        }
        validate_control_against_limits(&self.robot, &self.motors, &self.control)?;
        let limits = build_limits(&self.robot, &self.motors, &self.control, &self.urdf_robot)?;
        self.limits = limits;
        Ok(())
    }

    /// Joint-space feedback sample if available (cache is already joint-space).
    pub fn joint_feedback(&self, joint: &str) -> Option<JointFeedback> {
        let motor = motor_for_joint(&self.motors, joint)?;
        let address = MotorAddress::from(motor);
        let state = self.motor_states.get(&address)?;
        Some(JointFeedback {
            position_rad: f64::from(state.position_rad),
            velocity_rad_s: f64::from(state.velocity_rad_s),
            torque_nm: f64::from(state.torque_nm),
            temperature_c: state.temperature_c,
            fault: state.fault,
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

    /// Override synthetic feedback for one joint (unit tests / replay without CAN RX).
    pub fn set_synthetic_joint_feedback(
        &mut self,
        joint: &str,
        position_rad: f32,
        velocity_rad_s: f32,
    ) -> Result<(), DavoutError> {
        let motor = motor_for_joint(&self.motors, joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?
            .clone();
        let address = MotorAddress::from(&motor);
        let now = Instant::now();
        self.motor_states.insert(
            address,
            MotorState {
                position_rad,
                velocity_rad_s,
                torque_nm: 0.0,
                temperature_c: 0.0,
                fault: 0,
                updated: Some(now),
            },
        );
        self.last_recv = Some(now);
        Ok(())
    }

    /// Mark supervisor Ready when every configured joint is Verified.
    pub fn set_homing_complete(&mut self) -> Result<(), DavoutError> {
        if self.hardware_estop {
            return Ok(());
        }
        self.homing
            .require_ready()
            .map_err(|e| DavoutError::Homing {
                message: e.to_string(),
            })?;
        self.mode = OperationalMode::Ready;
        self.sync_active_reporting();
        debug!("supervisor READY (all joints verified)");
        Ok(())
    }

    /// Verify encoder reading after `set_zero_position` for manual-reference homing.
    pub fn verify_zero_after_set(
        &mut self,
        joint: &str,
        operator: &str,
        sign_test_passed: bool,
    ) -> Result<f64, DavoutError> {
        let motor = motor_for_joint(&self.motors, joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?
            .clone();
        let effective = self
            .homing_config
            .homing
            .effective_joint(joint)
            .ok_or_else(|| DavoutError::Homing {
                message: format!("joint {joint} missing from homing.yaml"),
            })?;
        let position = self
            .joint_position_rad(joint)
            .ok_or_else(|| DavoutError::HomingVerify {
                joint: joint.to_string(),
                message: "no feedback after set-zero".to_string(),
            })?;
        let lim = self
            .limits
            .get(joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        verify_manual_reference(
            &mut self.homing,
            &motor,
            &effective,
            position,
            lim.hard_lower(),
            lim.hard_upper(),
            sign_test_passed,
            operator,
            None,
        )
        .map_err(|e| match e {
            VerifyError::Registry(r) => DavoutError::Homing {
                message: r.to_string(),
            },
            other => DavoutError::HomingVerify {
                joint: joint.to_string(),
                message: other.to_string(),
            },
        })?;
        Ok(position)
    }

    /// Free-drive calibration path for Consul/MCP: validate joint, enable if needed,
    /// firmware SetZero, verify, then always disable.
    ///
    /// Refuses when already [`OperationalMode::Active`] so success cannot
    /// `disable_all` out from under GravityComp / hold.
    pub fn calibrate_joint_zero(
        &mut self,
        joint: &str,
        operator: &str,
        sign_test_passed: bool,
    ) -> Result<f64, DavoutError> {
        let joint = joint.trim();
        if joint.is_empty() {
            return Err(DavoutError::UnknownJoint {
                joint: joint.to_string(),
            });
        }
        // Resolve before enabling so UnknownJoint cannot leave the bus ACTIVE.
        let _motor =
            motor_for_joint(&self.motors, joint).ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::Homing {
                message: "set-zero refused while ACTIVE; disable motors first".into(),
            });
        }

        // Enable is inside the closure so a mid-bus enable failure still runs
        // disable_all (drives may be hardware-enabled while mode is still Disabled).
        let result = (|| {
            self.request_enable_for_calibration()?;
            self.set_zero_position(joint)?;
            let _ = self.refresh_feedback();
            self.verify_zero_after_set(joint, operator, sign_test_passed)
        })();

        if let Err(disable_err) = self.disable_all() {
            tracing::warn!(
                error = %disable_err,
                joint = %joint,
                "disable_all after set-zero failed"
            );
            if result.is_ok() {
                return Err(disable_err);
            }
        }
        result
    }

    pub fn mode(&self) -> OperationalMode {
        self.mode
    }

    pub fn control_mode(&self) -> ControlMode {
        self.control_mode
    }

    pub fn set_control_mode(&mut self, mode: ControlMode) {
        self.control_mode = mode;
        debug!(?mode, "control mode set");
    }

    /// Seed the tau_ff rate limiter with current measured torque for each joint.
    ///
    /// Called on mode transitions to ensure the rate limiter slews from the correct
    /// starting point. Do NOT clear `last_tau_ff` — that would cause `rate_limit_tau_ff`
    /// to use `unwrap_or(target)`, passing torque through unclamped (safety regression).
    pub fn seed_tau_ff_rate_limiter(&mut self) {
        for motor in &self.motors.motors {
            let joint = &motor.joint;
            let measured = self.joint_torque_rad(joint).unwrap_or(0.0);
            self.last_tau_ff.insert(joint.clone(), measured);
        }
    }

    /// Seed zero feedback for all configured motors (unit tests without CAN RX).
    pub fn seed_synthetic_feedback(&mut self) {
        let now = Instant::now();
        for motor in &self.motors.motors {
            self.motor_states.insert(
                MotorAddress::from(motor),
                MotorState {
                    position_rad: 0.0,
                    velocity_rad_s: 0.0,
                    torque_nm: 0.0,
                    temperature_c: 0.0,
                    fault: 0,
                    updated: Some(now),
                },
            );
        }
    }

    pub fn clear_motor_states(&mut self) {
        self.motor_states.clear();
    }

    pub fn bus_mut(&mut self) -> &mut B {
        &mut self.bus
    }

    pub fn set_hardware_estop(&mut self, asserted: bool) {
        // Runtime GPIO/input wiring is not yet integrated on the Pi bench — see
        // docs/position-hold-control-review.md (hardware E-stop gap).
        self.hardware_estop = asserted;
        if asserted {
            self.mode = OperationalMode::Disabled;
            self.control_mode = ControlMode::Disabled;
            self.active_since = None;
            self.feedback_velocity_trips.clear();
            self.last_feedback_samples.clear();
            self.wrong_sign_state.clear();
            warn!("hardware E-stop asserted — disabled");
        }
    }

    pub fn set_homing_complete_unchecked(&mut self) {
        if self.hardware_estop {
            return;
        }
        self.mode = OperationalMode::Ready;
        self.sync_active_reporting();
        debug!("supervisor READY (unchecked — deprecated)");
    }

    #[tracing::instrument(skip(self))]
    pub fn request_enable(&mut self, enable: bool) -> Result<(), DavoutError> {
        self.request_enable_with_policy(enable, EnablePolicy::Normal)
    }

    /// Enable drives for firmware `SetZero` before any joint is Verified.
    pub fn request_enable_for_calibration(&mut self) -> Result<(), DavoutError> {
        self.request_enable_with_policy(true, EnablePolicy::ZeroCalibration)
    }

    fn request_enable_with_policy(
        &mut self,
        enable: bool,
        policy: EnablePolicy,
    ) -> Result<(), DavoutError> {
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if enable {
            if self.mode == OperationalMode::Active {
                return Ok(());
            }
            match (self.mode, policy) {
                (OperationalMode::Ready, _) => {}
                (OperationalMode::Disabled, EnablePolicy::ZeroCalibration) => {}
                (OperationalMode::Disabled, EnablePolicy::Normal) => {
                    self.homing
                        .require_ready()
                        .map_err(|e| DavoutError::Homing {
                            message: format!("cannot enable: {e}"),
                        })?;
                    return Err(DavoutError::NotActive { mode: self.mode });
                }
                (mode, _) => {
                    return Err(DavoutError::NotActive { mode });
                }
            }
            for motor in &self.motors.motors {
                let address = MotorAddress::from(motor);
                info!(
                    joint = %motor.joint,
                    interface = %address.interface,
                    device_id = address.device_id,
                    "enabling motor"
                );
                self.bus.enable_drive_at(&address)?;
                self.bus.set_run_mode_at(&address, RunMode::Mit)?;
            }
            self.mode = OperationalMode::Active;
            self.active_since = Some(Instant::now());
            self.wrong_sign_state.clear();
            self.last_tick = None;
            self.sync_active_reporting();
            info!(motor_count = self.motors.motors.len(), "supervisor ACTIVE");
        } else {
            self.disable_all()?;
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
        self.poll_feedback(Duration::ZERO)
    }

    /// Poll CAN feedback up to [`feedback_poll_budget_us`](marengo_config::ControlSection::feedback_poll_budget_us).
    pub fn refresh_feedback(&mut self) -> Result<usize, DavoutError> {
        self.poll_feedback(self.feedback_poll_timeout())
    }

    fn poll_feedback(&mut self, budget: Duration) -> Result<usize, DavoutError> {
        let quiet = self.feedback_drain_quiet();
        let mut raw_states = HashMap::new();
        match self
            .bus
            .recv_all_addressed(&self.motor_types, &mut raw_states, budget, quiet)
        {
            Ok(n) => {
                self.last_refresh_frames = self.last_refresh_frames.saturating_add(n);
                let received_at = Instant::now();
                if n > 0 {
                    trace!(count = n, "received motor feedback batch");
                }
                for (address, raw) in raw_states {
                    let Some(motor) = self
                        .motors
                        .motors
                        .iter()
                        .find(|m| MotorAddress::from(*m) == address)
                    else {
                        continue;
                    };
                    let motor = motor.clone();
                    let mut state = motor_to_joint_state(&motor, raw)?;
                    self.check_feedback_velocity(&motor, &mut state, received_at)?;
                    self.check_feedback_position(&motor, &state)?;
                    self.motor_states.insert(address, state);
                }
                self.last_recv = Some(received_at);
                Ok(n)
            }
            Err(BusError::RecvTimeout) => Ok(0),
            Err(e) => {
                warn!(error = %e, "motor feedback receive failed");
                Err(e.into())
            }
        }
    }

    fn check_feedback_velocity(
        &mut self,
        motor: &MotorEntry,
        state: &mut MotorState,
        received_at: Instant,
    ) -> Result<(), DavoutError> {
        if self.mode != OperationalMode::Active {
            self.feedback_velocity_trips.remove(&motor.joint);
            self.last_feedback_samples.remove(&motor.joint);
            return Ok(());
        }
        let lim = self
            .limits
            .get(&motor.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: motor.joint.clone(),
            })?;
        let raw_velocity = f64::from(state.velocity_rad_s);
        let position = f64::from(state.position_rad);
        let previous = self.last_feedback_samples.get(&motor.joint).copied();
        let position_velocity = previous.and_then(|prev| {
            let dt = received_at.duration_since(prev.received_at).as_secs_f64();
            (dt > 0.0).then_some((position - prev.position_rad) / dt)
        });
        let measured_velocity = position_velocity.unwrap_or(raw_velocity);
        state.velocity_rad_s = measured_velocity as f32;
        let fault_threshold = lim.velocity + FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S;

        if raw_velocity.abs() > lim.velocity && measured_velocity.abs() <= fault_threshold {
            debug!(
                joint = %motor.joint,
                position_rad = position,
                previous_position_rad = previous.map(|prev| prev.position_rad),
                raw_velocity_rad_s = raw_velocity,
                measured_velocity_rad_s = measured_velocity,
                fault_threshold_rad_s = fault_threshold,
                limit_rad_s = lim.velocity,
                "ignored uncorroborated feedback velocity spike"
            );
            self.feedback_velocity_trips.remove(&motor.joint);
            self.last_feedback_samples.insert(
                motor.joint.clone(),
                FeedbackSample {
                    position_rad: position,
                    received_at,
                },
            );
            return Ok(());
        }

        if measured_velocity.abs() > fault_threshold {
            let trips = self
                .feedback_velocity_trips
                .entry(motor.joint.clone())
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
            warn!(
                joint = %motor.joint,
                position_rad = position,
                previous_position_rad = previous.map(|prev| prev.position_rad),
                raw_velocity_rad_s = raw_velocity,
                measured_velocity_rad_s = measured_velocity,
                fault_threshold_rad_s = fault_threshold,
                limit_rad_s = lim.velocity,
                trips = *trips,
                "feedback velocity limit exceeded"
            );
            self.last_feedback_samples.insert(
                motor.joint.clone(),
                FeedbackSample {
                    position_rad: position,
                    received_at,
                },
            );
            if *trips >= FEEDBACK_VELOCITY_LIMIT_TRIPS {
                return Err(DavoutError::Limit {
                    joint: motor.joint.clone(),
                    message: format!(
                        "feedback |velocity| {measured_velocity} > {fault_threshold} (limit {} + margin {})",
                        lim.velocity, FEEDBACK_VELOCITY_FAULT_MARGIN_RAD_S
                    ),
                });
            }
        } else {
            self.feedback_velocity_trips.remove(&motor.joint);
        }
        self.last_feedback_samples.insert(
            motor.joint.clone(),
            FeedbackSample {
                position_rad: position,
                received_at,
            },
        );
        Ok(())
    }

    fn check_feedback_position(
        &mut self,
        motor: &MotorEntry,
        state: &MotorState,
    ) -> Result<(), DavoutError> {
        if self.mode != OperationalMode::Active {
            return Ok(());
        }
        let lim = self
            .limits
            .get(&motor.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: motor.joint.clone(),
            })?;
        let position = f64::from(state.position_rad);
        if measured_position_fault(position, lim) {
            self.homing.mark_out_of_limits(&motor.joint);
            return Err(DavoutError::Limit {
                joint: motor.joint.clone(),
                message: format!(
                    "measured position {position} outside [{}, {}] (+ slack {})",
                    lim.hard_lower(),
                    lim.hard_upper(),
                    lim.margin.measured_fault_slack_rad
                ),
            });
        }
        Ok(())
    }

    fn check_comm_watchdog(&self) -> Result<(), DavoutError> {
        let max_ms = self.control.control.comm_watchdog_ms;
        if self.mode != OperationalMode::Active {
            return Ok(());
        }
        if max_ms != 0 {
            let last_feedback_or_enable = self.last_recv.or(self.active_since);
            let Some(last) = last_feedback_or_enable else {
                warn!(
                    max_ms,
                    "comm watchdog expired before first feedback timestamp"
                );
                return Err(DavoutError::CommWatchdog { ms: max_ms });
            };
            if last.elapsed() > Duration::from_millis(max_ms) {
                warn!(
                    max_ms,
                    elapsed_ms = last.elapsed().as_millis(),
                    "comm watchdog expired"
                );
                return Err(DavoutError::CommWatchdog { ms: max_ms });
            }
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
        self.check_comm_watchdog()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode != OperationalMode::Active {
            return Err(DavoutError::NotActive { mode: self.mode });
        }
        let filtered = self.filter_mit_command(cmd, motor)?;
        let scale = motor_position_scale(motor)?;
        let wire = MitCommand {
            device_id: motor.device_id,
            motor_type: motor.motor_type,
            position_rad: (filtered.position_rad * scale) as f32,
            velocity_rad_s: (filtered.velocity_rad_s * scale) as f32,
            kp: (filtered.kp / scale.powi(2)) as f32,
            kd: (filtered.kd / scale.powi(2)) as f32,
            torque_ff_nm: (filtered.torque_ff_nm / scale) as f32,
        };
        self.bus.mit_control_all_at(&[AddressedMitCommand {
            address: MotorAddress::from(motor),
            command: wire,
        }])?;
        Ok(())
    }

    /// Filter and send a batch of MIT commands (one per joint).
    pub fn send_mit_batch(&mut self, cmds: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        let batch_tick = Instant::now();
        for cmd in cmds {
            let joint = cmd.joint.clone();
            let motor = motor_for_joint(&self.motors, &joint)
                .ok_or(DavoutError::UnknownJoint { joint })?
                .clone();
            self.check_comm_watchdog()?;
            if self.hardware_estop {
                return Err(DavoutError::Estop);
            }
            if self.mode != OperationalMode::Active {
                return Err(DavoutError::NotActive { mode: self.mode });
            }
            let filtered = self.filter_mit_command_at_tick(cmd, &motor, self.last_tick)?;
            let scale = motor_position_scale(&motor)?;
            let wire = MitCommand {
                device_id: motor.device_id,
                motor_type: motor.motor_type,
                position_rad: (filtered.position_rad * scale) as f32,
                velocity_rad_s: (filtered.velocity_rad_s * scale) as f32,
                kp: (filtered.kp / scale.powi(2)) as f32,
                kd: (filtered.kd / scale.powi(2)) as f32,
                torque_ff_nm: (filtered.torque_ff_nm / scale) as f32,
            };
            self.bus.mit_control_all_at(&[AddressedMitCommand {
                address: MotorAddress::from(&motor),
                command: wire,
            }])?;
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
        self.check_comm_watchdog()?;
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode != OperationalMode::Active {
            return Err(DavoutError::NotActive { mode: self.mode });
        }
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
        let address = MotorAddress::from(&motor);
        self.bus.set_run_mode_at(&address, RunMode::Speed)?;
        self.bus.write_parameter_at(
            &address,
            ParameterId::LimitSpeed,
            ParameterValue::F32((cap * scale.abs()) as f32),
        )?;
        self.bus
            .speed_control_at(&address, (capped.velocity_rad_s * scale) as f32)?;
        Ok(capped.velocity_rad_s)
    }

    /// Best-effort speed reference zero for bench firmware speed mode.
    pub fn stop_speed_command(&mut self, joint: &str) -> Result<(), DavoutError> {
        let motor = motor_for_joint(&self.motors, joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?
            .clone();
        let address = MotorAddress::from(&motor);
        self.bus.speed_control_at(&address, 0.0)?;
        Ok(())
    }

    /// Set firmware zero for a joint after mechanical homing.
    pub fn set_zero_position(&mut self, joint: &str) -> Result<(), DavoutError> {
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode != OperationalMode::Active {
            return Err(DavoutError::NotActive { mode: self.mode });
        }
        let motor = motor_for_joint(&self.motors, joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?
            .clone();
        let address = MotorAddress::from(&motor);
        self.bus.set_zero_position_at(&address)?;
        Ok(())
    }

    /// Disable all drives (best-effort zero speed, zero-torque MIT, then DISABLE).
    #[tracing::instrument(skip(self))]
    pub fn disable_all(&mut self) -> Result<(), DavoutError> {
        for motor in &self.motors.motors {
            let address = MotorAddress::from(motor);
            let _ = self.bus.speed_control_at(&address, 0.0);
            let wire = MitCommand {
                device_id: motor.device_id,
                motor_type: motor.motor_type,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                kp: 0.0,
                kd: 0.0,
                torque_ff_nm: 0.0,
            };
            let _ = self.bus.mit_control_all_at(&[AddressedMitCommand {
                address: address.clone(),
                command: wire,
            }]);
            let _ = self.bus.disable_drive_at(&address);
        }
        self.mode = OperationalMode::Disabled;
        self.control_mode = ControlMode::Disabled;
        self.active_since = None;
        self.feedback_velocity_trips.clear();
        self.last_feedback_samples.clear();
        self.wrong_sign_state.clear();
        self.last_tick = None;
        self.sync_active_reporting();
        debug!("supervisor DISABLED");
        Ok(())
    }

    /// Free-drive sensing: type-24 per joint when diagnostics/leases say so and not ACTIVE.
    /// ACTIVE uses MIT status replies instead; type-24 must stay off then.
    pub fn sync_active_reporting(&mut self) {
        let mode_active = self.mode == OperationalMode::Active;
        let global = self.control.control.bench.active_reporting_diagnostics;
        let now = Instant::now();
        self.active_reporting
            .sync(&mut self.bus, &self.motors, mode_active, global, now);
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
        self.apply_danger_zone_clamps(&mut out, q_meas, dq_meas);
        if out.velocity_rad_s.abs() > vel_cap {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!("|velocity| {} > {}", out.velocity_rad_s, vel_cap),
            });
        }
        out.torque_ff_nm = out
            .torque_ff_nm
            .clamp(-lim.tau_ff_max, lim.tau_ff_max)
            .clamp(-defaults.tau_ff_max_nm, defaults.tau_ff_max_nm);

        out.torque_ff_nm = rate_limit_tau_ff(
            &mut self.last_tau_ff,
            &out.joint,
            out.torque_ff_nm,
            self.control.control.tau_ff_rate_limit_nm_per_s,
            previous_tick,
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
        let cap = self.speed_cap_for_joint(&cmd.joint)?;
        let mut out = cmd;
        out.velocity_rad_s = out.velocity_rad_s.clamp(-cap, cap);
        Ok(out)
    }

    fn apply_danger_zone_clamps(&self, cmd: &mut MitJointCommand, q_meas: f64, dq_meas: f64) {
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
                }
                _ => {}
            }
        }
    }

    /// Apply URDF + bench limits without sending (for tests and planners).
    pub fn filter_command(&self, cmd: JointCommand) -> Result<JointCommand, DavoutError> {
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

fn rate_limit_tau_ff(
    last: &mut HashMap<String, f64>,
    joint: &str,
    target: f64,
    rate_nm_s: f64,
    last_tick: Option<Instant>,
) -> f64 {
    let prev = last.get(joint).copied().unwrap_or(target);
    let dt = last_tick
        .map(|t| t.elapsed().as_secs_f64())
        .unwrap_or(0.01)
        .max(1e-4);
    let max_step = rate_nm_s * dt;
    let delta = (target - prev).clamp(-max_step, max_step);
    let out = prev + delta;
    last.insert(joint.to_string(), out);
    out
}

fn motor_position_scale(motor: &MotorEntry) -> Result<f64, DavoutError> {
    if motor.direction == 0 {
        return Err(DavoutError::InvalidMotorConfig {
            joint: motor.joint.clone(),
            message: "direction must be -1 or 1".to_string(),
        });
    }
    if motor.gear_ratio <= 0.0 {
        return Err(DavoutError::InvalidMotorConfig {
            joint: motor.joint.clone(),
            message: "gear_ratio must be positive".to_string(),
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
mod tests {
    #![allow(clippy::expect_used)]

    use marengo_config::LimitPatch;
    use robstride::{CanBus, CanFrame, CommunicationType, MemoryBus, ReceivedCanFrame};

    use super::*;

    #[derive(Default)]
    struct RoutedMemoryBus {
        tx: Vec<(MotorAddress, CanFrame)>,
        rx: Vec<ReceivedCanFrame>,
    }

    impl CanBus for RoutedMemoryBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            self.tx.push((MotorAddress::new("can0", 0), frame.clone()));
            Ok(())
        }

        fn send_frame_to(
            &mut self,
            address: &MotorAddress,
            frame: &CanFrame,
        ) -> Result<(), BusError> {
            self.tx.push((address.clone(), frame.clone()));
            Ok(())
        }

        fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
            out.append(&mut self.rx);
            Ok(())
        }
    }

    impl MotorBus for RoutedMemoryBus {}

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn bench_verify_all_joints<B: MotorBus>(sup: &mut Supervisor<B>) {
        let motors = sup.motors.motors.clone();
        sup.homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("bench homing");
    }

    fn bench_ready_active<B: MotorBus>(sup: &mut Supervisor<B>) {
        bench_verify_all_joints(sup);
        sup.set_homing_complete().expect("ready");
        sup.request_enable(true).expect("enable");
    }

    #[test]
    fn enable_blocked_without_verified_homing() {
        let temp =
            std::env::temp_dir().join(format!("marengo-homing-empty-{}.yaml", std::process::id()));
        let _ = std::fs::write(&temp, "joints: []\n");
        std::env::set_var("MARENGO_CALIBRATION_RECORD", &temp);
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let err = sup.request_enable(true).expect_err("blocked");
        assert!(matches!(err, DavoutError::Homing { .. }));
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn commissioning_wire_publishes_drive_active_and_homing() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let joint = "right_elbow_pitch".to_string();
        let (homing, drive, ool) = sup.joint_commissioning_wire(&joint);
        assert_eq!(
            homing,
            marengo_homing::to_proto_homing_state(marengo_homing::JointHomingState::Unhomed) as i32
        );
        assert!(!drive);
        assert!(!ool);

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
    fn measured_position_fault_marks_out_of_limits() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
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
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");

        sup.apply_limit_patch(&elbow_limit_patch(0.1, 1.5))
            .expect("apply patch");

        let policy = sup.joint_limit_policy("right_elbow_pitch").expect("policy");
        assert!((policy.hard_lower() - 0.1).abs() < 1e-9);
        assert!((policy.hard_upper() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn rebuild_limits_uses_in_memory_config() {
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
        let motor = sup
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == "right_elbow_pitch")
            .expect("motor");
        // Master URDF elbow hard upper is 1.2 — set bench below that so rebuild
        // uses the in-memory bench cap (URDF ∩ bench).
        motor.bench.position_upper_rad = 1.0;

        sup.rebuild_limits().expect("rebuild");

        let policy = sup.joint_limit_policy("right_elbow_pitch").expect("policy");
        assert!((policy.hard_upper() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn limit_patch_expands_urdf_when_past_current_hard() {
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
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
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
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
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
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

    /// Fails `enable_drive_at` after N successful enables (simulates mid-bus CAN fault).
    struct FailAfterNEnableBus {
        inner: MemoryBus,
        succeed: usize,
        enable_calls: usize,
        disable_calls: usize,
    }

    impl CanBus for FailAfterNEnableBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            self.inner.send_frame(frame)
        }

        fn send_frame_to(
            &mut self,
            address: &MotorAddress,
            frame: &CanFrame,
        ) -> Result<(), BusError> {
            self.inner.send_frame_to(address, frame)
        }

        fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
            self.inner.recv_frames_from(out)
        }
    }

    impl MotorBus for FailAfterNEnableBus {
        fn enable_drive_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
            if self.enable_calls >= self.succeed {
                return Err(BusError::Driver("injected enable failure".into()));
            }
            self.enable_calls += 1;
            self.inner.enable_drive_at(address)
        }

        fn disable_drive_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
            self.disable_calls += 1;
            self.inner.disable_drive_at(address)
        }
    }

    #[test]
    fn calibrate_joint_zero_disables_after_partial_enable_failure() {
        let motor_count = Supervisor::from_repo(repo_root(), MemoryBus::default())
            .expect("supervisor")
            .motors
            .motors
            .len();
        assert!(
            motor_count >= 2,
            "bench profile needs >=2 motors to inject mid-enable failure"
        );
        let bus = FailAfterNEnableBus {
            inner: MemoryBus::default(),
            succeed: 1,
            enable_calls: 0,
            disable_calls: 0,
        };
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let joint = sup.motors.motors[0].joint.clone();
        let err = sup
            .calibrate_joint_zero(&joint, "test", true)
            .expect_err("partial enable must fail");
        assert!(matches!(err, DavoutError::Bus(_)), "got {err}");
        assert_eq!(
            sup.mode(),
            OperationalMode::Disabled,
            "mode must not stick ACTIVE after failed calibration enable"
        );
        assert!(
            sup.bus.disable_calls >= motor_count,
            "disable_all must run after enable failure; disable_calls={}",
            sup.bus.disable_calls
        );
    }

    #[test]
    fn rate_limit_uses_shared_previous_tick() {
        use std::collections::HashMap;
        use std::time::{Duration, Instant};

        let mut last = HashMap::new();
        last.insert("j1".to_string(), 0.0);
        last.insert("j2".to_string(), 0.0);
        let prev = Instant::now() - Duration::from_millis(10);
        let out1 = rate_limit_tau_ff(&mut last, "j1", 10.0, 100.0, Some(prev));
        let out2 = rate_limit_tau_ff(&mut last, "j2", 10.0, 100.0, Some(prev));
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        bench_ready_active(&mut sup);

        // Set feedback with known position/velocity (torque_nm will be 0.0).
        sup.set_synthetic_joint_feedback("right_shoulder_pitch", 1.0, 0.5)
            .expect("feedback");

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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.seed_synthetic_feedback();
        sup.set_synthetic_joint_feedback("right_shoulder_pitch", 1.0, -0.5)
            .expect("feedback");
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.seed_synthetic_feedback();
        sup.set_synthetic_joint_feedback("right_shoulder_pitch", 0.2, -0.5)
            .expect("feedback");
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
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        assert!(!sup.bus.tx.is_empty());
        assert!(sup.bus.tx[0].extended);
    }

    #[test]
    fn enable_sends_lifecycle_and_mit_run_mode() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        // Free-drive type-24 frames are covered elsewhere; this test asserts enable + MIT run mode.
        sup.control.control.bench.active_reporting_diagnostics = false;
        bench_verify_all_joints(&mut sup);
        sup.set_homing_complete().expect("ready");
        sup.bus.tx.clear();
        sup.request_enable(true).expect("enable");
        assert_eq!(sup.mode(), OperationalMode::Active);
        assert_eq!(sup.bus.tx.len(), sup.motors.motors.len() * 2);
        let first = robstride::unpack_ext_id(sup.bus.tx[0].id).expect("enable id");
        assert_eq!(
            first.comm_type,
            robstride::CommunicationType::Enable.as_u8()
        );
        let second = robstride::unpack_ext_id(sup.bus.tx[1].id).expect("run_mode id");
        assert_eq!(
            second.comm_type,
            robstride::CommunicationType::WriteParameter.as_u8()
        );
    }

    #[test]
    fn firmware_speed_mode_requires_config_flag() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
            &Supervisor::from_repo(repo_root(), MemoryBus::default())
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

    /// Synthetic insert writes joint space; inverted direction must not re-transform on read.
    #[test]
    fn joint_feedback_preserves_synthetic_joint_space_with_inverted_direction() {
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
        let pitch = motor_for_joint(&sup.motors, "right_shoulder_pitch")
            .expect("right_shoulder_pitch")
            .joint
            .clone();
        for m in &mut sup.motors.motors {
            if m.joint == pitch {
                m.direction = -1;
            }
        }
        assert_eq!(
            motor_for_joint(&sup.motors, &pitch)
                .expect("pitch")
                .direction,
            -1
        );
        sup.set_synthetic_joint_feedback(&pitch, 1.0, 0.0)
            .expect("synthetic");
        let fb = sup.joint_feedback(&pitch).expect("feedback");
        assert!(
            (fb.position_rad - 1.0).abs() < 1e-9,
            "expected joint-space 1.0 without re-transform, got {}",
            fb.position_rad
        );
        assert!(
            (sup.joint_position_rad(&pitch).expect("position") - 1.0).abs() < 1e-9,
            "scalar accessor must match joint_feedback"
        );
    }

    #[test]
    fn joint_feedback_exposes_temperature_and_fault() {
        let mut sup = Supervisor::from_repo(repo_root(), MemoryBus::default()).expect("supervisor");
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
            ),
            data,
            extended: true,
        }
    }

    #[test]
    fn joint_feedback_transforms_once_on_refresh_with_inverted_scale() {
        let bus = RoutedMemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let joint = "right_elbow_pitch".to_string();
        for m in &mut sup.motors.motors {
            if m.joint == joint {
                m.direction = -1;
                m.gear_ratio = 2.0;
                m.can_interface = "can0".to_string();
            }
        }
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
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can0".to_string()),
            frame: status_frame_motor_space(
                motor.device_id,
                motor.motor_type,
                motor_pos,
                motor_vel,
                motor_tau,
                25.0,
            ),
        });

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
        let bus = RoutedMemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.motors.motors[0].can_interface = "can0".to_string();
        sup.motors.motors[0].device_id = 1;
        sup.motors.motors[1].can_interface = "can1".to_string();
        sup.motors.motors[1].device_id = 1;
        sup.motor_types = sup
            .motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();
        let status = [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x00, 0xC8];
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can0".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                data: status,
                extended: true,
            },
        });
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can1".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                data: status,
                extended: true,
            },
        });

        let count = sup.refresh_feedback().expect("feedback");

        assert_eq!(count, 2);
        let j0 = sup.motors.motors[0].joint.clone();
        let j1 = sup.motors.motors[1].joint.clone();
        assert!(sup.joint_feedback(&j0).is_some());
        assert!(sup.joint_feedback(&j1).is_some());
    }

    #[test]
    fn active_feedback_velocity_above_limit_faults() {
        let bus = RoutedMemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        bench_ready_active(&mut sup);
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
        let bus = RoutedMemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.motors.motors[0].can_interface = "can0".to_string();
        sup.motors.motors[0].device_id = 1;
        sup.motor_types = sup
            .motors
            .motors
            .iter()
            .map(|m| (MotorAddress::from(m), m.motor_type))
            .collect();
        bench_ready_active(&mut sup);
        let stationary_overspeed = ReceivedCanFrame {
            interface: Some("can0".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                data: [0x7f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0x00, 0xc8],
                extended: true,
            },
        };

        sup.bus.rx.push(stationary_overspeed.clone());
        sup.refresh_feedback().expect("first spike ignored");
        sup.bus.rx.push(stationary_overspeed);
        sup.refresh_feedback()
            .expect("stationary repeated spike remains ignored");

        assert!(sup.feedback_velocity_trips.is_empty());
        let joint = sup.motors.motors[0].joint.clone();
        let state = sup.joint_feedback(&joint).expect("sanitized state cached");
        assert_eq!(state.velocity_rad_s, 0.0);
    }

    #[test]
    fn cruise_near_limit_measured_velocity_does_not_fault() {
        let bus = RoutedMemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        bench_ready_active(&mut sup);
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
        let bus = RoutedMemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        bench_ready_active(&mut sup);
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        bench_ready_active(&mut sup);
        sup.bus.tx.clear();
        let mut motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        motor.direction = -1;
        motor.gear_ratio = 2.0;
        sup.send_mit_joint(
            MitJointCommand {
                joint: "right_elbow_pitch".to_string(),
                kp: 8.0,
                kd: 4.0,
                position_rad: 0.5,
                velocity_rad_s: 0.25,
                torque_ff_nm: 2.0,
            },
            &motor,
        )
        .expect("send");
        let expected = MitCommand {
            device_id: motor.device_id,
            motor_type: motor.motor_type,
            position_rad: -1.0,
            velocity_rad_s: -0.5,
            kp: 2.0,
            kd: 1.0,
            torque_ff_nm: -1.0,
        };
        let (expected_id, expected_data) = robstride::encode_mit(&expected);
        assert_eq!(sup.bus.tx.len(), 1);
        assert_eq!(sup.bus.tx[0].id, expected_id);
        assert_eq!(sup.bus.tx[0].data, expected_data);
    }

    #[test]
    fn watchdog_fires_when_active_without_first_feedback() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.control.control.comm_watchdog_ms = 1;
        bench_ready_active(&mut sup);
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
        assert!(matches!(err, DavoutError::CommWatchdog { ms: 1 }));
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        // Repo bench yaml may enable the flag; force on then off to emit disables.
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        sup.bus.tx.clear();
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.sync_active_reporting();
        let off = type24_tx_frames(&sup.bus.tx);
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        // Normalize to off, then enable.
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.sync_active_reporting();
        for m in &sup.motors.motors {
            assert!(!sup.active_reporting_applied(&m.joint));
        }
        sup.bus.tx.clear();
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        let type24 = type24_tx_frames(&sup.bus.tx);
        assert_eq!(type24.len(), sup.motors.motors.len());
        for frame in type24 {
            assert_eq!(frame.data[6], 0x01, "enable F_CMD");
        }
        let tx_before = sup.bus.tx.len();
        bench_ready_active(&mut sup);
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| !sup.active_reporting_applied(&m.joint)));
        let new_frames = &sup.bus.tx[tx_before..];
        for frame in type24_tx_frames(new_frames) {
            assert_eq!(
                frame.data[6], 0x00,
                "disable before Active (MIT owns status)"
            );
        }
    }

    #[test]
    fn active_reporting_zero_tx_during_active_mit_batch() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        bench_ready_active(&mut sup);
        let motor = motor_for_joint(&sup.motors, "right_elbow_pitch")
            .expect("motor")
            .clone();
        let tx_before = sup.bus.tx.len();
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
        assert!(!sup.bus.tx.is_empty());
        let new_frames = &sup.bus.tx[tx_before..];
        assert!(
            type24_tx_frames(new_frames).is_empty(),
            "MIT batch must not include type-24 frames"
        );
    }

    #[test]
    fn active_reporting_stays_armed_through_ready_for_free_drive() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        // Normalize off then on so enable TX is observed.
        sup.control.control.bench.active_reporting_diagnostics = false;
        sup.sync_active_reporting();
        for m in &sup.motors.motors {
            assert!(!sup.active_reporting_applied(&m.joint));
        }
        sup.bus.tx.clear();
        sup.control.control.bench.active_reporting_diagnostics = true;
        sup.sync_active_reporting();
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| sup.active_reporting_applied(&m.joint)));
        assert_eq!(type24_tx_frames(&sup.bus.tx).len(), sup.motors.motors.len());
        bench_verify_all_joints(&mut sup);
        let tx_before = sup.bus.tx.len();
        sup.set_homing_complete().expect("ready");
        assert_eq!(sup.mode(), OperationalMode::Ready);
        assert!(sup
            .motors
            .motors
            .iter()
            .all(|m| sup.active_reporting_applied(&m.joint)));
        let off_frames = type24_tx_frames(&sup.bus.tx[tx_before..]);
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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
        assert!(matches!(err, DavoutError::CommWatchdog { ms: 50 }));
    }

    #[test]
    fn filter_command_clamps_position_into_envelope() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
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

    fn wrong_sign_sup() -> Supervisor<MemoryBus> {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        // Master control.yaml disables the watchdog during elbow commissioning;
        // force-enable for unit coverage of the trip path.
        sup.control.control.wrong_sign_watchdog.enabled = true;
        bench_ready_active(&mut sup);
        sup.set_control_mode(ControlMode::GravityComp);
        sup.set_synthetic_joint_feedback("right_shoulder_pitch", 1.0, 0.5)
            .expect("feedback");
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
        let mut sup = wrong_sign_sup();
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
        let mut sup = wrong_sign_sup();
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
        let mut sup = wrong_sign_sup();
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
        let mut sup = wrong_sign_sup();
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
        let mut sup = wrong_sign_sup();
        sup.set_synthetic_joint_feedback("right_shoulder_pitch", 1.0, 0.01)
            .expect("feedback");
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
        let mut sup = wrong_sign_sup();
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
        let mut sup = wrong_sign_sup();
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
        let mut sup = wrong_sign_sup();
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
