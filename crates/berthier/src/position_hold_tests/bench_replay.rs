// Bench replay: the 2026-10-04 pitch motion suite (`var/motion-suite/20261004T1306…1311Z`)
// through production `PositionHold`, the master pitch entry and URDF τ_g, against a plant fitted
// to those every-tick traces. Scored per move like `analyze-position-trace.py --score-bench`.
//
// Plant fit (bench traces; residual τ − τ_g(model) − J·q̈ against a 50 ms velocity):
// - gravity: the master model is within ±0.05 Nm in every 0.1 rad bin of −1.1…1.2 rad, so the
//   plant uses the model (and ±2σ_A variants);
// - friction falls with speed: ≈ 0.6 Nm at 0.03–0.09 rad/s, 0.5 at 0.1, 0.45 at 0.14 and
//   0.36–0.40 above 0.2 rad/s, with no viscous slope up to 1.5 rad/s;
// - inertia: the commanded MIT torque against the measured acceleration gives 0.13–0.14 kg·m²
//   (zone rows excluded); the URDF link alone is 0.074 (no rotor), and the reported τ_meas
//   gives 0.05–0.06. 0.14 reproduces the bench's stop overshoot (8–10 mrad) and the
//   repeatability moves' tracking; the URDF value gives 2 mrad stops;
// - a position-periodic torque ripple: the speed ripple of the 0.31 rad/s waves repeats at
//   the same q across waves (correlation 0.97) and peaks at 28.6 cycles/rad (20 per motor
//   turn at 9:1); 0.4 Nm with `fs` gives the 0.25–1.05 Nm breakaway spread the bench shows
//   (hold rests 0.005–0.028 rad off target, a creep at 1.1 Nm excess before a slip);
// - the drive damps on its own velocity estimate, quantized to one count per 5 ms (the
//   measured torque follows `kd·(v_des − dq_reported)` better than a smooth velocity);
// - Davout's `clamp_velocity` danger zones act on the MIT velocity from measured q and dq.

#![allow(clippy::expect_used)]

use std::collections::VecDeque;

use super::law_gates::{master_pitch, progress_threshold, rs03_count};
use super::*;
use crate::position_wave::PositionWave;

const DT: f64 = 0.005;
const HZ: u32 = 200;
const SUBSTEPS: usize = 20;
const TAU_LIMIT: f64 = 5.0;
const TAU_FF_RATE_NM_S: f64 = 60.0;
const TOTAL_TORQUE_HORIZON_TICKS: f64 = 2.0;
const TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S: f64 = 0.1;
const PITCH: &str = "right_shoulder_pitch";

/// Fitted friction: `fc + (fs − fc)·exp(−|v|/v_b)` against the motion; `fs` also breaks away.
const FIT_FC: f64 = 0.37;
const FIT_FS: f64 = 0.65;
const FIT_V_B: f64 = 0.08;
const FIT_INERTIA: f64 = 0.14;
const FIT_RIPPLE_NM: f64 = 0.4;
const FIT_RIPPLE_CYCLES_PER_RAD: f64 = 28.6;
/// Gravity model error ±2σ_A of the wave fit (σ_A 0.054 of A 2.661).
const FIT_GRAVITY_ERROR: f64 = 0.04;

/// Bench pass bar (`--score-bench`).
const SLOW_PLAN_RAD_S: f64 = 0.3;
const SHORT_MOVE_RAD: f64 = 0.1;
const SLOW_TRACK_MAX_RAD: f64 = 0.03;
const OVERSHOOT_MAX: f64 = 0.20;
const STOP_MAX_RAD: f64 = 0.01;
const TAU_STEP_MAX_NM: f64 = 0.05;
const MIN_PLAN_RAD_S: f64 = 0.05;
/// Speed window of the bench score: Δq over 50 ms.
const SPEED_WINDOW_TICKS: usize = 10;
const ENCODER_NOISE_COUNTS: f64 = 1.0;

#[derive(Clone, Copy, Debug)]
struct BenchPlant {
    inertia: f64,
    gravity_a: f64,
    gravity_b: f64,
    fc: f64,
    fs: f64,
    v_b: f64,
    ripple_nm: f64,
}

impl BenchPlant {
    /// Sliding friction magnitude: `fc + (fs − fc)·exp(−|v|/v_b)`.
    fn friction(&self, speed: f64) -> f64 {
        self.fc + (self.fs - self.fc) * (-speed / self.v_b).exp()
    }

