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
// - the drive damps on its own velocity estimate, modelled by default as one count per 5 ms.
//   That estimate is a hypothesis, not a measurement: the 2026-10-04 control audit
//   (`docs/reviews/2026-10-04-control-audit-pitch-slow-speed.md`, "Test of the drive-damping
//   hypothesis") found the measured torque follows a 50 ms-smooth velocity better than the
//   host's one-count difference, so the richer replay below also runs a 20 ms count
//   difference and the exact velocity;
// - Davout's `clamp_velocity` danger zones act on the MIT velocity from measured q and dq.
//
// The richer replay (`Fidelity::audit`, the kd 5 vs kd 6 qualification at the end of this file)
// adds what the audit's final replay added: the drive-estimator hypotheses, extra old feedback
// samples (transport delay), a ripple phase, Davout's common-factor total-torque reshape and
// the reshaped / last-sent τ_ff it reports back to the law, and the RS03 MIT wire quantization
// through `robstride::encode_mit`. `Fidelity::BASE` keeps the original replay exactly.

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
/// Chatter / limit-cycle bar: the measured position range over the final second of a hold whose
/// reference is stationary. ADR 0040's bench bar for final-second drift (5 mrad).
const HOLD_EXCURSION_MAX_RAD: f64 = 0.005;
/// Repeatability bar: spread of the final positions at one target (ADR 0040 bench, 10 mrad).
const REPEAT_SPREAD_MAX_RAD: f64 = 0.01;
/// Reference-speed band of the smoothness metric `V` (audit "Metrics").
const SMOOTH_BAND_RAD_S: (f64, f64) = (0.05, 0.4);
/// Pitch motor direction (`config/motors.yaml`: `direction: -1`, `gear_ratio: 1.0`). Davout
/// sends `q_m = −q`, `v_m = −v`, `τ_m = −τ` with unchanged gains.
const PITCH_DIRECTION: f64 = -1.0;

#[derive(Clone, Copy, Debug)]
struct BenchPlant {
    inertia: f64,
    gravity_a: f64,
    gravity_b: f64,
    fc: f64,
    fs: f64,
    v_b: f64,
    ripple_nm: f64,
    /// Ripple phase at q = 0 (rad); the audit's uncertainty grid sweeps 0, π/2, π.
    ripple_phase: f64,
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
        self.ripple_nm
            * (std::f64::consts::TAU * FIT_RIPPLE_CYCLES_PER_RAD * q + self.ripple_phase).sin()
    }
}

/// The drive's own velocity estimate for its `kd` term (audit: "The three drive estimators").
/// None is claimed to be the measured firmware behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DriveEstimator {
    /// Encoder-count difference over 5 ms (one host tick).
    Count5ms,
    /// Encoder-count difference over 20 ms.
    Count20ms,
    /// The exact plant velocity.
    Ideal,
}

impl DriveEstimator {
    const ALL: [Self; 3] = [Self::Count5ms, Self::Count20ms, Self::Ideal];

    /// Substeps in the count-difference window.
    fn window(self) -> usize {
        match self {
            Self::Count5ms | Self::Ideal => SUBSTEPS,
            Self::Count20ms => 4 * SUBSTEPS,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Count5ms => "5 ms",
            Self::Count20ms => "20 ms",
            Self::Ideal => "ideal",
        }
    }
}

/// Rigid joint with velocity-weakening stick-slip friction, driven by an MIT servo that damps on
/// its own velocity estimate ([`DriveEstimator`]).
struct Plant {
    q: f64,
    dq: f64,
    model: BenchPlant,
    estimator: DriveEstimator,
    counts: VecDeque<f64>,
}

impl Plant {
    fn new(q: f64, model: BenchPlant, estimator: DriveEstimator) -> Self {
        let count = (q / rs03_count()).round();
        Self {
            q,
            dq: 0.0,
            model,
            estimator,
            counts: std::iter::repeat_n(count, estimator.window() + 1).collect(),
        }
    }

