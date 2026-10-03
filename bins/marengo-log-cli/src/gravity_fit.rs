//! `gravity-fit`: turn `pi_gravity_calibrate` session directories into a proposed URDF
//! inertial patch and a dated calibration record.
//!
//! Each directory (`var/gravity-calibration/<TS>/`) holds `plan.json`, the session's
//! `position-trace.csv`, the Pi's pre-session `pi-marengo.urdf` and `config/*.yaml`.
//!
//! 1. **Steady state per step**: a step is the run of trace ticks after its `hold-at` until
//!    any joint's operator target changes. Its window is the last `measure_sec` (≥ 1 s) of the
//!    trailing run of settled ticks (every |dq| ≤ [`SETTLED_DQ_RAD_S`], stepped joint within
//!    [`SETTLED_BAND_RAD`] of target). Averages: q, drive torque `tau_meas` (joint space; at
//!    rest it equals gravity plus static friction) and the commanded `tau_p + tau_ff_cmd`
//!    cross-check. The P term is not subtracted: the drive torque already contains it.
//! 2. **Friction**: every pose is measured approached from below and from above; the mean
//!    cancels Coulomb friction ([`cancel_friction`]).
//! 3. **Fit**: [`fit_gravity_params`] on the Pi URDF (ADR 0017), with poses limited to
//!    `control.yaml` soft ∩ `motors.yaml` hard windows.
//! 4. **Output**: `<out-dir>/<date>-gravity-<ts>.{json,md}` whenever a fit ran;
//!    `.urdf.patch` only when it is accepted. The patch touches only `<inertial>` mass/origin
//!    lines of right-arm links. Nothing is applied: syncing the URDF to the Pi stays an
//!    explicit `pi_sync_bench_urdf`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use armee_dynamics::calibration::{
    cancel_friction, default_params, fit_gravity_params, CalibrationError, FitOptions, FittedLink,
    GravityFit, GravitySample, InertialParam, JointWindow,
};
use armee_dynamics::{gravity_model_from_urdf, DynamicsError, DynamicsModel, UrdfGravityModel};
use marengo_config::{
    commanded_position_window, load_control_config_from, load_motors_config_from,
    load_robot_config_from, ConfigError,
};
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;

/// Settled: every joint's |dq| at most this (rad/s).
pub const SETTLED_DQ_RAD_S: f64 = 0.05;
/// Settled: the stepped joint within this of its target (rad).
pub const SETTLED_BAND_RAD: f64 = 0.05;
/// Shortest averaging window (s).
pub const MIN_WINDOW_S: f64 = 1.0;
/// Trace target match tolerance (trace prints 6 decimals).
const TARGET_TOL_RAD: f64 = 5e-5;
/// Patched-URDF τ must match the fit within this (rounding of printed mass/COM).
const PATCH_ROUNDTRIP_TOL_NM: f64 = 0.005;
/// Limb whose links the patch may touch.
const PATCH_LIMB: &str = "right_arm";

const PLAN_FILE: &str = "plan.json";
const TRACE_FILE: &str = "position-trace.csv";
const PI_URDF_FILE: &str = "pi-marengo.urdf";
const PROPOSED_URDF_FILE: &str = "proposed-marengo.urdf";
const PATCH_PATH: &str = "assets/urdf/marengo.urdf";

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
    #[error("step {step} ({joint} → {target_rad} rad): {message}")]
    Steady {
        step: usize,
        joint: String,
        target_rad: f64,
        message: String,
    },
    #[error("{0}")]
    OutOfLimits(String),
    #[error("urdf patch: {0}")]
    Patch(String),
}