    fn gravity(&self, q: f64) -> f64 {
        self.gravity_a * q.sin() + self.gravity_b * q.cos()
    }

    fn ripple(&self, q: f64) -> f64 {
        self.ripple_nm * (std::f64::consts::TAU * FIT_RIPPLE_CYCLES_PER_RAD * q).sin()
    }
}

/// Rigid joint with velocity-weakening stick-slip friction, driven by an MIT servo that damps on
/// its own quantized velocity estimate (count difference over one 5 ms window).
struct Plant {
    q: f64,
    dq: f64,
    model: BenchPlant,
    counts: VecDeque<f64>,
}

impl Plant {
    fn new(q: f64, model: BenchPlant) -> Self {
        let count = (q / rs03_count()).round();
        Self {
            q,
            dq: 0.0,
            model,
            counts: std::iter::repeat_n(count, SUBSTEPS + 1).collect(),
        }
    }

    fn step(&mut self, cmd: &DavoutMit) {
        let m = self.model;
        let h = DT / SUBSTEPS as f64;
        let grid = rs03_count();
        for _ in 0..SUBSTEPS {
            let v_est = (self.counts[SUBSTEPS] - self.counts[0]) * grid / DT;
            let motor = (cmd.kp * (cmd.position_rad - self.q)
                + cmd.kd * (cmd.velocity_rad_s - v_est)
                + cmd.torque_ff_nm)
                .clamp(-TAU_LIMIT, TAU_LIMIT);
            let drive = motor - m.gravity(self.q) - m.ripple(self.q);
            if self.dq == 0.0 {
                if drive.abs() > m.fs {
                    let accel = (drive - m.fs * drive.signum()) / m.inertia;
                    self.dq = accel * h;
                }
            } else {
                let accel = (drive - m.friction(self.dq.abs()) * self.dq.signum()) / m.inertia;
                let next = self.dq + accel * h;
                self.dq = if next * self.dq < 0.0 { 0.0 } else { next };
            }
            self.q += self.dq * h;
            self.counts.pop_front();
            self.counts.push_back((self.q / grid).round());
        }
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

/// One motion-suite step.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// `hold-at target` for `secs`.
    Move(f64, f64),
    /// `hold-at to`, retargeted to `back` once q is halfway, then `secs` of rest.
    Reverse { to: f64, back: f64, secs: f64 },
    /// Single-cycle raised-cosine `wave min max 1 half_period_s` (starts and ends at `min`).
    Wave(f64, f64, f64),
}

/// Waves of the suite (`plan.json`): `(min, max, half period s)`. The long-moves waves are each
/// preceded by a hold at `min`; their peak speeds are 0.16–1.10 rad/s.
const LONG_WAVES: [(f64, f64, f64); 9] = [
    (-0.279, 0.279, 5.42),
    (-0.279, 0.279, 2.71),
    (-0.279, 0.279, 1.51),
    (-0.559, 0.559, 7.68),
    (-0.559, 0.559, 3.84),
    (-0.559, 0.559, 2.14),
    (-1.007, 1.007, 10.3),
    (-1.007, 1.007, 5.15),
    (-1.007, 1.007, 2.87),
];
const SWEEP_WAVES: [(f64, f64, f64); 3] = [
    (-1.038, 1.2, 11.25),
    (-1.038, 1.2, 5.63),
    (-1.038, 1.2, 3.13),
];
const HOLD_SECS: f64 = 3.0;
const WAVE_SLACK_SECS: f64 = 0.5;

/// The suite sessions as `(name, steps)`, waves with their recorded half periods.
fn sessions() -> Vec<(&'static str, Vec<Step>)> {
    let mut long = vec![Step::Move(-0.279, HOLD_SECS)];
    let mut at = -0.279;
    for &(lo, hi, half) in &LONG_WAVES {
        if (at - lo).abs() > 1e-9 {
            long.push(Step::Move(lo, HOLD_SECS));
        }
        long.push(Step::Wave(lo, hi, half));
        long.push(Step::Move(lo, HOLD_SECS));
        at = lo;
    }
    long.push(Step::Move(0.0, HOLD_SECS));
    let mut sweeps = vec![Step::Move(-1.038, HOLD_SECS)];
    for &(lo, hi, half) in &SWEEP_WAVES {
        sweeps.push(Step::Wave(lo, hi, half));
        sweeps.push(Step::Move(lo, HOLD_SECS));
    }
    for a in [0.279, 0.559, 1.007] {
        sweeps.push(Step::Move(-a, HOLD_SECS));
        sweeps.push(Step::Reverse {
            to: a,
            back: -a,
            secs: HOLD_SECS,
        });
    }
    sweeps.push(Step::Move(0.0, HOLD_SECS));
    let mut short = vec![];
    for (base, dir) in [(-1.038, 1.0), (0.0, 1.0), (1.2, -1.0)] {
        short.push(Step::Move(base, HOLD_SECS));
        for d in [0.02, 0.05, 0.1] {
            short.push(Step::Move(base + dir * d, HOLD_SECS));
            short.push(Step::Move(base, HOLD_SECS));
        }
    }
    short.push(Step::Move(0.0, HOLD_SECS));
    let extremes = vec![
        Step::Move(-1.038, 8.0),
        Step::Move(1.2, 8.0),
        Step::Move(-1.038, HOLD_SECS),
        Step::Move(0.0, HOLD_SECS),
    ];
    let mut repeat = vec![];
    for _ in 0..5 {
        repeat.push(Step::Move(0.279, HOLD_SECS));
        repeat.push(Step::Move(0.0, HOLD_SECS));
    }
    vec![
        ("long_moves", long),
        ("sweeps", sweeps),
        ("short_moves", short),
        ("gravity_extremes", extremes),
        ("repeatability", repeat),
    ]
}

/// One scored move (a `hold-at` target or a whole wave), like `--score-bench`.
#[derive(Debug, Clone)]
struct MoveScore {
    label: String,
    slow: bool,
    plan: f64,
    tracking: f64,
    overshoot: Option<f64>,
    stop: f64,
    tau_step: f64,
    clamps: usize,
}

impl MoveScore {
    fn failures(&self) -> Vec<&'static str> {
        let mut out = vec![];
        if self.slow && self.tracking > SLOW_TRACK_MAX_RAD {
            out.push("tracking");
        }
        if !self.slow && self.overshoot.is_some_and(|o| o > OVERSHOOT_MAX) {
            out.push("speed");
        }
        if self.stop > STOP_MAX_RAD {
            out.push("stop");
        }
        if self.tau_step > TAU_STEP_MAX_NM {
            out.push("tau_step");
        }
        if self.clamps > 0 {
            out.push("clamp");
        }
        out
    }

