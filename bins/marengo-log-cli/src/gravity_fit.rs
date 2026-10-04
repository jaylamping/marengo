//! `gravity-fit`: turn calibration-suite session directories into a proposed URDF
//! inertial patch, a proposed `control.yaml` friction patch, and a dated record.
//!
//! Each directory (`var/gravity-calibration/<TS>/`) holds `plan.json`, the session's
//! `position-trace.csv`, the Pi's pre-session `pi-marengo.urdf` and `config/*.yaml`.
//!
//! 1. **Steady state per step**: a hold step is the run of trace ticks after its `hold-at`
//!    until any joint's operator target changes. Its window is the last `measure_sec`
//!    (≥ 1 s) of the trailing run of settled ticks (every |dq| ≤ [`SETTLED_DQ_RAD_S`],
//!    stepped joint within [`SETTLED_BAND_RAD`] of target). Averages: q, drive torque
//!    `tau_meas` (joint space; at rest it equals gravity plus static friction) and the
//!    commanded `tau_p + tau_ff_cmd` cross-check. The P term is not subtracted: the drive
//!    torque already contains it.
//! 2. **Friction**: every pose is measured approached from below and from above; the mean
//!    cancels Coulomb friction ([`cancel_friction`]), while ½·(below − above) estimates it.
//!    Wave steps contribute moving samples (|dq| > [`WAVE_DQ_DEADBAND_RAD_S`]): with the
//!    *fitted* gravity subtracted, τ against dq regresses to Coulomb `fc` and viscous `fv`.
//! 3. **Fit**: [`fit_gravity_params`] on the Pi URDF (ADR 0017), with poses limited to
//!    `control.yaml` soft ∩ `motors.yaml` hard windows. Default parameters cover every
//!    link excited by *any* fused session's sweep, not just one sweep's downstream links.
//! 4. **Partial sessions**: steps whose target never appears, or that never settle, are
//!    skipped and reported; only measured holds with a valid window enter the fit. The
//!    session is refused only when too few friction-cancelled poses remain.
//! 5. **Output**: `<out-dir>/<date>-gravity-<ts>.{json,md}` whenever a fit ran;
//!    `.urdf.patch` only when it is accepted, `.control.patch` only when the static and
//!    wave friction estimates agree (see [`FRICTION_AGREE_ABS_NM`]) and are non-negative.
//!    Patches touch only right-arm `<inertial>` lines and `friction.fc/fv` values.
//!    Nothing is applied: URDF sync stays an explicit `pi_sync_bench_urdf`, and the
//!    friction patch is applied by hand to `control.yaml`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use armee_dynamics::calibration::{
    assess_identifiability, cancel_friction, fit_gravity_params, CalibrationError, FitOptions,
    FittedLink, GravityFit, GravitySample, InertialParam, JointWindow,
};
use armee_dynamics::{gravity_model_from_urdf, DynamicsError, DynamicsModel, UrdfGravityModel};
use marengo_config::{
    commanded_position_window, load_control_config_from, load_motors_config_from,
    load_robot_config_from, ConfigError,
};
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;

/// Settled: every joint's |dq| at most this (rad/s). Must exceed one drive velocity count:
/// at rest Robstride feedback dithers between 0 and ±1 count (≈0.075–0.082 rad/s on the
/// 2026-10-04 right arm), so a sub-count bound never sees five joints settled at once.
pub const SETTLED_DQ_RAD_S: f64 = 0.1;
/// Settled: the stepped joint within this of its target (rad).
pub const SETTLED_BAND_RAD: f64 = 0.05;
/// Shortest averaging window (s).
pub const MIN_WINDOW_S: f64 = 1.0;
/// Wave samples need |dq| above this (rad/s): well clear of the settled dither above,
/// well below typical wave peaks (≈0.3–1.0 rad/s), so turn-around dwells never enter the
/// friction regression.
pub const WAVE_DQ_DEADBAND_RAD_S: f64 = 0.15;
/// Fewest moving wave samples for a per-joint wave friction estimate.
pub const MIN_WAVE_SAMPLES: usize = 20;
/// Static and wave Coulomb estimates must agree within `max(ABS, REL · max(|both|))` for a
/// friction patch to be proposed. 0.05 Nm is a quarter of the 0.20 Nm gravity-gate band;
/// 25 % relative keeps small-friction joints from failing on noise.
pub const FRICTION_AGREE_ABS_NM: f64 = 0.05;
/// See [`FRICTION_AGREE_ABS_NM`].
pub const FRICTION_AGREE_REL: f64 = 0.25;
/// Trace target match tolerance (trace prints 6 decimals).
const TARGET_TOL_RAD: f64 = 5e-5;
/// Patched-URDF τ must match the fit within this (rounding of printed mass/COM).
const PATCH_ROUNDTRIP_TOL_NM: f64 = 0.005;
/// Links lighter than this are fixtures, not fit candidates (mirrors the fitter's floor).
const MIN_FIT_MASS_KG: f64 = 0.01;
/// A COM closer than this to its link origin defines no principal axis (fitter's floor).
const MIN_PRINCIPAL_AXIS_M: f64 = 1e-6;
/// Limb whose links the patch may touch.
const PATCH_LIMB: &str = "right_arm";

const PLAN_FILE: &str = "plan.json";
const TRACE_FILE: &str = "position-trace.csv";
const PI_URDF_FILE: &str = "pi-marengo.urdf";
const PROPOSED_URDF_FILE: &str = "proposed-marengo.urdf";
const PATCH_PATH: &str = "assets/urdf/marengo.urdf";
const CONTROL_PATCH_PATH: &str = "config/control.yaml";

