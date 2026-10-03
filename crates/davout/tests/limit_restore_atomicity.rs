//! Existing-public restoration atomicity; ordinary constructor, no reference grants.
#![allow(clippy::expect_used)]

use davout::{MemoryBus, Supervisor};

fn supervisor() -> Supervisor<MemoryBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    Supervisor::from_repo(root, MemoryBus::default()).expect("ordinary unqualified owner")
}

fn snapshot(
    supervisor: &Supervisor<MemoryBus>,
) -> (String, String, String, Vec<davout::JointLimitPolicy>) {
    (
        format!("{:?}", supervisor.motors),
        format!("{:?}", supervisor.control),
        format!("{:?}", supervisor.urdf_robot()),
        supervisor
            .motors
            .motors
            .iter()
            .map(|motor| {
                *supervisor
                    .joint_limit_policy(&motor.joint)
                    .expect("installed policy")
            })
            .collect(),
    )
}

#[test]
fn rejected_numeric_restore_preserves_all_installed_state() {
    let mut supervisor = supervisor();
    let before = snapshot(&supervisor);
    let mut motors = supervisor.motors.clone();
    let mut control = supervisor.control.clone();
    let mut model = supervisor.urdf_robot().clone();
    motors.motors[0].direction = 0;
    control
        .control
        .joints
        .get_mut(&motors.motors[0].joint)
        .expect("control")
        .gravity_comp
        .kp += 0.5;
    model.joints[0].limit.upper += 0.25;
    assert!(
        supervisor
            .restore_limit_snapshot(motors, control, model)
            .is_err(),
        "invalid numeric policy was accepted"
    );
    assert!(
        snapshot(&supervisor) == before,
        "rejected numeric restore partially installed motors/control/model/derived limits"
    );
}

#[test]
fn rejected_model_restore_preserves_all_installed_state() {
    let mut supervisor = supervisor();
    let before = snapshot(&supervisor);
    let motors = supervisor.motors.clone();
    let mut control = supervisor.control.clone();
    let mut model = supervisor.urdf_robot().clone();
    let joint = &motors.motors[0].joint;
    control
        .control
        .joints
        .get_mut(joint)
        .expect("control")
        .gravity_comp
        .kp += 0.5;
    model.joints.retain(|entry| &entry.name != joint);
    assert!(
        supervisor
            .restore_limit_snapshot(motors, control, model)
            .is_err(),
        "incomplete model was accepted"
    );
    assert!(
        snapshot(&supervisor) == before,
        "rejected missing-joint model partially installed control/model/derived limits"
    );
}