    fn line(&self) -> String {
        let overshoot = self
            .overshoot
            .map_or_else(|| "-".into(), |o| format!("{:+.0}%", o * 100.0));
        format!(
            "{} plan {:.2} rad/s{} speed {} track {:.4} stop {:.4} τ step {:.3} clamps {} {:?}",
            self.label,
            self.plan,
            if self.slow { " (slow)" } else { "" },
            overshoot,
            self.tracking,
            self.stop,
            self.tau_step,
            self.clamps,
            self.failures()
        )
    }
}

/// Per-tick record of one move.
#[derive(Default)]
struct Rows {
    q: Vec<f64>,
    q_ref: Vec<f64>,
    dq_ref: Vec<f64>,
    tau_ff: Vec<f64>,
    clamps: usize,
}

fn windowed_speeds(x: &[f64]) -> Vec<f64> {
    x.windows(SPEED_WINDOW_TICKS + 1)
        .map(|w| (w[SPEED_WINDOW_TICKS] - w[0]) / (SPEED_WINDOW_TICKS as f64 * DT))
        .collect()
}

fn score(
    label: String,
    rows: &Rows,
    wave: Option<(f64, f64)>,
    start: f64,
    target: f64,
    prev_tau: Option<f64>,
) -> MoveScore {
    let tracking = rows
        .q
        .iter()
        .zip(&rows.q_ref)
        .map(|(q, r)| (q - r).abs())
        .fold(0.0, f64::max);
    let mut tau_step: f64 = 0.0;
    let mut last = prev_tau;
    for &t in &rows.tau_ff {
        if let Some(p) = last {
            tau_step = tau_step.max((t - p).abs());
        }
        last = Some(t);
    }
    let speeds = windowed_speeds(&rows.q);
    let (plan, overshoot, stop, slow) = if let Some((lo, hi)) = wave {
        let plan = windowed_speeds(&rows.q_ref)
            .iter()
            .map(|v| v.abs())
            .fold(0.0, f64::max);
        let peak = speeds.iter().map(|v| v.abs()).fold(0.0, f64::max);
        let q_max = rows.q.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let q_min = rows.q.iter().copied().fold(f64::INFINITY, f64::min);
        let stop = (q_max - hi).max(lo - q_min).max(0.0);
        (
            plan,
            Some((peak - plan) / plan),
            stop,
            plan <= SLOW_PLAN_RAD_S,
        )
    } else {
        let plan = rows.dq_ref.iter().map(|v| v.abs()).fold(0.0, f64::max);
        let dir = (target - start).signum();
        let peak = speeds
            .iter()
            .map(|v| v * dir)
            .fold(f64::NEG_INFINITY, f64::max);
        let stop = rows
            .q
            .iter()
            .map(|q| (q - target) * dir)
            .fold(0.0, f64::max);
        let slow = plan <= SLOW_PLAN_RAD_S || (target - start).abs() <= SHORT_MOVE_RAD + 1e-9;
        let overshoot = (plan >= MIN_PLAN_RAD_S).then(|| (peak - plan) / plan);
        (plan, overshoot, stop, slow)
    };
    MoveScore {
        label,
        slow,
        plan,
        tracking,
        overshoot,
        stop,
        tau_step,
        clamps: rows.clamps,
    }
}

