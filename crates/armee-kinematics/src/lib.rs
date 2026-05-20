//! Forward/inverse kinematics and joint limit helpers from `assets/urdf/marengo.urdf`.

use std::path::{Path, PathBuf};

use thiserror::Error;
use urdf_rs::{JointType, read_file};

#[derive(Debug, Error)]
pub enum UrdfError {
    #[error("failed to read URDF at {path}: {message}")]
    Read { path: String, message: String },
}

/// Paths to checked-in test fixtures under `sim/fixtures/`.
pub mod fixtures {
    use super::PathBuf;

    /// Minimal 2-DOF URDF used in CI (not the production Marengo model).
    pub fn minimal_urdf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/fixtures/minimal.urdf")
    }

    /// Minimal MJCF paired with [`minimal_urdf`](fn@minimal_urdf) for sim smoke tests.
    pub fn minimal_mjcf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/fixtures/minimal.xml")
    }
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

/// All joint entries in the URDF (includes fixed, mimic, etc.).
pub fn joint_entry_count(robot: &urdf_rs::Robot) -> usize {
    robot.joints.len()
}

/// Joints that can be commanded (revolute, continuous, prismatic).
pub fn actuated_joint_count(robot: &urdf_rs::Robot) -> usize {
    robot
        .joints
        .iter()
        .filter(|j| {
            matches!(
                j.joint_type,
                JointType::Revolute | JointType::Continuous | JointType::Prismatic
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn loads_minimal_fixture() {
        let robot = load_urdf(fixtures::minimal_urdf()).expect("parse");
        assert_eq!(joint_entry_count(&robot), 2);
        assert_eq!(actuated_joint_count(&robot), 2);
    }

    #[test]
    fn joint_limits_match_fixture() {
        let robot = load_urdf(fixtures::minimal_urdf()).expect("parse");
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