#[derive(Debug, Error)]
pub enum GravityFitError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("config: {0}")]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Dynamics(#[from] DynamicsError),
    #[error("{0}")]
    Calibration(#[from] CalibrationError),
    #[error("plan: {0}")]
    Plan(String),
    #[error("trace line {line}: {message}")]
    Trace { line: usize, message: String },
    #[error("{0}")]
    OutOfLimits(String),
    #[error("too few usable poses ({got} friction-cancelled, {skipped} steps skipped): {hint}")]
    TooFewPoses {
        got: usize,
        skipped: usize,
        hint: String,
    },
    #[error("urdf patch: {0}")]
    Patch(String),
}

impl GravityFitError {
    /// Inputs outside the joint windows, sessions with nothing identifiable left after
    /// skipping aborted steps, and fewer samples than parameters are refusals (exit 2),
    /// not tool errors.
    pub fn is_refusal(&self) -> bool {
        matches!(
            self,
            Self::OutOfLimits(_)
                | Self::TooFewPoses { .. }
                | Self::Calibration(
                    CalibrationError::PoseOutOfLimits { .. }
                        | CalibrationError::TooFewSamples { .. }
                )
        )
    }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> GravityFitError + '_ {
    move |source| GravityFitError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Arguments of `marengo-log-cli gravity-fit`.
pub struct GravityFitArgs {
    pub dirs: Vec<PathBuf>,
    pub fit: Vec<String>,
    pub fit_joints: Vec<String>,
    pub out_dir: PathBuf,
    pub repo_urdf: PathBuf,
}

/// Result of a run that produced a fit.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Proposed,
    Refused,
}

#[derive(Debug, Deserialize)]
struct Plan {
    version: u32,
    session_ts: String,
    #[serde(default)]
    profile: String,
    sweep_joint: String,
    #[serde(default)]
    fixed_rad: BTreeMap<String, f64>,
    #[serde(default)]
    poses_rad: Vec<f64>,
    #[serde(default = "default_approach_offset")]
    approach_offset_rad: f64,
    #[serde(default = "default_measure_sec")]
    measure_sec: f64,
    steps: Vec<PlanStep>,
    #[serde(default)]
    gravity_gate_report: String,
    /// False when marengo-pi aborted: steps after the abort never appear in the trace.
    /// Absent (v1 plans) means complete.
    #[serde(default = "default_session_complete")]
    session_complete: bool,
}

fn default_approach_offset() -> f64 {
    0.05
}

fn default_measure_sec() -> f64 {
    1.5
}

fn default_session_complete() -> bool {
    true
}

impl Plan {
    /// Fixed poses from `fixed_rad`, overlaid with `fixed` steps (steps win).
    fn effective_fixed(&self) -> BTreeMap<String, f64> {
        let mut fixed = self.fixed_rad.clone();
        for step in &self.steps {
            if let PlanStep::Fixed { joint, target_rad } = step {
                fixed.insert(joint.clone(), *target_rad);
            }
        }
        fixed
    }
}

/// One plan step. v1 steps carry no `kind` and mean `hold`; v2 steps discriminate.
#[derive(Debug, Clone)]
enum PlanStep {
    Hold {
        joint: String,
        target_rad: f64,
        measure: bool,
        pose_index: Option<usize>,
        approach: Option<Approach>,
    },
    Wave {
        joint: String,
        min_rad: f64,
        max_rad: f64,
        cycles: u32,
        half_period_s: f64,
    },
    Fixed {
        joint: String,
        target_rad: f64,
    },
}

impl PlanStep {
    fn kind(&self) -> &'static str {
        match self {
            Self::Hold { .. } => "hold",
            Self::Wave { .. } => "wave",
            Self::Fixed { .. } => "fixed",
        }
    }

    fn joint(&self) -> &str {
        match self {
            Self::Hold { joint, .. } | Self::Wave { joint, .. } | Self::Fixed { joint, .. } => {
                joint
            }
        }
    }

    fn target_rad(&self) -> Option<f64> {
        match self {
            Self::Hold { target_rad, .. } | Self::Fixed { target_rad, .. } => Some(*target_rad),
            Self::Wave { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawStep {
    kind: Option<String>,
    joint: Option<String>,
    target_rad: Option<f64>,
    measure: Option<bool>,
    pose_index: Option<usize>,
    approach: Option<Approach>,
    min_rad: Option<f64>,
    max_rad: Option<f64>,
    cycles: Option<u32>,
    half_period_s: Option<f64>,
}

impl<'de> Deserialize<'de> for PlanStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawStep::deserialize(deserializer)?;
        let kind = raw.kind.as_deref().unwrap_or("hold");
        let joint = raw
            .joint
            .ok_or_else(|| serde::de::Error::missing_field("joint"))?;
        match kind {
            "hold" => Ok(PlanStep::Hold {
                joint,
                target_rad: raw
                    .target_rad
                    .ok_or_else(|| serde::de::Error::missing_field("target_rad"))?,
                measure: raw.measure.unwrap_or(false),
                pose_index: raw.pose_index,
                approach: raw.approach,
            }),
            "wave" => Ok(PlanStep::Wave {
                joint,
                min_rad: raw
                    .min_rad
                    .ok_or_else(|| serde::de::Error::missing_field("min_rad"))?,
                max_rad: raw
                    .max_rad
                    .ok_or_else(|| serde::de::Error::missing_field("max_rad"))?,
                cycles: raw
                    .cycles
                    .ok_or_else(|| serde::de::Error::missing_field("cycles"))?,
                half_period_s: raw
                    .half_period_s
                    .ok_or_else(|| serde::de::Error::missing_field("half_period_s"))?,
            }),
            "fixed" => Ok(PlanStep::Fixed {
                joint,
                target_rad: raw
                    .target_rad
                    .ok_or_else(|| serde::de::Error::missing_field("target_rad"))?,
            }),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["hold", "wave", "fixed"],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Approach {
    Below,
    Above,
}

/// One trace row (joint) of one tick.
#[derive(Debug, Clone)]
struct Row {
    q: f64,
    dq: f64,
    target_raw: f64,
    tau_meas: f64,
    tau_cmd: f64,
}

#[derive(Debug, Clone)]
struct Tick {
    t_ms: u64,
    rows: BTreeMap<String, Row>,
}

/// Steady-state average of one measured step.
#[derive(Debug, Clone)]
struct Measurement {
    plan_index: usize,
    pose_index: usize,
    approach: Approach,
    q: Vec<f64>,
    tau: Vec<f64>,
    tau_cmd: Vec<f64>,
    samples: usize,
    window_s: f64,
}

/// Friction-cancelled pose with its diagnostics.
#[derive(Debug, Clone)]
struct PoseData {
    sample: GravitySample,
    friction: Vec<f64>,
    tau_cmd: Vec<f64>,
    below: Measurement,
    above: Measurement,
}

/// One moving wave sample: full joint positions, wave-joint velocity, measured torque.
#[derive(Debug, Clone)]
struct WaveSample {
    q: Vec<f64>,
    dq: f64,
    tau: f64,
}

/// Wave samples of one wave step.
#[derive(Debug, Clone)]
struct WaveData {
    plan_index: usize,
    joint: String,
    samples: Vec<WaveSample>,
}

/// A plan step the trace never completed, with the reason it was left out of the fit.
#[derive(Debug, Clone)]
struct SkippedStep {
    index: usize,
    kind: String,
    joint: String,
    target_rad: Option<f64>,
    reason: String,
}

struct Session {
    dir: PathBuf,
    plan: Plan,
    poses: Vec<PoseData>,
    waves: Vec<WaveData>,
    skipped: Vec<SkippedStep>,
}

/// Run `gravity-fit`; `Ok` carries whether a patch was proposed.
pub fn run(args: &GravityFitArgs) -> Result<Outcome, GravityFitError> {
    let first = args
        .dirs
        .first()
        .ok_or_else(|| GravityFitError::Plan("at least one --dir is required".into()))?;
    let config = first.join("config");
    let robot = load_robot_config_from(&config)?;
    let control = load_control_config_from(&config)?;
    let motors = load_motors_config_from(&config)?;
    let joints = robot.robot.joints.clone();
    let windows = joints
        .iter()
        .map(|j| {
            commanded_position_window(&control, &motors, j)
                .map(|(lower_rad, upper_rad)| JointWindow {
                    joint: j.clone(),
                    lower_rad,
                    upper_rad,
                })
                .ok_or_else(|| {
                    GravityFitError::OutOfLimits(format!(
                        "{j}: no soft (control.yaml) ∩ hard (motors.yaml) window"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sessions = Vec::with_capacity(args.dirs.len());

    let urdf_path = first.join(PI_URDF_FILE);
    let pi_urdf = std::fs::read_to_string(&urdf_path).map_err(io(&urdf_path))?;
    let local_urdf = std::fs::read_to_string(&args.repo_urdf).map_err(io(&args.repo_urdf))?;
    for dir in &args.dirs {
        if dir != first {
            let other = std::fs::read_to_string(dir.join(PI_URDF_FILE))
                .map_err(io(&dir.join(PI_URDF_FILE)))?;
            if other != pi_urdf {
                return Err(GravityFitError::Plan(format!(
                    "{} used a different Pi URDF than {}; fit sessions of one URDF only",
                    dir.display(),
                    first.display()
                )));
            }
            let other_config = dir.join("config");
            let other_robot = load_robot_config_from(&other_config)?;
            if other_robot.robot.joints != joints {
                return Err(GravityFitError::Plan(format!(
                    "{} has a different robot.yaml joint list",
                    dir.display()
                )));
            }
            let other_control = load_control_config_from(&other_config)?;
            let other_motors = load_motors_config_from(&other_config)?;
            let other_windows = joints
                .iter()
                .map(|joint| {
                    commanded_position_window(&other_control, &other_motors, joint)
                        .map(|(lower_rad, upper_rad)| JointWindow {
                            joint: joint.clone(),
                            lower_rad,
                            upper_rad,
                        })
                        .ok_or_else(|| {
                            GravityFitError::OutOfLimits(format!(
                                "{}: no soft (control.yaml) ∩ hard (motors.yaml) window in {}",
                                joint,
                                dir.display()
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if other_windows != windows {
                return Err(GravityFitError::Plan(format!(
                    "{} has different effective calibration windows",
                    dir.display()
                )));
            }
        }
        sessions.push(load_session(dir, &joints, &windows)?);
    }

    let model = gravity_model_from_urdf(&urdf_path, &joints)?;
    let samples: Vec<GravitySample> = sessions
        .iter()
        .flat_map(|s| s.poses.iter().map(|p| p.sample.clone()))
        .collect();
    let opts = FitOptions::default();
    let params = if args.fit.is_empty() {
        let sweep_poses: Vec<Vec<f64>> = samples.iter().map(|s| s.q.clone()).collect();
        default_params_union(&model, &sessions, &sweep_poses, &args.fit_joints, &opts)?
    } else {
        args.fit
            .iter()
            .map(|s| s.parse())
            .collect::<Result<Vec<InertialParam>, _>>()?
    };
    let limb_links = limb_links(&model, &robot.robot.limbs, &joints)?;
    if let Some(p) = params.iter().find(|p| !limb_links.contains(&p.link)) {
        return Err(GravityFitError::Plan(format!(
            "{p}: only {PATCH_LIMB} links may be calibrated ({})",
            limb_links.join(", ")
        )));
    }

    let fit = fit_gravity_params(&model, &params, &samples, &windows, &args.fit_joints, &opts)?;

    let stem = record_stem(&sessions)?;
    std::fs::create_dir_all(&args.out_dir).map_err(io(&args.out_dir))?;
    let patch_file = if fit.accepted() {
        let patched = patch_inertials(&pi_urdf, &fit.links)?;
        let proposed = args.out_dir.join(format!("{stem}.{PROPOSED_URDF_FILE}"));
        std::fs::write(&proposed, &patched).map_err(io(&proposed))?;
        verify_patched(&proposed, &joints, &fit)?;
        let diff = unified_diff(&pi_urdf, &patched, PATCH_PATH);
        let path = args.out_dir.join(format!("{stem}.urdf.patch"));
        std::fs::write(&path, diff).map_err(io(&path))?;
        Some(path)
    } else {
        None
    };

    let friction = fit_friction(&sessions, &fit, &joints, &model);
    let control_path = first.join("config/control.yaml");
    let captured_control = std::fs::read_to_string(&control_path).map_err(io(&control_path))?;
    let control_patch_file = if fit.accepted() {
        patch_control_friction(&captured_control, &friction).map(|patched| {
            let diff = unified_diff(&captured_control, &patched, CONTROL_PATCH_PATH);
            let path = args.out_dir.join(format!("{stem}.control.patch"));
            std::fs::write(&path, &diff).map_err(io(&path))?;
            Ok::<_, GravityFitError>(path)
        })
    } else {
        None
    }
    .transpose()?;

    let record = record_json(
        &sessions,
        &fit,
        &friction,
        &opts,
        local_urdf == pi_urdf,
        patch_file.as_deref(),
        control_patch_file.as_deref(),
    );
    let json_path = args.out_dir.join(format!("{stem}.json"));
    let pretty = serde_json::to_string_pretty(&record).map_err(|source| GravityFitError::Json {
        path: json_path.clone(),
        source,
    })?;
    std::fs::write(&json_path, format!("{pretty}\n")).map_err(io(&json_path))?;
    let md_path = args.out_dir.join(format!("{stem}.md"));
    let md = record_markdown(&Record {
        stem: &stem,
        sessions: &sessions,
        fit: &fit,
        friction: &friction,
        opts: &opts,
        local_matches_pi: local_urdf == pi_urdf,
        patch: patch_file.as_deref(),
        control_patch: control_patch_file.as_deref(),
    });
    std::fs::write(&md_path, md).map_err(io(&md_path))?;

    println!("{}", summary(&fit));
    println!("{}", friction_summary(&friction));
    println!("record: {}", json_path.display());
    println!("record: {}", md_path.display());
    match &patch_file {
        Some(p) => {
            println!("proposed patch: {}", p.display());
            if let Some(c) = &control_patch_file {
                println!("proposed friction patch: {}", c.display());
            }
            println!(
                "Nothing applied. Review, `git apply` it to the Pi URDF copy at {PATCH_PATH} \
                 (ADR 0017), then pi_sync_bench_urdf as a separate explicit step. \
                 Apply the friction patch by hand to {CONTROL_PATCH_PATH}."
            );
            Ok(Outcome::Proposed)
        }
        None => {
            println!("no patch proposed: {}", fit.verdict);
            Ok(Outcome::Refused)
        }
    }
}

fn load_session(
    dir: &Path,
    joints: &[String],
    windows: &[JointWindow],
) -> Result<Session, GravityFitError> {
    let plan_path = dir.join(PLAN_FILE);
    let text = std::fs::read_to_string(&plan_path).map_err(io(&plan_path))?;
    let plan: Plan = serde_json::from_str(&text).map_err(|source| GravityFitError::Json {
        path: plan_path.clone(),
        source,
    })?;
    validate_plan(&plan, joints, windows)?;
    let trace_path = dir.join(TRACE_FILE);
    let trace = std::fs::read_to_string(&trace_path).map_err(io(&trace_path))?;
    let ticks = parse_trace(&trace)?;
    let seg = segment_plan(&plan, &ticks);
    let (measurements, unsettled) = measure_steps(&plan, &ticks, joints, &seg.holds);
    let (poses, unpaired) = pair_approaches(&plan, measurements);
    let waves = collect_waves(&plan, &ticks, joints, &seg.waves);
    let mut skipped = seg.skipped;
    skipped.extend(unsettled);
    skipped.extend(unpaired);
    skipped.sort_by_key(|s| s.index);
    if poses.is_empty() {
        return Err(GravityFitError::TooFewPoses {
            got: 0,
            skipped: skipped.len(),
            hint: format!(
                "{}: no friction-cancelled pose survived; skipped: {}",
                dir.display(),
                skipped
                    .iter()
                    .map(|s| format!("{} {}", s.index, s.reason))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        });
    }
    Ok(Session {
        dir: dir.to_path_buf(),
        plan,
        poses,
        waves,
        skipped,
    })
}

fn validate_plan(
    plan: &Plan,
    joints: &[String],
    windows: &[JointWindow],
) -> Result<(), GravityFitError> {
    if plan.version != 1 && plan.version != 2 {
        return Err(GravityFitError::Plan(format!(
            "unsupported plan version {}",
            plan.version
        )));
    }
    if !joints.contains(&plan.sweep_joint) {
        return Err(GravityFitError::Plan(format!(
            "sweep joint {} not in robot.yaml",
            plan.sweep_joint
        )));
    }
    let window = |joint: &str| {
        windows
            .iter()
            .find(|w| w.joint == joint)
            .ok_or_else(|| GravityFitError::Plan(format!("step joint {joint} not in robot.yaml")))
    };
    let check = |joint: &str, rad: f64, what: &str| -> Result<(), GravityFitError> {
        let w = window(joint)?;
        if !rad.is_finite() || !(w.lower_rad..=w.upper_rad).contains(&rad) {
            return Err(GravityFitError::OutOfLimits(format!(
                "{what}: {joint} {rad} rad outside soft ∩ hard window [{:.4}, {:.4}]",
                w.lower_rad, w.upper_rad
            )));
        }
        Ok(())
    };
    for (joint, &rad) in &plan.effective_fixed() {
        check(joint, rad, "fixed pose")?;
    }
    for &rad in &plan.poses_rad {
        check(&plan.sweep_joint, rad, "pose")?;
    }
    for (i, s) in plan.steps.iter().enumerate() {
        match s {
            PlanStep::Hold {
                joint,
                target_rad,
                measure,
                pose_index,
                approach,
            } => {
                check(joint, *target_rad, &format!("step {i}"))?;
                if *measure {
                    let ok = pose_index.is_some_and(|p| p < plan.poses_rad.len())
                        && approach.is_some()
                        && joint == &plan.sweep_joint;
                    if !ok {
                        return Err(GravityFitError::Plan(format!(
                            "measured step {i} needs the sweep joint, a valid pose_index and an approach"
                        )));
                    }
                }
            }
            PlanStep::Wave {
                joint,
                min_rad,
                max_rad,
                cycles,
                half_period_s,
            } => {
                window(joint)?;
                if !min_rad.is_finite() || !max_rad.is_finite() || min_rad >= max_rad {
                    return Err(GravityFitError::Plan(format!(
                        "wave step {i} needs min_rad < max_rad (got {min_rad}..{max_rad})"
                    )));
                }
                check(joint, *min_rad, &format!("wave step {i} min"))?;
                check(joint, *max_rad, &format!("wave step {i} max"))?;
                if *cycles < 1 {
                    return Err(GravityFitError::Plan(format!(
                        "wave step {i} needs cycles ≥ 1"
                    )));
                }
                if !half_period_s.is_finite() || *half_period_s <= 0.0 {
                    return Err(GravityFitError::Plan(format!(
                        "wave step {i} needs a positive half_period_s"
                    )));
                }
            }
            PlanStep::Fixed { joint, target_rad } => {
                check(joint, *target_rad, &format!("fixed step {i}"))?;
            }
        }
    }
    Ok(())
}

/// Quote-aware CSV field split (berthier quotes fields containing commas, never quotes).
fn split_csv(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(line[start..i].trim_matches('"'));
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(line[start..].trim_matches('"'));
    out
}

fn parse_trace(text: &str) -> Result<Vec<Tick>, GravityFitError> {
    let mut lines = text.lines().enumerate();
    let header = lines
        .next()
        .map(|(_, h)| split_csv(h))
        .ok_or_else(|| GravityFitError::Trace {
            line: 1,
            message: "empty trace".into(),
        })?;
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| GravityFitError::Trace {
                line: 1,
                message: format!("missing column {name}"),
            })
    };
    let [tick_c, t_c, joint_c, q_c, dq_c, target_c, meas_c, p_c, ff_c] = [
        col("tick")?,
        col("t_ms")?,
        col("joint")?,
        col("q")?,
        col("dq")?,
        col("target_raw")?,
        col("tau_meas")?,
        col("tau_p")?,
        col("tau_ff_cmd")?,
    ];
    let mut ticks: Vec<(u64, Tick)> = Vec::new();
    let mut tick_indices: HashMap<u64, usize> = HashMap::new();
    let mut last_t_ms = None;
    for (i, line) in lines {
        if line.trim().is_empty() {
            continue;
        }
        let f = split_csv(line);
        let bad = |message: String| GravityFitError::Trace {
            line: i + 1,
            message,
        };
        let get = |c: usize| {
            f.get(c)
                .copied()
                .ok_or_else(|| bad(format!("missing column {c}")))
        };
        let num = |c: usize| -> Result<f64, GravityFitError> {
            let s = get(c)?;
            s.parse::<f64>().map_err(|e| bad(format!("{s:?}: {e}")))
        };
        let int = |c: usize| -> Result<u64, GravityFitError> {
            let s = get(c)?;
            s.parse::<u64>().map_err(|e| bad(format!("{s:?}: {e}")))
        };
        let tick = int(tick_c)?;
        let t_ms = int(t_c)?;
        if let Some(previous) = last_t_ms {
            if t_ms < previous {
                return Err(bad(format!(
                    "non-monotonic trace time: {t_ms} ms follows {previous} ms"
                )));
            }
        }
        last_t_ms = Some(t_ms);
        let row = Row {
            q: num(q_c)?,
            dq: num(dq_c)?,
            target_raw: num(target_c)?,
            tau_meas: num(meas_c)?,
            tau_cmd: num(p_c)? + num(ff_c)?,
        };
        let joint = get(joint_c)?.to_string();
        if let Some(&index) = tick_indices.get(&tick) {
            ticks[index].1.rows.insert(joint, row);
        } else {
            tick_indices.insert(tick, ticks.len());
            let mut rows = BTreeMap::new();
            rows.insert(joint, row);
            ticks.push((tick, Tick { t_ms, rows }));
        }
    }
    Ok(ticks.into_iter().map(|(_, tick)| tick).collect())
}

fn targets(tick: &Tick) -> BTreeMap<&str, f64> {
    tick.rows
        .iter()
        .map(|(j, r)| (j.as_str(), r.target_raw))
        .collect()
}

fn same_targets(a: &BTreeMap<&str, f64>, b: &BTreeMap<&str, f64>) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|((ja, ta), (jb, tb))| ja == jb && (ta - tb).abs() <= TARGET_TOL_RAD)
}

/// Found hold/fixed segments and wave ranges of a (possibly partial) session, plus the
/// plan steps the trace never completed.
#[derive(Debug)]
struct Segmentation {
    holds: Vec<HoldSeg>,
    waves: Vec<WaveSeg>,
    skipped: Vec<SkippedStep>,
}

/// Tick range `[start, end)` of one found hold/fixed step.
#[derive(Debug)]
struct HoldSeg {
    plan_index: usize,
    start: usize,
    end: usize,
}

/// Tick range `[start, end)` attributed to one wave step.
#[derive(Debug)]
struct WaveSeg {
    plan_index: usize,
    start: usize,
    end: usize,
}

fn segment_plan(plan: &Plan, ticks: &[Tick]) -> Segmentation {
    // Hold/fixed steps in plan order; a step whose target never appears after the
    // cursor is skipped (aborted session) without moving the cursor.
    let mut found: Vec<HoldSeg> = Vec::new();
    let mut skipped = Vec::new();
    let mut cursor = 0;
    for (k, step) in plan.steps.iter().enumerate() {
        let Some(target) = step.target_rad() else {
            continue;
        };
        let joint = step.joint().to_string();
        let at = ticks[cursor..].iter().position(|tick| {
            tick.rows
                .get(&joint)
                .is_some_and(|row| (row.target_raw - target).abs() <= TARGET_TOL_RAD)
        });
        match at {
            Some(off) => {
                found.push(HoldSeg {
                    plan_index: k,
                    start: cursor + off,
                    end: ticks.len(),
                });
                cursor += off + 1;
            }
            None => skipped.push(SkippedStep {
                index: k,
                kind: step.kind().to_string(),
                joint,
                target_rad: Some(target),
                reason: "hold-at target never appears in the trace (session aborted?)".into(),
            }),
        }
    }
    // Ends: first target change after the start, capped by the next found start.
    let starts: Vec<usize> = found.iter().map(|s| s.start).collect();
    for (i, seg) in found.iter_mut().enumerate() {
        let limit = starts.get(i + 1).copied().unwrap_or(ticks.len());
        let entry = targets(&ticks[seg.start]);
        seg.end = (seg.start..limit)
            .find(|&t| !same_targets(&targets(&ticks[t]), &entry))
            .unwrap_or(limit);
    }
    // Wave steps take the gap between their neighboring found holds (trace start/end
    // when the wave leads or trails); consecutive waves in one gap split it evenly.
    let wave_indices: Vec<usize> = plan
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s, PlanStep::Wave { .. }))
        .map(|(k, _)| k)
        .collect();
    let mut waves = Vec::new();
    let mut g = 0;
    while g < wave_indices.len() {
        let mut h = g + 1;
        let bounds = |k: usize| {
            let lo = found
                .iter()
                .filter(|s| s.plan_index < k)
                .map(|s| s.end)
                .max()
                .unwrap_or(0);
            let hi = found
                .iter()
                .filter(|s| s.plan_index > k)
                .map(|s| s.start)
                .min()
                .unwrap_or(ticks.len());
            (lo, hi)
        };
        while h < wave_indices.len() && bounds(wave_indices[h]) == bounds(wave_indices[g]) {
            h += 1;
        }
        let (lo, hi) = bounds(wave_indices[g]);
        if hi <= lo {
            for &k in &wave_indices[g..h] {
                if let PlanStep::Wave { joint, .. } = &plan.steps[k] {
                    skipped.push(SkippedStep {
                        index: k,
                        kind: "wave".into(),
                        joint: joint.clone(),
                        target_rad: None,
                        reason: "no trace ticks between the neighboring holds".into(),
                    });
                }
            }
        } else {
            let n = h - g;
            for (m, &k) in wave_indices[g..h].iter().enumerate() {
                let start = lo + (hi - lo) * m / n;
                let end = lo + (hi - lo) * (m + 1) / n;
                if end > start {
                    waves.push(WaveSeg {
                        plan_index: k,
                        start,
                        end,
                    });
                } else if let PlanStep::Wave { joint, .. } = &plan.steps[k] {
                    skipped.push(SkippedStep {
                        index: k,
                        kind: "wave".into(),
                        joint: joint.clone(),
                        target_rad: None,
                        reason: "empty trace range for this wave step".into(),
                    });
                }
            }
        }
        g = h;
    }
    Segmentation {
        holds: found,
        waves,
        skipped,
    }
}

fn measure_steps(
    plan: &Plan,
    ticks: &[Tick],
    joints: &[String],
    segs: &[HoldSeg],
) -> (Vec<Measurement>, Vec<SkippedStep>) {
    let measure_s = plan.measure_sec.max(MIN_WINDOW_S);
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for seg in segs {
        let (start, end) = (seg.start, seg.end);
        let PlanStep::Hold {
            joint,
            target_rad,
            measure: true,
            pose_index: Some(pose_index),
            approach: Some(approach),
        } = &plan.steps[seg.plan_index]
        else {
            continue;
        };
        let skip = |message: String| SkippedStep {
            index: seg.plan_index,
            kind: "hold".into(),
            joint: joint.clone(),
            target_rad: Some(*target_rad),
            reason: message,
        };
        let settled = |t: &Tick| {
            joints.iter().all(|j| {
                t.rows
                    .get(j)
                    .is_some_and(|r| r.dq.abs() <= SETTLED_DQ_RAD_S)
            }) && t
                .rows
                .get(joint.as_str())
                .is_some_and(|r| (r.q - *target_rad).abs() <= SETTLED_BAND_RAD)
        };
        let Some(run_start) = (start..end)
            .rev()
            .take_while(|&i| settled(&ticks[i]))
            .last()
        else {
            skipped.push(skip("never settled".into()));
            continue;
        };
        let last_ms = ticks[end - 1].t_ms;
        let Some(run_s) = last_ms
            .checked_sub(ticks[run_start].t_ms)
            .map(|ms| ms as f64 / 1000.0)
        else {
            skipped.push(skip("trace time regressed within settled window".into()));
            continue;
        };
        if run_s < MIN_WINDOW_S {
            skipped.push(skip(format!(
                "settled for only {run_s:.2} s (< {MIN_WINDOW_S} s): increase settle_sec"
            )));
            continue;
        }
        let from_ms = last_ms.saturating_sub((measure_s * 1000.0) as u64);
        let window: Vec<&Tick> = ticks[run_start..end]
            .iter()
            .filter(|t| t.t_ms >= from_ms)
            .collect();
        if window.is_empty() {
            skipped.push(skip("empty measurement window".into()));
            continue;
        }
        let mean = |f: &dyn Fn(&Row) -> f64| -> Vec<f64> {
            joints
                .iter()
                .map(|j| {
                    window
                        .iter()
                        .filter_map(|t| t.rows.get(j))
                        .map(f)
                        .sum::<f64>()
                        / window.len() as f64
                })
                .collect()
        };
        let window_start_ms = window.first().map_or(last_ms, |tick| tick.t_ms);
        let window_s = last_ms
            .checked_sub(window_start_ms)
            .map(|ms| ms as f64 / 1000.0)
            .unwrap_or(0.0);
        out.push(Measurement {
            plan_index: seg.plan_index,
            pose_index: *pose_index,
            approach: *approach,
            q: mean(&|r| r.q),
            tau: mean(&|r| r.tau_meas),
            tau_cmd: mean(&|r| r.tau_cmd),
            samples: window.len(),
            window_s,
        });
    }
    (out, skipped)
}

fn pair_approaches(
    plan: &Plan,
    measurements: Vec<Measurement>,
) -> (Vec<PoseData>, Vec<SkippedStep>) {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for (i, &pose) in plan.poses_rad.iter().enumerate() {
        let below = measurements
            .iter()
            .rfind(|m| m.pose_index == i && m.approach == Approach::Below)
            .cloned();
        let above = measurements
            .iter()
            .rfind(|m| m.pose_index == i && m.approach == Approach::Above)
            .cloned();
        match (below, above) {
            (Some(below), Some(above)) => {
                let (tau, friction) = match cancel_friction(&below.tau, &above.tau) {
                    Ok(v) => v,
                    Err(e) => {
                        skipped.push(SkippedStep {
                            index: below.plan_index,
                            kind: "hold".into(),
                            joint: plan.sweep_joint.clone(),
                            target_rad: Some(pose),
                            reason: format!("friction cancel failed: {e}"),
                        });
                        continue;
                    }
                };
                out.push(PoseData {
                    sample: GravitySample {
                        label: format!("{} {} rad [{}]", plan.sweep_joint, pose, plan.session_ts),
                        q: below
                            .q
                            .iter()
                            .zip(&above.q)
                            .map(|(b, a)| (b + a) / 2.0)
                            .collect(),
                        tau_meas: tau,
                    },
                    friction,
                    tau_cmd: below
                        .tau_cmd
                        .iter()
                        .zip(&above.tau_cmd)
                        .map(|(b, a)| (b + a) / 2.0)
                        .collect(),
                    below,
                    above,
                });
            }
            (present, _) => {
                let missing = if present
                    .as_ref()
                    .is_some_and(|m| m.approach == Approach::Below)
                {
                    Approach::Above
                } else {
                    Approach::Below
                };
                let (index, joint, target) =
                    present.map_or((0, plan.sweep_joint.clone(), Some(pose)), |m| {
                        let step_joint = match &plan.steps[m.plan_index] {
                            PlanStep::Hold { joint, .. } => joint.clone(),
                            other => other.joint().to_string(),
                        };
                        (
                            m.plan_index,
                            step_joint,
                            plan.steps[m.plan_index].target_rad(),
                        )
                    });
                skipped.push(SkippedStep {
                    index,
                    kind: "hold".into(),
                    joint,
                    target_rad: target,
                    reason: format!(
                        "pose {i} ({pose} rad): no {missing:?} measurement; friction cannot cancel"
                    ),
                });
            }
        }
    }
    (out, skipped)
}

/// Moving wave samples of each wave step: ticks with |dq| above the deadband on the
/// wave joint, carrying the full joint positions for the fitted-gravity subtraction.
fn collect_waves(
    plan: &Plan,
    ticks: &[Tick],
    joints: &[String],
    segs: &[WaveSeg],
) -> Vec<WaveData> {
    let mut out = Vec::new();
    for seg in segs {
        let PlanStep::Wave { joint, .. } = &plan.steps[seg.plan_index] else {
            continue;
        };
        let mut samples = Vec::new();
        for tick in &ticks[seg.start..seg.end] {
            let Some(row) = tick.rows.get(joint.as_str()) else {
                continue;
            };
            if row.dq.abs() <= WAVE_DQ_DEADBAND_RAD_S {
                continue;
            }
            let mut q = Vec::with_capacity(joints.len());
            let mut complete = true;
            for j in joints {
                match tick.rows.get(j) {
                    Some(r) => q.push(r.q),
                    None => {
                        complete = false;
                        break;
                    }
                }
            }
            if complete {
                samples.push(WaveSample {
                    q,
                    dq: row.dq,
                    tau: row.tau_meas,
                });
            }
        }
        out.push(WaveData {
            plan_index: seg.plan_index,
            joint: joint.clone(),
            samples,
        });
    }
    out
}

/// Per-joint friction result: the static-hold Coulomb estimate, the wave-regression
/// Coulomb/viscous estimates, and whether they agree enough to propose a patch.
#[derive(Debug, Clone)]
struct JointFriction {
    joint: String,
    static_fc_nm: Option<f64>,
    static_poses: usize,
    wave_fc_nm: Option<f64>,
    wave_fv: Option<f64>,
    wave_r2: Option<f64>,
    wave_samples: usize,
    consistent: bool,
    proposed_fc_nm: Option<f64>,
    proposed_fv: Option<f64>,
    note: String,
}

/// Least-squares fit of `r = fc·sign(dq) + fv·dq`; returns `(fc, fv, R²)`.
fn regress_friction(samples: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    let n = samples.len() as f64;
    if samples.len() < MIN_WAVE_SAMPLES {
        return None;
    }
    let (mut s2, mut sv, mut v2, mut sr, mut vr) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(dq, r) in samples {
        let s = dq.signum();
        s2 += s * s;
        sv += s * dq;
        v2 += dq * dq;
        sr += s * r;
        vr += dq * r;
    }
    let det = s2 * v2 - sv * sv;
    if det <= 0.0 || !det.is_finite() {
        return None;
    }
    let fc = (v2 * sr - sv * vr) / det;
    let fv = (s2 * vr - sv * sr) / det;
    if !fc.is_finite() || !fv.is_finite() {
        return None;
    }
    let mean = samples.iter().map(|&(_, r)| r).sum::<f64>() / n;
    let (mut ss_res, mut ss_tot) = (0.0, 0.0);
    for &(dq, r) in samples {
        let pred = fc * dq.signum() + fv * dq;
        ss_res += (r - pred) * (r - pred);
        ss_tot += (r - mean) * (r - mean);
    }
    let r2 = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else if ss_res == 0.0 {
        1.0
    } else {
        0.0
    };
    Some((fc, fv, r2))
}

/// Per-joint friction table. The static estimate is the mean ½·(below − above) holding
/// torque over matched poses; the wave estimate regresses (τ_meas − τ_g fitted) against
/// dq. A patch is proposed only when both exist, both Coulomb estimates and fv are
/// non-negative, and they agree within [`FRICTION_AGREE_ABS_NM`] / [`FRICTION_AGREE_REL`];
/// the proposed fc is their mean, fv comes from the wave. With no accepted gravity fit
/// there is no fitted gravity to regress against, so waves are reported but unused.
fn fit_friction(
    sessions: &[Session],
    fit: &GravityFit,
    joints: &[String],
    base_model: &UrdfGravityModel,
) -> Vec<JointFriction> {
    let fitted_model = if fit.accepted() {
        let mut model = base_model.clone();
        let mut ok = true;
        for link in &fit.links {
            match model.clone().with_link_inertial(&link.link, link.fitted) {
                Ok(m) => model = m,
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        ok.then_some(model)
    } else {
        None
    };
    joints
        .iter()
        .enumerate()
        .map(|(j, joint)| {
            let statics: Vec<f64> = sessions
                .iter()
                .flat_map(|s| &s.poses)
                .map(|p| p.friction[j])
                .collect();
            let static_fc = if statics.is_empty() {
                None
            } else {
                Some(statics.iter().sum::<f64>() / statics.len() as f64)
            };
            let mut pairs: Vec<(f64, f64)> = Vec::new();
            let mut wave_samples = 0;
            if let Some(model) = &fitted_model {
                for s in sessions {
                    for w in &s.waves {
                        if &w.joint != joint {
                            continue;
                        }
                        for sample in &w.samples {
                            let Ok(tau) = model.gravity_torques(&sample.q) else {
                                continue;
                            };
                            pairs.push((sample.dq, sample.tau - tau[j]));
                        }
                        wave_samples += w.samples.len();
                    }
                }
            }
            let (wave_fc, wave_fv, wave_r2) = match regress_friction(&pairs) {
                Some((fc, fv, r2)) => (Some(fc), Some(fv), Some(r2)),
                None => (None, None, None),
            };
            let (consistent, proposed, note) = match (static_fc, wave_fc, wave_fv) {
                (Some(fs), Some(fw), Some(fv)) if fs >= 0.0 && fw >= 0.0 && fv >= 0.0 => {
                    let tol =
                        FRICTION_AGREE_ABS_NM.max(FRICTION_AGREE_REL * fs.abs().max(fw.abs()));
                    if (fs - fw).abs() <= tol {
                        (
                            true,
                            Some(((fs + fw) / 2.0, fv)),
                            "static and wave agree".to_string(),
                        )
                    } else {
                        (
                            false,
                            None,
                            format!(
                                "static {fs:.4} vs wave {fw:.4} Nm differ by more than {tol:.4}"
                            ),
                        )
                    }
                }
                (Some(fs), Some(fw), Some(fv)) => (
                    false,
                    None,
                    format!(
                        "negative estimate (static {fs:.4}, wave {fw:.4}, fv {fv:.5}); no patch"
                    ),
                ),
                _ => {
                    let why = if fitted_model.is_none() {
                        "no accepted gravity fit to regress against"
                    } else if pairs.len() < MIN_WAVE_SAMPLES {
                        "too few moving wave samples"
                    } else {
                        "no matched static poses"
                    };
                    (false, None, format!("{why}; static reported only"))
                }
            };
            JointFriction {
                joint: joint.clone(),
                static_fc_nm: static_fc,
                static_poses: statics.len(),
                wave_fc_nm: wave_fc,
                wave_fv,
                wave_r2,
                wave_samples,
                consistent,
                proposed_fc_nm: proposed.map(|(fc, _)| fc),
                proposed_fv: proposed.map(|(_, fv)| fv),
                note,
            }
        })
        .collect()
}

/// Rewrite `fc:`/`fv:` lines inside each proposed joint's `friction:` block of the
/// captured `control.yaml` text. Returns the patched text, or `None` when no joint has
/// a proposal or its block is missing (fail closed: that joint is left out).
fn patch_control_friction(captured: &str, friction: &[JointFriction]) -> Option<String> {
    let want: BTreeMap<&str, (f64, f64)> = friction
        .iter()
        .filter_map(|f| match (f.proposed_fc_nm, f.proposed_fv) {
            (Some(fc), Some(fv)) => Some((f.joint.as_str(), (fc, fv))),
            _ => None,
        })
        .collect();
    if want.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut in_joints = false;
    let mut joint: Option<String> = None;
    let mut in_friction = false;
    let mut patched: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for line in captured.split_inclusive('\n') {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if indent == 2 && trimmed.starts_with("joints:") {
            in_joints = true;
            joint = None;
            in_friction = false;
        } else if indent == 2 && in_joints && trimmed.contains(':') {
            in_joints = false;
            joint = None;
            in_friction = false;
        } else if in_joints && indent == 4 && trimmed.ends_with(':') {
            joint = Some(trimmed.trim_end_matches(':').to_string());
            in_friction = false;
        } else if indent == 6 && trimmed.starts_with("friction:") {
            in_friction = joint.is_some();
        } else if in_friction && indent == 8 {
            if let (Some(j), Some(&(fc, fv))) = (
                joint.as_ref(),
                joint.as_deref().and_then(|name| want.get(name)),
            ) {
                if let Some(rest) = trimmed.strip_prefix("fc:") {
                    let comment = rest.find('#').map(|i| &rest[i..]).unwrap_or("");
                    let value = format!("{fc:.4}");
                    out.push(format!("        fc: {value}{comment}\n"));
                    patched.entry(j.clone()).or_insert((false, false)).0 = true;
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("fv:") {
                    let comment = rest.find('#').map(|i| &rest[i..]).unwrap_or("");
                    let value = format!("{fv:.5}");
                    out.push(format!("        fv: {value}{comment}\n"));
                    patched.entry(j.clone()).or_insert((false, false)).1 = true;
                    continue;
                }
            }
        } else if indent <= 4 {
            in_friction = false;
        }
        out.push(line.to_string());
    }
    let text: String = out.concat();
    if want
        .keys()
        .all(|j| patched.get(*j).is_some_and(|&(fc, fv)| fc && fv))
        && text != captured
    {
        Some(text)
    } else {
        None
    }
}
/// Default parameters covering every link excited by any fused session's sweep.
///
/// Candidates are the mass scales of every massive link downstream of *each* distinct
/// sweep joint (then their COM offsets), each group ordered by how strongly one plausible
/// change moves the torques over *all* poses. A candidate is kept only if the set stays
/// identifiable. Same greedy rule as the fitter's single-sweep default, but the union
/// keeps links (e.g. a forearm loaded only by an elbow sweep) that one sweep's
/// downstream set would drop.
fn default_params_union(
    model: &UrdfGravityModel,
    sessions: &[Session],
    poses: &[Vec<f64>],
    fit_joints: &[String],
    opts: &FitOptions,
) -> Result<Vec<InertialParam>, GravityFitError> {
    let mut sweeps: Vec<&str> = Vec::new();
    for s in sessions {
        if !sweeps.contains(&s.plan.sweep_joint.as_str()) {
            sweeps.push(&s.plan.sweep_joint);
        }
    }
    // Most-upstream sweep first, so shared links rank under the sweep that loads them
    // most; ties keep session order.
    sweeps.sort_by_key(|s| {
        std::cmp::Reverse(model.links_downstream_of(s).map(|l| l.len()).unwrap_or(0))
    });
    if sweeps.is_empty() {
        return Err(GravityFitError::Plan("no sessions".into()));
    }
    let mut masses = Vec::new();
    let mut coms = Vec::new();
    for sweep in &sweeps {
        for link in model.links_downstream_of(sweep)? {
            let inertial = model.link_inertial(&link)?;
            if inertial.mass_kg <= MIN_FIT_MASS_KG {
                continue;
            }
            let mass = InertialParam::mass(&link);
            if !masses.contains(&mass) {
                masses.push(mass);
            }
            let com_norm = inertial.com_m.iter().map(|v| v * v).sum::<f64>().sqrt();
            if com_norm > MIN_PRINCIPAL_AXIS_M {
                let com = InertialParam::com(&link);
                if !coms.contains(&com) {
                    coms.push(com);
                }
            }
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
            .collect::<Result<Vec<_>, GravityFitError>>()?;
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.link.cmp(&b.1.link)));
        for (_, p) in ranked {
            chosen.push(p);
            let ident = assess_identifiability(model, &chosen, poses, fit_joints, opts)?;
            if ident.min_singular_nm.is_nan()
                || ident.min_singular_nm < opts.min_identifiable_nm
                || ident.condition.is_nan()
                || ident.condition > opts.max_condition
            {
                chosen.pop();
            }
        }
    }
    if chosen.is_empty() {
        return Err(CalibrationError::NoParams.into());
    }
    Ok(chosen)
}

/// Links downstream of the first modelled joint of [`PATCH_LIMB`].
fn limb_links(
    model: &UrdfGravityModel,
    limbs: &BTreeMap<String, Vec<String>>,
    joints: &[String],
) -> Result<Vec<String>, GravityFitError> {
    let root = limbs
        .get(PATCH_LIMB)
        .and_then(|members| members.iter().find(|j| joints.contains(j)))
        .ok_or_else(|| {
            GravityFitError::Plan(format!("robot.yaml has no modelled {PATCH_LIMB} limb"))
        })?;
    Ok(model.links_downstream_of(root)?)
}

fn record_stem(sessions: &[Session]) -> Result<String, GravityFitError> {
    let ts: Vec<&str> = sessions
        .iter()
        .map(|s| s.plan.session_ts.as_str())
        .collect();
    let first = ts.first().copied().unwrap_or_default();
    let valid = first.len() >= 8 && first.as_bytes()[..8].iter().all(u8::is_ascii_digit);
    if !valid
        || ts
            .iter()
            .any(|t| !t.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return Err(GravityFitError::Plan(format!(
            "session_ts {first:?} is not a YYYYMMDDTHHMMSSZ stamp"
        )));
    }
    Ok(format!(
        "{}-{}-{}-gravity-{}",
        &first[..4],
        &first[4..6],
        &first[6..8],
        ts.join("-")
    ))
}

fn fmt_mass(v: f64) -> String {
    format!("{:.4}", (v * 1e4).round() / 1e4 + 0.0)
}

fn fmt_xyz(v: [f64; 3]) -> String {
    v.map(|x| format!("{:.5}", (x * 1e5).round() / 1e5 + 0.0))
        .join(" ")
}

/// Replace `attr="…"` on `line`.
fn set_attr(line: &str, attr: &str, value: &str) -> Option<String> {
    let key = format!("{attr}=\"");
    let start = line.find(&key)? + key.len();
    let len = line[start..].find('"')?;
    Some(format!(
        "{}{}{}",
        &line[..start],
        value,
        &line[start + len..]
    ))
}

fn link_name(line: &str) -> Option<&str> {
    let rest = &line[line.find("<link")? + "<link".len()..];
    let start = rest.find("name=\"")? + "name=\"".len();
    let len = rest[start..].find('"')?;
    Some(&rest[start..start + len])
}

/// Rewrite only `<mass value>` and the `<origin xyz>` inside `<inertial>` of `links`.
fn patch_inertials(urdf: &str, links: &[FittedLink]) -> Result<String, GravityFitError> {
    let mut out = String::with_capacity(urdf.len());
    let mut current: Option<&FittedLink> = None;
    let mut in_inertial = false;
    let mut done: Vec<(&str, bool, bool)> = Vec::new();
    for line in urdf.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let mut written = None;
        if trimmed.starts_with("<link") {
            current = link_name(trimmed).and_then(|n| links.iter().find(|l| l.link == n));
            if trimmed.trim_end().ends_with("/>") {
                current = None;
            }
        } else if trimmed.starts_with("</link") {
            current = None;
        } else if trimmed.starts_with("<inertial") {
            in_inertial = true;
        } else if trimmed.starts_with("</inertial") {
            in_inertial = false;
        } else if let (Some(link), true) = (current, in_inertial) {
            let entry = match done.iter().position(|d| d.0 == link.link) {
                Some(i) => i,
                None => {
                    done.push((link.link.as_str(), false, false));
                    done.len() - 1
                }
            };
            if trimmed.starts_with("<mass") {
                written = set_attr(line, "value", &fmt_mass(link.fitted.mass_kg));
                done[entry].1 = written.is_some();
            } else if trimmed.starts_with("<origin") {
                written = set_attr(line, "xyz", &fmt_xyz(link.fitted.com_m));
                done[entry].2 = written.is_some();
            }
        }
        out.push_str(written.as_deref().unwrap_or(line));
    }
    for l in links {
        let com_moved = fmt_xyz(l.fitted.com_m) != fmt_xyz(l.cad.com_m);
        match done.iter().find(|d| d.0 == l.link) {
            Some(&(_, true, origin)) if origin || !com_moved => {}
            _ => {
                return Err(GravityFitError::Patch(format!(
                    "link {}: no <inertial> <mass value> {}line to rewrite",
                    l.link,
                    if com_moved { "/ <origin xyz> " } else { "" }
                )))
            }
        }
    }
    Ok(out)
}

/// Reload the patched URDF and confirm it reproduces the fitted torques.
fn verify_patched(path: &Path, joints: &[String], fit: &GravityFit) -> Result<(), GravityFitError> {
    let patched = gravity_model_from_urdf(path, joints)?;
    for pose in &fit.poses {
        let tau = patched.gravity_torques(&pose.q)?;
        for (j, (got, want)) in tau.iter().zip(&pose.tau_fit).enumerate() {
            if (got - want).abs() > PATCH_ROUNDTRIP_TOL_NM {
                return Err(GravityFitError::Patch(format!(
                    "{}: patched URDF gives {got:.4} Nm on {}, fit {want:.4} Nm",
                    pose.label, joints[j]
                )));
            }
        }
    }
    Ok(())
}

/// Unified diff of two texts with equal line counts (inertial values rewritten in place).
fn unified_diff(old: &str, new: &str, path: &str) -> String {
    const CONTEXT: usize = 3;
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let changed: Vec<usize> = (0..a.len().min(b.len()))
        .filter(|&i| a[i] != b[i])
        .collect();
    let mut out = format!("--- a/{path}\n+++ b/{path}\n");
    let mut i = 0;
    while i < changed.len() {
        let start = changed[i].saturating_sub(CONTEXT);
        let mut last = changed[i];
        while i + 1 < changed.len() && changed[i + 1] <= last + 2 * CONTEXT + 1 {
            i += 1;
            last = changed[i];
        }
        let end = (last + CONTEXT + 1).min(a.len());
        let len = end - start;
        let _ = writeln!(out, "@@ -{},{len} +{},{len} @@", start + 1, start + 1);
        for k in start..end {
            let line = |s: &str| {
                if s.ends_with('\n') {
                    s.to_string()
                } else {
                    format!("{s}\n\\ No newline at end of file\n")
                }
            };
            if a[k] == b[k] {
                out.push(' ');
                out.push_str(&line(a[k]));
            } else {
                out.push('-');
                out.push_str(&line(a[k]));
                out.push('+');
                out.push_str(&line(b[k]));
            }
        }
        i += 1;
    }
    out
}

fn round(v: f64, digits: i32) -> f64 {
    let s = 10f64.powi(digits);
    (v * s).round() / s + 0.0
}

fn rounded(v: &[f64]) -> Vec<f64> {
    v.iter().map(|x| round(*x, 5)).collect()
}

/// `path` relative to the working directory (repo root under the MCP) when inside it, so
/// committed records carry no workstation-specific prefix.
fn display_relative(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

fn record_json(
    sessions: &[Session],
    fit: &GravityFit,
    friction: &[JointFriction],
    opts: &FitOptions,
    local_matches_pi: bool,
    patch: Option<&Path>,
    control_patch: Option<&Path>,
) -> Value {
    let pose_data: Vec<&PoseData> = sessions.iter().flat_map(|s| &s.poses).collect();
    json!({
        "version": 2,
        "kind": "gravity_calibration",
        "verdict": fit.verdict.to_string(),
        "accepted": fit.accepted(),
        "sessions": sessions.iter().map(|s| json!({
            "dir": display_relative(&s.dir),
            "session_ts": s.plan.session_ts,
            "plan_version": s.plan.version,
            "session_complete": s.plan.session_complete,
            "profile": s.plan.profile,
            "sweep_joint": s.plan.sweep_joint,
            "fixed_rad": s.plan.effective_fixed(),
            "poses_rad": s.plan.poses_rad,
            "approach_offset_rad": s.plan.approach_offset_rad,
            "measure_sec": s.plan.measure_sec,
            "wave_steps": s.waves.iter().map(|w| json!({
                "plan_index": w.plan_index,
                "joint": w.joint,
                "moving_samples": w.samples.len(),
            })).collect::<Vec<_>>(),
            "skipped_steps": s.skipped.iter().map(|k| json!({
                "step": k.index,
                "kind": k.kind,
                "joint": k.joint,
                "target_rad": k.target_rad,
                "reason": k.reason,
            })).collect::<Vec<_>>(),
            "gravity_gate_report": s.plan.gravity_gate_report,
        })).collect::<Vec<_>>(),
        "urdf_base": "Pi live URDF captured before the session (pi-marengo.urdf), ADR 0017",
        "local_urdf_matches_pi": local_matches_pi,
        "joints": fit.joints,
        "fit_joints": fit.fit_joints,
        "options": {
            "torque_sigma_nm": opts.torque_sigma_nm,
            "prior_mass_scale_sigma": opts.prior_mass_scale_sigma,
            "prior_com_offset_sigma_m": opts.prior_com_offset_sigma_m,
            "ident_mass_scale_step": opts.ident_mass_scale_step,
            "ident_com_offset_step_m": opts.ident_com_offset_step_m,
            "min_identifiable_nm": opts.min_identifiable_nm,
            "max_condition": opts.max_condition,
            "max_residual_nm": opts.max_residual_nm,
            "settled_dq_rad_s": SETTLED_DQ_RAD_S,
            "settled_band_rad": SETTLED_BAND_RAD,
        },
        "identifiability": {
            "min_singular_nm": round(fit.identifiability.min_singular_nm, 6),
            "max_singular_nm": round(fit.identifiability.max_singular_nm, 6),
            "condition": if fit.identifiability.condition.is_finite() {
                json!(round(fit.identifiability.condition, 3))
            } else {
                Value::Null
            },
            "weakest_direction": fit.identifiability.weakest_direction.iter()
                .map(|(p, w)| json!({"param": p, "weight": round(*w, 4)}))
                .collect::<Vec<_>>(),
        },
        "params": fit.params.iter().map(|p| json!({
            "param": p.param.to_string(),
            "cad": p.cad,
            "fitted": round(p.fitted, 5),
            "sigma": round(p.sigma, 5),
        })).collect::<Vec<_>>(),
        "links": fit.links.iter().map(|l| json!({
            "link": l.link,
            "mass_kg": {"cad": l.cad.mass_kg, "fitted": round(l.fitted.mass_kg, 4)},
            "com_m": {"cad": l.cad.com_m, "fitted": rounded(&l.fitted.com_m)},
            "principal_axis": rounded(&l.principal_axis),
        })).collect::<Vec<_>>(),
        "residual_nm": {
            "rms_before": round(fit.rms_before_nm, 5),
            "rms_after": round(fit.rms_after_nm, 5),
            "max_abs_before": round(fit.max_abs_before_nm, 5),
            "max_abs_after": round(fit.max_abs_after_nm, 5),
        },
        "poses": fit.poses.iter().zip(&pose_data).map(|(p, d)| json!({
            "label": p.label,
            "q": rounded(&p.q),
            "tau_meas": rounded(&p.tau_meas),
            "friction_half_diff": rounded(&d.friction),
            "tau_cmd_cross_check": rounded(&d.tau_cmd),
            "tau_cad": rounded(&p.tau_cad),
            "tau_fit": rounded(&p.tau_fit),
            "approaches": [approach_json(&d.below), approach_json(&d.above)],
        })).collect::<Vec<_>>(),
        "friction": {
            "method": "static ½·(below − above) per matched pose (mean per joint); wave OLS of (τ_meas − τ_g fitted) = fc·sign(dq) + fv·dq over |dq| > deadband",
            "deadband_rad_s": WAVE_DQ_DEADBAND_RAD_S,
            "min_wave_samples": MIN_WAVE_SAMPLES,
            "agreement_tol_nm": {"abs": FRICTION_AGREE_ABS_NM, "rel": FRICTION_AGREE_REL},
            "joints": friction.iter().map(|f| (f.joint.clone(), json!({
                "static_fc_nm": f.static_fc_nm.map(|v| round(v, 5)),
                "static_poses": f.static_poses,
                "wave_fc_nm": f.wave_fc_nm.map(|v| round(v, 5)),
                "wave_fv": f.wave_fv.map(|v| round(v, 5)),
                "wave_r2": f.wave_r2.map(|v| round(v, 4)),
                "wave_samples": f.wave_samples,
                "consistent": f.consistent,
                "proposed_fc_nm": f.proposed_fc_nm.map(|v| round(v, 5)),
                "proposed_fv": f.proposed_fv.map(|v| round(v, 5)),
                "note": f.note,
            }))).collect::<serde_json::Map<String, Value>>(),
        },
        "patch": patch.map(display_relative),
        "control_patch": control_patch.map(display_relative),
        "apply": "Never automatic. Review the patch, apply it to the Pi URDF copy at assets/urdf/marengo.urdf, commit, then pi_sync_bench_urdf (ADR 0017). Apply the friction patch by hand to config/control.yaml. Gram-level weighing of the frozen arm supersedes this record.",
    })
}

fn approach_json(m: &Measurement) -> Value {
    json!({
        "approach": match m.approach {
            Approach::Below => "below",
            Approach::Above => "above",
        },
        "samples": m.samples,
        "window_s": round(m.window_s, 3),
        "q": rounded(&m.q),
        "tau_meas": rounded(&m.tau),
    })
}

fn summary(fit: &GravityFit) -> String {
    let mut s = format!(
        "gravity-fit: {} | residual max {:.4} → {:.4} Nm, rms {:.4} → {:.4} Nm | \
         identifiability min σ {:.4} Nm, cond {:.1}",
        fit.verdict,
        fit.max_abs_before_nm,
        fit.max_abs_after_nm,
        fit.rms_before_nm,
        fit.rms_after_nm,
        fit.identifiability.min_singular_nm,
        fit.identifiability.condition
    );
    for p in &fit.params {
        let _ = write!(
            s,
            "\n  {}: {:.4} → {:.4} (±{:.4})",
            p.param, p.cad, p.fitted, p.sigma
        );
    }
    s
}

fn friction_summary(friction: &[JointFriction]) -> String {
    let mut s = String::from("friction per joint (static fc | wave fc, fv, R², n):");
    for f in friction {
        let _ = write!(
            s,
            "\n  {}: {} | {}, {}, R² {}, n={} | {}{}",
            f.joint,
            f.static_fc_nm
                .map_or("-".to_string(), |v| format!("{v:.4}")),
            f.wave_fc_nm.map_or("-".to_string(), |v| format!("{v:.4}")),
            f.wave_fv.map_or("-".to_string(), |v| format!("{v:.5}")),
            f.wave_r2.map_or("-".to_string(), |v| format!("{v:.3}")),
            f.wave_samples,
            if f.consistent { "agree" } else { "no patch" },
            match (f.proposed_fc_nm, f.proposed_fv) {
                (Some(fc), Some(fv)) => format!(" → propose fc={fc:.4} fv={fv:.5}"),
                _ => format!(" ({})", f.note),
            },
        );
    }
    s
}

/// Inputs to the markdown record (bundled: more than seven plain arguments).
struct Record<'a> {
    stem: &'a str,
    sessions: &'a [Session],
    fit: &'a GravityFit,
    friction: &'a [JointFriction],
    opts: &'a FitOptions,
    local_matches_pi: bool,
    patch: Option<&'a Path>,
    control_patch: Option<&'a Path>,
}

fn record_markdown(rec: &Record) -> String {
    let stem = rec.stem;
    let sessions = rec.sessions;
    let fit = rec.fit;
    let friction = rec.friction;
    let opts = rec.opts;
    let local_matches_pi = rec.local_matches_pi;
    let patch = rec.patch;
    let control_patch = rec.control_patch;
    let mut md = String::new();
    let _ = writeln!(md, "# Gravity calibration {stem}\n");
    let _ = writeln!(md, "**Verdict:** {}\n", fit.verdict);
    let _ = writeln!(
        md,
        "Generated by `marengo-log-cli gravity-fit` from calibration-suite sessions; see \
         [limb playbook](../limb-playbook.md#gravity-calibration-after-hardware-changes). \
         Gram-level weighing of the frozen arm supersedes this record.\n"
    );
    let _ = writeln!(md, "## Sessions\n");
    let _ = writeln!(
        md,
        "| session | plan | complete | profile | sweep | fixed | poses (rad) | δ approach | skipped |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");
    for s in sessions {
        let fixed = s
            .plan
            .effective_fixed()
            .iter()
            .map(|(j, v)| format!("{j}={v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            md,
            "| {} | v{} | {} | {} | {} | {} | {:?} | {} | {} |",
            s.plan.session_ts,
            s.plan.version,
            s.plan.session_complete,
            s.plan.profile,
            s.plan.sweep_joint,
            if fixed.is_empty() { "-".into() } else { fixed },
            s.plan.poses_rad,
            s.plan.approach_offset_rad,
            s.skipped.len(),
        );
    }
    let _ = writeln!(
        md,
        "\nURDF base: Pi live URDF captured before the session (ADR 0017). Local \
         `{PATCH_PATH}` matches it: {}.\n",
        if local_matches_pi {
            "yes"
        } else {
            "**no**: copy the Pi URDF into the repo before applying the patch"
        }
    );
    let _ = writeln!(md, "## Parameters\n");
    let _ = writeln!(md, "| parameter | CAD | fitted | ±σ |");
    let _ = writeln!(md, "|---|---|---|---|");
    for p in &fit.params {
        let _ = writeln!(
            md,
            "| `{}` | {:.4} | {:.4} | {:.4} |",
            p.param, p.cad, p.fitted, p.sigma
        );
    }
    let _ = writeln!(
        md,
        "\n| link | mass CAD → fitted (kg) | COM CAD → fitted (m) |"
    );
    let _ = writeln!(md, "|---|---|---|");
    for l in &fit.links {
        let _ = writeln!(
            md,
            "| `{}` | {} → {} | {} → {} |",
            l.link,
            fmt_mass(l.cad.mass_kg),
            fmt_mass(l.fitted.mass_kg),
            fmt_xyz(l.cad.com_m),
            fmt_xyz(l.fitted.com_m)
        );
    }
    let id = &fit.identifiability;
    let _ = writeln!(
        md,
        "\n## Identifiability\n\nmin singular value {:.4} Nm (≥ {:.3}), condition {:.1} (≤ {:.0}); \
         weakest direction: {}.\n",
        id.min_singular_nm,
        opts.min_identifiable_nm,
        id.condition,
        opts.max_condition,
        id.weakest_direction
            .iter()
            .map(|(p, w)| format!("{w:+.2}·`{p}`"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let _ = writeln!(
        md,
        "## Residuals (Nm)\n\nmax |τ_meas − τ| {:.4} → {:.4} (limit {:.2}); rms {:.4} → {:.4}. \
         Fit joints: {}.\n",
        fit.max_abs_before_nm,
        fit.max_abs_after_nm,
        opts.max_residual_nm,
        fit.rms_before_nm,
        fit.rms_after_nm,
        fit.fit_joints.join(", ")
    );
    let _ = writeln!(
        md,
        "| pose | joint | q | τ_meas | friction ½Δ | τ_p+τ_ff | τ_CAD | τ_fit | before | after |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|");
    let pose_data: Vec<&PoseData> = sessions.iter().flat_map(|s| &s.poses).collect();
    for (p, d) in fit.poses.iter().zip(&pose_data) {
        for (j, joint) in fit.joints.iter().enumerate() {
            let _ = writeln!(
                md,
                "| {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:+.4} | {:+.4} |",
                p.label,
                joint,
                p.q[j],
                p.tau_meas[j],
                d.friction[j],
                d.tau_cmd[j],
                p.tau_cad[j],
                p.tau_fit[j],
                p.tau_meas[j] - p.tau_cad[j],
                p.tau_meas[j] - p.tau_fit[j]
            );
        }
    }
    let _ = writeln!(
        md,
        "\n## Friction (Nm)\n\nStatic ½·(below − above) per matched pose (mean per joint); \
         wave OLS of (τ_meas − τ_g fitted) = fc·sign(dq) + fv·dq over |dq| > {WAVE_DQ_DEADBAND_RAD_S} \
         rad/s. Patch proposed only when both exist, fc/fv ≥ 0, and the Coulomb estimates \
         agree within max({FRICTION_AGREE_ABS_NM}, {FRICTION_AGREE_REL}·max(|both|)).\n"
    );
    let _ = writeln!(
        md,
        "| joint | static fc | poses | wave fc | wave fv | R² | samples | consistent | proposed | note |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|---|");
    for f in friction {
        let num = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.4}"));
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            f.joint,
            num(f.static_fc_nm),
            f.static_poses,
            num(f.wave_fc_nm),
            f.wave_fv.map_or("-".to_string(), |x| format!("{x:.5}")),
            f.wave_r2.map_or("-".to_string(), |x| format!("{x:.3}")),
            f.wave_samples,
            f.consistent,
            match (f.proposed_fc_nm, f.proposed_fv) {
                (Some(fc), Some(fv)) => format!("fc={fc:.4} fv={fv:.5}"),
                _ => "-".into(),
            },
            f.note,
        );
    }
    let skipped_all: Vec<(&Session, &SkippedStep)> = sessions
        .iter()
        .flat_map(|s| s.skipped.iter().map(move |k| (s, k)))
        .collect();
    if !skipped_all.is_empty() {
        let _ = writeln!(md, "\n## Skipped steps (partial sessions)\n");
        let _ = writeln!(
            md,
            "| session | step | kind | joint | target (rad) | reason |"
        );
        let _ = writeln!(md, "|---|---|---|---|---|---|");
        for (s, k) in &skipped_all {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} |",
                s.plan.session_ts,
                k.index,
                k.kind,
                k.joint,
                k.target_rad.map_or("-".to_string(), |t| format!("{t:.4}")),
                k.reason,
            );
        }
    }
    let _ = writeln!(md, "\n## Gravity gate at session start\n");
    for s in sessions {
        let _ = writeln!(
            md,
            "```text\n{}\n```\n",
            s.plan.gravity_gate_report.trim_end()
        );
    }
    let _ = writeln!(md, "## Apply (never automatic)\n");
    match patch {
        Some(p) => {
            let name = p
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let _ = writeln!(
                md,
                "1. Review `{name}`: it changes only `<inertial>` mass/origin of right-arm links.\n\
                 2. Make `{PATCH_PATH}` equal to the Pi URDF (ADR 0017), then `git apply \
                 docs/commissioning/calibrations/{name}` and commit.\n\
                 3. `pi_sync_bench_urdf`, then `pi_restart_marengo_pi` so Davout reloads the model.\n\
                 4. Re-run `pi_hold_on` (the gravity gate) to confirm the residual."
            );
            match control_patch {
                Some(c) => {
                    let cname = c
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    let _ = writeln!(
                        md,
                        "\n5. Review `{cname}`: it changes only `friction.fc/fv` of the joints \
                         whose static and wave estimates agree. Apply it by hand to \
                         `{CONTROL_PATCH_PATH}` on the Pi (Pi is the source of truth), then \
                         re-run the phase's holds as a no-refit check."
                    );
                }
                None => {
                    let _ = writeln!(
                        md,
                        "\nNo friction patch: static and wave estimates did not both exist, \
                         agree, and stay non-negative (see Friction table)."
                    );
                }
            }
        }
        None => {
            let _ = writeln!(
                md,
                "No patch proposed ({}). Fix the cause (parameters, sweep, unmodelled load) and re-run.",
                fit.verdict
            );
        }
    }
    md
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use armee_dynamics::LinkInertial;

    const HEADER: &str = "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event";

    fn row(tick: u64, joint: &str, q: f64, dq: f64, target: f64, tau: f64) -> String {
        format!(
            "{tick},{},{joint},{q},{dq},0,0,0,{target},{target},0,0,0,0,0,\"Hold, settled\",static,0.1,0,0,0,0.2,{tau},0,18,3,0,0,0,tick",
            tick * 5
        )
    }

    fn plan(steps: Vec<PlanStep>) -> Plan {
        Plan {
            version: 2,
            session_ts: "20261003T120000Z".into(),
            profile: "arm_attached".into(),
            sweep_joint: "a".into(),
            fixed_rad: BTreeMap::new(),
            poses_rad: vec![0.5],
            approach_offset_rad: 0.05,
            measure_sec: 1.0,
            steps,
            gravity_gate_report: String::new(),
            session_complete: true,
        }
    }

    fn step(target: f64, approach: Option<Approach>) -> PlanStep {
        PlanStep::Hold {
            joint: "a".into(),
            target_rad: target,
            measure: approach.is_some(),
            pose_index: approach.map(|_| 0),
            approach,
        }
    }

    #[test]
    fn steady_window_averages_settled_tail_and_cancels_friction() {
        let mut csv = vec![HEADER.to_string()];
        let mut tick = 0;
        let mut push = |target: f64, q: f64, dq: f64, tau: f64, n: u64, csv: &mut Vec<String>| {
            for _ in 0..n {
                csv.push(row(tick, "a", q, dq, target, tau));
                csv.push(row(tick, "b", 0.0, 0.0, 0.0, 0.01));
                tick += 1;
            }
        };
        push(0.45, 0.45, 0.0, 9.0, 100, &mut csv);
        push(0.5, 0.47, 0.4, 5.0, 100, &mut csv); // moving
        push(0.5, 0.498, 0.0, 1.08, 400, &mut csv); // settled 2 s, from below
        push(0.55, 0.55, 0.0, 9.0, 100, &mut csv);
        push(0.5, 0.502, 0.0, 0.92, 400, &mut csv); // from above
        let p = plan(vec![
            step(0.45, None),
            step(0.5, Some(Approach::Below)),
            step(0.55, None),
            step(0.5, Some(Approach::Above)),
        ]);
        let ticks = parse_trace(&csv.join("\n")).unwrap();
        let joints = vec!["a".to_string(), "b".to_string()];
        let seg = segment_plan(&p, &ticks);
        assert!(seg.skipped.is_empty());
        let (m, skipped) = measure_steps(&p, &ticks, &joints, &seg.holds);
        assert!(skipped.is_empty());
        assert_eq!(m.len(), 2);
        assert!((m[0].tau[0] - 1.08).abs() < 1e-9);
        assert!((m[0].tau_cmd[0] - 0.3).abs() < 1e-9);
        assert_eq!(m[0].samples, 201, "last 1 s at 5 ms ticks, inclusive");
        let (poses, skipped) = pair_approaches(&p, m);
        assert!(skipped.is_empty());
        assert!((poses[0].sample.tau_meas[0] - 1.0).abs() < 1e-9);
        assert!((poses[0].friction[0] - 0.08).abs() < 1e-9);
        assert!((poses[0].sample.q[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn trace_parser_groups_interleaved_ticks_and_rejects_time_regression() {
        let mut csv = vec![HEADER.to_string()];
        csv.push(row(1, "a", 0.5, 0.0, 0.5, 1.0));
        csv.push(row(2, "a", 0.5, 0.0, 0.5, 1.0));
        csv.push(row(1, "b", 0.0, 0.0, 0.0, 0.1).replacen("5,", "11,", 1));
        let ticks = parse_trace(&csv.join("\n")).expect("interleaved rows");
        assert_eq!(ticks.len(), 2);
        assert_eq!(ticks[0].rows.len(), 2);

        let csv = [
            HEADER,
            &row(2, "a", 0.5, 0.0, 0.5, 1.0),
            &row(1, "a", 0.5, 0.0, 0.5, 1.0),
        ]
        .join("\n");
        assert!(matches!(
            parse_trace(&csv),
            Err(GravityFitError::Trace { .. })
        ));
    }

    #[test]
    fn unsettled_and_missing_targets_are_skipped_not_refused() {
        let mut csv = vec![HEADER.to_string()];
        for t in 0..300 {
            csv.push(row(t, "a", 0.3, 0.2, 0.5, 1.0));
        }
        let ticks = parse_trace(&csv.join("\n")).unwrap();
        let joints = vec!["a".to_string()];
        let p = plan(vec![step(0.5, Some(Approach::Below))]);
        let seg = segment_plan(&p, &ticks);
        assert!(seg.skipped.is_empty(), "target 0.5 is present");
        let (m, skipped) = measure_steps(&p, &ticks, &joints, &seg.holds);
        assert!(m.is_empty());
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].reason.contains("never settled"), "{skipped:?}");
        let p = plan(vec![step(0.7, Some(Approach::Below))]);
        let seg = segment_plan(&p, &ticks);
        assert_eq!(seg.skipped.len(), 1);
        assert!(seg.skipped[0].reason.contains("never appears"), "{seg:?}");
    }

    #[test]
    fn patch_rewrites_only_inertial_mass_and_origin() {
        let urdf = "<robot>\n  <link name=\"x\">\n    <inertial>\n      <origin xyz=\"0 0 -0.1\" rpy=\"0 0 0\"/>\n      <mass value=\"0.5\"/>\n    </inertial>\n    <visual>\n      <origin xyz=\"0 0 -0.1\" rpy=\"0 0 0\"/>\n    </visual>\n  </link>\n  <joint name=\"j\" type=\"revolute\">\n    <origin xyz=\"0 0 -0.1\" rpy=\"0 0 0\"/>\n    <limit lower=\"-1\" upper=\"1\" effort=\"5\" velocity=\"2\"/>\n  </joint>\n</robot>\n";
        let link = FittedLink {
            link: "x".into(),
            cad: LinkInertial {
                mass_kg: 0.5,
                com_m: [0.0, 0.0, -0.1],
            },
            fitted: LinkInertial {
                mass_kg: 0.61234,
                com_m: [0.0, 0.0, -0.112345],
            },
            principal_axis: [0.0, 0.0, -1.0],
        };
        let patched = patch_inertials(urdf, std::slice::from_ref(&link)).unwrap();
        let diff = unified_diff(urdf, &patched, PATCH_PATH);
        let changed: Vec<&str> = diff
            .lines()
            .filter(|l| {
                (l.starts_with('-') || l.starts_with('+'))
                    && !l.starts_with("---")
                    && !l.starts_with("+++")
            })
            .collect();
        assert_eq!(
            changed,
            [
                "-      <origin xyz=\"0 0 -0.1\" rpy=\"0 0 0\"/>",
                "+      <origin xyz=\"0.00000 0.00000 -0.11235\" rpy=\"0 0 0\"/>",
                "-      <mass value=\"0.5\"/>",
                "+      <mass value=\"0.6123\"/>",
            ]
        );
        assert!(diff.contains("@@ -1,8 +1,8 @@"), "{diff}");

        let missing = FittedLink {
            link: "nope".into(),
            ..link
        };
        assert!(patch_inertials(urdf, &[missing]).is_err());
    }

    #[test]
    fn plan_targets_outside_windows_are_refusals() {
        let windows = vec![JointWindow {
            joint: "a".into(),
            lower_rad: -0.1,
            upper_rad: 0.52,
        }];
        let joints = vec!["a".to_string()];
        let p = plan(vec![step(0.55, None), step(0.5, Some(Approach::Above))]);
        let err = validate_plan(&p, &joints, &windows).unwrap_err();
        assert!(err.is_refusal(), "{err}");
        assert!(err.to_string().contains("step 0"), "{err}");
    }
}