    fn step(&mut self, cmd: &DavoutMit) {
        let m = self.model;
        let h = DT / SUBSTEPS as f64;
        let grid = rs03_count();
        let window = self.estimator.window();
        for _ in 0..SUBSTEPS {
            let v_est = match self.estimator {
                DriveEstimator::Ideal => self.dq,
                // `window / SUBSTEPS · DT` is exactly DT for the 5 ms window.
                _ => {
                    (self.counts[window] - self.counts[0]) * grid
                        / (window as f64 / SUBSTEPS as f64 * DT)
                }
            };
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
    /// Final-second measured position range of a hold with a stationary reference.
    hold: Option<f64>,
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
        if self.hold.is_some_and(|h| h > HOLD_EXCURSION_MAX_RAD) {
            out.push("hold");
        }
        out
    }

    fn line(&self) -> String {
        let overshoot = self
            .overshoot
            .map_or_else(|| "-".into(), |o| format!("{:+.0}%", o * 100.0));
        format!(
            "{} plan {:.2} rad/s{} speed {} track {:.4} stop {:.4} τ step {:.3} clamps {} hold {} \
             {:?}",
            self.label,
            self.plan,
            if self.slow { " (slow)" } else { "" },
            overshoot,
            self.tracking,
            self.stop,
            self.tau_step,
            self.clamps,
            self.hold.map_or_else(|| "-".into(), |h| format!("{h:.4}")),
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
    /// τ_ff as decoded from the MIT wire frame (richer replay only; empty otherwise).
    tau_wire: Vec<f64>,
    clamps: usize,
}

impl Rows {
    /// The last commanded and wire τ_ff, the `prev` of the next move's step score.
    fn last(&self) -> Prev {
        Prev {
            tau: self.tau_ff.last().copied(),
            wire: self.tau_wire.last().copied(),
        }
    }
}

/// The previous move's last commanded and wire τ_ff.
#[derive(Clone, Copy, Default)]
struct Prev {
    tau: Option<f64>,
    wire: Option<f64>,
}

impl Prev {
    fn or(self, other: Self) -> Self {
        Self {
            tau: self.tau.or(other.tau),
            wire: self.wire.or(other.wire),
        }
    }
}

/// Largest change between consecutive values, starting from `prev`.
fn max_step(values: &[f64], prev: Option<f64>) -> f64 {
    let mut step: f64 = 0.0;
    let mut last = prev;
    for &t in values {
        if let Some(p) = last {
            step = step.max((t - p).abs());
        }
        last = Some(t);
    }
    step
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
    prev: Prev,
) -> MoveScore {
    let tracking = rows
        .q
        .iter()
        .zip(&rows.q_ref)
        .map(|(q, r)| (q - r).abs())
        .fold(0.0, f64::max);
    // The larger of the law's step and the decoded wire's (audit "Metrics").
    let tau_step = max_step(&rows.tau_ff, prev.tau).max(max_step(&rows.tau_wire, prev.wire));
    let last_second = rows.q.len().saturating_sub(HZ as usize);
    let hold = (wave.is_none()
        && rows.q.len() >= HZ as usize
        && rows.dq_ref[last_second..].iter().all(|v| *v == 0.0)
        && rows.q_ref[last_second..]
            .iter()
            .all(|r| *r == rows.q_ref[last_second]))
    .then(|| {
        let tail = &rows.q[last_second..];
        let hi = tail.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lo = tail.iter().copied().fold(f64::INFINITY, f64::min);
        hi - lo
    });
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
        hold,
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

/// What the replay models beyond the original fitted-plant replay (audit "Experiments").
#[derive(Clone, Copy, Debug)]
struct Fidelity {
    estimator: DriveEstimator,
    /// Feedback samples older than the base one-tick delay (the audit's "extra old samples").
    extra_delay_ticks: usize,
    /// Davout's common-factor total-torque reshape, the reshape and last-sent τ_ff it reports
    /// back to the law (as `ControlLoop` does), and the RS03 MIT wire quantization.
    davout_and_wire: bool,
}

impl Fidelity {
    /// The original replay: 5 ms count estimator, one tick of delay, no reshape, no wire.
    const BASE: Self = Self {
        estimator: DriveEstimator::Count5ms,
        extra_delay_ticks: 0,
        davout_and_wire: false,
    };

    fn audit(estimator: DriveEstimator, extra_delay_ticks: usize) -> Self {
        Self {
            estimator,
            extra_delay_ticks,
            davout_and_wire: true,
        }
    }
}

/// Davout's common-factor reshape (`davout/src/total_torque.rs` `clamp_total_torque`): scale the
/// position and velocity errors toward feedback until the predicted total torque fits the cap.
fn reshape_total_torque(cmd: &mut DavoutMit, q: f64, dq: f64) {
    let e = cmd.position_rad - q;
    let e_dot = cmd.velocity_rad_s - dq;
    let pd = cmd.kp * e + cmd.kd * e_dot;
    let e_max = dq.abs().max(cmd.velocity_rad_s.abs()) * TOTAL_TORQUE_HORIZON_TICKS / f64::from(HZ);
    let margin = cmd.kp * e_max + cmd.kd * TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S;
    let upper = TAU_LIMIT - margin - cmd.torque_ff_nm;
    let lower = -TAU_LIMIT + margin - cmd.torque_ff_nm;
    let target = if lower <= upper {
        pd.clamp(lower, upper)
    } else {
        -cmd.torque_ff_nm
    };
    let reachable = target.clamp(pd.min(0.0), pd.max(0.0));
    if reachable == pd {
        return;
    }
    let scale = reachable / pd;
    cmd.position_rad = q + scale * e;
    cmd.velocity_rad_s = dq + scale * e_dot;
}

/// The joint-space command the drive decodes: Davout's motor transform, `robstride::encode_mit`
/// (f32 and RS03 field quantization) and the field decode, back to joint space.
fn through_wire(cmd: &DavoutMit) -> DavoutMit {
    let motor_type = marengo_config::MotorType::Rs03;
    let frame = robstride::MitCommand {
        device_id: 1,
        motor_type,
        position_rad: (PITCH_DIRECTION * cmd.position_rad) as f32,
        velocity_rad_s: (PITCH_DIRECTION * cmd.velocity_rad_s) as f32,
        kp: cmd.kp as f32,
        kd: cmd.kd as f32,
        torque_ff_nm: (PITCH_DIRECTION * cmd.torque_ff_nm) as f32,
    };
    let (id, data) = robstride::encode_mit(&frame).expect("pitch MIT command in range");
    let fields = robstride::decode_mit_command_fields(id, &data).expect("type-1 MIT frame");
    let ranges = robstride::motor_type::MitRanges::for_motor_type(motor_type);
    let signed = |raw: u16, scale: f32| f64::from((f32::from(raw) / 32767.0 - 1.0) * scale);
    let gain = |raw: u16, scale: f32| f64::from(f32::from(raw) / 65535.0 * scale);
    DavoutMit {
        joint: cmd.joint.clone(),
        kp: gain(fields.kp, ranges.kp_scale),
        kd: gain(fields.kd, ranges.kd_scale),
        position_rad: PITCH_DIRECTION * signed(fields.position, ranges.position_scale),
        velocity_rad_s: PITCH_DIRECTION * signed(fields.velocity, ranges.velocity_scale),
        torque_ff_nm: PITCH_DIRECTION * signed(fields.torque_ff, ranges.torque_scale),
    }
}

struct Sim<'a> {
    params: [HoldJointParams; 1],
    names: [String; 1],
    model: (f64, f64),
    zones: &'a [marengo_config::DangerZoneRule],
    hold: PositionHold,
    plant: Plant,
    rng: Rng,
    fidelity: Fidelity,
    /// Sensed positions, newest last; Berthier sees the oldest (the transport delay).
    feedback: VecDeque<f64>,
    q_meas: f64,
    dq_meas: f64,
    tick: u64,
    tau_wire: f64,
    /// Davout reshaped the previous command (richer replay).
    reshaped: bool,
    /// The τ_ff Davout last sent (richer replay).
    ff_sent: Option<f64>,
    wave: Option<PositionWave>,
    fault: Option<String>,
    /// Smoothness `V` accumulator: Σ(true dq − dq_ref)² and rows in the band.
    v_sq: f64,
    v_rows: usize,
    max_total: f64,
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
            speed: SpeedFraction::FULL,
        });
    }

