//! Candidate-interface regression probe; not an unchanged c306 behavioral test.
#![allow(clippy::expect_used)]

use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{DavoutError, JointHomingState, Supervisor};

#[test]
fn rejected_active_restore_cannot_replace_private_installed_model() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
    )
    .expect("closed virtual initial reference");
    let joint = supervisor.motors.motors[0].joint.clone();
    supervisor
        .enable_targets(std::slice::from_ref(&joint))
        .expect("active owner");
    let generation = supervisor.reference_generation();
    let stop_generation = supervisor.stop_generation();
    let before_policy = (
        format!("{:?}", supervisor.motors),
        format!("{:?}", supervisor.control),
    );
    let before_limits = *supervisor
        .joint_limit_policy(&joint)
        .expect("installed limits");
    let before_tx = supervisor.bus().frames().to_vec();
    let mut replacement = supervisor.urdf_robot().clone();
    let original_upper = replacement
        .joints
        .iter()
        .find(|entry| entry.name == joint)
        .expect("model joint")
        .limit
        .upper;
    replacement
        .joints
        .iter_mut()
        .find(|entry| entry.name == joint)
        .expect("model joint")
        .limit
        .upper += 0.25;
    let mut changed_motors = supervisor.motors.clone();
    changed_motors.motors[0].direction *= -1;
    let mut changed_control = supervisor.control.clone();
    changed_control
        .control
        .joints
        .get_mut(&joint)
        .expect("control")
        .gravity_comp
        .kp += 0.5;
    let result = supervisor.restore_limit_snapshot(changed_motors, changed_control, replacement);
    let observed_upper = supervisor
        .urdf_robot()
        .joints
        .iter()
        .find(|entry| entry.name == joint)
        .expect("installed model joint")
        .limit
        .upper;
    println!("restore={result:?}; original_upper={original_upper}; installed_upper={observed_upper}; reference={:?}; generation_before={generation}; generation_after={}", supervisor.joint_homing_state(&joint), supervisor.reference_generation());
    assert!(matches!(result, Err(DavoutError::LimitPatchActive)));
    assert_eq!(
        supervisor.joint_homing_state(&joint),
        JointHomingState::Verified
    );
    assert_eq!(supervisor.reference_generation(), generation);
    assert_eq!(
        observed_upper, original_upper,
        "rejected Active restore installed a different private model under an intact reference"
    );
    assert_eq!(
        (
            format!("{:?}", supervisor.motors),
            format!("{:?}", supervisor.control)
        ),
        before_policy
    );
    assert_eq!(
        *supervisor
            .joint_limit_policy(&joint)
            .expect("installed limits"),
        before_limits
    );
    assert_eq!(supervisor.mode(), davout::OperationalMode::Active);
    assert_eq!(supervisor.stop_generation(), stop_generation);
    assert_eq!(supervisor.bus().frames(), before_tx);
}
