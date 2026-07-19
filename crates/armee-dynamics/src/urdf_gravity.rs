//! Gravity torques via virtual work (numerical ∂COM/∂q).

use std::collections::HashMap;
use std::path::Path;

use armee_kinematics::load_urdf;
use nalgebra::{Isometry3, Rotation3, Translation3, Unit, Vector3};
use urdf_rs::{JointType, Robot};

use crate::{DynamicsError, PureGravityTorque};

const GRAVITY: Vector3<f64> = Vector3::new(0.0, 0.0, -9.81);
const DQ_EPS: f64 = 1e-6;

/// Gravity compensation model built from URDF kinematics and link masses.
pub struct UrdfGravityModel {
    joint_names: Vec<String>,
    robot: Robot,
    /// Cached: for each link name, the ordered list of joint indices (root→leaf).
    /// Eliminates the O(n) `joints.iter().find()` scan per link_transform call.
    link_chains: HashMap<String, Vec<usize>>,
}

impl UrdfGravityModel {
    pub fn from_urdf(
        urdf_path: impl AsRef<Path>,
        joint_names: &[String],
    ) -> Result<Self, DynamicsError> {
        let robot = load_urdf(urdf_path)?;
        for name in joint_names {
            if !robot.joints.iter().any(|j| &j.name == name) {
                return Err(DynamicsError::UnknownJoint {
                    joint: name.clone(),
                });
            }
        }

        // Precompute the parent joint chain (root→leaf) for each link so that
        // link_transform can do a direct index lookup instead of an O(n) scan.
        let mut link_chains = HashMap::new();
        for link in &robot.links {
            let mut chain = Vec::new();
            let mut current = link.name.clone();
            loop {
                let joint_idx = robot.joints.iter().position(|j| j.child.link == current);
                let Some(idx) = joint_idx else {
                    break;
                };
                chain.push(idx);
                current = robot.joints[idx].parent.link.clone();
            }
            chain.reverse(); // root→leaf order
            link_chains.insert(link.name.clone(), chain);
        }

        Ok(Self {
            joint_names: joint_names.to_vec(),
            robot,
            link_chains,
        })
    }

    fn link_com_world(&self, q_map: &[(String, f64)]) -> Vec<(f64, Vector3<f64>)> {
        let mut out = Vec::new();
        for link in &self.robot.links {
            let mass = link.inertial.mass.value;
            if mass <= 0.0 {
                continue;
            }
            let transform = self.link_transform(&link.name, q_map);
            let o = &link.inertial.origin;
            let com_local = Vector3::new(o.xyz.0[0], o.xyz.0[1], o.xyz.0[2]);
            let com_world = transform * com_local;
            out.push((mass, com_world));
        }
        out
    }

    fn link_transform(&self, link_name: &str, q_map: &[(String, f64)]) -> Isometry3<f64> {
        let chain: Vec<&urdf_rs::Joint> = match self.link_chains.get(link_name) {
            Some(indices) => indices.iter().map(|&i| &self.robot.joints[i]).collect(),
            None => Vec::new(),
        };

        let mut t = Isometry3::identity();
        for joint in chain {
            t *= pose_to_isometry(&joint.origin);
            let q = q_map
                .iter()
                .find(|(n, _)| n == &joint.name)
                .map(|(_, v)| *v)
                .unwrap_or(0.0);
            if joint.joint_type == JointType::Revolute || joint.joint_type == JointType::Continuous
            {
                let axis = Vector3::new(
                    joint.axis.xyz.0[0],
                    joint.axis.xyz.0[1],
                    joint.axis.xyz.0[2],
                );
                let axis = Unit::new_normalize(axis);
                t *= Isometry3::from_parts(
                    Translation3::identity(),
                    Rotation3::from_axis_angle(&axis, q).into(),
                );
            }
        }
        t
    }
}

fn pose_to_isometry(pose: &urdf_rs::Pose) -> Isometry3<f64> {
    let xyz = pose.xyz.0;
    let rpy = pose.rpy.0;
    let t = Translation3::new(xyz[0], xyz[1], xyz[2]);
    let r = Rotation3::from_euler_angles(rpy[0], rpy[1], rpy[2]);
    Isometry3::from_parts(t, r.into())
}

impl super::DynamicsModel for UrdfGravityModel {
    fn joint_names(&self) -> &[String] {
        &self.joint_names
    }

    fn gravity_torques(&self, q: &[f64]) -> Result<PureGravityTorque, DynamicsError> {
        if q.len() != self.joint_names.len() {
            return Err(DynamicsError::JointCount {
                expected: self.joint_names.len(),
                got: q.len(),
            });
        }
        let mut q_map: Vec<(String, f64)> = self
            .joint_names
            .iter()
            .cloned()
            .zip(q.iter().copied())
            .collect();

        let mut tau = vec![0.0; q.len()];
        for (i, _joint_name) in self.joint_names.iter().enumerate() {
            let mut dpe_dq = 0.0;
            let q0 = q[i];
            for sign in [-1.0f64, 1.0] {
                q_map[i].1 = q0 + sign * DQ_EPS;
                let coms = self.link_com_world(&q_map);
                let pe: f64 = coms.iter().map(|(m, p)| -m * GRAVITY.dot(p)).sum();
                dpe_dq += sign * pe / (2.0 * DQ_EPS);
            }
            q_map[i].1 = q0;
            tau[i] = dpe_dq;
        }
        Ok(PureGravityTorque(tau))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::path::Path;

    use super::*;

    fn arm_3dof_right_urdf() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/urdf/arm_3dof_right.urdf")
    }

    #[test]
    #[ignore]
    fn link_chains_built_correctly() {
        let model = UrdfGravityModel::from_urdf(
            arm_3dof_right_urdf(),
            &["right_shoulder_pitch".to_string()],
        )
        .expect("build model from arm_3dof_right.urdf");

        // base_link is the root: no parent joint → empty chain.
        let base_chain = model
            .link_chains
            .get("base_link")
            .expect("base_link has a cached chain");
        assert!(
            base_chain.is_empty(),
            "root link should have an empty joint chain, got {base_chain:?}",
        );

        // right_upper_arm_stub is the child of right_shoulder_pitch (joint index 0).
        let arm_chain = model
            .link_chains
            .get("right_upper_arm_stub")
            .expect("right_upper_arm_stub has a cached chain");
        assert_eq!(
            arm_chain,
            &vec![0usize],
            "right_upper_arm_stub chain should be [0] (right_shoulder_pitch), got {arm_chain:?}",
        );
    }
}
