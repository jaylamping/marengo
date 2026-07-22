//! Validated, compare-and-swap transactions for bringup profile YAML.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    apply_limit_patch_to_control, apply_limit_patch_to_motor, ensure_soft_inset,
    load_control_config_from, load_homing_config_from, load_motors_config_from,
    load_robot_config_from, profile_content_revision, validate_control_against_limits,
    validate_control_config, validate_limit_patch, validate_motors_against_robot,
    validate_robot_control_joint_coverage, write_motors_control_and_urdf, ConfigError,
    ControlConfigFile, HomingConfigFile, LimitPatch,
    MotorsConfigFile, RobotConfigFile, BRINGUP_PROFILE_SLUGS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertLimitResult {
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddJointResult {
    pub joint: String,
    pub revision: String,
}

/// Atomic motors.yaml + control.yaml validate-then-commit (limits / overlay write-behind).
pub fn write_motors_and_control(
    config_dir: impl AsRef<Path>,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
) -> Result<(), ConfigError> {
    let config_dir = config_dir.as_ref();
    let robot = load_robot_config_from(config_dir)?;
    let homing = load_homing_config_from(config_dir)?;
    validate_profile(&robot, motors, control)?;
    write_profile(config_dir, &robot, motors, control, &homing)
}

pub fn upsert_joint_limits(
    repo_root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
    patch: &LimitPatch,
    expected_revision: Option<&str>,
) -> Result<UpsertLimitResult, ConfigError> {
    let repo_root = repo_root.as_ref();
    let config_dir = config_dir.as_ref();
    check_revision(config_dir, expected_revision)?;
    validate_limit_patch(patch)?;
    let mut patch = patch.clone();
    ensure_soft_inset(&mut patch);

    let robot = load_robot_config_from(config_dir)?;
    let mut motors = load_motors_config_from(config_dir)?;
    let mut control = load_control_config_from(config_dir)?;

    let motor = motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == patch.joint)
        .ok_or_else(|| {
            transaction_error(
                config_dir,
                format!("joint {} not in motors.yaml", patch.joint),
            )
        })?;
    apply_limit_patch_to_motor(motor, &patch)?;
    let control_entry = control
        .control
        .joints
        .get_mut(&patch.joint)
        .ok_or_else(|| {
            transaction_error(
                config_dir,
                format!("joint {} not in control.yaml", patch.joint),
            )
        })?;
    apply_limit_patch_to_control(control_entry, &patch)?;

    validate_profile(&robot, &motors, &control)?;
    // Single atomic path shared with Pi write-behind and local git sync.
    write_motors_control_and_urdf(repo_root, config_dir, &motors, &control)?;

    Ok(UpsertLimitResult {
        revision: profile_content_revision(config_dir)?,
    })
}

