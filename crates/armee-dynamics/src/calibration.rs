//! Gravity-model calibration from bench holding torques.
//!
//! Fits per-link **mass scale** and/or **COM offset along the link's principal axis** so
//! [`UrdfGravityModel`] reproduces measured steady-state joint torques. The operator runs
//! this whenever right-arm hardware changes; gram-level weighing of the finished arm
//! supersedes it (`docs/commissioning/limb-playbook.md`).
//!
//! ## Model
//!
//! A link contributes `m · P(c, q)` to τ_g, where `P` is the holding torque of a unit point
//! mass at link-frame point `c` ([`UrdfGravityModel::point_mass_torques`]) and is affine in
//! `c`. With parameters `s` (mass scale) and `d` (COM offset, m):
//!
//! ```text
//! m = s · m_cad        c = c_cad + d · u        u = c_cad / |c_cad|
//! ```
//!
//! The principal axis `u` is the direction from the link frame origin (its parent joint) to
//! the CAD COM, so `d` moves the COM along the link without changing its direction.
//!
//! ## Fit
//!
//! Regularised Gauss–Newton (MAP with a Gaussian prior centred on CAD):
//! `min Σ (r/σ_τ)² + Σ ((θ − θ_cad)/σ_θ)²`. The problem is bilinear only when a link has both
//! parameters, so it converges in a few iterations.
//!
//! A proposal is refused ([`FitVerdict`]) when Gauss–Newton does not reach a stationary point
//! (singular normal equations, a non-finite step, or the iteration limit), when the posterior
//! covariance is not finite and positive (σ is then `NaN`, never 0), or when the data cannot
//! identify the parameters:
//! with each Jacobian column scaled by a *plausible hardware change*
//! ([`FitOptions::ident_mass_scale_step`], [`FitOptions::ident_com_offset_step_m`]), the
//! smallest singular value must reach [`FitOptions::min_identifiable_nm`] (every such change,
//! in any combination, visibly moves the stacked torques) and the condition number must stay
//! below
//! [`FitOptions::max_condition`]), when a fitted value is physically implausible, or when the
//! worst residual after the fit exceeds [`FitOptions::max_residual_nm`].

use std::fmt;
use std::str::FromStr;

use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use crate::{DynamicsError, DynamicsModel, LinkInertial, UrdfGravityModel};

/// Which inertial quantity of a link is fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InertialParamKind {
    /// `m = s · m_cad`.
    MassScale,
    /// `c = c_cad + d · u` along the principal axis `u`.
    ComOffset,
}

/// One fitted parameter, written `mass:<link>` or `com:<link>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InertialParam {
    pub link: String,
    pub kind: InertialParamKind,
}

impl InertialParam {
    pub fn mass(link: &str) -> Self {
        Self {
            link: link.to_string(),
            kind: InertialParamKind::MassScale,
        }
    }

    pub fn com(link: &str) -> Self {
        Self {
            link: link.to_string(),
            kind: InertialParamKind::ComOffset,
        }
    }
}

impl fmt::Display for InertialParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            InertialParamKind::MassScale => "mass",
            InertialParamKind::ComOffset => "com",
        };
        write!(f, "{kind}:{}", self.link)
    }
}

impl FromStr for InertialParam {
    type Err = CalibrationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || CalibrationError::BadParamSpec {
            spec: s.to_string(),
        };
        let (kind, link) = s.split_once(':').ok_or_else(bad)?;
        if link.is_empty() || !link.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(bad());
        }
        match kind {
            "mass" => Ok(Self::mass(link)),
            "com" => Ok(Self::com(link)),
            _ => Err(bad()),
        }
    }
}

/// Steady-state measurement at one pose: joint positions and friction-cancelled holding
/// torque (joint space, model joint order).
#[derive(Debug, Clone, PartialEq)]
pub struct GravitySample {
    pub label: String,
    pub q: Vec<f64>,
    pub tau_meas: Vec<f64>,
}

/// Allowed position window for one joint (soft ∩ hard limits).
#[derive(Debug, Clone, PartialEq)]
pub struct JointWindow {
    pub joint: String,
    pub lower_rad: f64,
    pub upper_rad: f64,
}

