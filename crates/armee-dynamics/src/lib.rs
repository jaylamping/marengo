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
//!   gravity. A Y-axis pendulum with its COM directly below the pivot at q=0 needs
//!   positive holding torque at positive q. For a general chain, lateral offsets and
//!   origin rotations change that relation: torque sign follows the complete model,
//!   not joint-angle sign alone.
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

pub mod calibration;
pub mod drive_loss;
mod urdf_gravity;

use std::path::Path;

use armee_kinematics::UrdfError;
use thiserror::Error;
pub use urdf_gravity::{LinkInertial, UrdfGravityModel};

#[derive(Debug, Error)]
pub enum DynamicsError {
    #[error("urdf: {0}")]
    Urdf(#[from] UrdfError),
    #[error("joint count mismatch: expected {expected}, got {got}")]
    JointCount { expected: usize, got: usize },
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("unknown link {link}")]
    UnknownLink { link: String },
    /// An actuated URDF joint is neither modelled nor explicitly held: its angle would be
    /// silently evaluated at 0 rad and every τ_g downstream of it would be wrong.
    #[error("URDF joint {joint} is actuated but not listed in the model joints (robot.yaml)")]
    UnmodelledJoint { joint: String },
    #[error("joint {joint} listed more than once")]
    DuplicateJoint { joint: String },
    /// A named model joint is fixed (or otherwise not a revolute/continuous joint).
    #[error("joint {joint} is {kind}, not an actuated revolute/continuous joint")]
    NotActuated { joint: String, kind: &'static str },
    /// The gravity model cannot represent this joint (prismatic motion, mimic, floating base).
    #[error("joint {joint}: {what} is not supported by the gravity model")]
    UnsupportedJoint { joint: String, what: &'static str },
    #[error("URDF link chain of {link} is cyclic or ambiguous")]
    CyclicChain { link: String },
    #[error("invalid URDF model: {what}")]
    InvalidModel { what: String },
    #[error("non-finite input to the gravity model: {what}")]
    NonFiniteInput { what: &'static str },
    #[error("gravity model produced a non-finite torque for joint {joint}")]
    NonFiniteTorque { joint: String },
    /// The drive-loss admission grid would take too long to evaluate.
    #[error("drive-loss grid needs {evaluations} model evaluations, above the {max} bound")]
    LossGridTooLarge { evaluations: usize, max: usize },
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
pub struct PureGravityTorque(pub(crate) Vec<f64>);

impl PureGravityTorque {
    /// Borrow the torque values as a slice (used by the Berthier tick).
    pub fn as_slice(&self) -> &[f64] {
        &self.0
    }

    /// Consume the newtype and return the inner Vec.
    pub fn into_inner(self) -> Vec<f64> {
        self.0
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
///
/// `joint_names` must name **every** actuated URDF joint exactly once; see
/// [`UrdfGravityModel::from_urdf`].
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
/// Fails closed: a non-finite range or a non-finite torque is an error (`f64::max` would
/// otherwise swallow a NaN and report a smaller maximum).
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
    if !q_min.is_finite() || !q_max.is_finite() {
        return Err(DynamicsError::NonFiniteInput {
            what: "gravity sweep range",
        });
    }
    let steps = steps.max(2);
    let mut max_tau = 0.0f64;
    let mut q_vec = vec![0.0; n];
    for i in 0..steps {
        let t = i as f64 / (steps - 1) as f64;
        q_vec[joint_index] = q_min + (q_max - q_min) * t;
        let tau = model.gravity_torques(&q_vec)?;
        let v = tau
            .get(joint_index)
            .copied()
            .ok_or(DynamicsError::JointCount {
                expected: n,
                got: tau.len(),
            })?;
        if !v.is_finite() {
            return Err(DynamicsError::NonFiniteTorque {
                joint: model.joint_names()[joint_index].clone(),
            });
        }
        max_tau = max_tau.max(v.abs());
    }
    Ok(max_tau)
}

/// Fraction of the torque limit above which the preflight warns.
pub const GRAVITY_WARN_FRACTION: f64 = 0.8;

/// Outcome of the pre-enable gravity saturation check for one joint.
#[derive(Debug)]
pub enum GravityRangeVerdict {
    /// `max |τ_g|` is comfortably below the limit.
    Within { tau_max_nm: f64 },
    /// Above [`GRAVITY_WARN_FRACTION`] of the limit but not over it: enable with a warning.
    Near { tau_max_nm: f64 },
    /// Over the limit (or the limit itself is not a usable number): refuse.
    Saturated { tau_max_nm: f64 },
    /// The model could not evaluate the range (bad URDF inertial, NaN, wrong dimensions).
    /// Refuse: an unevaluated gravity load is not a zero gravity load.
    Unevaluable(DynamicsError),
}

impl GravityRangeVerdict {
    /// True when enabling must be refused.
    pub fn refuses(&self) -> bool {
        matches!(self, Self::Saturated { .. } | Self::Unevaluable(_))
    }
}

/// Shared preflight for `marengo-pi` and `motor-repl`: sweep `joint_index` over its bench range
/// and compare `max |τ_g|` with `torque_limit_nm`.
pub fn check_gravity_range(
    model: &dyn DynamicsModel,
    joint_index: usize,
    q_min: f64,
    q_max: f64,
    torque_limit_nm: f64,
    steps: usize,
) -> GravityRangeVerdict {
    let tau_max_nm = match max_gravity_torque_over_range(model, joint_index, q_min, q_max, steps) {
        Ok(tau) => tau,
        Err(error) => return GravityRangeVerdict::Unevaluable(error),
    };
    // Written so a NaN limit saturates instead of comparing false.
    if tau_max_nm.is_nan() || torque_limit_nm.is_nan() || tau_max_nm > torque_limit_nm {
        GravityRangeVerdict::Saturated { tau_max_nm }
    } else if tau_max_nm > GRAVITY_WARN_FRACTION * torque_limit_nm {
        GravityRangeVerdict::Near { tau_max_nm }
    } else {
        GravityRangeVerdict::Within { tau_max_nm }
    }
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

    fn arm_3dof_right_model() -> UrdfGravityModel {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/urdf/archive/seed-arm_3dof_right/contributor.urdf");
        UrdfGravityModel::from_urdf_with_held(
            &path,
            &["right_shoulder_pitch".to_string()],
            &[
                ("right_shoulder_roll".to_string(), 0.0),
                ("right_upper_arm_yaw".to_string(), 0.0),
            ],
        )
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

    /// Scripted model: constant torque, an error, or NaN, for the preflight verdicts.
    struct Scripted {
        names: Vec<String>,
        result: Result<f64, ()>,
    }

    impl DynamicsModel for Scripted {
        fn joint_names(&self) -> &[String] {
            &self.names
        }

        fn gravity_torques(&self, _q: &[f64]) -> Result<PureGravityTorque, DynamicsError> {
            match self.result {
                Ok(tau) => Ok(PureGravityTorque(vec![tau])),
                Err(()) => Err(DynamicsError::InvalidModel {
                    what: "scripted failure".to_string(),
                }),
            }
        }
    }

    fn scripted(result: Result<f64, ()>) -> Scripted {
        Scripted {
            names: vec!["j".to_string()],
            result,
        }
    }

    #[test]
    fn preflight_classifies_torque_against_the_limit() {
        let limit = 10.0;
        assert!(matches!(
            check_gravity_range(&scripted(Ok(5.0)), 0, 0.0, 1.0, limit, 5),
            GravityRangeVerdict::Within { .. }
        ));
        assert!(matches!(
            check_gravity_range(&scripted(Ok(-9.0)), 0, 0.0, 1.0, limit, 5),
            GravityRangeVerdict::Near { .. }
        ));
        let over = check_gravity_range(&scripted(Ok(11.0)), 0, 0.0, 1.0, limit, 5);
        assert!(matches!(over, GravityRangeVerdict::Saturated { .. }) && over.refuses());
    }

    /// L7: `.unwrap_or(0.0)` on a model error used to read as "no gravity load" and pass.
    #[test]
    fn preflight_refuses_when_the_model_cannot_be_evaluated() {
        let broken = check_gravity_range(&scripted(Err(())), 0, 0.0, 1.0, 10.0, 5);
        assert!(
            matches!(broken, GravityRangeVerdict::Unevaluable(_)) && broken.refuses(),
            "{broken:?}"
        );
        // NaN torque (e.g. a zero axis that slipped through) and a NaN sweep range refuse too.
        assert!(check_gravity_range(&scripted(Ok(f64::NAN)), 0, 0.0, 1.0, 10.0, 5).refuses());
        assert!(check_gravity_range(&scripted(Ok(1.0)), 0, f64::NAN, 1.0, 10.0, 5).refuses());
        // A NaN limit saturates; an out-of-range joint index is an error, not a pass.
        assert!(check_gravity_range(&scripted(Ok(1.0)), 0, 0.0, 1.0, f64::NAN, 5).refuses());
        assert!(check_gravity_range(&scripted(Ok(1.0)), 3, 0.0, 1.0, 10.0, 5).refuses());
    }
}
