// ADR 0039 simulation gates: the legacy law and the scaled-PD law each drive the same
// stick-slip plant through production `PositionHold::{apply_retarget, tick}`.
//
// Plant (shoulder-pitch-like): inertia, gravity, static/Coulomb friction (fs 0.14, fc 0.08 Nm,
// the ADR's plant), and a drive that runs the MIT law `kp·(q_des − q) + kd·(v_des − dq) + τ_ff`
// on its own velocity at 4 kHz, with the joint torque limit. Between Berthier and the drive the
// harness applies Davout's τ_ff cap and 60 Nm/s rate limit. Feedback: q on the RS03 grid
// (0.383 mrad) with ±0.3 count encoder noise, dq as the 5 ms grid difference (the ~0.077 rad/s
// quantum the bench reports), one tick of transport delay.
//
// Each Phase 2 gate must fail on the legacy law and pass on the scaled-PD law.
//
// The Phase 3 pitch trial (`pitch_bench_trial_meets_bench_criteria`) runs the master pitch
// entry of `config/control.yaml` and the master URDF's τ_g against the plant the 2026-10-04
// wave fit identified, and must meet the bench pass criteria before the bench runs them.

use std::path::{Path, PathBuf};

use armee_dynamics::gravity_model_from_urdf;
use armee_dynamics::lumped::lumped_terms;

use super::*;
use crate::position_law::{ReferenceFriction, ScaledPdGains};
use crate::position_setpoint::downward_return_seed_velocity;

const DT: f64 = 0.005;
const HZ: u32 = 200;
const SUBSTEPS: usize = 20;

// Phase 2 pitch tuning (control.yaml before the 2026-10-04 fit); inputs, not config.
const KP: f64 = 18.0;
const KD: f64 = 3.0;
const KI: f64 = 5.0;
const FC: f64 = 0.08;
const FRICTION_K: f64 = 10.0;
const LEAD: f64 = 0.12;
const SLEW: f64 = 0.15;
const V_TRAJ: f64 = 1.25;
const THRESHOLD: f64 = 0.15;
const A_MAX: f64 = 4.5;
const VELOCITY_CAP: f64 = 2.5;
const TAU_LIMIT: f64 = 5.0;
const TAU_FF_RATE_NM_S: f64 = 60.0;

// Phase 2 plant.
const INERTIA: f64 = 0.12;
const GRAVITY_NM: f64 = 2.7;
const PLANT_FS: f64 = 0.14;
const PLANT_FC: f64 = 0.08;
const PLANT_FV: f64 = 0.02;

/// ADR 0039 gate: no host τ_ff step larger than this per tick outside retarget ticks (Nm).
const TAU_STEP_GATE_NM: f64 = 0.05;
/// ADR 0039 gate: measured velocity overshoot below this fraction of the planned peak.
const OVERSHOOT_GATE: f64 = 0.20;

/// Davout's total-torque prediction (`davout/src/total_torque.rs`): transport-delay horizon
/// in ticks and velocity-estimate margin (rad/s).
const TOTAL_TORQUE_HORIZON_TICKS: f64 = 2.0;
const TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S: f64 = 0.1;

/// Window of the bench speed estimate (`scripts/analyze-position-trace.py --score-bench`):
/// Δq over 50 ms, so one encoder count is 8 mrad/s rather than one 0.077 rad/s quantum.
const SPEED_WINDOW_TICKS: usize = 10;

fn rs03_count() -> f64 {
    f64::from(4.0 * std::f32::consts::PI) / 32767.0
}

fn progress_threshold() -> f64 {
    let span = f64::from(4.0 * std::f32::consts::PI);
    span / 32767.0 / 2.0 + 16.0 * f64::from(f32::EPSILON) * span
}

