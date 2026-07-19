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
//!
//! ## Sign Convention
//!
//! `gravity_torques(q)` returns **joint-space holding torque** τ_g(q) in Nm — the torque
//! required to hold the arm against gravity at pose q.
//!
//! - **Positive τ_g** means the motor must produce **positive joint torque** to counteract
//!   gravity. For a shoulder pitch joint at q=0.3 rad (arm partially raised), τ_g is positive
//!   because gravity pulls the arm down and the motor must push up.
//! - **Gravity vector**: `[0, 0, -9.81]` (Z-down, standard URDF convention).
//! - **Computation**: τ_g = ∂P/∂q where P = -Σ(mᵢ · g · COMᵢ(q)) is the potential energy.
//!   This is the virtual-work gradient (numerical central difference, DQ_EPS = 1e-6).
//!
//! ### Motor-space transform
//!
//! τ_g is in **joint space**. The motor-space torque applied to the wire is:
//!
//! ```text
//! τ_motor = τ_g / (direction · gear_ratio)
//! ```
//!
//! This transform is applied by **Davout** `send_mit_joint` (not here). For the right
//! shoulder pitch bench motor (`direction: -1`, `gear_ratio: 1.0`), a positive joint-space
//! τ_g becomes a negative motor-space torque.
//!
//! ### PureGravityTorque contract
//!
//! The [`PureGravityTorque`] newtype enforces that `gravity_torques()` returns **only**
//! the gravity component — no friction, no payload estimation, no velocity coupling.
//! This prevents future "enhancements" from silently folding non-gravity terms into
//! `gravity_torques()`, which would corrupt Impedance and Position modes (both add
//! their own friction/damping terms on top of τ_g).
//!
//! ### Safety
//!
//! Wrong τ_g sign is a **safety issue** — the motor accelerates the arm in the direction
//! of gravity instead of holding it. Validate with `motor-repl gravity-preview` before
//! bench enable, and rely on the Davout wrong-sign watchdog for runtime detection.

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

/// Joint-space gravity holding torque τ_g(q) in Nm.
///
/// **Pure gravity only** — no friction, no payload estimation, no velocity coupling.
/// This newtype enforces at the type level that `gravity_torques()` returns only
/// the gravity component. Motor-space transform: τ_motor = τ_g / (direction·gear_ratio),
/// applied by Davout.
///
/// Sign convention: positive τ_g means the motor must produce positive joint torque
/// to hold the arm against gravity at pose q. Wrong sign is a safety issue — validate
/// with `motor-repl gravity-preview` before bench enable.
#[derive(Debug, Clone, PartialEq)]
pub struct PureGravityTorque(pub Vec<f64>);

impl PureGravityTorque {
    /// Access the torque values as a slice.
    pub fn as_slice(&self) -> &[f64] {
        &self.0
    }

    /// Consume the newtype and return the inner Vec.
    pub fn into_inner(self) -> Vec<f64> {
        self.0
    }
}

impl std::ops::Index<usize> for PureGravityTorque {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        &self.0[i]
    }
}

impl std::ops::Deref for PureGravityTorque {
    type Target = [f64];
    fn deref(&self) -> &[f64] {
        &self.0
    }
}

/// Gravity torques τ_g(q) for actuated joints (Nm).
pub trait DynamicsModel {
    fn joint_names(&self) -> &[String];
    fn gravity_torques(&self, q: &[f64]) -> Result<PureGravityTorque, DynamicsError>;
}

/// Load gravity model from URDF and ordered joint names (e.g. from `robot.yaml`).
pub fn gravity_model_from_urdf(
    urdf_path: impl AsRef<Path>,
    joint_names: &[String],
) -> Result<UrdfGravityModel, DynamicsError> {
    UrdfGravityModel::from_urdf(urdf_path, joint_names)
}

