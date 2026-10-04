//! Validated, compare-and-swap transactions for master config YAML.

use crate::atomic_file::write_atomic;
use crate::{
    load_control_config_from, load_homing_config_from, load_motors_config_from,
    load_robot_config_from, validate_safety_config, ConfigError, ControlConfigFile, LimitPatch,
    MotorsConfigFile, ProfileWriteLock,
};
#[cfg(test)]
use crate::{HomingConfigFile, RobotConfigFile};
use std::fs;
use std::path::Path;

/// Atomic motors.yaml + control.yaml validate-then-commit (limits / overlay write-behind).
pub fn write_motors_and_control(
    config_dir: impl AsRef<Path>,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
) -> Result<(), ConfigError> {
    let config_dir = config_dir.as_ref();
    let _lock = ProfileWriteLock::acquire(config_dir)?;
    let robot = load_robot_config_from(config_dir)?;
    let homing = load_homing_config_from(config_dir)?;
    validate_safety_config(&robot, motors, control, &homing)?;
    write_motors_control_files(config_dir, motors, control)
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

/// Four-file validate-then-commit (robot + motors + control + homing).
/// No production caller after the bringup-profile upsert removal (B7); kept
/// test-gated as the rollback-path oracle below.
#[cfg(test)]
pub(crate) fn write_profile(
    config_dir: &Path,
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
    homing: &HomingConfigFile,
) -> Result<(), ConfigError> {
    validate_safety_config(robot, motors, control, homing)?;
    let documents = [
        (
            "robot.yaml",
            serialize_yaml(config_dir, "robot.yaml", robot)?,
        ),
        (
            "motors.yaml",
            serialize_yaml(config_dir, "motors.yaml", motors)?,
        ),
        (
            "control.yaml",
            serialize_yaml(config_dir, "control.yaml", control)?,
        ),
        (
            "homing.yaml",
            serialize_yaml(config_dir, "homing.yaml", homing)?,
        ),
    ];
    let files = documents
        .iter()
        .map(|(name, text)| (*name, text.as_bytes()))
        .collect::<Vec<_>>();
    write_files_with_rollback(config_dir, &files)
}

pub(crate) fn write_motors_control_files(
    config_dir: &Path,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
) -> Result<(), ConfigError> {
    let documents = [
        (
            "motors.yaml",
            serialize_yaml(config_dir, "motors.yaml", motors)?,
        ),
        (
            "control.yaml",
            serialize_yaml(config_dir, "control.yaml", control)?,
        ),
    ];
    let files = documents
        .iter()
        .map(|(name, text)| (*name, text.as_bytes()))
        .collect::<Vec<_>>();
    write_files_with_rollback(config_dir, &files)
}

fn serialize_yaml<T: serde::Serialize>(
    config_dir: &Path,
    name: &str,
    value: &T,
) -> Result<String, ConfigError> {
    serde_yaml::to_string(value).map_err(|error| ConfigError::Parse {
        path: config_dir.join(name),
        message: error.to_string(),
    })
}

fn write_files_with_rollback(
    config_dir: &Path,
    files: &[(&str, &[u8])],
) -> Result<(), ConfigError> {
    let mut backups = Vec::with_capacity(files.len());
    for (name, _) in files {
        let path = config_dir.join(name);
        let contents = fs::read(&path).map_err(|error| ConfigError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        backups.push((path, contents));
    }

    for (index, (name, contents)) in files.iter().enumerate() {
        let path = config_dir.join(name);
        if let Err(error) = write_atomic(&path, contents) {
            for (restore_path, restore_bytes) in backups[..index].iter().rev() {
                if let Err(restore_error) = write_atomic(restore_path, restore_bytes) {
                    return Err(ConfigError::Io {
                        path: restore_path.clone(),
                        message: format!(
                            "write failed ({error}); rollback also failed: {restore_error}"
                        ),
                    });
                }
            }
            return Err(error);
        }
    }
    Ok(())
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
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    use crate::{
        load_control_config_from, load_homing_config_from, load_motors_config_from,
        load_robot_config_from, resolve_repo_root,
    };

    use super::*;

    fn profile_txn_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("profile_txn test lock")
    }

    const PROFILE_FILES: [&str; 4] = ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"];

    fn copy_master_config_tree() -> (tempfile::TempDir, PathBuf) {
        let root = resolve_repo_root();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        for name in PROFILE_FILES {
            fs::copy(root.join("config").join(name), config_dir.join(name))
                .expect("copy profile file");
        }
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let urdf_rel = PathBuf::from(&robot.robot.urdf);
        let urdf_dest = temp.path().join(&urdf_rel);
        if let Some(parent) = urdf_dest.parent() {
            fs::create_dir_all(parent).expect("urdf dir");
        }
        fs::copy(root.join(&urdf_rel), &urdf_dest).expect("copy urdf");
        (temp, config_dir)
    }

    #[test]
    fn profile_preflight_failure_does_not_partially_write() {
        let _guard = profile_txn_test_lock();
        let (_temp, config_dir) = copy_master_config_tree();
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let mut motors = load_motors_config_from(&config_dir).expect("motors");
        let control = load_control_config_from(&config_dir).expect("control");
        let homing = load_homing_config_from(&config_dir).expect("homing");
        let old_motors = fs::read(config_dir.join("motors.yaml")).expect("old motors");
        let elbow = motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == "right_elbow_pitch")
            .expect("elbow");
        elbow.bench.position_upper_rad += 0.1;

        let control_path = config_dir.join("control.yaml");
        fs::remove_file(&control_path).expect("remove control file");
        fs::create_dir(&control_path).expect("make rename blocker");
        let marker = control_path.join("marker");
        fs::write(&marker, b"preserved").expect("write marker");

        assert!(write_profile(&config_dir, &robot, &motors, &control, &homing).is_err());

        assert_eq!(
            fs::read(config_dir.join("motors.yaml")).expect("restored motors"),
            old_motors
        );
        assert_eq!(fs::read(marker).expect("preserved marker"), b"preserved");
        assert!(
            fs::read_dir(&config_dir)
                .expect("config entries")
                .all(|entry| !entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")),
            "failed rename must clean up the staged file"
        );
    }
}
