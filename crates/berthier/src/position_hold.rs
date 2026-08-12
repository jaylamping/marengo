//! Position-hold lifecycle and per-tick control law (`ControlMode::Position`).
//!
//! [`PositionHold`] owns latched targets, planners, recovery/breakaway latches, and integral
//! state. [`ControlLoop`](crate::ControlLoop) builds a [`HoldWorld`] each tick and sends the
//! returned MIT batch through Davout.

use armee_kinematics::{
    approach_velocity_cap, clamp_position_in_envelope, effective_command_bounds, JointLimitPolicy,
};
use davout::MitJointCommand as DavoutMit;
use marengo_config::FrictionGains;
use thiserror::Error;
use tracing::{info, trace};

use crate::friction::POSITION_HOLD_ERROR_DEADBAND_RAD;
use crate::position_feedforward::compose_position_hold_feedforward;
use crate::position_profile::{position_hold_v_max, PlannerEvent};
use crate::position_setpoint::{
    apply_lead_follow_hold_short, clamp_trajectory_setpoint, descent_breakaway_confirmed,
    descent_stuck_mit_pull, downward_return_seed_velocity, home_final_approach_stuck_pull_rad,
    low_angle_breakaway_active, planner_drifted_from_measurement, planner_should_freeze_on_descent,
    planner_should_latch_on_overshoot_hold, planner_should_lead_follow_hold_short,
    planner_should_recover_ascent_stall, planner_should_reopen_premature_hold,
    planner_should_resync_stuck_lead, position_hold_effective_max_lead, position_hold_mit_kd,
    position_hold_mit_velocity, reopen_planner_from_premature_hold,
    POSITION_RETURN_DESCENT_SEED_RAD, POSITION_SETTLE_TOLERANCE_RAD,
};
use crate::position_trajectory::{
    filter_dq_ema, JointPositionPlanner, TrapezoidPhase, POSITION_DAMPING_DQ_FILTER_ALPHA,
};
use crate::position_wave::PositionWave;

/// Historical advance-path default when a joint has no `control.yaml` entry.
/// Compose used `unwrap_or(0.15)`; keep that split.
pub const ADVANCE_MAX_LEAD_DEFAULT: f64 = 0.10;

/// Sustained ascent recovery without measured progress before tick faults (disable path).
pub const POSITION_ASCENT_STALL_FAULT_MS: u64 = 2000;

const MAX_INTEGRAL_NM: f64 = 0.5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AscentRecovery {
    #[default]
    Idle,
    Active {
        stalled_ms: u64,
    },
}

impl AscentRecovery {
    fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }

    fn stalled_ms(self) -> u64 {
        match self {
            Self::Idle => 0,
            Self::Active { stalled_ms } => stalled_ms,
        }
    }

    fn update(&mut self, active: bool, progressing: bool, tick_ms: u64) -> u64 {
        if !active {
            *self = Self::Idle;
            return 0;
        }
        let stalled_ms = if progressing {
            0
        } else {
            self.stalled_ms().saturating_add(tick_ms)
        };
        *self = Self::Active { stalled_ms };
        stalled_ms
    }
}

