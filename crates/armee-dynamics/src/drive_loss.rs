//! Offline gravity bound for single-drive-loss admission (ADR 0038).
//!
//! When one joint's drive is lost, it and every joint distal to it (its
//! subtree) are disabled and swing freely, while the joints proximal to it keep
//! holding with a gravity model whose subtree angles are frozen at their last
//! measured value. This module bounds, over a grid, what that costs each
//! holding joint: its full `τ_g` and the change of `τ_g` the frozen angles can
//! hide. It is a static bound: the swing of a free subtree adds dynamic load
//! that no gravity grid captures.

use crate::{DynamicsError, DynamicsModel};

/// Grid points across each shed joint's range (endpoints included).
pub const SHED_GRID_STEPS: usize = 7;
/// Grid points across each holding joint's range (endpoints included), plus
/// the rest pose (0 rad) when it lies inside the range.
pub const HOLDING_GRID_STEPS: usize = 4;
/// Refuse a grid larger than this many model evaluations instead of stalling.
pub const MAX_LOSS_GRID_EVALUATIONS: usize = 50_000;

/// Worst case over the grid for one joint that keeps holding.
#[derive(Debug, Clone, PartialEq)]
pub struct HoldingTorqueBound {
    /// Index of the holding joint in the model's joint order.
    pub index: usize,
    pub joint: String,
    /// Largest `|τ_g|` of this joint anywhere on the grid (Nm).
    pub max_abs_nm: f64,
    /// Largest `|τ_g(q_S) − τ_g(q_S')|` of this joint between two shed-subtree
    /// poses `q_S`, `q_S'` with the holding joints fixed: the model error a
    /// subtree frozen at one pose hides while it hangs at another (Nm).
    pub max_delta_nm: f64,
}

fn grid(lower: f64, upper: f64, steps: usize, rest: bool) -> Vec<f64> {
    let steps = steps.max(2);
    let mut points: Vec<f64> = (0..steps)
        .map(|i| lower + (upper - lower) * i as f64 / (steps - 1) as f64)
        .collect();
    if rest && lower < 0.0 && upper > 0.0 {
        points.push(0.0);
    }
    points
}

/// Advance a mixed-radix counter; false once every combination was visited.
fn next_combination(counter: &mut [usize], sizes: &[usize]) -> bool {
    for (digit, size) in counter.iter_mut().zip(sizes) {
        *digit += 1;
        if *digit < *size {
            return true;
        }
        *digit = 0;
    }
    false
}

