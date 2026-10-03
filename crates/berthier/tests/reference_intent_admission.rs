//! New control intent is refused during an existing reference reservation.
#![allow(clippy::expect_used)]

use berthier::{ControlLoop, ControlMode, GainOverride, LoopError};
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{DavoutError, ReferenceCancelReason, ReferencePhase, ReferenceRequest};

#[test]
fn reserved_owner_refuses_new_torque_mode_and_gain_intent() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut controller = ControlLoop::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::Unreferenced,
        200,
        50,
    )
    .expect("actual closed controller owner");
    let stamp = controller
        .supervisor()
        .reference_snapshot()
        .next_stamp
        .expect("issued stamp");
    let handle = controller
        .supervisor_mut()
        .begin_reference(ReferenceRequest {
            stamp,
            joint: "right_elbow_pitch".into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("real reservation");
    controller.supervisor_mut().bus_mut().clear_trace();
    controller.set_control_mode(ControlMode::GravityComp);
    let mode = controller.control_mode();
    let torque = controller.set_torque_cmd("right_elbow_pitch", 0.25);
    controller.set_control_mode(ControlMode::Impedance);
    let gain = controller.apply_gain_override(
        "right_elbow_pitch",
        GainOverride {
            kp: 8.0,
            kd: 1.25,
            ki: 0.0,
            fc: 0.0,
        },
    );
    let latched = controller.torque_cmd("right_elbow_pitch");
    let gain_latched = controller.gain_override("right_elbow_pitch").cloned();
    let snapshot = controller.supervisor().reference_snapshot();
    let trace = controller.supervisor().bus().frames().to_vec();
    let cleanup = controller
        .supervisor_mut()
        .cancel_reference(&handle, ReferenceCancelReason::Operator);
    let outside = controller.set_torque_cmd("right_elbow_pitch", 0.25);
    let outside_mode = controller.control_mode();
    let outside_latched = controller.torque_cmd("right_elbow_pitch");
    controller.inhibit_motion_for_shutdown();
    drop(controller);

    assert!(
        matches!(
            torque,
            Err(LoopError::Safety(DavoutError::ReferenceBusy { .. }))
        ),
        "reference reservation must reject new torque intent before latching it"
    );
    assert!(matches!(
        gain,
        Err(LoopError::Safety(DavoutError::ReferenceBusy { .. }))
    ));
    assert_eq!(mode, ControlMode::Disabled);
    assert_eq!(latched, 0.0);
    assert!(gain_latched.is_none());
    assert_eq!(snapshot.phase, ReferencePhase::BaselineStop);
    assert_eq!(snapshot.handle.as_ref(), Some(&handle));
    assert!(trace.is_empty());
    assert_eq!(
        cleanup
            .expect("mandatory actual cleanup")
            .stop
            .attempts
            .len(),
        15
    );
    outside.expect("same intent accepted outside the reservation without drive permission");
    assert_eq!(outside_mode, ControlMode::TorqueOnly);
    assert_eq!(outside_latched, 0.25);
}
