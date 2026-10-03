//! A geometry-specific reference for the archived arm, not live bench acceptance.
//!
//! The old Berthier tests assumed sign(tau_g) == sign(q) and omitted joint-origin
//! lever arms. That assumption is false with lateral offsets. The analytic tests
//! below use the archived fixture's documented mass moments, not model output.

#![allow(clippy::expect_used)]

use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use std::f64::consts::{FRAC_PI_2, PI};
use std::path::Path;

#[test]
fn archived_arm_holding_torque_includes_lateral_joint_offsets() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/urdf/archive/seed-arm_3dof_right/contributor.urdf");
    // The other two joints are held at zero explicitly; the model never assumes it.
    let model = UrdfGravityModel::from_urdf_with_held(
        path,
        &["right_shoulder_pitch".to_owned()],
        &[
            ("right_shoulder_roll".to_owned(), 0.0),
            ("right_upper_arm_yaw".to_owned(), 0.0),
        ],
    )
    .expect("archived arm fixture");

    // Other joint angles are zero. Shoulder-frame mass moments are:
    // X = 0.3*(0.05 + 0.05) + 0.7*(0.05 + 0.08) = 0.121 kg*m.
    // Z = 0.7*(-0.36) = -0.252 kg*m.
    // Therefore U(q) = -1.18701*sin(q) - 2.47212*cos(q) joules, and
    // holding torque = 2.47212*sin(q) - 1.18701*cos(q) Nm.
    // At zero it is negative; zero angle is not the hanging-COM reference.
    for (angle, expected) in [
        (0.0, -1.18701),
        (FRAC_PI_2, 2.47212),
        (-FRAC_PI_2, -2.47212),
        (PI, 1.18701),
    ] {
        let tau = model.gravity_torques(&[angle]).expect("valid pose");
        assert!(
            (tau[0] - expected).abs() < 1e-6,
            "q={angle}: holding torque {}, expected {expected} Nm",
            tau[0]
        );
    }
}