/// Fit tuning. Defaults are the documented commissioning values.
#[derive(Debug, Clone, PartialEq)]
pub struct FitOptions {
    /// Measurement noise per joint torque after friction cancellation (Nm); weights the
    /// data against the CAD prior.
    pub torque_sigma_nm: f64,
    /// Prior σ of a mass scale around CAD (1.0): regularisation toward CAD.
    pub prior_mass_scale_sigma: f64,
    /// Prior σ of a COM offset around CAD (m): regularisation toward CAD.
    pub prior_com_offset_sigma_m: f64,
    /// Plausible hardware change of a mass scale, used to scale identifiability.
    pub ident_mass_scale_step: f64,
    /// Plausible hardware change of a COM offset (m), used to scale identifiability.
    pub ident_com_offset_step_m: f64,
    /// Smallest singular value of the step-scaled data Jacobian (Nm): any combination of
    /// plausible changes must move the stacked torques by at least this.
    pub min_identifiable_nm: f64,
    /// Largest allowed condition number of that Jacobian.
    pub max_condition: f64,
    /// Refuse when any |τ_meas − τ_fit| exceeds this (Nm).
    pub max_residual_nm: f64,
    /// Plausible mass-scale range.
    pub mass_scale_range: (f64, f64),
    /// Plausible |COM offset| (m).
    pub max_com_offset_m: f64,
    /// Slack on measured poses outside the joint windows (rad), e.g. hanging jitter at 0.
    pub pose_limit_slack_rad: f64,
}

impl Default for FitOptions {
    fn default() -> Self {
        Self {
            torque_sigma_nm: 0.02,
            prior_mass_scale_sigma: 0.5,
            prior_com_offset_sigma_m: 0.05,
            ident_mass_scale_step: 0.25,
            ident_com_offset_step_m: 0.02,
            min_identifiable_nm: 0.05,
            max_condition: 100.0,
            max_residual_nm: 0.10,
            mass_scale_range: (0.5, 2.0),
            max_com_offset_m: 0.05,
            pose_limit_slack_rad: 0.03,
        }
    }
}

/// Input errors: the fit is not attempted.
#[derive(Debug, Error)]
pub enum CalibrationError {
    #[error(transparent)]
    Dynamics(#[from] DynamicsError),
    #[error("bad parameter spec {spec:?} (expected mass:<link> or com:<link>)")]
    BadParamSpec { spec: String },
    #[error("no parameters to fit")]
    NoParams,
    #[error("parameter {param} listed twice")]
    DuplicateParam { param: String },
    #[error("link {link} has no mass in the URDF; nothing to scale")]
    MasslessLink { link: String },
    #[error("link {link} COM sits on its frame origin; no principal axis for a COM offset")]
    NoPrincipalAxis { link: String },
    #[error("need at least {needed} samples, got {got}")]
    TooFewSamples { needed: usize, got: usize },
    #[error("sample {label}: expected {expected} joint values, got {got}")]
    SampleDims {
        label: String,
        expected: usize,
        got: usize,
    },
    #[error("sample {label}: non-finite value")]
    NonFinite { label: String },
    #[error("no joint window for {joint}")]
    MissingWindow { joint: String },
    #[error("joint {joint} has invalid calibration window [{lower}, {upper}]")]
    InvalidWindow {
        joint: String,
        lower: f64,
        upper: f64,
    },
    #[error(
        "sample {label}: {joint} q={q:.4} rad outside allowed window [{lower:.4}, {upper:.4}]"
    )]
    PoseOutOfLimits {
        label: String,
        joint: String,
        q: f64,
        lower: f64,
        upper: f64,
    },
    #[error("unknown fit joint {joint}")]
    UnknownFitJoint { joint: String },
}

/// Why no change is proposed (or that one is).
#[derive(Debug, Clone, PartialEq)]
pub enum FitVerdict {
    Accepted,
    /// Gauss–Newton did not reach a stationary point (singular normal equations, non-finite
    /// step, or iteration limit): the reported θ is not a fit.
    NotConverged {
        reason: String,
    },
    IllConditioned {
        reason: String,
    },
    ImplausibleParameter {
        param: String,
        value: f64,
    },
    ResidualTooHigh {
        max_abs_nm: f64,
        limit_nm: f64,
    },
}

impl fmt::Display for FitVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted => write!(f, "accepted"),
            Self::NotConverged { reason } => write!(f, "refused: not converged ({reason})"),
            Self::IllConditioned { reason } => write!(f, "refused: ill-conditioned ({reason})"),
            Self::ImplausibleParameter { param, value } => {
                write!(f, "refused: implausible {param} = {value:.4}")
            }
            Self::ResidualTooHigh {
                max_abs_nm,
                limit_nm,
            } => write!(
                f,
                "refused: max residual after fit {max_abs_nm:.4} Nm > {limit_nm:.2} Nm"
            ),
        }
    }
}

