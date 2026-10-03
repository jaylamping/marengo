//! Gravity torques via virtual work (numerical ∂COM/∂q).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use armee_kinematics::load_urdf;
use nalgebra::{Isometry3, Point3, Rotation3, Translation3, Unit, Vector3};
use urdf_rs::{JointType, Robot};

use crate::{DynamicsError, PureGravityTorque};

const GRAVITY: Vector3<f64> = Vector3::new(0.0, 0.0, -9.81);
const DQ_EPS: f64 = 1e-6;

/// Mass (kg) and centre of mass (m, link frame) of one URDF link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkInertial {
    pub mass_kg: f64,
    pub com_m: [f64; 3],
}

/// A zero-length joint axis cannot be normalised (NaN torques); anything shorter is refused.
const MIN_AXIS_NORM: f64 = 1e-9;

/// Gravity compensation model built from URDF kinematics and link masses.
///
/// Construction validates the model and fails closed, so [`gravity_torques`] never has to
/// guess: every actuated joint is either modelled (an angle in `q`) or explicitly held at a
/// stated angle, no joint kind the model cannot represent (prismatic, mimic, floating) is
/// present, the link tree is acyclic, and axes, origins and inertials are finite.
///
/// [`gravity_torques`]: crate::DynamicsModel::gravity_torques
#[derive(Clone)]
pub struct UrdfGravityModel {
    joint_names: Vec<String>,
    /// Actuated joints outside `joint_names`, locked at the stated angle (rad).
    held: Vec<(String, f64)>,
    robot: Robot,
    /// Cached: for each link name, the ordered list of joint indices (root→leaf).
    /// Eliminates the O(n) `joints.iter().find()` scan per link_transform call.
    link_chains: HashMap<String, Vec<usize>>,
}

impl UrdfGravityModel {
    /// Load the model from a URDF file. `joint_names` (robot.yaml order) must name **every**
    /// actuated URDF joint exactly once.
    pub fn from_urdf(
        urdf_path: impl AsRef<Path>,
        joint_names: &[String],
    ) -> Result<Self, DynamicsError> {
        Self::from_urdf_with_held(urdf_path, joint_names, &[])
    }

    /// Like [`from_urdf`](Self::from_urdf) for a bench slice that models only some joints:
    /// every actuated joint not in `joint_names` must be listed in `held` with the angle (rad)
    /// it is locked at. The angle is stated, never assumed.
    pub fn from_urdf_with_held(
        urdf_path: impl AsRef<Path>,
        joint_names: &[String],
        held: &[(String, f64)],
    ) -> Result<Self, DynamicsError> {
        Self::from_robot(load_urdf(urdf_path)?, joint_names, held)
    }

    fn from_robot(
        robot: Robot,
        joint_names: &[String],
        held: &[(String, f64)],
    ) -> Result<Self, DynamicsError> {
        validate_model(&robot, joint_names, held)?;

        // Precompute the parent joint chain (root→leaf) for each link so that
        // link_transform can do a direct index lookup instead of an O(n) scan.
        let mut link_chains = HashMap::new();
        for link in &robot.links {
            let mut chain = Vec::new();
            let mut current = link.name.clone();
            while let Some(idx) = robot.joints.iter().position(|j| j.child.link == current) {
                // A tree path is never longer than the joint count; more means a cycle.
                if chain.len() >= robot.joints.len() {
                    return Err(DynamicsError::CyclicChain {
                        link: link.name.clone(),
                    });
                }
                chain.push(idx);
                current = robot.joints[idx].parent.link.clone();
            }
            chain.reverse(); // root→leaf order
            link_chains.insert(link.name.clone(), chain);
        }

        Ok(Self {
            joint_names: joint_names.to_vec(),
            held: held.to_vec(),
            robot,
            link_chains,
        })
    }

    /// Mass and COM of `link` as loaded from the URDF.
    pub fn link_inertial(&self, link: &str) -> Result<LinkInertial, DynamicsError> {
        let l = self.find_link(link)?;
        let o = &l.inertial.origin.xyz.0;
        Ok(LinkInertial {
            mass_kg: l.inertial.mass.value,
            com_m: [o[0], o[1], o[2]],
        })
    }

