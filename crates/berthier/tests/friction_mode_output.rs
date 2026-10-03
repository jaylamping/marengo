//! Friction selection through the shared controller and Davout output path.
//! Virtual initial references are initial conditions, not device reference proof.

#![allow(clippy::expect_used)]

mod support;

use berthier::{ControlLoop, ControlMode, GainOverride};
use davout::simulation::{InitialVirtualReference, SimulationBus};
use robstride::CanFrame;
use support::queue_all_status;

fn command(mode: ControlMode, fc: f64) -> CanFrame {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let joint = "right_upper_arm_yaw";
    let mut controller = ControlLoop::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![joint.into()]),
        200,
        50,
    )
    .expect("virtual initial reference");
    controller
        .supervisor_mut()
        .enable_targets(&[joint.into()])
        .expect("scoped enable");
    controller.set_control_mode(mode);
    let applied = controller.apply_gain_override(
        joint,
        GainOverride {
            kp: 0.0,
            kd: 0.0,
            ki: 0.0,
            fc,
        },
    );
    if mode == ControlMode::GravityComp {
        // L-berthier-10: the override has no effect here and says so.
        assert!(matches!(
            applied,
            Err(berthier::LoopError::GainOverrideNotApplicable { .. })
        ));
    } else {
        applied.expect("valid output-only override");
    }
    queue_all_status(controller.supervisor_mut(), Some((joint, 0.0, 0.2)));
    // Let the configured torque slew admit this small independent friction step.
    std::thread::sleep(std::time::Duration::from_millis(5));
    controller.supervisor_mut().bus_mut().clear_trace();
    controller.tick(None).expect("actual output tick");
    assert_eq!(controller.supervisor().bus().frames().len(), 1);
    controller.supervisor().bus().frames()[0].clone()
}

#[test]
fn friction_override_changes_impedance_output_and_is_ignored_in_gravity_comp() {
    let gravity_zero = command(ControlMode::GravityComp, 0.0);
    let gravity_friction = command(ControlMode::GravityComp, 0.1);
    assert_eq!(
        gravity_zero, gravity_friction,
        "gravity mode ignores friction override"
    );
    let impedance_zero = command(ControlMode::Impedance, 0.0);
    let impedance_friction = command(ControlMode::Impedance, 0.1);
    assert_eq!(
        impedance_zero.data, impedance_friction.data,
        "friction changes only torque"
    );
    for frame in [&impedance_zero, &impedance_friction] {
        assert_eq!((frame.id >> 24) & 0x1f, 1);
        assert_eq!(frame.id & 0xff, 3);
        assert_eq!(&frame.data[4..8], &[0; 4]);
    }
    // Independent literal master yaw: direction -1, ratio 1, RS02 ±17 Nm.
    let torque = |frame: &CanFrame| -(((frame.id >> 8) & 0xffff) as f64 / 32767.0 - 1.0) * 17.0;
    let delta = torque(&impedance_friction) - torque(&impedance_zero);
    assert!(
        (0.09..0.102).contains(&delta),
        "actual friction torque delta {delta}"
    );
}
