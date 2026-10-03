//! `build_limits` must refuse bad model/config input, never panic or fall back to defaults
//! (audit leads K1, K7).
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use crate::simulation::{InitialVirtualReference, SimulationBus};
use crate::{build_limits, DavoutError, Supervisor};
use marengo_config::motor_type_key;

const JOINT: &str = "right_elbow_pitch";

fn supervisor() -> Supervisor<SimulationBus> {
    Supervisor::from_simulation(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
    )
    .expect("supervisor")
}

fn urdf_with_limits(sup: &Supervisor<SimulationBus>, lower: f64, upper: f64) -> urdf_rs::Robot {
    let mut urdf = sup.urdf_robot().clone();
    let joint = urdf
        .joints
        .iter_mut()
        .find(|j| j.name == JOINT)
        .expect("joint");
    joint.limit.lower = lower;
    joint.limit.upper = upper;
    urdf
}

#[test]
fn inverted_or_nan_urdf_limits_are_an_error_not_a_panic() {
    let sup = supervisor();
    for (lower, upper) in [(2.0, -2.0), (f64::NAN, 1.0), (0.0, f64::NAN)] {
        let urdf = urdf_with_limits(&sup, lower, upper);
        let err = build_limits(&sup.robot, &sup.motors, &sup.control, &urdf)
            .expect_err("bad URDF limits must be refused");
        assert!(
            matches!(err, DavoutError::Urdf(_)),
            "[{lower}, {upper}] -> {err}"
        );
    }
}

#[test]
fn bench_torque_cap_never_binds_tau_ff_max() {
    // P-davout-15: `build_limits` used to apply `.min(motor.bench.torque_limit_nm)`
    // on top of `effort`, but `effort` already folds the bench cap in, so the extra
    // `.min` could never change the result. Pin that redundancy: if a future config
    // ever lets the bench cap bind beyond `effort`, this fails and the removal must
    // be revisited.
    let sup = supervisor();
    let policies = build_limits(&sup.robot, &sup.motors, &sup.control, sup.urdf_robot())
        .expect("master limits build");
    assert!(!policies.is_empty(), "master config must cover joints");
    for motor in &sup.motors.motors {
        let policy = policies.get(&motor.joint).expect("joint policy");
        let defaults = sup
            .control
            .control
            .motor_type_defaults
            .get(motor_type_key(motor.motor_type))
            .expect("motor type defaults");
        assert!(
            policy.effort <= motor.bench.torque_limit_nm,
            "{}: effort {} exceeds bench cap {}",
            motor.joint,
            policy.effort,
            motor.bench.torque_limit_nm
        );
        let expected = policy.effort.min(defaults.tau_ff_max_nm);
        assert!(
            (policy.tau_ff_max - expected).abs() < 1e-12,
            "{}: tau_ff_max {} != effort.min(defaults) {expected}",
            motor.joint,
            policy.tau_ff_max
        );
    }
}

#[test]
fn joint_without_control_entry_is_refused_not_defaulted() {
    let mut sup = supervisor();
    assert!(sup.control.control.joints.remove(JOINT).is_some());
    let err = build_limits(&sup.robot, &sup.motors, &sup.control, sup.urdf_robot())
        .expect_err("missing control.joints entry");
    assert!(
        matches!(&err, DavoutError::Limit { joint, message }
            if joint == JOINT && message.contains("missing control.joints")),
        "{err}"
    );
}
