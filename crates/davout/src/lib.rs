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
//! - [`danger_zones`](marengo_config::DangerZoneRule) from `config/control.yaml` (fault on rule hit).
//! - Comm watchdog: stale feedback → [`DavoutError::CommWatchdog`].
//! - [`disable_all`]: best-effort zero-torque MIT on shutdown.
//! - [`refresh_feedback`]: poll bus, update [`MotorState`] cache (via robstride).
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
//! ```
//!
//! Limits are built from [`armee_kinematics`] + [`marengo_config`] at startup.
//! See [safety.md](../../docs/safety.md), [ADR 0004](../../docs/decisions/0004-control-modes-and-mit.md),
//! and [ADR 0009](../../docs/decisions/0009-dynamic-position-limit-envelope.md).

pub use armee_kinematics::JointLimitPolicy;

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
    validate_control_against_limits, validate_motors_against_robot, ControlConfigFile,
    HomingConfigFile, MotorEntry, MotorType, MotorsConfigFile, RobotConfigFile,
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
}

pub use marengo_homing::JointHomingState;
pub use robstride::bus::{BusError, MemoryBus, MotorAddress, MotorBus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnablePolicy {
    Normal,
    ZeroCalibration,
}

const FEEDBACK_VELOCITY_LIMIT_TRIPS: u8 = 2;
const FEEDBACK_VELOCITY_CORROBORATION_EPS_RAD_S: f64 = 0.05;

#[derive(Debug, Clone, Copy)]
struct FeedbackSample {
    position_rad: f64,
    velocity_rad_s: f64,
    received_at: Instant,
}

/// Safety supervisor — the only gateway to the motor bus.
pub struct Supervisor<B: MotorBus> {
    mode: OperationalMode,
    control_mode: ControlMode,
    hardware_estop: bool,
    limits: HashMap<String, JointLimitPolicy>,
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
    last_tick: Option<Instant>,
}

impl<B: MotorBus> Supervisor<B> {
    /// Build supervisor from repo `config/` and URDF limits.
    pub fn from_repo(repo_root: impl AsRef<Path>, bus: B) -> Result<Self, DavoutError> {
        let root = repo_root.as_ref();
        let robot = load_robot_config(root)?;
        let motors = load_motors_config(root)?;
        let control = load_control_config(root)?;
        let homing_config = load_homing_config(root)?;
        validate_motors_against_robot(&robot, &motors)?;
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
        Ok(Self {
            mode: OperationalMode::Disabled,
            control_mode: ControlMode::Disabled,
            hardware_estop: false,
            limits,
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
            last_tick: None,
        })
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

    /// Per-joint limit policy (hard/soft bounds + margin config). ADR 0009.
    pub fn joint_limit_policy(&self, joint: &str) -> Option<&JointLimitPolicy> {
        self.limits.get(joint)
    }

    /// Command velocity cap (rad/s) from control.yaml resolution. ADR 0010.
    pub fn joint_velocity_cap(&self, joint: &str) -> Option<f64> {
        self.limits.get(joint).map(|lim| lim.velocity)
    }

    /// Joint-space feedback position (rad) if available.
    pub fn joint_position_rad(&self, joint: &str) -> Option<f64> {
        let motor = motor_for_joint(&self.motors, joint)?;
        let address = MotorAddress::from(motor);
        let raw = self.motor_states.get(&address)?;
        motor_to_joint_state(motor, *raw)
            .ok()
            .map(|s| f64::from(s.position_rad))
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

    pub fn motor_states(&self) -> &HashMap<MotorAddress, MotorState> {
        &self.motor_states
    }

    pub fn bus_mut(&mut self) -> &mut B {
        &mut self.bus
    }

    pub fn set_hardware_estop(&mut self, asserted: bool) {
        self.hardware_estop = asserted;
        if asserted {
            self.mode = OperationalMode::Disabled;
            self.control_mode = ControlMode::Disabled;
            self.active_since = None;
            self.feedback_velocity_trips.clear();
            self.last_feedback_samples.clear();
            warn!("hardware E-stop asserted — disabled");
        }
    }

    pub fn set_homing_complete_unchecked(&mut self) {
        if self.hardware_estop {
            return;
        }
        self.mode = OperationalMode::Ready;
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
            info!(motor_count = self.motors.motors.len(), "supervisor ACTIVE");
        } else {
            self.disable_all()?;
        }
        Ok(())
    }

    /// Poll CAN feedback; updates watchdog timestamp on success.
    pub fn refresh_feedback(&mut self) -> Result<usize, DavoutError> {
        let timeout = self.feedback_poll_timeout();
        let mut raw_states = HashMap::new();
        match self
            .bus
            .recv_all_addressed(&self.motor_types, &mut raw_states, timeout)
        {
            Ok(n) => {
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
        let velocity = raw_velocity.abs();
        let position = f64::from(state.position_rad);
        let previous = self.last_feedback_samples.get(&motor.joint).copied();
        let position_velocity = previous.and_then(|prev| {
            let dt = received_at.duration_since(prev.received_at).as_secs_f64();
            (dt > 0.0).then_some((position - prev.position_rad) / dt)
        });
        let sanitized_velocity = position_velocity.unwrap_or(0.0);
        state.velocity_rad_s = sanitized_velocity as f32;
        if velocity > lim.velocity {
            if position_velocity
                .map(|v| v.abs() <= lim.velocity + FEEDBACK_VELOCITY_CORROBORATION_EPS_RAD_S)
                .unwrap_or(true)
            {
                info!(
                    joint = %motor.joint,
                    position_rad = position,
                    previous_position_rad = previous.map(|prev| prev.position_rad),
                    raw_velocity_rad_s = raw_velocity,
                    previous_velocity_rad_s = previous.map(|prev| prev.velocity_rad_s),
                    position_velocity_rad_s = position_velocity,
                    sanitized_velocity_rad_s = sanitized_velocity,
                    limit_rad_s = lim.velocity,
                    "ignored uncorroborated feedback velocity spike"
                );
                self.feedback_velocity_trips.remove(&motor.joint);
                self.last_feedback_samples.insert(
                    motor.joint.clone(),
                    FeedbackSample {
                        position_rad: position,
                        velocity_rad_s: sanitized_velocity,
                        received_at,
                    },
                );
                return Ok(());
            }
            let trips = self
                .feedback_velocity_trips
                .entry(motor.joint.clone())
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
            warn!(
                joint = %motor.joint,
                position_rad = position,
                previous_position_rad = previous.map(|prev| prev.position_rad),
                velocity_rad_s = raw_velocity,
                previous_velocity_rad_s = previous.map(|prev| prev.velocity_rad_s),
                position_velocity_rad_s = position_velocity,
                limit_rad_s = lim.velocity,
                trips = *trips,
                "feedback velocity limit exceeded"
            );
            self.last_feedback_samples.insert(
                motor.joint.clone(),
                FeedbackSample {
                    position_rad: position,
                    velocity_rad_s: sanitized_velocity,
                    received_at,
                },
            );
            if *trips >= FEEDBACK_VELOCITY_LIMIT_TRIPS {
                return Err(DavoutError::Limit {
                    joint: motor.joint.clone(),
                    message: format!("feedback |velocity| {velocity} > {}", lim.velocity),
                });
            }
        } else {
            self.feedback_velocity_trips.remove(&motor.joint);
        }
        self.last_feedback_samples.insert(
            motor.joint.clone(),
            FeedbackSample {
                position_rad: position,
                velocity_rad_s: sanitized_velocity,
                received_at,
            },
        );
        Ok(())
    }

    fn check_feedback_position(
        &self,
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
        self.apply_danger_zones(&filtered)?;
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
        for cmd in cmds {
            let joint = cmd.joint.clone();
            let motor = motor_for_joint(&self.motors, &joint)
                .ok_or(DavoutError::UnknownJoint { joint })?
                .clone();
            self.send_mit_joint(cmd, &motor)?;
        }
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
        debug!("supervisor DISABLED");
        Ok(())
    }

    fn feedback_poll_timeout(&self) -> Duration {
        let watchdog = Duration::from_millis(self.control.control.comm_watchdog_ms);
        if watchdog.is_zero() {
            Duration::from_millis(1)
        } else {
            watchdog.min(Duration::from_millis(1))
        }
    }

    pub fn filter_mit_command(
        &mut self,
        cmd: MitJointCommand,
        motor: &MotorEntry,
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
        let keepalive = out.kp == 0.0 && out.kd == 0.0 && out.torque_ff_nm == 0.0;
        if !keepalive {
            let clamped =
                clamp_position_in_envelope(lim, q_meas, out.velocity_rad_s, out.position_rad);
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
            self.last_tick,
        );
        self.last_tick = Some(Instant::now());

        Ok(out)
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

    fn apply_danger_zones(&self, cmd: &MitJointCommand) -> Result<(), DavoutError> {
        for rule in &self.control.control.danger_zones {
            if rule.joint != cmd.joint {
                continue;
            }
            if cmd.position_rad > rule.position_above_rad
                && cmd.velocity_rad_s < rule.velocity_below_rad_s
                && rule.action == "clamp_velocity"
            {
                return Err(DavoutError::DangerZone {
                    name: rule.name.clone(),
                    joint: cmd.joint.clone(),
                });
            }
        }
        Ok(())
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

fn build_limits(
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

    use super::*;
    use robstride::{CanBus, CanFrame, MemoryBus, ReceivedCanFrame};

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
    fn rejects_velocity_outside_limits() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let err = sup
            .filter_command(JointCommand {
                joint: "shoulder_roll".to_string(),
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
                joint: "shoulder_roll".to_string(),
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
        let motor = motor_for_joint(&sup.motors, "elbow")
            .expect("motor")
            .clone();
        sup.send_mit_joint(
            MitJointCommand {
                joint: "elbow".to_string(),
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
        bench_ready_active(&mut sup);
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
                joint: "elbow".to_string(),
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
            "elbow",
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
        assert!(sup
            .motor_states()
            .contains_key(&MotorAddress::new("can0", 1)));
        assert!(sup
            .motor_states()
            .contains_key(&MotorAddress::new("can1", 1)));
    }

    #[test]
    fn active_feedback_velocity_above_limit_faults() {
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
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can0".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8],
                extended: true,
            },
        });

        sup.refresh_feedback().expect("initial feedback");
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can0".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                data: [0x8f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0x00, 0xc8],
                extended: true,
            },
        });

        sup.refresh_feedback()
            .expect("first corroborated overspeed warns only");
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can0".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 1, robstride::DEFAULT_HOST_ID),
                data: [0x9f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0x00, 0xc8],
                extended: true,
            },
        });

        let err = sup.refresh_feedback().expect_err("overspeed feedback");

        assert!(matches!(err, DavoutError::Limit { .. }));
        assert!(err.to_string().contains("feedback |velocity|"));
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
        let state = sup
            .motor_states()
            .get(&MotorAddress::new("can0", 1))
            .expect("sanitized state cached");
        assert_eq!(state.velocity_rad_s, 0.0);
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
        let mut motor = motor_for_joint(&sup.motors, "elbow")
            .expect("motor")
            .clone();
        motor.direction = -1;
        motor.gear_ratio = 2.0;
        sup.send_mit_joint(
            MitJointCommand {
                joint: "elbow".to_string(),
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
        let motor = motor_for_joint(&sup.motors, "elbow")
            .expect("motor")
            .clone();
        let err = sup
            .send_mit_joint(
                MitJointCommand {
                    joint: "elbow".to_string(),
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

    #[test]
    fn filter_command_clamps_position_into_envelope() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let out = sup
            .filter_command(JointCommand {
                joint: "elbow".to_string(),
                position_rad: 99.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            })
            .expect("clamp");
        let policy = sup.joint_limit_policy("elbow").expect("policy");
        assert!(out.position_rad <= policy.hard_upper());
        assert!(out.position_rad < 99.0);
    }
}
