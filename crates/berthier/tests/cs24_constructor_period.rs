// Existing-public constructor regression; no new error API needed for baseline.
// Unexecuted preparation. Parent binds/formats/runs with child-only J-backed temp.
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::time::Duration;

use berthier::{ControlLoop, ControlMode};
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::OperationalMode;

#[test]
fn zero_rounded_loop_period_is_rejected_by_public_simulation_factory() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master_control =
        std::fs::read(source.join("config/control.yaml")).expect("immutable master control input");
    let fixture = support::fixture_tree_without_diagnostics("cs24-constructor-period", &source);
    let fixture_path = fixture.path().to_path_buf();

    // Positive control uses the same valid root/reference declaration as the bad
    // period case. Fully construct and DROP it before calling the invalid case.
    let valid = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("valid 200 Hz public factory must construct with the actual model/config");
    let valid_period = valid.loop_period();
    let valid_hz = valid.configured_loop_hz();
    let valid_modes = (valid.supervisor().mode(), valid.control_mode());
    let valid_joints = valid.joint_names().len();
    let valid_no_transmissions = valid.supervisor().bus().transmissions().is_empty();
    drop(valid);

    // 1 / u32::MAX seconds rounds to Duration::ZERO at nanosecond resolution.
    // All other inputs are valid. This invokes the ORIGINAL public constructor,
    // not a missing new API or a deliberately invalid history/reference input.
    let invalid = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        u32::MAX,
        50,
    );
    let rejected = invalid.is_err();
    let error = invalid.as_ref().err().map(|error| error.to_string());
    let admitted_period = invalid.as_ref().ok().map(|ctrl| ctrl.loop_period());
    let admitted_hz = invalid.as_ref().ok().map(|ctrl| ctrl.configured_loop_hz());
    let admitted_modes = invalid
        .as_ref()
        .ok()
        .map(|ctrl| (ctrl.supervisor().mode(), ctrl.control_mode()));
    let admitted_transmissions = invalid
        .as_ref()
        .ok()
        .map(|ctrl| ctrl.supervisor().bus().transmissions().len());
    drop(invalid);
    drop(fixture);

    assert!(!fixture_path
        .try_exists()
        .expect("actual exclusive fixture cleanup"));
    assert_eq!(
        std::fs::read(source.join("config/control.yaml")).expect("master unchanged"),
        master_control
    );
    assert_eq!(valid_period, Duration::from_millis(5));
    assert_eq!(valid_hz, 200);
    assert_eq!(
        valid_modes,
        (OperationalMode::Disabled, ControlMode::Disabled)
    );
    assert_eq!(valid_joints, 5);
    assert!(
        valid_no_transmissions,
        "valid construction does not emit actuator commands"
    );
    assert!(rejected,
        "CS24 period: the public factory must reject a zero-rounded controller period; admitted_period={admitted_period:?}, admitted_hz={admitted_hz:?}, admitted_modes={admitted_modes:?}, admitted_transmissions={admitted_transmissions:?}, error={error:?}");
}
