//! Position-hold lifecycle and per-tick control law (`ControlMode::Position`).
//!
//! [`PositionHold`] owns latched targets, planners, recovery/breakaway latches, and integral
//! state. [`ControlLoop`](crate::ControlLoop) builds a [`HoldWorld`] each tick and sends the
//! returned MIT batch through Davout.

use std::fmt;
use std::time::Duration;

use armee_kinematics::{
    approach_velocity_cap, clamp_position_in_envelope, effective_command_bounds, JointLimitPolicy,
};
use davout::MitJointCommand as DavoutMit;
use marengo_config::{FrictionGains, PositionLaw};
use thiserror::Error;
use tracing::{info, trace};

use crate::friction::POSITION_HOLD_ERROR_DEADBAND_RAD;
use crate::position_feedforward::compose_position_hold_feedforward;
use crate::position_law::{advance_reference, compose_feedforward, ScaledPdGains, ScaledPdState};
use crate::position_profile::{position_hold_v_max, PlannerEvent};
use crate::position_setpoint::{
    apply_lead_follow_hold_short, clamp_trajectory_setpoint, descent_breakaway_confirmed,
    descent_stuck_mit_pull, downward_return_seed_velocity, home_final_approach_stuck_pull_rad,
    home_target_tolerance, lead_follow_stuck_residual, low_angle_breakaway_active,
    outbound_stall_direction, planner_drifted_from_measurement, planner_should_freeze_on_descent,
    planner_should_latch_on_overshoot_hold, planner_should_lead_follow_hold_short,
    planner_should_recover_ascent_stall, planner_should_reopen_premature_hold,
    planner_should_resync_stuck_lead, position_hold_effective_max_lead, position_hold_mit_kd,
    position_hold_mit_velocity, reopen_planner_from_premature_hold,
    POSITION_HOME_FINAL_PULL_THROUGH_RAD, POSITION_RETURN_DESCENT_SEED_RAD,
    POSITION_RETURN_RESYNC_RAD, POSITION_SETTLE_TOLERANCE_RAD,
};
use crate::position_trajectory::{
    filter_dq_ema, JointPositionPlanner, TrapezoidPhase, POSITION_DAMPING_DQ_FILTER_ALPHA,
};
use crate::position_wave::PositionWave;

/// Historical advance-path default when a joint has no `control.yaml` entry.
/// Compose used `unwrap_or(0.15)`; keep that split.
pub const ADVANCE_MAX_LEAD_DEFAULT: f64 = 0.10;

/// Outstanding ascent without measured progress before tick faults (disable path).
/// The hold-tracking fuse shares this budget.
pub const POSITION_ASCENT_STALL_FAULT_MS: u64 = 2000;

/// Hold-tracking fuse arms only while `|q − target|` exceeds this band (both directions).
pub const POSITION_HOLD_TRACKING_BAND_RAD: f64 = POSITION_RETURN_RESYNC_RAD;

const MAX_INTEGRAL_NM: f64 = 0.5;

/// No-progress budget: only a new best measured level renews it; retreat cannot rebase it.
#[derive(Debug, Clone, Copy)]
struct ProgressBudget {
    /// Best credited value of the progress measure (larger is closer to done).
    credited: f64,
    stalled: Duration,
}

impl ProgressBudget {
    fn stalled_ms(slot: Option<Self>) -> u64 {
        slot.map_or(0, |budget| {
            u64::try_from(budget.stalled.as_millis()).unwrap_or(u64::MAX)
        })
    }

    /// Observe one tick of `measure` and return the stalled time (ms).
    fn observe(
        slot: &mut Option<Self>,
        measure: f64,
        progress_threshold: f64,
        period: Duration,
    ) -> u64 {
        match slot.as_mut() {
            Some(budget) => {
                // Direct law callers have ideal continuous measurements. Installed controllers
                // supply Davout's decoded-grid threshold; this floor only bounds f64 roundoff.
                let threshold = if progress_threshold == 0.0 {
                    8.0 * f64::EPSILON * measure.abs().max(budget.credited.abs()).max(1.0)
                } else {
                    progress_threshold
                };
                if measure - budget.credited > threshold {
                    budget.credited = measure;
                    budget.stalled = Duration::ZERO;
                } else {
                    budget.stalled = budget.stalled.saturating_add(period);
                }
            }
            None => {
                *slot = Some(Self {
                    credited: measure,
                    stalled: period,
                });
            }
        }
        Self::stalled_ms(*slot)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AscentRecovery {
    /// Planner recovery retains its existing velocity/lead policy.
    planner_active: bool,
    /// Outbound direction (`±1.0`) the credited level is measured in; `0.0` when no episode.
    direction: f64,
    /// The safety budget has a geometric lifetime independent of that policy.
    /// Progress measure: `direction * q` (a new encoder level further outbound).
    progress: Option<ProgressBudget>,
}

impl AscentRecovery {
    fn is_active(self) -> bool {
        self.planner_active
    }

    fn stalled_ms(self) -> u64 {
        ProgressBudget::stalled_ms(self.progress)
    }

    /// `outbound` is the direction of the commanded outbound move, or `None` when the joint is
    /// not in a watched episode. A changed direction starts a new episode: the credited level of
    /// the old direction is meaningless in the new one.
    fn update(
        &mut self,
        planner_active: bool,
        outbound: Option<f64>,
        q: f64,
        progress_threshold: f64,
        period: Duration,
    ) -> u64 {
        self.planner_active = planner_active;
        let Some(direction) = outbound else {
            self.direction = 0.0;
            self.progress = None;
            return 0;
        };
        if self.direction != direction {
            self.direction = direction;
            self.progress = None;
        }
        ProgressBudget::observe(
            &mut self.progress,
            direction * q,
            progress_threshold,
            period,
        )
    }
}

/// Commanded wave speed below this is a dwell (endpoint turn-around): the wave stall budget
/// neither accrues nor renews there.
pub const POSITION_WAVE_STALL_MIN_COMMANDED_SPEED_RAD_S: f64 = 0.05;

/// Measured motion of at least this (or the installed feedback-grid threshold, if larger) away
/// from the credited level renews the wave stall budget. Below the hold-tracking band.
pub const POSITION_WAVE_STALL_PROGRESS_RAD: f64 = 0.02;

/// Wave stall fuse: while the wave commands motion, the encoder must keep leaving its credited
/// level. A joint that is jammed (or whose output is saturated against friction or an obstacle)
/// shows no measured motion while the wave target keeps moving, which the hold-tracking
/// closeness measure cannot see because the target sweeps through the stuck `q`.
#[derive(Debug, Clone, Copy, Default)]
struct WaveMotionWatch {
    anchor: Option<f64>,
    stalled: Duration,
}

impl WaveMotionWatch {
    fn stalled_ms(self) -> u64 {
        u64::try_from(self.stalled.as_millis()).unwrap_or(u64::MAX)
    }

    fn update(
        &mut self,
        q: f64,
        commanded_speed: f64,
        progress_threshold: f64,
        period: Duration,
    ) -> u64 {
        let anchor = *self.anchor.get_or_insert(q);
        let renew = POSITION_WAVE_STALL_PROGRESS_RAD.max(progress_threshold);
        if (q - anchor).abs() > renew {
            self.anchor = Some(q);
            self.stalled = Duration::ZERO;
        } else if commanded_speed.abs() >= POSITION_WAVE_STALL_MIN_COMMANDED_SPEED_RAD_S {
            self.stalled = self.stalled.saturating_add(period);
        }
        self.stalled_ms()
    }
}

/// Hold-tracking fuse: off target beyond [`POSITION_HOLD_TRACKING_BAND_RAD`] while the net
/// commanded torque (`tau_p + tau_ff`) pushes away from the target.
///
/// Progress measure: `−|q − target|` (a new closest encoder level to the target).
#[derive(Debug, Clone, Copy, Default)]
struct HoldTracking {
    progress: Option<ProgressBudget>,
}

impl HoldTracking {
    #[cfg(test)]
    fn stalled_ms(self) -> u64 {
        ProgressBudget::stalled_ms(self.progress)
    }

    fn update(
        &mut self,
        armed: bool,
        q: f64,
        target: f64,
        progress_threshold: f64,
        period: Duration,
    ) -> u64 {
        if !armed {
            self.progress = None;
            return 0;
        }
        let closeness = -(q - target).abs();
        ProgressBudget::observe(&mut self.progress, closeness, progress_threshold, period)
    }
}

/// Hold-tracking arms when the arm is outside the band and net commanded torque opposes target.
fn hold_tracking_opposed(q: f64, target: f64, net_commanded_torque: f64) -> bool {
    let to_target = target - q;
    to_target.abs() > POSITION_HOLD_TRACKING_BAND_RAD && net_commanded_torque * to_target < 0.0
}

/// Joint-space controller state when a position-hold fuse trips.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldFuseTrip {
    pub q: f64,
    pub target: f64,
    pub tau_p: f64,
    pub tau_ff: f64,
    pub tau_g: f64,
}

impl fmt::Display for HoldFuseTrip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "q={:.4} rad, target={:.4} rad, tau_p={:+.3} Nm, tau_ff={:+.3} Nm, tau_g={:+.3} Nm",
            self.q, self.target, self.tau_p, self.tau_ff, self.tau_g
        )
    }
}

#[derive(Debug, Error)]
pub enum HoldError {
    #[error(
        "position hold: outbound ascent stall on {joint}: no measured progress toward target for {ms} ms ({trip})"
    )]
    AscentStall {
        joint: String,
        ms: u64,
        trip: HoldFuseTrip,
    },
    #[error(
        "position hold: hold tracking failure on {joint}: off target with net commanded torque opposing it and no measured progress for {ms} ms ({trip})"
    )]
    HoldTracking {
        joint: String,
        ms: u64,
        trip: HoldFuseTrip,
    },
    #[error(
        "position wave: wave stall on {joint}: no measured motion while the wave commanded motion for {ms} ms ({trip})"
    )]
    WaveStall {
        joint: String,
        ms: u64,
        trip: HoldFuseTrip,
    },
    #[error("position hold: no setpoint latched for joint {joint}")]
    MissingSetpoint { joint: String },
    #[error("position hold: length mismatch")]
    LenMismatch,
    #[error("position hold: invalid nominal tick period {seconds} seconds")]
    InvalidPeriod { seconds: f64 },
}

/// Position law for one joint, with the scaled-PD parameters resolved for this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoldLaw {
    /// ADR 0007 law (default).
    Legacy,
    /// ADR 0039 scaled-PD law ([`crate::position_law`]).
    ScaledPd(ScaledPdGains),
}

impl HoldLaw {
    pub fn kind(&self) -> PositionLaw {
        match self {
            Self::Legacy => PositionLaw::Legacy,
            Self::ScaledPd(_) => PositionLaw::ScaledPd,
        }
    }
}

