//! Command-eligible joints from master `robot.yaml`.
//!
//! The allowlist is the loaded profile's `robot.joints` — not a hardcoded left/right
//! bench table. Inventory aliases resolve only when their canonical name is wired.

use std::collections::HashSet;
use std::env;
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

    /// Restrict to joints present in `subset` (ephemeral harness filter).
    pub fn intersect_subset(&self, subset: &HashSet<String>) -> Self {
        Self {
            joints: self
                .joints
                .iter()
                .filter(|j| subset.contains(*j))
                .cloned()
                .collect(),
        }
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
    let mut allowlist = CommandJointAllowlist::from_robot(&robot);
    if let Some(subset) = joint_subset_from_env() {
        allowlist = allowlist.intersect_subset(&subset);
    }
    Ok(allowlist)
}

/// Optional runtime joint filter (`MARENGO_JOINT_SUBSET=joint1,joint2`).
pub fn joint_subset_from_env() -> Option<HashSet<String>> {
    let raw = env::var("MARENGO_JOINT_SUBSET").ok()?;
    let joints: HashSet<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if joints.is_empty() {
        None
    } else {
        Some(joints)
    }
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
        "elbow" => {
            out.push("right_elbow_pitch");
            out.push("left_elbow");
        }
        "right_elbow_pitch" => out.push("elbow"),
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
        assert_eq!(
            resolve_command_joint("elbow", &allowlist),
            Some("right_elbow_pitch")
        );
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

    #[test]
    fn joint_subset_filter_narrows_allowlist() {
        let full = CommandJointAllowlist::from_joints([
            "right_shoulder_roll",
            "right_shoulder_pitch",
            "right_upper_arm_yaw",
            "right_elbow_pitch",
        ]);
        let subset: HashSet<String> = [
            "right_shoulder_roll".to_string(),
            "right_shoulder_pitch".to_string(),
        ]
        .into_iter()
        .collect();
        let filtered = full.intersect_subset(&subset);
        assert!(filtered.contains("right_shoulder_roll"));
        assert!(filtered.contains("right_shoulder_pitch"));
        assert!(!filtered.contains("right_upper_arm_yaw"));
        assert!(!filtered.contains("right_elbow_pitch"));
    }

    #[test]
    fn elbow_alias_resolves_right_bench_joint() {
        let allowlist = CommandJointAllowlist::from_joints(["right_elbow_pitch"]);
        assert_eq!(
            resolve_command_joint("elbow", &allowlist),
            Some("right_elbow_pitch")
        );
    }
}