    /// One control tick; records into `rows`. Returns false on a fault.
    fn tick(&mut self, rows: &mut Rows) -> bool {
        self.tick += 1;
        let (a, b) = self.model;
        let tau_g = [a * self.q_meas.sin() + b * self.q_meas.cos()];
        let rich = self.fidelity.davout_and_wire;
        let reshaped = [self.reshaped];
        let ff_last_sent = [self.ff_sent];
        let out = match self.hold.tick(HoldWorld {
            q: &[self.q_meas],
            dq_meas: &[self.dq_meas],
            tau_g: &tau_g,
            joints: &self.params,
            joint_names: &self.names,
            dt: DT,
            hz: HZ,
            tick_count: self.tick,
            reshaped: if rich { &reshaped } else { &[] },
            ff_last_sent: if rich { &ff_last_sent } else { &[] },
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
        let commanded_ff = cmd.torque_ff_nm;
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
        self.max_total = self.max_total.max(total);
        let clamped = total > TAU_LIMIT;
        if clamped {
            rows.clamps += 1;
        }
        if rich {
            if clamped {
                reshape_total_torque(&mut cmd, self.q_meas, self.dq_meas);
            }
            self.reshaped = clamped || (commanded_ff - self.tau_wire).abs() > 1e-6;
            self.ff_sent = Some(self.tau_wire);
            cmd = through_wire(&cmd);
            rows.tau_wire.push(cmd.torque_ff_nm);
        }
        self.plant.step(&cmd);
        let band = diag.dq_ref.abs();
        if (SMOOTH_BAND_RAD_S.0..=SMOOTH_BAND_RAD_S.1).contains(&band) {
            self.v_sq += (self.plant.dq - diag.dq_ref).powi(2);
            self.v_rows += 1;
        }
        let sample = self.sense();
        self.feedback.push_back(sample);
        while self.feedback.len() > self.fidelity.extra_delay_ticks + 1 {
            self.feedback.pop_front();
        }
        // Davout's velocity is the difference of consecutive (delayed) samples, so the delay
        // never makes a multi-tick derivative.
        let q_next = self.feedback[0];
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
        fidelity: Fidelity,
    ) -> Sim<'a> {
        let mut sim = Sim {
            params: [params.clone()],
            names: [String::from(PITCH)],
            model,
            zones,
            hold: PositionHold::with_progress_thresholds(vec![progress_threshold()]),
            plant: Plant::new(0.0, plant, fidelity.estimator),
            rng: Rng(0x2545_f491_4f6c_dd1d),
            fidelity,
            feedback: VecDeque::new(),
            q_meas: 0.0,
            dq_meas: 0.0,
            tick: 0,
            tau_wire: 0.0,
            reshaped: false,
            ff_sent: None,
            wave: None,
            fault: None,
            v_sq: 0.0,
            v_rows: 0,
            max_total: 0.0,
        };
        sim.q_meas = sim.sense();
        sim.feedback = std::iter::repeat_n(sim.q_meas, fidelity.extra_delay_ticks + 1).collect();
        sim.hold.set_joint_law(0, params.law.kind());
        sim.hold.arm(&[sim.q_meas], &[sim.q_meas], 0);
        sim
    }
}

/// One replayed session.
struct Replay {
    scores: Vec<MoveScore>,
    fault: Option<String>,
    /// Every `hold-at` move: `(target, final measured q)`.
    finals: Vec<(f64, f64)>,
    v_sq: f64,
    v_rows: usize,
    max_total: f64,
}

/// Run `steps` from rest at 0 and score every move.
fn replay(
    session: &str,
    steps: &[Step],
    params: &HoldJointParams,
    model: (f64, f64),
    plant: BenchPlant,
    zones: &[marengo_config::DangerZoneRule],
    fidelity: Fidelity,
) -> Replay {
    let mut sim = Sim::armed(params, model, plant, zones, fidelity);
    let mut finals = vec![];
    let scores = replay_steps(&mut sim, session, steps, &mut finals);
    Replay {
        scores,
        fault: sim.fault,
        finals,
        v_sq: sim.v_sq,
        v_rows: sim.v_rows,
        max_total: sim.max_total,
    }
}

fn replay_steps(
    sim: &mut Sim,
    session: &str,
    steps: &[Step],
    finals: &mut Vec<(f64, f64)>,
) -> Vec<MoveScore> {
    let ticks = |secs: f64| (secs * f64::from(HZ)).round() as usize;
    let mut scores = vec![];
    let mut commanded = 0.0;
    let mut prev = Prev::default();
    let params = sim.params[0].clone();
    for (n, step) in steps.iter().enumerate() {
        let label = |what: String| format!("{session} #{n} {what}");
        match *step {
            Step::Move(target, secs) => {
                let start = commanded;
                sim.retarget(target);
                let mut rows = Rows::default();
                for _ in 0..ticks(secs) {
                    if !sim.tick(&mut rows) {
                        return scores;
                    }
                }
                let s = score(
                    label(format!("move {start:+.3}→{target:+.3}")),
                    &rows,
                    None,
                    start,
                    target,
                    prev,
                );
                prev = rows.last();
                if (target - start).abs() > 1e-3 {
                    scores.push(s);
                }
                finals.push((target, sim.q_meas));
                commanded = target;
            }
            Step::Reverse { to, back, secs } => {
                let start = commanded;
                sim.retarget(to);
                let mut rows = Rows::default();
                let half = 0.5 * (start + to);
                while (sim.q_meas - half) * (to - start).signum() < 0.0 {
                    if !sim.tick(&mut rows) {
                        return scores;
                    }
                }
                scores.push(score(
                    label(format!("move {start:+.3}→{to:+.3}")),
                    &rows,
                    None,
                    start,
                    to,
                    prev,
                ));
                prev = rows.last();
                sim.retarget(back);
                let mut rows = Rows::default();
                for _ in 0..ticks(secs) {
                    if !sim.tick(&mut rows) {
                        return scores;
                    }
                }
                scores.push(score(
                    label(format!("move {to:+.3}→{back:+.3} (reversal)")),
                    &rows,
                    None,
                    to,
                    back,
                    prev,
                ));
                prev = rows.last();
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
                        return scores;
                    }
                }
                let s = score(
                    label(format!("wave [{lo:+.3}, {hi:+.3}] T {half_s} s")),
                    &rows,
                    Some((lo, hi)),
                    lo,
                    lo,
                    prev,
                );
                prev = rows.last();
                scores.push(s);
                sim.wave = None;
                let mut rows = Rows::default();
                for _ in 0..ticks(WAVE_SLACK_SECS) {
                    if !sim.tick(&mut rows) {
                        return scores;
                    }
                }
                prev = rows.last().or(prev);
                commanded = lo;
            }
        }
    }
    scores
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
        ripple_phase: 0.0,
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

