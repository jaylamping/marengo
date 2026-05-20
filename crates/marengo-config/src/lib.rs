//! Load [`config/`](../../config/) YAML files with validation.

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

#[derive(Debug, Clone, Deserialize)]
pub struct MotorEntry {
    pub joint: String,
    pub driver: String,
    pub can_interface: String,
    pub device_id: u8,
    pub direction: i8,
    pub firmware_version: String,
    pub bench: MotorBenchLimits,
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
pub fn resolve_urdf_path(repo_root: impl AsRef<Path>, robot: &RobotConfigFile) -> Result<PathBuf, ConfigError> {
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
        assert_eq!(cfg.robot.name, "marengo");
        assert!(cfg.robot.urdf.contains("marengo.urdf"));
        assert!(cfg.robot.bench.max_joint_velocity_rad_s > 0.0);
        assert_eq!(cfg.robot.joints.len(), 2);
    }

    #[test]
    fn motors_yaml_parses_and_matches_robot() {
        let robot = load_robot_config(repo_root()).expect("robot");
        let motors = load_motors_config(repo_root()).expect("motors");
        assert_eq!(motors.motors.len(), 2);
        validate_motors_against_robot(&robot, &motors).expect("joint names align");
        let m = motor_for_joint(&motors, "joint1").expect("joint1");
        assert_eq!(m.device_id, 1);
    }

    #[test]
    fn network_yaml_parses() {
        let cfg = load_network_config(repo_root()).expect("network.yaml");
        assert!(cfg.network.chappe_bind.contains(':'));
    }

    #[test]
    fn production_urdf_resolves() {
        let cfg = load_robot_config(repo_root()).expect("robot.yaml");
        let path = resolve_urdf_path(repo_root(), &cfg).expect("marengo.urdf");
        assert!(path.ends_with("marengo.urdf"));
    }
}