pub fn add_joint_from_source(
    repo_root: impl AsRef<Path>,
    target_dir: impl AsRef<Path>,
    source_dir: impl AsRef<Path>,
    joint: &str,
    expected_revision: Option<&str>,
) -> Result<AddJointResult, ConfigError> {
    let repo_root = repo_root.as_ref();
    let target_dir = target_dir.as_ref();
    let source_dir = source_dir.as_ref();
    check_revision(target_dir, expected_revision)?;
    if joint.trim().is_empty() {
        return Err(transaction_error(target_dir, "joint must not be empty"));
    }
    if !joint_in_profile_urdf(repo_root, target_dir, joint)? {
        return Err(transaction_error(
            target_dir,
            format!("joint {joint} is not declared in the target URDF"),
        ));
    }

    let mut target_robot = load_robot_config_from(target_dir)?;
    let mut target_motors = load_motors_config_from(target_dir)?;
    let mut target_control = load_control_config_from(target_dir)?;
    let mut target_homing = load_homing_config_from(target_dir)?;
    if target_robot.robot.joints.iter().any(|entry| entry == joint)
        || target_motors
            .motors
            .iter()
            .any(|motor| motor.joint == joint)
        || target_control.control.joints.contains_key(joint)
        || target_homing.homing.joints.contains_key(joint)
    {
        return Err(transaction_error(
            target_dir,
            format!("joint {joint} already exists in the target profile"),
        ));
    }

    let source_motors = load_motors_config_from(source_dir)?;
    let source_control = load_control_config_from(source_dir)?;
    let source_homing = load_homing_config_from(source_dir)?;
    let source_motor = source_motors
        .motors
        .iter()
        .find(|motor| motor.joint == joint)
        .ok_or_else(|| transaction_error(source_dir, format!("joint {joint} not in motors.yaml")))?
        .clone();
    let source_control_entry = source_control
        .control
        .joints
        .get(joint)
        .ok_or_else(|| transaction_error(source_dir, format!("joint {joint} not in control.yaml")))?
        .clone();
    let source_homing_entry = source_homing
        .homing
        .joints
        .get(joint)
        .ok_or_else(|| transaction_error(source_dir, format!("joint {joint} not in homing.yaml")))?
        .clone();

    target_robot.robot.joints.push(joint.to_string());
    target_motors.motors.push(source_motor);
    target_control
        .control
        .joints
        .insert(joint.to_string(), source_control_entry);
    target_homing
        .homing
        .joints
        .insert(joint.to_string(), source_homing_entry);

    validate_profile(&target_robot, &target_motors, &target_control)?;
    write_profile(
        target_dir,
        &target_robot,
        &target_motors,
        &target_control,
        &target_homing,
    )?;

    Ok(AddJointResult {
        joint: joint.to_string(),
        revision: profile_content_revision(target_dir)?,
    })
}

pub fn joint_in_motors(config_dir: impl AsRef<Path>, joint: &str) -> Result<bool, ConfigError> {
    let motors = load_motors_config_from(config_dir)?;
    Ok(motors.motors.iter().any(|motor| motor.joint == joint))
}

pub fn joint_in_profile_urdf(
    repo_root: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
    joint: &str,
) -> Result<bool, ConfigError> {
    let robot = load_robot_config_from(config_dir)?;
    let urdf_path = repo_root.as_ref().join(&robot.robot.urdf);
    let urdf = fs::read_to_string(&urdf_path).map_err(|error| ConfigError::Io {
        path: urdf_path,
        message: error.to_string(),
    })?;
    Ok(urdf.contains(&format!("name=\"{joint}\"")))
}

pub fn limit_patch_from_motor(
    config_dir: impl AsRef<Path>,
    joint: &str,
) -> Result<LimitPatch, ConfigError> {
    let config_dir = config_dir.as_ref();
    let motors = load_motors_config_from(config_dir)?;
    let control = load_control_config_from(config_dir)?;
    let motor = motors
        .motors
        .iter()
        .find(|motor| motor.joint == joint)
        .ok_or_else(|| {
            transaction_error(config_dir, format!("joint {joint} not in motors.yaml"))
        })?;
    let control_entry = control.control.joints.get(joint).ok_or_else(|| {
        transaction_error(config_dir, format!("joint {joint} not in control.yaml"))
    })?;
    Ok(LimitPatch {
        joint: joint.to_string(),
        position_lower_rad: motor.bench.position_lower_rad,
        position_upper_rad: motor.bench.position_upper_rad,
        torque_limit_nm: Some(motor.bench.torque_limit_nm),
        position_soft_lower_rad: control_entry.position_soft_lower_rad,
        position_soft_upper_rad: control_entry.position_soft_upper_rad,
        velocity_max_rad_s: control_entry.velocity_max_rad_s,
    })
}

pub fn membership_slugs_for_joint(
    repo_root: impl AsRef<Path>,
    joint: &str,
) -> Result<Vec<String>, ConfigError> {
    let bringup_root = repo_root.as_ref().join("config/bringup");
    let mut memberships = Vec::new();
    for slug in BRINGUP_PROFILE_SLUGS {
        let profile = bringup_root.join(slug);
        if joint_in_motors(&profile, joint)? {
            memberships.push((*slug).to_string());
        }
    }
    Ok(memberships)
}

