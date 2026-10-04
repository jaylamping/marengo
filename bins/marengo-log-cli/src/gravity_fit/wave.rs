//! Wave method of `gravity-fit` (`pi_joint_calibrate` `method: "wave"`).
//!
//! Static holds measure the controller command inside the stiction band, so they cannot
//! place the gravity curve better than ±F_s. A wave drives through stiction in both
//! directions; at the same q the up- and down-going torques straddle gravity:
//!
//! ```text
//! τ_up   = τ_g(q) + I·q̈ + f(|q̇|)        τ_down = τ_g(q) + I·q̈ − f(|q̇|)
//! ```
//!
//! 1. **Bins.** Each wave step (centre c, amplitude a) is cut into three bins of width
//!    a·[`BIN_SHARE_OF_AMPLITUDE`] centred on c and c ± a/2: the central part of the raised
//!    cosine, where speed is ≥ 66 % of peak and the bins stay clear of the turnarounds.
//!    Moving samples (|q̇| above the deadband) split by direction; a bin needs
//!    [`MIN_BIN_SAMPLES`] each way. Per direction the mean of `τ − I·q̈` (I from the URDF at
//!    the bin pose, q̈ the measured second difference); **gravity** is the up/down mean,
//!    **friction** half their difference, at the mean |q̇|.
//! 2. **Lumped fit per swept joint.** With the other joints fixed (one group per sweep joint
//!    and fixed pose), the swept joint's gravity is exactly `A·sin q + B·cos q`
//!    ([`armee_dynamics::lumped`]). A and B are least squares over the bins; with ≥ 2 speeds
//!    the URDF inertia error ΔI (rotor inertia is not in the URDF) is fitted with them, since
//!    ΔI·q̈ is odd about each wave centre and grows with ω²; with one speed only the centre
//!    bins (q̈ ≈ 0) enter. Bins at the same centre, ΔI·q̈ removed, pool into pose estimates.
//! 3. **Gates, derived from the data.** σ_cross is the pooled spread of per-session pose
//!    means across sessions; the residual gate is `max(GATE_SIGMAS·σ_cross, readout step)`.
//!    A group is accepted only with ≥ 2 sessions sharing a pose, ≥ 3 replicated poses, the
//!    gate within the suite's residual limit ([`FitOptions::max_residual_nm`]: the data must
//!    be repeatable enough to check it), every replicated pose residual within the gate, and
//!    σ_A, σ_B within the gate (the sweep identifies both). A pose with a single bin (one
//!    wave step of one session) has no spread to judge it by: it stays in the fit and its
//!    residual is reported, but it never decides the verdict.
//! 4. **URDF patch.** For accepted groups, the smallest COM shift of the carried right-arm
//!    links that reproduces every fitted (A, B), masses unchanged
//!    ([`armee_dynamics::lumped::lumped_com_patch`]), refused above
//!    [`FitOptions::max_com_offset_m`]. The record lists how much the shift moves every other
//!    joint's gravity.
//! 5. **Friction patch per swept joint**, independent of gravity (the half-difference
//!    cancels it): `fc + fv·|q̇|` over the pose/speed bins, `fs` only when the Stribeck term
//!    `(fs − fc)·exp(−|q̇|/v_b)` (v_b from `control.yaml`) is significant, and fv only when
//!    significant (else fc is the Coulomb mean and fv 0: a slope from a narrow speed span is
//!    never extrapolated to cruise speeds). Significance and the residual gate use the bin
//!    noise, `max(RMS sampling error of the points, σ_cross)`; the gate is
//!    `GATE_SIGMAS·noise`, floored at the readout step and capped at the suite limit. σ_cross
//!    alone is too small: a wave repeats the same torque swings at the same q in every
//!    session, so bins agree across sessions far better than any smooth friction model can
//!    match bins at other poses (2026-10-04 pitch: σ_cross 0.008 Nm, bin noise 0.047 Nm,
//!    scatter about the Coulomb mean 0.041 Nm).
//!
//! Nothing is applied.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use armee_dynamics::calibration::{FitOptions, FittedLink};
use armee_dynamics::lumped::{lumped_com_patch, lumped_terms, LumpedTarget, LumpedTerms};
use armee_dynamics::{gravity_model_from_urdf, DynamicsModel, UrdfGravityModel};
use marengo_config::{motor_for_joint, ControlConfigFile, MotorsConfigFile};
use robstride::motor_type::MitRanges;
use serde_json::{json, Value};

use super::{
    display_relative, fmt_xyz, io, limb_links, patch_control_friction, patch_inertials,
    record_stem, round, unified_diff, FrictionProposal, GravityFitArgs, GravityFitError, Outcome,
    PlanStep, Session, CONTROL_PATCH_PATH, PATCH_PATH, PATCH_ROUNDTRIP_TOL_NM, PROPOSED_URDF_FILE,
    WAVE_DQ_DEADBAND_RAD_S,
};

/// Bin width as a share of the wave amplitude: three bins (c, c ± a/2) cover the central
/// ±0.75·a, where the raised cosine runs at ≥ 66 % of its peak speed.
pub const BIN_SHARE_OF_AMPLITUDE: f64 = 0.5;
/// Fewest moving samples per direction in a bin. `pi_joint_calibrate` sizes wave cycles so
/// the centre bin collects this many at the fastest speed.
pub const MIN_BIN_SAMPLES: usize = 10;
/// Residual gate = this many σ_cross (pooled cross-session spread of pose means).
pub const GATE_SIGMAS: f64 = 3.0;
/// Bin centres closer than this pool into one pose (rad).
const POSE_KEY_RAD: f64 = 1e-4;
/// A, B and at least one residual degree of freedom.
const MIN_POSES: usize = 3;

pub(super) struct Inputs<'a> {
    pub args: &'a GravityFitArgs,
    pub sessions: &'a [Session],
    pub model: &'a UrdfGravityModel,
    pub joints: &'a [String],
    pub limbs: &'a BTreeMap<String, Vec<String>>,
    pub control: &'a ControlConfigFile,
    pub motors: &'a MotorsConfigFile,
    pub pi_urdf: &'a str,
    pub local_matches_pi: bool,
    pub control_path: &'a Path,
}

/// Gravity and friction estimate of one bin of one wave step.
#[derive(Debug, Clone)]
struct Bin {
    session: usize,
    plan_index: usize,
    center_rad: f64,
    half_period_s: f64,
    /// The bin centred on the wave centre (q̈ ≈ 0 there by symmetry).
    centre: bool,
    /// Mean pose (up/down average), model joint order.
    q: Vec<f64>,
    gravity_nm: f64,
    friction_nm: f64,
    /// Standard error of `friction_nm` from the samples' scatter about each direction mean
    /// (independent samples assumed).
    friction_se_nm: f64,
    speed_rad_s: f64,
    inertia_kg_m2: f64,
    /// Mean q̈ (up/down average) and the I·q̈ subtracted.
    ddq_rad_s2: f64,
    inertia_torque_nm: f64,
    n_up: usize,
    n_down: usize,
}

/// A bin left out for too few samples in one direction.
#[derive(Debug, Clone)]
struct DroppedBin {
    session: usize,
    plan_index: usize,
    center_rad: f64,
    n_up: usize,
    n_down: usize,
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (n > 0).then(|| sum / n as f64)
}

fn key(center_rad: f64) -> i64 {
    (center_rad / POSE_KEY_RAD).round() as i64
}

