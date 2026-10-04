//! Independent rigid-body examples, deliberately separate from mutable robot CAD.
//!
//! Holding torque is dU/dq. For the pendulum, U = -9.81 cos(q).
//! For the two-link chain, U = -39.24 cos(shoulder) - 7.3575 cos(shoulder + elbow).
//! Expected torques below are worked values at exact special angles, not values
//! regenerated from this library. These tests require no hardware or simulator.

#![allow(clippy::expect_used)]

use armee_dynamics::{gravity_model_from_urdf, DynamicsModel, UrdfGravityModel};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_6, PI};
use std::path::Path;

fn model(fixture: &str, joints: &[&str]) -> UrdfGravityModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);
    let joints: Vec<String> = joints.iter().map(|name| (*name).to_owned()).collect();
    gravity_model_from_urdf(path, &joints).expect("analytic URDF fixture must load")
}

fn assert_torques(model: &UrdfGravityModel, q: &[f64], expected: &[f64]) {
    let actual = model.gravity_torques(q).expect("valid analytic pose");
    assert_eq!(actual.len(), expected.len());
    for (joint, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() < 1e-6,
            "joint {joint}, q={q:?}: holding torque {actual}, expected {expected} Nm"
        );
    }
}

#[test]
fn pendulum_holding_torque_has_analytic_sign_and_magnitude() {
    // 2 kg, COM 0.5 m below the pivot: m*g*l = 9.81 Nm.
    let model = model("pendulum.urdf", &["pivot"]);
    for (angle, torque) in [
        (0.0, 0.0),
        (FRAC_PI_6, 4.905),
        (-FRAC_PI_6, -4.905),
        (FRAC_PI_2, 9.81),
        (-FRAC_PI_2, -9.81),
        (PI, 0.0),
    ] {
        assert_torques(&model, &[angle], &[torque]);
    }
}

#[test]
fn two_link_chain_accounts_for_distal_load_and_joint_coupling() {
    // Upper link: 2 kg at 0.5 m. Elbow: 1 m below shoulder.
    // Distal link: 3 kg at 0.25 m below elbow.
    let model = model("two-link.urdf", &["shoulder", "elbow"]);
    for (q, expected) in [
        ([0.0, 0.0], [0.0, 0.0]),
        ([0.0, FRAC_PI_2], [7.3575, 7.3575]),
        ([FRAC_PI_2, 0.0], [46.5975, 7.3575]),
        ([FRAC_PI_2, -FRAC_PI_2], [39.24, 0.0]),
        ([FRAC_PI_6, FRAC_PI_3], [26.9775, 7.3575]),
        ([-FRAC_PI_6, -FRAC_PI_3], [-26.9775, -7.3575]),
    ] {
        assert_torques(&model, &q, &expected);
    }
}

#[test]
fn rotated_joint_origin_changes_the_gravity_reference() {
    // The same pendulum starts horizontal after a +pi/2 origin rotation.
    let model = model("rotated-pendulum.urdf", &["pivot"]);
    for (angle, torque) in [(0.0, 9.81), (FRAC_PI_2, 0.0), (PI, -9.81)] {
        assert_torques(&model, &[angle], &[torque]);
    }
}

#[test]
fn configured_joint_order_defines_input_and_output_order() {
    let model = model("two-link.urdf", &["elbow", "shoulder"]);
    assert_torques(&model, &[0.0, FRAC_PI_2], &[7.3575, 46.5975]);
}

#[test]
fn joint_inertia_sums_carried_point_masses_and_link_tensors() {
    // About the shoulder: upper 2·0.5² + 0.01; distal 3·(distance to the axis)² + 0.01, the
    // distance being 1.25 m when straight and √(1² + 0.25²) m with the elbow at π/2.
    // About the elbow: 3·0.25² + 0.01 at any pose.
    let model = model("two-link.urdf", &["shoulder", "elbow"]);
    for (q, shoulder, elbow) in [
        ([0.0, 0.0], 0.51 + 4.6975, 0.1975),
        ([0.7, FRAC_PI_2], 0.51 + 3.1975, 0.1975),
    ] {
        let got = model.joint_inertia("shoulder", &q).expect("shoulder");
        assert!((got - shoulder).abs() < 1e-9, "shoulder at {q:?}: {got}");
        let got = model.joint_inertia("elbow", &q).expect("elbow");
        assert!((got - elbow).abs() < 1e-9, "elbow at {q:?}: {got}");
    }
}
