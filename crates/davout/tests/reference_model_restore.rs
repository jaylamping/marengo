//! Typed model installation must refuse before touching a live reservation.
#![allow(clippy::expect_used)]

use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{DavoutError, ReferenceCancelReason, ReferencePhase, ReferenceRequest, Supervisor};

#[test]
fn model_restore_refuses_while_reserved_and_succeeds_after_cleanup() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut owner = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::Unreferenced,
    )
    .expect("actual isolated virtual owner");
    let before_upper = owner
        .urdf_robot()
        .joints
        .iter()
        .find(|joint| joint.name == "right_elbow_pitch")
        .expect("installed elbow")
        .limit
        .upper;
    let mut candidate = owner.urdf_robot().clone();
    candidate
        .joints
        .iter_mut()
        .find(|joint| joint.name == "right_elbow_pitch")
        .expect("copied elbow")
        .limit
        .upper = before_upper - 0.05;
    let motors = owner.motors.clone();
    let control = owner.control.clone();
    let handle = owner
        .begin_reference(ReferenceRequest {
            stamp: owner.reference_snapshot().next_stamp.expect("issued stamp"),
            joint: "right_elbow_pitch".into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("reserve with no drive output");
    owner.bus_mut().clear_trace();
    let attempted =
        owner.restore_limit_snapshot(motors.clone(), control.clone(), candidate.clone());
    let during_upper = owner
        .urdf_robot()
        .joints
        .iter()
        .find(|joint| joint.name == "right_elbow_pitch")
        .expect("retained elbow")
        .limit
        .upper;
    let during = owner.reference_snapshot();
    let during_trace = owner.bus().frames().to_vec();
    let cleanup = owner.cancel_reference(&handle, ReferenceCancelReason::Operator);
    let outside = owner.restore_limit_snapshot(motors, control, candidate);
    let outside_upper = owner
        .urdf_robot()
        .joints
        .iter()
        .find(|joint| joint.name == "right_elbow_pitch")
        .expect("replaced elbow")
        .limit
        .upper;
    let usable = owner.reference_snapshot().usable_reference;
    drop(owner);

    assert!(
        matches!(attempted, Err(DavoutError::ReferenceBusy { .. })),
        "typed model restore must refuse before mutating a live acquisition"
    );
    assert_eq!(during_upper, before_upper);
    assert_eq!(during.phase, ReferencePhase::BaselineStop);
    assert_eq!(during.handle.as_ref(), Some(&handle));
    assert!(during_trace.is_empty());
    assert_eq!(cleanup.expect("actual cleanup").stop.attempts.len(), 15);
    outside.expect("same valid model installs outside the reservation");
    assert_eq!(outside_upper, before_upper - 0.05);
    assert!(!usable);
}