impl GravityFitError {
    /// Inputs outside the joint windows are a refusal (exit 2), not a tool error.
    pub fn is_refusal(&self) -> bool {
        matches!(
            self,
            Self::OutOfLimits(_) | Self::Calibration(CalibrationError::PoseOutOfLimits { .. })
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
    poses_rad: Vec<f64>,
    approach_offset_rad: f64,
    measure_sec: f64,
    steps: Vec<Step>,
    #[serde(default)]
    gravity_gate_report: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Step {
    joint: String,
    target_rad: f64,
    measure: bool,
    pose_index: Option<usize>,
    approach: Option<Approach>,
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

struct Session {
    dir: PathBuf,
    plan: Plan,
    poses: Vec<PoseData>,
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
        let poses: Vec<Vec<f64>> = samples.iter().map(|s| s.q.clone()).collect();
        let sweep = most_upstream_sweep(&model, &sessions)?;
        default_params(&model, &sweep, &poses, &args.fit_joints, &opts)?
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

    let record = record_json(
        &sessions,
        &fit,
        &opts,
        local_urdf == pi_urdf,
        patch_file.as_deref(),
    );
    let json_path = args.out_dir.join(format!("{stem}.json"));
    let pretty = serde_json::to_string_pretty(&record).map_err(|source| GravityFitError::Json {
        path: json_path.clone(),
        source,
    })?;
    std::fs::write(&json_path, format!("{pretty}\n")).map_err(io(&json_path))?;
    let md_path = args.out_dir.join(format!("{stem}.md"));
    let md = record_markdown(
        &stem,
        &sessions,
        &fit,
        &opts,
        local_urdf == pi_urdf,
        patch_file.as_deref(),
    );
    std::fs::write(&md_path, md).map_err(io(&md_path))?;

    println!("{}", summary(&fit));
    println!("record: {}", json_path.display());
    println!("record: {}", md_path.display());
    match &patch_file {
        Some(p) => {
            println!("proposed patch: {}", p.display());
            println!(
                "Nothing applied. Review, `git apply` it to the Pi URDF copy at {PATCH_PATH} \
                 (ADR 0017), then pi_sync_bench_urdf as a separate explicit step."
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
    let measurements = measure_steps(&plan, &ticks, joints)?;
    let poses = pair_approaches(&plan, measurements)?;
    Ok(Session {
        dir: dir.to_path_buf(),
        plan,
        poses,
    })
}

fn validate_plan(
    plan: &Plan,
    joints: &[String],
    windows: &[JointWindow],
) -> Result<(), GravityFitError> {
    if plan.version != 1 {
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
        if !(w.lower_rad..=w.upper_rad).contains(&rad) {
            return Err(GravityFitError::OutOfLimits(format!(
                "{what}: {joint} {rad} rad outside soft ∩ hard window [{:.4}, {:.4}]",
                w.lower_rad, w.upper_rad
            )));
        }
        Ok(())
    };
    for (joint, &rad) in &plan.fixed_rad {
        check(joint, rad, "fixed pose")?;
    }
    for &rad in &plan.poses_rad {
        check(&plan.sweep_joint, rad, "pose")?;
    }
    for (i, s) in plan.steps.iter().enumerate() {
        check(&s.joint, s.target_rad, &format!("step {i}"))?;
        if s.measure {
            let ok = s.pose_index.is_some_and(|p| p < plan.poses_rad.len())
                && s.approach.is_some()
                && s.joint == plan.sweep_joint;
            if !ok {
                return Err(GravityFitError::Plan(format!(
                    "measured step {i} needs the sweep joint, a valid pose_index and an approach"
                )));
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

/// Tick ranges `[start, end)` of each plan step, in order.
fn segment_steps(plan: &Plan, ticks: &[Tick]) -> Result<Vec<(usize, usize)>, GravityFitError> {
    let mut starts = Vec::with_capacity(plan.steps.len());
    let mut next = 0;
    for (i, tick) in ticks.iter().enumerate() {
        let Some(step) = plan.steps.get(next) else {
            break;
        };
        let Some(row) = tick.rows.get(&step.joint) else {
            continue;
        };
        let changed = i == 0
            || ticks[i - 1]
                .rows
                .get(&step.joint)
                .is_none_or(|prev| (prev.target_raw - row.target_raw).abs() > TARGET_TOL_RAD);
        if changed && (row.target_raw - step.target_rad).abs() <= TARGET_TOL_RAD {
            starts.push(i);
            next += 1;
        }
    }
    if starts.len() != plan.steps.len() {
        let step = &plan.steps[starts.len()];
        return Err(GravityFitError::Steady {
            step: starts.len(),
            joint: step.joint.clone(),
            target_rad: step.target_rad,
            message: "hold-at target never appears in the trace (session aborted?)".into(),
        });
    }
    Ok(starts
        .iter()
        .enumerate()
        .map(|(k, &start)| {
            let limit = starts.get(k + 1).copied().unwrap_or(ticks.len());
            let entry = targets(&ticks[start]);
            let end = (start..limit)
                .find(|&i| !same_targets(&targets(&ticks[i]), &entry))
                .unwrap_or(limit);
            (start, end)
        })
        .collect())
}

fn measure_steps(
    plan: &Plan,
    ticks: &[Tick],
    joints: &[String],
) -> Result<Vec<Measurement>, GravityFitError> {
    let segments = segment_steps(plan, ticks)?;
    let measure_s = plan.measure_sec.max(MIN_WINDOW_S);
    let mut out = Vec::new();
    for (k, (step, &(start, end))) in plan.steps.iter().zip(&segments).enumerate() {
        let (Some(pose_index), Some(approach), true) =
            (step.pose_index, step.approach, step.measure)
        else {
            continue;
        };
        let fail = |message: String| GravityFitError::Steady {
            step: k,
            joint: step.joint.clone(),
            target_rad: step.target_rad,
            message,
        };
        let settled = |t: &Tick| {
            joints.iter().all(|j| {
                t.rows
                    .get(j)
                    .is_some_and(|r| r.dq.abs() <= SETTLED_DQ_RAD_S)
            }) && t
                .rows
                .get(&step.joint)
                .is_some_and(|r| (r.q - step.target_rad).abs() <= SETTLED_BAND_RAD)
        };
        let run_start = (start..end)
            .rev()
            .take_while(|&i| settled(&ticks[i]))
            .last()
            .ok_or_else(|| fail("never settled".into()))?;
        let last_ms = ticks[end - 1].t_ms;
        let run_ms = last_ms
            .checked_sub(ticks[run_start].t_ms)
            .ok_or_else(|| fail("trace time regressed within settled window".into()))?;
        let run_s = run_ms as f64 / 1000.0;
        if run_s < MIN_WINDOW_S {
            return Err(fail(format!(
                "settled for only {run_s:.2} s (< {MIN_WINDOW_S} s): increase settle_sec"
            )));
        }
        let from_ms = last_ms.saturating_sub((measure_s * 1000.0) as u64);
        let window: Vec<&Tick> = ticks[run_start..end]
            .iter()
            .filter(|t| t.t_ms >= from_ms)
            .collect();
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
        let window_ms = last_ms
            .checked_sub(window_start_ms)
            .ok_or_else(|| fail("trace time regressed within measurement window".into()))?;
        out.push(Measurement {
            pose_index,
            approach,
            q: mean(&|r| r.q),
            tau: mean(&|r| r.tau_meas),
            tau_cmd: mean(&|r| r.tau_cmd),
            samples: window.len(),
            window_s: window_ms as f64 / 1000.0,
        });
    }
    Ok(out)
}

fn pair_approaches(
    plan: &Plan,
    measurements: Vec<Measurement>,
) -> Result<Vec<PoseData>, GravityFitError> {
    let mut out = Vec::new();
    for (i, &pose) in plan.poses_rad.iter().enumerate() {
        let find = |a: Approach| {
            measurements
                .iter()
                .rfind(|m| m.pose_index == i && m.approach == a)
                .cloned()
                .ok_or_else(|| {
                    GravityFitError::Plan(format!(
                        "pose {i} ({pose} rad): no {a:?} measurement; friction cannot cancel"
                    ))
                })
        };
        let below = find(Approach::Below)?;
        let above = find(Approach::Above)?;
        let (tau, friction) = cancel_friction(&below.tau, &above.tau)?;
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
    Ok(out)
}

/// Swept joint with the most downstream links (pitch when pitch and elbow sweeps are fused).
fn most_upstream_sweep(
    model: &UrdfGravityModel,
    sessions: &[Session],
) -> Result<String, GravityFitError> {
    let mut best: Option<(usize, String)> = None;
    for s in sessions {
        let n = model.links_downstream_of(&s.plan.sweep_joint)?.len();
        if best.as_ref().is_none_or(|(m, _)| n > *m) {
            best = Some((n, s.plan.sweep_joint.clone()));
        }
    }
    best.map(|(_, j)| j)
        .ok_or_else(|| GravityFitError::Plan("no sessions".into()))
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
    opts: &FitOptions,
    local_matches_pi: bool,
    patch: Option<&Path>,
) -> Value {
    let pose_data: Vec<&PoseData> = sessions.iter().flat_map(|s| &s.poses).collect();
    json!({
        "version": 1,
        "kind": "gravity_calibration",
        "verdict": fit.verdict.to_string(),
        "accepted": fit.accepted(),
        "sessions": sessions.iter().map(|s| json!({
            "dir": display_relative(&s.dir),
            "session_ts": s.plan.session_ts,
            "profile": s.plan.profile,
            "sweep_joint": s.plan.sweep_joint,
            "fixed_rad": s.plan.fixed_rad,
            "poses_rad": s.plan.poses_rad,
            "approach_offset_rad": s.plan.approach_offset_rad,
            "measure_sec": s.plan.measure_sec,
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
        "patch": patch.map(display_relative),
        "apply": "Never automatic. Review the patch, apply it to the Pi URDF copy at assets/urdf/marengo.urdf, commit, then pi_sync_bench_urdf (ADR 0017). Gram-level weighing of the frozen arm supersedes this record.",
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

fn record_markdown(
    stem: &str,
    sessions: &[Session],
    fit: &GravityFit,
    opts: &FitOptions,
    local_matches_pi: bool,
    patch: Option<&Path>,
) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# Gravity calibration {stem}\n");
    let _ = writeln!(md, "**Verdict:** {}\n", fit.verdict);
    let _ = writeln!(
        md,
        "Generated by `marengo-log-cli gravity-fit` from `pi_gravity_calibrate` sessions; see \
         [limb playbook](../limb-playbook.md#gravity-calibration-after-hardware-changes). \
         Gram-level weighing of the frozen arm supersedes this record.\n"
    );
    let _ = writeln!(md, "## Sessions\n");
    let _ = writeln!(
        md,
        "| session | profile | sweep | fixed | poses (rad) | δ approach |"
    );
    let _ = writeln!(md, "|---|---|---|---|---|---|");
    for s in sessions {
        let fixed = s
            .plan
            .fixed_rad
            .iter()
            .map(|(j, v)| format!("{j}={v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {:?} | {} |",
            s.plan.session_ts,
            s.plan.profile,
            s.plan.sweep_joint,
            if fixed.is_empty() { "-".into() } else { fixed },
            s.plan.poses_rad,
            s.plan.approach_offset_rad
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

    fn plan(steps: Vec<Step>) -> Plan {
        Plan {
            version: 1,
            session_ts: "20261003T120000Z".into(),
            profile: "arm_attached".into(),
            sweep_joint: "a".into(),
            fixed_rad: BTreeMap::new(),
            poses_rad: vec![0.5],
            approach_offset_rad: 0.05,
            measure_sec: 1.0,
            steps,
            gravity_gate_report: String::new(),
        }
    }

    fn step(target: f64, approach: Option<Approach>) -> Step {
        Step {
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
        let m = measure_steps(&p, &ticks, &joints).unwrap();
        assert_eq!(m.len(), 2);
        assert!((m[0].tau[0] - 1.08).abs() < 1e-9);
        assert!((m[0].tau_cmd[0] - 0.3).abs() < 1e-9);
        assert_eq!(m[0].samples, 201, "last 1 s at 5 ms ticks, inclusive");
        let poses = pair_approaches(&p, m).unwrap();
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
    fn refuses_unsettled_step_and_missing_target() {
        let mut csv = vec![HEADER.to_string()];
        for t in 0..300 {
            csv.push(row(t, "a", 0.3, 0.2, 0.5, 1.0));
        }
        let ticks = parse_trace(&csv.join("\n")).unwrap();
        let joints = vec!["a".to_string()];
        let err = measure_steps(
            &plan(vec![step(0.5, Some(Approach::Below))]),
            &ticks,
            &joints,
        )
        .unwrap_err();
        assert!(err.to_string().contains("never settled"), "{err}");
        let err = measure_steps(
            &plan(vec![step(0.7, Some(Approach::Below))]),
            &ticks,
            &joints,
        )
        .unwrap_err();
        assert!(err.to_string().contains("never appears"), "{err}");
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
