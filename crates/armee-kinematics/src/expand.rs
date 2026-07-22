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
            message: format!(
                "invalid hard envelope [{hard_lower}, {hard_upper}] for expand"
            ),
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

    let mut changed = false;
    if hard_lower < joint.limit.lower {
        joint.limit.lower = hard_lower;
        changed = true;
    }
    if hard_upper > joint.limit.upper {
        joint.limit.upper = hard_upper;
        changed = true;
    }

    // Keep safety_controller soft inside the new hard (expand soft outward only when
    // it would otherwise sit outside hard after a hard expand).
    if let Some(soft) = joint.safety_controller.as_mut() {
        if soft.soft_lower_limit < joint.limit.lower {
            soft.soft_lower_limit = joint.limit.lower;
            changed = true;
        }
        if soft.soft_upper_limit > joint.limit.upper {
            soft.soft_upper_limit = joint.limit.upper;
            changed = true;
        }
        if soft.soft_lower_limit > soft.soft_upper_limit {
            soft.soft_lower_limit = joint.limit.lower;
            soft.soft_upper_limit = joint.limit.upper;
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
        assert!(expand_urdf_joint_hard(&mut robot, "right_elbow_pitch", -0.8, 0.5)
            .expect("expand"));
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
}
