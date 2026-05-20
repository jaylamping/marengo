//! Forward/inverse kinematics and joint limit helpers from `assets/urdf/marengo.urdf`.

#![forbid(unsafe_code)]

use std::path::Path;

use thiserror::Error;
use urdf_rs::read_file;

#[derive(Debug, Error)]
pub enum UrdfError {
    #[error("failed to read URDF at {path}: {message}")]
    Read { path: String, message: String },
}

/// Load and parse a URDF file.
pub fn load_urdf(path: impl AsRef<Path>) -> Result<urdf_rs::Robot, UrdfError> {
    let path = path.as_ref();
    let path_str = path.display().to_string();
    read_file(path).map_err(|e| UrdfError::Read {
        path: path_str,
        message: e.to_string(),
    })
}

/// Number of actuated joints in the robot description.
pub fn joint_count(robot: &urdf_rs::Robot) -> usize {
    robot.joints.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn minimal_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/fixtures/minimal.urdf")
    }

    #[test]
    fn loads_minimal_fixture() {
        let robot = load_urdf(minimal_fixture()).expect("parse");
        assert_eq!(joint_count(&robot), 2);
    }

    #[test]
    fn joint_limits_match_fixture() {
        let robot = load_urdf(minimal_fixture()).expect("parse");
        let j1 = robot
            .joints
            .iter()
            .find(|j| j.name == "joint1")
            .expect("joint1");
        let lower = j1.limit.lower;
        let upper = j1.limit.upper;
        assert!((lower + 1.57).abs() < 1e-6);
        assert!((upper - 1.57).abs() < 1e-6);
    }
}
