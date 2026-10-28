//! Independent exact-gravity oracle on the live master URDF (review CFG-10, prior L13).
//!
//! The library differentiates the potential energy numerically. This oracle uses the exact
//! one-pass formula instead, with its own forward kinematics and explicit rotation matrices:
//!
//! τ_i = −a_i · ((Σ_{k ∈ sub(i)} m_k c_k − m_sub(i) o_i) × g)
//!
//! (a_i, o_i: joint axis and origin in the world; c_k: link COM). Agreement over many poses
//! (about 1e-9 Nm today) pins the COM-as-a-point and axis-in-joint-frame conventions on the
//! live arm, and is the acceptance check for replacing the numerical gradient with the exact
//! formula. The master's joint rpy are all zero; `rotated-pendulum.urdf` covers rpy.

#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::path::Path;

use armee_dynamics::{gravity_model_from_urdf, DynamicsModel};
use nalgebra::{Matrix3, Vector3};

const G: Vector3<f64> = Vector3::new(0.0, 0.0, -9.81);

fn rot_x(a: f64) -> Matrix3<f64> {
    let (s, c) = a.sin_cos();
    Matrix3::new(1.0, 0.0, 0.0, 0.0, c, -s, 0.0, s, c)
}

fn rot_y(a: f64) -> Matrix3<f64> {
    let (s, c) = a.sin_cos();
    Matrix3::new(c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c)
}

fn rot_z(a: f64) -> Matrix3<f64> {
    let (s, c) = a.sin_cos();
    Matrix3::new(c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0)
}

/// Rodrigues rotation by `angle` about unit `axis`.
fn rot_axis(axis: &Vector3<f64>, angle: f64) -> Matrix3<f64> {
    let k = Matrix3::new(
        0.0, -axis.z, axis.y, axis.z, 0.0, -axis.x, -axis.y, axis.x, 0.0,
    );
    Matrix3::identity() + k * angle.sin() + k * k * (1.0 - angle.cos())
}

fn vec3(v: [f64; 3]) -> Vector3<f64> {
    Vector3::new(v[0], v[1], v[2])
}

/// Exact τ_g for the joints in `names` at pose `q`.
fn oracle(robot: &urdf_rs::Robot, names: &[String], q: &[f64]) -> Vec<f64> {
    let angle: HashMap<&str, f64> = names.iter().map(String::as_str).zip(q.to_vec()).collect();
    let parent_joint: HashMap<&str, &urdf_rs::Joint> = robot
        .joints
        .iter()
        .map(|joint| (joint.child.link.as_str(), joint))
        .collect();
    // World pose (R, p) of each link frame, and per joint its world axis and origin.
    let mut frame: HashMap<&str, (Matrix3<f64>, Vector3<f64>)> = HashMap::new();
    let mut axis_origin: HashMap<&str, (Vector3<f64>, Vector3<f64>)> = HashMap::new();
    fn resolve<'a>(
        link: &'a str,
        parent_joint: &HashMap<&'a str, &'a urdf_rs::Joint>,
        angle: &HashMap<&str, f64>,
        frame: &mut HashMap<&'a str, (Matrix3<f64>, Vector3<f64>)>,
        axis_origin: &mut HashMap<&'a str, (Vector3<f64>, Vector3<f64>)>,
    ) -> (Matrix3<f64>, Vector3<f64>) {
        if let Some(pose) = frame.get(link) {
            return *pose;
        }
        let pose = match parent_joint.get(link) {
            None => (Matrix3::identity(), Vector3::zeros()),
            Some(joint) => {
                let (r_parent, p_parent) =
                    resolve(&joint.parent.link, parent_joint, angle, frame, axis_origin);
                let rpy = joint.origin.rpy.0;
                // URDF fixed-axis roll-pitch-yaw: R = Rz(yaw) Ry(pitch) Rx(roll).
                let r_joint = r_parent * rot_z(rpy[2]) * rot_y(rpy[1]) * rot_x(rpy[0]);
                let p_joint = p_parent + r_parent * vec3(joint.origin.xyz.0);
                let axis = vec3(joint.axis.xyz.0).normalize();
                let q = angle.get(joint.name.as_str()).copied().unwrap_or(0.0);
                axis_origin.insert(joint.name.as_str(), (r_joint * axis, p_joint));
                (r_joint * rot_axis(&axis, q), p_joint)
            }
        };
        frame.insert(link, pose);
        pose
    }
    let mut com: HashMap<&str, (f64, Vector3<f64>)> = HashMap::new();
    for link in &robot.links {
        let (r, p) = resolve(
            &link.name,
            &parent_joint,
            &angle,
            &mut frame,
            &mut axis_origin,
        );
        com.insert(
            link.name.as_str(),
            (
                link.inertial.mass.value,
                p + r * vec3(link.inertial.origin.xyz.0),
            ),
        );
    }
    // Links below each joint: walk each link's parent chain.
    let mut below: HashMap<&str, Vec<&str>> = HashMap::new();
    for link in &robot.links {
        let mut current = link.name.as_str();
        while let Some(joint) = parent_joint.get(current) {
            below
                .entry(joint.name.as_str())
                .or_default()
                .push(link.name.as_str());
            current = joint.parent.link.as_str();
        }
    }
    names
        .iter()
        .map(|name| {
            let (axis, origin) = axis_origin[name.as_str()];
            let (mass, moment) = below[name.as_str()].iter().fold(
                (0.0, Vector3::zeros()),
                |(mass, moment), link| {
                    let (m, c) = com[link];
                    (mass + m, moment + c * m)
                },
            );
            -axis.dot(&((moment - origin * mass).cross(&G)))
        })
        .collect()
}

#[test]
fn master_urdf_gravity_matches_the_exact_one_pass_formula() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let robot_cfg =
        marengo_config::load_robot_config_from(root.join("config")).expect("robot.yaml");
    let urdf_path = root.join(&robot_cfg.robot.urdf);
    let names = robot_cfg.robot.joints.clone();
    let model = gravity_model_from_urdf(&urdf_path, &names).expect("master model");
    let robot = armee_kinematics::load_urdf(&urdf_path).expect("urdf");

    // Deterministic poses across ±π on every joint.
    let mut state: u64 = 0x2026_1028;
    let mut uniform = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    let mut worst = 0.0_f64;
    for _ in 0..300 {
        let q: Vec<f64> = names
            .iter()
            .map(|_| (uniform() * 2.0 - 1.0) * std::f64::consts::PI)
            .collect();
        let library = model.gravity_torques(&q).expect("finite pose");
        let exact = oracle(&robot, &names, &q);
        for (a, b) in library.iter().zip(&exact) {
            worst = worst.max((a - b).abs());
        }
    }
    assert!(
        worst < 1e-6,
        "numerical vs exact τ_g differ by {worst:e} Nm"
    );
}