/// One fitted parameter with its CAD value and posterior σ.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamEstimate {
    pub param: InertialParam,
    /// 1.0 for a mass scale, 0.0 for a COM offset.
    pub cad: f64,
    pub fitted: f64,
    pub sigma: f64,
}

/// Resulting inertial of a link touched by the fit.
#[derive(Debug, Clone, PartialEq)]
pub struct FittedLink {
    pub link: String,
    pub cad: LinkInertial,
    pub fitted: LinkInertial,
    pub principal_axis: [f64; 3],
}

/// Model vs measurement at one sample.
#[derive(Debug, Clone, PartialEq)]
pub struct PoseResidual {
    pub label: String,
    pub q: Vec<f64>,
    pub tau_meas: Vec<f64>,
    pub tau_cad: Vec<f64>,
    pub tau_fit: Vec<f64>,
}

/// Singular-value summary of the step-scaled data Jacobian at CAD.
#[derive(Debug, Clone, PartialEq)]
pub struct Identifiability {
    pub min_singular_nm: f64,
    pub max_singular_nm: f64,
    pub condition: f64,
    /// Weakest parameter combination (unit vector in step units), largest weights first.
    pub weakest_direction: Vec<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GravityFit {
    pub joints: Vec<String>,
    pub fit_joints: Vec<String>,
    pub params: Vec<ParamEstimate>,
    pub links: Vec<FittedLink>,
    pub poses: Vec<PoseResidual>,
    pub identifiability: Identifiability,
    pub rms_before_nm: f64,
    pub rms_after_nm: f64,
    pub max_abs_before_nm: f64,
    pub max_abs_after_nm: f64,
    pub verdict: FitVerdict,
}

impl GravityFit {
    pub fn accepted(&self) -> bool {
        self.verdict == FitVerdict::Accepted
    }
}

/// Identifiability of `params` at the CAD values for measurement poses `poses`, using
/// only kinematics (no torques): the singular values of the data Jacobian with each column
/// scaled by a plausible hardware change, i.e. Nm of stacked-torque change per step.
pub fn assess_identifiability(
    model: &UrdfGravityModel,
    params: &[InertialParam],
    poses: &[Vec<f64>],
    fit_joints: &[String],
    opts: &FitOptions,
) -> Result<Identifiability, CalibrationError> {
    if params.is_empty() {
        return Err(CalibrationError::NoParams);
    }
    for (i, q) in poses.iter().enumerate() {
        if q.len() != model.joint_names().len() {
            return Err(CalibrationError::SampleDims {
                label: format!("pose {i}"),
                expected: model.joint_names().len(),
                got: q.len(),
            });
        }
    }
    let fit_idx = fit_joint_indices(model.joint_names(), fit_joints)?;
    let links = link_states(model, params, poses)?;
    Ok(identifiability(
        &links,
        &prior_mean(params),
        &ident_steps(params, opts),
        params,
        &fit_idx,
    ))
}

/// Default parameters for a sweep of `sweep_joint` measured at `poses`.
///
/// Candidates are the mass scales of every massive link downstream of the swept joint, then
/// their COM offsets, each group ordered by how strongly one plausible change moves the torques.
/// A candidate is kept only if the set stays identifiable
/// ([`FitOptions::min_identifiable_nm`], [`FitOptions::max_condition`]); the selection uses
/// kinematics only, never the measured torques. Errors with [`CalibrationError::NoParams`]
/// when the sweep identifies nothing.
pub fn default_params(
    model: &UrdfGravityModel,
    sweep_joint: &str,
    poses: &[Vec<f64>],
    fit_joints: &[String],
    opts: &FitOptions,
) -> Result<Vec<InertialParam>, CalibrationError> {
    let mut masses = Vec::new();
    let mut coms = Vec::new();
    for link in model.links_downstream_of(sweep_joint)? {
        let inertial = model.link_inertial(&link)?;
        if inertial.mass_kg <= MIN_FIT_MASS_KG {
            continue;
        }
        masses.push(InertialParam::mass(&link));
        if inertial.com_m.iter().map(|v| v * v).sum::<f64>().sqrt() > MIN_PRINCIPAL_AXIS_M {
            coms.push(InertialParam::com(&link));
        }
    }
    let mut chosen: Vec<InertialParam> = Vec::new();
    for group in [masses, coms] {
        let mut ranked = group
            .into_iter()
            .map(|p| {
                let single = assess_identifiability(
                    model,
                    std::slice::from_ref(&p),
                    poses,
                    fit_joints,
                    opts,
                )?;
                Ok((single.max_singular_nm, p))
            })
            .collect::<Result<Vec<_>, CalibrationError>>()?;
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.link.cmp(&b.1.link)));
        for (_, p) in ranked {
            chosen.push(p);
            let ident = assess_identifiability(model, &chosen, poses, fit_joints, opts)?;
            if unidentifiable_reason(&ident, opts).is_some() {
                chosen.pop();
            }
        }
    }
    if chosen.is_empty() {
        return Err(CalibrationError::NoParams);
    }
    Ok(chosen)
}