/// Davout's `clamp_velocity` zones on the pitch, from master `control.yaml`.
fn pitch_velocity_zones() -> Vec<marengo_config::DangerZoneRule> {
    let config = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
    marengo_config::load_control_config_from(&config)
        .expect("control.yaml")
        .control
        .danger_zones
        .into_iter()
        .filter(|z| z.joint == PITCH && z.action == "clamp_velocity")
        .collect()
}

struct Sim<'a> {
    params: [HoldJointParams; 1],
    names: [String; 1],
    model: (f64, f64),
    zones: &'a [marengo_config::DangerZoneRule],
    hold: PositionHold,
    plant: Plant,
    rng: Rng,
    q_meas: f64,
    dq_meas: f64,
    tick: u64,
    tau_wire: f64,
    wave: Option<PositionWave>,
    fault: Option<String>,
}

impl Sim<'_> {
    fn sense(&mut self) -> f64 {
        let grid = rs03_count();
        ((self.plant.q + (self.rng.unit() - 0.5) * ENCODER_NOISE_COUNTS * grid) / grid).round()
            * grid
    }

    fn retarget(&mut self, target: f64) {
        self.hold.apply_retarget(HoldRetarget {
            joint_idx: 0,
            clamped: target,
            requested: target,
            q: self.q_meas,
            tick: self.tick,
            dq_seed: Some(self.dq_meas),
            downward_seed: None,
        });
    }

    /// One control tick; records into `rows`. Returns false on a fault.
    fn tick(&mut self, rows: &mut Rows) -> bool {
        self.tick += 1;
        let (a, b) = self.model;
        let tau_g = [a * self.q_meas.sin() + b * self.q_meas.cos()];
        let out = match self.hold.tick(HoldWorld {
            q: &[self.q_meas],
            dq_meas: &[self.dq_meas],
            tau_g: &tau_g,
            joints: &self.params,
            joint_names: &self.names,
            dt: DT,
            hz: HZ,
            tick_count: self.tick,
            wave: &mut self.wave,
        }) {
            Ok(out) => out,
            Err(error) => {
                self.fault = Some(error.to_string());
                return false;
            }
        };
        let mut cmd = out.mit[0].clone();
        let diag = &out.diag[0];
        rows.q.push(self.q_meas);
        rows.q_ref.push(diag.q_ref);
        rows.dq_ref.push(diag.dq_ref);
        rows.tau_ff.push(cmd.torque_ff_nm);
        for zone in self.zones {
            if self.q_meas > zone.position_above_rad && self.dq_meas < zone.velocity_below_rad_s {
                cmd.velocity_rad_s = cmd
                    .velocity_rad_s
                    .clamp(-zone.max_velocity_rad_s, zone.max_velocity_rad_s);
            }
        }
        let step = TAU_FF_RATE_NM_S * DT;
        let requested = cmd.torque_ff_nm.clamp(-TAU_LIMIT, TAU_LIMIT);
        self.tau_wire += (requested - self.tau_wire).clamp(-step, step);
        cmd.torque_ff_nm = self.tau_wire;
        let pd = cmd.kp * (cmd.position_rad - self.q_meas)
            + cmd.kd * (cmd.velocity_rad_s - self.dq_meas);
        let e_max = self.dq_meas.abs().max(cmd.velocity_rad_s.abs()) * TOTAL_TORQUE_HORIZON_TICKS
            / f64::from(HZ);
        let total = (pd + cmd.torque_ff_nm).abs()
            + cmd.kp * e_max
            + cmd.kd * TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S;
        if total > TAU_LIMIT {
            rows.clamps += 1;
        }
        self.plant.step(&cmd);
        let q_next = self.sense();
        self.dq_meas = (q_next - self.q_meas) / DT;
        self.q_meas = q_next;
        true
    }

    /// A session armed at rest at q = 0.
    fn armed<'a>(
        params: &HoldJointParams,
        model: (f64, f64),
        plant: BenchPlant,
        zones: &'a [marengo_config::DangerZoneRule],
    ) -> Sim<'a> {
        let mut sim = Sim {
            params: [params.clone()],
            names: [String::from(PITCH)],
            model,
            zones,
            hold: PositionHold::with_progress_thresholds(vec![progress_threshold()]),
            plant: Plant::new(0.0, plant),
            rng: Rng(0x2545_f491_4f6c_dd1d),
            q_meas: 0.0,
            dq_meas: 0.0,
            tick: 0,
            tau_wire: 0.0,
            wave: None,
            fault: None,
        };
        sim.q_meas = sim.sense();
        sim.hold.set_joint_law(0, params.law.kind());
        sim.hold.arm(&[sim.q_meas], &[sim.q_meas], 0);
        sim
    }
}