/// Bound `τ_g` of every joint outside `shed` when the `shed` joints (indices in
/// the model's joint order) hang anywhere in their `ranges` while the others
/// sit anywhere on theirs. `ranges` holds one `(lower, upper)` per model joint.
///
/// Fails closed on a non-finite or inverted range, an out-of-range index, a
/// non-finite torque or a grid above [`MAX_LOSS_GRID_EVALUATIONS`].
pub fn subtree_loss_torque_bounds(
    model: &dyn DynamicsModel,
    shed: &[usize],
    ranges: &[(f64, f64)],
) -> Result<Vec<HoldingTorqueBound>, DynamicsError> {
    let names = model.joint_names();
    let n = names.len();
    if ranges.len() != n {
        return Err(DynamicsError::JointCount {
            expected: n,
            got: ranges.len(),
        });
    }
    if let Some(&index) = shed.iter().find(|&&index| index >= n) {
        return Err(DynamicsError::JointCount {
            expected: n,
            got: index + 1,
        });
    }
    if ranges
        .iter()
        .any(|(lower, upper)| !lower.is_finite() || !upper.is_finite() || lower > upper)
    {
        return Err(DynamicsError::NonFiniteInput {
            what: "drive-loss grid range",
        });
    }
    let holding: Vec<usize> = (0..n).filter(|index| !shed.contains(index)).collect();
    let shed_points: Vec<Vec<f64>> = shed
        .iter()
        .map(|&index| grid(ranges[index].0, ranges[index].1, SHED_GRID_STEPS, false))
        .collect();
    let holding_points: Vec<Vec<f64>> = holding
        .iter()
        .map(|&index| grid(ranges[index].0, ranges[index].1, HOLDING_GRID_STEPS, true))
        .collect();
    let shed_sizes: Vec<usize> = shed_points.iter().map(Vec::len).collect();
    let holding_sizes: Vec<usize> = holding_points.iter().map(Vec::len).collect();
    let evaluations = shed_sizes
        .iter()
        .chain(&holding_sizes)
        .try_fold(1_usize, |total, size| total.checked_mul(*size))
        .unwrap_or(usize::MAX);
    if evaluations > MAX_LOSS_GRID_EVALUATIONS {
        return Err(DynamicsError::LossGridTooLarge {
            evaluations,
            max: MAX_LOSS_GRID_EVALUATIONS,
        });
    }
    let mut bounds: Vec<HoldingTorqueBound> = holding
        .iter()
        .map(|&index| HoldingTorqueBound {
            index,
            joint: names[index].clone(),
            max_abs_nm: 0.0,
            max_delta_nm: 0.0,
        })
        .collect();
    let mut q = vec![0.0; n];
    let mut holding_counter = vec![0; holding.len()];
    loop {
        for (slot, &index) in holding.iter().enumerate() {
            q[index] = holding_points[slot][holding_counter[slot]];
        }
        // Per holding joint, min and max τ over the shed grid at this pose.
        let mut spread = vec![(f64::INFINITY, f64::NEG_INFINITY); holding.len()];
        let mut shed_counter = vec![0; shed.len()];
        loop {
            for (slot, &index) in shed.iter().enumerate() {
                q[index] = shed_points[slot][shed_counter[slot]];
            }
            let tau = model.gravity_torques(&q)?;
            for (slot, bound) in bounds.iter_mut().enumerate() {
                let value = tau[bound.index];
                if !value.is_finite() {
                    return Err(DynamicsError::NonFiniteTorque {
                        joint: bound.joint.clone(),
                    });
                }
                bound.max_abs_nm = bound.max_abs_nm.max(value.abs());
                let (low, high) = &mut spread[slot];
                *low = low.min(value);
                *high = high.max(value);
            }
            if !next_combination(&mut shed_counter, &shed_sizes) {
                break;
            }
        }
        for (bound, (low, high)) in bounds.iter_mut().zip(spread) {
            bound.max_delta_nm = bound.max_delta_nm.max(high - low);
        }
        if !next_combination(&mut holding_counter, &holding_sizes) {
            break;
        }
    }
    Ok(bounds)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::UrdfGravityModel;
    use armee_kinematics::fixtures;

    fn arm() -> UrdfGravityModel {
        let joints: Vec<String> = [
            "right_shoulder_pitch",
            "right_shoulder_roll",
            "right_upper_arm_yaw",
            "right_elbow_pitch",
            "right_lower_arm_yaw",
        ]
        .map(str::to_owned)
        .to_vec();
        UrdfGravityModel::from_urdf(fixtures::production_urdf(), &joints).expect("bench model")
    }

    #[test]
    fn subtree_follows_the_urdf_parent_chain() {
        let model = arm();
        assert_eq!(
            model.subtree_joints("right_lower_arm_yaw").expect("known"),
            vec![4]
        );
        assert_eq!(
            model.subtree_joints("right_elbow_pitch").expect("known"),
            vec![3, 4]
        );
        assert_eq!(
            model.subtree_joints("right_shoulder_pitch").expect("known"),
            vec![0, 1, 2, 3, 4]
        );
        assert!(matches!(
            model.subtree_joints("left_knee"),
            Err(DynamicsError::UnknownJoint { .. })
        ));
    }

    #[test]
    fn a_light_distal_subtree_hides_less_torque_than_the_forearm() {
        let model = arm();
        // Holding joints at one elevated pose; the shed joints swing through ±0.5 rad.
        let mut ranges = vec![(0.8, 0.8), (0.3, 0.3), (0.2, 0.2), (0.4, 0.4), (0.0, 0.0)];
        ranges[4] = (-0.5, 0.5);
        let hand = subtree_loss_torque_bounds(&model, &[4], &ranges).expect("bounded");
        assert_eq!(hand.len(), 4);
        ranges[3] = (-0.5, 0.5);
        let forearm = subtree_loss_torque_bounds(&model, &[3, 4], &ranges).expect("bounded");
        let pitch = |bounds: &[HoldingTorqueBound]| {
            bounds
                .iter()
                .find(|b| b.index == 0)
                .cloned()
                .expect("pitch holds")
        };
        let (hand, forearm) = (pitch(&hand), pitch(&forearm));
        assert!(forearm.max_delta_nm > 0.05, "{forearm:?}");
        assert!(
            hand.max_delta_nm < forearm.max_delta_nm / 5.0,
            "{hand:?} vs {forearm:?}"
        );
        assert!(forearm.max_abs_nm >= forearm.max_delta_nm / 2.0);
    }

    #[test]
    fn invalid_inputs_fail_closed() {
        let model = arm();
        assert!(matches!(
            subtree_loss_torque_bounds(&model, &[4], &[(-0.5, 0.5); 4]),
            Err(DynamicsError::JointCount { .. })
        ));
        assert!(matches!(
            subtree_loss_torque_bounds(&model, &[7], &[(-0.5, 0.5); 5]),
            Err(DynamicsError::JointCount { .. })
        ));
        let mut ranges = vec![(-0.5, 0.5); 5];
        ranges[2] = (0.5, -0.5);
        assert!(matches!(
            subtree_loss_torque_bounds(&model, &[4], &ranges),
            Err(DynamicsError::NonFiniteInput { .. })
        ));
        ranges[2] = (f64::NAN, 0.5);
        assert!(matches!(
            subtree_loss_torque_bounds(&model, &[4], &ranges),
            Err(DynamicsError::NonFiniteInput { .. })
        ));
    }
}