/// Descent speed cap from Davout `clamp_velocity` danger zones (`control.danger_zones`): while
/// the joint is above `above_rad` and descending, Davout clamps the MIT velocity to
/// `max_velocity_rad_s`. Under drive damping that clamp brakes against a faster reference and
/// steps the drive torque when it releases, so the reference itself respects it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DescentCap {
    pub above_rad: f64,
    pub max_velocity_rad_s: f64,
}

impl DescentCap {
    /// The tightest cap over the joint's `clamp_velocity` zones: the lowest threshold and the
    /// lowest speed, so the result covers every zone. `None` when no zone names the joint.
    pub fn from_zones(zones: &[marengo_config::DangerZoneRule], joint: &str) -> Option<Self> {
        zones
            .iter()
            .filter(|z| z.joint == joint && z.action == "clamp_velocity")
            .fold(None, |acc: Option<Self>, z| {
                Some(match acc {
                    None => Self {
                        above_rad: z.position_above_rad,
                        max_velocity_rad_s: z.max_velocity_rad_s,
                    },
                    Some(cap) => Self {
                        above_rad: cap.above_rad.min(z.position_above_rad),
                        max_velocity_rad_s: cap.max_velocity_rad_s.min(z.max_velocity_rad_s),
                    },
                })
            })
    }

    /// Reference speed limit for one tick: capped while the reference moves down
    /// (`target < q_ref`) with the reference or the measured joint above the threshold (Davout
    /// tests measured q, which lags a descending reference).
    pub fn limit(&self, v_max: f64, q_ref: f64, q: f64, target: f64) -> f64 {
        if target < q_ref && q_ref.max(q) > self.above_rad {
            v_max.min(self.max_velocity_rad_s)
        } else {
            v_max
        }
    }

    /// Largest descending speed of a raised-cosine wave over `[min_rad, max_rad]` with peak
    /// speed `peak` while q is above the threshold: `q = c + A·cos θ` descends at `A·ω·sin θ`,
    /// so above `max(above, c)` the speed is at most `peak·√(1 − u²)`, `u = (max(above, c) −
    /// c)/A`. Zero when the band stays at or below the threshold.
    pub fn wave_descent_speed(&self, min_rad: f64, max_rad: f64, peak: f64) -> f64 {
        if max_rad <= self.above_rad {
            return 0.0;
        }
        let center = 0.5 * (min_rad + max_rad);
        let amplitude = 0.5 * (max_rad - min_rad);
        let u = ((self.above_rad.max(center) - center) / amplitude).clamp(0.0, 1.0);
        peak * (1.0 - u * u).sqrt()
    }
}

/// Per-joint config + measurements needed for one hold tick.
#[derive(Debug, Clone)]
pub struct HoldJointParams {
    pub kp: f64,
    pub kd: f64,
    pub ki: f64,
    /// MIT / compose max lead (legacy default 0.15 when cfg missing).
    pub max_lead: f64,
    /// MIT / compose velocity deadband (legacy default 0.02 when cfg missing).
    pub vel_deadband: f64,
    /// Advance freeze/resync max lead (legacy default 0.10 when cfg missing).
    pub advance_max_lead: f64,
    /// Advance freeze/resync deadband (legacy default [`POSITION_HOLD_ERROR_DEADBAND_RAD`]).
    pub advance_vel_deadband: f64,
    pub slew_rad_s: f64,
    pub trajectory_v_max: f64,
    pub trajectory_threshold_rad: f64,
    pub a_max: f64,
    /// Supervisor joint velocity cap (rad/s), if configured.
    pub velocity_cap: Option<f64>,
    pub friction: Option<FrictionGains>,
    pub limit_policy: Option<JointLimitPolicy>,
    pub tau_meas: f64,
    /// Position law for this joint (selector + scaled-PD parameters).
    pub law: HoldLaw,
    /// Danger-zone descent cap on the reference speed (scaled-PD law only), if any.
    pub descent_cap: Option<DescentCap>,
}

/// Atomic operator retarget — owns raw/clamped/planner/latch/dq ordering.
#[derive(Debug, Clone)]
pub struct HoldRetarget {
    pub joint_idx: usize,
    pub clamped: f64,
    pub requested: f64,
    pub q: f64,
    pub tick: u64,
    pub dq_seed: Option<f64>,
    pub downward_seed: Option<f64>,
}

/// Tick inputs borrowed from [`ControlLoop`](crate::ControlLoop).
pub struct HoldWorld<'a> {
    pub q: &'a [f64],
    pub dq_meas: &'a [f64],
    pub tau_g: &'a [f64],
    pub joints: &'a [HoldJointParams],
    pub joint_names: &'a [String],
    pub dt: f64,
    pub hz: u32,
    pub tick_count: u64,
    /// In-loop wave slot — hold clears it when finished (Ok or before AscentStall return).
    pub wave: &'a mut Option<PositionWave>,
}

/// Per-joint diagnostics for 1 Hz logs and position-trace CSV.
#[derive(Debug, Clone)]
pub struct HoldJointDiag {
    pub move_dist: f64,
    pub v_max_eff: f64,
    pub planner_event: PlannerEvent,
    pub q: f64,
    pub dq_raw: f64,
    pub dq_filt: f64,
    pub q_traj: f64,
    pub dq_traj: f64,
    pub q_des: f64,
    pub target: f64,
    pub target_raw: f64,
    pub lead: f64,
    pub lead_sat: bool,
    pub settle_error: f64,
    pub settling: bool,
    pub friction_mode: &'static str,
    pub retarget_age_ms: u64,
    pub joint_stuck: bool,
    pub planner_frozen: bool,
    pub phase: &'static str,
    pub kp: f64,
    pub kd: f64,
    pub tau_g: f64,
    pub tau_f: f64,
    pub tau_d: f64,
    pub tau_ff_cmd: f64,
    pub tau_meas: f64,
    pub tau_p: f64,
    pub mit_velocity: f64,
    pub mit_kd: f64,
    pub q_env_lo: f64,
    pub q_env_hi: f64,
    pub retarget_tick: Option<u64>,
    /// Nominal time without a qualifying new encoder high level (ms); 0 when exempt.
    pub ascent_stall_ms: u64,
    /// Nominal time off target with opposing net commanded torque and no progress (ms).
    pub hold_tracking_ms: u64,
    pub law: PositionLaw,
    /// Reference the law commands: legacy planner `q_traj`, scaled-PD `q_ref`.
    pub q_ref: f64,
    /// Reference velocity the law commands: legacy `dq_traj`, scaled-PD `v_ref`.
    pub dq_ref: f64,
    /// Reference governor scale `s` (legacy: 0 while the planner is frozen, else 1).
    pub time_scale: f64,
    /// Integral torque term (Nm).
    pub tau_i: f64,
}

/// Per-joint inputs [`PositionHold::compose`] hands to the scaled-PD compose.
#[derive(Debug, Clone, Copy)]
struct ScaledJointTick {
    index: usize,
    gains: ScaledPdGains,
    q_ref: f64,
    phase: TrapezoidPhase,
    target: f64,
    target_raw: f64,
    retarget_age_ms: u64,
    move_dist: f64,
    wave_owned: bool,
}

/// Output of [`PositionHold::tick`]: MIT batch + per-joint diag.
#[derive(Debug)]
pub struct HoldTickOut {
    pub mit: Vec<DavoutMit>,
    pub diag: Vec<HoldJointDiag>,
}

/// Owns all position-hold lifecycle state for the control loop.
#[derive(Debug)]
pub struct PositionHold {
    n_joints: usize,
    setpoints: Option<Vec<f64>>,
    setpoints_raw: Option<Vec<f64>>,
    planners: Option<Vec<JointPositionPlanner>>,
    retarget_tick: Option<Vec<u64>>,
    dq_filtered: Option<Vec<f64>>,
    planner_frozen: Option<Vec<bool>>,
    ascent_recovery: Option<Vec<AscentRecovery>>,
    hold_tracking: Vec<HoldTracking>,
    /// Wave-owned joints are exempt from the ascent/tracking fuses; this budget replaces them.
    wave_motion: Vec<WaveMotionWatch>,
    /// Davout's immutable installed feedback grid; zero denotes ideal continuous law inputs.
    progress_thresholds: Vec<f64>,
    /// Only joints commanded by the position owner can exhaust its watchdog budget.
    commanded_joints: Vec<bool>,
    descent_breakaway: Option<Vec<bool>>,
    descent_was_stuck: Option<Vec<bool>>,
    planner_events: Option<Vec<PlannerEvent>>,
    integral_error: Option<Vec<f64>>,
    /// Post-advance effective max lead for compose (FIX A: one value per joint per tick).
    tick_effective_max_lead: Vec<f64>,
    tick_stuck_residual: Vec<bool>,
    /// Last law each joint ran (retargets between ticks follow it).
    laws: Vec<PositionLaw>,
    /// Scaled-PD time scale, commanded velocity and integral, per joint.
    scaled: Vec<ScaledPdState>,
}

impl PositionHold {
    pub fn new(n_joints: usize) -> Self {
        Self {
            n_joints,
            setpoints: None,
            setpoints_raw: None,
            planners: None,
            retarget_tick: None,
            dq_filtered: None,
            planner_frozen: None,
            ascent_recovery: None,
            hold_tracking: vec![HoldTracking::default(); n_joints],
            wave_motion: vec![WaveMotionWatch::default(); n_joints],
            progress_thresholds: vec![0.0; n_joints],
            commanded_joints: vec![true; n_joints],
            descent_breakaway: None,
            descent_was_stuck: None,
            planner_events: None,
            integral_error: None,
            tick_effective_max_lead: vec![0.0; n_joints],
            tick_stuck_residual: vec![false; n_joints],
            laws: vec![PositionLaw::Legacy; n_joints],
            scaled: vec![ScaledPdState::default(); n_joints],
        }
    }

    pub(crate) fn with_progress_thresholds(progress_thresholds: Vec<f64>) -> Self {
        let mut hold = Self::new(progress_thresholds.len());
        hold.progress_thresholds = progress_thresholds;
        hold
    }

    pub(crate) fn set_commanded_joint(&mut self, joint: usize, commanded: bool) {
        if let Some(slot) = self.commanded_joints.get_mut(joint) {
            *slot = commanded;
        }
    }

    /// Law joint `joint` runs; retargets between ticks follow it. Each tick also adopts
    /// [`HoldJointParams::law`]. Switching law restarts the scaled-PD state (time scale 1,
    /// integral 0); the reference itself continues.
    pub(crate) fn set_joint_law(&mut self, joint: usize, law: PositionLaw) {
        if let Some(slot) = self.laws.get_mut(joint) {
            if *slot != law {
                *slot = law;
                self.scaled[joint] = ScaledPdState::default();
            }
        }
    }

    pub fn is_armed(&self) -> bool {
        self.setpoints.is_some()
    }

    pub fn targets(&self) -> Option<&[f64]> {
        self.setpoints.as_deref()
    }

