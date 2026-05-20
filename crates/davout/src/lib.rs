//! Safety layer: joint/velocity/torque limits, estop, and command vetoes.
//!
//! **All motor commands must pass through [`Supervisor`]** before reaching [`robstride`].

use std::collections::HashMap;
use std::path::Path;

use armee_kinematics::{JointLimits, joint_limits, load_urdf};
use marengo_config::{
    MotorEntry, MotorsConfigFile, RobotConfigFile, load_motors_config, load_robot_config,
    motor_for_joint, resolve_urdf_path, validate_motors_against_robot,
};
pub use robstride::bus::{BusError, CanBus, JointMotion, MemoryBus};
use robstride::bus::send_motion;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalMode {
    Disabled,
    Ready,
    Active,
}

/// Command from control stack before safety filtering.
#[derive(Debug, Clone, PartialEq)]
pub struct JointCommand {
    pub joint: String,
    pub position_rad: f64,
    pub velocity_rad_s: f64,
    pub torque_nm: f64,
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
}

/// Effective limits for one joint (URDF ∩ bench YAML).
#[derive(Debug, Clone, Copy)]
struct EffectiveLimits {
    lower: f64,
    upper: f64,
    velocity: f64,
    effort: f64,
}

/// Safety supervisor — the only gateway to the motor bus.
pub struct Supervisor<B: CanBus> {
    mode: OperationalMode,
    hardware_estop: bool,
    limits: HashMap<String, EffectiveLimits>,
    motors: MotorsConfigFile,
    bus: B,
}

impl<B: CanBus> Supervisor<B> {
    /// Build supervisor from repo `config/` and URDF limits.
    pub fn from_repo(repo_root: impl AsRef<Path>, bus: B) -> Result<Self, DavoutError> {
        let root = repo_root.as_ref();
        let robot = load_robot_config(root)?;
        let motors = load_motors_config(root)?;
        validate_motors_against_robot(&robot, &motors)?;
        let urdf_path = resolve_urdf_path(root, &robot)?;
        let urdf_robot = load_urdf(&urdf_path)?;
        let limits = build_limits(&robot, &motors, &urdf_robot)?;
        Ok(Self {
            mode: OperationalMode::Disabled,
            hardware_estop: false,
            limits,
            motors,
            bus,
        })
    }

    pub fn mode(&self) -> OperationalMode {
        self.mode
    }

    pub fn set_hardware_estop(&mut self, asserted: bool) {
        self.hardware_estop = asserted;
        if asserted {
            self.mode = OperationalMode::Disabled;
            warn!("hardware E-stop asserted — disabled");
        }
    }

    /// Mark homing complete; transition to Ready if E-stop clear.
    pub fn set_homing_complete(&mut self) {
        if self.hardware_estop {
            return;
        }
        self.mode = OperationalMode::Ready;
        debug!("supervisor READY");
    }

    /// Operator enable request (Ready → Active).
    pub fn request_enable(&mut self, enable: bool) -> Result<(), DavoutError> {
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if enable {
            if self.mode != OperationalMode::Ready {
                return Err(DavoutError::NotActive {
                    mode: self.mode,
                });
            }
            self.mode = OperationalMode::Active;
            debug!("supervisor ACTIVE");
        } else {
            self.mode = OperationalMode::Disabled;
            debug!("supervisor DISABLED");
        }
        Ok(())
    }

    /// Filter and send a joint command. **This is the motor path.**
    pub fn send_joint_command(&mut self, cmd: JointCommand) -> Result<(), DavoutError> {
        if self.hardware_estop {
            return Err(DavoutError::Estop);
        }
        if self.mode != OperationalMode::Active {
            return Err(DavoutError::NotActive { mode: self.mode });
        }
        let filtered = self.filter_command(cmd)?;
        let motor = motor_for_joint(&self.motors, &filtered.joint).ok_or_else(|| DavoutError::UnknownJoint {
            joint: filtered.joint.clone(),
        })?;
        let motion = JointMotion {
            joint: filtered.joint.clone(),
            device_id: motor.device_id,
            position_rad: filtered.position_rad as f32,
            velocity_rad_s: filtered.velocity_rad_s as f32,
            torque_nm: filtered.torque_nm as f32,
        };
        send_motion(&mut self.bus, &motion)?;
        Ok(())
    }

    /// Apply URDF + bench limits without sending (for tests and planners).
    pub fn filter_command(&self, cmd: JointCommand) -> Result<JointCommand, DavoutError> {
        let lim = self.limits.get(&cmd.joint).ok_or_else(|| DavoutError::UnknownJoint {
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

fn build_limits(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    urdf_robot: &urdf_rs::Robot,
) -> Result<HashMap<String, EffectiveLimits>, DavoutError> {
    let mut map = HashMap::new();
    for joint_name in &robot.robot.joints {
        let urdf_lim = joint_limits(urdf_robot, joint_name)?;
        let motor = motor_for_joint(motors, joint_name).ok_or_else(|| DavoutError::UnknownJoint {
            joint: joint_name.clone(),
        })?;
        map.insert(
            joint_name.clone(),
            effective_limits(&urdf_lim, motor, &robot.robot.bench),
        );
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
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use robstride::MemoryBus;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn rejects_position_outside_limits() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let err = sup
            .filter_command(JointCommand {
                joint: "joint1".to_string(),
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
                joint: "joint1".to_string(),
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            })
            .expect_err("not active");
        assert!(matches!(err, DavoutError::NotActive { .. }));
    }

    #[test]
    fn send_records_frame_when_active() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        sup.set_homing_complete();
        sup.request_enable(true).expect("enable");
        sup.send_joint_command(JointCommand {
            joint: "joint1".to_string(),
            position_rad: 0.1,
            velocity_rad_s: 0.0,
            torque_nm: 0.0,
        })
        .expect("send");
    }
}