fn friction_gains() -> FrictionGains {
    FrictionGains {
        fc: FC,
        fv: 0.0,
        fo: 0.0,
        k: FRICTION_K,
        fs: None,
        v_b: None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Law {
    Legacy,
    ScaledPd,
}

fn params(law: Law) -> HoldJointParams {
    let hold_law = match law {
        Law::Legacy => HoldLaw::Legacy,
        // Every scaled-PD value is the documented default resolved from the pitch entry.
        Law::ScaledPd => HoldLaw::ScaledPd(ScaledPdGains {
            kd: KD,
            e0: LEAD * marengo_config::DEFAULT_POSITION_TIME_SCALE_E0_FRACTION,
            e1: LEAD,
            integral_band: marengo_config::DEFAULT_POSITION_INTEGRAL_BAND_RAD,
            integral_leak_s: marengo_config::DEFAULT_POSITION_INTEGRAL_LEAK_S,
            friction: Some(ReferenceFriction::from_gains(&friction_gains(), None)),
        }),
    };
    HoldJointParams {
        kp: KP,
        kd: KD,
        ki: KI,
        max_lead: LEAD,
        vel_deadband: 0.02,
        advance_max_lead: LEAD,
        advance_vel_deadband: 0.02,
        slew_rad_s: SLEW,
        trajectory_v_max: V_TRAJ,
        trajectory_threshold_rad: THRESHOLD,
        a_max: A_MAX,
        velocity_cap: Some(VELOCITY_CAP),
        friction: Some(friction_gains()),
        limit_policy: None,
        tau_meas: 0.0,
        law: hold_law,
    }
}

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// True joint: inertia, gravity `a·sin q + b·cos q`, static/Coulomb/viscous friction.
#[derive(Clone, Copy, Debug)]
struct PlantModel {
    inertia: f64,
    gravity_a: f64,
    gravity_b: f64,
    fs: f64,
    fc: f64,
    fv: f64,
}

/// Rigid joint with stick-slip friction, driven by an MIT servo.
struct Plant {
    q: f64,
    dq: f64,
    model: PlantModel,
}

impl Plant {
    fn step(&mut self, cmd: &DavoutMit) {
        let m = self.model;
        let h = DT / SUBSTEPS as f64;
        for _ in 0..SUBSTEPS {
            let motor = (cmd.kp * (cmd.position_rad - self.q)
                + cmd.kd * (cmd.velocity_rad_s - self.dq)
                + cmd.torque_ff_nm)
                .clamp(-TAU_LIMIT, TAU_LIMIT);
            let drive = motor - m.gravity_a * self.q.sin() - m.gravity_b * self.q.cos();
            if self.dq == 0.0 {
                if drive.abs() <= m.fs {
                    continue;
                }
                let accel = (drive - m.fc * drive.signum()) / m.inertia;
                self.dq = accel * h;
            } else {
                let accel = (drive - m.fc * self.dq.signum() - m.fv * self.dq) / m.inertia;
                let next = self.dq + accel * h;
                // Sticks when the velocity crosses zero.
                self.dq = if next * self.dq < 0.0 { 0.0 } else { next };
            }
            self.q += self.dq * h;
        }
    }
}

/// Davout's τ_ff cap and rate limiter at the nominal period.
fn davout_tau_ff(previous: f64, requested: f64) -> f64 {
    let step = TAU_FF_RATE_NM_S * DT;
    let target = requested.clamp(-TAU_LIMIT, TAU_LIMIT);
    (previous + (target - previous).clamp(-step, step)).clamp(-TAU_LIMIT, TAU_LIMIT)
}

/// Davout's worst-case total MIT torque for the sent command and its feedback (Nm).
fn predicted_total_torque(cmd: &DavoutMit, q: f64, dq: f64) -> f64 {
    let pd = cmd.kp * (cmd.position_rad - q) + cmd.kd * (cmd.velocity_rad_s - dq);
    let e_max = dq.abs().max(cmd.velocity_rad_s.abs()) * TOTAL_TORQUE_HORIZON_TICKS / f64::from(HZ);
    (pd + cmd.torque_ff_nm).abs() + cmd.kp * e_max + cmd.kd * TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S
}

#[derive(Debug, Default)]
struct Metrics {
    /// Largest host τ_ff change in one tick, outside retarget ticks (Nm).
    max_tau_step: f64,
    /// Largest host τ_ff change in one tick, retarget ticks included (Nm).
    max_tau_step_any: f64,
    /// Ticks with a full-rate moving reference, and those with measured dq exactly 0.
    moving_rows: usize,
    stuck_rows: usize,
    /// Per move: largest true speed above the planned peak, as a fraction of the plan.
    overshoot: Vec<f64>,
    /// Per move: the same from Δq over [`SPEED_WINDOW_TICKS`] (the bench estimate).
    overshoot_windowed: Vec<f64>,
    /// Per move: furthest true position past the target in the direction of travel (rad).
    end_overshoot: Vec<f64>,
    /// Largest speed excess over the scenario's overshoot bound (≤ 1 passes).
    overshoot_vs_bound: f64,
    /// Largest true speed (rad/s).
    max_speed: f64,
    /// Wire kd / v_des toggles and planner phase changes while the joint rests at its target.
    rest_toggles: usize,
    /// Ticks whose predicted total torque exceeds the cap (Davout would clamp them).
    total_torque_clamps: usize,
    /// Largest predicted total torque (Nm).
    max_total_torque: f64,
    /// Fuse trip, if the law faulted.
    fault: Option<String>,
}

impl Metrics {
    fn worst(values: &[f64]) -> f64 {
        values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    }
}

/// Velocity overshoot gate (rad/s above the planned peak): the ADR's 20 % of plan, floored at
/// `fs/kd`. A joint resting in its static dead zone (`|e| ≤ fs/kp`) starts a move that far
/// behind its reference, and a PD with drive damping catches a lag `e` up at `(kp/kd)·e` above
/// the reference speed, so `fs/kd` is the catch-up excess no law with this kp/kd avoids.
fn overshoot_bound(v_plan: f64) -> f64 {
    (OVERSHOOT_GATE * v_plan).max(PLANT_FS / KD)
}

/// Gravity plants: exact model, and true gravity at 1/(1 ± 50 %) of the model (the ADR's ±50 %
/// model error; ×2 is the elbow-style light model) plus ×1.5.
const GRAVITY_SCALES: [f64; 4] = [1.0, 0.67, 1.5, 2.0];

/// Encoder noise, peak to peak, in feedback counts: at rest the reading toggles between two
/// adjacent levels, so the 5 ms difference dithers by one velocity quantum.
const NOISE_COUNTS: f64 = 1.0;

/// A gravity-calibration-like session (`pi_gravity_calibrate` approaches): a fast outbound
/// move, slow slew approaches from below and from above, and the return home, each followed
/// by a rest window. `(target rad, seconds)`.
const MOVES: [(f64, f64); 4] = [(0.75, 2.5), (0.80, 2.0), (0.70, 2.0), (0.0, 2.5)];

/// One simulated session: the joint's parameters, the τ_g model Berthier holds, the true plant.
struct Scenario<'a> {
    params: HoldJointParams,
    /// `(A, B)` of the τ_g model Berthier feeds forward.
    model: (f64, f64),
    plant: PlantModel,
    moves: &'a [(f64, f64)],
    /// Overshoot bound for `overshoot_vs_bound` (rad/s) given the planned peak.
    bound: fn(f64) -> f64,
}

fn phase2(law: Law, gravity_scale: f64) -> Scenario<'static> {
    Scenario {
        params: params(law),
        model: (GRAVITY_NM, 0.0),
        plant: PlantModel {
            inertia: INERTIA,
            gravity_a: gravity_scale * GRAVITY_NM,
            gravity_b: 0.0,
            fs: PLANT_FS,
            fc: PLANT_FC,
            fv: PLANT_FV,
        },
        moves: &MOVES,
        bound: overshoot_bound,
    }
}