fn check_revision(config_dir: &Path, expected_revision: Option<&str>) -> Result<(), ConfigError> {
    let Some(expected) = expected_revision else {
        return Ok(());
    };
    let actual = profile_content_revision(config_dir)?;
    if actual != expected {
        return Err(transaction_error(
            config_dir,
            format!("profile revision mismatch: expected {expected}, found {actual}"),
        ));
    }
    Ok(())
}

fn validate_profile(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
) -> Result<(), ConfigError> {
    validate_control_config(control)?;
    validate_motors_against_robot(robot, motors)?;
    validate_robot_control_joint_coverage(robot, control)?;
    validate_control_against_limits(robot, motors, control)
}

fn write_profile(
    config_dir: &Path,
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
    homing: &HomingConfigFile,
) -> Result<(), ConfigError> {
    let documents = [
        serialize_yaml(config_dir, "robot.yaml", robot)?,
        serialize_yaml(config_dir, "motors.yaml", motors)?,
        serialize_yaml(config_dir, "control.yaml", control)?,
        serialize_yaml(config_dir, "homing.yaml", homing)?,
    ];

    let mut temporary_paths = Vec::with_capacity(documents.len());
    for (path, text) in &documents {
        let temporary = path.with_extension("yaml.tmp");
        if let Err(error) = fs::write(&temporary, text) {
            remove_temporary_files(&temporary_paths);
            return Err(ConfigError::Io {
                path: temporary,
                message: error.to_string(),
            });
        }
        temporary_paths.push(temporary);
    }

    for ((path, _), temporary) in documents.iter().zip(&temporary_paths) {
        if let Err(error) = fs::rename(temporary, path) {
            remove_temporary_files(&temporary_paths);
            return Err(ConfigError::Io {
                path: path.clone(),
                message: error.to_string(),
            });
        }
    }
    Ok(())
}

fn serialize_yaml<T: Serialize>(
    config_dir: &Path,
    name: &str,
    value: &T,
) -> Result<(PathBuf, String), ConfigError> {
    let path = config_dir.join(name);
    let text = serde_yaml::to_string(value).map_err(|error| ConfigError::Parse {
        path: path.clone(),
        message: error.to_string(),
    })?;
    Ok((path, text))
}