/// Links lighter than this are fixtures/placeholders, not fit candidates.
const MIN_FIT_MASS_KG: f64 = 0.01;
/// A COM closer than this to the link origin defines no principal axis.
const MIN_PRINCIPAL_AXIS_M: f64 = 1e-6;

const MAX_ITERATIONS: usize = 50;
const STEP_TOLERANCE: f64 = 1e-12;

/// Per-link data precomputed per pose: `P(c, q) = a + b · c`.
struct LinkTerms {
    /// Indexed `[pose][joint]`.
    a: Vec<Vec<f64>>,
    /// Indexed `[pose][axis][joint]`.
    b: Vec<[Vec<f64>; 3]>,
}

struct LinkState {
    link: String,
    cad: LinkInertial,
    axis: [f64; 3],
    mass_param: Option<usize>,
    com_param: Option<usize>,
    terms: LinkTerms,
}

impl LinkState {
    fn inertial(&self, theta: &DVector<f64>) -> LinkInertial {
        let s = self.mass_param.map_or(1.0, |i| theta[i]);
        let d = self.com_param.map_or(0.0, |i| theta[i]);
        LinkInertial {
            mass_kg: s * self.cad.mass_kg,
            com_m: [
                self.cad.com_m[0] + d * self.axis[0],
                self.cad.com_m[1] + d * self.axis[1],
                self.cad.com_m[2] + d * self.axis[2],
            ],
        }
    }

    /// `P(c, q_k)` for joint `j`.
    fn point_torque(&self, k: usize, j: usize, c: [f64; 3]) -> f64 {
        self.terms.a[k][j]
            + self.terms.b[k][0][j] * c[0]
            + self.terms.b[k][1][j] * c[1]
            + self.terms.b[k][2][j] * c[2]
    }

    /// `∂P/∂d = b · u` for joint `j` at pose `k`.
    fn along_axis(&self, k: usize, j: usize) -> f64 {
        (0..3)
            .map(|ax| self.terms.b[k][ax][j] * self.axis[ax])
            .sum()
    }
}

