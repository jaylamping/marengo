//! Headless simulation harness for Marengo integration tests.
//!
//! MuJoCo stepping is run via `sim/scripts/smoke_test.py` in CI (`check-sim`).
//! This crate holds Rust-side fixture validation and future in-process bindings.

use std::path::{Path, PathBuf};

use armee_kinematics::fixtures;

/// Default MJCF path for CI fixture tests.
pub fn default_model_path() -> PathBuf {
    fixtures::minimal_mjcf()
}

/// Production MJCF path (`assets/mjcf/marengo.xml`).
pub fn production_model_path() -> PathBuf {
    fixtures::production_mjcf()
}

/// 4-DOF arm bring-up MJCF (`assets/mjcf/arm_4dof.xml`).
pub fn arm_4dof_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mjcf/arm_4dof.xml")
}

/// Right 4-DOF bench MJCF (`assets/mjcf/arm_4dof_right.xml`).
pub fn arm_4dof_right_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mjcf/arm_4dof_right.xml")
}

/// Returns true if the model file exists (for skip logic in tests).
pub fn model_exists(path: &Path) -> bool {
    path.is_file()
}

/// Count actuated hinge joints in a minimal MJCF (fixture consistency helper).
pub fn count_mjcf_hinge_joints(xml: &str) -> usize {
    xml.matches("type=\"hinge\"").count()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use armee_kinematics::{actuated_joint_count, load_urdf};

    #[test]
    fn default_fixture_exists() {
        let p = default_model_path();
        assert!(model_exists(&p), "missing {p:?} — run from repo root");
    }

    #[test]
    fn urdf_and_mjcf_fixture_dof_match() {
        let robot = load_urdf(fixtures::minimal_urdf()).expect("urdf");
        let urdf_dof = actuated_joint_count(&robot);
        let mjcf = std::fs::read_to_string(fixtures::minimal_mjcf()).expect("mjcf");
        let mjcf_dof = count_mjcf_hinge_joints(&mjcf);
        assert_eq!(
            urdf_dof, mjcf_dof,
            "URDF actuated joints ({urdf_dof}) must match MJCF hinges ({mjcf_dof})"
        );
    }

    #[test]
    fn production_urdf_and_mjcf_dof_match() {
        let urdf_path = fixtures::production_urdf();
        let mjcf_path = production_model_path();
        assert!(model_exists(&urdf_path), "missing {urdf_path:?}");
        assert!(model_exists(&mjcf_path), "missing {mjcf_path:?}");
        let robot = load_urdf(&urdf_path).expect("production urdf");
        let urdf_dof = actuated_joint_count(&robot);
        let mjcf = std::fs::read_to_string(&mjcf_path).expect("production mjcf");
        let mjcf_dof = count_mjcf_hinge_joints(&mjcf);
        assert_eq!(urdf_dof, mjcf_dof, "production URDF/MJCF DOF mismatch");
    }

    #[test]
    fn arm_4dof_urdf_and_mjcf_dof_match() {
        let urdf_path = fixtures::arm_4dof_urdf();
        let mjcf_path = arm_4dof_model_path();
        assert!(model_exists(&urdf_path), "missing {urdf_path:?}");
        assert!(model_exists(&mjcf_path), "missing {mjcf_path:?}");
        let robot = load_urdf(&urdf_path).expect("arm_4dof urdf");
        let urdf_dof = actuated_joint_count(&robot);
        let mjcf = std::fs::read_to_string(&mjcf_path).expect("arm_4dof mjcf");
        let mjcf_dof = count_mjcf_hinge_joints(&mjcf);
        assert_eq!(urdf_dof, mjcf_dof, "arm_4dof URDF/MJCF DOF mismatch");
    }

    #[test]
    fn arm_4dof_right_urdf_and_mjcf_dof_and_elbow_axis_match() {
        let urdf_path = fixtures::arm_4dof_right_urdf();
        let mjcf_path = arm_4dof_right_model_path();
        assert!(model_exists(&urdf_path), "missing {urdf_path:?}");
        assert!(model_exists(&mjcf_path), "missing {mjcf_path:?}");
        let robot = load_urdf(&urdf_path).expect("arm_4dof_right urdf");
        let urdf_dof = actuated_joint_count(&robot);
        let mjcf = std::fs::read_to_string(&mjcf_path).expect("arm_4dof_right mjcf");
        let mjcf_dof = count_mjcf_hinge_joints(&mjcf);
        assert_eq!(urdf_dof, mjcf_dof, "arm_4dof_right URDF/MJCF DOF mismatch");
        assert!(
            mjcf.contains("name=\"right_elbow_pitch\"") && mjcf.contains("axis=\"0 1 0\""),
            "MJCF elbow must be right_elbow_pitch on +Y"
        );
        let elbow = robot
            .joints
            .iter()
            .find(|j| j.name == "right_elbow_pitch")
            .expect("elbow");
        assert!((elbow.axis.xyz[1] - 1.0).abs() < 1e-9);
    }
}