/// The master pitch entry replays the bench suite on the fitted plant and its ±2σ_A gravity
/// variants and meets the bench bar on every move: slow track ≤ 0.03 rad, fast speed overshoot
/// ≤ 20 %, stop ≤ 0.01 rad, τ_ff step ≤ 0.05 Nm per tick, no predicted total-torque clamp,
/// final-second hold range ≤ 5 mrad, no fault.
///
/// The gravity variants used to be exempt from the stop gate: at kp 18 a 4 % model error moved
/// the rest point by ΔA·sin q/kp ≈ 5 mrad at the extremes. At kp 40 that offset is
/// 0.04·2.661·sin 1.2/40 ≈ 2.5 mrad and the leaky reference integral trims it, so the worst stop
/// on any variant is 5.46 mrad (gravity×0.96, both kd 5 and kd 6), and the stop is gated on
/// every plant.
#[test]
fn pitch_bench_suite_replay_meets_bench_bar() {
    let (params, model, _) = master_pitch();
    let zones = pitch_velocity_zones();
    let mut report = vec![];
    let mut failed = vec![];
    for (plant_label, plant) in fitted_plants(model) {
        for (session, steps) in sessions() {
            let Replay { scores, fault, .. } = replay(
                session,
                &steps,
                &params,
                model,
                plant,
                &zones,
                Fidelity::BASE,
            );
            if let Some(fault) = fault {
                failed.push(format!("{plant_label} {session}: fault {fault}"));
            }
            for s in scores {
                let line = format!("{plant_label} {}", s.line());
                if !s.failures().is_empty() {
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

/// The slowest recorded waves (`var/motion-suite/20261004T202431Z`, `…T202646Z`).
const SLOW_WAVES: [Step; 4] = [
    Step::Move(-0.992, HOLD_SECS),
    Step::Wave(-0.992, 0.992, 23.93),
    Step::Move(-1.038, HOLD_SECS),
    Step::Wave(-1.038, 2.533, 49.87),
];

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
        ripple_phase: 0.0,
    };
    let Replay { scores, fault, .. } = replay(
        "slow_waves",
        &SLOW_WAVES,
        &params,
        model,
        crest,
        &zones,
        Fidelity::BASE,
    );
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
    let mut sim = Sim::armed(params, model, plant, &zones, Fidelity::BASE);
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

// ---- Richer plant: drive kd 5 vs kd 6 (ADR 0040 follow-up) ----
//
// ADR 0040 chose drive kd 6 over the audit's kd 5 on the simpler law gate
// (`law_gates::pitch_bench_trial_meets_bench_criteria`: Coulomb fs 0.37 Nm, URDF inertia, no
// ripple). This section replays both gains, everything else the master pitch entry, against the
// audit's richer plant (`docs/reviews/2026-10-04-control-audit-pitch-slow-speed.md`,
// "Experiments"), every number from that report:
// - inertia J 0.14 kg·m² (`FIT_INERTIA`), and 0.10 / 0.18 in the uncertainty grid ("vary
//   J 0.10–0.18");
// - Stribeck friction `fc + (fs − fc)·exp(−|v|/v_b)`: breakaway fs 0.65 Nm, Coulomb fc 0.37 Nm,
//   Stribeck velocity v_b 0.08 rad/s, no viscous term (the fit found no slope up to 1.5 rad/s);
// - position ripple 0.4 Nm at 28.6 cycles/rad (the conservative replay amplitude; the wave fits
//   measured 0.196–0.242 Nm), phase 0, π/2, π in the grid;
// - "fitted" gravity (the master URDF model) and ×0.96 / ×1.04 (±2σ_A); "crest": static
//   breakaway fs + ripple = 1.05 Nm with no ripple term;
// - drive estimator: count difference over 5 ms, over 20 ms, or the exact velocity;
// - transport delay: the base one tick plus 0, 1 or 2 extra old feedback samples (the audit's
//   prescribed table uses 1);
// - Davout's danger-zone velocity clamp, τ_ff cap and 60 Nm/s rate limit, total-torque
//   prediction and common-factor reshape, the reshape fed back to the law, and the RS03 MIT
//   wire quantization.
//
// Sessions: the five suite sessions above, the audit's extended stops at 0.1 / 0.496 / 1.55 rad
// and the slow waves through −1.038…2.533 rad. Pass bar per move, as the original replay: slow
// track ≤ 30 mrad, fast 50 ms speed excess ≤ 20 %, stop ≤ 10 mrad, τ_ff step (law and decoded
// wire) ≤ 0.05 Nm, no predicted total-torque clamp, final-second hold range ≤ 5 mrad, no fault;
// and per case a repeat spread ≤ 10 mrad. Here the stop is gated on every gravity variant.

/// The suite plus the audit's extended stops and slow waves.
fn audit_sessions() -> Vec<(&'static str, Vec<Step>)> {
    let mut out = sessions();
    let mut stops = vec![];
    for a in [0.1, 0.496, 1.55] {
        stops.push(Step::Move(a, HOLD_SECS));
        stops.push(Step::Move(0.0, HOLD_SECS));
    }
    out.push(("extended_stops", stops));
    out.push(("slow_waves", SLOW_WAVES.to_vec()));
    out
}

/// The master pitch entry with drive kd (the wire kd and the law's) replaced by `kd`.
fn pitch_with_kd(kd: f64) -> (HoldJointParams, (f64, f64)) {
    let (mut params, model, _) = master_pitch();
    assert_eq!(params.law.kind(), PositionLaw::ScaledPd);
    params.kd = kd;
    if let HoldLaw::ScaledPd(gains) = &mut params.law {
        gains.kd = kd;
    }
    (params, model)
}

/// One richer-plant case.
#[derive(Clone, Debug)]
struct Case {
    label: String,
    plant: BenchPlant,
    fidelity: Fidelity,
}

fn crest(model: (f64, f64)) -> BenchPlant {
    BenchPlant {
        fs: FIT_FS + FIT_RIPPLE_NM,
        ripple_nm: 0.0,
        ..fitted_plants(model)[0].1
    }
}

/// The audit's prescribed table: four plants × three drive estimators, one extra old sample.
fn prescribed_cases(model: (f64, f64)) -> Vec<Case> {
    let mut plants = fitted_plants(model);
    plants.push(("crest".into(), crest(model)));
    let mut out = vec![];
    for (name, plant) in plants {
        for estimator in DriveEstimator::ALL {
            out.push(Case {
                label: format!("{name}, {}, delay +1", estimator.label()),
                plant,
                fidelity: Fidelity::audit(estimator, 1),
            });
        }
    }
    out
}

/// The audit's uncertainty grid on the fitted plant: estimator × J {0.10, 0.14, 0.18} × ripple
/// phase {0, π/2, π} × extra old samples {0, 1, 2} (81 cases).
fn uncertainty_cases(model: (f64, f64)) -> Vec<Case> {
    let nominal = fitted_plants(model)[0].1;
    let phases = [
        ("0", 0.0),
        ("π/2", std::f64::consts::FRAC_PI_2),
        ("π", std::f64::consts::PI),
    ];
    let mut out = vec![];
    for estimator in DriveEstimator::ALL {
        for inertia in [0.10, 0.14, 0.18] {
            for (phase_label, phase) in phases {
                for delay in 0..=2 {
                    out.push(Case {
                        label: format!(
                            "J {inertia:.2}, {}, phase {phase_label}, delay +{delay}",
                            estimator.label()
                        ),
                        plant: BenchPlant {
                            inertia,
                            ripple_phase: phase,
                            ..nominal
                        },
                        fidelity: Fidelity::audit(estimator, delay),
                    });
                }
            }
        }
    }
    out
}

/// Worst case of one gain on one case, over every session.
#[derive(Debug, Default)]
struct Summary {
    slow_track: f64,
    fast_overshoot: f64,
    stop: f64,
    tau_step: f64,
    clamps: usize,
    hold: f64,
    repeat_spread: f64,
    /// RMS(true velocity − reference velocity) on 0.05 ≤ |v_ref| ≤ 0.4 rad/s.
    v_rms: f64,
    max_total: f64,
    /// Failed criteria, and the failing moves.
    failed: std::collections::BTreeSet<&'static str>,
    lines: Vec<String>,
}

impl Summary {
    fn passed(&self) -> bool {
        self.failed.is_empty()
    }

    fn row(&self, kd: f64, label: &str) -> String {
        format!(
            "| {kd} | {label} | {:.2} | {:.2} | {:.2} | {:.4} | {} | {:.2} | {:.2} | {:.4} | {:.2} \
             | {} |",
            self.slow_track * 1e3,
            self.fast_overshoot * 100.0,
            self.stop * 1e3,
            self.tau_step,
            self.clamps,
            self.hold * 1e3,
            self.repeat_spread * 1e3,
            self.v_rms,
            self.max_total,
            if self.passed() {
                "PASS".into()
            } else {
                self.failed.iter().copied().collect::<Vec<_>>().join(", ")
            }
        )
    }
}

const TABLE_HEADER: &str = "| kd | case | slow track mrad | fast excess % | stop mrad | τ_ff step \
     Nm | clamps | hold mrad | repeat mrad | V rad/s | max total Nm | verdict |";

fn qualify(
    kd: f64,
    case: &Case,
    sessions: &[(&'static str, Vec<Step>)],
    zones: &[marengo_config::DangerZoneRule],
) -> Summary {
    let (params, model) = pitch_with_kd(kd);
    let mut out = Summary::default();
    let (mut v_sq, mut v_rows) = (0.0, 0usize);
    for (session, steps) in sessions {
        let run = replay(
            session,
            steps,
            &params,
            model,
            case.plant,
            zones,
            case.fidelity,
        );
        if let Some(fault) = run.fault {
            out.failed.insert("fault");
            out.lines.push(format!("{session}: fault {fault}"));
        }
        for s in &run.scores {
            if s.slow {
                out.slow_track = out.slow_track.max(s.tracking);
            } else if let Some(o) = s.overshoot {
                out.fast_overshoot = out.fast_overshoot.max(o);
            }
            out.stop = out.stop.max(s.stop);
            out.tau_step = out.tau_step.max(s.tau_step);
            out.clamps += s.clamps;
            out.hold = out.hold.max(s.hold.unwrap_or(0.0));
            let failures = s.failures();
            if !failures.is_empty() {
                out.failed.extend(failures);
                out.lines.push(s.line());
            }
        }
        if *session == "repeatability" {
            let mut targets: Vec<f64> = run.finals.iter().map(|(t, _)| *t).collect();
            targets.dedup();
            for target in targets {
                let qs = run.finals.iter().filter(|(t, _)| *t == target);
                let hi = qs
                    .clone()
                    .map(|(_, q)| *q)
                    .fold(f64::NEG_INFINITY, f64::max);
                let lo = qs.map(|(_, q)| *q).fold(f64::INFINITY, f64::min);
                out.repeat_spread = out.repeat_spread.max(hi - lo);
            }
        }
        v_sq += run.v_sq;
        v_rows += run.v_rows;
        out.max_total = out.max_total.max(run.max_total);
    }
    if out.repeat_spread > REPEAT_SPREAD_MAX_RAD {
        out.failed.insert("repeat");
    }
    out.v_rms = (v_sq / v_rows.max(1) as f64).sqrt();
    out
}

/// Every `(kd, case)` pair, on all cores; results in input order (each run is deterministic).
fn qualify_all(kds: &[f64], cases: &[Case]) -> Vec<(f64, String, Summary)> {
    let zones = pitch_velocity_zones();
    let sessions = audit_sessions();
    let jobs: Vec<(f64, &Case)> = kds
        .iter()
        .flat_map(|&kd| cases.iter().map(move |case| (kd, case)))
        .collect();
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    let mut results: Vec<Option<Summary>> = (0..jobs.len()).map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let (jobs, zones, sessions) = (&jobs, &zones, &sessions);
                scope.spawn(move || {
                    (w..jobs.len())
                        .step_by(workers)
                        .map(|n| (n, qualify(jobs[n].0, jobs[n].1, sessions, zones)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            for (n, summary) in handle.join().expect("replay worker") {
                results[n] = Some(summary);
            }
        }
    });
    jobs.iter()
        .zip(results)
        .map(|((kd, case), s)| (*kd, case.label.clone(), s.expect("every job ran")))
        .collect()
}

fn table(results: &[(f64, String, Summary)]) -> String {
    let mut out = vec![TABLE_HEADER.to_string()];
    out.extend(results.iter().map(|(kd, label, s)| s.row(*kd, label)));
    out.join("\n")
}

fn find<'a>(results: &'a [(f64, String, Summary)], kd: f64, label: &str) -> &'a Summary {
    results
        .iter()
        .find(|(k, l, _)| *k == kd && l == label)
        .map(|(_, _, s)| s)
        .expect("case in results")
}

fn failed_set(s: &Summary) -> Vec<&'static str> {
    s.failed.iter().copied().collect()
}

/// A representative subset of the full sweep below (both gains on four cases), asserting what
/// the sweep found. kd 6 is smoother and overshoots less than kd 5, but on the richer plant it
/// trips Davout's predicted total-torque bound where kd 5 meets the whole bar: at the gravity
/// extremes (|τ_ff| ≈ 2.8–3.2 Nm) a slow move whose measured velocity steps back one encoder
/// count (−0.077 rad/s) against a 0.09–0.15 rad/s reference predicts
/// `kd·(v − dq) + kd·0.1` ≈ 1.6–2.0 Nm of damping at kd 6, 0.3 Nm more than at kd 5, which
/// takes the prediction to 5.0–5.24 Nm (cap 5). On the audit's worst overshoot corner both
/// gains miss the 20 % bar; kd 6 by less.
#[test]
fn pitch_kd6_clamps_on_the_richer_plant_where_kd5_passes() {
    let (_, model) = pitch_with_kd(6.0);
    let mut all = prescribed_cases(model);
    all.extend(uncertainty_cases(model));
    let labels = [
        "fitted, 20 ms, delay +1",
        "gravity×1.04, 5 ms, delay +1",
        "crest, 5 ms, delay +1",
        "J 0.10, 20 ms, phase π/2, delay +2",
    ];
    let cases: Vec<Case> = labels
        .iter()
        .map(|label| {
            all.iter()
                .find(|c| c.label == *label)
                .cloned()
                .expect("case")
        })
        .collect();
    let results = qualify_all(&[5.0, 6.0], &cases);
    let summary = table(&results);
    let at = |kd: f64, n: usize| find(&results, kd, labels[n]);

    // Fitted plant: both gains pass; kd 6 moves more smoothly and overshoots less.
    assert!(at(5.0, 0).passed() && at(6.0, 0).passed(), "{summary}");
    assert!(at(6.0, 0).v_rms < at(5.0, 0).v_rms, "{summary}");
    assert!(
        at(6.0, 0).fast_overshoot < at(5.0, 0).fast_overshoot,
        "{summary}"
    );
    // Gravity×1.04 and the ripple crest: kd 5 meets the whole bar; kd 6 fails only on the
    // predicted total-torque clamp.
    for n in [1, 2] {
        assert!(at(5.0, n).passed(), "{summary}");
        assert!(at(5.0, n).max_total <= TAU_LIMIT, "{summary}");
        assert_eq!(failed_set(at(6.0, n)), ["clamp"], "{summary}");
        assert!(at(6.0, n).max_total > TAU_LIMIT, "{summary}");
    }
    // The audit's worst corner (J 0.10, 20 ms estimator, two extra old samples): kd 5
    // overshoots the fast bar; kd 6 overshoots less, still over the bar, and also clamps.
    assert_eq!(failed_set(at(5.0, 3)), ["speed"], "{summary}");
    assert_eq!(failed_set(at(6.0, 3)), ["clamp", "speed"], "{summary}");
    assert!(
        at(6.0, 3).fast_overshoot < at(5.0, 3).fast_overshoot,
        "{summary}"
    );
}

/// Full sweep: both gains on the audit's prescribed table (12 cases) and its 81-case uncertainty
/// grid, written as a table to `$TMPDIR/berthier-pitch-kd-sweep.txt`. About 30 s on four cores
/// in a debug build, so opt-in:
/// `cargo test -p berthier --lib pitch_kd_richer_plant_full_sweep -- --ignored`.
///
/// Result (2026-10-28, recorded in ADR 0040): prescribed, kd 5 passes 12/12 and kd 6 7/12, its
/// five failures all predicted total-torque clamps (gravity×1.04 with the 5 and 20 ms
/// estimators, crest with all three); grid, kd 5 passes 71/81 (10 fast-overshoot failures,
/// worst 27.36 %) and kd 6 45/81 (34 with clamps, one of them also over the speed bar, and 2
/// speed only, worst 22.37 %).
#[test]
#[ignore = "full kd 5 / kd 6 sweep (~30 s on 4 cores); run with --ignored, report in $TMPDIR"]
fn pitch_kd_richer_plant_full_sweep() {
    let (_, model) = pitch_with_kd(6.0);
    let prescribed = prescribed_cases(model);
    let grid = uncertainty_cases(model);
    let mut cases = prescribed.clone();
    cases.extend(grid.iter().cloned());
    let results = qualify_all(&[5.0, 6.0], &cases);
    // Library sources may not print (scripts/check-println-crates.sh): the report goes to a
    // file instead.
    let mut report = table(&results);
    for (kd, label, s) in &results {
        if !s.passed() {
            report.push_str(&format!("\nkd {kd} {label}:\n  {}", s.lines.join("\n  ")));
        }
    }
    let count = |kd: f64, set: &[Case], pass: bool| {
        set.iter()
            .filter(|c| find(&results, kd, &c.label).passed() == pass)
            .count()
    };
    for kd in [5.0, 6.0] {
        report.push_str(&format!(
            "\nkd {kd}: prescribed {}/{} pass, grid {}/{} pass",
            count(kd, &prescribed, true),
            prescribed.len(),
            count(kd, &grid, true),
            grid.len()
        ));
    }
    let report_path = std::env::temp_dir().join("berthier-pitch-kd-sweep.txt");
    std::fs::write(&report_path, report).expect("write the sweep report");
    let only = |kd: f64, set: &[Case], allowed: &[&str]| {
        set.iter().all(|c| {
            find(&results, kd, &c.label)
                .failed
                .iter()
                .all(|f| allowed.contains(f))
        })
    };
    // kd 5 meets the whole bar on the prescribed plants; on the grid it only overshoots.
    assert_eq!(count(5.0, &prescribed, false), 0);
    assert!(only(5.0, &grid, &["speed"]));
    // kd 6 fails prescribed plants, and only on the predicted total-torque clamp.
    assert!(count(6.0, &prescribed, false) > 0);
    assert!(only(6.0, &prescribed, &["clamp"]));
    assert!(only(6.0, &grid, &["clamp", "speed"]));
}
