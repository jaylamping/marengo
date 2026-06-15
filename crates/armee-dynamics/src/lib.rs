//! # armee-dynamics — rigid-body dynamics (feedforward only)
//!
//! Computes **gravity compensation torques** `tau_g(q)` for actuated joints. Pure Rust, no
//! CAN, no safety policy. Used by Berthier in gravity-comp and impedance modes.
//!
//! ## Responsibilities
//!
//! - [`DynamicsModel::gravity_torques`]: `q` (rad, joint order from `robot.yaml`) → `tau` (Nm).
//! - [`UrdfGravityModel`]: URDF kinematics + link masses; virtual-work gradient (numerical).
//!
//! ## Does not
//!
//! - Forward/inverse kinematics for arbitrary frames (see [`armee_kinematics`] for limits/FK growth).
//! - Coriolis, mass matrix, or contact dynamics (future extensions need a new ADR).
//! - Send commands or read encoders (Berthier / robstride).
//!
//! ## Accuracy
//!
//! Estimates depend on URDF inertials (CAD export). Cross-check in sim per
//! [ADR 0005](../../docs/decisions/0005-dynamics-library.md). Wrong `tau_g` sign is a safety
//! issue — validate with `motor-repl gravity-preview` before bench enable.

mod urdf_gravity;

use std::path::Path;

use armee_kinematics::UrdfError;
use thiserror::Error;
pub use urdf_gravity::UrdfGravityModel;

#[derive(Debug, Error)]
pub enum DynamicsError {
    #[error("urdf: {0}")]
    Urdf(#[from] UrdfError),
    #[error("joint count mismatch: expected {expected}, got {got}")]
    JointCount { expected: usize, got: usize },
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
}

/// Gravity torques τ_g(q) for actuated joints (Nm).
pub trait DynamicsModel {
    fn joint_names(&self) -> &[String];
    fn gravity_torques(&self, q: &[f64]) -> Result<Vec<f64>, DynamicsError>;
}

/// Load gravity model from URDF and ordered joint names (e.g. from `robot.yaml`).
pub fn gravity_model_from_urdf(
    urdf_path: impl AsRef<Path>,
    joint_names: &[String],
) -> Result<UrdfGravityModel, DynamicsError> {
    UrdfGravityModel::from_urdf(urdf_path, joint_names)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use armee_kinematics::fixtures;

    fn arm_joints() -> Vec<String> {
        vec![
            "shoulder_pitch".to_string(),
            "shoulder_roll".to_string(),
            "upper_arm_yaw".to_string(),
            "elbow".to_string(),
        ]
    }

    #[test]
    fn gravity_torques_length_matches_joints() {
        let path = fixtures::arm_4dof_urdf();
        let model = gravity_model_from_urdf(&path, &arm_joints()).expect("model");
        let q = vec![0.0; 4];
        let tau = model.gravity_torques(&q).expect("tau");
        assert_eq!(tau.len(), 4);
    }

    #[test]
    fn bent_pose_nonzero_gravity() {
        let path = fixtures::arm_4dof_urdf();
        let model = gravity_model_from_urdf(&path, &arm_joints()).expect("model");
        // At q=0 many Z-aligned axes yield d(COM_z)/dq ≈ 0; use a bent pose.
        let q = vec![0.0, 0.3, 0.0, 0.8];
        let tau = model.gravity_torques(&q).expect("tau");
        let sum: f64 = tau.iter().map(|t| t.abs()).sum();
        assert!(sum > 0.1, "expected measurable gravity load, got {tau:?}");
    }

    #[test]
    fn elevated_elbow_changes_elbow_torque() {
        let path = fixtures::arm_4dof_urdf();
        let model = gravity_model_from_urdf(&path, &arm_joints()).expect("model");
        let q_down = vec![0.0, 0.0, 0.0, 0.5];
        let q_up = vec![0.0, 0.8, 0.0, 2.0];
        let tau_down = model.gravity_torques(&q_down).expect("down");
        let tau_up = model.gravity_torques(&q_up).expect("up");
        assert!(
            (tau_up[3] - tau_down[3]).abs() > 0.05,
            "elbow torque should change between poses: down={tau_down:?} up={tau_up:?}",
        );
    }

    fn shoulder_pitch_joints() -> Vec<String> {
        vec![
            "left_shoulder_pitch".to_string(),
            "right_shoulder_pitch".to_string(),
        ]
    }

    #[test]
    fn weighted_bench_right_heavier_at_pitch() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/urdf/shoulder_pitch_weighted.urdf");
        let model =
            gravity_model_from_urdf(&path, &shoulder_pitch_joints()).expect("weighted model");
        let tau = model
            .gravity_torques(&[0.0, 0.3])
            .expect("tau at q_right=0.3");
        assert!(
            tau[1].abs() > tau[0].abs(),
            "loaded right side should see larger |tau_g|: left={} right={}",
            tau[0],
            tau[1],
        );
    }
}