fn bins(
    sessions: &[Session],
    model: &UrdfGravityModel,
    joints: &[String],
) -> Result<(Vec<Bin>, Vec<DroppedBin>), GravityFitError> {
    let mut out = Vec::new();
    let mut dropped = Vec::new();
    for (s_i, s) in sessions.iter().enumerate() {
        for w in &s.waves {
            let PlanStep::Wave {
                joint,
                min_rad,
                max_rad,
                half_period_s,
                ..
            } = &s.plan.steps[w.plan_index]
            else {
                continue;
            };
            let Some(j) = joints.iter().position(|n| n == joint) else {
                continue;
            };
            let amplitude = (max_rad - min_rad) / 2.0;
            let centre = (max_rad + min_rad) / 2.0;
            let width = amplitude * BIN_SHARE_OF_AMPLITUDE;
            for k in [-1.0, 0.0, 1.0] {
                let center_rad = centre + k * width;
                let inside = |q: &[f64]| (q[j] - center_rad).abs() <= width / 2.0;
                let up: Vec<_> = w
                    .samples
                    .iter()
                    .filter(|x| x.dq > WAVE_DQ_DEADBAND_RAD_S && inside(&x.q))
                    .collect();
                let down: Vec<_> = w
                    .samples
                    .iter()
                    .filter(|x| x.dq < -WAVE_DQ_DEADBAND_RAD_S && inside(&x.q))
                    .collect();
                if up.len() < MIN_BIN_SAMPLES || down.len() < MIN_BIN_SAMPLES {
                    dropped.push(DroppedBin {
                        session: s_i,
                        plan_index: w.plan_index,
                        center_rad,
                        n_up: up.len(),
                        n_down: down.len(),
                    });
                    continue;
                }
                let q: Vec<f64> = (0..joints.len())
                    .map(|i| {
                        let u = mean(up.iter().map(|x| x.q[i])).unwrap_or_default();
                        let d = mean(down.iter().map(|x| x.q[i])).unwrap_or_default();
                        (u + d) / 2.0
                    })
                    .collect();
                let inertia = model.joint_inertia(joint, &q)?;
                let side = |xs: &[&super::WaveSample]| {
                    (
                        mean(xs.iter().map(|x| x.tau)).unwrap_or_default(),
                        mean(xs.iter().map(|x| x.ddq)).unwrap_or_default(),
                        mean(xs.iter().map(|x| x.dq.abs())).unwrap_or_default(),
                    )
                };
                let (tau_u, ddq_u, v_u) = side(&up);
                let (tau_d, ddq_d, v_d) = side(&down);
                let net_u = tau_u - inertia * ddq_u;
                let net_d = tau_d - inertia * ddq_d;
                // Sample variance of τ − I·q̈ about its direction mean (n ≥ MIN_BIN_SAMPLES).
                let var = |xs: &[&super::WaveSample], m: f64| {
                    xs.iter()
                        .map(|x| (x.tau - inertia * x.ddq - m).powi(2))
                        .sum::<f64>()
                        / (xs.len() - 1) as f64
                };
                let friction_se_nm = 0.5
                    * (var(&up, net_u) / up.len() as f64 + var(&down, net_d) / down.len() as f64)
                        .sqrt();
                out.push(Bin {
                    session: s_i,
                    plan_index: w.plan_index,
                    center_rad,
                    half_period_s: *half_period_s,
                    centre: k == 0.0,
                    q,
                    gravity_nm: (net_u + net_d) / 2.0,
                    friction_nm: (net_u - net_d) / 2.0,
                    friction_se_nm,
                    speed_rad_s: (v_u + v_d) / 2.0,
                    inertia_kg_m2: inertia,
                    ddq_rad_s2: (ddq_u + ddq_d) / 2.0,
                    inertia_torque_nm: inertia * (ddq_u + ddq_d) / 2.0,
                    n_up: up.len(),
                    n_down: down.len(),
                });
            }
        }
    }
    Ok((out, dropped))
}

/// Pooled standard deviation of values about their group means; `None` without a degree of
/// freedom.
fn pooled_sigma(groups: &[Vec<f64>]) -> Option<f64> {
    let (mut ss, mut dof) = (0.0, 0usize);
    for g in groups.iter().filter(|g| g.len() > 1) {
        let m = g.iter().sum::<f64>() / g.len() as f64;
        ss += g.iter().map(|v| (v - m).powi(2)).sum::<f64>();
        dof += g.len() - 1;
    }
    (dof > 0).then(|| (ss / dof as f64).sqrt())
}