/// Run `steps` from rest at 0 and score every move.
fn replay(
    session: &str,
    steps: &[Step],
    params: &HoldJointParams,
    model: (f64, f64),
    plant: BenchPlant,
    zones: &[marengo_config::DangerZoneRule],
) -> (Vec<MoveScore>, Option<String>) {
    let mut sim = Sim::armed(params, model, plant, zones);
    let ticks = |secs: f64| (secs * f64::from(HZ)).round() as usize;
    let mut scores = vec![];
    let mut commanded = 0.0;
    let mut prev_tau = None;
    for (n, step) in steps.iter().enumerate() {
        let label = |what: String| format!("{session} #{n} {what}");
        match *step {
            Step::Move(target, secs) => {
                let start = commanded;
                sim.retarget(target);
                let mut rows = Rows::default();
                for _ in 0..ticks(secs) {
                    if !sim.tick(&mut rows) {
                        return (scores, sim.fault);
                    }
                }
                let s = score(
                    label(format!("move {start:+.3}→{target:+.3}")),
                    &rows,
                    None,
                    start,
                    target,
                    prev_tau,
                );
                prev_tau = rows.tau_ff.last().copied();
                if (target - start).abs() > 1e-3 {
                    scores.push(s);
                }
                commanded = target;
            }
            Step::Reverse { to, back, secs } => {
                let start = commanded;
                sim.retarget(to);
                let mut rows = Rows::default();
                let half = 0.5 * (start + to);
                while (sim.q_meas - half) * (to - start).signum() < 0.0 {
                    if !sim.tick(&mut rows) {
                        return (scores, sim.fault);
                    }
                }
                scores.push(score(
                    label(format!("move {start:+.3}→{to:+.3}")),
                    &rows,
                    None,
                    start,
                    to,
                    prev_tau,
                ));
                prev_tau = rows.tau_ff.last().copied();
                sim.retarget(back);
                let mut rows = Rows::default();
                for _ in 0..ticks(secs) {
                    if !sim.tick(&mut rows) {
                        return (scores, sim.fault);
                    }
                }
                scores.push(score(
                    label(format!("move {to:+.3}→{back:+.3} (reversal)")),
                    &rows,
                    None,
                    to,
                    back,
                    prev_tau,
                ));
                prev_tau = rows.tau_ff.last().copied();
                commanded = back;
            }
            Step::Wave(lo, hi, recorded_half_s) => {
                // The suite now plans each wave within the descent cap marengo-pi admits
                // (`validate_wave_motion`): the recorded half period, or the zone's minimum.
                let half_s = params.descent_cap.map_or(recorded_half_s, |cap| {
                    let factor = cap.wave_descent_speed(lo, hi, 1.0);
                    let min_half =
                        std::f64::consts::PI * 0.5 * (hi - lo) * factor / cap.max_velocity_rad_s;
                    recorded_half_s.max((min_half * 100.0).ceil() / 100.0)
                });
                let half_ticks = (half_s * f64::from(HZ)).round() as u64;
                sim.wave = Some(PositionWave::new(0, lo, hi, sim.tick, half_ticks, 1));
                let mut rows = Rows::default();
                for _ in 0..(2 * half_ticks as usize) {
                    if !sim.tick(&mut rows) {
                        return (scores, sim.fault);
                    }
                }
                let s = score(
                    label(format!("wave [{lo:+.3}, {hi:+.3}] T {half_s} s")),
                    &rows,
                    Some((lo, hi)),
                    lo,
                    lo,
                    prev_tau,
                );
                prev_tau = rows.tau_ff.last().copied();
                scores.push(s);
                sim.wave = None;
                let mut rows = Rows::default();
                for _ in 0..ticks(WAVE_SLACK_SECS) {
                    if !sim.tick(&mut rows) {
                        return (scores, sim.fault);
                    }
                }
                prev_tau = rows.tau_ff.last().copied().or(prev_tau);
                commanded = lo;
            }
        }
    }
    (scores, sim.fault)
}

