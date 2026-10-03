// L-berthier-09: the per-joint `unwrap_or` fallbacks in `ControlLoop` are unreachable because
// construction refuses a robot joint without a `control.yaml` entry. Pins that precondition at
// the actual constructor, so a future relaxation of the config check re-opens the fail-open
// stiffness defaults (Position impedance kp 20 / kd 1, slew 0.15 vs 0.25, ...).
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use berthier::{ControlLoop, LoopError};
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::DavoutError;

#[test]
fn construction_refuses_a_robot_joint_without_a_control_entry() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = support::FixtureTree::new("config-joint-coverage", &source);
    let control_path = fixture.path().join("config/control.yaml");
    let text = std::fs::read_to_string(&control_path).expect("copied fixture control");
    let start = text
        .find("    right_lower_arm_yaw:\n")
        .expect("joint block present");
    let end = text[start..]
        .find("\n  danger_zones:")
        .map(|offset| start + offset)
        .expect("joint block ends at danger_zones");
    let mut without = String::new();
    without.push_str(&text[..start]);
    without.push_str(&text[end..]);
    std::fs::write(&control_path, without).expect("fixture-only edit");

    let result = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    );
    let error = result
        .err()
        .expect("incomplete control.yaml must be refused");
    assert!(
        matches!(&error, LoopError::Safety(DavoutError::Config(_)))
            && error.to_string().contains("right_lower_arm_yaw"),
        "{error:?}"
    );
}
