//! Lumped gravity of one swept joint, and the smallest URDF change that reproduces it.
//!
//! With every other joint fixed, a revolute joint's gravity holding torque is exactly
//!
//! ```text
//! τ(q) = A·sin q + B·cos q          (= C·sin(q + φ), C = √(A² + B²), φ = atan2(B, A))
//! ```
//!
//! because the carried COMs rotate rigidly about the joint axis and only their component
//! perpendicular to it produces torque. A one-joint sweep identifies only these two numbers:
//! they lump the first moment of everything the joint carries, so a sweep cannot say which
//! link is wrong. [`lumped_terms`] reads A and B off the URDF (`A = τ(π/2)`, `B = τ(0)`).
//!
//! [`lumped_com_patch`] turns fitted (A, B) targets back into a URDF that reproduces them:
//! link masses stay as they are (they can be weighed) and the COMs of the carried links move
//! by the smallest total displacement `Σ |Δc_k|²` that meets every target exactly. Each
//! link's torque is affine in its COM (`m·P(c, q)`), so the targets are linear in the shifts
//! and the minimum-norm solution is closed-form. Heavier links and longer levers move the
//! torque more per metre, so they take more of the change.

use std::f64::consts::FRAC_PI_2;

use nalgebra::{DMatrix, DVector};

use crate::calibration::{CalibrationError, FittedLink};
use crate::{DynamicsError, DynamicsModel, LinkInertial, UrdfGravityModel};

/// The two coefficients of one joint's gravity torque at a fixed pose of the others.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LumpedTerms {
    /// Coefficient of `sin q` (Nm).
    pub a_nm: f64,
    /// Coefficient of `cos q` (Nm).
    pub b_nm: f64,
}

impl LumpedTerms {
    /// `A·sin q + B·cos q`.
    pub fn torque(&self, q: f64) -> f64 {
        self.a_nm * q.sin() + self.b_nm * q.cos()
    }

    /// Amplitude `C = √(A² + B²)` of `C·sin(q + φ)`.
    pub fn amplitude_nm(&self) -> f64 {
        self.a_nm.hypot(self.b_nm)
    }

    /// Phase `φ = atan2(B, A)` of `C·sin(q + φ)` (rad).
    pub fn phase_rad(&self) -> f64 {
        self.b_nm.atan2(self.a_nm)
    }
}

/// A and B of `joint` with every other joint at its entry of `q_fixed` (the joint's own
/// entry is ignored).
pub fn lumped_terms(
    model: &UrdfGravityModel,
    joint: &str,
    q_fixed: &[f64],
) -> Result<LumpedTerms, DynamicsError> {
    let j = joint_index(model, joint)?;
    let mut q = q_fixed.to_vec();
    if q.len() != model.joint_names().len() {
        return Err(DynamicsError::JointCount {
            expected: model.joint_names().len(),
            got: q.len(),
        });
    }
    q[j] = FRAC_PI_2;
    let a_nm = model.gravity_torques(&q)?[j];
    q[j] = 0.0;
    let b_nm = model.gravity_torques(&q)?[j];
    Ok(LumpedTerms { a_nm, b_nm })
}

/// One fitted (A, B) the patched URDF must reproduce.
#[derive(Debug, Clone, PartialEq)]
pub struct LumpedTarget {
    pub joint: String,
    /// Pose of the other joints (model order); the joint's own entry is ignored.
    pub q_fixed: Vec<f64>,
    pub terms: LumpedTerms,
}

/// Targets reproduced to this (Nm) by the solution before rounding.
const SOLVE_TOL_NM: f64 = 1e-9;
/// Smallest singular value (Nm per m) of the shift map that still counts as reachable.
const MIN_REACH_NM_PER_M: f64 = 1e-6;

