//! Candidate-interface restoration contracts; no physical reference acquisition.
#![allow(clippy::expect_used)]

use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{JointHomingState, Supervisor};

#[path = "../../marengo-homing/tests/support/mod.rs"]
mod support;

fn owner(directory: &support::TestDirectory) -> Supervisor<SimulationBus> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Supervisor::from_simulation_with_calibration_record_path(
        root,
        SimulationBus::default(),
        directory.path().join("history.yaml"),
        InitialVirtualReference::AllConfigured,
    )
    .expect("closed virtual initial reference")
}

#[test]
fn invalid_disabled_restore_preserves_installed_model_policy_and_reference() {
    let directory = support::TestDirectory::new("invalid-model-restore-review");
    let mut supervisor = owner(&directory);
    let joint = supervisor.motors.motors[0].joint.clone();
    let generation = supervisor.reference_generation();
    let old_upper = supervisor
        .urdf_robot()
        .joints
        .iter()
        .find(|entry| entry.name == joint)
        .expect("model joint")
        .limit
        .upper;
    let old_soft_lower = supervisor
        .control
        .control
        .joints
        .get(&joint)
        .expect("control joint")
        .position_soft_lower_rad;
    let old_hard_upper = supervisor
        .joint_limit_policy(&joint)
        .expect("limit policy")
        .hard_upper();
    let mut replacement = supervisor.urdf_robot().clone();
    replacement
        .joints
        .iter_mut()
        .find(|entry| entry.name == joint)
        .expect("model joint")
        .limit
        .upper += 0.25;
    let mut invalid_control = supervisor.control.clone();
    invalid_control
        .control
        .joints
        .get_mut(&joint)
        .expect("control joint")
        .position_soft_lower_rad = Some(99.0);
    let result =
        supervisor.restore_limit_snapshot(supervisor.motors.clone(), invalid_control, replacement);
    let observed_upper = supervisor
        .urdf_robot()
        .joints
        .iter()
        .find(|entry| entry.name == joint)
        .expect("model joint")
        .limit
        .upper;
    let observed_soft_lower = supervisor
        .control
        .control
        .joints
        .get(&joint)
        .expect("control joint")
        .position_soft_lower_rad;
    println!("invalid restore={result:?}; model {old_upper}->{observed_upper}; soft_lower {old_soft_lower:?}->{observed_soft_lower:?}; reference={:?}; generation {generation}->{}", supervisor.joint_homing_state(&joint), supervisor.reference_generation());
    assert!(result.is_err(), "invalid control must reject");
    assert_eq!(
        observed_upper, old_upper,
        "invalid restore replaced private model before validation"
    );
    assert_eq!(
        observed_soft_lower, old_soft_lower,
        "invalid restore installed invalid policy"
    );
    assert_eq!(
        supervisor
            .joint_limit_policy(&joint)
            .expect("installed limit")
            .hard_upper(),
        old_hard_upper
    );
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Verified
    );
    assert_eq!(supervisor.reference_generation(), generation);
}

#[test]
fn successful_disabled_restore_remains_available_and_revokes_reference_control() {
    let directory = support::TestDirectory::new("valid-model-restore-review");
    let mut supervisor = owner(&directory);
    let joint = supervisor.motors.motors[0].joint.clone();
    let generation = supervisor.reference_generation();
    let before_tx = supervisor.bus().frames().to_vec();
    let mut replacement = supervisor.urdf_robot().clone();
    let model_joint = replacement
        .joints
        .iter_mut()
        .find(|entry| entry.name == joint)
        .expect("model joint");
    // Change the private model without changing the effective bench envelope.
    // This is a software fixture, not a physical limit update.
    model_joint.limit.upper -= 0.001;
    let restored_upper = model_joint.limit.upper;
    supervisor
        .restore_limit_snapshot(
            supervisor.motors.clone(),
            supervisor.control.clone(),
            replacement,
        )
        .expect("valid disabled restoration");
    assert_eq!(
        supervisor
            .urdf_robot()
            .joints
            .iter()
            .find(|entry| entry.name == joint)
            .expect("installed joint")
            .limit
            .upper,
        restored_upper
    );
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Unhomed
    );
    assert_eq!(supervisor.reference_generation(), generation + 1);
    assert_eq!(supervisor.bus().frames(), before_tx);
    supervisor.bus_mut().clear_trace();
    assert!(supervisor.enable_targets(&[joint]).is_err());
    assert!(supervisor.bus().frames().is_empty());
}
