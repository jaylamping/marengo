//! # armee-kinematics — URDF kinematics helpers
//!
//! Parses Marengo URDF models and exposes **kinematic facts** for config and safety. No CAN,
//! no control loop, no dynamics torques.
//!
//! ## Responsibilities
//!
//! - [`load_urdf`], [`joint_limits`]: position/velocity/effort limits per joint name.
//! - [`actuated_joint_names`] / [`actuated_joint_count`]: revolute/prismatic joints only.
//! - [`fixtures`]: paths to `arm_4dof.urdf`, `marengo.urdf`, sim fixtures.
//!
//! ## Does not
//!
//! - Compute `tau_g` ([`armee_dynamics`]).
//! - Filter commands or manage enable state (Davout).
//! - Full FK/IK solver for tool poses (planned; keep out of this crate until needed).
//!
//! URDF is the geometric source of truth; keep [`hardware/docs/kinematics.md`](../../hardware/docs/kinematics.md)
//! in sync when joints change.

use std::path::{Path, PathBuf};

use thiserror::Error;
use urdf_rs::{read_file, JointType};

#[derive(Debug, Error)]
pub enum UrdfError {
    #[error("failed to read URDF at {path}: {message}")]
    Read { path: String, message: String },
}

/// Paths to checked-in test fixtures under `sim/fixtures/`.
pub mod fixtures {
    use super::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Minimal 2-DOF URDF used in CI (not the production Marengo model).
    pub fn minimal_urdf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/fixtures/minimal.urdf")
    }

    /// Minimal MJCF paired with [`minimal_urdf`](fn@minimal_urdf) for sim smoke tests.
    pub fn minimal_mjcf() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sim/fixtures/minimal.xml")
    }

    /// Production Marengo URDF (`assets/urdf/marengo.urdf`).
    pub fn production_urdf() -> PathBuf {
        repo_root().join("assets/urdf/marengo.urdf")
    }

    /// 4-DOF arm bring-up URDF (`assets/urdf/arm_4dof.urdf`) — humanoid arm subset.
    pub fn arm_4dof_urdf() -> PathBuf {
        repo_root().join("assets/urdf/arm_4dof.urdf")
    }

    /// Production Marengo MJCF (`assets/mjcf/marengo.xml`).
    pub fn production_mjcf() -> PathBuf {
        repo_root().join("assets/mjcf/marengo.xml")
    }

    /// 4-DOF arm bring-up MJCF (`assets/mjcf/arm_4dof.xml`).
    pub fn arm_4dof_mjcf() -> PathBuf {
        repo_root().join("assets/mjcf/arm_4dof.xml")
    }
}

/// Position limits for a revolute/prismatic joint (radians or meters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointLimits {
    pub lower: f64,
    pub upper: f64,
    pub velocity: f64,
    pub effort: f64,
}

/// Limits for a named joint; errors if joint is missing or not actuated.
pub fn joint_limits(robot: &urdf_rs::Robot, name: &str) -> Result<JointLimits, UrdfError> {
    let joint = robot
        .joints
        .iter()
        .find(|j| j.name == name)
        .ok_or_else(|| UrdfError::Read {
            path: name.to_string(),
            message: "joint not found".to_string(),
        })?;
    if !matches!(
        joint.joint_type,
        JointType::Revolute | JointType::Continuous | JointType::Prismatic
    ) {
        return Err(UrdfError::Read {
            path: name.to_string(),
            message: "joint is not actuated".to_string(),
        });
    }
    Ok(JointLimits {
        lower: joint.limit.lower,
        upper: joint.limit.upper,
        velocity: joint.limit.velocity,
        effort: joint.limit.effort,
    })
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

/// Actuated joint names in URDF document order.
pub fn actuated_joint_names(robot: &urdf_rs::Robot) -> Vec<String> {
    robot
        .joints
        .iter()
        .filter(|j| {
            matches!(
                j.joint_type,
                JointType::Revolute | JointType::Continuous | JointType::Prismatic
            )
        })
        .map(|j| j.name.clone())
        .collect()
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
    fn loads_production_urdf() {
        let path = fixtures::production_urdf();
        let robot = load_urdf(&path).expect("production urdf");
        assert_eq!(actuated_joint_count(&robot), 2);
    }

    #[test]
    fn loads_arm_4dof_urdf() {
        let path = fixtures::arm_4dof_urdf();
        let robot = load_urdf(&path).expect("arm_4dof");
        assert_eq!(actuated_joint_count(&robot), 4);
        let names = actuated_joint_names(&robot);
        assert!(names.contains(&"elbow".to_string()));
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