/// Fit `params` to `samples`; see the module docs for the model, prior and refusals.
///
/// `windows` must cover every model joint; samples outside them (plus
/// [`FitOptions::pose_limit_slack_rad`]) are refused. Empty `fit_joints` fits every joint.
pub fn fit_gravity_params(
    model: &UrdfGravityModel,
    params: &[InertialParam],
    samples: &[GravitySample],
    windows: &[JointWindow],
    fit_joints: &[String],
    opts: &FitOptions,
) -> Result<GravityFit, CalibrationError> {
    let joints = model.joint_names().to_vec();
    validate_samples(samples, &joints, windows, params.len(), opts)?;
    let fit_idx = fit_joint_indices(&joints, fit_joints)?;
    let poses_q: Vec<Vec<f64>> = samples.iter().map(|s| s.q.clone()).collect();
    let links = link_states(model, params, &poses_q)?;
    let theta0 = prior_mean(params);
    let sigma = prior_sigma(params, opts);

    let tau_cad = samples
        .iter()
        .map(|s| Ok(model.gravity_torques(&s.q)?.into_inner()))
        .collect::<Result<Vec<_>, DynamicsError>>()?;

    let identifiability = identifiability(
        &links,
        &theta0,
        &ident_steps(params, opts),
        params,
        &fit_idx,
    );

    // MAP Gauss–Newton.
    let lambda = sigma.map(|s| 1.0 / (s * s));
    let w = 1.0 / (opts.torque_sigma_nm * opts.torque_sigma_nm);
    let mut theta = theta0.clone();
    let mut convergence = Convergence::MaxIterations {
        last_step: f64::NAN,
    };
    for iteration in 0..MAX_ITERATIONS {
        let r = residual(&links, &theta, samples, &tau_cad, &fit_idx);
        let jac = jacobian(&links, &theta, &fit_idx, params.len());
        let normal = normal_matrix(&jac, w, &lambda);
        let rhs = jac.transpose() * &r * w - lambda.component_mul(&(&theta - &theta0));
        let Some(step) = normal.lu().solve(&rhs) else {
            convergence = Convergence::Singular { iteration };
            break;
        };
        if step.iter().any(|v| !v.is_finite()) {
            convergence = Convergence::NonFinite { iteration };
            break;
        }
        theta += &step;
        let step_norm = step.norm();
        if step_norm < STEP_TOLERANCE {
            convergence = Convergence::Converged;
            break;
        }
        convergence = Convergence::MaxIterations {
            last_step: step_norm,
        };
    }
    // Posterior covariance at the *final* θ (not the last pre-step iterate). A singular or
    // non-finite normal matrix yields NaN σ, never a confident 0.
    let posterior = normal_matrix(
        &jacobian(&links, &theta, &fit_idx, params.len()),
        w,
        &lambda,
    )
    .try_inverse();

    let estimates = params
        .iter()
        .enumerate()
        .map(|(i, p)| ParamEstimate {
            param: p.clone(),
            cad: theta0[i],
            fitted: theta[i],
            sigma: posterior_sigma(posterior.as_ref(), i),
        })
        .collect::<Vec<_>>();

    let poses = samples
        .iter()
        .enumerate()
        .map(|(k, s)| PoseResidual {
            label: s.label.clone(),
            q: s.q.clone(),
            tau_meas: s.tau_meas.clone(),
            tau_cad: tau_cad[k].clone(),
            tau_fit: (0..joints.len())
                .map(|j| model_torque(&links, &theta, &tau_cad, k, j))
                .collect(),
        })
        .collect::<Vec<_>>();

    let before: Vec<f64> = poses
        .iter()
        .flat_map(|p| fit_idx.iter().map(|&j| p.tau_meas[j] - p.tau_cad[j]))
        .collect();
    let after: Vec<f64> = poses
        .iter()
        .flat_map(|p| fit_idx.iter().map(|&j| p.tau_meas[j] - p.tau_fit[j]))
        .collect();

    let verdict = verdict(&convergence, &identifiability, &estimates, &after, opts);
    let fitted_links = links
        .iter()
        .map(|l| FittedLink {
            link: l.link.clone(),
            cad: l.cad,
            fitted: l.inertial(&theta),
            principal_axis: l.axis,
        })
        .collect();

    Ok(GravityFit {
        fit_joints: fit_idx.iter().map(|&j| joints[j].clone()).collect(),
        joints,
        params: estimates,
        links: fitted_links,
        poses,
        identifiability,
        rms_before_nm: rms(&before),
        rms_after_nm: rms(&after),
        max_abs_before_nm: max_abs(&before),
        max_abs_after_nm: max_abs(&after),
        verdict,
    })
}

fn prior_mean(params: &[InertialParam]) -> DVector<f64> {
    DVector::from_iterator(
        params.len(),
        params.iter().map(|p| match p.kind {
            InertialParamKind::MassScale => 1.0,
            InertialParamKind::ComOffset => 0.0,
        }),
    )
}

fn prior_sigma(params: &[InertialParam], opts: &FitOptions) -> DVector<f64> {
    DVector::from_iterator(
        params.len(),
        params.iter().map(|p| match p.kind {
            InertialParamKind::MassScale => opts.prior_mass_scale_sigma,
            InertialParamKind::ComOffset => opts.prior_com_offset_sigma_m,
        }),
    )
}