/// Ordinary least squares `y ≈ X·c` by the normal equations (≤ 3 columns).
/// Returns the coefficients and `(XᵀX)⁻¹`, or `None` when `XᵀX` is singular.
fn least_squares(x: &[Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, Vec<Vec<f64>>)> {
    let n = x.first()?.len();
    let mut g = vec![vec![0.0; n]; n];
    let mut r = vec![0.0; n];
    for (row, &yi) in x.iter().zip(y) {
        for a in 0..n {
            r[a] += row[a] * yi;
            for b in 0..n {
                g[a][b] += row[a] * row[b];
            }
        }
    }
    let inv = invert(&g)?;
    let c = (0..n)
        .map(|a| (0..n).map(|b| inv[a][b] * r[b]).sum())
        .collect();
    Some((c, inv))
}

/// Gauss–Jordan inverse with partial pivoting; `None` when (numerically) singular.
fn invert(m: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let scale = m
        .iter()
        .flatten()
        .fold(0.0f64, |acc, v| acc.max(v.abs()))
        .max(f64::MIN_POSITIVE);
    let mut a: Vec<Vec<f64>> = m
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut r = row.clone();
            r.extend((0..n).map(|k| if k == i { 1.0 } else { 0.0 }));
            r
        })
        .collect();
    for col in 0..n {
        let pivot = (col..n).max_by(|&p, &q| a[p][col].abs().total_cmp(&a[q][col].abs()))?;
        if a[pivot][col].abs() <= scale * 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        let p = a[col][col];
        for v in a[col].iter_mut() {
            *v /= p;
        }
        let pivot_row = a[col].clone();
        for (r, row) in a.iter_mut().enumerate() {
            if r != col {
                let f = row[col];
                for (v, pv) in row.iter_mut().zip(&pivot_row) {
                    *v -= f * pv;
                }
            }
        }
    }
    Some(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

/// One pooled pose (bin centre) of a group.
#[derive(Debug, Clone)]
struct PoseEstimate {
    center_rad: f64,
    /// Mean swept-joint angle of its bins.
    q_rad: f64,
    gravity_nm: f64,
    bins: usize,
    /// Per session: (session_ts, mean gravity).
    session_means: Vec<(String, f64)>,
    /// max − min of the session means (≥ 2 sessions).
    spread_nm: Option<f64>,
    cad_nm: f64,
    fit_nm: Option<f64>,
}

impl PoseEstimate {
    /// Whether this pose sets the residual gate: it needs a replicate (a second bin from
    /// another session, speed or step). A single bin (one wave step in one session) has no
    /// spread to judge its residual by; it stays in the fit and is reported, never gated.
    fn gated(&self) -> bool {
        self.bins > 1
    }
}

/// Lumped fit of one sweep joint at one fixed pose of the others.
#[derive(Debug, Clone)]
struct GroupFit {
    joint: String,
    fixed: BTreeMap<String, f64>,
    q_fixed: Vec<f64>,
    sessions: Vec<String>,
    cad: LumpedTerms,
    fit: Option<LumpedTerms>,
    sigma_a_nm: Option<f64>,
    sigma_b_nm: Option<f64>,
    /// Fitted inertia error ΔI (kg·m²) and its σ, when ≥ 2 speeds separate it.
    delta_inertia: Option<(f64, Option<f64>)>,
    speeds: usize,
    poses: Vec<PoseEstimate>,
    sigma_cross_nm: Option<f64>,
    sigma_repeat_nm: Option<f64>,
    floor_nm: f64,
    gate_nm: Option<f64>,
    /// Max / RMS residual over the gated (replicated) poses.
    max_residual_nm: Option<f64>,
    rms_residual_nm: Option<f64>,
    /// Max residual over the single-bin poses (reported, not gated).
    max_ungated_residual_nm: Option<f64>,
    max_residual_cad_nm: f64,
    inertia_kg_m2: (f64, f64),
    refusals: Vec<String>,
}

impl GroupFit {
    fn accepted(&self) -> bool {
        self.fit.is_some() && self.refusals.is_empty()
    }
}

/// Readout resolution of `joint`'s torque estimate in joint space (Nm): the gate floor.
fn readout_floor(motors: &MotorsConfigFile, joint: &str) -> Result<f64, GravityFitError> {
    let m = motor_for_joint(motors, joint).ok_or_else(|| {
        GravityFitError::Plan(format!(
            "{joint}: no motors.yaml entry for the readout floor"
        ))
    })?;
    Ok(MitRanges::for_motor_type(m.motor_type).feedback_torque_step() * m.gear_ratio.abs())
}

/// `(sweep joint, fixed pose)` of a session: the other joints' plan poses, 1 µrad keys.
fn group_key(s: &Session) -> (String, Vec<(String, i64)>) {
    let fixed = s
        .plan
        .effective_fixed()
        .into_iter()
        .filter(|(j, _)| j != &s.plan.sweep_joint)
        .map(|(j, v)| (j, (v * 1e6).round() as i64))
        .collect();
    (s.plan.sweep_joint.clone(), fixed)
}

fn fit_group(
    inputs: &Inputs,
    members: &[usize],
    bins: &[Bin],
    opts: &FitOptions,
) -> Result<GroupFit, GravityFitError> {
    let first = &inputs.sessions[members[0]];
    let joint = first.plan.sweep_joint.clone();
    let j = inputs
        .joints
        .iter()
        .position(|n| n == &joint)
        .ok_or_else(|| GravityFitError::Plan(format!("sweep joint {joint} not modelled")))?;
    let fixed: BTreeMap<String, f64> = first
        .plan
        .effective_fixed()
        .into_iter()
        .filter(|(n, _)| n != &joint)
        .collect();
    let q_fixed: Vec<f64> = inputs
        .joints
        .iter()
        .map(|n| fixed.get(n).copied().unwrap_or(0.0))
        .collect();
    let cad = lumped_terms(inputs.model, &joint, &q_fixed)?;
    let floor_nm = readout_floor(inputs.motors, &joint)?;
    let mine: Vec<&Bin> = bins
        .iter()
        .filter(|b| members.contains(&b.session))
        .collect();
    let mut speeds: Vec<i64> = mine
        .iter()
        .map(|b| (b.half_period_s * 1000.0).round() as i64)
        .collect();
    speeds.sort_unstable();
    speeds.dedup();
    // The URDF inertia misses the rotor and may be wrong; its error ΔI·q̈ is odd about each
    // wave centre and scales with ω², so with ≥ 2 speeds it separates from the gravity slope
    // and is fitted. With one speed only the centre bins (q̈ ≈ 0 by symmetry) are used.
    let fit_inertia = speeds.len() >= 2;
    let used: Vec<&Bin> = mine
        .iter()
        .copied()
        .filter(|b| fit_inertia || b.centre)
        .collect();
    let rows: Vec<Vec<f64>> = used
        .iter()
        .map(|b| {
            let mut r = vec![b.q[j].sin(), b.q[j].cos()];
            if fit_inertia {
                r.push(b.ddq_rad_s2);
            }
            r
        })
        .collect();
    let y: Vec<f64> = used.iter().map(|b| b.gravity_nm).collect();
    let solved = (used.len() > rows.first().map_or(0, Vec::len))
        .then(|| least_squares(&rows, &y))
        .flatten();
    let delta_inertia = solved
        .as_ref()
        .and_then(|(c, _)| c.get(2).copied())
        .unwrap_or(0.0);
    let corrected = |b: &Bin| b.gravity_nm - delta_inertia * b.ddq_rad_s2;
    let mut by_key: BTreeMap<i64, Vec<&Bin>> = BTreeMap::new();
    for b in &used {
        by_key.entry(key(b.center_rad)).or_default().push(b);
    }
    let mut poses = Vec::new();
    let mut cross_groups = Vec::new();
    let mut repeat_groups = Vec::new();
    for group in by_key.values() {
        let mut per_session: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        for b in group {
            per_session.entry(b.session).or_default().push(corrected(b));
        }
        let session_means: Vec<(String, f64)> = per_session
            .iter()
            .map(|(s, v)| {
                (
                    inputs.sessions[*s].plan.session_ts.clone(),
                    v.iter().sum::<f64>() / v.len() as f64,
                )
            })
            .collect();
        let means: Vec<f64> = session_means.iter().map(|(_, m)| *m).collect();
        let spread_nm = (means.len() > 1).then(|| {
            means.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - means.iter().copied().fold(f64::INFINITY, f64::min)
        });
        cross_groups.push(means);
        repeat_groups.push(group.iter().map(|b| corrected(b)).collect::<Vec<_>>());
        let q_rad = mean(group.iter().map(|b| b.q[j])).unwrap_or_default();
        poses.push(PoseEstimate {
            center_rad: group[0].center_rad,
            q_rad,
            gravity_nm: mean(group.iter().map(|b| corrected(b))).unwrap_or_default(),
            bins: group.len(),
            session_means,
            spread_nm,
            cad_nm: cad.torque(q_rad),
            fit_nm: None,
        });
    }
    let sigma_cross_nm = pooled_sigma(&cross_groups);
    let sigma_repeat_nm = pooled_sigma(&repeat_groups);
    let gate_nm = sigma_cross_nm.map(|s| (GATE_SIGMAS * s).max(floor_nm));
    let inertia_kg_m2 = mine
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), b| {
            (lo.min(b.inertia_kg_m2), hi.max(b.inertia_kg_m2))
        });
    let max_residual_cad_nm = poses
        .iter()
        .map(|p| (p.gravity_nm - p.cad_nm).abs())
        .fold(0.0, f64::max);

    let mut refusals = Vec::new();
    let gated = poses.iter().filter(|p| p.gated()).count();
    if gated < MIN_POSES {
        refusals.push(format!(
            "{gated} replicated pose bins (of {}); A, B and a residual need ≥ {MIN_POSES} \
             (a single-bin pose is reported, never gated)",
            poses.len()
        ));
    }
    match (sigma_cross_nm, gate_nm) {
        (Some(s), Some(g)) if g > opts.max_residual_nm => refusals.push(format!(
            "cross-session spread: {GATE_SIGMAS}·σ_cross = {:.4} Nm (σ_cross {s:.4}) exceeds the \
             suite residual gate {:.2} Nm; the data are not repeatable enough to check it",
            GATE_SIGMAS * s,
            opts.max_residual_nm
        )),
        (None, _) | (_, None) => refusals
            .push("cross-session spread unavailable: needs ≥ 2 sessions sharing a pose bin".into()),
        _ => {}
    }
    let (mut fit, mut sigma_a_nm, mut sigma_b_nm, mut sigma_inertia) = (None, None, None, None);
    let (mut max_residual_nm, mut rms_residual_nm, mut max_ungated_residual_nm) =
        (None, None, None);
    match solved {
        None => refusals.push("A, B not solvable from these bins (singular)".into()),
        Some((c, inv)) => {
            let terms = LumpedTerms {
                a_nm: c[0],
                b_nm: c[1],
            };
            let (mut worst, mut worst_ungated): (f64, Option<f64>) = (0.0, None);
            let mut ss = 0.0;
            for p in &mut poses {
                let f = terms.torque(p.q_rad);
                p.fit_nm = Some(f);
                let r = (p.gravity_nm - f).abs();
                if p.gated() {
                    worst = worst.max(r);
                    ss += r.powi(2);
                } else {
                    worst_ungated = Some(worst_ungated.map_or(r, |w| w.max(r)));
                }
            }
            max_residual_nm = Some(worst);
            rms_residual_nm = Some((ss / gated.max(1) as f64).sqrt());
            max_ungated_residual_nm = worst_ungated;
            // Per-bin noise: the bin-level residual scatter, but never below the
            // cross-session spread (bins of one session share its offset).
            let dof = used.len() - c.len();
            let bin_ss: f64 = rows
                .iter()
                .zip(&y)
                .map(|(r, yi)| (yi - r.iter().zip(&c).map(|(a, b)| a * b).sum::<f64>()).powi(2))
                .sum();
            let sigma = (bin_ss / dof.max(1) as f64)
                .sqrt()
                .max(sigma_cross_nm.unwrap_or(0.0));
            sigma_a_nm = Some(sigma * inv[0][0].sqrt());
            sigma_b_nm = Some(sigma * inv[1][1].sqrt());
            sigma_inertia = fit_inertia.then(|| sigma * inv[2][2].sqrt());
            if let Some(g) = gate_nm {
                if worst > g {
                    refusals.push(format!(
                        "residual: max |pose mean − fit| {worst:.4} Nm > gate {g:.4} Nm"
                    ));
                }
                if let (Some(sa), Some(sb)) = (sigma_a_nm, sigma_b_nm) {
                    if sa.is_nan() || sb.is_nan() || sa > g || sb > g {
                        refusals.push(format!(
                            "identifiability: σ_A {sa:.4} / σ_B {sb:.4} Nm exceed the gate \
                             {g:.4} Nm; widen the sweep (more poses, larger range)"
                        ));
                    }
                }
            }
            fit = Some(terms);
        }
    }
    Ok(GroupFit {
        joint,
        fixed,
        q_fixed,
        sessions: members
            .iter()
            .map(|&s| inputs.sessions[s].plan.session_ts.clone())
            .collect(),
        cad,
        fit,
        sigma_a_nm,
        sigma_b_nm,
        delta_inertia: fit_inertia.then_some((delta_inertia, sigma_inertia)),
        speeds: speeds.len(),
        poses,
        sigma_cross_nm,
        sigma_repeat_nm,
        floor_nm,
        gate_nm,
        max_residual_nm,
        rms_residual_nm,
        max_ungated_residual_nm,
        max_residual_cad_nm,
        inertia_kg_m2,
        refusals,
    })
}

