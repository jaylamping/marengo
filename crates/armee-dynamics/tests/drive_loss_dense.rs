//! The ADR 0038 admission grid samples τ_g; it is not a bound (review CFG-02). This pins
//! that every `shed_subtree` joint in the master config still passes the admission
//! inequality when the same quantities are sampled on a much denser grid.

#![allow(clippy::expect_used)]

use std::path::Path;

use armee_dynamics::drive_loss::subtree_loss_torque_bounds;
use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use marengo_config::{
    load_control_config_from, load_motors_config_from, load_robot_config_from, motor_for_joint,
    motor_type_key, OnDriveLoss,
};

/// Points per shed joint and per holding joint on the dense grid (the admission grid uses
/// 7 and 4 + rest).
const DENSE_SHED: usize = 9;
const DENSE_HOLDING: usize = 7;

fn linspace(lower: f64, upper: f64, steps: usize) -> Vec<f64> {
    (0..steps)
        .map(|i| lower + (upper - lower) * i as f64 / (steps - 1) as f64)
        .collect()
}

fn next(counter: &mut [usize], size: usize) -> bool {
    for digit in counter.iter_mut() {
        *digit += 1;
        if *digit < size {
            return true;
        }
        *digit = 0;
    }
    false
}

/// Dense-grid `max |τ_g,h| + max |Δτ_g,h|` for every holding joint h.
fn dense_need(model: &UrdfGravityModel, shed: &[usize], ranges: &[(f64, f64)]) -> Vec<f64> {
    let n = ranges.len();
    let holding: Vec<usize> = (0..n).filter(|index| !shed.contains(index)).collect();
    let shed_points: Vec<Vec<f64>> = shed
        .iter()
        .map(|&i| linspace(ranges[i].0, ranges[i].1, DENSE_SHED))
        .collect();
    let holding_points: Vec<Vec<f64>> = holding
        .iter()
        .map(|&i| linspace(ranges[i].0, ranges[i].1, DENSE_HOLDING))
        .collect();
    let mut max_abs = vec![0.0_f64; n];
    let mut max_delta = vec![0.0_f64; n];
    let mut q = vec![0.0; n];
    let mut hold = vec![0; holding.len()];
    loop {
        for (slot, &index) in holding.iter().enumerate() {
            q[index] = holding_points[slot][hold[slot]];
        }
        let mut spread = vec![(f64::INFINITY, f64::NEG_INFINITY); n];
        let mut swing = vec![0; shed.len()];
        loop {
            for (slot, &index) in shed.iter().enumerate() {
                q[index] = shed_points[slot][swing[slot]];
            }
            let tau = model.gravity_torques(&q).expect("finite pose");
            for &h in &holding {
                max_abs[h] = max_abs[h].max(tau[h].abs());
                spread[h] = (spread[h].0.min(tau[h]), spread[h].1.max(tau[h]));
            }
            if !next(&mut swing, DENSE_SHED) {
                break;
            }
        }
        for &h in &holding {
            max_delta[h] = max_delta[h].max(spread[h].1 - spread[h].0);
        }
        if !next(&mut hold, DENSE_HOLDING) {
            break;
        }
    }
    (0..n).map(|h| max_abs[h] + max_delta[h]).collect()
}

#[test]
#[ignore = "about 85k model evaluations, 150 s in a debug build; run with --release -- --ignored"]
fn master_shed_subtree_joints_pass_admission_on_a_dense_grid() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = root.join("config");
    let robot = load_robot_config_from(&config).expect("robot.yaml");
    let motors = load_motors_config_from(&config).expect("motors.yaml");
    let control = load_control_config_from(&config)
        .expect("control.yaml")
        .control;
    let joints = robot.robot.joints.clone();
    let model =
        UrdfGravityModel::from_urdf(root.join(&robot.robot.urdf), &joints).expect("master model");
    let urdf = armee_kinematics::load_urdf(root.join(&robot.robot.urdf)).expect("urdf");

    let mut ranges = Vec::new();
    let mut caps = Vec::new();
    for joint in &joints {
        let entry = &control.joints[joint];
        let motor = motor_for_joint(&motors, joint).expect("motor");
        let effort = armee_kinematics::joint_limits(&urdf, joint)
            .expect("limits")
            .effort;
        let type_cap = control.motor_type_defaults[motor_type_key(entry.motor_type)].tau_ff_max_nm;
        ranges.push((
            entry
                .position_soft_lower_rad
                .unwrap_or(motor.bench.position_lower_rad),
            entry
                .position_soft_upper_rad
                .unwrap_or(motor.bench.position_upper_rad),
        ));
        caps.push(effort.min(motor.bench.torque_limit_nm).min(type_cap));
    }
    let margin = control
        .drive_loss
        .as_ref()
        .map_or(0.0, |drive_loss| drive_loss.tau_margin_nm);

    let mut checked = 0;
    for joint in &joints {
        if control.joints[joint].on_drive_loss != OnDriveLoss::ShedSubtree {
            continue;
        }
        let shed = model.subtree_joints(joint).expect("subtree");
        let sampled = subtree_loss_torque_bounds(&model, &shed, &ranges).expect("admission grid");
        let dense = dense_need(&model, &shed, &ranges);
        for bound in &sampled {
            let h = bound.index;
            let grid_need = bound.max_abs_nm + bound.max_delta_nm;
            eprintln!(
                "{joint} lost: {} grid {grid_need:.3} Nm, dense {:.3} Nm, cap {:.3} Nm",
                bound.joint, dense[h], caps[h]
            );
            assert!(
                dense[h] + margin <= caps[h],
                "{joint} lost: holding {} needs {:.3} + margin {margin:.3} Nm on the dense grid, \
                 above its {:.3} Nm cap",
                bound.joint,
                dense[h],
                caps[h]
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "master config has no shed_subtree joint");
}
