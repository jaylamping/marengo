//! Cross-crate sign convention tests for joint-space tau_g.
//!
//! Verifies that tau_g from armee-dynamics is positive at positive q. Gravity pulls
//! down, so the motor must push up against gravity with positive joint torque.

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use armee_dynamics::DynamicsModel;

/// Repo root relative to `CARGO_MANIFEST_DIR` of the berthier crate.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn tau_g_sign_positive_at_positive_q() {
    // At q=0.3 rad, tau_g should be POSITIVE for the right_shoulder_pitch joint.
    // Gravity pulls the arm down; the motor must push up (= positive joint torque).
    // Golden value from bench-weighted-700g-results.md Phase 1.
    let model_path = repo_root().join("assets/urdf/archive/seed-arm_3dof_right/contributor.urdf");
    let model =
        armee_dynamics::gravity_model_from_urdf(&model_path, &["right_shoulder_pitch".to_string()])
            .expect("build UrdfGravityModel");
    let tau = model.gravity_torques(&[0.3]).expect("tau_g at q=0.3");
    assert!(
        tau[0] > 0.0,
        "tau_g at positive q must be positive: got {}",
        tau[0],
    );
    // Magnitude for the bench stub URDF (0.7 kg @ 0.36 m COM) — not the weighted golden.
    assert!(
        (tau[0] - 0.590).abs() < 0.05,
        "tau_g at q=0.3 expected ~0.590, got {}",
        tau[0],
    );
}

#[test]
fn tau_g_at_pi_over_2_positive() {
    // Confirm sign is positive even at large q.
    let model_path = repo_root().join("assets/urdf/archive/seed-arm_3dof_right/contributor.urdf");
    let model =
        armee_dynamics::gravity_model_from_urdf(&model_path, &["right_shoulder_pitch".to_string()])
            .expect("build UrdfGravityModel");
    let tau = model
        .gravity_torques(&[std::f64::consts::FRAC_PI_2])
        .expect("tau_g at q=π/2");
    assert!(
        tau[0] > 0.0,
        "tau_g at q=π/2 must be positive: got {}",
        tau[0],
    );
}