/// One pooled (pose bin, speed) friction point.
#[derive(Debug, Clone)]
struct FrictionPoint {
    center_rad: f64,
    half_period_s: f64,
    speed_rad_s: f64,
    friction_nm: f64,
    /// Standard error of `friction_nm` from its bins' samples.
    se_nm: f64,
    session_means: Vec<(String, f64)>,
    fit_nm: Option<f64>,
}

/// Friction of one swept joint from its wave bins.
#[derive(Debug, Clone)]
struct FrictionFit {
    joint: String,
    points: Vec<FrictionPoint>,
    speeds: usize,
    v_b_rad_s: f64,
    current: (f64, f64, f64),
    fc_nm: Option<f64>,
    fv: Option<f64>,
    fs_nm: Option<f64>,
    sigma_fc_nm: Option<f64>,
    sigma_fv: Option<f64>,
    fs_note: String,
    fv_note: String,
    sigma_cross_nm: Option<f64>,
    /// RMS of the points' sampling standard errors (the bin noise).
    sigma_bin_nm: Option<f64>,
    gate_nm: Option<f64>,
    max_residual_nm: Option<f64>,
    refusals: Vec<String>,
}

impl FrictionFit {
    fn proposal(&self) -> Option<FrictionProposal> {
        match (self.refusals.is_empty(), self.fc_nm, self.fv) {
            (true, Some(fc), Some(fv)) => Some(FrictionProposal {
                joint: self.joint.clone(),
                fc,
                fv,
                fs: self.fs_nm,
            }),
            _ => None,
        }
    }
}

fn fit_friction(
    inputs: &Inputs,
    joint: &str,
    bins: &[Bin],
    opts: &FitOptions,
) -> Result<FrictionFit, GravityFitError> {
    let entry = inputs.control.control.joints.get(joint);
    let v_b_rad_s = entry.map_or(
        marengo_config::DEFAULT_FRICTION_STRIBECK_VELOCITY_RAD_S,
        |e| e.friction.stribeck_velocity_rad_s(),
    );
    let current = entry.map_or((f64::NAN, f64::NAN, f64::NAN), |e| {
        (e.friction.fc, e.friction.fv, e.friction.static_nm())
    });
    let floor_nm = readout_floor(inputs.motors, joint)?;
    let mut by_key: BTreeMap<(i64, i64), Vec<&Bin>> = BTreeMap::new();
    for b in bins.iter().filter(|b| {
        matches!(&inputs.sessions[b.session].plan.steps[b.plan_index],
            PlanStep::Wave { joint: j, .. } if j == joint)
    }) {
        by_key
            .entry((key(b.center_rad), (b.half_period_s * 1000.0).round() as i64))
            .or_default()
            .push(b);
    }
    let mut points = Vec::new();
    let mut cross = Vec::new();
    for group in by_key.values() {
        let mut per_session: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        for b in group {
            per_session
                .entry(b.session)
                .or_default()
                .push(b.friction_nm);
        }
        let session_means: Vec<(String, f64)> = per_session
            .iter()
            .map(|(s, v)| {
                (
                    inputs.sessions[*s].plan.session_ts.clone(),
                    v.iter().sum::<f64>() / v.len() as f64,
                )
            })
            .collect();
        cross.push(session_means.iter().map(|(_, m)| *m).collect::<Vec<_>>());
        let se_nm = group
            .iter()
            .map(|b| b.friction_se_nm.powi(2))
            .sum::<f64>()
            .sqrt()
            / group.len() as f64;
        points.push(FrictionPoint {
            center_rad: group[0].center_rad,
            half_period_s: group[0].half_period_s,
            speed_rad_s: mean(group.iter().map(|b| b.speed_rad_s)).unwrap_or_default(),
            friction_nm: mean(group.iter().map(|b| b.friction_nm)).unwrap_or_default(),
            se_nm,
            session_means,
            fit_nm: None,
        });
    }
    let mut speeds: Vec<i64> = by_key.keys().map(|k| k.1).collect();
    speeds.sort_unstable();
    speeds.dedup();
    let sigma_cross_nm = pooled_sigma(&cross);
    // The gate is the bin noise, not the cross-session spread alone: a wave repeats the same
    // torque swings at the same q in every session, so σ_cross can be far below how well a
    // smooth fc + fv·v (+ Stribeck) can match bins at different poses and speeds. Each
    // point's own sampling error bounds that; σ_cross stays the repeatability check.
    let sigma_bin_nm = mean(points.iter().map(|p: &FrictionPoint| p.se_nm.powi(2))).map(f64::sqrt);
    let sigma_noise = match (sigma_bin_nm, sigma_cross_nm) {
        (Some(b), Some(c)) => Some(b.max(c)),
        (b, c) => b.or(c),
    };
    let gate_nm = sigma_cross_nm
        .and(sigma_noise)
        .map(|s| (GATE_SIGMAS * s).max(floor_nm).min(opts.max_residual_nm));
    let mut out = FrictionFit {
        joint: joint.to_string(),
        points,
        speeds: speeds.len(),
        v_b_rad_s,
        current,
        fc_nm: None,
        fv: None,
        fs_nm: None,
        sigma_fc_nm: None,
        sigma_fv: None,
        fs_note: String::new(),
        fv_note: String::new(),
        sigma_cross_nm,
        sigma_bin_nm,
        gate_nm,
        max_residual_nm: None,
        refusals: Vec::new(),
    };
    if out.speeds < 2 {
        out.refusals
            .push(format!("{} wave speed(s): fv needs ≥ 2", out.speeds));
    }
    if out.points.len() < MIN_POSES {
        out.refusals.push(format!(
            "{} pose/speed bins; fc, fv and a residual need ≥ {MIN_POSES}",
            out.points.len()
        ));
    }
    match sigma_cross_nm {
        Some(s) if GATE_SIGMAS * s > opts.max_residual_nm => out.refusals.push(format!(
            "cross-session spread: {GATE_SIGMAS}·σ_cross = {:.4} Nm (σ_cross {s:.4}) exceeds {:.2} Nm",
            GATE_SIGMAS * s,
            opts.max_residual_nm
        )),
        None => out.refusals.push(
            "cross-session spread unavailable: needs ≥ 2 sessions sharing a pose/speed bin"
                .into(),
        ),
        Some(_) => {}
    }
    let y: Vec<f64> = out.points.iter().map(|p| p.friction_nm).collect();
    let linear: Vec<Vec<f64>> = out
        .points
        .iter()
        .map(|p| vec![1.0, p.speed_rad_s])
        .collect();
    let Some((c, inv)) = (out.points.len() >= 2)
        .then(|| least_squares(&linear, &y))
        .flatten()
    else {
        out.refusals
            .push("fc, fv not solvable (one speed or one bin)".into());
        return Ok(out);
    };
    let (mut fc, mut fv) = (c[0], c[1]);
    let residual_sigma = |coef: &[f64], rows: &[Vec<f64>]| -> Option<f64> {
        let dof = y.len().checked_sub(coef.len()).filter(|&d| d > 0)?;
        let ss: f64 = rows
            .iter()
            .zip(&y)
            .map(|(r, yi)| (yi - r.iter().zip(coef).map(|(a, b)| a * b).sum::<f64>()).powi(2))
            .sum();
        Some((ss / dof as f64).sqrt())
    };
    let sigma = sigma_noise.or_else(|| residual_sigma(&c, &linear));
    out.sigma_fc_nm = sigma.map(|s| s * inv[0][0].sqrt());
    out.sigma_fv = sigma.map(|s| s * inv[1][1].sqrt());
    // Stribeck: fs only when the breakaway excess is significant at these speeds.
    let stribeck: Vec<Vec<f64>> = out
        .points
        .iter()
        .map(|p| vec![1.0, p.speed_rad_s, (-p.speed_rad_s / v_b_rad_s).exp()])
        .collect();
    out.fs_note = match (out.points.len() > 3)
        .then(|| least_squares(&stribeck, &y))
        .flatten()
    {
        Some((c3, inv3)) => {
            let s3 = sigma_noise.or_else(|| residual_sigma(&c3, &stribeck));
            match s3.map(|s| s * inv3[2][2].sqrt()) {
                Some(se) if c3[2] > 0.0 && c3[2] > GATE_SIGMAS * se => {
                    fc = c3[0];
                    fv = c3[1];
                    out.fs_nm = Some(c3[0] + c3[2]);
                    format!(
                        "fs − fc = {:.4} ± {se:.4} Nm at v_b {v_b_rad_s} rad/s",
                        c3[2]
                    )
                }
                Some(se) => format!(
                    "not identifiable: fs − fc = {:.4} ± {se:.4} Nm at v_b {v_b_rad_s} rad/s \
                     (slowest bin {:.3} rad/s)",
                    c3[2],
                    out.points
                        .iter()
                        .map(|p| p.speed_rad_s)
                        .fold(f64::INFINITY, f64::min)
                ),
                None => "not identifiable: no spread estimate".into(),
            }
        }
        None => "not identifiable: Stribeck column singular at these speeds".into(),
    };
    // fv only when the speeds resolve it; otherwise friction is the Coulomb mean (fv 0) and
    // nothing is extrapolated from a narrow speed span to the planner's cruise speeds.
    let (v_lo, v_hi) = out
        .points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.speed_rad_s), hi.max(p.speed_rad_s))
        });
    out.fv_note = match (out.fs_nm, out.sigma_fv) {
        (Some(_), _) => "with the Stribeck term".into(),
        (None, Some(se)) if fv.abs() > GATE_SIGMAS * se => {
            format!("fv = {fv:.4} ± {se:.4} Nm·s/rad over |q̇| {v_lo:.3}–{v_hi:.3} rad/s")
        }
        (None, se) => {
            let note = format!(
                "not identifiable: fv = {fv:.4} ± {} Nm·s/rad over |q̇| {v_lo:.3}–{v_hi:.3} \
                 rad/s; Coulomb only (fc = mean, fv = 0)",
                se.map_or("-".into(), |s| format!("{s:.4}"))
            );
            fc = mean(y.iter().copied()).unwrap_or(f64::NAN);
            fv = 0.0;
            out.sigma_fc_nm = sigma.map(|s| s / (y.len() as f64).sqrt());
            note
        }
    };
    let coef: Vec<f64> = match out.fs_nm {
        Some(fs) => vec![fc, fv, fs - fc],
        None => vec![fc, fv],
    };
    let mut worst: f64 = 0.0;
    for p in &mut out.points {
        let mut f = coef[0] + coef[1] * p.speed_rad_s;
        if let Some(d) = coef.get(2) {
            f += d * (-p.speed_rad_s / v_b_rad_s).exp();
        }
        p.fit_nm = Some(f);
        worst = worst.max((p.friction_nm - f).abs());
    }
    out.max_residual_nm = Some(worst);
    if let Some(g) = gate_nm {
        if worst > g {
            out.refusals.push(format!(
                "residual: max |bin − fit| {worst:.4} Nm > gate {g:.4} Nm"
            ));
        }
    }
    if fc.is_nan() || fc <= 0.0 {
        out.refusals.push(format!("fc {fc:.4} Nm ≤ 0"));
    }
    if fv.is_nan() || fv < 0.0 {
        out.refusals.push(format!(
            "fv {fv:.5} Nm·s/rad < 0: friction falls with speed here (Stribeck below these \
             speeds?) and no significant fs term models it"
        ));
    }
    out.fc_nm = Some(fc);
    out.fv = Some(fv);
    Ok(out)
}