fn validate_samples(
    samples: &[GravitySample],
    joints: &[String],
    windows: &[JointWindow],
    n_params: usize,
    opts: &FitOptions,
) -> Result<(), CalibrationError> {
    if n_params == 0 {
        return Err(CalibrationError::NoParams);
    }
    let needed = 2;
    if samples.len() < needed {
        return Err(CalibrationError::TooFewSamples {
            needed,
            got: samples.len(),
        });
    }
    let windows = joints
        .iter()
        .map(|j| {
            windows
                .iter()
                .find(|w| &w.joint == j)
                .ok_or_else(|| CalibrationError::MissingWindow { joint: j.clone() })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for s in samples {
        for got in [s.q.len(), s.tau_meas.len()] {
            if got != joints.len() {
                return Err(CalibrationError::SampleDims {
                    label: s.label.clone(),
                    expected: joints.len(),
                    got,
                });
            }
        }
        if s.q.iter().chain(&s.tau_meas).any(|v| !v.is_finite()) {
            return Err(CalibrationError::NonFinite {
                label: s.label.clone(),
            });
        }
        for (w, &q) in windows.iter().zip(&s.q) {
            if !w.lower_rad.is_finite() || !w.upper_rad.is_finite() || w.lower_rad > w.upper_rad {
                return Err(CalibrationError::InvalidWindow {
                    joint: w.joint.clone(),
                    lower: w.lower_rad,
                    upper: w.upper_rad,
                });
            }
            if q < w.lower_rad - opts.pose_limit_slack_rad
                || q > w.upper_rad + opts.pose_limit_slack_rad
            {
                return Err(CalibrationError::PoseOutOfLimits {
                    label: s.label.clone(),
                    joint: w.joint.clone(),
                    q,
                    lower: w.lower_rad,
                    upper: w.upper_rad,
                });
            }
        }
    }
    Ok(())
}

fn fit_joint_indices(
    joints: &[String],
    fit_joints: &[String],
) -> Result<Vec<usize>, CalibrationError> {
    if fit_joints.is_empty() {
        return Ok((0..joints.len()).collect());
    }
    fit_joints
        .iter()
        .map(|f| {
            joints
                .iter()
                .position(|j| j == f)
                .ok_or_else(|| CalibrationError::UnknownFitJoint { joint: f.clone() })
        })
        .collect()
}

fn link_states(
    model: &UrdfGravityModel,
    params: &[InertialParam],
    poses: &[Vec<f64>],
) -> Result<Vec<LinkState>, CalibrationError> {
    let mut links: Vec<LinkState> = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let pos = match links.iter().position(|l| l.link == p.link) {
            Some(pos) => pos,
            None => {
                let cad = model.link_inertial(&p.link)?;
                if cad.mass_kg <= 0.0 {
                    return Err(CalibrationError::MasslessLink {
                        link: p.link.clone(),
                    });
                }
                let norm = cad.com_m.iter().map(|v| v * v).sum::<f64>().sqrt();
                let axis = if norm > MIN_PRINCIPAL_AXIS_M {
                    cad.com_m.map(|v| v / norm)
                } else {
                    [0.0; 3]
                };
                links.push(LinkState {
                    link: p.link.clone(),
                    cad,
                    axis,
                    mass_param: None,
                    com_param: None,
                    terms: link_terms(model, &p.link, poses)?,
                });
                links.len() - 1
            }
        };
        let state = &mut links[pos];
        let slot = match p.kind {
            InertialParamKind::MassScale => &mut state.mass_param,
            InertialParamKind::ComOffset => {
                if state.axis == [0.0; 3] {
                    return Err(CalibrationError::NoPrincipalAxis {
                        link: p.link.clone(),
                    });
                }
                &mut state.com_param
            }
        };
        if slot.replace(i).is_some() {
            return Err(CalibrationError::DuplicateParam {
                param: p.to_string(),
            });
        }
    }
    Ok(links)
}

fn link_terms(
    model: &UrdfGravityModel,
    link: &str,
    poses: &[Vec<f64>],
) -> Result<LinkTerms, CalibrationError> {
    let mut a = Vec::with_capacity(poses.len());
    let mut b = Vec::with_capacity(poses.len());
    for q in poses {
        let origin = model.point_mass_torques(link, [0.0; 3], q)?;
        let mut axes: [Vec<f64>; 3] = Default::default();
        for (axis, slot) in axes.iter_mut().enumerate() {
            let mut e = [0.0; 3];
            e[axis] = 1.0;
            let at = model.point_mass_torques(link, e, q)?;
            *slot = at.iter().zip(&origin).map(|(x, o)| x - o).collect();
        }
        a.push(origin);
        b.push(axes);
    }
    Ok(LinkTerms { a, b })
}

/// τ_j at pose k with parameters θ: CAD torque plus each fitted link's change.
fn model_torque(
    links: &[LinkState],
    theta: &DVector<f64>,
    tau_cad: &[Vec<f64>],
    k: usize,
    j: usize,
) -> f64 {
    links.iter().fold(tau_cad[k][j], |acc, l| {
        let fitted = l.inertial(theta);
        acc + fitted.mass_kg * l.point_torque(k, j, fitted.com_m)
            - l.cad.mass_kg * l.point_torque(k, j, l.cad.com_m)
    })
}

/// Stacked `τ_meas − τ(θ)` over samples × fit joints.
fn residual(
    links: &[LinkState],
    theta: &DVector<f64>,
    samples: &[GravitySample],
    tau_cad: &[Vec<f64>],
    fit_idx: &[usize],
) -> DVector<f64> {
    DVector::from_iterator(
        samples.len() * fit_idx.len(),
        samples.iter().enumerate().flat_map(|(k, s)| {
            fit_idx
                .iter()
                .map(move |&j| s.tau_meas[j] - model_torque(links, theta, tau_cad, k, j))
        }),
    )
}

/// `∂τ/∂θ` stacked like [`residual`].
fn jacobian(
    links: &[LinkState],
    theta: &DVector<f64>,
    fit_idx: &[usize],
    n_params: usize,
) -> DMatrix<f64> {
    let poses = links.first().map_or(0, |l| l.terms.a.len());
    let mut jac = DMatrix::zeros(poses * fit_idx.len(), n_params);
    for k in 0..poses {
        for (jj, &j) in fit_idx.iter().enumerate() {
            let row = k * fit_idx.len() + jj;
            for l in links {
                let fitted = l.inertial(theta);
                if let Some(i) = l.mass_param {
                    jac[(row, i)] = l.cad.mass_kg * l.point_torque(k, j, fitted.com_m);
                }
                if let Some(i) = l.com_param {
                    jac[(row, i)] = fitted.mass_kg * l.along_axis(k, j);
                }
            }
        }
    }
    jac
}

fn ident_steps(params: &[InertialParam], opts: &FitOptions) -> DVector<f64> {
    DVector::from_iterator(
        params.len(),
        params.iter().map(|p| match p.kind {
            InertialParamKind::MassScale => opts.ident_mass_scale_step,
            InertialParamKind::ComOffset => opts.ident_com_offset_step_m,
        }),
    )
}

fn identifiability(
    links: &[LinkState],
    theta0: &DVector<f64>,
    steps: &DVector<f64>,
    params: &[InertialParam],
    fit_idx: &[usize],
) -> Identifiability {
    let scaled = jacobian(links, theta0, fit_idx, params.len()) * DMatrix::from_diagonal(steps);
    let svd = scaled.svd(false, true);
    let sv = &svd.singular_values;
    // Fewer rows than parameters: the missing singular values are zero.
    let (min_sv, min_idx) = if sv.len() < params.len() {
        (0.0, None)
    } else {
        (sv.min(), Some(sv.imin()))
    };
    let max_sv = sv.max();
    let mut weakest = match (min_idx, svd.v_t.as_ref()) {
        (Some(i), Some(v_t)) => params
            .iter()
            .enumerate()
            .map(|(p, param)| (param.to_string(), v_t[(i, p)]))
            .collect::<Vec<_>>(),
        _ => params.iter().map(|p| (p.to_string(), f64::NAN)).collect(),
    };
    weakest.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
    Identifiability {
        min_singular_nm: min_sv,
        max_singular_nm: max_sv,
        condition: if min_sv > 0.0 {
            max_sv / min_sv
        } else {
            f64::INFINITY
        },
        weakest_direction: weakest,
    }
}

/// `Some(reason)` when the data cannot identify the parameters.
fn unidentifiable_reason(ident: &Identifiability, opts: &FitOptions) -> Option<String> {
    if ident.min_singular_nm.is_nan() || ident.min_singular_nm < opts.min_identifiable_nm {
        let combo = ident
            .weakest_direction
            .iter()
            .filter(|(_, w)| w.abs() >= 0.2)
            .map(|(p, w)| format!("{w:+.2}·{p}"))
            .collect::<Vec<_>>()
            .join(" ");
        return Some(format!(
            "one plausible change along [{combo}] moves the stacked torques by only {:.4} Nm < {:.3} Nm; \
             drop those parameters or add a sweep that loads them",
            ident.min_singular_nm, opts.min_identifiable_nm
        ));
    }
    if ident.condition.is_nan() || ident.condition > opts.max_condition {
        return Some(format!(
            "condition number {:.1} > {:.0}",
            ident.condition, opts.max_condition
        ));
    }
    None
}

/// How the Gauss–Newton iteration ended.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Convergence {
    Converged,
    /// The normal equations could not be solved (LU found no solution).
    Singular {
        iteration: usize,
    },
    /// A step contained NaN/inf.
    NonFinite {
        iteration: usize,
    },
    /// Still stepping after [`MAX_ITERATIONS`].
    MaxIterations {
        last_step: f64,
    },
}

