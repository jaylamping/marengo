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

use std::collections::HashSet;
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
    #[error("duplicate motor CAN address {interface}:{device_id} in motors.yaml")]
    DuplicateMotorAddress { interface: String, device_id: u8 },
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

/// Repository root: `MARENGO_ROOT` env, else compile-time workspace root from this crate.
pub fn resolve_repo_root() -> PathBuf {
    std::env::var("MARENGO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// Config directory: `MARENGO_CONFIG_DIR` or `<repo_root>/config`.
pub fn resolve_config_dir(repo_root: impl AsRef<Path>) -> PathBuf {
    std::env::var("MARENGO_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_root.as_ref().join("config"))
}

/// Load `robot.yaml` from `config_dir`.
pub fn load_robot_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<RobotConfigFile, ConfigError> {
    read_yaml(&config_dir.as_ref().join("robot.yaml"))
}

/// Load `motors.yaml` from `config_dir`.
pub fn load_motors_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<MotorsConfigFile, ConfigError> {
    read_yaml(&config_dir.as_ref().join("motors.yaml"))
}

/// Load `control.yaml` from `config_dir`.
pub fn load_control_config_from(
    config_dir: impl AsRef<Path>,
) -> Result<ControlConfigFile, ConfigError> {
    read_yaml(&config_dir.as_ref().join("control.yaml"))
}

/// Load `config/robot.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_robot_config(repo_root: impl AsRef<Path>) -> Result<RobotConfigFile, ConfigError> {
    load_robot_config_from(resolve_config_dir(repo_root))
}

/// Load `config/motors.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_motors_config(repo_root: impl AsRef<Path>) -> Result<MotorsConfigFile, ConfigError> {
    load_motors_config_from(resolve_config_dir(repo_root))
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
    /// Max rate for ramping position-hold setpoints (`hold-at` / retarget). Default 0.25 rad/s.
    #[serde(default = "default_position_slew_rad_s")]
    pub position_slew_rad_s: f64,
    /// Max rad the ramped MIT setpoint may lead measured `q` (prevents racing ahead at high slew).
    #[serde(default = "default_position_slew_max_lead_rad")]
    pub position_slew_max_lead_rad: f64,
    /// Added to every position-hold target (rad). Bench trim when mechanical zero ≠ encoder zero.
    #[serde(default)]
    pub position_hold_trim_rad: f64,
}

fn default_position_slew_rad_s() -> f64 {
    0.25
}

fn default_position_slew_max_lead_rad() -> f64 {
    0.15
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

/// Load `config/control.yaml` relative to `repo_root` (honours `MARENGO_CONFIG_DIR`).
pub fn load_control_config(repo_root: impl AsRef<Path>) -> Result<ControlConfigFile, ConfigError> {
    load_control_config_from(resolve_config_dir(repo_root))
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
    let mut addresses = HashSet::new();
    for m in &motors.motors {
        if !robot.robot.joints.iter().any(|j| j == &m.joint) {
            return Err(ConfigError::UnknownMotorJoint {
                joint: m.joint.clone(),
            });
        }
        if !addresses.insert((m.can_interface.clone(), m.device_id)) {
            return Err(ConfigError::DuplicateMotorAddress {
                interface: m.can_interface.clone(),
                device_id: m.device_id,
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
        resolve_repo_root()
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
        assert_eq!(m.device_id, 11);
        assert_eq!(m.motor_type, MotorType::Rs03);
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
    fn arm_4dof_left_bringup_config_validates() {
        let root = repo_root();
        let config_dir = root.join("config/bringup/arm_4dof_left");
        let robot = load_robot_config_from(&config_dir).expect("arm robot.yaml");
        let motors = load_motors_config_from(&config_dir).expect("arm motors.yaml");
        validate_motors_against_robot(&robot, &motors).expect("arm joints align");
        assert_eq!(
            motor_for_joint(&motors, "shoulder_pitch")
                .expect("pitch")
                .device_id,
            12
        );
    }

    #[test]
    fn shoulder_pitch_dual_bringup_config_validates() {
        let root = repo_root();
        let config_dir = root.join("config/bringup/shoulder_pitch_dual");
        let robot = load_robot_config_from(&config_dir).expect("bringup robot.yaml");
        let motors = load_motors_config_from(&config_dir).expect("bringup motors.yaml");
        assert_eq!(motors.motors.len(), 2);
        validate_motors_against_robot(&robot, &motors).expect("bringup joints align");
        let left = motor_for_joint(&motors, "left_shoulder_pitch").expect("left");
        let right = motor_for_joint(&motors, "right_shoulder_pitch").expect("right");
        assert_eq!(left.device_id, 12);
        assert_eq!(left.can_interface, "can1");
        assert_eq!(right.device_id, 2);
        assert_eq!(right.can_interface, "can0");
        let urdf = resolve_urdf_path(&root, &robot).expect("bringup urdf");
        assert!(urdf.ends_with("shoulder_pitch_dual.urdf"));
        load_control_config_from(&config_dir).expect("bringup control.yaml");
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