/// The smallest COM displacement (Σ |Δc|² over `links`, masses unchanged) that makes the
/// model reproduce every target exactly.
///
/// `links` are the links the patch may touch (e.g. one limb); only those a target joint
/// carries and that have mass move. Returns one [`FittedLink`] per moved link (`principal_axis`
/// is the unit shift direction). Fails when a target is not reachable by COM shifts of those
/// links (its rows are degenerate) or targets contradict each other.
pub fn lumped_com_patch(
    model: &UrdfGravityModel,
    targets: &[LumpedTarget],
    links: &[String],
) -> Result<Vec<FittedLink>, CalibrationError> {
    if targets.is_empty() {
        return Err(CalibrationError::NoParams);
    }
    let mut movable: Vec<(String, LinkInertial)> = Vec::new();
    for t in targets {
        for link in model.links_downstream_of(&t.joint)? {
            let inertial = model.link_inertial(&link)?;
            if inertial.mass_kg > 0.0
                && links.contains(&link)
                && !movable.iter().any(|(l, _)| *l == link)
            {
                movable.push((link, inertial));
            }
        }
    }
    if movable.is_empty() {
        return Err(CalibrationError::NoParams);
    }
    // Rows: A then B of each target. Columns: x, y, z shift of each movable link.
    let rows = 2 * targets.len();
    let cols = 3 * movable.len();
    let mut m = DMatrix::<f64>::zeros(rows, cols);
    let mut r = DVector::<f64>::zeros(rows);
    for (t_i, t) in targets.iter().enumerate() {
        let j = joint_index(model, &t.joint)?;
        let cad = lumped_terms(model, &t.joint, &t.q_fixed)?;
        r[2 * t_i] = t.terms.a_nm - cad.a_nm;
        r[2 * t_i + 1] = t.terms.b_nm - cad.b_nm;
        for (angle, row) in [(FRAC_PI_2, 2 * t_i), (0.0, 2 * t_i + 1)] {
            let mut q = t.q_fixed.clone();
            q[j] = angle;
            for (k, (link, inertial)) in movable.iter().enumerate() {
                let base = model.point_mass_torques(link, inertial.com_m, &q)?[j];
                for axis in 0..3 {
                    let mut p = inertial.com_m;
                    p[axis] += 1.0;
                    let moved = model.point_mass_torques(link, p, &q)?[j];
                    m[(row, 3 * k + axis)] = inertial.mass_kg * (moved - base);
                }
            }
        }
    }
    let svd = m.clone().svd(true, true);
    let smallest = svd
        .singular_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    if smallest.is_nan() || smallest < MIN_REACH_NM_PER_M {
        return Err(CalibrationError::LumpedUnreachable {
            reason: format!(
                "COM shifts of {} cannot move every target independently (smallest singular \
                 value {smallest:.3e} Nm/m)",
                movable
                    .iter()
                    .map(|(l, _)| l.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    let shift = svd
        .solve(&r, 0.0)
        .map_err(|e| CalibrationError::LumpedUnreachable {
            reason: e.to_string(),
        })?;
    let miss = (&m * &shift - &r).amax();
    if miss.is_nan() || miss > SOLVE_TOL_NM {
        return Err(CalibrationError::LumpedUnreachable {
            reason: format!("targets contradict each other (miss {miss:.3e} Nm)"),
        });
    }
    Ok(movable
        .into_iter()
        .enumerate()
        .filter_map(|(k, (link, cad))| {
            let d = [shift[3 * k], shift[3 * k + 1], shift[3 * k + 2]];
            let norm = d.iter().map(|v| v * v).sum::<f64>().sqrt();
            (norm > 0.0).then(|| FittedLink {
                link,
                cad,
                fitted: LinkInertial {
                    mass_kg: cad.mass_kg,
                    com_m: [
                        cad.com_m[0] + d[0],
                        cad.com_m[1] + d[1],
                        cad.com_m[2] + d[2],
                    ],
                },
                principal_axis: [d[0] / norm, d[1] / norm, d[2] / norm],
            })
        })
        .collect())
}

fn joint_index(model: &UrdfGravityModel, joint: &str) -> Result<usize, DynamicsError> {
    model
        .joint_names()
        .iter()
        .position(|j| j == joint)
        .ok_or_else(|| DynamicsError::UnknownJoint {
            joint: joint.to_string(),
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::path::Path;

    use super::*;

    fn arm() -> (UrdfGravityModel, Vec<String>) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let joints = vec![
            "right_shoulder_pitch".to_string(),
            "right_shoulder_roll".to_string(),
            "right_upper_arm_yaw".to_string(),
            "right_elbow_pitch".to_string(),
            "right_lower_arm_yaw".to_string(),
        ];
        let model = UrdfGravityModel::from_urdf(root.join("assets/urdf/marengo.urdf"), &joints)
            .expect("live URDF");
        (model, joints)
    }

    #[test]
    fn lumped_terms_reproduce_the_model_at_any_angle() {
        let (model, _) = arm();
        let fixed = [0.0, 0.2, -0.3, 0.6, 0.1];
        let terms = lumped_terms(&model, "right_shoulder_pitch", &fixed).unwrap();
        for q in [-0.9, -0.3, 0.0, 0.4, 1.3, 2.5] {
            let mut pose = fixed;
            pose[0] = q;
            let tau = model.gravity_torques(&pose).unwrap()[0];
            assert!((terms.torque(q) - tau).abs() < 1e-6, "q {q}: {tau}");
        }
        let c = terms.amplitude_nm();
        let phi = terms.phase_rad();
        assert!((c * (0.7 + phi).sin() - terms.torque(0.7)).abs() < 1e-9);
    }

    #[test]
    fn com_patch_reproduces_targets_with_masses_kept_and_minimal_shift() {
        let (model, joints) = arm();
        let fixed = vec![0.0; joints.len()];
        let cad = lumped_terms(&model, "right_shoulder_pitch", &fixed).unwrap();
        let want = LumpedTerms {
            a_nm: cad.a_nm * 0.82,
            b_nm: cad.b_nm - 0.15,
        };
        let target = LumpedTarget {
            joint: "right_shoulder_pitch".into(),
            q_fixed: fixed.clone(),
            terms: want,
        };
        let limb = model.links_downstream_of("right_shoulder_pitch").unwrap();
        let links = lumped_com_patch(&model, std::slice::from_ref(&target), &limb).unwrap();
        assert!(!links.is_empty());
        let mut patched = model.clone();
        for l in &links {
            assert_eq!(l.fitted.mass_kg, l.cad.mass_kg, "{} mass kept", l.link);
            patched = patched.with_link_inertial(&l.link, l.fitted).unwrap();
        }
        let got = lumped_terms(&patched, "right_shoulder_pitch", &fixed).unwrap();
        assert!((got.a_nm - want.a_nm).abs() < 1e-6, "{got:?} vs {want:?}");
        assert!((got.b_nm - want.b_nm).abs() < 1e-6, "{got:?} vs {want:?}");
        // Minimal: the pitch axis is world y at this pose and every link frame is aligned
        // with the world, so a COM shift along y moves no pitch torque. A minimum-norm
        // solution puts nothing in that null direction.
        for l in &links {
            assert!(
                (l.fitted.com_m[1] - l.cad.com_m[1]).abs() < 1e-9,
                "{} shifted along the pitch axis: {l:?}",
                l.link
            );
        }
    }

    #[test]
    fn com_patch_refuses_unreachable_targets() {
        let (model, joints) = arm();
        let fixed = vec![0.0; joints.len()];
        // Two different A targets for the same joint and pose cannot both hold.
        let cad = lumped_terms(&model, "right_shoulder_pitch", &fixed).unwrap();
        let t = |a: f64| LumpedTarget {
            joint: "right_shoulder_pitch".into(),
            q_fixed: fixed.clone(),
            terms: LumpedTerms {
                a_nm: a,
                b_nm: cad.b_nm,
            },
        };
        let limb = model.links_downstream_of("right_shoulder_pitch").unwrap();
        assert!(matches!(
            lumped_com_patch(&model, &[t(cad.a_nm), t(cad.a_nm + 0.3)], &limb),
            Err(CalibrationError::LumpedUnreachable { .. })
        ));
    }
}
