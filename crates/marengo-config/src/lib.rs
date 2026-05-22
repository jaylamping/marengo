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
//! | `network.yaml` | [`NetworkConfigFile`] | Chappe / bins |
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

use std::path::{Path, PathBuf};

use serde::Deserialize;
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotConfigFile {
    pub robot: RobotSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotSection {
    pub name: String,
    pub urdf: String,
    pub bench: BenchSection,
    pub joints: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BenchSection {
    pub max_joint_velocity_rad_s: f64,
    pub max_joint_torque_nm: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotorsConfigFile {
    pub motors: Vec<MotorEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MotorType {
    Rs00,
    Rs02,
    Rs03,
    Rs04,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct MotorBenchLimits {
    pub position_lower_rad: f64,
    pub position_upper_rad: f64,
    pub velocity_limit_rad_s: f64,
    pub torque_limit_nm: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkConfigFile {
    pub network: NetworkSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkSection {
    pub chappe_bind: String,
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

/// Load `config/robot.yaml` relative to `repo_root`.
pub fn load_robot_config(repo_root: impl AsRef<Path>) -> Result<RobotConfigFile, ConfigError> {
    let path = repo_root.as_ref().join("config/robot.yaml");
    read_yaml(&path)
}

/// Load `config/motors.yaml` relative to `repo_root`.
pub fn load_motors_config(repo_root: impl AsRef<Path>) -> Result<MotorsConfigFile, ConfigError> {
    let path = repo_root.as_ref().join("config/motors.yaml");
    read_yaml(&path)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlConfigFile {
    pub control: ControlSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlSection {
    pub loop_hz: u32,
    pub chappe_state_hz: u32,
    pub comm_watchdog_ms: u64,
    pub tau_ff_rate_limit_nm_per_s: f64,
    pub disable_on_exit: bool,
    #[serde(default)]
    pub bench: ControlBenchSection,
    pub motor_type_defaults: std::collections::HashMap<String, MotorTypeDefaults>,
    pub joints: std::collections::HashMap<String, JointControlEntry>,
    pub danger_zones: Vec<DangerZoneRule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ControlBenchSection {
    #[serde(default)]
    pub allow_firmware_speed_mode: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotorTypeDefaults {
    pub kp_max: f64,
    pub kd_max: f64,
    pub tau_ff_max_nm: f64,
    pub velocity_max_rad_s: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JointControlEntry {
    pub motor_type: MotorType,
    pub gravity_comp: ModeGains,
    pub impedance: ModeGains,
    pub friction: FrictionGains,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModeGains {
    pub kp: f64,
    pub kd: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrictionGains {
    pub fc: f64,
    pub fv: f64,
    pub fo: f64,
    pub k: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DangerZoneRule {
    pub name: String,
    pub joint: String,
    pub position_above_rad: f64,
    pub velocity_below_rad_s: f64,
    pub action: String,
    pub max_velocity_rad_s: f64,
}

/// Load `config/control.yaml` relative to `repo_root`.
pub fn load_control_config(repo_root: impl AsRef<Path>) -> Result<ControlConfigFile, ConfigError> {
    let path = repo_root.as_ref().join("config/control.yaml");
    read_yaml(&path)
}

/// Load `config/network.yaml` relative to `repo_root`.
pub fn load_network_config(repo_root: impl AsRef<Path>) -> Result<NetworkConfigFile, ConfigError> {
    let path = repo_root.as_ref().join("config/network.yaml");
    read_yaml(&path)
}

/// Ensure every motor entry references a joint declared in `robot.yaml`.
pub fn validate_motors_against_robot(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
) -> Result<(), ConfigError> {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn robot_yaml_parses() {
        let cfg = load_robot_config(repo_root()).expect("robot.yaml");
        assert_eq!(cfg.robot.name, "marengo_arm_4dof");
        assert!(cfg.robot.urdf.contains("arm_4dof"));
        assert!(cfg.robot.bench.max_joint_velocity_rad_s > 0.0);
        assert_eq!(cfg.robot.joints.len(), 4);
        assert!(cfg.robot.urdf.contains("arm_4dof"));
    }

    #[test]
    fn motors_yaml_parses_and_matches_robot() {
        let robot = load_robot_config(repo_root()).expect("robot");
        let motors = load_motors_config(repo_root()).expect("motors");
        assert_eq!(motors.motors.len(), 4);
        validate_motors_against_robot(&robot, &motors).expect("joint names align");
        let m = motor_for_joint(&motors, "shoulder_roll").expect("shoulder_roll");
        assert_eq!(m.device_id, 1);
        assert_eq!(m.motor_type, MotorType::Rs03);
    }

    #[test]
    fn network_yaml_parses() {
        let cfg = load_network_config(repo_root()).expect("network.yaml");
        assert!(cfg.network.chappe_bind.contains(':'));
    }

    #[test]
    fn production_urdf_resolves() {
        let cfg = load_robot_config(repo_root()).expect("robot.yaml");
        let path = resolve_urdf_path(repo_root(), &cfg).expect("arm_4dof.urdf");
        assert!(path.ends_with("arm_4dof.urdf"));
    }

    #[test]
    fn control_yaml_parses() {
        let cfg = load_control_config(repo_root()).expect("control.yaml");
        assert_eq!(cfg.control.loop_hz, 200);
        assert!(cfg.control.motor_type_defaults.contains_key("rs03"));
    }

    #[test]
    fn humanoid_config_templates_parse_and_align() {
        let root = repo_root();
        let robot_path = root.join("config/robot_humanoid.yaml");
        let motors_path = root.join("config/motors_humanoid.yaml");
        let robot: RobotConfigFile = read_yaml(&robot_path).expect("robot_humanoid.yaml");
        let motors: MotorsConfigFile = read_yaml(&motors_path).expect("motors_humanoid.yaml");
        assert_eq!(robot.robot.name, "marengo_humanoid");
        assert_eq!(robot.robot.joints.len(), 23);
        assert_eq!(motors.motors.len(), 23);
        validate_motors_against_robot(&robot, &motors).expect("humanoid joint names align");
        let knee = motor_for_joint(&motors, "left_knee").expect("left_knee");
        assert_eq!(knee.motor_type, MotorType::Rs04);
        let hip = motor_for_joint(&motors, "left_hip_pitch").expect("left_hip_pitch");
        assert_eq!(hip.motor_type, MotorType::Rs04);
    }
}