fn remove_temporary_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn transaction_error(path: impl AsRef<Path>, message: impl Into<String>) -> ConfigError {
    ConfigError::Parse {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::fs;

    use crate::{
        load_control_config_from, load_homing_config_from, load_motors_config_from,
        load_robot_config_from, profile_content_revision, resolve_repo_root,
    };

    use super::*;

    const PROFILE_FILES: [&str; 4] = ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"];

    fn copy_profile(source: &Path, target: &Path) {
        for name in PROFILE_FILES {
            fs::copy(source.join(name), target.join(name)).expect("copy profile file");
        }
    }

    fn copy_profile_tree(profile: &str) -> (tempfile::TempDir, PathBuf) {
        let root = resolve_repo_root();
        let source = root.join("config/bringup").join(profile);
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config/bringup").join(profile);
        fs::create_dir_all(&config_dir).expect("config dir");
        copy_profile(&source, &config_dir);
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let urdf_rel = PathBuf::from(&robot.robot.urdf);
        let urdf_dest = temp.path().join(&urdf_rel);
        if let Some(parent) = urdf_dest.parent() {
            fs::create_dir_all(parent).expect("urdf dir");
        }
        fs::copy(root.join(&urdf_rel), &urdf_dest).expect("copy urdf");
        (temp, config_dir)
    }

    fn elbow_patch() -> LimitPatch {
        LimitPatch {
            joint: "right_elbow_pitch".to_string(),
            position_lower_rad: -0.1,
            position_upper_rad: 1.3,
            torque_limit_nm: Some(2.0),
            position_soft_lower_rad: Some(-0.2),
            position_soft_upper_rad: Some(1.4),
            velocity_max_rad_s: Some(1.0),
        }
    }

    #[test]
    fn upsert_writes_validated_limits_and_advances_revision() {
        let (temp, config_dir) = copy_profile_tree("arm_4dof_right");
        let before = profile_content_revision(&config_dir).expect("revision");

        let result = upsert_joint_limits(temp.path(), &config_dir, &elbow_patch(), Some(&before))
            .expect("upsert");

        assert_ne!(result.revision, before);
        let motors = load_motors_config_from(&config_dir).expect("motors");
        let elbow = motors
            .motors
            .iter()
            .find(|motor| motor.joint == "right_elbow_pitch")
            .expect("elbow motor");
        assert_eq!(elbow.bench.position_upper_rad, 1.3);
        let control = load_control_config_from(&config_dir).expect("control");
        assert_eq!(
            control.control.joints["right_elbow_pitch"].position_soft_upper_rad,
            Some(1.3)
        );
        for name in PROFILE_FILES {
            assert!(!config_dir.join(format!("{name}.tmp")).exists());
        }
    }

    #[test]
    fn upsert_rejects_stale_revision_without_writing() {
        let (temp, config_dir) = copy_profile_tree("arm_4dof_right");
        let before = profile_content_revision(&config_dir).expect("revision");

        assert!(
            upsert_joint_limits(temp.path(), &config_dir, &elbow_patch(), Some("stale")).is_err()
        );

        assert_eq!(
            profile_content_revision(&config_dir).expect("revision after rejection"),
            before
        );
    }

    #[test]
    fn adds_joint_when_target_urdf_declares_it() {
        let root = resolve_repo_root();
        let source = root.join("config/bringup/arm_4dof_right");
        let target_source = root.join("config/bringup/arm_3dof_right");
        let temp = tempfile::tempdir().expect("tempdir");
        copy_profile(&target_source, temp.path());
        let mut robot = load_robot_config_from(temp.path()).expect("robot");
        robot.robot.urdf = "assets/urdf/arm_4dof_right.urdf".to_string();
        fs::write(
            temp.path().join("robot.yaml"),
            serde_yaml::to_string(&robot).expect("serialize robot"),
        )
        .expect("write robot");

        let result = add_joint_from_source(&root, temp.path(), &source, "right_elbow_pitch", None)
            .expect("add elbow");

        assert_eq!(result.joint, "right_elbow_pitch");
        assert!(load_robot_config_from(temp.path())
            .expect("robot")
            .robot
            .joints
            .iter()
            .any(|joint| joint == "right_elbow_pitch"));
        assert!(joint_in_motors(temp.path(), "right_elbow_pitch").expect("membership"));
        assert!(load_control_config_from(temp.path())
            .expect("control")
            .control
            .joints
            .contains_key("right_elbow_pitch"));
        assert!(load_homing_config_from(temp.path())
            .expect("homing")
            .homing
            .joints
            .contains_key("right_elbow_pitch"));
    }

    #[test]
    fn refuses_elbow_when_target_urdf_is_three_dof() {
        let root = resolve_repo_root();
        let source = root.join("config/bringup/arm_4dof_right");
        let target_source = root.join("config/bringup/arm_3dof_right");
        let temp = tempfile::tempdir().expect("tempdir");
        copy_profile(&target_source, temp.path());
        let before = profile_content_revision(temp.path()).expect("revision");

        assert!(
            add_joint_from_source(&root, temp.path(), &source, "right_elbow_pitch", None).is_err()
        );
        assert_eq!(
            profile_content_revision(temp.path()).expect("revision after rejection"),
            before
        );
    }

    #[test]
    fn reports_profile_membership_and_current_limits() {
        let root = resolve_repo_root();
        let profile = root.join("config/bringup/arm_4dof_right");

        assert!(joint_in_motors(&profile, "right_elbow_pitch").expect("motor membership"));
        assert!(joint_in_profile_urdf(&root, &profile, "right_elbow_pitch").expect("URDF"));
        let patch = limit_patch_from_motor(&profile, "right_elbow_pitch").expect("limits");
        // Matches config/bringup/arm_4dof_right/motors.yaml (Davout live hard).
        assert!((patch.position_upper_rad - 1.034701585769653).abs() < 1e-9);
        let slugs = membership_slugs_for_joint(&root, "right_elbow_pitch").expect("slugs");
        assert!(slugs.iter().any(|slug| slug == "arm_4dof_right"));
        assert!(!slugs.iter().any(|slug| slug == "arm_3dof_right"));
    }
}