impl Convergence {
    fn failure(&self) -> Option<String> {
        match *self {
            Self::Converged => None,
            Self::Singular { iteration } => Some(format!(
                "normal equations singular at iteration {iteration}; θ is not a solution"
            )),
            Self::NonFinite { iteration } => Some(format!(
                "non-finite Gauss–Newton step at iteration {iteration}"
            )),
            Self::MaxIterations { last_step } => Some(format!(
                "no convergence after {MAX_ITERATIONS} iterations (last step norm {last_step:.3e})"
            )),
        }
    }
}

/// `JᵀJ / σ_τ² + diag(λ)`: the MAP normal matrix (and, inverted, the posterior covariance).
fn normal_matrix(jac: &DMatrix<f64>, w: f64, lambda: &DVector<f64>) -> DMatrix<f64> {
    jac.transpose() * jac * w + DMatrix::from_diagonal(lambda)
}

/// Posterior σ of parameter `i`; NaN (never 0) when the covariance is missing or its
/// diagonal is not a finite positive variance.
fn posterior_sigma(posterior: Option<&DMatrix<f64>>, i: usize) -> f64 {
    match posterior.map(|p| p[(i, i)]) {
        Some(variance) if variance.is_finite() && variance > 0.0 => variance.sqrt(),
        _ => f64::NAN,
    }
}

