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
//! See [safety.md](../../docs/safety.md) and [ADR 0004](../../docs/decisions/0004-control-modes-and-mit.md).

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use armee_kinematics::{joint_limits, load_urdf, JointLimits};
use marengo_config::{
    load_control_config, load_motors_config, load_robot_config, motor_for_joint, resolve_urdf_path,
    validate_motors_against_robot, ControlConfigFile, MotorEntry, MotorType, MotorsConfigFile,
    RobotConfigFile,
};
pub use robstride::bus::{BusError, MemoryBus, MotorAddress, MotorBus};
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
}

/// Effective limits for one joint (URDF ∩ bench YAML).
#[derive(Debug, Clone, Copy)]
struct EffectiveLimits {
    lower: f64,
    upper: f64,
    velocity: f64,
    effort: f64,
    tau_ff_max: f64,
}

/// Safety supervisor — the only gateway to the motor bus.
pub struct Supervisor<B: MotorBus> {
    mode: OperationalMode,
    control_mode: ControlMode,
    hardware_estop: bool,
    limits: HashMap<String, EffectiveLimits>,
    pub motors: MotorsConfigFile,
    pub control: ControlConfigFile,
    motor_types: HashMap<MotorAddress, MotorType>,
    bus: B,
    motor_states: HashMap<MotorAddress, MotorState>,
    last_recv: Option<Instant>,
    active_since: Option<Instant>,
    last_tau_ff: HashMap<String, f64>,
    last_tick: Option<Instant>,
}

impl<B: MotorBus> Supervisor<B> {
    /// Build supervisor from repo `config/` and URDF limits.
    pub fn from_repo(repo_root: impl AsRef<Path>, bus: B) -> Result<Self, DavoutError> {
        let root = repo_root.as_ref();
        let robot = load_robot_config(root)?;
        let motors = load_motors_config(root)?;
        let control = load_control_config(root)?;
        validate_motors_against_robot(&robot, &motors)?;
        let urdf_path = resolve_urdf_path(root, &robot)?;
        let urdf_robot = load_urdf(&urdf_path)?;
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
            motor_types,
            bus,
            motor_states: HashMap::new(),
            last_recv: None,
            active_since: None,
            last_tau_ff: HashMap::new(),
            last_tick: None,
        })
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
            warn!("hardware E-stop asserted — disabled");
        }
    }

    pub fn set_homing_complete(&mut self) {
        if self.hardware_estop {
            return;
        }
        self.mode = OperationalMode::Ready;
        debug!("supervisor READY");
    }

    pub fn request_enable(&mut self, enable: bool) -> Result<(), DavoutError> {
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if enable {
            if self.mode != OperationalMode::Ready {
                return Err(DavoutError::NotActive { mode: self.mode });
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
                    self.motor_states
                        .insert(address, motor_to_joint_state(motor, raw)?);
                }
                self.last_recv = Some(Instant::now());
                Ok(n)
            }
            Err(BusError::RecvTimeout) => Ok(0),
            Err(e) => {
                warn!(error = %e, "motor feedback receive failed");
                Err(e.into())
            }
        }
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
        let cap = self.speed_cap_for_joint(&capped.joint, &motor)?;
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
        if out.position_rad < lim.lower || out.position_rad > lim.upper {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!(
                    "position {} outside [{}, {}]",
                    out.position_rad, lim.lower, lim.upper
                ),
            });
        }
        let vel_cap = lim.velocity.min(defaults.velocity_max_rad_s);
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

    fn speed_cap_for_joint(&self, joint: &str, motor: &MotorEntry) -> Result<f64, DavoutError> {
        let lim = self
            .limits
            .get(joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        let type_key = motor_type_key(motor.motor_type);
        let defaults = self
            .control
            .control
            .motor_type_defaults
            .get(type_key)
            .ok_or_else(|| DavoutError::Limit {
                joint: joint.to_string(),
                message: format!("no defaults for motor type {type_key}"),
            })?;
        Ok(lim.velocity.min(defaults.velocity_max_rad_s))
    }

    pub fn filter_speed_command(
        &self,
        cmd: SpeedCommand,
        motor: &MotorEntry,
    ) -> Result<SpeedCommand, DavoutError> {
        let cap = self.speed_cap_for_joint(&cmd.joint, motor)?;
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
        let out = cmd;
        if out.position_rad < lim.lower {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!("position {} < lower {}", out.position_rad, lim.lower),
            });
        }
        if out.position_rad > lim.upper {
            return Err(DavoutError::Limit {
                joint: out.joint.clone(),
                message: format!("position {} > upper {}", out.position_rad, lim.upper),
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

fn motor_type_key(ty: MotorType) -> &'static str {
    match ty {
        MotorType::Rs00 => "rs00",
        MotorType::Rs02 => "rs02",
        MotorType::Rs03 => "rs03",
        MotorType::Rs04 => "rs04",
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
) -> Result<HashMap<String, EffectiveLimits>, DavoutError> {
    let mut map = HashMap::new();
    for joint_name in &robot.robot.joints {
        let urdf_lim = joint_limits(urdf_robot, joint_name)?;
        let motor =
            motor_for_joint(motors, joint_name).ok_or_else(|| DavoutError::UnknownJoint {
                joint: joint_name.clone(),
            })?;
        let type_key = motor_type_key(motor.motor_type);
        let defaults = control
            .control
            .motor_type_defaults
            .get(type_key)
            .ok_or_else(|| DavoutError::Limit {
                joint: joint_name.clone(),
                message: format!("missing motor_type_defaults.{type_key}"),
            })?;
        let mut eff = effective_limits(&urdf_lim, motor, &robot.robot.bench);
        eff.tau_ff_max = eff
            .effort
            .min(motor.bench.torque_limit_nm)
            .min(defaults.tau_ff_max_nm);
        map.insert(joint_name.clone(), eff);
    }
    Ok(map)
}

fn effective_limits(
    urdf: &JointLimits,
    motor: &MotorEntry,
    bench: &marengo_config::BenchSection,
) -> EffectiveLimits {
    EffectiveLimits {
        lower: urdf.lower.max(motor.bench.position_lower_rad),
        upper: urdf.upper.min(motor.bench.position_upper_rad),
        velocity: urdf
            .velocity
            .min(motor.bench.velocity_limit_rad_s)
            .min(bench.max_joint_velocity_rad_s),
        effort: urdf
            .effort
            .min(motor.bench.torque_limit_nm)
            .min(bench.max_joint_torque_nm),
        tau_ff_max: motor.bench.torque_limit_nm,
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

    #[test]
    fn rejects_position_outside_limits() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let err = sup
            .filter_command(JointCommand {
                joint: "shoulder_roll".to_string(),
                position_rad: 99.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            })
            .expect_err("limit");
        assert!(matches!(err, DavoutError::Limit { .. }));
    }

    #[test]
    fn active_mode_required_to_send() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.set_homing_complete();
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
        sup.set_homing_complete();
        sup.request_enable(true).expect("enable");
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
        sup.set_homing_complete();
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
        sup.set_homing_complete();
        sup.request_enable(true).expect("enable");
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
                id: robstride::pack_ext_id(2, 0, 1),
                data: status,
                extended: true,
            },
        });
        sup.bus.rx.push(ReceivedCanFrame {
            interface: Some("can1".to_string()),
            frame: CanFrame {
                id: robstride::pack_ext_id(2, 0, 1),
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
    fn send_mit_converts_joint_command_to_motor_space() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.set_homing_complete();
        sup.request_enable(true).expect("enable");
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
        sup.set_homing_complete();
        sup.request_enable(true).expect("enable");
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
}
