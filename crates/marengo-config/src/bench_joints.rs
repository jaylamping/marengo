//! Command-eligible joints from master `robot.yaml`.
//!
//! The allowlist is the loaded profile's `robot.joints` — not a hardcoded left/right
//! bench table. Inventory aliases resolve only when their canonical name is wired.

use std::collections::HashSet;
use std::env;
use std::path::Path;

use crate::{
    load_robot_config_from, resolve_config_dir, resolve_repo_root, ConfigError, ControlConfigFile,
    MotorsConfigFile, RobotConfigFile,
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
        validate_joint_subset(&robot, &subset)?;
        allowlist = allowlist.intersect_subset(&subset);
        if allowlist.iter().next().is_none() {
            return Err(ConfigError::EmptyJointSubset);
        }
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

/// Fail closed when any subset name is missing from `robot.joints`.
pub fn validate_joint_subset(
    robot: &RobotConfigFile,
    subset: &HashSet<String>,
) -> Result<(), ConfigError> {
    let known: HashSet<&str> = robot.robot.joints.iter().map(String::as_str).collect();
    for name in subset {
        if !known.contains(name.as_str()) {
            return Err(ConfigError::UnknownJointSubset {
                joint: name.clone(),
            });
        }
    }
    Ok(())
}

/// Narrow robot/motors/control to `MARENGO_JOINT_SUBSET` (preserves robot.joints order).
///
/// Used by Davout boot so Enable/homing/limits match the ephemeral wired set — not only
/// the command allowlist overlay.
pub fn apply_joint_subset(
    robot: &mut RobotConfigFile,
    motors: &mut MotorsConfigFile,
    control: &mut ControlConfigFile,
    subset: &HashSet<String>,
) -> Result<(), ConfigError> {
    validate_joint_subset(robot, subset)?;
    let filtered: Vec<String> = robot
        .robot
        .joints
        .iter()
        .filter(|j| subset.contains(*j))
        .cloned()
        .collect();
    if filtered.is_empty() {
        return Err(ConfigError::EmptyJointSubset);
    }
    let keep: HashSet<String> = filtered.iter().cloned().collect();
    robot.robot.joints = filtered;
    motors.motors.retain(|m| keep.contains(&m.joint));
    if motors.motors.is_empty() {
        return Err(ConfigError::EmptyJointSubset);
    }
    control.control.joints.retain(|name, _| keep.contains(name));
    control.control.actuator_groups.retain(|_, group| {
        group.joints.retain(|j| keep.contains(j));
        !group.joints.is_empty()
    });
    control
        .control
        .danger_zones
        .retain(|zone| keep.contains(&zone.joint));
    Ok(())
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
    fn master_config_accepts_right_five_dof_joints() {
        let root = resolve_repo_root();
        let allowlist =
            load_command_joint_allowlist_from(resolve_config_dir(&root)).expect("master allowlist");
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
            resolve_command_joint("right_lower_arm_yaw", &allowlist),
            Some("right_lower_arm_yaw")
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

    #[test]
    fn validate_joint_subset_rejects_unknown_names() {
        let robot = load_robot_config_from(resolve_config_dir(resolve_repo_root())).expect("robot");
        let subset: HashSet<String> = ["not_a_joint".to_string()].into_iter().collect();
        let err = validate_joint_subset(&robot, &subset).expect_err("unknown");
        assert!(matches!(err, ConfigError::UnknownJointSubset { .. }));
    }

    #[test]
    fn apply_joint_subset_filters_motors_and_preserves_order() {
        use crate::{load_control_config_from, load_motors_config_from};

        let config_dir = resolve_config_dir(resolve_repo_root());
        let mut robot = load_robot_config_from(&config_dir).expect("robot");
        let mut motors = load_motors_config_from(&config_dir).expect("motors");
        let mut control = load_control_config_from(&config_dir).expect("control");
        let subset: HashSet<String> = [
            "right_shoulder_roll".to_string(),
            "right_shoulder_pitch".to_string(),
            "right_upper_arm_yaw".to_string(),
        ]
        .into_iter()
        .collect();
        apply_joint_subset(&mut robot, &mut motors, &mut control, &subset).expect("subset");
        assert_eq!(
            robot.robot.joints,
            vec![
                "right_shoulder_roll".to_string(),
                "right_shoulder_pitch".to_string(),
                "right_upper_arm_yaw".to_string(),
            ]
        );
        assert_eq!(motors.motors.len(), 3);
        assert!(motors.motors.iter().all(|m| m.joint != "right_elbow_pitch"));
        assert!(!control.control.joints.contains_key("right_elbow_pitch"));
    }
}
