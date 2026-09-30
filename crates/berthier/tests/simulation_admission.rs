//! Constructor conformance: only the closed constructor declares virtual initial state.

#![allow(clippy::expect_used)]

use berthier::ControlLoop;
use davout::simulation::{InitialVirtualReference, SimulationBus};

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn ordinary_constructor_with_closed_bus_still_has_no_reference_authority() {
    let mut controller = ControlLoop::from_repo(root(), SimulationBus::default(), 200, 50)
        .expect("ordinary constructor");
    assert!(controller
        .supervisor_mut()
        .enable_targets(&["right_shoulder_pitch".into()])
        .is_err());
    assert!(controller
        .supervisor()
        .bus()
        .frames()
        .iter()
        .all(|frame| (frame.id >> 24) & 0x1f != 3));
}

#[test]
fn simulation_constructor_admits_only_declared_virtual_joints() {
    let mut controller = ControlLoop::from_simulation(
        root(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec!["right_shoulder_roll".into()]),
        200,
        50,
    )
    .expect("virtual initial condition");
    assert!(controller
        .supervisor_mut()
        .enable_targets(&["right_shoulder_pitch".into()])
        .is_err());
    assert!(controller
        .supervisor()
        .bus()
        .frames()
        .iter()
        .all(|frame| (frame.id >> 24) & 0x1f != 3));
    controller
        .supervisor_mut()
        .enable_targets(&["right_shoulder_roll".into()])
        .expect("declared joint admits normal production Enable");
    assert!(controller
        .supervisor()
        .bus()
        .frames()
        .iter()
        .any(|frame| frame.id == 0x0300_fd02));
}
