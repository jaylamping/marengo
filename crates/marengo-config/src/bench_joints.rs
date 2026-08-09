//! Command-eligible joints from master `robot.yaml`.
//!
//! The allowlist is the loaded profile's `robot.joints` — not a hardcoded left/right
//! bench table. Inventory aliases resolve only when their canonical name is wired.

use std::collections::HashSet;
use std::path::Path;

use crate::{
    load_robot_config_from, resolve_config_dir, resolve_repo_root, ConfigError, RobotConfigFile,
};

/// Joints that may appear in actuator harness commands for the active profile.
#[derive(Debug, Clone, Default)]
pub struct CommandJointAllowlist {
    joints: HashSet<String>,
}

impl CommandJointAllowlist {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_joints<I, S>(joints: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            joints: joints.into_iter().map(Into::into).collect(),
        }
    }

    pub fn from_robot(robot: &RobotConfigFile) -> Self {
        Self::from_joints(robot.robot.joints.iter().cloned())
    }

    pub fn contains(&self, joint: &str) -> bool {
        self.joints.contains(joint)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.joints.iter().map(String::as_str)
    }
}

/// Load command allowlist from `MARENGO_CONFIG_DIR` (or master `<repo>/config`).
pub fn load_command_joint_allowlist() -> Result<CommandJointAllowlist, ConfigError> {
    load_command_joint_allowlist_from(resolve_config_dir(resolve_repo_root()))
}

/// Load command allowlist from an explicit config directory.
pub fn load_command_joint_allowlist_from(
    config_dir: impl AsRef<Path>,
) -> Result<CommandJointAllowlist, ConfigError> {
    let robot = load_robot_config_from(config_dir)?;
    Ok(CommandJointAllowlist::from_robot(&robot))
}

/// Resolve an operator/inventory joint name to a wired canonical name.
pub fn resolve_command_joint<'a>(
    input: &str,
    allowlist: &'a CommandJointAllowlist,
) -> Option<&'a str> {
    for candidate in joint_lookup_candidates(input) {
        if let Some(canonical) = allowlist.joints.get(candidate) {
            return Some(canonical.as_str());
        }
    }
    None
}

fn joint_lookup_candidates(input: &str) -> Vec<&str> {
    let mut out = Vec::with_capacity(3);
    out.push(input);
    match input {
        "left_shoulder_roll" => out.push("shoulder_roll"),
        "left_shoulder_pitch" => out.push("shoulder_pitch"),
        "left_upper_arm_yaw" => out.push("upper_arm_yaw"),
        "left_elbow" => out.push("elbow"),
        "shoulder_roll" => out.push("left_shoulder_roll"),
        "shoulder_pitch" => out.push("left_shoulder_pitch"),
        "upper_arm_yaw" => out.push("left_upper_arm_yaw"),
        "elbow" => out.push("left_elbow"),
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::{resolve_config_dir, resolve_repo_root, resolve_urdf_path};

    #[test]
    fn master_config_accepts_right_four_dof_joints() {
        let root = resolve_repo_root();
        let allowlist = load_command_joint_allowlist_from(resolve_config_dir(&root))
            .expect("master allowlist");
        assert_eq!(
            resolve_command_joint("right_shoulder_pitch", &allowlist),
            Some("right_shoulder_pitch")
        );
        assert_eq!(
            resolve_command_joint("right_shoulder_roll", &allowlist),
            Some("right_shoulder_roll")
        );
        assert_eq!(
            resolve_command_joint("right_upper_arm_yaw", &allowlist),
            Some("right_upper_arm_yaw")
        );
        assert_eq!(
            resolve_command_joint("right_elbow_pitch", &allowlist),
            Some("right_elbow_pitch")
        );
        assert_eq!(
            resolve_command_joint("left_shoulder_pitch", &allowlist),
            None
        );
        assert_eq!(resolve_command_joint("elbow", &allowlist), None);
    }

    #[test]
    fn master_boot_resolution_uses_repo_config_when_pi_path_missing() {
        let root = resolve_repo_root();
        let config_dir = resolve_config_dir(&root);
        assert!(config_dir.ends_with("config"));
        let robot = load_robot_config_from(&config_dir).expect("robot");
        assert_eq!(robot.robot.urdf, "assets/urdf/marengo.urdf");
        let path = resolve_urdf_path(&root, &robot).expect("urdf");
        assert!(path.ends_with("marengo.urdf"));
    }

    #[test]
    fn rejects_unwired_inventory() {
        let allowlist = CommandJointAllowlist::from_joints(["right_shoulder_pitch"]);
        assert_eq!(resolve_command_joint("left_wrist_pitch", &allowlist), None);
        assert_eq!(resolve_command_joint("left_knee", &allowlist), None);
    }
}