    /// Operator-requested targets before envelope clamp, for tests.
    /// (Pre-tick reads; per-tick diag `target_raw` only exists after a tick.)
    #[cfg(test)]
    pub(crate) fn raw_targets_for_test(&self) -> Option<&[f64]> {
        self.setpoints_raw.as_deref()
    }

    /// Planner trajectory references (status / tests).
    pub fn q_traj(&self) -> Option<Vec<f64>> {
        self.planners
            .as_ref()
            .map(|planners| planners.iter().map(|p| p.q_traj).collect())
    }

    #[cfg(test)]
    pub fn planner_state(&self, joint_idx: usize) -> Option<(f64, f64)> {
        let planner = self.planners.as_ref()?.get(joint_idx)?;
        Some((planner.q_traj, planner.dq_traj))
    }

    /// Clamped target as latched: within the joint's grid-aware home tolerance it is home.
    ///
    /// Every home/outbound classifier compares against `0.0`, so a latch one feedback count
    /// off zero behaves exactly like an exact-zero latch (planner, settle bands and fuses).
    fn home_classified(&self, joint_idx: usize, target: f64) -> f64 {
        let threshold = self
            .progress_thresholds
            .get(joint_idx)
            .copied()
            .unwrap_or(0.0);
        if target.abs() <= home_target_tolerance(threshold) {
            0.0
        } else {
            target
        }
    }

    /// Latch targets and initialize planners at measured `q`.
    pub fn arm(&mut self, q: &[f64], targets: &[f64], tick: u64) {
        let mut raw = targets.to_vec();
        raw.resize(self.n_joints, 0.0);
        let sp = raw
            .iter()
            .enumerate()
            .map(|(i, &target)| self.home_classified(i, target))
            .collect();
        self.setpoints_raw = Some(raw);
        self.setpoints = Some(sp);
        self.init_planners(q, tick);
    }

    /// Ensure setpoints exist (copy `q` if needed) without resetting planners if already armed.
    pub fn ensure_armed_from_q(&mut self, q: &[f64], tick: u64) {
        if self.setpoints.is_none() {
            self.arm(q, q, tick);
        }
    }

    /// Atomic retarget: raw + clamped target; on change also planner sync, latch/onset clear, dq seed.
    ///
    /// Same-target (`|Δ| ≤ 1e-6`) is a full no-op on planner/latches/fuse (idempotent hold-at).
    /// Returns `true` when the clamped target changed, or when this call first armed the hold.
    /// A non-finite target or measured `q` is refused without touching any state (`false`):
    /// `NaN` would otherwise be latched, because `(old − NaN).abs() > 1e-6` is false.
    pub fn apply_retarget(&mut self, cmd: HoldRetarget) -> bool {
        if cmd.joint_idx >= self.n_joints
            || !cmd.clamped.is_finite()
            || !cmd.requested.is_finite()
            || !cmd.q.is_finite()
        {
            return false;
        }
        let clamped = self.home_classified(cmd.joint_idx, cmd.clamped);
        // Refuse half-apply: ensure clamped setpoints exist before writing raw.
        let was_unarmed = self.setpoints.is_none();
        if was_unarmed {
            let q = vec![cmd.q; self.n_joints];
            let mut targets = vec![cmd.q; self.n_joints];
            targets[cmd.joint_idx] = clamped;
            self.arm(&q, &targets, cmd.tick);
        }
        self.setpoints_raw
            .get_or_insert_with(|| vec![0.0; self.n_joints])[cmd.joint_idx] = cmd.requested;
        let changed = self.set_clamped_target(cmd.joint_idx, clamped, cmd.tick);
        // First arm sees equal targets after `arm`, so treat unarmed as a full retarget apply.
        if was_unarmed || changed {
            // Scaled PD replans from the reference's own state; only the first arm starts at q.
            if self.laws[cmd.joint_idx] == PositionLaw::Legacy {
                self.sync_retarget_planner(cmd.joint_idx, cmd.q, clamped, cmd.downward_seed);
            }
            if let Some(dq) = cmd.dq_seed {
                self.seed_dq_filter(cmd.joint_idx, dq);
            }
        }
        was_unarmed || changed
    }

    /// Seed filtered `dq` for one joint (arm / latch paths).
    pub fn seed_dq_filter(&mut self, joint_idx: usize, dq: f64) {
        if joint_idx >= self.n_joints {
            return;
        }
        self.dq_filtered
            .get_or_insert_with(|| vec![0.0; self.n_joints])[joint_idx] = dq;
    }

    /// Update latched clamped target. Clears latches only when `|old − target| > 1e-6`.
    fn set_clamped_target(&mut self, joint_idx: usize, target: f64, tick: u64) -> bool {
        let old = self
            .setpoints
            .as_ref()
            .and_then(|sp| sp.get(joint_idx).copied());
        if let Some(sp) = self.setpoints.as_mut() {
            sp[joint_idx] = target;
        }
        let changed = old.map(|o| (o - target).abs() > 1e-6).unwrap_or(true);
        if changed {
            self.mark_retarget(joint_idx, tick);
        }
        changed
    }

    fn sync_retarget_planner(
        &mut self,
        joint_idx: usize,
        q: f64,
        target: f64,
        downward_seed: Option<f64>,
    ) {
        // Planners come from arm/ensure_armed; do not invent qi for other joints here.
        let Some(planner) = self.planners.as_mut().and_then(|p| p.get_mut(joint_idx)) else {
            return;
        };
        planner.reset_target(q, target);
        if let Some(seed) = downward_seed {
            planner.seed_downward_return_if_needed(
                q,
                target,
                POSITION_RETURN_DESCENT_SEED_RAD,
                seed,
            );
        }
    }

    pub fn clear(&mut self) {
        // Disable/mode changes clear intent, never the installed measurement profile or the
        // per-joint law selection.
        let progress_thresholds = std::mem::take(&mut self.progress_thresholds);
        let laws = std::mem::take(&mut self.laws);
        *self = Self::with_progress_thresholds(progress_thresholds);
        self.laws = laws;
    }

    /// Force Hold at `q_traj` (tests: residual lead-follow / AscentStall scenarios).
    #[cfg(test)]
    pub fn force_planner_hold_at(
        &mut self,
        joint_idx: usize,
        q_traj: f64,
    ) -> Result<(), HoldError> {
        let planner = self
            .planners
            .as_mut()
            .and_then(|p| p.get_mut(joint_idx))
            .ok_or_else(|| HoldError::MissingSetpoint {
                joint: format!("joint[{joint_idx}]"),
            })?;
        planner.q_traj = q_traj;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        Ok(())
    }

    /// Force Cruise ahead of measured `q` (tests: ascent-stall without lead-follow residual).
    #[cfg(test)]
    pub fn force_planner_cruise_at(
        &mut self,
        joint_idx: usize,
        q_traj: f64,
        dq_traj: f64,
    ) -> Result<(), HoldError> {
        let planner = self
            .planners
            .as_mut()
            .and_then(|p| p.get_mut(joint_idx))
            .ok_or_else(|| HoldError::MissingSetpoint {
                joint: format!("joint[{joint_idx}]"),
            })?;
        planner.resume_cruise_toward(q_traj, dq_traj);
        Ok(())
    }

    #[cfg(test)]
    pub fn ascent_stall_ms_at(&self, joint_idx: usize) -> u64 {
        self.ascent_recovery
            .as_ref()
            .and_then(|v| v.get(joint_idx).copied())
            .unwrap_or_default()
            .stalled_ms()
    }

    #[cfg(test)]
    pub fn hold_tracking_ms_at(&self, joint_idx: usize) -> u64 {
        self.hold_tracking
            .get(joint_idx)
            .copied()
            .unwrap_or_default()
            .stalled_ms()
    }

