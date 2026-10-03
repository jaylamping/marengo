//! `build_limits` must refuse bad model/config input, never panic or fall back to defaults
//! (audit leads K1, K7).
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use crate::simulation::{InitialVirtualReference, SimulationBus};
use crate::{build_limits, DavoutError, Supervisor};

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