/// The URDF patch for the accepted groups, with its checks.
struct UrdfPatch {
    links: Vec<FittedLink>,
    /// Per group: (joint, fixed, A/B reproduced by the patched URDF).
    reproduced: Vec<LumpedTerms>,
    /// Max |Δτ_g| the shift causes on every joint over each group's pose bins.
    side_effects_nm: Vec<(String, f64)>,
    proposed: PathBuf,
    patch: PathBuf,
}

fn propose_urdf(
    inputs: &Inputs,
    groups: &[GroupFit],
    stem: &str,
    opts: &FitOptions,
) -> Result<Result<UrdfPatch, String>, GravityFitError> {
    let accepted: Vec<&GroupFit> = groups.iter().filter(|g| g.accepted()).collect();
    if accepted.is_empty() {
        return Ok(Err("no group passed its gates".into()));
    }
    let targets: Vec<LumpedTarget> = accepted
        .iter()
        .filter_map(|g| {
            g.fit.map(|terms| LumpedTarget {
                joint: g.joint.clone(),
                q_fixed: g.q_fixed.clone(),
                terms,
            })
        })
        .collect();
    let allowed = limb_links(inputs.model, inputs.limbs, inputs.joints)?;
    let links = match lumped_com_patch(inputs.model, &targets, &allowed) {
        Ok(l) => l,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let shift = |l: &FittedLink| {
        (0..3)
            .map(|i| (l.fitted.com_m[i] - l.cad.com_m[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    if let Some(l) = links.iter().find(|l| shift(l) > opts.max_com_offset_m) {
        return Ok(Err(format!(
            "implausible: {} COM would move {:.4} m (> {:.3} m)",
            l.link,
            shift(l),
            opts.max_com_offset_m
        )));
    }
    let patched_text = patch_inertials(inputs.pi_urdf, &links)?;
    let proposed = inputs
        .args
        .out_dir
        .join(format!("{stem}.{PROPOSED_URDF_FILE}"));
    std::fs::write(&proposed, &patched_text).map_err(io(&proposed))?;
    let patched = gravity_model_from_urdf(&proposed, inputs.joints)?;
    let mut reproduced = Vec::new();
    for t in &targets {
        let got = lumped_terms(&patched, &t.joint, &t.q_fixed)?;
        if (got.a_nm - t.terms.a_nm).abs() > PATCH_ROUNDTRIP_TOL_NM
            || (got.b_nm - t.terms.b_nm).abs() > PATCH_ROUNDTRIP_TOL_NM
        {
            return Err(GravityFitError::Patch(format!(
                "{}: patched URDF gives A {:.4} B {:.4} Nm, fit A {:.4} B {:.4} Nm",
                t.joint, got.a_nm, got.b_nm, t.terms.a_nm, t.terms.b_nm
            )));
        }
        reproduced.push(got);
    }
    let mut side = vec![0.0f64; inputs.joints.len()];
    for g in &accepted {
        let j = inputs.joints.iter().position(|n| n == &g.joint);
        for p in &g.poses {
            let mut q = g.q_fixed.clone();
            if let Some(j) = j {
                q[j] = p.q_rad;
            }
            let before = inputs.model.gravity_torques(&q)?;
            let after = patched.gravity_torques(&q)?;
            for (k, s) in side.iter_mut().enumerate() {
                *s = s.max((after[k] - before[k]).abs());
            }
        }
    }
    let diff = unified_diff(inputs.pi_urdf, &patched_text, PATCH_PATH);
    let patch = inputs.args.out_dir.join(format!("{stem}.urdf.patch"));
    std::fs::write(&patch, diff).map_err(io(&patch))?;
    Ok(Ok(UrdfPatch {
        links,
        reproduced,
        side_effects_nm: inputs.joints.iter().cloned().zip(side).collect(),
        proposed,
        patch,
    }))
}

pub(super) fn run(inputs: &Inputs) -> Result<Outcome, GravityFitError> {
    let opts = FitOptions::default();
    for s in inputs.sessions {
        if let Some((i, other)) = s.plan.steps.iter().enumerate().find(
            |(_, st)| matches!(st, PlanStep::Wave { joint, .. } if joint != &s.plan.sweep_joint),
        ) {
            return Err(GravityFitError::Plan(format!(
                "{}: wave step {i} moves {}, not the sweep joint {}",
                s.dir.display(),
                other.joint(),
                s.plan.sweep_joint
            )));
        }
    }
    let (bins, dropped) = bins(inputs.sessions, inputs.model, inputs.joints)?;
    let mut keys: Vec<(String, Vec<(String, i64)>)> = Vec::new();
    let mut members: Vec<Vec<usize>> = Vec::new();
    for (i, s) in inputs.sessions.iter().enumerate() {
        let k = group_key(s);
        match keys.iter().position(|x| *x == k) {
            Some(g) => members[g].push(i),
            None => {
                keys.push(k);
                members.push(vec![i]);
            }
        }
    }
    let groups = members
        .iter()
        .map(|m| fit_group(inputs, m, &bins, &opts))
        .collect::<Result<Vec<_>, _>>()?;
    let mut swept: Vec<&str> = Vec::new();
    for s in inputs.sessions {
        if !swept.contains(&s.plan.sweep_joint.as_str()) {
            swept.push(&s.plan.sweep_joint);
        }
    }
    let friction = swept
        .iter()
        .map(|j| fit_friction(inputs, j, &bins, &opts))
        .collect::<Result<Vec<_>, _>>()?;

    let stem = record_stem(inputs.sessions)?;
    let out_dir = &inputs.args.out_dir;
    std::fs::create_dir_all(out_dir).map_err(io(out_dir))?;
    let urdf = propose_urdf(inputs, &groups, &stem, &opts)?;
    let captured_control =
        std::fs::read_to_string(inputs.control_path).map_err(io(inputs.control_path))?;
    let proposals: Vec<FrictionProposal> = friction.iter().filter_map(|f| f.proposal()).collect();
    let control_patch = match patch_control_friction(&captured_control, &proposals) {
        Some(patched) => {
            let path = out_dir.join(format!("{stem}.control.patch"));
            let diff = unified_diff(&captured_control, &patched, CONTROL_PATCH_PATH);
            std::fs::write(&path, diff).map_err(io(&path))?;
            Some(path)
        }
        None => None,
    };

    let record = record_json(
        inputs,
        &groups,
        &friction,
        &bins,
        &dropped,
        &urdf,
        &control_patch,
        &opts,
    );
    let json_path = out_dir.join(format!("{stem}.json"));
    let pretty = serde_json::to_string_pretty(&record).map_err(|source| GravityFitError::Json {
        path: json_path.clone(),
        source,
    })?;
    std::fs::write(&json_path, format!("{pretty}\n")).map_err(io(&json_path))?;
    let md_path = out_dir.join(format!("{stem}.md"));
    let md = record_markdown(
        inputs,
        &stem,
        &groups,
        &friction,
        &dropped,
        &urdf,
        &control_patch,
        &opts,
    );
    std::fs::write(&md_path, md).map_err(io(&md_path))?;

    println!("{}", summary(&groups, &friction));
    println!("record: {}", json_path.display());
    println!("record: {}", md_path.display());
    if let Some(c) = &control_patch {
        println!("proposed friction patch: {}", c.display());
    }
    match &urdf {
        Ok(p) => {
            println!("proposed patch: {}", p.patch.display());
            println!(
                "Nothing applied. Review, `git apply` it to the Pi URDF copy at {PATCH_PATH} \
                 (ADR 0017), then pi_sync_bench_urdf as a separate explicit step. \
                 Apply the friction patch by hand to {CONTROL_PATCH_PATH}."
            );
            Ok(Outcome::Proposed)
        }
        Err(why) => {
            println!("no URDF patch proposed: {why}");
            Ok(Outcome::Refused)
        }
    }
}

fn opt(v: Option<f64>, digits: i32) -> Value {
    v.map_or(Value::Null, |x| json!(round(x, digits)))
}

#[allow(clippy::too_many_arguments)]
fn record_json(
    inputs: &Inputs,
    groups: &[GroupFit],
    friction: &[FrictionFit],
    bins: &[Bin],
    dropped: &[DroppedBin],
    urdf: &Result<UrdfPatch, String>,
    control_patch: &Option<PathBuf>,
    opts: &FitOptions,
) -> Value {
    let ts = |s: usize| inputs.sessions[s].plan.session_ts.clone();
    json!({
        "version": 2,
        "kind": "gravity_calibration",
        "method": "wave",
        "accepted": urdf.is_ok(),
        "verdict": match urdf {
            Ok(_) => "accepted".to_string(),
            Err(why) => format!("refused: {why}"),
        },
        "sessions": inputs.sessions.iter().map(|s| json!({
            "dir": display_relative(&s.dir),
            "session_ts": s.plan.session_ts,
            "plan_version": s.plan.version,
            "session_complete": s.plan.session_complete,
            "profile": s.plan.profile,
            "sweep_joint": s.plan.sweep_joint,
            "fixed_rad": s.plan.effective_fixed(),
            "poses_rad": s.plan.poses_rad,
            "wave_steps": s.waves.iter().map(|w| json!({
                "plan_index": w.plan_index,
                "moving_samples": w.samples.len(),
            })).collect::<Vec<_>>(),
            "skipped_steps": s.skipped.iter().map(|k| json!({
                "step": k.index, "kind": k.kind, "joint": k.joint,
                "target_rad": k.target_rad, "reason": k.reason,
            })).collect::<Vec<_>>(),
            "gravity_gate_report": s.plan.gravity_gate_report,
        })).collect::<Vec<_>>(),
        "urdf_base": "Pi live URDF captured before the session (pi-marengo.urdf), ADR 0017",
        "local_urdf_matches_pi": inputs.local_matches_pi,
        "options": {
            "bin_share_of_amplitude": BIN_SHARE_OF_AMPLITUDE,
            "min_bin_samples": MIN_BIN_SAMPLES,
            "deadband_rad_s": WAVE_DQ_DEADBAND_RAD_S,
            "gate_sigmas": GATE_SIGMAS,
            "suite_residual_gate_nm": opts.max_residual_nm,
            "max_com_shift_m": opts.max_com_offset_m,
            "inertia": "I from the URDF links about the swept axis (no rotor), q̈ = measured second difference; with ≥ 2 speeds the error ΔI is fitted with A, B, else only centre bins are used",
        },
        "groups": groups.iter().map(|g| json!({
            "joint": g.joint,
            "fixed_rad": g.fixed,
            "sessions": g.sessions,
            "accepted": g.accepted(),
            "refusals": g.refusals,
            "cad": {"a_nm": round(g.cad.a_nm, 5), "b_nm": round(g.cad.b_nm, 5),
                    "c_nm": round(g.cad.amplitude_nm(), 5), "phi_rad": round(g.cad.phase_rad(), 5)},
            "fit": g.fit.map_or(Value::Null, |f| json!({
                "a_nm": round(f.a_nm, 5), "b_nm": round(f.b_nm, 5),
                "c_nm": round(f.amplitude_nm(), 5), "phi_rad": round(f.phase_rad(), 5),
                "sigma_a_nm": opt(g.sigma_a_nm, 5), "sigma_b_nm": opt(g.sigma_b_nm, 5),
                "delta_inertia_kg_m2": g.delta_inertia.map_or(Value::Null, |(d, s)| json!({
                    "value": round(d, 5), "sigma": opt(s, 5),
                })),
                "speeds": g.speeds,
            })),
            "gate": {
                "cross_session_sigma_nm": opt(g.sigma_cross_nm, 5),
                "repeat_sigma_nm": opt(g.sigma_repeat_nm, 5),
                "readout_floor_nm": round(g.floor_nm, 6),
                "residual_gate_nm": opt(g.gate_nm, 5),
                "max_residual_nm": opt(g.max_residual_nm, 5),
                "rms_residual_nm": opt(g.rms_residual_nm, 5),
                "max_ungated_residual_nm": opt(g.max_ungated_residual_nm, 5),
                "max_residual_cad_nm": round(g.max_residual_cad_nm, 5),
            },
            "inertia_kg_m2": [round(g.inertia_kg_m2.0, 5), round(g.inertia_kg_m2.1, 5)],
            "poses": g.poses.iter().map(|p| json!({
                "center_rad": round(p.center_rad, 4),
                "q_rad": round(p.q_rad, 5),
                "gravity_nm": round(p.gravity_nm, 5),
                "bins": p.bins,
                "gated": p.gated(),
                "session_means_nm": p.session_means.iter()
                    .map(|(t, m)| (t.clone(), json!(round(*m, 5))))
                    .collect::<serde_json::Map<String, Value>>(),
                "cross_session_spread_nm": opt(p.spread_nm, 5),
                "cad_nm": round(p.cad_nm, 5),
                "fit_nm": opt(p.fit_nm, 5),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "bins": bins.iter().map(|b| json!({
            "session": ts(b.session),
            "step": b.plan_index,
            "center_rad": round(b.center_rad, 4),
            "half_period_s": b.half_period_s,
            "q_rad": rounded_opt(&b.q),
            "gravity_nm": round(b.gravity_nm, 5),
            "friction_nm": round(b.friction_nm, 5),
            "friction_se_nm": round(b.friction_se_nm, 5),
            "speed_rad_s": round(b.speed_rad_s, 4),
            "inertia_torque_nm": round(b.inertia_torque_nm, 5),
            "samples": [b.n_up, b.n_down],
        })).collect::<Vec<_>>(),
        "dropped_bins": dropped.iter().map(|d| json!({
            "session": ts(d.session),
            "step": d.plan_index,
            "center_rad": round(d.center_rad, 4),
            "samples": [d.n_up, d.n_down],
        })).collect::<Vec<_>>(),
        "friction": friction.iter().map(|f| (f.joint.clone(), json!({
            "accepted": f.proposal().is_some(),
            "refusals": f.refusals,
            "fc_nm": opt(f.fc_nm, 5),
            "fv": opt(f.fv, 5),
            "fs_nm": opt(f.fs_nm, 5),
            "sigma_fc_nm": opt(f.sigma_fc_nm, 5),
            "sigma_fv": opt(f.sigma_fv, 5),
            "fs_note": f.fs_note,
            "fv_note": f.fv_note,
            "v_b_rad_s": f.v_b_rad_s,
            "current": {"fc": f.current.0, "fv": f.current.1, "fs": f.current.2},
            "speeds": f.speeds,
            "cross_session_sigma_nm": opt(f.sigma_cross_nm, 5),
            "bin_sigma_nm": opt(f.sigma_bin_nm, 5),
            "residual_gate_nm": opt(f.gate_nm, 5),
            "max_residual_nm": opt(f.max_residual_nm, 5),
            "points": f.points.iter().map(|p| json!({
                "center_rad": round(p.center_rad, 4),
                "half_period_s": p.half_period_s,
                "speed_rad_s": round(p.speed_rad_s, 4),
                "friction_nm": round(p.friction_nm, 5),
                "se_nm": round(p.se_nm, 5),
                "session_means_nm": p.session_means.iter()
                    .map(|(t, m)| (t.clone(), json!(round(*m, 5))))
                    .collect::<serde_json::Map<String, Value>>(),
                "fit_nm": opt(p.fit_nm, 5),
            })).collect::<Vec<_>>(),
        }))).collect::<serde_json::Map<String, Value>>(),
        "urdf_patch": match urdf {
            Ok(p) => json!({
                "patch": display_relative(&p.patch),
                "proposed_urdf": display_relative(&p.proposed),
                "links": p.links.iter().map(|l| json!({
                    "link": l.link,
                    "mass_kg": l.cad.mass_kg,
                    "com_m": {"cad": l.cad.com_m, "fitted": rounded_opt(&l.fitted.com_m)},
                    "shift_m": rounded_opt(&[
                        l.fitted.com_m[0] - l.cad.com_m[0],
                        l.fitted.com_m[1] - l.cad.com_m[1],
                        l.fitted.com_m[2] - l.cad.com_m[2],
                    ]),
                })).collect::<Vec<_>>(),
                "reproduced": p.reproduced.iter()
                    .map(|t| json!({"a_nm": round(t.a_nm, 5), "b_nm": round(t.b_nm, 5)}))
                    .collect::<Vec<_>>(),
                "max_gravity_change_nm": p.side_effects_nm.iter()
                    .map(|(j, v)| (j.clone(), json!(round(*v, 5))))
                    .collect::<serde_json::Map<String, Value>>(),
            }),
            Err(why) => json!({"refused": why}),
        },
        "control_patch": control_patch.as_deref().map(display_relative),
        "apply": "Never automatic. Review the patches; apply the URDF patch to the Pi URDF copy at assets/urdf/marengo.urdf, commit, then pi_sync_bench_urdf (ADR 0017). Apply the friction patch by hand to config/control.yaml on the Pi.",
    })
}

fn rounded_opt(v: &[f64]) -> Vec<f64> {
    v.iter().map(|x| round(*x, 5)).collect()
}

fn summary(groups: &[GroupFit], friction: &[FrictionFit]) -> String {
    let mut s = String::from("gravity-fit (wave):");
    for g in groups {
        let fit = g.fit.map_or("-".to_string(), |f| {
            format!(
                "A {:.4} ±{:.4} B {:.4} ±{:.4}",
                f.a_nm,
                g.sigma_a_nm.unwrap_or(f64::NAN),
                f.b_nm,
                g.sigma_b_nm.unwrap_or(f64::NAN)
            )
        });
        let _ = write!(
            s,
            "\n  {} (CAD A {:.4} B {:.4}): {fit} | gate {} | max residual {} | {}",
            g.joint,
            g.cad.a_nm,
            g.cad.b_nm,
            g.gate_nm.map_or("-".into(), |v| format!("{v:.4}")),
            g.max_residual_nm.map_or("-".into(), |v| format!("{v:.4}")),
            if g.accepted() {
                "accepted".to_string()
            } else {
                format!("refused: {}", g.refusals.join("; "))
            }
        );
    }
    for f in friction {
        let _ = write!(
            s,
            "\n  friction {}: fc {} fv {} fs {} | {}",
            f.joint,
            f.fc_nm.map_or("-".into(), |v| format!("{v:.4}")),
            f.fv.map_or("-".into(), |v| format!("{v:.5}")),
            f.fs_nm.map_or("-".into(), |v| format!("{v:.4}")),
            if f.proposal().is_some() {
                "proposed".to_string()
            } else {
                format!("no patch: {}", f.refusals.join("; "))
            }
        );
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn record_markdown(
    inputs: &Inputs,
    stem: &str,
    groups: &[GroupFit],
    friction: &[FrictionFit],
    dropped: &[DroppedBin],
    urdf: &Result<UrdfPatch, String>,
    control_patch: &Option<PathBuf>,
    opts: &FitOptions,
) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Gravity calibration {stem} (wave method)\n");
    let _ = writeln!(
        md,
        "**URDF patch:** {}\n",
        match urdf {
            Ok(_) => "proposed".to_string(),
            Err(why) => format!("none ({why})"),
        }
    );
    let _ = writeln!(
        md,
        "Up/down bin means of local waves (`marengo-log-cli gravity-fit`, wave method): \
         gravity = mean of the two directions after I·q̈, friction = half their difference. \
         Each swept joint is fitted as the lumped `A·sin q + B·cos q` its URDF structure \
         implies; its gate is {GATE_SIGMAS}·σ of the cross-session spread, floored at the \
         torque readout step, and must stay within the suite's {:.2} Nm. Only replicated \
         poses (≥ 2 bins) are gated; a single-bin pose stays in the fit and is reported. \
         The friction gate is {GATE_SIGMAS}·max(bin noise, σ_cross), capped at the same \
         {:.2} Nm; fv is kept only when the speeds resolve it.\n",
        opts.max_residual_nm, opts.max_residual_nm
    );
    let _ = writeln!(md, "## Sessions\n");
    let _ = writeln!(
        md,
        "| session | plan | complete | sweep | fixed | wave steps | skipped |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|");
    for s in inputs.sessions {
        let fixed = s
            .plan
            .effective_fixed()
            .iter()
            .map(|(j, v)| format!("{j}={v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            md,
            "| {} | v{} | {} | {} | {} | {} | {} |",
            s.plan.session_ts,
            s.plan.version,
            s.plan.session_complete,
            s.plan.sweep_joint,
            if fixed.is_empty() { "-".into() } else { fixed },
            s.waves.len(),
            s.skipped.len()
        );
    }
    let _ = writeln!(
        md,
        "\nURDF base: Pi live URDF captured before the session (ADR 0017). Local `{PATCH_PATH}` \
         matches it: {}.\n",
        if inputs.local_matches_pi {
            "yes"
        } else {
            "**no**: copy the Pi URDF into the repo before applying the patch"
        }
    );
    for g in groups {
        let _ = writeln!(md, "## {} (fixed: {:?})\n", g.joint, g.fixed);
        let _ = writeln!(
            md,
            "**{}**{}\n",
            if g.accepted() { "Accepted" } else { "Refused" },
            if g.refusals.is_empty() {
                String::new()
            } else {
                format!(": {}", g.refusals.join("; "))
            }
        );
        let _ = writeln!(md, "| term | CAD | fit | ±σ |");
        let _ = writeln!(md, "|---|---|---|---|");
        let f = g.fit;
        let num = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.4}"));
        let _ = writeln!(
            md,
            "| A (sin q) | {:.4} | {} | {} |",
            g.cad.a_nm,
            num(f.map(|t| t.a_nm)),
            num(g.sigma_a_nm)
        );
        let _ = writeln!(
            md,
            "| B (cos q) | {:.4} | {} | {} |",
            g.cad.b_nm,
            num(f.map(|t| t.b_nm)),
            num(g.sigma_b_nm)
        );
        let _ = writeln!(
            md,
            "| C (amplitude) | {:.4} | {} | |",
            g.cad.amplitude_nm(),
            num(f.map(|t| t.amplitude_nm()))
        );
        let _ = writeln!(
            md,
            "| φ (rad) | {:.4} | {} | |\n",
            g.cad.phase_rad(),
            num(f.map(|t| t.phase_rad()))
        );
        let _ = writeln!(
            md,
            "Gate: σ_cross {} Nm, σ_repeat {} Nm, readout floor {:.4} Nm → gate {} Nm; max \
             residual {} over replicated poses ({} over single-bin poses, not gated; CAD \
             {:.4}) Nm. I {:.4}–{:.4} kg·m² (URDF, no rotor); {}.\n",
            num(g.sigma_cross_nm),
            num(g.sigma_repeat_nm),
            g.floor_nm,
            num(g.gate_nm),
            num(g.max_residual_nm),
            num(g.max_ungated_residual_nm),
            g.max_residual_cad_nm,
            g.inertia_kg_m2.0,
            g.inertia_kg_m2.1,
            match g.delta_inertia {
                Some((d, s)) => format!(
                    "fitted ΔI {d:.4} ± {} kg·m² over {} speeds",
                    s.map_or("-".into(), |s| format!("{s:.4}")),
                    g.speeds
                ),
                None => "one speed: centre bins only, ΔI not fitted".into(),
            }
        );
        let _ = writeln!(
            md,
            "| bin centre | q | gravity | bins | gated | cross-session spread | CAD | fit | residual |"
        );
        let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");
        for p in &g.poses {
            let _ = writeln!(
                md,
                "| {:.4} | {:.4} | {:.4} | {} | {} | {} | {:.4} | {} | {} |",
                p.center_rad,
                p.q_rad,
                p.gravity_nm,
                p.bins,
                if p.gated() { "yes" } else { "no" },
                num(p.spread_nm),
                p.cad_nm,
                num(p.fit_nm),
                num(p.fit_nm.map(|f| p.gravity_nm - f))
            );
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "## Friction\n");
    let _ = writeln!(
        md,
        "| joint | current fc / fv / fs | fit fc | fv | fs | σ_cross | bin noise | gate | max residual | speeds | patch |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|---|");
    for f in friction {
        let num = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.4}"));
        let _ = writeln!(
            md,
            "| {} | {} / {} / {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            f.joint,
            f.current.0,
            f.current.1,
            f.current.2,
            num(f.fc_nm),
            f.fv.map_or("-".to_string(), |x| format!("{x:.5}")),
            num(f.fs_nm),
            num(f.sigma_cross_nm),
            num(f.sigma_bin_nm),
            num(f.gate_nm),
            num(f.max_residual_nm),
            f.speeds,
            if f.proposal().is_some() {
                "proposed".to_string()
            } else {
                f.refusals.join("; ")
            }
        );
        let _ = writeln!(md, "\nfs: {}\n\nfv: {}\n", f.fs_note, f.fv_note);
    }
    if !dropped.is_empty() {
        let _ = writeln!(
            md,
            "## Dropped bins (< {MIN_BIN_SAMPLES} moving samples in a direction)\n"
        );
        let _ = writeln!(md, "| session | step | centre | up | down |");
        let _ = writeln!(md, "|---|---|---|---|---|");
        for d in dropped {
            let _ = writeln!(
                md,
                "| {} | {} | {:.4} | {} | {} |",
                inputs.sessions[d.session].plan.session_ts,
                d.plan_index,
                d.center_rad,
                d.n_up,
                d.n_down
            );
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "## Apply (never automatic)\n");
    match urdf {
        Ok(p) => {
            let _ = writeln!(
                md,
                "URDF: the smallest COM shift of the carried links that reproduces the fitted \
                 A, B (masses unchanged):\n"
            );
            let _ = writeln!(md, "| link | COM CAD → fitted (m) |");
            let _ = writeln!(md, "|---|---|");
            for l in &p.links {
                let _ = writeln!(
                    md,
                    "| `{}` | {} → {} |",
                    l.link,
                    fmt_xyz(l.cad.com_m),
                    fmt_xyz(l.fitted.com_m)
                );
            }
            let _ = writeln!(
                md,
                "\nMax |Δτ_g| over the fitted poses: {}.\n",
                p.side_effects_nm
                    .iter()
                    .map(|(j, v)| format!("{j} {v:.4} Nm"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let _ =
                writeln!(
                md,
                "1. Make `{PATCH_PATH}` equal to the Pi URDF (ADR 0017), then `git apply` `{}`.\n\
                 2. `pi_sync_bench_urdf`, then `pi_restart_marengo_pi`.\n\
                 3. Re-run the phase as a no-refit check.",
                p.patch.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            );
        }
        Err(why) => {
            let _ = writeln!(md, "No URDF patch: {why}.");
        }
    }
    match control_patch {
        Some(c) => {
            let _ = writeln!(
                md,
                "\nFriction: review `{}` and apply it by hand to `{CONTROL_PATCH_PATH}` on the Pi.",
                c.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            );
        }
        None => {
            let _ = writeln!(md, "\nNo friction patch (see the Friction table).");
        }
    }
    md
}