    #[cfg(test)]
    pub fn set_ascent_stall_ms_for_test(&mut self, joint_idx: usize, ms: u64) {
        self.init_latch_state();
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            if joint_idx < recovery.len() {
                recovery[joint_idx] = AscentRecovery {
                    planner_active: true,
                    direction: 1.0,
                    progress: Some(ProgressBudget {
                        credited: self
                            .planners
                            .as_ref()
                            .and_then(|planners| planners.get(joint_idx))
                            .map_or(0.0, |planner| planner.q_traj),
                        stalled: Duration::from_millis(ms),
                    }),
                };
            }
        }
    }

    #[cfg(test)]
    pub fn set_ascent_recovery_for_test(&mut self, joint_idx: usize, active: bool) {
        self.init_latch_state();
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            if let Some(state) = recovery.get_mut(joint_idx) {
                *state = AscentRecovery {
                    planner_active: active,
                    direction: 0.0,
                    progress: None,
                };
            }
        }
    }

    #[cfg(test)]
    pub fn retarget_tick_at(&self, joint_idx: usize) -> Option<u64> {
        self.retarget_tick
            .as_ref()
            .and_then(|v| v.get(joint_idx).copied())
    }

    #[cfg(test)]
    pub fn tick_effective_max_lead_at(&self, joint_idx: usize) -> f64 {
        self.tick_effective_max_lead
            .get(joint_idx)
            .copied()
            .unwrap_or(0.0)
    }

    #[cfg(test)]
    pub fn dq_filtered_at(&self, joint_idx: usize) -> Option<f64> {
        self.dq_filtered
            .as_ref()
            .and_then(|v| v.get(joint_idx).copied())
    }

    #[cfg(test)]
    pub fn integral_at(&self, joint_idx: usize) -> f64 {
        self.integral_error
            .as_ref()
            .and_then(|v| v.get(joint_idx).copied())
            .unwrap_or(0.0)
    }

    /// Advance planners then compose MIT for every joint.
    pub fn tick(&mut self, mut world: HoldWorld<'_>) -> Result<HoldTickOut, HoldError> {
        let n = self.n_joints;
        if world.q.len() != n
            || world.dq_meas.len() != n
            || world.tau_g.len() != n
            || world.joints.len() != n
            || world.joint_names.len() != n
        {
            return Err(HoldError::LenMismatch);
        }
        if !self.is_armed() {
            let joint = world
                .joint_names
                .first()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            return Err(HoldError::MissingSetpoint { joint });
        }

        // Zero is useful for pure composition. Positive periods must remain representable;
        // negative/nonfinite/overflowing durations cannot mutate planner or watchdog state.
        let period = Duration::try_from_secs_f64(world.dt)
            .map_err(|_| HoldError::InvalidPeriod { seconds: world.dt })?;
        if world.dt > 0.0 && period.is_zero() {
            return Err(HoldError::InvalidPeriod { seconds: world.dt });
        }

        self.advance(&mut world, period)?;
        self.compose(&world, period)
    }

    fn init_planners(&mut self, q: &[f64], tick: u64) {
        let targets = self.setpoints.clone().unwrap_or_else(|| q.to_vec());
        self.planners = Some(
            (0..self.n_joints)
                .map(|i| {
                    let qi = q.get(i).copied().unwrap_or(0.0);
                    let ti = targets.get(i).copied().unwrap_or(qi);
                    JointPositionPlanner::new_for_target(qi, ti)
                })
                .collect(),
        );
        self.retarget_tick = Some(vec![tick; self.n_joints]);
        self.dq_filtered = Some(vec![0.0; self.n_joints]);
        self.init_latch_state();
        Self::fill_bool(&mut self.planner_frozen, false);
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            recovery.fill(AscentRecovery::default());
        }
        self.hold_tracking.fill(HoldTracking::default());
        self.wave_motion.fill(WaveMotionWatch::default());
        self.scaled.fill(ScaledPdState::default());
        Self::fill_bool(&mut self.descent_breakaway, false);
        Self::fill_bool(&mut self.descent_was_stuck, false);
    }

    fn init_latch_state(&mut self) {
        let n = self.n_joints;
        Self::ensure_bool_vec(&mut self.planner_frozen, n);
        if self.ascent_recovery.is_none() {
            self.ascent_recovery = Some(vec![AscentRecovery::default(); n]);
        }
        Self::ensure_bool_vec(&mut self.descent_breakaway, n);
        Self::ensure_bool_vec(&mut self.descent_was_stuck, n);
        if self.integral_error.is_none() {
            self.integral_error = Some(vec![0.0; n]);
        }
    }

    fn ensure_bool_vec(slot: &mut Option<Vec<bool>>, n: usize) {
        if slot.is_none() {
            *slot = Some(vec![false; n]);
        }
    }

    fn fill_bool(slot: &mut Option<Vec<bool>>, value: bool) {
        if let Some(v) = slot.as_mut() {
            v.fill(value);
        }
    }

    fn bool_at(slot: &Option<Vec<bool>>, i: usize) -> bool {
        slot.as_ref()
            .and_then(|v| v.get(i).copied())
            .unwrap_or(false)
    }

    fn set_bool_at(slot: &mut Option<Vec<bool>>, i: usize, value: bool) {
        if let Some(v) = slot.as_mut() {
            if let Some(cell) = v.get_mut(i) {
                *cell = value;
            }
        }
    }

    /// Latch clamped + raw targets together (wave drive / wave end).
    fn latch_joint_target(&mut self, idx: usize, target: f64) {
        let clamped = self.home_classified(idx, target);
        if let (Some(setpoints), Some(raw)) = (self.setpoints.as_mut(), self.setpoints_raw.as_mut())
        {
            setpoints[idx] = clamped;
            raw[idx] = target;
        }
    }

    fn mark_retarget(&mut self, joint_idx: usize, tick: u64) {
        self.init_latch_state();
        self.retarget_tick
            .get_or_insert_with(|| vec![0; self.n_joints])[joint_idx] = tick;
        Self::set_bool_at(&mut self.planner_frozen, joint_idx, false);
        // A retarget ends the planner-recovery policy episode but never renews a safety budget:
        // only measured progress (or the commanded condition ending) does. Otherwise a stream of
        // retargets, however small or frequent, keeps a stalled or sagging joint unfused.
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            recovery[joint_idx].planner_active = false;
        }
        Self::set_bool_at(&mut self.descent_breakaway, joint_idx, false);
        Self::set_bool_at(&mut self.descent_was_stuck, joint_idx, false);
        if let Some(integral) = self.integral_error.as_mut() {
            integral[joint_idx] = 0.0;
        }
    }

    fn retarget_age_ms(&self, joint_idx: usize, tick_count: u64, hz: u32) -> u64 {
        let Some(ticks) = self.retarget_tick.as_ref() else {
            return u64::MAX;
        };
        let retarget_tick = ticks.get(joint_idx).copied().unwrap_or(0);
        tick_count
            .saturating_sub(retarget_tick)
            .saturating_mul(1000)
            / u64::from(hz.max(1))
    }

    fn filtered_dq(&mut self, joint_idx: usize, dq_raw: f64) -> f64 {
        match self.dq_filtered.as_mut() {
            None => {
                self.dq_filtered = Some(vec![dq_raw; self.n_joints]);
                dq_raw
            }
            Some(filtered) => {
                let next = filter_dq_ema(
                    filtered[joint_idx],
                    dq_raw,
                    POSITION_DAMPING_DQ_FILTER_ALPHA,
                );
                filtered[joint_idx] = next;
                next
            }
        }
    }

    fn clamp_v_max(velocity_cap: Option<f64>, v_requested: f64) -> f64 {
        velocity_cap.map_or(v_requested, |cap| v_requested.min(cap))
    }

    /// Scaled-PD cruise speed: the legacy selection with the move measured from the
    /// reference, so it never depends on measured `q`.
    fn scaled_v_max(jp: &HoldJointParams, planner: &JointPositionPlanner, target: f64) -> f64 {
        Self::clamp_v_max(
            jp.velocity_cap,
            position_hold_v_max(
                (target - planner.q_traj).abs(),
                jp.slew_rad_s,
                jp.trajectory_v_max,
                jp.trajectory_threshold_rad,
                planner.dq_traj.abs(),
            ),
        )
    }

    fn reset_planner_with_downward_seed(
        planner: &mut JointPositionPlanner,
        q: f64,
        target: f64,
        slew_rad_s: f64,
        v_max: f64,
    ) {
        planner.reset_target(q, target);
        planner.seed_downward_return_if_needed(
            q,
            target,
            POSITION_RETURN_DESCENT_SEED_RAD,
            downward_return_seed_velocity(slew_rad_s, v_max, q, target),
        );
    }

    fn advance(&mut self, world: &mut HoldWorld<'_>, period: Duration) -> Result<(), HoldError> {
        let mut wave_cmd_dq: Option<(usize, f64)> = None;
        let mut clear_finished_wave = false;
        if let Some(wave) = world.wave.as_mut() {
            let joint_name = world.joint_names[wave.joint_index].clone();
            let idx = wave.joint_index;
            if let Some((target, dq_wave)) =
                wave.target_and_velocity_at_tick(world.tick_count, world.hz)
            {
                self.latch_joint_target(idx, target);
                wave_cmd_dq = Some((idx, dq_wave));
            } else {
                // Keep the Option through this tick so the finishing joint still takes the
                // wave planner path (resume at end, dq≈0). Clear on every exit below so
                // AscentStall cannot leave a finished wave stuck.
                let end = wave.end_position_rad();
                self.latch_joint_target(idx, end);
                info!(joint = %joint_name, end_rad = end, "position wave complete");
                clear_finished_wave = true;
            }
        }

        let advance_result = self.advance_planners(world, wave_cmd_dq, period);
        if clear_finished_wave {
            *world.wave = None;
        }
        advance_result
    }

    fn advance_planners(
        &mut self,
        world: &mut HoldWorld<'_>,
        wave_cmd_dq: Option<(usize, f64)>,
        period: Duration,
    ) -> Result<(), HoldError> {
        let Some(targets) = self.setpoints.clone() else {
            return Ok(());
        };
        if self.planners.is_none() {
            self.init_planners(world.q, world.tick_count);
            // Seed dq filter from measured velocities on first planner init.
            if let Some(filtered) = self.dq_filtered.as_mut() {
                for (i, dq) in world.dq_meas.iter().enumerate() {
                    filtered[i] = *dq;
                }
            }
        }
        self.init_latch_state();
        for (i, jp) in world.joints.iter().enumerate() {
            self.set_joint_law(i, jp.law.kind());
        }

        let dq_filtered: Vec<f64> = (0..self.n_joints)
            .map(|i| self.filtered_dq(i, world.dq_meas[i]))
            .collect();

        let v_max_caps: Vec<f64> = (0..self.n_joints)
            .map(|i| {
                let jp = &world.joints[i];
                let move_dist = (targets[i] - world.q[i]).abs();
                let planner_speed = self
                    .planners
                    .as_ref()
                    .and_then(|planners| planners.get(i))
                    .map(|planner| planner.dq_traj.abs())
                    .unwrap_or(0.0);
                Self::clamp_v_max(
                    jp.velocity_cap,
                    position_hold_v_max(
                        move_dist,
                        jp.slew_rad_s,
                        jp.trajectory_v_max,
                        jp.trajectory_threshold_rad,
                        planner_speed,
                    ),
                )
            })
            .collect();

        let retarget_age_ms: Vec<u64> = (0..self.n_joints)
            .map(|i| self.retarget_age_ms(i, world.tick_count, world.hz))
            .collect();

        let Some(planners) = self.planners.as_mut() else {
            return Ok(());
        };
        if self.planner_events.is_none() {
            self.planner_events = Some(vec![PlannerEvent::Tick; self.n_joints]);
        }
        // Finished waves stay Some until advance returns so this index still matches.
        let wave_joint_index = world.wave.as_ref().map(|w| w.joint_index);
        let dt = world.dt;
        self.tick_stuck_residual.fill(false);

        for i in 0..self.n_joints {
            let name = &world.joint_names[i];
            let jp = &world.joints[i];
            let mut event = PlannerEvent::Tick;
            if wave_joint_index == Some(i) {
                let target = targets[i];
                let dq_raw = wave_cmd_dq
                    .filter(|(ji, _)| *ji == i)
                    .map(|(_, d)| d)
                    .unwrap_or(0.0);
                // The wave was admitted against the velocity cap and the trajectory speed
                // (`validate_wave_motion`); cap its velocity by those only. `v_max_caps` would
                // pick the slew speed, because each wave sample is within the trajectory
                // threshold of q, and pin the feed-forward there (bench 2026-10-04).
                let cap = Self::clamp_v_max(jp.velocity_cap, jp.trajectory_v_max);
                let dq = dq_raw.clamp(-cap, cap);
                planners[i].resume_cruise_toward(target, dq);
                // Wave owns the joint — do not carry a pre-wave AscentStall fuse across.
                Self::set_bool_at(&mut self.planner_frozen, i, false);
                if let Some(recovery) = self
                    .ascent_recovery
                    .as_mut()
                    .and_then(|states| states.get_mut(i))
                {
                    *recovery = AscentRecovery::default();
                }
                if self.commanded_joints[i] {
                    self.wave_motion[i].update(
                        world.q[i],
                        dq_raw,
                        self.progress_thresholds[i],
                        period,
                    );
                } else {
                    self.wave_motion[i] = WaveMotionWatch::default();
                }
                let settle = targets[i] - world.q[i];
                let approaching = planners[i].dq_traj * settle > POSITION_HOLD_ERROR_DEADBAND_RAD;
                self.tick_effective_max_lead[i] = position_hold_effective_max_lead(
                    jp.max_lead,
                    retarget_age_ms[i],
                    approaching,
                    settle,
                    world.q[i],
                );
                if let Some(events) = self.planner_events.as_mut() {
                    events[i] = event;
                }
                // The wave sample is the reference; the governor does not scale it.
                self.scaled[i].s = 1.0;
                self.scaled[i].v_c = planners[i].dq_traj;
                continue;
            }

            if let HoldLaw::ScaledPd(gains) = &jp.law {
                self.wave_motion[i] = WaveMotionWatch::default();
                Self::set_bool_at(&mut self.planner_frozen, i, false);
                let outbound = if self.commanded_joints[i] {
                    outbound_stall_direction(targets[i], targets[i] - world.q[i])
                } else {
                    None
                };
                // Budget only; compose trips the fuse once it knows this tick's torques.
                if let Some(recovery) = self
                    .ascent_recovery
                    .as_mut()
                    .and_then(|states| states.get_mut(i))
                {
                    recovery.update(
                        false,
                        outbound,
                        world.q[i],
                        self.progress_thresholds[i],
                        period,
                    );
                }
                let planner = &mut planners[i];
                let v_max = Self::scaled_v_max(jp, planner, targets[i]);
                // reference_step changes speed by at most a_max·dt, so entering or leaving the
                // cap is acceleration-limited (no velocity step at the threshold).
                let v_max = jp.descent_cap.as_ref().map_or(v_max, |cap| {
                    cap.limit(v_max, planner.q_traj, world.q[i], targets[i])
                });
                let v_tick = jp
                    .limit_policy
                    .as_ref()
                    .map(|policy| approach_velocity_cap(policy, world.q[i], planner.dq_traj, v_max))
                    .unwrap_or(v_max);
                let (q_ref, v_ref, phase) = advance_reference(
                    &mut self.scaled[i],
                    gains,
                    planner.q_traj,
                    planner.dq_traj,
                    world.q[i],
                    targets[i],
                    v_tick,
                    jp.a_max,
                    dt,
                );
                planner.set_reference(q_ref, v_ref, phase);
                self.tick_effective_max_lead[i] = gains.e1;
                if let Some(events) = self.planner_events.as_mut() {
                    events[i] = event;
                }
                continue;
            }

            self.wave_motion[i] = WaveMotionWatch::default();
            let v_max = v_max_caps[i];
            let move_dist = (targets[i] - world.q[i]).abs();
            trace!(
                joint = %name,
                move_dist,
                v_max_eff = v_max,
                "position hold profile"
            );

            let settle_error_eff = targets[i] - world.q[i];
            let approaching_target_eff =
                planners[i].dq_traj * settle_error_eff > POSITION_HOLD_ERROR_DEADBAND_RAD;
            let effective_max_lead = position_hold_effective_max_lead(
                jp.advance_max_lead,
                retarget_age_ms[i],
                approaching_target_eff,
                settle_error_eff,
                world.q[i],
            );
            if planner_should_resync_stuck_lead(
                &planners[i],
                world.q[i],
                targets[i],
                dq_filtered[i],
                effective_max_lead,
                jp.advance_vel_deadband,
            ) {
                event = PlannerEvent::ResyncStuckLead;
                Self::reset_planner_with_downward_seed(
                    &mut planners[i],
                    world.q[i],
                    targets[i],
                    jp.slew_rad_s,
                    v_max,
                );
            } else if planner_should_reopen_premature_hold(
                &planners[i],
                world.q[i],
                targets[i],
                dq_filtered[i],
                jp.advance_vel_deadband,
            ) {
                event = PlannerEvent::Reset;
                reopen_planner_from_premature_hold(&mut planners[i], world.q[i], targets[i], v_max);
            } else if planner_should_lead_follow_hold_short(
                &planners[i],
                world.q[i],
                targets[i],
                dq_filtered[i],
                jp.advance_vel_deadband,
            ) {
                apply_lead_follow_hold_short(
                    &mut planners[i],
                    world.q[i],
                    targets[i],
                    effective_max_lead,
                    jp.slew_rad_s,
                );
            } else if planner_drifted_from_measurement(
                &planners[i],
                world.q[i],
                targets[i],
                effective_max_lead,
            ) {
                event = PlannerEvent::Reset;
                Self::reset_planner_with_downward_seed(
                    &mut planners[i],
                    world.q[i],
                    targets[i],
                    jp.slew_rad_s,
                    v_max,
                );
            }

            let lag = world.q[i] - planners[i].q_traj;
            let to_target = targets[i] - world.q[i];
            let dq_f = dq_filtered[i];
            let was_planner_frozen = Self::bool_at(&self.planner_frozen, i);
            let was_ascent_recovering = self
                .ascent_recovery
                .as_ref()
                .and_then(|states| states.get(i).copied())
                .unwrap_or_default()
                .is_active();
            let freeze_descent = planner_should_freeze_on_descent(
                was_planner_frozen,
                targets[i],
                world.q[i],
                to_target,
                lag,
                planners[i].dq_traj,
                dq_f,
                jp.advance_vel_deadband,
                jp.advance_max_lead,
            );
            let lead_follow = planner_should_lead_follow_hold_short(
                &planners[i],
                world.q[i],
                targets[i],
                dq_f,
                jp.advance_vel_deadband,
            );
            let stuck_residual = lead_follow_stuck_residual(
                lead_follow,
                world.q[i],
                targets[i],
                dq_f,
                jp.advance_vel_deadband,
            );
            self.tick_stuck_residual[i] = stuck_residual;
            let ascent_recovering = (!lead_follow || stuck_residual)
                && planner_should_recover_ascent_stall(
                    was_ascent_recovering,
                    targets[i],
                    world.q[i],
                    planners[i].q_traj,
                    to_target,
                    dq_f,
                    jp.advance_vel_deadband,
                );
            let freeze = freeze_descent || (lead_follow && !stuck_residual);
            if freeze && !was_planner_frozen {
                event = PlannerEvent::FreezeEnter;
            } else if was_planner_frozen && !freeze {
                event = PlannerEvent::FreezeExit;
            }
            Self::set_bool_at(&mut self.planner_frozen, i, freeze);
            // Home targets are latched as exactly 0.0 (`home_classified`), so this outbound
            // classification does not hinge on a single feedback count near zero.
            let outbound = if self.commanded_joints[i] {
                outbound_stall_direction(targets[i], to_target)
            } else {
                None
            };
            // Budget only; compose trips the fuse once it knows this tick's torques.
            if let Some(recovery) = self
                .ascent_recovery
                .as_mut()
                .and_then(|states| states.get_mut(i))
            {
                recovery.update(
                    ascent_recovering,
                    outbound,
                    world.q[i],
                    self.progress_thresholds[i],
                    period,
                );
            }
            if planner_should_latch_on_overshoot_hold(
                world.q[i],
                planners[i].q_traj,
                targets[i],
                dq_f,
                jp.advance_vel_deadband,
            ) {
                planners[i].latch_at_target(targets[i]);
                event = PlannerEvent::Latch;
            }
            if !freeze {
                let v_tick = jp
                    .limit_policy
                    .as_ref()
                    .map(|policy| {
                        approach_velocity_cap(policy, world.q[i], planners[i].dq_traj, v_max)
                    })
                    .unwrap_or(v_max);
                planners[i].tick(targets[i], dt, v_tick, jp.a_max);
                if let Some(ref policy) = jp.limit_policy {
                    let clamped = clamp_position_in_envelope(
                        policy,
                        world.q[i],
                        planners[i].dq_traj,
                        planners[i].q_traj,
                    );
                    if (clamped - planners[i].q_traj).abs() > 1e-9 {
                        event = PlannerEvent::EnvelopeClamp;
                    }
                    planners[i].q_traj = clamped;
                }
            }
            // Tag recovery ticks for the position trace / 1 Hz hold log so operators can tell
            // Berthier policy from a Davout limit trip. Resync / latch / envelope keep priority.
            if ascent_recovering && matches!(event, PlannerEvent::Tick) {
                event = PlannerEvent::AscentBreakaway;
            }
            // Compose uses post-advance planner state (same tick) — one effective lead.
            let settle = targets[i] - world.q[i];
            let approaching = planners[i].dq_traj * settle > POSITION_HOLD_ERROR_DEADBAND_RAD;
            self.tick_effective_max_lead[i] = position_hold_effective_max_lead(
                jp.max_lead,
                retarget_age_ms[i],
                approaching,
                settle,
                world.q[i],
            );
            if let Some(events) = self.planner_events.as_mut() {
                events[i] = event;
            }
        }
        Ok(())
    }

    fn compose(
        &mut self,
        world: &HoldWorld<'_>,
        period: Duration,
    ) -> Result<HoldTickOut, HoldError> {
        let n = self.n_joints;
        let mut mit = Vec::with_capacity(n);
        let mut diag = Vec::with_capacity(n);
        self.init_latch_state();
        let wave_joint_index = world.wave.as_ref().map(|w| w.joint_index);

        for i in 0..n {
            let name = &world.joint_names[i];
            let jp = &world.joints[i];
            let (q_traj, dq_traj, traj_phase) = {
                let planner = self
                    .planners
                    .as_ref()
                    .and_then(|p| p.get(i))
                    .ok_or_else(|| HoldError::MissingSetpoint {
                        joint: name.clone(),
                    })?;
                (planner.q_traj, planner.dq_traj, planner.phase())
            };
            let dq_raw = world.dq_meas[i];
            let dq = self
                .dq_filtered
                .as_ref()
                .and_then(|f| f.get(i).copied())
                .unwrap_or(dq_raw);
            let target = self
                .setpoints
                .as_deref()
                .and_then(|sp| sp.get(i).copied())
                .unwrap_or(q_traj);
            let target_raw = self
                .setpoints_raw
                .as_ref()
                .and_then(|r| r.get(i).copied())
                .unwrap_or(target);
            let retarget_age_ms = self.retarget_age_ms(i, world.tick_count, world.hz);
            let settle_error = target - world.q[i];
            let approaching_target = dq_traj * settle_error > POSITION_HOLD_ERROR_DEADBAND_RAD;
            let effective_max_lead = self.tick_effective_max_lead[i];
            let move_dist = (target - world.q[i]).abs();
            let v_max_eff = Self::clamp_v_max(
                jp.velocity_cap,
                position_hold_v_max(
                    move_dist,
                    jp.slew_rad_s,
                    jp.trajectory_v_max,
                    jp.trajectory_threshold_rad,
                    dq_traj.abs(),
                ),
            );

            if let HoldLaw::ScaledPd(gains) = jp.law {
                let joint = ScaledJointTick {
                    index: i,
                    gains,
                    q_ref: q_traj,
                    phase: traj_phase,
                    target,
                    target_raw,
                    retarget_age_ms,
                    move_dist,
                    wave_owned: wave_joint_index == Some(i),
                };
                let (command, joint_diag) = self.compose_scaled_joint(world, period, joint)?;
                mit.push(command);
                diag.push(joint_diag);
                continue;
            }

            let mut breakaway = Self::bool_at(&self.descent_breakaway, i);
            let limit_policy = jp.limit_policy.as_ref();
            let mut q_des = clamp_trajectory_setpoint(
                q_traj,
                world.q[i],
                target,
                effective_max_lead,
                limit_policy,
                dq_traj,
            );
            let to_target = target - world.q[i];
            let stuck_residual = self.tick_stuck_residual.get(i).copied().unwrap_or(false);
            let stuck_now =
                descent_stuck_mit_pull(to_target, world.q[i], target, dq, jp.vel_deadband, false);
            if stuck_now {
                Self::set_bool_at(&mut self.descent_was_stuck, i, true);
            }
            let was_stuck = Self::bool_at(&self.descent_was_stuck, i);
            if !breakaway
                && was_stuck
                && descent_breakaway_confirmed(to_target, dq, jp.vel_deadband)
            {
                breakaway = true;
                Self::set_bool_at(&mut self.descent_breakaway, i, true);
            }
            let joint_stuck = stuck_now && !breakaway;
            if joint_stuck {
                let stuck_pull = home_final_approach_stuck_pull_rad(world.q[i], target);
                q_des = q_des.min(world.q[i] - stuck_pull);
            }
            if stuck_residual {
                let pull_through = (target + POSITION_HOME_FINAL_PULL_THROUGH_RAD)
                    .min(world.q[i] + effective_max_lead);
                q_des = q_des.max(pull_through);
            }
            if let Some(policy) = limit_policy {
                q_des = clamp_position_in_envelope(policy, world.q[i], dq_traj, q_des);
            }
            let planner_frozen = Self::bool_at(&self.planner_frozen, i);
            let lead = q_des - world.q[i];
            let settling = matches!(traj_phase, TrapezoidPhase::Hold)
                && settle_error.abs() <= POSITION_SETTLE_TOLERANCE_RAD;
            let sustained_low_angle_breakaway = approaching_target
                && low_angle_breakaway_active(world.q[i], target, settle_error, approaching_target)
                && dq.abs() < jp.vel_deadband;
            let ff = compose_position_hold_feedforward(
                world.tau_g[i],
                jp.kd,
                dq,
                dq_traj,
                settle_error,
                jp.vel_deadband,
                effective_max_lead,
                retarget_age_ms,
                traj_phase,
                jp.friction.as_ref(),
                approaching_target,
                sustained_low_angle_breakaway,
            );
            let mut tau_ff_cmd = ff.tau_ff_cmd;
            if jp.ki > 0.0 && settle_error.abs() < 0.1 && retarget_age_ms > 1000 {
                if let Some(integral) = self.integral_error.as_mut() {
                    integral[i] = (integral[i] + settle_error * world.dt)
                        .clamp(-MAX_INTEGRAL_NM / jp.ki, MAX_INTEGRAL_NM / jp.ki);
                    tau_ff_cmd += jp.ki * integral[i];
                }
            } else if let Some(integral) = self.integral_error.as_mut() {
                // Outside its window the integral is not applied; it must not survive to
                // re-apply a stale torque on re-entry.
                integral[i] = 0.0;
            }
            let lead_sat = lead.abs() >= effective_max_lead - 1e-6;
            let tau_p = jp.kp * lead;
            let trip = HoldFuseTrip {
                q: world.q[i],
                target,
                tau_p,
                tau_ff: tau_ff_cmd,
                tau_g: world.tau_g[i],
            };
            let ascent_stall_ms = self
                .ascent_recovery
                .as_ref()
                .and_then(|v| v.get(i).copied())
                .unwrap_or_default()
                .stalled_ms();
            if ascent_stall_ms >= POSITION_ASCENT_STALL_FAULT_MS {
                return Err(HoldError::AscentStall {
                    joint: name.clone(),
                    ms: ascent_stall_ms,
                    trip,
                });
            }
            let wave_stall_ms = self.wave_motion[i].stalled_ms();
            if wave_stall_ms >= POSITION_ASCENT_STALL_FAULT_MS {
                return Err(HoldError::WaveStall {
                    joint: name.clone(),
                    ms: wave_stall_ms,
                    trip,
                });
            }
            // Symmetric to the ascent fuse and independent of home/outbound classification:
            // a latched hold that sags (or is pushed) off target by its own net command.
            let tracking_armed = self.commanded_joints[i]
                && wave_joint_index != Some(i)
                && hold_tracking_opposed(world.q[i], target, tau_p + tau_ff_cmd);
            let hold_tracking_ms = self.hold_tracking[i].update(
                tracking_armed,
                world.q[i],
                target,
                self.progress_thresholds[i],
                period,
            );
            if hold_tracking_ms >= POSITION_ASCENT_STALL_FAULT_MS {
                return Err(HoldError::HoldTracking {
                    joint: name.clone(),
                    ms: hold_tracking_ms,
                    trip,
                });
            }
            let mit_velocity = position_hold_mit_velocity(
                dq,
                dq_traj,
                jp.vel_deadband,
                retarget_age_ms,
                approaching_target,
            );
            let mit_kd = if settle_error.abs() < 0.1 {
                position_hold_mit_kd(jp.kd, dq, jp.vel_deadband)
            } else {
                0.0
            };
            let planner_event = self
                .planner_events
                .as_ref()
                .and_then(|e| e.get(i).copied())
                .unwrap_or(PlannerEvent::Tick);
            let (q_env_lo, q_env_hi) = limit_policy
                .map(|p| effective_command_bounds(p, world.q[i], dq_traj))
                .unwrap_or((f64::NAN, f64::NAN));
            let retarget_tick = self
                .retarget_tick
                .as_ref()
                .and_then(|ticks| ticks.get(i).copied());

            diag.push(HoldJointDiag {
                move_dist,
                v_max_eff,
                planner_event,
                q: world.q[i],
                dq_raw,
                dq_filt: dq,
                q_traj,
                dq_traj,
                q_des,
                target,
                target_raw,
                lead,
                lead_sat,
                settle_error,
                settling,
                friction_mode: ff.friction_mode.as_str(),
                retarget_age_ms,
                joint_stuck,
                planner_frozen,
                phase: traj_phase.as_str(),
                kp: jp.kp,
                kd: jp.kd,
                tau_g: world.tau_g[i],
                tau_f: ff.tau_f,
                tau_d: ff.tau_d,
                tau_ff_cmd,
                tau_meas: jp.tau_meas,
                tau_p,
                mit_velocity,
                mit_kd,
                q_env_lo,
                q_env_hi,
                retarget_tick,
                ascent_stall_ms,
                hold_tracking_ms,
                law: PositionLaw::Legacy,
                q_ref: q_traj,
                dq_ref: dq_traj,
                time_scale: if planner_frozen { 0.0 } else { 1.0 },
                tau_i: tau_ff_cmd - ff.tau_ff_cmd,
            });
            mit.push(DavoutMit {
                joint: name.clone(),
                kp: jp.kp,
                kd: mit_kd,
                position_rad: q_des,
                velocity_rad_s: mit_velocity,
                torque_ff_nm: tau_ff_cmd,
            });
        }

        Ok(HoldTickOut { mit, diag })
    }

    /// ADR 0039 scaled-PD compose for one joint: drive-side PD on the reference, feed-forward
    /// from the reference velocity, the leaky integral, and the three fuses.
    fn compose_scaled_joint(
        &mut self,
        world: &HoldWorld<'_>,
        period: Duration,
        joint: ScaledJointTick,
    ) -> Result<(DavoutMit, HoldJointDiag), HoldError> {
        let i = joint.index;
        let name = &world.joint_names[i];
        let jp = &world.joints[i];
        let q = world.q[i];
        let dq_raw = world.dq_meas[i];
        let target = joint.target;
        let settle_error = target - q;
        let state = &mut self.scaled[i];
        let ff = compose_feedforward(
            state,
            &joint.gains,
            jp.ki,
            settle_error,
            world.tau_g[i],
            world.dt,
        );
        let v_c = state.v_c;
        let time_scale = state.s;
        let limit_policy = jp.limit_policy.as_ref();
        let mut planner_event = PlannerEvent::Tick;
        let q_des = match limit_policy {
            Some(policy) => {
                let clamped = clamp_position_in_envelope(policy, q, v_c, joint.q_ref);
                if (clamped - joint.q_ref).abs() > 1e-9 {
                    planner_event = PlannerEvent::EnvelopeClamp;
                }
                clamped
            }
            None => joint.q_ref,
        };
        let lead = q_des - q;
        let tau_p = jp.kp * lead;
        let kd = joint.gains.kd;
        let trip = HoldFuseTrip {
            q,
            target,
            tau_p,
            tau_ff: ff.tau_ff,
            tau_g: world.tau_g[i],
        };
        let ascent_stall_ms = self
            .ascent_recovery
            .as_ref()
            .and_then(|v| v.get(i).copied())
            .unwrap_or_default()
            .stalled_ms();
        if ascent_stall_ms >= POSITION_ASCENT_STALL_FAULT_MS {
            return Err(HoldError::AscentStall {
                joint: name.clone(),
                ms: ascent_stall_ms,
                trip,
            });
        }
        let wave_stall_ms = self.wave_motion[i].stalled_ms();
        if wave_stall_ms >= POSITION_ASCENT_STALL_FAULT_MS {
            return Err(HoldError::WaveStall {
                joint: name.clone(),
                ms: wave_stall_ms,
                trip,
            });
        }
        // ADR 0039: the net commanded torque is the wire terms, including the drive's damping.
        let net_commanded = tau_p + kd * (v_c - dq_raw) + ff.tau_ff;
        let tracking_armed = self.commanded_joints[i]
            && !joint.wave_owned
            && hold_tracking_opposed(q, target, net_commanded);
        let hold_tracking_ms = self.hold_tracking[i].update(
            tracking_armed,
            q,
            target,
            self.progress_thresholds[i],
            period,
        );
        if hold_tracking_ms >= POSITION_ASCENT_STALL_FAULT_MS {
            return Err(HoldError::HoldTracking {
                joint: name.clone(),
                ms: hold_tracking_ms,
                trip,
            });
        }
        let (q_env_lo, q_env_hi) = limit_policy
            .map(|p| effective_command_bounds(p, q, v_c))
            .unwrap_or((f64::NAN, f64::NAN));
        let planner_event = match self.planner_events.as_ref().and_then(|e| e.get(i).copied()) {
            Some(event) if event != PlannerEvent::Tick => event,
            _ => planner_event,
        };
        let diag = HoldJointDiag {
            move_dist: joint.move_dist,
            v_max_eff: self
                .planners
                .as_ref()
                .and_then(|p| p.get(i))
                .map_or(0.0, |planner| Self::scaled_v_max(jp, planner, target)),
            planner_event,
            q,
            dq_raw,
            dq_filt: dq_raw,
            q_traj: joint.q_ref,
            dq_traj: v_c,
            q_des,
            target,
            target_raw: joint.target_raw,
            lead,
            lead_sat: lead.abs() >= joint.gains.e1 - 1e-6,
            settle_error,
            settling: matches!(joint.phase, TrapezoidPhase::Hold)
                && settle_error.abs() <= POSITION_SETTLE_TOLERANCE_RAD,
            friction_mode: "reference",
            retarget_age_ms: joint.retarget_age_ms,
            joint_stuck: false,
            planner_frozen: false,
            phase: joint.phase.as_str(),
            kp: jp.kp,
            kd,
            tau_g: world.tau_g[i],
            tau_f: ff.tau_fric,
            tau_d: 0.0,
            tau_ff_cmd: ff.tau_ff,
            tau_meas: jp.tau_meas,
            tau_p,
            mit_velocity: v_c,
            mit_kd: kd,
            q_env_lo,
            q_env_hi,
            retarget_tick: self
                .retarget_tick
                .as_ref()
                .and_then(|ticks| ticks.get(i).copied()),
            ascent_stall_ms,
            hold_tracking_ms,
            law: PositionLaw::ScaledPd,
            q_ref: joint.q_ref,
            dq_ref: v_c,
            time_scale,
            tau_i: ff.tau_i,
        };
        let command = DavoutMit {
            joint: name.clone(),
            kp: jp.kp,
            kd,
            position_rad: q_des,
            velocity_rad_s: v_c,
            torque_ff_nm: ff.tau_ff,
        };
        Ok((command, diag))
    }
}

