//! Model load must fail closed (audit leads L-armee-dynamics-01..05, prior review M05):
//! no silent q = 0 for unlisted joints, no silent skips of fixed/prismatic/duplicate joints,
//! no NaN from a zero axis, no hang on a cyclic chain.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use armee_dynamics::{
    gravity_model_from_urdf, max_gravity_torque_over_range, DynamicsError, DynamicsModel,
    UrdfGravityModel,
};

fn write_urdf(body: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let path = dir.join(format!(
        "model-validation-{}-{}.urdf",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, format!(r#"<robot name="t">{body}</robot>"#)).expect("write urdf");
    path
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| (*n).to_owned()).collect()
}

fn link(name: &str, mass: &str) -> String {
    format!(
        r#"<link name="{name}"><inertial><origin xyz="0 0 -0.5"/><mass value="{mass}"/>
        <inertia ixx="1" ixy="0" ixz="0" iyy="1" iyz="0" izz="1"/></inertial></link>"#
    )
}

fn joint(name: &str, kind: &str, parent: &str, child: &str, extra: &str) -> String {
    format!(
        r#"<joint name="{name}" type="{kind}"><parent link="{parent}"/><child link="{child}"/>
        <origin xyz="0 0 0"/>{extra}</joint>"#
    )
}

const AXIS_Y: &str = r#"<axis xyz="0 1 0"/><limit lower="-1" upper="1" effort="1" velocity="1"/>"#;

/// Two revolute joints in a chain: base -> upper -> lower.
fn two_joint(shoulder_extra: &str, elbow_kind: &str, elbow_extra: &str) -> PathBuf {
    write_urdf(&format!(
        "{}{}{}{}{}",
        link("base", "0"),
        link("upper", "2"),
        link("lower", "1"),
        joint("shoulder", "revolute", "base", "upper", shoulder_extra),
        joint("elbow", elbow_kind, "upper", "lower", elbow_extra),
    ))
}

#[test]
fn unlisted_actuated_joint_is_refused_not_evaluated_at_zero() {
    let path = two_joint(AXIS_Y, "revolute", AXIS_Y);
    let err = match gravity_model_from_urdf(&path, &names(&["shoulder"])) {
        Ok(_) => panic!("elbow is actuated but not listed; the model must refuse"),
        Err(e) => e,
    };
    assert!(
        matches!(&err, DynamicsError::UnmodelledJoint { joint } if joint == "elbow"),
        "{err}"
    );
    // Full list loads.
    gravity_model_from_urdf(&path, &names(&["shoulder", "elbow"])).expect("complete list");
}

#[test]
fn live_urdf_with_a_joint_missing_from_robot_yaml_is_refused() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let robot = marengo_config::load_robot_config_from(&root.join("config")).expect("robot.yaml");
    let urdf = root.join(&robot.robot.urdf);
    let mut joints = robot.robot.joints.clone();
    joints.retain(|j| j != "right_elbow_pitch");
    assert!(gravity_model_from_urdf(&urdf, &joints).is_err());
    gravity_model_from_urdf(&urdf, &robot.robot.joints).expect("live list loads");
}

#[test]
fn duplicate_and_fixed_names_are_refused() {
    let path = two_joint(AXIS_Y, "revolute", AXIS_Y);
    let err = match gravity_model_from_urdf(&path, &names(&["shoulder", "elbow", "shoulder"])) {
        Ok(_) => panic!("duplicate joint name must be refused"),
        Err(e) => e,
    };
    assert!(
        matches!(&err, DynamicsError::DuplicateJoint { joint } if joint == "shoulder"),
        "{err}"
    );

    let fixed = two_joint(AXIS_Y, "fixed", "");
    let err = match gravity_model_from_urdf(&fixed, &names(&["shoulder", "elbow"])) {
        Ok(_) => panic!("a fixed joint in the model list must be refused"),
        Err(e) => e,
    };
    assert!(
        matches!(&err, DynamicsError::NotActuated { joint, .. } if joint == "elbow"),
        "{err}"
    );
}

#[test]
fn prismatic_and_mimic_joints_are_refused() {
    let prismatic = two_joint(AXIS_Y, "prismatic", AXIS_Y);
    let err = match gravity_model_from_urdf(&prismatic, &names(&["shoulder", "elbow"])) {
        Ok(_) => panic!("prismatic joints are not representable"),
        Err(e) => e,
    };
    assert!(
        matches!(err, DynamicsError::UnsupportedJoint { .. }),
        "{err}"
    );

    let mimic = two_joint(
        AXIS_Y,
        "revolute",
        &format!(r#"{AXIS_Y}<mimic joint="shoulder" multiplier="1"/>"#),
    );
    let err = match gravity_model_from_urdf(&mimic, &names(&["shoulder", "elbow"])) {
        Ok(_) => panic!("mimic joints are not representable"),
        Err(e) => e,
    };
    assert!(
        matches!(err, DynamicsError::UnsupportedJoint { .. }),
        "{err}"
    );
}

#[test]
fn zero_axis_is_refused_instead_of_producing_nan() {
    let path = two_joint(
        r#"<axis xyz="0 0 0"/><limit lower="-1" upper="1" effort="1" velocity="1"/>"#,
        "revolute",
        AXIS_Y,
    );
    let err = match gravity_model_from_urdf(&path, &names(&["shoulder", "elbow"])) {
        Ok(_) => panic!("zero axis must be refused"),
        Err(e) => e,
    };
    assert!(matches!(err, DynamicsError::InvalidModel { .. }), "{err}");
}

#[test]
fn non_finite_inertial_is_refused() {
    for mass in ["NaN", "-1.0", "inf"] {
        let path = write_urdf(&format!(
            "{}{}{}",
            link("base", "0"),
            link("upper", mass),
            joint("shoulder", "revolute", "base", "upper", AXIS_Y),
        ));
        assert!(
            gravity_model_from_urdf(&path, &names(&["shoulder"])).is_err(),
            "mass {mass}"
        );
    }
}

#[test]
fn cyclic_chain_is_refused_not_hung() {
    // a -> b and b -> a: every link has a parent joint, so a root-ward walk never ends.
    let path = write_urdf(&format!(
        "{}{}{}{}",
        link("a", "1"),
        link("b", "1"),
        joint("ab", "revolute", "a", "b", AXIS_Y),
        joint("ba", "revolute", "b", "a", AXIS_Y),
    ));
    let err = match gravity_model_from_urdf(&path, &names(&["ab", "ba"])) {
        Ok(_) => panic!("cycle must be refused"),
        Err(e) => e,
    };
    assert!(matches!(err, DynamicsError::CyclicChain { .. }), "{err}");
}

#[test]
fn held_joints_are_explicit_and_validated() {
    let path = two_joint(AXIS_Y, "revolute", AXIS_Y);
    let model = UrdfGravityModel::from_urdf_with_held(
        &path,
        &names(&["shoulder"]),
        &[("elbow".to_owned(), 0.7)],
    )
    .expect("held elbow");
    let full = gravity_model_from_urdf(&path, &names(&["shoulder", "elbow"])).expect("full");
    let held = model.gravity_torques(&[0.3]).expect("held tau");
    let reference = full.gravity_torques(&[0.3, 0.7]).expect("full tau");
    assert!((held[0] - reference[0]).abs() < 1e-9);
    // Unknown, duplicated or non-finite holds are refused.
    for held in [
        vec![("nope".to_owned(), 0.0)],
        vec![("shoulder".to_owned(), 0.0), ("elbow".to_owned(), 0.0)],
        vec![("elbow".to_owned(), f64::NAN)],
    ] {
        assert!(
            UrdfGravityModel::from_urdf_with_held(&path, &names(&["shoulder"]), &held).is_err()
        );
    }
}

#[test]
fn non_finite_pose_is_an_error_not_a_nan_torque() {
    let path = two_joint(AXIS_Y, "revolute", AXIS_Y);
    let model = gravity_model_from_urdf(&path, &names(&["shoulder", "elbow"])).expect("model");
    assert!(matches!(
        model.gravity_torques(&[f64::NAN, 0.0]),
        Err(DynamicsError::NonFiniteInput { .. })
    ));
    assert!(max_gravity_torque_over_range(&model, 0, f64::NAN, 1.0, 5).is_err());
    assert!(max_gravity_torque_over_range(&model, 0, 0.0, f64::INFINITY, 5).is_err());
}