    /// Copy of the model with `link`'s mass and COM replaced (inertia tensor untouched).
    pub fn with_link_inertial(
        &self,
        link: &str,
        inertial: LinkInertial,
    ) -> Result<Self, DynamicsError> {
        let mut out = self.clone();
        let l = out
            .robot
            .links
            .iter_mut()
            .find(|l| l.name == link)
            .ok_or_else(|| DynamicsError::UnknownLink {
                link: link.to_string(),
            })?;
        l.inertial.mass.value = inertial.mass_kg;
        l.inertial.origin.xyz.0 = inertial.com_m;
        Ok(out)
    }

    /// Links whose pose depends on `joint` (its child link and everything below it), in URDF
    /// link order.
    pub fn links_downstream_of(&self, joint: &str) -> Result<Vec<String>, DynamicsError> {
        let idx = self
            .robot
            .joints
            .iter()
            .position(|j| j.name == joint)
            .ok_or_else(|| DynamicsError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        Ok(self
            .robot
            .links
            .iter()
            .filter(|l| {
                self.link_chains
                    .get(&l.name)
                    .is_some_and(|chain| chain.contains(&idx))
            })
            .map(|l| l.name.clone())
            .collect())
    }

    /// Joint-space holding torque (Nm) of a **unit** point mass fixed at `point_m` in
    /// `link`'s frame, at pose `q`. Same sign convention and virtual-work gradient as
    /// [`DynamicsModel::gravity_torques`](crate::DynamicsModel::gravity_torques); a link's
    /// contribution there equals `mass · point_mass_torques(link, com, q)`.
    pub fn point_mass_torques(
        &self,
        link: &str,
        point_m: [f64; 3],
        q: &[f64],
    ) -> Result<Vec<f64>, DynamicsError> {
        self.find_link(link)?;
        let mut q_map = self.q_map(q)?;
        let p = Point3::new(point_m[0], point_m[1], point_m[2]);
        let mut tau = vec![0.0; q.len()];
        for (i, t) in tau.iter_mut().enumerate() {
            let q0 = q[i];
            let mut dpe_dq = 0.0;
            for sign in [-1.0f64, 1.0] {
                q_map[i].1 = q0 + sign * DQ_EPS;
                let world = self.link_transform(link, &q_map).transform_point(&p);
                dpe_dq += sign * -GRAVITY.dot(&world.coords) / (2.0 * DQ_EPS);
            }
            q_map[i].1 = q0;
            *t = dpe_dq;
        }
        if let Some(i) = tau.iter().position(|t| !t.is_finite()) {
            return Err(DynamicsError::NonFiniteTorque {
                joint: self.joint_names[i].clone(),
            });
        }
        Ok(tau)
    }

    fn find_link(&self, link: &str) -> Result<&urdf_rs::Link, DynamicsError> {
        self.robot
            .links
            .iter()
            .find(|l| l.name == link)
            .ok_or_else(|| DynamicsError::UnknownLink {
                link: link.to_string(),
            })
    }

    /// Joint-name → angle map: the modelled joints first (so index `i` is `q[i]`), then the
    /// held joints at their stated angles.
    fn q_map(&self, q: &[f64]) -> Result<Vec<(String, f64)>, DynamicsError> {
        if q.len() != self.joint_names.len() {
            return Err(DynamicsError::JointCount {
                expected: self.joint_names.len(),
                got: q.len(),
            });
        }
        if q.iter().any(|v| !v.is_finite()) {
            return Err(DynamicsError::NonFiniteInput {
                what: "joint angles q",
            });
        }
        Ok(self
            .joint_names
            .iter()
            .cloned()
            .zip(q.iter().copied())
            .chain(self.held.iter().cloned())
            .collect())
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
            // A COM is a point: joint origins translate it as well as rotate it.
            // Multiplying an Isometry by Vector3 would drop every translation.
            let com_world = transform.transform_point(&Point3::from(com_local));
            out.push((mass, com_world.coords));
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
            if joint.joint_type == JointType::Revolute || joint.joint_type == JointType::Continuous
            {
                // Construction guarantees every revolute/continuous joint is in `q_map`; a
                // miss is a bug, and a NaN angle makes the torque non-finite (refused by the
                // caller) instead of silently evaluating the joint at 0 rad.
                let q = q_map
                    .iter()
                    .find(|(n, _)| n == &joint.name)
                    .map_or(f64::NAN, |(_, v)| *v);
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

fn joint_kind(joint_type: &JointType) -> &'static str {
    match joint_type {
        JointType::Revolute => "revolute",
        JointType::Continuous => "continuous",
        JointType::Prismatic => "prismatic",
        JointType::Fixed => "fixed",
        JointType::Floating => "floating",
        JointType::Planar => "planar",
        JointType::Spherical => "spherical",
    }
}

fn finite_all(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Refuse any URDF/joint-list combination the gravity model would silently mis-evaluate.
fn validate_model(
    robot: &Robot,
    joint_names: &[String],
    held: &[(String, f64)],
) -> Result<(), DynamicsError> {
    let invalid = |what: String| DynamicsError::InvalidModel { what };

    // Names: unique in the URDF, unique across modelled + held.
    let mut seen_urdf: HashSet<&str> = HashSet::new();
    for joint in &robot.joints {
        if !seen_urdf.insert(joint.name.as_str()) {
            return Err(invalid(format!("duplicate URDF joint {}", joint.name)));
        }
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for name in joint_names.iter().chain(held.iter().map(|(n, _)| n)) {
        if !seen.insert(name.as_str()) {
            return Err(DynamicsError::DuplicateJoint {
                joint: name.clone(),
            });
        }
    }
    if let Some((joint, _)) = held.iter().find(|(_, angle)| !angle.is_finite()) {
        return Err(invalid(format!(
            "held joint {joint} has a non-finite angle"
        )));
    }

    // Joint kinds the model can represent: fixed, revolute, continuous; no mimic.
    for joint in &robot.joints {
        let what = match joint.joint_type {
            JointType::Fixed | JointType::Revolute | JointType::Continuous => None,
            JointType::Prismatic => Some("prismatic motion"),
            JointType::Floating | JointType::Planar | JointType::Spherical => {
                Some("floating/planar/spherical motion")
            }
        };
        if let Some(what) = what {
            return Err(DynamicsError::UnsupportedJoint {
                joint: joint.name.clone(),
                what,
            });
        }
        if joint.mimic.is_some() {
            return Err(DynamicsError::UnsupportedJoint {
                joint: joint.name.clone(),
                what: "mimic coupling",
            });
        }
        if !finite_all(&joint.origin.xyz.0) || !finite_all(&joint.origin.rpy.0) {
            return Err(invalid(format!(
                "joint {} has a non-finite origin",
                joint.name
            )));
        }
    }

    // Listed joints must exist and be revolute/continuous.
    for name in joint_names.iter().chain(held.iter().map(|(n, _)| n)) {
        let joint = robot
            .joints
            .iter()
            .find(|j| &j.name == name)
            .ok_or_else(|| DynamicsError::UnknownJoint {
                joint: name.clone(),
            })?;
        if !matches!(
            joint.joint_type,
            JointType::Revolute | JointType::Continuous
        ) {
            return Err(DynamicsError::NotActuated {
                joint: name.clone(),
                kind: joint_kind(&joint.joint_type),
            });
        }
    }

    // Every actuated joint is accounted for, and has a usable axis.
    for joint in &robot.joints {
        if !matches!(
            joint.joint_type,
            JointType::Revolute | JointType::Continuous
        ) {
            continue;
        }
        if !seen.contains(joint.name.as_str()) {
            return Err(DynamicsError::UnmodelledJoint {
                joint: joint.name.clone(),
            });
        }
        let a = joint.axis.xyz.0;
        let norm = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        if !finite_all(&a) || !norm.is_finite() || norm < MIN_AXIS_NORM {
            return Err(invalid(format!(
                "joint {} has a zero or non-finite axis {a:?}",
                joint.name
            )));
        }
    }

    // Link tree: one parent joint per link; inertials finite and non-negative.
    let mut children: HashSet<&str> = HashSet::new();
    for joint in &robot.joints {
        if !children.insert(joint.child.link.as_str()) {
            return Err(DynamicsError::CyclicChain {
                link: joint.child.link.clone(),
            });
        }
    }
    for link in &robot.links {
        let mass = link.inertial.mass.value;
        let com = link.inertial.origin.xyz.0;
        if !mass.is_finite() || mass < 0.0 || !finite_all(&com) {
            return Err(invalid(format!(
                "link {} has a non-finite or negative mass ({mass}) or COM {com:?}",
                link.name
            )));
        }
    }
    Ok(())
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
        let mut q_map = self.q_map(q)?;

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
        if let Some(i) = tau.iter().position(|t| !t.is_finite()) {
            return Err(DynamicsError::NonFiniteTorque {
                joint: self.joint_names[i].clone(),
            });
        }
        Ok(PureGravityTorque(tau))
    }
}
