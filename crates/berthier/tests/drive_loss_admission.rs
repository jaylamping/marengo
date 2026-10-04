//! Offline gravity-bound admission of `on_drive_loss: shed_subtree` (ADR 0038).

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use berthier::{ControlLoop, LoopError};
use davout::simulation::{InitialVirtualReference, SimulationBus};
use marengo_config::{load_control_config_from, write_control_config_from, OnDriveLoss};

mod support;
use support::FixtureTree;

const JOINTS: [&str; 5] = [
    "right_shoulder_pitch",
    "right_shoulder_roll",
    "right_upper_arm_yaw",
    "right_elbow_pitch",
    "right_lower_arm_yaw",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn controller(root: &Path) -> ControlLoop<SimulationBus> {
    ControlLoop::from_simulation(
        root,
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        25,
    )
    .expect("simulated controller")
}

/// A copy of the master tree where exactly `shedding` use `shed_subtree`,
/// with the admission margin set to `margin_nm`.
fn tree(label: &str, shedding: &[&str], margin_nm: f64) -> FixtureTree {
    let tree = FixtureTree::new(label, &repo_root());
    let config = tree.path().join("config");
    let mut control = load_control_config_from(&config).expect("copied control");
    for (joint, entry) in control.control.joints.iter_mut() {
        entry.on_drive_loss = if shedding.contains(&joint.as_str()) {
            OnDriveLoss::ShedSubtree
        } else {
            OnDriveLoss::DisableAll
        };
    }
    control
        .control
        .drive_loss
        .as_mut()
        .expect("master drive_loss block")
        .tau_margin_nm = margin_nm;
    write_control_config_from(&config, &control).expect("write fixture control");
    tree
}

#[test]
fn master_config_admits_its_shed_subtree_joints_and_installs_their_plans() {
    let root = repo_root();
    let master = load_control_config_from(root.join("config")).expect("master control");
    let shedding: Vec<&str> = JOINTS
        .into_iter()
        .filter(|joint| master.control.joints[*joint].on_drive_loss == OnDriveLoss::ShedSubtree)
        .collect();
    assert!(
        shedding.contains(&"right_lower_arm_yaw"),
        "the distal yaw is admitted on the master config"
    );
    let mut controller = controller(&root);
    let admitted = controller
        .admit_drive_loss()
        .expect("master admission passes");
    let joints: Vec<&str> = admitted.iter().map(|a| a.plan.joint.as_str()).collect();
    assert_eq!(joints, shedding);
    for admission in &admitted {
        assert_eq!(admission.plan.shed.first(), Some(&admission.plan.joint));
        for bound in &admission.bounds {
            eprintln!(
                "{} lost: {} holds with max |tau_g| {:.3} Nm, max |dtau_g| {:.3} Nm",
                admission.plan.joint, bound.joint, bound.max_abs_nm, bound.max_delta_nm
            );
        }
    }
    let plans = controller.supervisor().drive_loss_plans();
    assert_eq!(plans.len(), admitted.len());
    let elbow = plans.iter().find(|plan| plan.joint == "right_elbow_pitch");
    if let Some(elbow) = elbow {
        assert_eq!(elbow.shed, ["right_elbow_pitch", "right_lower_arm_yaw"]);
    }
}

#[test]
fn a_subtree_whose_gravity_bound_fails_is_refused() {
    // Losing roll drops the whole arm below it onto shoulder pitch.
    let tree = tree("drive-loss-roll", &["right_shoulder_roll"], 0.5);
    let error = controller(tree.path())
        .admit_drive_loss()
        .expect_err("roll fails the bound");
    assert!(
        matches!(&error, LoopError::DriveLossAdmission { joint, message }
            if joint == "right_shoulder_roll" && message.contains("right_shoulder_pitch")),
        "{error}"
    );
}

#[test]
fn a_margin_beyond_every_cap_refuses_even_the_distal_yaw() {
    let tree = tree("drive-loss-margin", &["right_lower_arm_yaw"], 10.0);
    let error = controller(tree.path())
        .admit_drive_loss()
        .expect_err("no cap leaves 10 Nm of margin");
    assert!(
        matches!(&error, LoopError::DriveLossAdmission { joint, .. } if joint == "right_lower_arm_yaw"),
        "{error}"
    );
}

#[test]
fn losing_the_root_joint_leaves_nothing_to_hold_and_is_refused() {
    let tree = tree("drive-loss-pitch", &["right_shoulder_pitch"], 0.5);
    let error = controller(tree.path())
        .admit_drive_loss()
        .expect_err("pitch sheds the whole arm");
    assert!(
        matches!(&error, LoopError::DriveLossAdmission { joint, message }
            if joint == "right_shoulder_pitch" && message.contains("nothing would hold")),
        "{error}"
    );
}
