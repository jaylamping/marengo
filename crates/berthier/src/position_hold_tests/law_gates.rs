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
// Each gate must fail on the legacy law and pass on the scaled-PD law.

use super::*;
use crate::position_law::{ReferenceFriction, ScaledPdGains};
use crate::position_setpoint::downward_return_seed_velocity;

const DT: f64 = 0.005;
const HZ: u32 = 200;
const SUBSTEPS: usize = 20;

// Master pitch tuning (config/control.yaml); these are inputs, not changes to it.
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

// Plant.
const INERTIA: f64 = 0.12;
const GRAVITY_NM: f64 = 2.7;
const PLANT_FS: f64 = 0.14;
const PLANT_FC: f64 = 0.08;
const PLANT_FV: f64 = 0.02;

/// ADR 0039 gate: no host τ_ff step larger than this per tick outside retarget ticks (Nm).
const TAU_STEP_GATE_NM: f64 = 0.05;
/// ADR 0039 gate: measured velocity overshoot below this fraction of the planned peak.
const OVERSHOOT_GATE: f64 = 0.20;

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

/// Rigid joint with stick-slip friction, driven by an MIT servo.
struct Plant {
    q: f64,
    dq: f64,
    gravity_scale: f64,
}

impl Plant {
    fn step(&mut self, cmd: &DavoutMit) {
        let h = DT / SUBSTEPS as f64;
        for _ in 0..SUBSTEPS {
            let motor = (cmd.kp * (cmd.position_rad - self.q)
                + cmd.kd * (cmd.velocity_rad_s - self.dq)
                + cmd.torque_ff_nm)
                .clamp(-TAU_LIMIT, TAU_LIMIT);
            let drive = motor - self.gravity_scale * GRAVITY_NM * self.q.sin();
            if self.dq == 0.0 {
                if drive.abs() <= PLANT_FS {
                    continue;
                }
                let accel = (drive - PLANT_FC * drive.signum()) / INERTIA;
                self.dq = accel * h;
            } else {
                let accel = (drive - PLANT_FC * self.dq.signum() - PLANT_FV * self.dq) / INERTIA;
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

#[derive(Debug, Default)]
struct Metrics {
    /// Largest host τ_ff change in one tick, outside retarget ticks (Nm).
    max_tau_step: f64,
    /// Ticks with a full-rate moving reference, and those with measured dq exactly 0.
    moving_rows: usize,
    stuck_rows: usize,
    /// Per move: largest true speed above the planned peak, as a fraction of the plan.
    overshoot: Vec<f64>,
    /// Largest speed excess over [`overshoot_bound`] (≤ 1 passes).
    overshoot_vs_bound: f64,
    /// Largest true speed (rad/s).
    max_speed: f64,
    /// Wire kd / v_des toggles and planner phase changes while the joint rests at its target.
    rest_toggles: usize,
    /// Fuse trip, if the law faulted.
    fault: Option<String>,
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

fn simulate(law: Law, gravity_scale: f64) -> Metrics {
    let moves = MOVES;
    let mut hold = PositionHold::with_progress_thresholds(vec![progress_threshold()]);
    let mut plant = Plant {
        q: 0.0,
        dq: 0.0,
        gravity_scale,
    };
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let grid = rs03_count();
    let sense = |q: f64, rng: &mut Rng| {
        ((q + (rng.unit() - 0.5) * NOISE_COUNTS * grid) / grid).round() * grid
    };
    let mut q_meas = sense(plant.q, &mut rng);
    let mut dq_meas = 0.0;
    let params = [params(law)];
    let names = [String::from("right_shoulder_pitch")];
    let mut wave = None;
    let mut metrics = Metrics::default();
    let mut tick: u64 = 0;
    let mut tau_wire = 0.0;
    let mut last_tau: Option<f64> = None;
    let mut last_wire: Option<(f64, f64, &'static str)> = None;
    hold.set_joint_law(0, params[0].law.kind());
    hold.arm(&[q_meas], &[q_meas], 0);
    for (move_index, (target, seconds)) in moves.into_iter().enumerate() {
        let start = q_meas;
        let v_plan = {
            let distance = (target - start).abs();
            let v = if distance <= THRESHOLD { SLEW } else { V_TRAJ };
            v.min((A_MAX * distance).sqrt()).min(VELOCITY_CAP)
        };
        metrics.overshoot.push(f64::NEG_INFINITY);
        let seed = downward_return_seed_velocity(SLEW, V_TRAJ, q_meas, target);
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
            let tau_g = [GRAVITY_NM * q_meas.sin()];
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
                if tick > retarget_tick + 1 {
                    metrics.max_tau_step = metrics
                        .max_tau_step
                        .max((cmd.torque_ff_nm - previous).abs());
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
            plant.step(&cmd);
            let speed = plant.dq.abs();
            metrics.max_speed = metrics.max_speed.max(speed);
            metrics.overshoot[move_index] =
                metrics.overshoot[move_index].max((speed - v_plan) / v_plan);
            metrics.overshoot_vs_bound = metrics
                .overshoot_vs_bound
                .max((speed - v_plan) / overshoot_bound(v_plan));
            // One tick of transport delay: the next tick sees this tick's end state.
            let q_next = sense(plant.q, &mut rng);
            dq_meas = (q_next - q_meas) / DT;
            q_meas = q_next;
        }
    }
    metrics
}

fn report(law: Law, scale: f64, metrics: &Metrics) -> String {
    let overshoot: Vec<i64> = metrics
        .overshoot
        .iter()
        .map(|v| (v * 100.0).round() as i64)
        .collect();
    format!(
        "{law:?} gravity×{scale}: max τ_ff step {:.3} Nm, overshoot per move {overshoot:?} %, \
         excess/bound {:.2}, max speed {:.2} rad/s, rest toggles {}, stuck rows {}/{}, fault {:?}",
        metrics.max_tau_step,
        metrics.overshoot_vs_bound,
        metrics.max_speed,
        metrics.rest_toggles,
        metrics.stuck_rows,
        metrics.moving_rows,
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
                simulate(Law::Legacy, scale),
                simulate(Law::ScaledPd, scale),
            )
        })
        .collect();
    let summary = runs
        .iter()
        .flat_map(|(scale, legacy, scaled)| {
            [
                report(Law::Legacy, *scale, legacy),
                report(Law::ScaledPd, *scale, scaled),
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
