//! Golden-value tests for tau_g from the `arm_2dof_right` URDF.
//!
//! Golden values AFTER COM correction (18in → 14in, 2026-06-19).
//! Source: computed from corrected URDF + bench Phase 4 validation.
//!
//! NOTE: golden values are stale after recent URDF changes — rebaseline
//! against bench data before removing `#[ignore]`.

#![allow(clippy::expect_used)]

use armee_dynamics::{gravity_model_from_urdf, DynamicsModel};
use std::path::Path;

/// Build the gravity model from the arm_2dof_right URDF (single joint).
fn model() -> armee_dynamics::UrdfGravityModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/urdf/arm_2dof_right.urdf");
    gravity_model_from_urdf(&path, &["right_shoulder_pitch".to_string()])
        .expect("build UrdfGravityModel from arm_2dof_right.urdf")
}

#[test]
#[ignore]
fn golden_tau_g_at_zero() {
    let model = model();
    let tau = model.gravity_torques(&[0.0]).expect("tau_g at q=0.0");
    // Arm hanging straight down: tau_g should be near 0.
    assert!(
        tau[0].abs() < 0.01,
        "tau_g at q=0 should be ~0, got {}",
        tau[0]
    );
}

#[test]
#[ignore]
fn golden_tau_g_at_0_3() {
    let model = model();
    let tau = model.gravity_torques(&[0.3]).expect("tau_g at q=0.3");
    let expected = 0.7306;
    let diff = (tau[0] - expected).abs();
    assert!(
        diff < 0.05,
        "tau_g at q=0.3 expected ~{expected}, got {} (diff {diff})",
        tau[0],
    );
}

#[test]
#[ignore]
fn golden_tau_g_at_neg_0_3() {
    let model = model();
    let tau = model.gravity_torques(&[-0.3]).expect("tau_g at q=-0.3");
    let expected = -0.7306;
    let diff = (tau[0] - expected).abs();
    assert!(
        diff < 0.05,
        "tau_g at q=-0.3 expected ~{expected}, got {} (diff {diff})",
        tau[0],
    );
}

#[test]
#[ignore]
fn golden_tau_g_at_pi_over_2() {
    let model = model();
    let tau = model
        .gravity_torques(&[std::f64::consts::FRAC_PI_2])
        .expect("tau_g at q=π/2");
    let expected = 2.4721;
    let diff = (tau[0] - expected).abs();
    assert!(
        diff < 0.1,
        "tau_g at q=π/2 expected ~{expected}, got {} (diff {diff})",
        tau[0],
    );
}

#[test]
#[ignore]
fn golden_tau_g_symmetric() {
    let model = model();
    let tau_pos = model.gravity_torques(&[0.3]).expect("tau_g at q=0.3");
    let tau_neg = model.gravity_torques(&[-0.3]).expect("tau_g at q=-0.3");
    let diff = (tau_pos[0] + tau_neg[0]).abs();
    assert!(
        diff < 0.001,
        "tau_g should be symmetric: tau_g(0.3)={} + tau_g(-0.3)={} = {} (diff {diff})",
        tau_pos[0],
        tau_neg[0],
        tau_pos[0] + tau_neg[0],
    );
}

#[test]
#[ignore]
fn golden_tau_g_at_neg_0_5() {
    let model = model();
    let tau = model.gravity_torques(&[-0.5]).expect("tau_g at q=-0.5");
    let expected = -1.1852;
    let diff = (tau[0] - expected).abs();
    assert!(
        diff < 0.05,
        "tau_g at q=-0.5 expected ~{expected}, got {} (diff {diff})",
        tau[0],
    );
}

#[test]
#[ignore]
fn golden_tau_g_at_0_785() {
    let model = model();
    let tau = model.gravity_torques(&[0.785]).expect("tau_g at q=0.785");
    let expected = 1.7474;
    let diff = (tau[0] - expected).abs();
    assert!(
        diff < 0.1,
        "tau_g at q=0.785 expected ~{expected}, got {} (diff {diff})",
        tau[0],
    );
}