/// Compute the maximum `|tau_g|` for a joint over a range of angles.
///
/// Samples `tau_g` at `steps` evenly-spaced points across `[q_min, q_max]` for the
/// joint at `joint_index` (other joints held at 0), and returns the maximum
/// absolute value. `steps` is clamped to a minimum of 2 (endpoints only).
///
/// Use this before enabling motors to verify gravity comp won't saturate the drive.
/// The range should come from the joint hard limits (`motors.yaml` bench bounds);
/// do not pass an unbounded range.
pub fn max_gravity_torque_over_range(
    model: &dyn DynamicsModel,
    joint_index: usize,
    q_min: f64,
    q_max: f64,
    steps: usize,
) -> Result<f64, DynamicsError> {
    let n = model.joint_names().len();
    if joint_index >= n {
        return Err(DynamicsError::JointCount {
            expected: n,
            got: joint_index + 1,
        });
    }
    let steps = steps.max(2);
    let mut max_tau = 0.0f64;
    let mut q_vec = vec![0.0; n];
    for i in 0..steps {
        let t = i as f64 / (steps - 1) as f64;
        q_vec[joint_index] = q_min + (q_max - q_min) * t;
        let tau = model.gravity_torques(&q_vec)?;
        if let Some(&v) = tau.as_slice().get(joint_index) {
            max_tau = max_tau.max(v.abs());
        }
    }
    Ok(max_tau)
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

    fn arm_3dof_right_model() -> UrdfGravityModel {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/urdf/arm_3dof_right.urdf");
        gravity_model_from_urdf(&path, &["right_shoulder_pitch".to_string()])
            .expect("arm_3dof_right model")
    }

    #[test]
    fn saturation_check_returns_max_over_range() {
        let model = arm_3dof_right_model();
        // Gravity torque grows with |q| up to π/2 for this single-pitch model.
        let tau_at_pi2 = model
            .gravity_torques(&[std::f64::consts::FRAC_PI_2])
            .expect("tau at π/2")[0]
            .abs();
        let max_tau =
            max_gravity_torque_over_range(&model, 0, 0.0, std::f64::consts::FRAC_PI_2, 20)
                .expect("max over range");
        assert!(
            (max_tau - tau_at_pi2).abs() < 1e-6,
            "max over [0, π/2] should equal |tau_g(π/2)|={tau_at_pi2}, got {max_tau}",
        );
        assert!(
            max_tau > 0.1,
            "expected non-trivial gravity load, got {max_tau}"
        );
    }

    #[test]
    fn saturation_check_with_zero_range() {
        let model = arm_3dof_right_model();
        let q = 0.3_f64;
        let tau_at_q = model.gravity_torques(&[q]).expect("tau at q")[0].abs();
        let max_tau = max_gravity_torque_over_range(&model, 0, q, q, 5).expect("max zero range");
        assert!(
            (max_tau - tau_at_q).abs() < 1e-6,
            "zero-width range should return |tau_g(q)|={tau_at_q}, got {max_tau}",
        );
    }

    #[test]
    fn saturation_check_with_min_steps() {
        let model = arm_3dof_right_model();
        let tau_at_end = model
            .gravity_torques(&[std::f64::consts::FRAC_PI_2])
            .expect("tau at π/2")[0]
            .abs();
        let max_tau = max_gravity_torque_over_range(&model, 0, 0.0, std::f64::consts::FRAC_PI_2, 2)
            .expect("max steps=2");
        assert!(
            (max_tau - tau_at_end).abs() < 1e-6,
            "steps=2 samples endpoints; max should be |tau_g(π/2)|={tau_at_end}, got {max_tau}",
        );
    }

    #[test]
    fn saturation_check_rejects_out_of_range_joint() {
        let model = arm_3dof_right_model();
        let err = max_gravity_torque_over_range(&model, 5, 0.0, 1.0, 10)
            .expect_err("out-of-range joint index should error");
        assert!(matches!(err, DynamicsError::JointCount { .. }));
    }
}
