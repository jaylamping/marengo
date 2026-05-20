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
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotConfigFile {
    pub robot: RobotSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RobotSection {
    pub name: String,
    pub urdf: String,
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

/// Load `config/network.yaml` relative to `repo_root`.
pub fn load_network_config(repo_root: impl AsRef<Path>) -> Result<NetworkConfigFile, ConfigError> {
    let path = repo_root.as_ref().join("config/network.yaml");
    read_yaml(&path)
}

/// Resolve URDF path from robot config; errors if the file is missing.
pub fn resolve_urdf_path(repo_root: impl AsRef<Path>, robot: &RobotConfigFile) -> Result<PathBuf, ConfigError> {
    let path = repo_root.as_ref().join(&robot.robot.urdf);
    if !path.is_file() {
        return Err(ConfigError::UrdfMissing { path });
    }
    Ok(path)
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
    }

    #[test]
    fn network_yaml_parses() {
        let cfg = load_network_config(repo_root()).expect("network.yaml");
        assert!(cfg.network.chappe_bind.contains(':'));
    }

    #[test]
    fn production_urdf_missing_is_reported() {
        let cfg = load_robot_config(repo_root()).expect("robot.yaml");
        assert!(matches!(
            resolve_urdf_path(repo_root(), &cfg),
            Err(ConfigError::UrdfMissing { .. })
        ));
    }
}