fn simulate(scenario: &Scenario) -> Metrics {
    let jp = &scenario.params;
    let mut hold = PositionHold::with_progress_thresholds(vec![progress_threshold()]);
    let mut plant = Plant {
        q: 0.0,
        dq: 0.0,
        model: scenario.plant,
    };
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let grid = rs03_count();
    let sense = |q: f64, rng: &mut Rng| {
        ((q + (rng.unit() - 0.5) * NOISE_COUNTS * grid) / grid).round() * grid
    };
    let mut q_meas = sense(plant.q, &mut rng);
    let mut dq_meas = 0.0;
    let params = [jp.clone()];
    let names = [String::from("right_shoulder_pitch")];
    let (model_a, model_b) = scenario.model;
    let cap = jp.velocity_cap.unwrap_or(f64::INFINITY);
    let mut wave = None;
    let mut metrics = Metrics::default();
    let mut tick: u64 = 0;
    let mut tau_wire = 0.0;
    let mut last_tau: Option<f64> = None;
    let mut last_wire: Option<(f64, f64, &'static str)> = None;
    let mut q_history: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    hold.set_joint_law(0, params[0].law.kind());
    hold.arm(&[q_meas], &[q_meas], 0);
    for (move_index, &(target, seconds)) in scenario.moves.iter().enumerate() {
        let start = q_meas;
        let direction = (target - start).signum();
        let v_plan = {
            let distance = (target - start).abs();
            let v = if distance <= jp.trajectory_threshold_rad {
                jp.slew_rad_s
            } else {
                jp.trajectory_v_max
            };
            v.min((jp.a_max * distance).sqrt()).min(cap)
        };
        metrics.overshoot.push(f64::NEG_INFINITY);
        metrics.overshoot_windowed.push(f64::NEG_INFINITY);
        metrics.end_overshoot.push(0.0);
        let seed =
            downward_return_seed_velocity(jp.slew_rad_s, jp.trajectory_v_max, q_meas, target);
        hold.apply_retarget(HoldRetarget {
            joint_idx: 0,
            clamped: target,
            requested: target,
            q: q_meas,
            tick,
            dq_seed: Some(dq_meas),
            downward_seed: Some(seed),
        });
        let retarget_tick = tick;
        let ticks = (seconds * f64::from(HZ)) as u64;
        let rest_from = retarget_tick + ticks - u64::from(HZ);
        for _ in 0..ticks {
            tick += 1;
            let tau_g = [model_a * q_meas.sin() + model_b * q_meas.cos()];
            let out = match hold.tick(HoldWorld {
                q: &[q_meas],
                dq_meas: &[dq_meas],
                tau_g: &tau_g,
                joints: &params,
                joint_names: &names,
                dt: DT,
                hz: HZ,
                tick_count: tick,
                wave: &mut wave,
            }) {
                Ok(out) => out,
                Err(error) => {
                    metrics.fault = Some(error.to_string());
                    return metrics;
                }
            };
            let mut cmd = out.mit[0].clone();
            let diag = &out.diag[0];
            if let Some(previous) = last_tau {
                let step = (cmd.torque_ff_nm - previous).abs();
                metrics.max_tau_step_any = metrics.max_tau_step_any.max(step);
                if tick > retarget_tick + 1 {
                    metrics.max_tau_step = metrics.max_tau_step.max(step);
                }
            }
            last_tau = Some(cmd.torque_ff_nm);
            let wire = (cmd.kd, cmd.velocity_rad_s, diag.phase);
            if tick >= rest_from {
                if let Some((kd, v, phase)) = last_wire {
                    if kd != wire.0 || v != wire.1 || phase != wire.2 {
                        metrics.rest_toggles += 1;
                    }
                }
            }
            // ADR stuck row: measured dq exactly 0 while the reference moves at full rate.
            if diag.dq_ref.abs() > 0.05 && diag.time_scale >= 1.0 {
                metrics.moving_rows += 1;
                if dq_meas == 0.0 {
                    metrics.stuck_rows += 1;
                }
            }
            last_wire = Some(wire);
            tau_wire = davout_tau_ff(tau_wire, cmd.torque_ff_nm);
            cmd.torque_ff_nm = tau_wire;
            let total = predicted_total_torque(&cmd, q_meas, dq_meas);
            metrics.max_total_torque = metrics.max_total_torque.max(total);
            if total > TAU_LIMIT {
                metrics.total_torque_clamps += 1;
            }
            plant.step(&cmd);
            let speed = plant.dq.abs();
            metrics.max_speed = metrics.max_speed.max(speed);
            metrics.overshoot[move_index] =
                metrics.overshoot[move_index].max((speed - v_plan) / v_plan);
            metrics.overshoot_vs_bound = metrics
                .overshoot_vs_bound
                .max((speed - v_plan) / (scenario.bound)(v_plan));
            q_history.push_back(plant.q);
            if q_history.len() > SPEED_WINDOW_TICKS {
                q_history.pop_front();
                let windowed =
                    (plant.q - q_history[0]).abs() / (SPEED_WINDOW_TICKS as f64 - 1.0) / DT;
                metrics.overshoot_windowed[move_index] =
                    metrics.overshoot_windowed[move_index].max((windowed - v_plan) / v_plan);
            }
            if direction != 0.0 {
                metrics.end_overshoot[move_index] =
                    metrics.end_overshoot[move_index].max((plant.q - target) * direction);
            }
            // One tick of transport delay: the next tick sees this tick's end state.
            let q_next = sense(plant.q, &mut rng);
            dq_meas = (q_next - q_meas) / DT;
            q_meas = q_next;
        }
    }
    metrics
}

fn report(label: &str, metrics: &Metrics) -> String {
    let percent = |v: &[f64]| {
        v.iter()
            .map(|v| (v * 100.0).round() as i64)
            .collect::<Vec<_>>()
    };
    let mrad = |v: &[f64]| {
        v.iter()
            .map(|v| (v * 1000.0).round() as i64)
            .collect::<Vec<_>>()
    };
    format!(
        "{label}: max τ_ff step {:.3} Nm ({:.3} with retargets), overshoot per move {:?} % \
         (50 ms Δq {:?} %), end-of-travel overshoot {:?} mrad, excess/bound {:.2}, max speed \
         {:.2} rad/s, rest toggles {}, stuck rows {}/{}, total-torque clamps {} (max {:.2} Nm), \
         fault {:?}",
        metrics.max_tau_step,
        metrics.max_tau_step_any,
        percent(&metrics.overshoot),
        percent(&metrics.overshoot_windowed),
        mrad(&metrics.end_overshoot),
        metrics.overshoot_vs_bound,
        metrics.max_speed,
        metrics.rest_toggles,
        metrics.stuck_rows,
        metrics.moving_rows,
        metrics.total_torque_clamps,
        metrics.max_total_torque,
        metrics.fault
    )
}

/// Both laws on every gravity plant, with a printable summary.
fn runs() -> (Vec<(f64, Metrics, Metrics)>, String) {
    let runs: Vec<_> = GRAVITY_SCALES
        .iter()
        .map(|&scale| {
            (
                scale,
                simulate(&phase2(Law::Legacy, scale)),
                simulate(&phase2(Law::ScaledPd, scale)),
            )
        })
        .collect();
    let summary = runs
        .iter()
        .flat_map(|(scale, legacy, scaled)| {
            [
                report(&format!("Legacy gravity×{scale}"), legacy),
                report(&format!("ScaledPd gravity×{scale}"), scaled),
            ]
        })
        .collect::<Vec<_>>()
        .join("\n");
    (runs, summary)
}

#[test]
fn gate_single_tick_tau_ff_step() {
    let (runs, summary) = runs();
    for (_, legacy, scaled) in &runs {
        assert!(scaled.fault.is_none(), "{summary}");
        assert!(scaled.max_tau_step <= TAU_STEP_GATE_NM, "{summary}");
        assert!(
            legacy.max_tau_step > TAU_STEP_GATE_NM,
            "gate must reject legacy:\n{summary}"
        );
    }
}

#[test]
fn gate_velocity_overshoot() {
    let (runs, summary) = runs();
    for (scale, legacy, scaled) in &runs {
        assert!(scaled.fault.is_none(), "{summary}");
        assert!(scaled.overshoot_vs_bound <= 1.0, "{summary}");
        // Davout's feedback-velocity fault (cap + 0.5 rad/s) never comes near.
        assert!(scaled.max_speed < VELOCITY_CAP + 0.5, "{summary}");
        // ADR stuck-row gate (< 10 %); the legacy law also passes it on this plant.
        assert!(scaled.stuck_rows * 10 < scaled.moving_rows, "{summary}");
        // With an exact model the legacy planner resets to q, so only a model error exposes it.
        if (scale - 1.0).abs() > 1e-9 {
            assert!(
                legacy.overshoot_vs_bound > 1.0,
                "gate must reject legacy:\n{summary}"
            );
        }
    }
}

#[test]
fn gate_no_chatter_at_rest() {
    let (runs, summary) = runs();
    for (_, legacy, scaled) in &runs {
        assert!(scaled.fault.is_none(), "{summary}");
        assert_eq!(scaled.rest_toggles, 0, "{summary}");
        assert!(
            legacy.rest_toggles > 0,
            "gate must reject legacy:\n{summary}"
        );
    }
}

// ---- Phase 3 pitch trial ----

const PITCH: &str = "right_shoulder_pitch";

/// Pitch friction the 2026-10-04 wave fit identified (record
/// `2026-10-04-gravity-20261004T113836Z-20261004T113951Z`): Coulomb mean fc; fv unresolved
/// (0, and 0.33 Nm·s/rad, the unidentified linear slope, as a variant).
const FIT_FC: f64 = 0.3526;
const FIT_FV_SLOPE: f64 = 0.3344;
/// Breakaway: the static-hold band of the same arm (every hold fits ±0.37 Nm about gravity,
/// `docs/commissioning/right-arm-calibration-suite.md`).
const FIT_FS: f64 = 0.37;
/// Gravity model error ±2·σ_A of the fit (σ_A 0.054 of A 2.661).
const FIT_GRAVITY_ERROR: f64 = 0.04;
/// Fitted inertia error ΔI and its σ (kg·m²): the URDF lacks the rotor, but the fit puts the
/// URDF value inside its band, so the URDF is nominal and ΔI + 2σ the heavy variant.
const FIT_DELTA_INERTIA: f64 = -0.028;
const FIT_DELTA_INERTIA_SIGMA: f64 = 0.034;

/// The bench pass criteria for the Phase 3 trial (per move).
const BENCH_OVERSHOOT: f64 = 0.20;
const BENCH_END_OVERSHOOT_RAD: f64 = 0.01;
const BENCH_TAU_STEP_NM: f64 = 0.05;

/// Calibration-like moves on both sides of hanging rest: up and the slow approaches, home,
/// down, home. `(target rad, seconds)`.
const TRIAL_MOVES: [(f64, f64); 6] = [
    (0.75, 2.5),
    (0.80, 2.0),
    (0.70, 2.0),
    (0.0, 2.5),
    (-0.5, 2.5),
    (0.0, 2.5),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Master pitch parameters (resolved like `ControlLoop` does for Position mode), the master
/// URDF's lumped pitch τ_g at the other joints' zero pose, and its pitch inertia.
fn master_pitch() -> (HoldJointParams, (f64, f64), f64) {
    let config = repo().join("config");
    let control = marengo_config::load_control_config_from(&config).expect("control.yaml");
    let joints = marengo_config::load_robot_config_from(&config)
        .expect("robot.yaml")
        .robot
        .joints;
    let entry = control.control.joints.get(PITCH).expect("pitch entry");
    let cap = marengo_config::resolve_joint_velocity_cap(PITCH, entry.motor_type, &control.control)
        .expect("velocity cap");
    let law = match entry.position_law {
        PositionLaw::ScaledPd => HoldLaw::ScaledPd(ScaledPdGains {
            kd: entry.impedance.kd,
            e0: entry.time_scale_e0_rad(),
            e1: entry.time_scale_e1_rad(),
            integral_band: entry.integral_band_rad(),
            integral_leak_s: entry.integral_leak_s(),
            friction: Some(ReferenceFriction::from_gains(&entry.friction, None)),
        }),
        PositionLaw::Legacy => HoldLaw::Legacy,
    };
    let params = HoldJointParams {
        kp: entry.impedance.kp,
        kd: entry.impedance.kd,
        ki: entry.impedance.ki,
        max_lead: entry.position_slew_max_lead_rad,
        vel_deadband: entry.position_trajectory_velocity_deadband_rad,
        advance_max_lead: entry.position_slew_max_lead_rad,
        advance_vel_deadband: entry.position_trajectory_velocity_deadband_rad,
        slew_rad_s: entry.position_slew_rad_s,
        trajectory_v_max: entry.position_trajectory_velocity_rad_s,
        trajectory_threshold_rad: entry.position_trajectory_threshold_rad,
        a_max: entry.position_trajectory_accel_rad_s2,
        velocity_cap: Some(cap),
        friction: Some(entry.friction.clone()),
        limit_policy: None,
        tau_meas: 0.0,
        law,
    };
    let urdf = gravity_model_from_urdf(repo().join("assets/urdf/marengo.urdf"), &joints)
        .expect("master URDF");
    let zero = vec![0.0; joints.len()];
    let terms = lumped_terms(&urdf, PITCH, &zero).expect("lumped pitch τ_g");
    let inertia = urdf.joint_inertia(PITCH, &zero).expect("pitch inertia");
    (params, (terms.a_nm, terms.b_nm), inertia)
}

/// The fitted plant and its variants: `(label, plant)`.
fn trial_plants(model: (f64, f64), inertia: f64) -> Vec<(String, PlantModel)> {
    let nominal = PlantModel {
        inertia,
        gravity_a: model.0,
        gravity_b: model.1,
        fs: FIT_FS,
        fc: FIT_FC,
        fv: 0.0,
    };
    let scaled = |s: f64| PlantModel {
        gravity_a: model.0 * s,
        gravity_b: model.1 * s,
        ..nominal
    };
    let heavy = inertia + FIT_DELTA_INERTIA + 2.0 * FIT_DELTA_INERTIA_SIGMA;
    vec![
        ("fitted".into(), nominal),
        (
            format!("fv {FIT_FV_SLOPE}"),
            PlantModel {
                fv: FIT_FV_SLOPE,
                ..nominal
            },
        ),
        (
            format!("inertia {heavy:.3}"),
            PlantModel {
                inertia: heavy,
                ..nominal
            },
        ),
        (
            format!("gravity×{}", 1.0 - FIT_GRAVITY_ERROR),
            scaled(1.0 - FIT_GRAVITY_ERROR),
        ),
        (
            format!("gravity×{}", 1.0 + FIT_GRAVITY_ERROR),
            scaled(1.0 + FIT_GRAVITY_ERROR),
        ),
    ]
}

fn strict_overshoot_bound(v_plan: f64) -> f64 {
    BENCH_OVERSHOOT * v_plan
}

/// The master pitch entry selects the scaled-PD law, and on the fitted plant (and its
/// variants) it meets the bench pass criteria: velocity overshoot ≤ 20 % of the planned speed
/// (Δq over 50 ms, as the bench scores it), ≤ 0.01 rad past every stop, no host τ_ff step above
/// 0.05 Nm in one tick (retargets included), no predicted total-torque clamp, no fault.
#[test]
fn pitch_bench_trial_meets_bench_criteria() {
    let (params, model, inertia) = master_pitch();
    assert_eq!(
        params.law.kind(),
        PositionLaw::ScaledPd,
        "pitch must select scaled_pd"
    );
    let results: Vec<(String, Metrics)> = trial_plants(model, inertia)
        .into_iter()
        .map(|(label, plant)| {
            let metrics = simulate(&Scenario {
                params: params.clone(),
                model,
                plant,
                moves: &TRIAL_MOVES,
                bound: strict_overshoot_bound,
            });
            (label, metrics)
        })
        .collect();
    let summary = results
        .iter()
        .map(|(label, m)| report(label, m))
        .collect::<Vec<_>>()
        .join("\n");
    for (_, m) in &results {
        assert!(m.fault.is_none(), "{summary}");
        assert!(m.max_tau_step_any <= BENCH_TAU_STEP_NM, "{summary}");
        assert!(
            Metrics::worst(&m.overshoot_windowed) <= BENCH_OVERSHOOT,
            "{summary}"
        );
        assert!(
            Metrics::worst(&m.end_overshoot) <= BENCH_END_OVERSHOOT_RAD,
            "{summary}"
        );
        assert_eq!(m.total_torque_clamps, 0, "{summary}");
        assert!(
            m.max_speed < params.velocity_cap.unwrap_or(VELOCITY_CAP) + 0.5,
            "{summary}"
        );
    }
}