fn verdict(
    convergence: &Convergence,
    ident: &Identifiability,
    estimates: &[ParamEstimate],
    after: &[f64],
    opts: &FitOptions,
) -> FitVerdict {
    if let Some(reason) = convergence.failure() {
        return FitVerdict::NotConverged { reason };
    }
    if let Some(e) = estimates.iter().find(|e| !e.sigma.is_finite()) {
        return FitVerdict::IllConditioned {
            reason: format!(
                "posterior covariance of {} is not finite and positive; the fit has no usable uncertainty",
                e.param
            ),
        };
    }
    if let Some(reason) = unidentifiable_reason(ident, opts) {
        return FitVerdict::IllConditioned { reason };
    }
    for e in estimates {
        let plausible = match e.param.kind {
            InertialParamKind::MassScale => {
                (opts.mass_scale_range.0..=opts.mass_scale_range.1).contains(&e.fitted)
            }
            InertialParamKind::ComOffset => e.fitted.abs() <= opts.max_com_offset_m,
        };
        if !plausible {
            return FitVerdict::ImplausibleParameter {
                param: e.param.to_string(),
                value: e.fitted,
            };
        }
    }
    let max_after = if after.iter().all(|v| v.is_finite()) {
        max_abs(after)
    } else {
        f64::INFINITY
    };
    if max_after > opts.max_residual_nm {
        return FitVerdict::ResidualTooHigh {
            max_abs_nm: max_after,
            limit_nm: opts.max_residual_nm,
        };
    }
    FitVerdict::Accepted
}

fn rms(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

/// Average holding torque of the two approaches to one pose: Coulomb/static friction acts
/// with opposite sign when the pose is reached from below and from above, so the mean
/// cancels it. Returns `(mean, half-difference)`; the half-difference estimates friction.
pub fn cancel_friction(
    from_below: &[f64],
    from_above: &[f64],
) -> Result<(Vec<f64>, Vec<f64>), CalibrationError> {
    if from_below.len() != from_above.len() {
        return Err(CalibrationError::SampleDims {
            label: "friction pair".into(),
            expected: from_below.len(),
            got: from_above.len(),
        });
    }
    Ok(from_below
        .iter()
        .zip(from_above)
        .map(|(b, a)| ((b + a) / 2.0, (b - a) / 2.0))
        .unzip())
}