#[derive(Debug, Error)]
pub enum HoldError {
    #[error(
        "position hold: ascent stall on {joint}: no progress during bounded recovery for {ms} ms"
    )]
    AscentStall { joint: String, ms: u64 },
    #[error("position hold: no setpoint latched for joint {joint}")]
    MissingSetpoint { joint: String },
    #[error("position hold: length mismatch")]
    LenMismatch,
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
    pub phase: String,
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
    /// Ascent-recovery fuse accumulator (ms); 0 when Idle / exempt.
    pub ascent_stall_ms: u64,
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
    descent_breakaway: Option<Vec<bool>>,
    descent_was_stuck: Option<Vec<bool>>,
    planner_events: Option<Vec<PlannerEvent>>,
    integral_error: Option<Vec<f64>>,
    /// Post-advance effective max lead for compose (FIX A: one value per joint per tick).
    tick_effective_max_lead: Vec<f64>,
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
            descent_breakaway: None,
            descent_was_stuck: None,
            planner_events: None,
            integral_error: None,
            tick_effective_max_lead: vec![0.0; n_joints],
        }
    }

    pub fn is_armed(&self) -> bool {
        self.setpoints.is_some()
    }

    pub fn targets(&self) -> Option<&[f64]> {
        self.setpoints.as_deref()
    }

    /// Operator-requested targets before envelope clamp (diagnostics / status).
    #[allow(dead_code)] // public status accessor; ControlLoop may wire later
    pub fn targets_raw(&self) -> Option<&[f64]> {
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

    /// Latch targets and initialize planners at measured `q`.
    pub fn arm(&mut self, q: &[f64], targets: &[f64], tick: u64) {
        let mut sp = targets.to_vec();
        sp.resize(self.n_joints, 0.0);
        self.setpoints_raw = Some(sp.clone());
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
    pub fn apply_retarget(&mut self, cmd: HoldRetarget) -> bool {
        if cmd.joint_idx >= self.n_joints {
            return false;
        }
        // Refuse half-apply: ensure clamped setpoints exist before writing raw.
        let was_unarmed = self.setpoints.is_none();
        if was_unarmed {
            let q = vec![cmd.q; self.n_joints];
            let mut targets = vec![cmd.q; self.n_joints];
            targets[cmd.joint_idx] = cmd.clamped;
            self.arm(&q, &targets, cmd.tick);
        }
        self.setpoints_raw
            .get_or_insert_with(|| vec![0.0; self.n_joints])[cmd.joint_idx] = cmd.requested;
        let changed = self.set_clamped_target(cmd.joint_idx, cmd.clamped, cmd.tick);
        // First arm sees equal targets after `arm`, so treat unarmed as a full retarget apply.
        if was_unarmed || changed {
            self.sync_retarget_planner(cmd.joint_idx, cmd.q, cmd.clamped, cmd.downward_seed);
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
        *self = Self::new(self.n_joints);
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
    pub fn set_ascent_stall_ms_for_test(&mut self, joint_idx: usize, ms: u64) {
        self.init_latch_state();
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            if joint_idx < recovery.len() {
                recovery[joint_idx] = AscentRecovery::Active { stalled_ms: ms };
            }
        }
    }

    #[cfg(test)]
    pub fn set_ascent_recovery_for_test(&mut self, joint_idx: usize, active: bool) {
        self.init_latch_state();
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            if let Some(state) = recovery.get_mut(joint_idx) {
                *state = if active {
                    AscentRecovery::Active { stalled_ms: 0 }
                } else {
                    AscentRecovery::Idle
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

        self.advance(&mut world)?;
        self.compose(&world)
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
            recovery.fill(AscentRecovery::Idle);
        }
        Self::fill_bool(&mut self.descent_breakaway, false);
        Self::fill_bool(&mut self.descent_was_stuck, false);
    }

    fn init_latch_state(&mut self) {
        let n = self.n_joints;
        Self::ensure_bool_vec(&mut self.planner_frozen, n);
        if self.ascent_recovery.is_none() {
            self.ascent_recovery = Some(vec![AscentRecovery::Idle; n]);
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
        if let (Some(setpoints), Some(raw)) = (self.setpoints.as_mut(), self.setpoints_raw.as_mut())
        {
            setpoints[idx] = target;
            raw[idx] = target;
        }
    }

    fn mark_retarget(&mut self, joint_idx: usize, tick: u64) {
        self.init_latch_state();
        self.retarget_tick
            .get_or_insert_with(|| vec![0; self.n_joints])[joint_idx] = tick;
        Self::set_bool_at(&mut self.planner_frozen, joint_idx, false);
        if let Some(recovery) = self.ascent_recovery.as_mut() {
            recovery[joint_idx] = AscentRecovery::Idle;
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

    fn advance(&mut self, world: &mut HoldWorld<'_>) -> Result<(), HoldError> {
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

        let advance_result = self.advance_planners(world, wave_cmd_dq);
        if clear_finished_wave {
            *world.wave = None;
        }
        advance_result
    }

    fn advance_planners(
        &mut self,
        world: &mut HoldWorld<'_>,
        wave_cmd_dq: Option<(usize, f64)>,
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
                let dq = dq_raw.clamp(-v_max_caps[i], v_max_caps[i]);
                planners[i].resume_cruise_toward(target, dq);
                // Wave owns the joint — do not carry a pre-wave AscentStall fuse across.
                Self::set_bool_at(&mut self.planner_frozen, i, false);
                if let Some(recovery) = self
                    .ascent_recovery
                    .as_mut()
                    .and_then(|states| states.get_mut(i))
                {
                    *recovery = AscentRecovery::Idle;
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
                continue;
            }

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
            let ascent_recovering = !lead_follow
                && planner_should_recover_ascent_stall(
                    was_ascent_recovering,
                    targets[i],
                    world.q[i],
                    planners[i].q_traj,
                    to_target,
                    dq_f,
                    jp.advance_vel_deadband,
                );
            let freeze = freeze_descent || lead_follow;
            if freeze && !was_planner_frozen {
                event = PlannerEvent::FreezeEnter;
            } else if was_planner_frozen && !freeze {
                event = PlannerEvent::FreezeExit;
            }
            Self::set_bool_at(&mut self.planner_frozen, i, freeze);
            let tick_ms = 1000 / u64::from(world.hz.max(1));
            let progressing_toward_target =
                (to_target > 0.0 && dq_f > 0.0) || (to_target < 0.0 && dq_f < 0.0);
            if let Some(recovery) = self
                .ascent_recovery
                .as_mut()
                .and_then(|states| states.get_mut(i))
            {
                let stalled_ms =
                    recovery.update(ascent_recovering, progressing_toward_target, tick_ms);
                if stalled_ms >= POSITION_ASCENT_STALL_FAULT_MS {
                    return Err(HoldError::AscentStall {
                        joint: name.clone(),
                        ms: stalled_ms,
                    });
                }
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

    fn compose(&mut self, world: &HoldWorld<'_>) -> Result<HoldTickOut, HoldError> {
        let n = self.n_joints;
        let mut mit = Vec::with_capacity(n);
        let mut diag = Vec::with_capacity(n);
        self.init_latch_state();

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
                if let Some(ref mut integral) = self.integral_error {
                    integral[i] = (integral[i] + settle_error * world.dt)
                        .clamp(-MAX_INTEGRAL_NM / jp.ki, MAX_INTEGRAL_NM / jp.ki);
                    tau_ff_cmd += jp.ki * integral[i];
                }
            }
            let lead_sat = lead.abs() >= effective_max_lead - 1e-6;
            let tau_p = jp.kp * lead;
            let mit_velocity = position_hold_mit_velocity(
                dq,
                dq_traj,
                jp.vel_deadband,
                retarget_age_ms,
                approaching_target,
            );
            let mit_kd = if settle_error.abs() < 0.1 {
                position_hold_mit_kd(jp.kd, world.q[i], target, dq, jp.vel_deadband)
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
                phase: format!("{traj_phase:?}"),
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
                ascent_stall_ms: self.ascent_stall_ms_at(i),
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
}

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
        }
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
        assert_eq!(hold.ascent_stall_ms_at(0), 0);
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
        assert_eq!(hold.targets_raw().unwrap()[0], 0.35);
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
        let mut previous_reference_lead = 0.0;
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
                    previous_reference_lead = reference_lead;
                }
                Err(HoldError::AscentStall { ms, .. }) => {
                    assert!(ms >= POSITION_ASCENT_STALL_FAULT_MS);
                    fault_tick = Some(tick_count);
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        let fuse_armed_tick = fuse_armed_tick.expect("recovery fuse must arm");
        let fault_tick = fault_tick.expect("true outbound stall must still fail closed");
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
        let _ = previous_reference_lead;
        Ok(())
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