#[cfg(test)]
#[path = "position_hold_tests/encoder_stop.rs"]
mod stall_progress_tests;

#[cfg(test)]
#[path = "position_hold_tests/progress_matrix.rs"]
mod progress_matrix_tests;

#[cfg(test)]
#[path = "position_hold_tests/period_contract.rs"]
mod period_contract_tests;

#[cfg(test)]
#[path = "position_hold_tests/numeric_contract.rs"]
mod numeric_contract_tests;

#[cfg(test)]
#[path = "position_hold_tests/hold_tracking.rs"]
mod hold_tracking_tests;

#[cfg(test)]
#[path = "position_hold_tests/fuse_audit.rs"]
mod fuse_audit_tests;

#[cfg(test)]
#[path = "position_hold_tests/law_gates.rs"]
mod law_gates;

#[cfg(test)]
#[path = "position_hold_tests/bench_replay.rs"]
mod bench_replay;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::position_setpoint::POSITION_RETURN_RESYNC_RAD;

    fn test_joint_params() -> HoldJointParams {
        HoldJointParams {
            kp: 8.0,
            kd: 1.25,
            ki: 0.0,
            max_lead: 0.15,
            vel_deadband: 0.02,
            advance_max_lead: ADVANCE_MAX_LEAD_DEFAULT,
            advance_vel_deadband: POSITION_HOLD_ERROR_DEADBAND_RAD,
            slew_rad_s: 0.15,
            trajectory_v_max: 2.0,
            trajectory_threshold_rad: 0.15,
            a_max: 4.8,
            velocity_cap: Some(2.0),
            friction: None,
            limit_policy: None,
            tau_meas: 0.0,
            law: HoldLaw::Legacy,
            descent_cap: None,
        }
    }

    /// Bench 2026-10-04T07:36:40Z: an elbow retarget 0.75 → 0.5 stepped `q_des` to the target
    /// once the planner came within `max_lead` of it (overshoot snap judged by target sign).
    /// With the arm following the planner, `q_des` must stay continuous and lead-bounded.
    #[test]
    fn descent_toward_positive_target_never_steps_q_des() {
        use crate::position_setpoint::{
            POSITION_DESCENT_STUCK_LEAD_RAD, POSITION_HOLD_ONSET_MAX_LEAD_RAD,
        };

        let params = [HoldJointParams {
            kp: 12.0,
            kd: 1.5,
            max_lead: 0.10,
            advance_max_lead: 0.10,
            advance_vel_deadband: 0.02,
            slew_rad_s: 0.12,
            trajectory_v_max: 1.43,
            a_max: 2.5,
            velocity_cap: Some(1.5),
            ..test_joint_params()
        }];
        let names = [String::from("right_elbow_pitch")];
        let dt = 0.005;
        let (start, target) = (0.75, 0.5);
        let mut hold = PositionHold::new(1);
        hold.arm(&[start], &[start], 0);
        assert!(hold.apply_retarget(HoldRetarget {
            joint_idx: 0,
            clamped: target,
            requested: target,
            q: start,
            tick: 1,
            dq_seed: Some(0.0),
            downward_seed: Some(downward_return_seed_velocity(0.12, 1.43, start, target)),
        }));
        let mut q = start;
        let mut dq = 0.0;
        let mut last_q_des: Option<f64> = None;
        let mut wave = None;
        for tick in 1..400u64 {
            let world = HoldWorld {
                q: &[q],
                dq_meas: &[dq],
                tau_g: &[0.0],
                joints: &params,
                joint_names: &names,
                dt,
                hz: 200,
                tick_count: tick,
                wave: &mut wave,
            };
            let out = hold.tick(world).unwrap();
            let q_des = out.mit[0].position_rad;
            assert!(
                (q_des - q).abs() <= POSITION_HOLD_ONSET_MAX_LEAD_RAD + 1e-9,
                "tick {tick}: lead {} exceeds the lead bound",
                q_des - q
            );
            if let Some(last) = last_q_des {
                // The descent stuck pull (`q − 0.03` while `dq` is still zero) releases in one
                // step of at most its own lead; the snap stepped by up to the boosted lead.
                assert!(
                    (q_des - last).abs() <= POSITION_DESCENT_STUCK_LEAD_RAD,
                    "tick {tick}: q_des stepped {last:.4} → {q_des:.4} (q={q:.4})"
                );
            }
            last_q_des = Some(q_des);
            // Ideal follower one tick behind the planner reference.
            let q_next = hold.q_traj().unwrap()[0];
            dq = (q_next - q) / dt;
            q = q_next;
        }
        assert!((q - target).abs() < 1e-6, "follower must arrive: q={q}");
    }

    #[test]
    fn same_target_retarget_is_full_noop_on_planner_and_fuse() {
        let mut hold = PositionHold::new(1);
        hold.arm(&[0.2], &[0.8], 100);
        // Lead in (resync, advance_max_lead] keeps bounded recovery active.
        hold.force_planner_cruise_at(0, 0.26, 0.05).unwrap();
        hold.set_ascent_recovery_for_test(0, true);
        hold.set_ascent_stall_ms_for_test(0, 1500);
        let (q_traj_before, dq_before) = hold.planner_state(0).unwrap();

        assert!(!hold.apply_retarget(HoldRetarget {
            joint_idx: 0,
            clamped: 0.8,
            requested: 0.8,
            q: 0.2,
            tick: 200,
            dq_seed: Some(0.0),
            downward_seed: None,
        }));
        assert_eq!(hold.ascent_stall_ms_at(0), 1500);
        assert_eq!(hold.retarget_tick_at(0), Some(100));
        let (q_traj_after, dq_after) = hold.planner_state(0).unwrap();
        assert!(
            (q_traj_after - q_traj_before).abs() < 1e-12,
            "same-target must not reset_target"
        );
        assert!((dq_after - dq_before).abs() < 1e-12);

        let params = [test_joint_params()];
        let names = [String::from("j0")];
        let q = [0.2];
        let dq = [0.0];
        let tau_g = [0.0];
        let mut wave = None;
        let world = HoldWorld {
            q: &q,
            dq_meas: &dq,
            tau_g: &tau_g,
            joints: &params,
            joint_names: &names,
            dt: 0.005,
            hz: 200,
            tick_count: 201,
            wave: &mut wave,
        };
        let _ = hold.tick(world).unwrap();
        assert!(
            hold.ascent_stall_ms_at(0) >= 1500,
            "fuse must survive same-target retarget + tick; got {}",
            hold.ascent_stall_ms_at(0)
        );

        assert!(hold.apply_retarget(HoldRetarget {
            joint_idx: 0,
            clamped: 1.0,
            requested: 1.0,
            q: 0.2,
            tick: 300,
            dq_seed: Some(0.0),
            downward_seed: None,
        }));
        assert!(
            hold.ascent_stall_ms_at(0) >= 1500,
            "a true retarget ends the planner-recovery policy episode but never renews the budget"
        );
        assert_eq!(hold.retarget_tick_at(0), Some(300));
    }

    #[test]
    fn finished_wave_cleared_inside_advance_before_ascent_stall() {
        // Joint 0: finished wave. Joint 1: AscentStall — proves clear happens before Err.
        let mut hold = PositionHold::new(2);
        hold.arm(&[0.0, 0.2], &[0.5, 0.8], 0);
        // half_period=1, cycles=1 → done after 2 ticks; tick_count=10 is finished.
        let mut wave = Some(PositionWave::new(0, 0.0, 0.2, 0, 1, 1));
        let params = [test_joint_params(), test_joint_params()];
        let names = [String::from("j0"), String::from("j1")];
        let q = [0.0, 0.2];
        let dq = [0.0, 0.0];
        let tau_g = [0.0, 0.0];
        // Lead 0.06 rad: above resync exit, below advance_max_lead drift reset.
        hold.force_planner_cruise_at(1, 0.26, 0.05).unwrap();
        hold.set_ascent_recovery_for_test(1, true);
        hold.set_ascent_stall_ms_for_test(1, POSITION_ASCENT_STALL_FAULT_MS);
        let world = HoldWorld {
            q: &q,
            dq_meas: &dq,
            tau_g: &tau_g,
            joints: &params,
            joint_names: &names,
            dt: 0.005,
            hz: 200,
            tick_count: 10,
            wave: &mut wave,
        };
        let err = hold.tick(world).unwrap_err();
        assert!(matches!(err, HoldError::AscentStall { joint, .. } if joint == "j1"));
        assert!(
            wave.is_none(),
            "finished wave must be cleared before AscentStall returns"
        );
        let (q_traj0, dq_traj0) = hold.planner_state(0).unwrap();
        assert!(
            (q_traj0 - 0.0).abs() < 1e-9 && dq_traj0.abs() < 1e-9,
            "finish tick must keep wave path (resume at end, dq≈0); got q_traj={q_traj0} dq={dq_traj0}"
        );
    }

    #[test]
    fn apply_retarget_unarmed_arms_without_desync() {
        let mut hold = PositionHold::new(1);
        assert!(hold.apply_retarget(HoldRetarget {
            joint_idx: 0,
            clamped: 0.3,
            requested: 0.35,
            q: 0.0,
            tick: 1,
            dq_seed: Some(0.07),
            downward_seed: None,
        }));
        assert!(hold.is_armed());
        assert_eq!(hold.targets().unwrap()[0], 0.3);
        assert_eq!(hold.setpoints_raw.as_ref().unwrap()[0], 0.35);
        assert!(
            (hold.dq_filtered_at(0).unwrap() - 0.07).abs() < 1e-12,
            "unarmed retarget must apply dq_seed"
        );
        let (q_traj, _) = hold.planner_state(0).unwrap();
        assert!(
            (q_traj - 0.0).abs() < 1e-12,
            "unarmed retarget must sync planner to measured q"
        );
    }

    #[test]
    fn compose_uses_max_lead_not_advance_max_lead_for_effective_lead() {
        // Advance allows a larger lead so q_traj survives; compose must clamp with max_lead.
        // Past onset window so max_lead is not boosted to POSITION_HOLD_ONSET_MAX_LEAD_RAD.
        let mut hold = PositionHold::new(1);
        hold.arm(&[0.0], &[0.5], 0);
        hold.force_planner_cruise_at(0, 0.12, 0.05).unwrap();
        let mut params = test_joint_params();
        params.advance_max_lead = 0.20;
        params.max_lead = 0.05;
        let names = [String::from("j0")];
        let q = [0.0];
        let dq = [0.0];
        let tau_g = [0.0];
        let mut wave = None;
        let tick_count = 100; // 500 ms at 200 Hz > onset
        let world = HoldWorld {
            q: &q,
            dq_meas: &dq,
            tau_g: &tau_g,
            joints: &[params.clone()],
            joint_names: &names,
            dt: 0.0,
            hz: 200,
            tick_count,
            wave: &mut wave,
        };
        let out = hold.tick(world).unwrap();
        let age_ms = hold.retarget_age_ms(0, tick_count, 200);
        let expected = position_hold_effective_max_lead(params.max_lead, age_ms, true, 0.5, 0.0);
        assert!(
            (hold.tick_effective_max_lead_at(0) - expected).abs() < 1e-12,
            "compose lead source must be max_lead; got {} want {}",
            hold.tick_effective_max_lead_at(0),
            expected
        );
        assert!(
            (expected - 0.05).abs() < 1e-12,
            "test requires unboosted max_lead=0.05; got {expected}"
        );
        let (q_traj, _) = hold.planner_state(0).unwrap();
        assert!(
            (q_traj - 0.12).abs() < 1e-9,
            "advance must keep q_traj under advance_max_lead; got {q_traj}"
        );
        assert!(
            (out.diag[0].q_des - 0.05).abs() < 1e-6,
            "compose must clamp q_des with max_lead=0.05, not advance lead; got {}",
            out.diag[0].q_des
        );
        const _: () = assert!(ADVANCE_MAX_LEAD_DEFAULT < 0.15);
    }

    #[test]
    fn ascent_stall_counter_resets_on_independent_measured_progress() -> Result<(), HoldError> {
        let mut hold = PositionHold::new(1);
        hold.arm(&[0.02], &[0.15], 0);
        let params = [test_joint_params()];
        let names = [String::from("j0")];
        let tau_g = [0.0];
        let mut wave = None;
        let mut largest_stall_ms = 0;
        for tick_count in 1..=100 {
            let out = hold.tick(HoldWorld {
                q: &[0.02],
                dq_meas: &[0.0],
                tau_g: &tau_g,
                joints: &params,
                joint_names: &names,
                dt: 0.005,
                hz: 200,
                tick_count,
                wave: &mut wave,
            })?;
            largest_stall_ms = largest_stall_ms.max(out.diag[0].ascent_stall_ms);
        }
        assert!(
            largest_stall_ms > 0,
            "stationary observations must arm the actual recovery fuse"
        );
        for step in 1_u32..=500 {
            // Independent measured trajectory: 0.01 rad/s over 5 ms steps.
            // The production law sees coherent motion, not its own q_traj echo.
            let position = 0.02 + f64::from(step) * 0.00005;
            let out = hold.tick(HoldWorld {
                q: &[position],
                dq_meas: &[0.01],
                tau_g: &tau_g,
                joints: &params,
                joint_names: &names,
                dt: 0.005,
                hz: 200,
                tick_count: 100 + u64::from(step),
                wave: &mut wave,
            })?;
            assert_eq!(
                out.diag[0].ascent_stall_ms, 0,
                "measured progress must reset the fuse on step {step}"
            );
            assert!(out.mit[0].position_rad.is_finite());
        }
        // Covers 2.5 s of controller time, beyond the 2 s no-progress fault
        // threshold, without introducing a host sleep or receive-time shortcut.
        Ok(())
    }

    #[test]
    fn outbound_ascent_stall_retries_to_lead_cap_before_fault() -> Result<(), HoldError> {
        let q = [0.02];
        let target = [0.30];
        let dq = [0.0];
        let tau_g = [0.0];
        let names = [String::from("j0")];
        let mut params = test_joint_params();
        params.max_lead = 0.12;
        params.advance_max_lead = 0.12;
        let params = [params];
        let mut hold = PositionHold::new(1);
        hold.arm(&q, &target, 0);

        let mut max_reference_lead: f64 = 0.0;
        let mut resyncs = 0_u32;
        let mut saw_breakaway = false;
        let mut fuse_armed_tick = None;
        let mut fault_tick = None;

        for tick_count in 100..700 {
            let mut wave = None;
            let world = HoldWorld {
                q: &q,
                dq_meas: &dq,
                tau_g: &tau_g,
                joints: &params,
                joint_names: &names,
                dt: 0.005,
                hz: 200,
                tick_count,
                wave: &mut wave,
            };
            match hold.tick(world) {
                Ok(out) => {
                    let (q_traj, _) = hold.planner_state(0).unwrap();
                    let reference_lead = q_traj - q[0];
                    let mit_lead = out.mit[0].position_rad - q[0];
                    max_reference_lead = max_reference_lead.max(reference_lead);
                    if out.diag[0].planner_event == PlannerEvent::ResyncStuckLead {
                        resyncs += 1;
                    }
                    if out.diag[0].planner_event == PlannerEvent::AscentBreakaway {
                        saw_breakaway = true;
                    }
                    if hold.ascent_stall_ms_at(0) > 0 && fuse_armed_tick.is_none() {
                        fuse_armed_tick = Some(tick_count);
                    }
                    assert!(
                        mit_lead <= params[0].max_lead + 1e-9,
                        "bounded recovery exceeded MIT lead cap: {mit_lead}"
                    );
                    assert_eq!(out.diag[0].ascent_stall_ms, hold.ascent_stall_ms_at(0));
                }
                Err(HoldError::AscentStall { ms, .. }) => {
                    assert!(ms >= POSITION_ASCENT_STALL_FAULT_MS);
                    fault_tick = Some(tick_count);
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        assert!(fuse_armed_tick.is_some(), "recovery fuse must arm");
        assert!(
            fault_tick.is_some(),
            "true outbound stall must still fail closed"
        );
        let fuse_armed_tick = fuse_armed_tick.unwrap();
        let fault_tick = fault_tick.unwrap();
        assert!(
            max_reference_lead > POSITION_RETURN_RESYNC_RAD * 2.0,
            "recovery never crossed the former freeze band: {max_reference_lead}"
        );
        assert!(resyncs >= 1, "recovery never resynced at the lead cap");
        assert!(saw_breakaway, "recovery ticks must tag ascent_breakaway");
        // 2 s at 200 Hz is 400 ticks after fuse arm. If resync cleared the fuse, the
        // fault would land hundreds of ticks later (each re-ramp is ~tens of ticks).
        assert!(
            fault_tick.saturating_sub(fuse_armed_tick) <= 410,
            "resync must not reset the recovery fuse; armed={fuse_armed_tick} faulted={fault_tick}"
        );
        Ok(())
    }

    #[test]
    fn lead_follow_stuck_residual_arms_recovery_and_pulls_past_target() {
        let q = [1.333];
        let target = [1.40];
        let dq = [0.0];
        let tau_g = [0.0];
        let params = [test_joint_params()];
        let names = [String::from("j0")];
        let mut hold = PositionHold::new(1);
        hold.arm(&q, &target, 0);
        hold.force_planner_hold_at(0, target[0]).unwrap();
        let mut wave = None;

        let out = hold
            .tick(HoldWorld {
                q: &q,
                dq_meas: &dq,
                tau_g: &tau_g,
                joints: &params,
                joint_names: &names,
                dt: 0.005,
                hz: 200,
                tick_count: 100,
                wave: &mut wave,
            })
            .unwrap();

        assert_eq!(out.diag[0].planner_event, PlannerEvent::AscentBreakaway);
        assert!(out.diag[0].ascent_stall_ms > 0);
        assert!(
            (out.diag[0].q_des - (target[0] + POSITION_HOME_FINAL_PULL_THROUGH_RAD)).abs() < 1e-9,
            "stuck residual must pull q_des past target; got {}",
            out.diag[0].q_des
        );
        assert!(
            out.diag[0].q_des - q[0] <= params[0].max_lead + 1e-9,
            "stuck residual pull must stay within max lead"
        );
    }

    #[test]
    fn ascent_recovery_releases_and_resumes_when_arm_moves() {
        let mut hold = PositionHold::new(1);
        hold.arm(&[0.20], &[0.80], 0);
        hold.force_planner_cruise_at(0, 0.26, 0.05).unwrap();

        let params = [test_joint_params()];
        let names = [String::from("j0")];
        let tau_g = [0.0];
        let q_stuck = [0.20];
        let dq_stuck = [0.0];
        for tick in 100..160 {
            let mut wave = None;
            hold.tick(HoldWorld {
                q: &q_stuck,
                dq_meas: &dq_stuck,
                tau_g: &tau_g,
                joints: &params,
                joint_names: &names,
                dt: 0.005,
                hz: 200,
                tick_count: tick,
                wave: &mut wave,
            })
            .unwrap();
        }
        assert!(
            hold.ascent_stall_ms_at(0) > 0,
            "fuse must be counting during stuck recovery"
        );

        // Breakaway: arm moves toward target above the recovery exit velocity.
        let mut q = 0.20;
        for tick in 160..=400 {
            q += 0.001;
            let q_arr = [q];
            let dq = [0.20];
            let mut wave = None;
            let out = hold
                .tick(HoldWorld {
                    q: &q_arr,
                    dq_meas: &dq,
                    tau_g: &tau_g,
                    joints: &params,
                    joint_names: &names,
                    dt: 0.005,
                    hz: 200,
                    tick_count: tick,
                    wave: &mut wave,
                })
                .unwrap();
            assert!(
                out.diag[0].q_des - q <= test_joint_params().max_lead + 1e-9,
                "commanded lead must stay bounded after release"
            );
        }
        assert_eq!(
            hold.ascent_stall_ms_at(0),
            0,
            "progress toward target must reset the fuse"
        );
        let (q_traj, dq_traj) = hold.planner_state(0).unwrap();
        assert!(
            q_traj > 0.30 && dq_traj > 0.0,
            "planner must resume advancing toward target after release; q_traj={q_traj} dq_traj={dq_traj}"
        );
    }
}
