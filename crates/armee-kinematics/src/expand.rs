//! Expand-only URDF hard envelopes (bench Set Limits).

use urdf_rs::JointType;

use crate::UrdfError;

/// Expand-only: widen a joint's URDF `<limit>` so it covers `[hard_lower, hard_upper]`.
///
/// Never shrinks. Returns `true` if the in-memory robot was mutated.
pub fn expand_urdf_joint_hard(
    robot: &mut urdf_rs::Robot,
    joint_name: &str,
    hard_lower: f64,
    hard_upper: f64,
) -> Result<bool, UrdfError> {
    if !hard_lower.is_finite() || !hard_upper.is_finite() || hard_lower >= hard_upper {
        return Err(UrdfError::Read {
            path: joint_name.to_string(),
            message: format!("invalid hard envelope [{hard_lower}, {hard_upper}] for expand"),
        });
    }
    let joint = robot
        .joints
        .iter_mut()
        .find(|j| j.name == joint_name)
        .ok_or_else(|| UrdfError::Read {
            path: joint_name.to_string(),
            message: "joint not found".to_string(),
        })?;
    if !matches!(
        joint.joint_type,
        JointType::Revolute | JointType::Continuous | JointType::Prismatic
    ) {
        return Err(UrdfError::Read {
            path: joint_name.to_string(),
            message: "joint is not actuated".to_string(),
        });
    }

    let new_lower = hard_lower.min(joint.limit.lower);
    let new_upper = hard_upper.max(joint.limit.upper);

    // Keep safety_controller soft inside the new hard. Soft bounds that cannot be made
    // consistent (inverted, or wholly outside hard) are an operator-visible URDF fault:
    // resetting them to the full hard range would silently widen the commandable envelope.
    // Validate before mutating anything so a refusal leaves the robot untouched.
    let soft = joint.safety_controller.as_ref().map(|s| {
        (
            (s.soft_lower_limit, s.soft_upper_limit),
            (
                s.soft_lower_limit.max(new_lower),
                s.soft_upper_limit.min(new_upper),
            ),
        )
    });
    if let Some(((raw_lower, raw_upper), (soft_lower, soft_upper))) = soft {
        if !raw_lower.is_finite() || !raw_upper.is_finite() || soft_lower > soft_upper {
            return Err(UrdfError::Read {
                path: joint_name.to_string(),
                message: format!(
                    "safety_controller soft [{raw_lower}, {raw_upper}] is inconsistent with hard [{new_lower}, {new_upper}]; fix the URDF soft bounds before expanding"
                ),
            });
        }
    }

    let mut changed = false;
    if new_lower < joint.limit.lower {
        joint.limit.lower = new_lower;
        changed = true;
    }
    if new_upper > joint.limit.upper {
        joint.limit.upper = new_upper;
        changed = true;
    }
    if let (Some(safety), Some((_, (soft_lower, soft_upper)))) =
        (joint.safety_controller.as_mut(), soft)
    {
        if safety.soft_lower_limit != soft_lower {
            safety.soft_lower_limit = soft_lower;
            changed = true;
        }
        if safety.soft_upper_limit != soft_upper {
            safety.soft_upper_limit = soft_upper;
            changed = true;
        }
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::{fixtures, load_urdf};

    #[test]
    fn expands_lower_and_upper_only_outward() {
        let mut robot = load_urdf(fixtures::arm_4dof_right_urdf()).expect("urdf");
        let before_lo = robot
            .joints
            .iter()
            .find(|j| j.name == "right_elbow_pitch")
            .expect("joint")
            .limit
            .lower;
        assert!(
            expand_urdf_joint_hard(&mut robot, "right_elbow_pitch", -0.8, 0.5).expect("expand")
        );
        let joint = robot
            .joints
            .iter()
            .find(|j| j.name == "right_elbow_pitch")
            .expect("joint");
        assert!((joint.limit.lower - (-0.8)).abs() < 1e-12);
        assert!(joint.limit.lower <= before_lo);
        assert!(joint.limit.upper >= 0.5);
    }

    #[test]
    fn never_shrinks() {
        let mut robot = load_urdf(fixtures::arm_4dof_urdf()).expect("urdf");
        let before = robot
            .joints
            .iter()
            .find(|j| j.name == "elbow")
            .expect("joint")
            .limit
            .clone();
        assert!(!expand_urdf_joint_hard(&mut robot, "elbow", 0.5, 1.0).expect("noop"));
        let after = &robot
            .joints
            .iter()
            .find(|j| j.name == "elbow")
            .expect("joint")
            .limit;
        assert_eq!(after.lower, before.lower);
        assert_eq!(after.upper, before.upper);
    }

    #[test]
    fn expands_past_current_hard() {
        let mut robot = load_urdf(fixtures::arm_4dof_urdf()).expect("urdf");
        assert!(expand_urdf_joint_hard(&mut robot, "elbow", -0.5, 3.0).expect("expand"));
        let joint = robot
            .joints
            .iter()
            .find(|j| j.name == "elbow")
            .expect("joint");
        assert!((joint.limit.lower - (-0.5)).abs() < 1e-12);
        assert!((joint.limit.upper - 3.0).abs() < 1e-12);
    }

    fn robot_with_soft(soft: &str) -> urdf_rs::Robot {
        urdf_rs::read_from_string(&format!(
            r#"<robot name="t">
              <link name="a"/><link name="b"/><link name="c"/>
              <joint name="j" type="revolute">
                <parent link="a"/><child link="b"/><axis xyz="0 1 0"/>
                <limit lower="0.0" upper="1.0" effort="1" velocity="1"/>{soft}
              </joint>
              <joint name="f" type="fixed"><parent link="b"/><child link="c"/></joint>
            </robot>"#
        ))
        .expect("test urdf")
    }

    fn limit_and_soft(robot: &urdf_rs::Robot) -> ((f64, f64), Option<(f64, f64)>) {
        let j = robot.joints.iter().find(|j| j.name == "j").expect("joint");
        (
            (j.limit.lower, j.limit.upper),
            j.safety_controller
                .as_ref()
                .map(|s| (s.soft_lower_limit, s.soft_upper_limit)),
        )
    }

    #[test]
    fn inverted_soft_refuses_without_widening_or_mutating() {
        // soft wholly above the hard range: clamping into hard inverts it. The old code reset
        // soft to the full hard range, erasing the operator's bounds and widening the envelope.
        let mut robot = robot_with_soft(
            r#"<safety_controller soft_lower_limit="5.0" soft_upper_limit="6.0"/>"#,
        );
        let before = limit_and_soft(&robot);
        let err = expand_urdf_joint_hard(&mut robot, "j", -0.5, 2.0).expect_err("refuse");
        assert!(err.to_string().contains("inconsistent"), "{err}");
        assert_eq!(limit_and_soft(&robot), before);
    }

    #[test]
    fn non_finite_soft_refuses() {
        let mut robot = robot_with_soft(
            r#"<safety_controller soft_lower_limit="NaN" soft_upper_limit="0.5"/>"#,
        );
        assert!(expand_urdf_joint_hard(&mut robot, "j", -0.5, 2.0).is_err());
    }

    #[test]
    fn consistent_soft_is_kept_and_clamped_into_hard() {
        let mut robot = robot_with_soft(
            r#"<safety_controller soft_lower_limit="0.2" soft_upper_limit="0.8"/>"#,
        );
        assert!(expand_urdf_joint_hard(&mut robot, "j", -0.5, 2.0).expect("expand"));
        let (hard, soft) = limit_and_soft(&robot);
        assert_eq!(hard, (-0.5, 2.0));
        assert_eq!(soft, Some((0.2, 0.8)));
    }

    #[test]
    fn invalid_envelope_and_unactuated_joint_are_refused() {
        let mut robot = robot_with_soft("");
        for (lo, hi) in [
            (1.0, 0.0),
            (0.5, 0.5),
            (f64::NAN, 1.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(expand_urdf_joint_hard(&mut robot, "j", lo, hi).is_err());
        }
        let err = expand_urdf_joint_hard(&mut robot, "f", -1.0, 1.0).expect_err("fixed");
        assert!(err.to_string().contains("not actuated"), "{err}");
        assert!(expand_urdf_joint_hard(&mut robot, "missing", -1.0, 1.0).is_err());
    }
}