fn fitted_plants(model: (f64, f64)) -> Vec<(String, BenchPlant)> {
    let nominal = BenchPlant {
        inertia: FIT_INERTIA,
        gravity_a: model.0,
        gravity_b: model.1,
        fc: FIT_FC,
        fs: FIT_FS,
        v_b: FIT_V_B,
        ripple_nm: FIT_RIPPLE_NM,
    };
    let scaled = |s: f64| BenchPlant {
        gravity_a: model.0 * s,
        gravity_b: model.1 * s,
        ..nominal
    };
    vec![
        ("fitted".into(), nominal),
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

/// The master pitch entry replays the bench suite on the fitted plant and meets the bench bar
/// on every move: slow track ≤ 0.03 rad, fast speed overshoot ≤ 20 %, stop ≤ 0.01 rad, τ_ff
/// step ≤ 0.05 Nm per tick, no predicted total-torque clamp, no fault. The ±2σ_A gravity
/// variants must meet the same bar except the stop: a 4 % model error moves the rest point by
/// ΔA·sin q/kp ≈ 5 mrad at the gravity extremes, which the static band then keeps (their stop
/// is printed, not gated).
#[test]
fn pitch_bench_suite_replay_meets_bench_bar() {
    let (params, model, _) = master_pitch();
    let zones = pitch_velocity_zones();
    let mut report = vec![];
    let mut failed = vec![];
    for (n, (plant_label, plant)) in fitted_plants(model).into_iter().enumerate() {
        for (session, steps) in sessions() {
            let (scores, fault) = replay(session, &steps, &params, model, plant, &zones);
            if let Some(fault) = fault {
                failed.push(format!("{plant_label} {session}: fault {fault}"));
            }
            for s in scores {
                let line = format!("{plant_label} {}", s.line());
                let gated = s.failures().into_iter().any(|f| n == 0 || f != "stop");
                if gated {
                    failed.push(line.clone());
                }
                report.push(line);
            }
        }
    }
    let report = report.join("\n");
    assert!(
        failed.is_empty(),
        "failures:\n{}\n\nall moves:\n{report}",
        failed.join("\n")
    );
}

/// The 2026-10-04 bench (`var/motion-suite/20261004T202431Z`, `…T202646Z`): at the turnarounds
/// of the slowest waves the reference velocity, and with it the reference friction feed-forward,
/// fades to zero; the joint stuck 40–47 mrad short for 1.8–4.5 s (track 0.043 and 0.047 rad
/// against 0.03) because `kp·e` alone had to beat breakaway. The plant here sits on a ripple
/// crest: static breakaway `fs + ripple` (1.05 Nm) with no ripple term, the bench's worst case.
#[test]
fn slow_wave_turnarounds_break_away_within_the_tracking_bar() {
    let (params, model, _) = master_pitch();
    let zones = pitch_velocity_zones();
    let crest = BenchPlant {
        inertia: FIT_INERTIA,
        gravity_a: model.0,
        gravity_b: model.1,
        fc: FIT_FC,
        fs: FIT_FS + FIT_RIPPLE_NM,
        v_b: FIT_V_B,
        ripple_nm: 0.0,
    };
    let steps = [
        Step::Move(-0.992, HOLD_SECS),
        Step::Wave(-0.992, 0.992, 23.93),
        Step::Move(-1.038, HOLD_SECS),
        Step::Wave(-1.038, 2.533, 49.87),
    ];
    let (scores, fault) = replay("slow_waves", &steps, &params, model, crest, &zones);
    assert_eq!(fault, None);
    let lines: Vec<String> = scores.iter().map(MoveScore::line).collect();
    let failed: Vec<&String> = scores
        .iter()
        .zip(&lines)
        .filter(|(s, _)| !s.failures().is_empty())
        .map(|(_, l)| l)
        .collect();
    assert!(
        failed.is_empty(),
        "failures:\n{failed:#?}\n\nall:\n{}",
        lines.join("\n")
    );
}

/// Every tick of a descent session: `(q_ref, dq_ref, measured q)`.
fn descent_rows(
    params: &HoldJointParams,
    model: (f64, f64),
) -> (Vec<(f64, f64, f64)>, Option<String>) {
    let zones = pitch_velocity_zones();
    let plant = fitted_plants(model)[0].1;
    let mut sim = Sim::armed(params, model, plant, &zones);
    let mut rows = Rows::default();
    let mut run = |sim: &mut Sim, target: f64, secs: f64, until: Option<f64>| {
        sim.retarget(target);
        for _ in 0..(secs * f64::from(HZ)) as usize {
            if !sim.tick(&mut rows) {
                return false;
            }
            if until.is_some_and(|q| sim.q_meas < q) {
                break;
            }
        }
        true
    };
    // Up to the top of the window, down to rest, up again and down past rest to the other
    // extreme, and a descent retargeted upward mid-zone and then down again.
    let ok = run(&mut sim, 1.2, 3.0, None)
        && run(&mut sim, 0.0, 4.0, None)
        && run(&mut sim, 1.2, 3.0, None)
        && run(&mut sim, -1.0, 6.0, None)
        && run(&mut sim, 1.2, 4.0, None)
        && run(&mut sim, -1.0, 4.0, Some(0.8))
        && run(&mut sim, 1.1, 2.0, None)
        && run(&mut sim, 0.0, 4.0, None);
    let fault = (!ok).then(|| sim.fault.clone().unwrap_or_default());
    let out = rows
        .q_ref
        .iter()
        .zip(&rows.dq_ref)
        .zip(&rows.q)
        .map(|((r, v), q)| (*r, *v, *q))
        .collect();
    (out, fault)
}

/// Ticks where the reference descends faster than the danger zone allows while the reference
/// or the measured joint is above its threshold.
fn zone_violations(rows: &[(f64, f64, f64)], cap: DescentCap) -> usize {
    rows.iter()
        .filter(|(q_ref, dq_ref, q)| {
            *dq_ref < 0.0
                && q_ref.max(*q) > cap.above_rad
                && -dq_ref > cap.max_velocity_rad_s + 1e-9
        })
        .count()
}

/// The pitch reference never descends faster than its `clamp_velocity` danger zone while the
/// reference or the measured joint is above the zone's threshold, and enters and leaves the cap
/// within `a_max·dt` per tick. Without the cap the same session violates it.
#[test]
fn descending_reference_respects_the_danger_zone_cap() {
    let (params, model, _) = master_pitch();
    let cap = params.descent_cap.expect("master pitch descent zone");
    let (rows, fault) = descent_rows(&params, model);
    assert!(fault.is_none(), "{fault:?}");
    assert_eq!(zone_violations(&rows, cap), 0);
    let dv = params.a_max * DT + 1e-9;
    for (n, pair) in rows.windows(2).enumerate() {
        assert!(
            (pair[1].1 - pair[0].1).abs() <= dv,
            "tick {n}: dq_ref {} → {}",
            pair[0].1,
            pair[1].1
        );
    }
    let uncapped = HoldJointParams {
        descent_cap: None,
        ..params
    };
    let (rows, _) = descent_rows(&uncapped, model);
    assert!(
        zone_violations(&rows, cap) > 0,
        "the session must exercise the cap"
    );
}
