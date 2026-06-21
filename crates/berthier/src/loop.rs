//! Periodic control loop (OpenArm-style refresh → compute → MIT send).

use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use armee_kinematics::{
    approach_velocity_cap, clamp_hold_target, clamp_position_in_envelope, effective_command_bounds,
};
use armee_proto::{ControlMode as ProtoControlMode, JointState, RobotState};
use chappe::Bus;
use davout::{
    ControlMode, DavoutError, MitJointCommand as DavoutMit, MotorAddress, MotorBus,
    OperationalMode, Supervisor,
};
use marengo_config::{load_robot_config, resolve_urdf_path};
use thiserror::Error;
use tracing::{debug, info, trace};

use crate::friction::{friction_torque, POSITION_HOLD_ERROR_DEADBAND_RAD};
use crate::position_feedforward::compose_position_hold_feedforward;
use crate::position_profile::{
    classify_position_profile, position_hold_v_max, position_profile_v_max, PlannerEvent,
};
use crate::position_setpoint::{
    approach_stuck_mit_pull, clamp_trajectory_setpoint, descent_breakaway_confirmed,
    descent_stuck_mit_pull, downward_return_seed_velocity, home_final_approach_stuck_pull_rad,
    planner_drifted_from_measurement, planner_overshoot_hold_while_moving, planner_premature_hold,
    planner_should_freeze_on_descent, planner_should_latch_on_overshoot_hold,
    planner_should_resync_stuck_lead, position_hold_effective_max_lead, position_hold_mit_kd,
    position_hold_mit_velocity, reopen_planner_from_premature_hold,
    POSITION_DESCENT_STUCK_LEAD_RAD, POSITION_RETURN_DESCENT_SEED_RAD,
    POSITION_SETTLE_TOLERANCE_RAD,
};
use crate::position_trace::{PositionTrace, PositionTraceRow};
use crate::position_trajectory::{
    filter_dq_ema, JointPositionPlanner, TrapezoidPhase, POSITION_DAMPING_DQ_FILTER_ALPHA,
};

#[derive(Debug, Error)]
pub enum LoopError {
    #[error("safety: {0}")]
    Safety(#[from] DavoutError),
    #[error("dynamics: {0}")]
    Dynamics(#[from] armee_dynamics::DynamicsError),
    #[error("config: {0}")]
    Config(#[from] marengo_config::ConfigError),
    #[error("chappe: {0}")]
    Chappe(#[from] chappe::BusError),
    #[error("position hold: no setpoint latched for joint {joint}")]
    MissingSetpoint { joint: String },
    #[error("position hold: unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("position hold: joint name required for hold-at on multi-joint configs")]
    JointNameRequired,
    #[error("missing motor feedback for joint {joint}")]
    MissingFeedback { joint: String },
}

/// Per-joint kp/kd transition ramp for smooth mode switching.
///
/// When armed, kp/kd are interpolated from `from` to `to` values over `total_ticks`.
/// Only applies to transitions between non-Disabled modes. tau_ff is NOT ramped
/// here — Davout's `rate_limit_tau_ff` owns that.
#[derive(Debug, Clone)]
struct GainRamp {
    /// Per-joint: (from_kp, from_kd, to_kp, to_kd)
    joints: Vec<(f64, f64, f64, f64)>,
    /// Ticks remaining in the ramp.
    ticks_remaining: u32,
    /// Total ticks in the ramp (for interpolation).
    total_ticks: u32,
}

/// Realtime control loop facade.
pub struct ControlLoop<B: MotorBus> {
    supervisor: Supervisor<B>,
    dynamics: UrdfGravityModel,
    joint_names: Vec<String>,
    control_mode: ControlMode,
    /// Final joint-space targets for [`ControlMode::Position`] (hold-on / hold-at).
    position_setpoints: Option<Vec<f64>>,
    /// Operator-requested targets before limit envelope clamp (diagnostics).
    position_setpoints_raw: Option<Vec<f64>>,
    /// Per-joint slew or trapezoidal planner toward [`Self::position_setpoints`].
    position_planners: Option<Vec<JointPositionPlanner>>,
    loop_period: Duration,
    chappe_publish_period: Duration,
    last_chappe: Option<Instant>,
    last_position_diag: Option<Instant>,
    tick_count: u64,
    loop_hz: u32,
    position_trace: Option<PositionTrace>,
    /// Control tick when each joint's latched target last changed (onset diagnostics).
    position_retarget_tick: Option<Vec<u64>>,
    /// EMA-filtered measured velocity per joint for position-hold damping FF only.
    position_dq_filtered: Option<Vec<f64>>,
    /// Hysteresis: planner tick frozen while arm stuck lagging on descent.
    position_planner_frozen: Option<Vec<bool>>,
    /// Once set, descent MIT pull stays off until next retarget.
    position_descent_breakaway: Option<Vec<bool>>,
    /// Set when MIT pull applied on descent; breakaway latch requires this.
    position_descent_was_stuck: Option<Vec<bool>>,
    /// Last planner event per joint (trace CSV `planner_event`).
    position_planner_events: Option<Vec<PlannerEvent>>,
    /// Cumulative per-tick phase times for 1 Hz diagnostics (`take_tick_phase_averages`).
    tick_phase: TickPhaseAccumulator,
    /// Previous supervisor mode (detect Active transition for feedback grace).
    last_operational_mode: OperationalMode,
    /// Ticks allowed without joint feedback immediately after enable (post-homing tick_count > 0).
    active_feedback_grace_ticks: u8,
    /// Active kp/kd transition ramp (None when no ramp in progress).
    gain_ramp: Option<GainRamp>,
}

/// Per-tick CPU time inside [`ControlLoop::tick`] (microseconds, averaged over a window).
#[derive(Debug, Clone, Copy, Default)]
pub struct TickPhaseAverages {
    pub ticks: u32,
    pub feedback_us: u64,
    pub gravity_us: u64,
    pub planner_us: u64,
    pub compose_us: u64,
    pub trace_us: u64,
    pub send_us: u64,
    pub chappe_us: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct TickPhaseSample {
    feedback_us: u64,
    gravity_us: u64,
    planner_us: u64,
    compose_us: u64,
    trace_us: u64,
    send_us: u64,
    chappe_us: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct TickPhaseAccumulator {
    ticks: u32,
    feedback_us: u64,
    gravity_us: u64,
    planner_us: u64,
    compose_us: u64,
    trace_us: u64,
    send_us: u64,
    chappe_us: u64,
}

impl TickPhaseAccumulator {
    fn record(&mut self, sample: TickPhaseSample) {
        self.ticks = self.ticks.saturating_add(1);
        self.feedback_us = self.feedback_us.saturating_add(sample.feedback_us);
        self.gravity_us = self.gravity_us.saturating_add(sample.gravity_us);
        self.planner_us = self.planner_us.saturating_add(sample.planner_us);
        self.compose_us = self.compose_us.saturating_add(sample.compose_us);
        self.trace_us = self.trace_us.saturating_add(sample.trace_us);
        self.send_us = self.send_us.saturating_add(sample.send_us);
        self.chappe_us = self.chappe_us.saturating_add(sample.chappe_us);
    }

    fn into_averages(self) -> Option<TickPhaseAverages> {
        if self.ticks == 0 {
            return None;
        }
        let n = u64::from(self.ticks);
        let avg = TickPhaseAverages {
            ticks: self.ticks,
            feedback_us: self.feedback_us / n,
            gravity_us: self.gravity_us / n,
            planner_us: self.planner_us / n,
            compose_us: self.compose_us / n,
            trace_us: self.trace_us / n,
            send_us: self.send_us / n,
            chappe_us: self.chappe_us / n,
        };
        Some(avg)
    }
}

fn phase_elapsed_us(since: Instant) -> (u64, Instant) {
    let now = Instant::now();
    let us = u64::try_from(now.duration_since(since).as_micros()).unwrap_or(u64::MAX);
    (us, now)
}

impl<B: MotorBus> ControlLoop<B> {
    pub fn from_repo(
        repo_root: impl AsRef<Path>,
        bus: B,
        loop_hz: u32,
        chappe_hz: u32,
    ) -> Result<Self, LoopError> {
        let root = repo_root.as_ref();
        let robot = load_robot_config(root)?;
        let joint_names = robot.robot.joints.clone();
        let urdf = resolve_urdf_path(root, &robot)?;
        let dynamics = UrdfGravityModel::from_urdf(&urdf, &joint_names)?;
        let loop_hz = loop_hz.max(1);
        let supervisor = Supervisor::from_repo(root, bus)?;
        Ok(Self {
            supervisor,
            dynamics,
            joint_names,
            control_mode: ControlMode::Disabled,
            position_setpoints: None,
            position_setpoints_raw: None,
            position_planners: None,
            loop_period: Duration::from_secs_f64(1.0 / f64::from(loop_hz)),
            chappe_publish_period: Duration::from_secs_f64(1.0 / f64::from(chappe_hz.max(1))),
            last_chappe: None,
            last_position_diag: None,
            tick_count: 0,
            loop_hz,
            position_trace: PositionTrace::from_env(loop_hz),
            position_retarget_tick: None,
            position_dq_filtered: None,
            position_planner_frozen: None,
            position_descent_breakaway: None,
            position_descent_was_stuck: None,
            position_planner_events: None,
            tick_phase: TickPhaseAccumulator::default(),
            last_operational_mode: OperationalMode::Disabled,
            active_feedback_grace_ticks: 0,
            gain_ramp: None,
        })
    }

    /// Mean per-tick phase times since the last call; resets the accumulator.
    pub fn take_tick_phase_averages(&mut self) -> Option<TickPhaseAverages> {
        std::mem::take(&mut self.tick_phase).into_averages()
    }

    /// Capture current joint positions as hold targets and commands (no ramp).
    pub fn latch_position_setpoints(&mut self) {
        let q = self.refresh_joint_positions();
        self.position_setpoints = Some(q.clone());
        self.position_setpoints_raw = Some(q.clone());
        self.init_position_planners(&q);
    }

    pub fn position_setpoints(&self) -> Option<&[f64]> {
        self.position_setpoints.as_deref()
    }

    /// Planner trajectory reference (for status / tests), not the clamped MIT setpoint.
    pub fn position_hold_commands(&self) -> Option<Vec<f64>> {
        self.position_planners
            .as_ref()
            .map(|planners| planners.iter().map(|p| p.q_traj).collect())
    }

    pub fn set_joint_position_setpoint(
        &mut self,
        joint: &str,
        position_rad: f64,
    ) -> Result<(), LoopError> {
        if self.position_setpoints.is_none() {
            self.latch_position_setpoints();
        }
        let Some(i) = self.joint_names.iter().position(|n| n == joint) else {
            return Err(LoopError::UnknownJoint {
                joint: joint.to_string(),
            });
        };
        let q_now = self.refresh_joint_positions();
        let requested = self.hold_target_trim(joint, position_rad);
        let dq_cmd = self.estimated_retarget_dq_cmd(joint, q_now[i], requested);
        let target = self.clamp_hold_target(joint, q_now[i], requested, dq_cmd);
        if self.position_setpoints_raw.is_none() {
            self.position_setpoints_raw = Some(vec![0.0; self.joint_names.len()]);
        }
        if let Some(raw) = self.position_setpoints_raw.as_mut() {
            raw[i] = requested;
        }
        let Some(setpoints) = self.position_setpoints.as_mut() else {
            return Err(LoopError::MissingSetpoint {
                joint: joint.to_string(),
            });
        };
        let old_target = setpoints[i];
        setpoints[i] = target;
        if (requested - target).abs() > 1e-6 {
            info!(
                joint = %joint,
                requested,
                clamped = target,
                q = q_now[i],
                "hold-at target clamped to limit envelope"
            );
        }
        if self.position_planners.is_none() {
            self.init_position_planners(&q_now);
        }
        let downward_seed_rate = if (old_target - target).abs() > 1e-6 {
            self.supervisor
                .control
                .control
                .joints
                .get(joint)
                .map(|cfg| {
                    let move_dist = (target - q_now[i]).abs();
                    let threshold = cfg.position_trajectory_threshold_rad;
                    let v_max = self.clamp_v_max(
                        joint,
                        position_profile_v_max(
                            move_dist,
                            cfg.position_slew_rad_s,
                            cfg.position_trajectory_velocity_rad_s,
                            threshold,
                        ),
                    );
                    downward_return_seed_velocity(cfg.position_slew_rad_s, v_max, q_now[i], target)
                })
        } else {
            None
        };
        if let Some(planners) = self.position_planners.as_mut() {
            planners[i].reset_target(q_now[i], target);
            if let Some(seed) = downward_seed_rate {
                planners[i].seed_downward_return_if_needed(
                    q_now[i],
                    target,
                    POSITION_RETURN_DESCENT_SEED_RAD,
                    seed,
                );
            }
        }
        if (old_target - target).abs() > 1e-6 {
            info!(
                joint = %joint,
                q = q_now[i],
                old_target,
                new_target = target,
                delta = target - q_now[i],
                tick = self.tick_count,
                "position hold retarget"
            );
            self.mark_position_retarget(i);
        }
        Ok(())
    }

    fn mark_position_retarget(&mut self, joint_index: usize) {
        self.init_descent_latch_state();
        if self.position_retarget_tick.is_none() {
            self.position_retarget_tick = Some(vec![0; self.joint_names.len()]);
        }
        if let Some(ticks) = self.position_retarget_tick.as_mut() {
            ticks[joint_index] = self.tick_count;
        }
        if let Some(frozen) = self.position_planner_frozen.as_mut() {
            frozen[joint_index] = false;
        }
        if let Some(breakaway) = self.position_descent_breakaway.as_mut() {
            breakaway[joint_index] = false;
        }
        if let Some(was_stuck) = self.position_descent_was_stuck.as_mut() {
            was_stuck[joint_index] = false;
        }
        let dq = self.joint_velocity(&self.joint_names[joint_index]);
        self.seed_dq_filter(joint_index, dq);
    }

    fn init_descent_latch_state(&mut self) {
        let n = self.joint_names.len();
        if self.position_planner_frozen.is_none() {
            self.position_planner_frozen = Some(vec![false; n]);
        }
        if self.position_descent_breakaway.is_none() {
            self.position_descent_breakaway = Some(vec![false; n]);
        }
        if self.position_descent_was_stuck.is_none() {
            self.position_descent_was_stuck = Some(vec![false; n]);
        }
    }

    fn position_retarget_age_ms(&self, joint_index: usize) -> u64 {
        let Some(ticks) = self.position_retarget_tick.as_ref() else {
            return u64::MAX;
        };
        let retarget_tick = ticks.get(joint_index).copied().unwrap_or(0);
        self.tick_count
            .saturating_sub(retarget_tick)
            .saturating_mul(1000)
            / u64::from(self.loop_hz.max(1))
    }

    fn init_position_planners(&mut self, q: &[f64]) {
        let targets = self
            .position_setpoints
            .clone()
            .unwrap_or_else(|| q.to_vec());
        let planners = self
            .joint_names
            .iter()
            .enumerate()
            .map(|(i, _name)| JointPositionPlanner::new_for_target(q[i], targets[i]))
            .collect();
        self.position_planners = Some(planners);
        self.position_retarget_tick = Some(vec![self.tick_count; self.joint_names.len()]);
        self.position_dq_filtered = Some(
            self.joint_names
                .iter()
                .map(|name| self.joint_velocity(name))
                .collect(),
        );
        self.init_descent_latch_state();
        if let Some(frozen) = self.position_planner_frozen.as_mut() {
            frozen.fill(false);
        }
        if let Some(breakaway) = self.position_descent_breakaway.as_mut() {
            breakaway.fill(false);
        }
        if let Some(was_stuck) = self.position_descent_was_stuck.as_mut() {
            was_stuck.fill(false);
        }
    }

    fn seed_dq_filter(&mut self, joint_index: usize, dq: f64) {
        if self.position_dq_filtered.is_none() {
            self.position_dq_filtered = Some(vec![0.0; self.joint_names.len()]);
        }
        if let Some(filtered) = self.position_dq_filtered.as_mut() {
            filtered[joint_index] = dq;
        }
    }

    fn filtered_dq_for_damping(&mut self, joint_index: usize, dq_raw: f64) -> f64 {
        if self.position_dq_filtered.is_none() {
            self.position_dq_filtered = Some(vec![dq_raw; self.joint_names.len()]);
            return dq_raw;
        }
        let Some(filtered) = self.position_dq_filtered.as_mut() else {
            return dq_raw;
        };
        let prev = filtered[joint_index];
        let next = filter_dq_ema(prev, dq_raw, POSITION_DAMPING_DQ_FILTER_ALPHA);
        filtered[joint_index] = next;
        next
    }

    fn hold_target_trim(&self, joint: &str, position_rad: f64) -> f64 {
        let trim = self
            .supervisor
            .control
            .control
            .joints
            .get(joint)
            .map(|c| c.position_hold_trim_rad)
            .unwrap_or(0.0);
        position_rad + trim
    }

    fn clamp_hold_target(&self, joint: &str, q: f64, requested_rad: f64, dq_cmd: f64) -> f64 {
        let Some(policy) = self.supervisor.joint_limit_policy(joint) else {
            return requested_rad;
        };
        clamp_hold_target(policy, q, dq_cmd, requested_rad)
    }

    fn clamp_v_max(&self, joint: &str, v_requested: f64) -> f64 {
        self.supervisor
            .joint_velocity_cap(joint)
            .map_or(v_requested, |cap| v_requested.min(cap))
    }

    fn estimated_retarget_dq_cmd(&self, joint: &str, q: f64, requested_rad: f64) -> f64 {
        let delta = requested_rad - q;
        if delta.abs() <= 1e-9 {
            return 0.0;
        }
        let cfg = self.supervisor.control.control.joints.get(joint);
        let slew = cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25);
        let trajectory_v = cfg
            .map(|c| c.position_trajectory_velocity_rad_s)
            .unwrap_or(0.30);
        let threshold = cfg
            .map(|c| c.position_trajectory_threshold_rad)
            .unwrap_or(0.15);
        let v_max = position_profile_v_max(delta.abs(), slew, trajectory_v, threshold);
        delta.signum() * self.clamp_v_max(joint, v_max)
    }

    pub fn clear_position_hold(&mut self) {
        self.position_setpoints = None;
        self.position_setpoints_raw = None;
        self.position_planners = None;
        self.position_retarget_tick = None;
        self.position_dq_filtered = None;
        self.position_planner_frozen = None;
        self.position_descent_breakaway = None;
        self.position_descent_was_stuck = None;
        self.position_planner_events = None;
    }

    /// Latch current `q` and enter [`ControlMode::Position`] (gravity FF + impedance gains).
    pub fn enter_position_hold(&mut self) -> Result<(), LoopError> {
        self.latch_position_setpoints();
        self.set_control_mode(ControlMode::Position);
        Ok(())
    }

    /// Enter position hold with an explicit setpoint (single-joint bench: one angle; multi-joint: joint name required).
    pub fn enter_position_hold_at(
        &mut self,
        joint: Option<&str>,
        position_rad: f64,
    ) -> Result<(), LoopError> {
        let q = self.refresh_joint_positions();
        if self.joint_names.len() == 1 {
            let name = self.joint_names[0].clone();
            if self.position_setpoints.is_none() {
                self.position_setpoints = Some(q.clone());
            }
            self.set_joint_position_setpoint(&name, position_rad)?;
        } else {
            let joint = joint.ok_or(LoopError::JointNameRequired)?;
            if self.position_setpoints.is_none() {
                self.position_setpoints = Some(q.clone());
            }
            self.set_joint_position_setpoint(joint, position_rad)?;
        }
        self.set_control_mode(ControlMode::Position);
        Ok(())
    }

    pub fn supervisor_mut(&mut self) -> &mut Supervisor<B> {
        &mut self.supervisor
    }

    pub fn set_control_mode(&mut self, mode: ControlMode) {
        let previous = self.control_mode;
        if mode != ControlMode::Position {
            self.position_setpoints = None;
            self.position_setpoints_raw = None;
            self.position_planners = None;
            self.position_retarget_tick = None;
            self.position_dq_filtered = None;
            self.position_planner_frozen = None;
            self.position_descent_breakaway = None;
            self.position_descent_was_stuck = None;
            self.position_planner_events = None;
            self.last_position_diag = None;
        }
        self.control_mode = mode;
        self.supervisor.set_control_mode(mode);
        if previous != mode {
            info!(?previous, ?mode, "control mode transition");
        }
        // Arm a kp/kd ramp for non-Disabled transitions (~100ms at 200Hz).
        // Disabled is excluded in both directions: instant disable for safety,
        // enable path bootstraps separately.
        if mode != ControlMode::Disabled && previous != ControlMode::Disabled {
            let n = self.joint_names.len();
            let from = self.current_effective_gains();
            let to = self.target_gains_for_mode(mode);
            self.gain_ramp = Some(GainRamp {
                joints: (0..n).map(|i| (from[i].0, from[i].1, to[i].0, to[i].1)).collect(),
                ticks_remaining: 20,
                total_ticks: 20,
            });
            // Seed the tau_ff rate limiter with current measured torque so the rate
            // limiter slews from the correct starting point (NOT cleared — clearing
            // causes unclamped torque step via unwrap_or(target)).
            self.supervisor.seed_tau_ff_rate_limiter();
        }
    }

    /// Effective per-joint (kp, kd) right now: interpolated ramp value if a ramp
    /// is in progress, otherwise the target gains for the current mode.
    fn current_effective_gains(&self) -> Vec<(f64, f64)> {
        if let Some(ref ramp) = self.gain_ramp {
            let progress = 1.0 - (ramp.ticks_remaining as f64 / ramp.total_ticks as f64);
            ramp.joints
                .iter()
                .map(|(from_kp, from_kd, to_kp, to_kd)| {
                    let kp = from_kp + (to_kp - from_kp) * progress;
                    let kd = from_kd + (to_kd - from_kd) * progress;
                    (kp, kd)
                })
                .collect()
        } else {
            self.target_gains_for_mode(self.control_mode)
        }
    }

    /// Target (kp, kd) per joint for a mode. GravityComp/TorqueOnly/Disabled → (0, 0);
    /// Impedance/Position → config impedance gains.
    fn target_gains_for_mode(&self, mode: ControlMode) -> Vec<(f64, f64)> {
        match mode {
            ControlMode::GravityComp | ControlMode::TorqueOnly | ControlMode::Disabled => {
                vec![(0.0, 0.0); self.joint_names.len()]
            }
            ControlMode::Impedance | ControlMode::Position => self
                .joint_names
                .iter()
                .map(|name| {
                    let cfg = self.supervisor.control.control.joints.get(name);
                    let kp = cfg.map(|c| c.impedance.kp).unwrap_or(0.0);
                    let kd = cfg.map(|c| c.impedance.kd).unwrap_or(0.0);
                    (kp, kd)
                })
                .collect(),
        }
    }

    pub fn control_mode(&self) -> ControlMode {
        self.control_mode
    }

    pub fn joint_names(&self) -> &[String] {
        &self.joint_names
    }

    pub fn loop_period(&self) -> Duration {
        self.loop_period
    }

    pub fn configured_loop_hz(&self) -> u32 {
        self.loop_hz
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// One control cycle: recv → compute → send → optional Chappe publish.
    pub fn tick(&mut self, chappe: Option<&Bus>) -> Result<(), LoopError> {
        let mut phase = TickPhaseSample::default();
        let mut t = Instant::now();

        self.supervisor.begin_tick_feedback();
        self.supervisor.drain_feedback()?;
        (phase.feedback_us, t) = phase_elapsed_us(t);

        let q = self.read_positions();

        let operational_mode = self.supervisor.mode();
        if operational_mode == OperationalMode::Active
            && self.last_operational_mode != OperationalMode::Active
        {
            // Homing/disabled ticks advance tick_count before the first Active cycle.
            self.active_feedback_grace_ticks = 2;
        }
        self.last_operational_mode = operational_mode;

        let needs_joint_feedback = operational_mode == OperationalMode::Active
            && self.control_mode != ControlMode::Disabled;
        let all_have_feedback = self
            .joint_names
            .iter()
            .all(|name| self.has_joint_feedback(name));
        // First tick (or first ticks after enable) may run before CAN status arrives.
        let feedback_bootstrap = needs_joint_feedback
            && !all_have_feedback
            && (self.tick_count == 0 || self.active_feedback_grace_ticks > 0);
        if needs_joint_feedback && !feedback_bootstrap {
            for name in &self.joint_names {
                if !self.has_joint_feedback(name) {
                    return Err(LoopError::MissingFeedback {
                        joint: name.clone(),
                    });
                }
            }
        }
        if self.active_feedback_grace_ticks > 0 {
            if all_have_feedback {
                self.active_feedback_grace_ticks = 0;
            } else {
                self.active_feedback_grace_ticks -= 1;
            }
        }

        if self.supervisor.mode() == OperationalMode::Active {
            if self.control_mode != ControlMode::Disabled {
                let tau_g = self.dynamics.gravity_torques(&q)?;
                (phase.gravity_us, t) = phase_elapsed_us(t);

                if self.control_mode == ControlMode::Position {
                    self.advance_position_commands(&q)?;
                }
                (phase.planner_us, t) = phase_elapsed_us(t);

                let log_position_diag = self.control_mode == ControlMode::Position
                    && self
                        .last_position_diag
                        .map(|t| t.elapsed() >= Duration::from_secs(1))
                        .unwrap_or(true);
                let mut batch = Vec::new();
                let mut trace_us_this_tick = 0u64;
                for i in 0..self.joint_names.len() {
                    let name = self.joint_names[i].clone();
                    let (base_kp, base_kd, tau_ff, q_des, mit_velocity) = match self.control_mode {
                        ControlMode::GravityComp | ControlMode::TorqueOnly => {
                            (0.0, 0.0, tau_g[i], q[i], 0.0)
                        }
                        ControlMode::Impedance => {
                            let cfg = self.supervisor.control.control.joints.get(&name);
                            let imp = cfg.map(|c| &c.impedance);
                            let fr = cfg.map(|c| &c.friction);
                            let kp = imp.map(|g| g.kp).unwrap_or(0.0);
                            let kd = imp.map(|g| g.kd).unwrap_or(0.0);
                            let dq = self.joint_velocity(&name);
                            let tau_f = fr
                                .map(|f| friction_torque(dq, f.fc, f.fv, f.fo, f.k))
                                .unwrap_or(0.0);
                            (kp, kd, tau_g[i] + tau_f, q[i], 0.0)
                        }
                        ControlMode::Position => {
                            let (kp, kd, max_lead, vel_deadband, friction) = {
                                let cfg = self.supervisor.control.control.joints.get(&name);
                                (
                                    cfg.map(|c| c.impedance.kp).unwrap_or(20.0),
                                    cfg.map(|c| c.impedance.kd).unwrap_or(1.0),
                                    cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.15),
                                    cfg.map(|c| c.position_trajectory_velocity_deadband_rad)
                                        .unwrap_or(0.02),
                                    cfg.map(|c| c.friction.clone()),
                                )
                            };
                            let (q_traj, dq_traj, traj_phase) = {
                                let planner = self
                                    .position_planners
                                    .as_ref()
                                    .and_then(|p| p.get(i))
                                    .ok_or_else(|| LoopError::MissingSetpoint {
                                        joint: name.clone(),
                                    })?;
                                (planner.q_traj, planner.dq_traj, planner.phase())
                            };
                            let dq_raw = self.joint_velocity(&name);
                            let dq = self
                                .position_dq_filtered
                                .as_ref()
                                .and_then(|f| f.get(i).copied())
                                .unwrap_or(dq_raw);
                            let target = self
                                .position_setpoints
                                .as_deref()
                                .and_then(|sp| sp.get(i).copied())
                                .unwrap_or(q_traj);
                            let retarget_age_ms = self.position_retarget_age_ms(i);
                            self.init_descent_latch_state();
                            let settle_error = target - q[i];
                            let approaching_target =
                                dq_traj * settle_error > POSITION_HOLD_ERROR_DEADBAND_RAD;
                            let effective_max_lead = position_hold_effective_max_lead(
                                max_lead,
                                retarget_age_ms,
                                approaching_target,
                                settle_error,
                                q[i],
                            );
                            let mut breakaway = self
                                .position_descent_breakaway
                                .as_ref()
                                .and_then(|l| l.get(i).copied())
                                .unwrap_or(false);
                            let limit_policy = self.supervisor.joint_limit_policy(&name);
                            let mut q_des = clamp_trajectory_setpoint(
                                q_traj,
                                q[i],
                                target,
                                effective_max_lead,
                                limit_policy,
                                dq_traj,
                            );
                            let to_target = target - q[i];
                            let stuck_now = descent_stuck_mit_pull(
                                to_target,
                                q[i],
                                target,
                                dq,
                                vel_deadband,
                                false,
                            );
                            if stuck_now {
                                if let Some(was_stuck) = self.position_descent_was_stuck.as_mut() {
                                    was_stuck[i] = true;
                                }
                            }
                            let was_stuck = self
                                .position_descent_was_stuck
                                .as_ref()
                                .and_then(|s| s.get(i).copied())
                                .unwrap_or(false);
                            if !breakaway
                                && was_stuck
                                && descent_breakaway_confirmed(to_target, dq, vel_deadband)
                            {
                                breakaway = true;
                                if let Some(latch) = self.position_descent_breakaway.as_mut() {
                                    latch[i] = true;
                                }
                            }
                            let joint_stuck = stuck_now && !breakaway;
                            if approach_stuck_mit_pull(
                                to_target,
                                q[i],
                                q_traj,
                                dq,
                                dq_traj,
                                vel_deadband,
                            ) {
                                q_des = q_des.max(
                                    (q[i] + POSITION_DESCENT_STUCK_LEAD_RAD)
                                        .min(q_traj + effective_max_lead),
                                );
                            }
                            if joint_stuck {
                                let stuck_pull = home_final_approach_stuck_pull_rad(q[i], target);
                                q_des = q_des.min(q[i] - stuck_pull);
                            }
                            if let Some(policy) = limit_policy {
                                q_des = clamp_position_in_envelope(policy, q[i], dq_traj, q_des);
                            }
                            let planner_frozen = self
                                .position_planner_frozen
                                .as_ref()
                                .and_then(|f| f.get(i).copied())
                                .unwrap_or(false);
                            let lead = q_des - q[i];
                            let settling = matches!(traj_phase, TrapezoidPhase::Hold)
                                && settle_error.abs() <= POSITION_SETTLE_TOLERANCE_RAD;
                            let ff = compose_position_hold_feedforward(
                                tau_g[i],
                                kd,
                                dq,
                                dq_traj,
                                settle_error,
                                vel_deadband,
                                effective_max_lead,
                                retarget_age_ms,
                                traj_phase,
                                friction.as_ref(),
                                approaching_target,
                            );
                            let friction_mode = ff.friction_mode;
                            let tau_f = ff.tau_f;
                            let tau_d = ff.tau_d;
                            let tau_ff_cmd = ff.tau_ff_cmd;
                            let tau_meas = self.joint_torque(&name);
                            let lead_sat = lead.abs() >= effective_max_lead - 1e-6;
                            let tau_p = kp * lead;
                            let mit_velocity = position_hold_mit_velocity(
                                dq_raw,
                                dq_traj,
                                vel_deadband,
                                retarget_age_ms,
                                approaching_target,
                            );
                            let mit_kd = position_hold_mit_kd(kd, q[i], target, dq, vel_deadband);
                            let phase_str = format!("{traj_phase:?}");
                            let planner_event = self
                                .position_planner_events
                                .as_ref()
                                .and_then(|e| e.get(i).copied())
                                .unwrap_or(PlannerEvent::Tick)
                                .as_str();
                            if log_position_diag {
                                info!(
                                    joint = %name,
                                    q = q[i],
                                    dq = dq_raw,
                                    dq_filt = dq,
                                    q_traj,
                                    dq_traj,
                                    q_des,
                                    target,
                                    lead,
                                    lead_sat,
                                    settle_error,
                                    settling,
                                    friction_mode = friction_mode.as_str(),
                                    retarget_age_ms,
                                    joint_stuck,
                                    planner_frozen,
                                    phase = %phase_str,
                                    kp,
                                    kd,
                                    tau_g = tau_g[i],
                                    tau_f,
                                    tau_d,
                                    tau_ff_cmd,
                                    tau_meas,
                                    tau_err = tau_meas - tau_ff_cmd,
                                    tau_p,
                                    dq_mit = mit_velocity,
                                    kp_mit = kp,
                                    kd_mit = mit_kd,
                                    "position hold command"
                                );
                            }
                            let retarget_tick = self
                                .position_retarget_tick
                                .as_ref()
                                .and_then(|ticks| ticks.get(i).copied());
                            if should_log_position_onset(
                                retarget_tick,
                                self.tick_count,
                                self.loop_hz,
                            ) {
                                debug!(
                                    joint = %name,
                                    retarget_age_ms,
                                    q = q[i],
                                    dq = dq_raw,
                                    dq_filt = dq,
                                    q_traj,
                                    dq_traj,
                                    lead,
                                    settle_error,
                                    settling,
                                    friction_mode = friction_mode.as_str(),
                                    tau_f,
                                    tau_d,
                                    tau_ff_cmd,
                                    phase = %phase_str,
                                    joint_stuck,
                                    planner_frozen,
                                    "position hold onset"
                                );
                            }
                            if let Some(trace) = self.position_trace.as_mut() {
                                let trace_start = Instant::now();
                                let t_ms =
                                    self.tick_count.saturating_mul(1000) / u64::from(self.loop_hz);
                                let (q_env_lo, q_env_hi) = limit_policy
                                    .map(|p| effective_command_bounds(p, q[i], dq_traj))
                                    .unwrap_or((f64::NAN, f64::NAN));
                                let target_raw = self
                                    .position_setpoints_raw
                                    .as_ref()
                                    .and_then(|r| r.get(i).copied())
                                    .unwrap_or(target);
                                let row = PositionTraceRow {
                                    joint: &name,
                                    q: q[i],
                                    dq: dq_raw,
                                    q_traj,
                                    dq_traj,
                                    q_des,
                                    target,
                                    target_raw,
                                    q_env_lo,
                                    q_env_hi,
                                    lead,
                                    lead_sat,
                                    settle_error,
                                    phase: &phase_str,
                                    friction_mode: friction_mode.as_str(),
                                    tau_p,
                                    tau_g: tau_g[i],
                                    tau_f,
                                    tau_d,
                                    tau_ff_cmd,
                                    tau_meas,
                                    dq_mit: mit_velocity,
                                    kp,
                                    kd,
                                    joint_stuck,
                                    planner_frozen,
                                    retarget_age_ms,
                                    planner_event,
                                };
                                let _ = trace.maybe_record(self.tick_count, t_ms, &row);
                                if self.tick_count % u64::from(self.loop_hz) == 0 {
                                    let _ = trace.flush();
                                }
                                trace_us_this_tick = trace_us_this_tick.saturating_add(
                                    u64::try_from(trace_start.elapsed().as_micros())
                                        .unwrap_or(u64::MAX),
                                );
                            }
                            (kp, mit_kd, tau_ff_cmd, q_des, mit_velocity)
                        }
                        ControlMode::Disabled => continue,
                    };
                    let (kp, kd) = if let Some(ref ramp) = self.gain_ramp {
                        let progress =
                            1.0 - (ramp.ticks_remaining as f64 / ramp.total_ticks as f64);
                        let (from_kp, from_kd, to_kp, to_kd) = ramp.joints[i];
                        let kp = from_kp + (to_kp - from_kp) * progress;
                        let kd = from_kd + (to_kd - from_kd) * progress;
                        (kp, kd)
                    } else {
                        (base_kp, base_kd)
                    };
                    batch.push(DavoutMit {
                        joint: name.clone(),
                        kp,
                        kd,
                        position_rad: q_des,
                        velocity_rad_s: mit_velocity,
                        torque_ff_nm: tau_ff,
                    });
                }
                if log_position_diag {
                    self.last_position_diag = Some(Instant::now());
                }
                phase.trace_us = trace_us_this_tick;
                (phase.compose_us, t) = phase_elapsed_us(t);

                self.supervisor.send_mit_batch(batch)?;
                (phase.send_us, t) = phase_elapsed_us(t);
                let _ = self.supervisor.drain_feedback();
                if let Some(ref mut ramp) = self.gain_ramp {
                    ramp.ticks_remaining = ramp.ticks_remaining.saturating_sub(1);
                    if ramp.ticks_remaining == 0 {
                        self.gain_ramp = None;
                    }
                }
            } else {
                // Robstride only streams status after MIT frames; hold current q with zero
                // gains/torque so comm watchdog stays fresh between enable and gravity-on.
                let batch: Vec<DavoutMit> = self
                    .joint_names
                    .iter()
                    .zip(q.iter())
                    .map(|(name, &position_rad)| DavoutMit {
                        joint: name.clone(),
                        kp: 0.0,
                        kd: 0.0,
                        position_rad,
                        velocity_rad_s: 0.0,
                        torque_ff_nm: 0.0,
                    })
                    .collect();
                (phase.compose_us, t) = phase_elapsed_us(t);
                self.supervisor.send_mit_batch(batch)?;
                (phase.send_us, t) = phase_elapsed_us(t);
                let _ = self.supervisor.drain_feedback();
            }
        }

        if let Some(bus) = chappe {
            let now = Instant::now();
            if self
                .last_chappe
                .map(|t| now.duration_since(t) >= self.chappe_publish_period)
                .unwrap_or(true)
            {
                self.publish_robot_state(bus, &q)?;
                self.last_chappe = Some(now);
                phase.chappe_us = phase_elapsed_us(t).0;
            }
        }

        self.tick_phase.record(phase);
        self.tick_count += 1;
        Ok(())
    }

    /// Advance per-joint planners toward latched targets.
    fn advance_position_commands(&mut self, q: &[f64]) -> Result<(), LoopError> {
        let Some(targets) = self.position_setpoints.clone() else {
            return Ok(());
        };
        if self.position_planners.is_none() {
            self.init_position_planners(q);
        }
        self.init_descent_latch_state();
        let dq_meas: Vec<f64> = self
            .joint_names
            .iter()
            .map(|name| self.joint_velocity(name))
            .collect();
        let dq_filtered: Vec<f64> = (0..self.joint_names.len())
            .map(|i| self.filtered_dq_for_damping(i, dq_meas[i]))
            .collect();
        let v_max_caps: Vec<f64> = self
            .joint_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let cfg = self.supervisor.control.control.joints.get(name);
                let slew_rad_s = cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25);
                let trajectory_v_max = cfg
                    .map(|c| c.position_trajectory_velocity_rad_s)
                    .unwrap_or(0.30);
                let threshold = cfg
                    .map(|c| c.position_trajectory_threshold_rad)
                    .unwrap_or(0.15);
                let move_dist = (targets[i] - q[i]).abs();
                let planner_speed = self
                    .position_planners
                    .as_ref()
                    .and_then(|planners| planners.get(i))
                    .map(|planner| planner.dq_traj.abs())
                    .unwrap_or(0.0);
                self.clamp_v_max(
                    name,
                    position_hold_v_max(
                        move_dist,
                        slew_rad_s,
                        trajectory_v_max,
                        threshold,
                        planner_speed,
                    ),
                )
            })
            .collect();
        let Some(planners) = self.position_planners.as_mut() else {
            return Ok(());
        };
        let dt = self.loop_period.as_secs_f64();
        if self.position_planner_events.is_none() {
            self.position_planner_events = Some(vec![PlannerEvent::Tick; self.joint_names.len()]);
        }
        for (i, name) in self.joint_names.iter().enumerate() {
            let mut event = PlannerEvent::Tick;
            let cfg = self.supervisor.control.control.joints.get(name);
            let a_max = cfg
                .map(|c| c.position_trajectory_accel_rad_s2)
                .unwrap_or(0.20);
            let max_lead = cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.10);
            let vel_deadband = cfg
                .map(|c| c.position_trajectory_velocity_deadband_rad)
                .unwrap_or(POSITION_HOLD_ERROR_DEADBAND_RAD);
            let v_max = v_max_caps[i];
            let threshold = cfg
                .map(|c| c.position_trajectory_threshold_rad)
                .unwrap_or(0.15);
            let move_dist = (targets[i] - q[i]).abs();
            let profile = classify_position_profile(q[i], targets[i], move_dist, threshold);
            trace!(joint = %name, ?profile, move_dist, v_max, "position hold profile");
            // FIX A: use effective max_lead (with onset boost) in drift/resync thresholds.
            // Without this, the onset boost in tick()'s q_des clamp is dead-lettered because
            // advance_position_commands resets q_traj to q whenever lead > raw max_lead.
            let settle_error_eff = targets[i] - q[i];
            let approaching_target_eff =
                planners[i].dq_traj * settle_error_eff > POSITION_HOLD_ERROR_DEADBAND_RAD;
            let retarget_age_ms_eff = self.position_retarget_age_ms(i);
            let effective_max_lead = position_hold_effective_max_lead(
                max_lead,
                retarget_age_ms_eff,
                approaching_target_eff,
                settle_error_eff,
                q[i],
            );
            let resync_stuck_lead = planner_should_resync_stuck_lead(
                &planners[i],
                q[i],
                targets[i],
                dq_filtered[i],
                effective_max_lead,
                vel_deadband,
            );
            if resync_stuck_lead {
                event = PlannerEvent::ResyncStuckLead;
                planners[i].reset_target(q[i], targets[i]);
                if let Some(cfg) = self.supervisor.control.control.joints.get(name) {
                    planners[i].seed_downward_return_if_needed(
                        q[i],
                        targets[i],
                        POSITION_RETURN_DESCENT_SEED_RAD,
                        downward_return_seed_velocity(
                            cfg.position_slew_rad_s,
                            v_max,
                            q[i],
                            targets[i],
                        ),
                    );
                }
            } else if planner_premature_hold(&planners[i], q[i], targets[i])
                || planner_overshoot_hold_while_moving(
                    &planners[i],
                    q[i],
                    targets[i],
                    dq_filtered[i],
                    vel_deadband,
                )
            {
                event = PlannerEvent::Reset;
                reopen_planner_from_premature_hold(&mut planners[i], q[i], targets[i], v_max);
            } else if planner_drifted_from_measurement(&planners[i], q[i], targets[i], effective_max_lead) {
                event = PlannerEvent::Reset;
                planners[i].reset_target(q[i], targets[i]);
                if let Some(cfg) = self.supervisor.control.control.joints.get(name) {
                    planners[i].seed_downward_return_if_needed(
                        q[i],
                        targets[i],
                        POSITION_RETURN_DESCENT_SEED_RAD,
                        downward_return_seed_velocity(
                            cfg.position_slew_rad_s,
                            v_max,
                            q[i],
                            targets[i],
                        ),
                    );
                }
            }
            let lag = q[i] - planners[i].q_traj;
            let to_target = targets[i] - q[i];
            let dq_filtered = dq_filtered[i];
            let was_frozen = self
                .position_planner_frozen
                .as_ref()
                .and_then(|f| f.get(i).copied())
                .unwrap_or(false);
            let freeze = planner_should_freeze_on_descent(
                was_frozen,
                targets[i],
                q[i],
                to_target,
                lag,
                planners[i].dq_traj,
                dq_filtered,
                vel_deadband,
                max_lead,
            );
            if freeze && !was_frozen {
                event = PlannerEvent::FreezeEnter;
            } else if was_frozen && !freeze {
                event = PlannerEvent::FreezeExit;
            }
            if let Some(frozen) = self.position_planner_frozen.as_mut() {
                frozen[i] = freeze;
            }
            if planner_should_latch_on_overshoot_hold(
                q[i],
                planners[i].q_traj,
                targets[i],
                dq_filtered,
                vel_deadband,
            ) {
                planners[i].latch_at_target(targets[i]);
                event = PlannerEvent::Latch;
            }
            if !freeze {
                let v_tick = if let Some(policy) = self.supervisor.joint_limit_policy(name) {
                    approach_velocity_cap(policy, q[i], planners[i].dq_traj, v_max)
                } else {
                    v_max
                };
                planners[i].tick(targets[i], dt, v_tick, a_max);
                if let Some(policy) = self.supervisor.joint_limit_policy(name) {
                    let clamped = clamp_position_in_envelope(
                        policy,
                        q[i],
                        planners[i].dq_traj,
                        planners[i].q_traj,
                    );
                    if (clamped - planners[i].q_traj).abs() > 1e-9 {
                        event = PlannerEvent::EnvelopeClamp;
                    }
                    planners[i].q_traj = clamped;
                }
            }
            if let Some(events) = self.position_planner_events.as_mut() {
                events[i] = event;
            }
        }
        Ok(())
    }

    /// Poll bus feedback, then read cached joint positions for planner init.
    fn refresh_joint_positions(&mut self) -> Vec<f64> {
        let _ = self.supervisor.drain_feedback();
        self.read_positions()
    }

    fn has_joint_feedback(&self, joint: &str) -> bool {
        self.supervisor
            .motors
            .motors
            .iter()
            .find(|m| m.joint == joint)
            .and_then(|m| self.supervisor.motor_states().get(&MotorAddress::from(m)))
            .is_some()
    }

    fn read_positions(&self) -> Vec<f64> {
        self.joint_names
            .iter()
            .map(|name| {
                self.supervisor
                    .motors
                    .motors
                    .iter()
                    .find(|m| &m.joint == name)
                    .and_then(|m| self.supervisor.motor_states().get(&MotorAddress::from(m)))
                    .map(|s| f64::from(s.position_rad))
                    .unwrap_or(0.0)
            })
            .collect()
    }

    fn joint_velocity(&self, joint: &str) -> f64 {
        self.supervisor
            .motors
            .motors
            .iter()
            .find(|m| m.joint == joint)
            .and_then(|m| self.supervisor.motor_states().get(&MotorAddress::from(m)))
            .map(|s| f64::from(s.velocity_rad_s))
            .unwrap_or(0.0)
    }

    fn joint_torque(&self, joint: &str) -> f64 {
        self.supervisor
            .motors
            .motors
            .iter()
            .find(|m| m.joint == joint)
            .and_then(|m| self.supervisor.motor_states().get(&MotorAddress::from(m)))
            .map(|s| f64::from(s.torque_nm))
            .unwrap_or(0.0)
    }

    fn publish_robot_state(&self, chappe: &Bus, q: &[f64]) -> Result<(), LoopError> {
        let joints: Vec<JointState> = self
            .joint_names
            .iter()
            .zip(q.iter())
            .map(|(name, &position)| {
                let state = self
                    .supervisor
                    .motors
                    .motors
                    .iter()
                    .find(|m| &m.joint == name)
                    .and_then(|m| self.supervisor.motor_states().get(&MotorAddress::from(m)));
                JointState {
                    name: name.clone(),
                    position,
                    velocity: state.map(|s| f64::from(s.velocity_rad_s)).unwrap_or(0.0),
                    effort: state.map(|s| f64::from(s.torque_nm)).unwrap_or(0.0),
                }
            })
            .collect();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let state = RobotState {
            timestamp_ms,
            joints,
        };
        chappe.publish("robot/state", "berthier", "marengo.v1.RobotState", &state)?;
        Ok(())
    }

    /// Preview gravity torques without sending (for motor-repl status).
    pub fn preview_gravity_torques(&self, q: &[f64]) -> Result<Vec<f64>, LoopError> {
        Ok(self.dynamics.gravity_torques(q)?.into_inner())
    }

    /// Read-only access to the dynamics model (for pre-flight saturation checks).
    pub fn dynamics_model(&self) -> &dyn DynamicsModel {
        &self.dynamics
    }

    /// Test-only: planner `(q_traj, dq_traj)` for replay assertions.
    #[cfg(test)]
    pub fn test_planner_state(&self, joint: &str) -> Option<(f64, f64)> {
        let i = self.joint_names.iter().position(|n| n == joint)?;
        let planner = self.position_planners.as_ref()?.get(i)?;
        Some((planner.q_traj, planner.dq_traj))
    }
}

/// High-rate onset logs for the first window after retarget (`MARENGO_POSITION_ONSET_LOG_MS`, default 250).
fn should_log_position_onset(retarget_tick: Option<u64>, tick: u64, loop_hz: u32) -> bool {
    static ONSET_MS: OnceLock<u64> = OnceLock::new();
    let onset_ms = *ONSET_MS.get_or_init(|| {
        std::env::var("MARENGO_POSITION_ONSET_LOG_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(250)
    });
    let Some(retarget_tick) = retarget_tick else {
        return false;
    };
    let age_ticks = tick.saturating_sub(retarget_tick);
    let age_ms = age_ticks.saturating_mul(1000) / u64::from(loop_hz.max(1));
    if age_ms > onset_ms {
        return false;
    }
    let decimate = u64::from(loop_hz.max(1) / 20).max(1);
    age_ticks % decimate == 0
}

/// Map Davout mode to protobuf (for logging / Consul).
pub fn proto_control_mode(mode: ControlMode) -> ProtoControlMode {
    match mode {
        ControlMode::Disabled => ProtoControlMode::Disabled,
        ControlMode::GravityComp => ProtoControlMode::GravityComp,
        ControlMode::Impedance => ProtoControlMode::Impedance,
        ControlMode::Position => ProtoControlMode::Position,
        ControlMode::TorqueOnly => ProtoControlMode::TorqueOnly,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant, clippy::expect_used)]

    use super::*;
    use armee_kinematics::JointLimitPolicy;
    use davout::{MemoryBus, OperationalMode};

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn test_loop() -> ControlLoop<MemoryBus> {
        ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop")
    }

    fn bench_ready_active(loop_ctrl: &mut ControlLoop<MemoryBus>) {
        let motors = loop_ctrl.supervisor_mut().motors.motors.clone();
        loop_ctrl
            .supervisor_mut()
            .homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("verify");
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("ready");
        loop_ctrl
            .supervisor_mut()
            .request_enable(true)
            .expect("enable");
        loop_ctrl.supervisor_mut().seed_synthetic_feedback();
    }

    #[test]
    fn enable_requires_verified_homing() {
        let mut loop_ctrl = test_loop();
        let err = loop_ctrl
            .supervisor_mut()
            .request_enable(true)
            .expect_err("enable without homing");
        assert!(matches!(err, DavoutError::Homing { .. }));
    }

    #[test]
    fn tick_sends_no_mit_when_supervisor_not_active() {
        let mut loop_ctrl = test_loop();
        let motors = loop_ctrl.supervisor_mut().motors.motors.clone();
        loop_ctrl
            .supervisor_mut()
            .homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("verify");
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("ready");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Ready);
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        assert!(
            loop_ctrl.supervisor_mut().bus_mut().tx.is_empty(),
            "Ready (not Active) must not emit MIT frames"
        );
    }

    #[test]
    fn tick_sends_comm_watchdog_keepalive_when_active_control_disabled() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
        loop_ctrl.supervisor_mut().bus_mut().tx.clear();
        loop_ctrl.tick(None).expect("tick");
        let n = loop_ctrl.joint_names.len();
        assert_eq!(
            loop_ctrl.supervisor_mut().bus_mut().tx.len(),
            n,
            "Active + Disabled control mode must send zero-gain MIT keepalive per joint"
        );
    }

    #[test]
    fn position_mode_without_feedback_errors_when_active() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.tick(None).expect("seed tick with feedback");
        loop_ctrl.supervisor_mut().clear_motor_states();
        loop_ctrl.set_control_mode(ControlMode::Position);
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 0.25)
            .expect("hold-at");
        let err = loop_ctrl.tick(None).expect_err("missing feedback");
        assert!(matches!(err, LoopError::MissingFeedback { .. }));
    }

    #[test]
    fn active_feedback_grace_after_disabled_ticks() {
        let mut loop_ctrl = test_loop();
        let motors = loop_ctrl.supervisor_mut().motors.motors.clone();
        loop_ctrl
            .supervisor_mut()
            .homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("verify");
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("ready");
        for _ in 0..5 {
            loop_ctrl.tick(None).expect("disabled ticks");
        }
        assert!(loop_ctrl.tick_count() >= 5);
        loop_ctrl
            .supervisor_mut()
            .request_enable(true)
            .expect("enable");
        loop_ctrl.set_control_mode(ControlMode::Position);
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl
            .tick(None)
            .expect("grace tick without feedback after homing ticks");
    }

    #[test]
    fn position_mode_without_latched_setpoint_errors() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.set_control_mode(ControlMode::Position);
        let err = loop_ctrl.tick(None).expect_err("missing setpoint");
        assert!(matches!(err, LoopError::MissingSetpoint { .. }));
    }

    #[test]
    fn enter_position_hold_latches_setpoints() {
        let mut loop_ctrl = test_loop();
        loop_ctrl.enter_position_hold().expect("hold");
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
        let sp = loop_ctrl.position_setpoints().expect("latched setpoints");
        assert_eq!(sp.len(), loop_ctrl.joint_names.len());
    }

    #[test]
    fn hold_at_sets_joint_target() {
        let mut loop_ctrl = test_loop();
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 0.42)
            .expect("hold-at");
        let sp = loop_ctrl.position_setpoints().expect("setpoints");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "shoulder_pitch")
            .expect("joint index");
        assert!((sp[i] - 0.42).abs() < 1e-9);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    }

    #[test]
    fn leaving_position_mode_clears_setpoints() {
        let mut loop_ctrl = test_loop();
        loop_ctrl.enter_position_hold().expect("hold");
        assert!(loop_ctrl.position_setpoints().is_some());
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(loop_ctrl.position_setpoints().is_none());
        assert_eq!(loop_ctrl.control_mode(), ControlMode::GravityComp);
    }

    #[test]
    fn position_hold_tick_runs_when_active() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Active);
    }

    #[test]
    fn position_hold_advances_trapezoid_on_first_tick() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 0.25)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "shoulder_pitch")
            .expect("joint");
        loop_ctrl.tick(None).expect("tick");
        let cmd = loop_ctrl.position_hold_commands().expect("cmd")[i];
        assert!(
            cmd > 0.0,
            "trapezoid planner must advance q_traj on first tick"
        );
    }

    #[test]
    fn clamp_trajectory_setpoint_brakes_when_q_outruns_traj() {
        let q_des = clamp_trajectory_setpoint(0.11, 1.10, 1.57, 0.03, None, 0.0);
        assert!((q_des - 1.07).abs() < 1e-12);
    }

    #[test]
    fn clamp_does_not_pull_back_when_slightly_ahead_approaching() {
        let q_des = clamp_trajectory_setpoint(0.035, 0.058, 0.1, 0.10, None, 0.0);
        assert!((q_des - 0.058).abs() < 1e-12);
    }

    #[test]
    fn clamp_brakes_when_arm_runs_far_ahead_of_planner() {
        let q_des = clamp_trajectory_setpoint(1.32, 1.36, 1.57, 0.10, None, 0.0);
        assert!(
            q_des < 1.36,
            "must not command q_des at measured q when lead > resync band"
        );
        assert!((q_des - 1.32).abs() < 1e-12);
    }

    #[test]
    fn clamp_caps_ahead_setpoint_at_target_on_approach() {
        let q_des = clamp_trajectory_setpoint(0.091, 0.105, 0.1, 0.10, None, 0.0);
        assert!((q_des - 0.1).abs() < 1e-12);
    }

    #[test]
    fn clamp_does_not_pull_forward_near_home_return() {
        let q_des = clamp_trajectory_setpoint(0.039, 0.006, 0.0, 0.10, None, -0.6);
        assert!(
            q_des <= 0.006,
            "near-home return must not brake descent by commanding ahead: {q_des}"
        );
    }

    #[test]
    fn clamp_latches_setpoint_at_target_on_large_hold_overshoot() {
        let q_des = clamp_trajectory_setpoint(1.65, 1.64, 1.57, 0.10, None, -0.3);
        assert!(
            (q_des - 1.57).abs() < 1e-12,
            "hold overshoot must not chase q_traj above measured q"
        );
        let q_des_negative = clamp_trajectory_setpoint(-0.66, -0.70, -0.64, 0.10, None, -0.3);
        assert!(
            (q_des_negative + 0.64).abs() < 1e-12,
            "negative hold overshoot must latch at target"
        );
    }

    #[test]
    fn clamp_does_not_jump_to_negative_target_before_planner_arrives() {
        let q_des = clamp_trajectory_setpoint(-0.0005, -0.0004, -0.6417, 0.10, None, -0.024);
        assert!(
            q_des > -0.02,
            "sub-home retarget must follow q_traj, not jump to target: {q_des}"
        );
        assert!(
            q_des > -0.6417,
            "q_des must not command the lower-limit target before planner reaches it"
        );
    }

    #[test]
    fn planner_latches_on_overshoot_hold_when_at_rest() {
        assert!(planner_should_latch_on_overshoot_hold(
            1.64, 1.57, 1.57, 0.0, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            1.64, 1.57, 1.57, 0.15, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            1.58, 1.57, 1.57, 0.0, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            0.105, 0.0, 0.0, 0.0, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            -0.016, -0.016, -0.641, 0.0, 0.02
        ));
        assert!(planner_should_latch_on_overshoot_hold(
            -0.70, -0.641, -0.641, 0.0, 0.02
        ));
    }

    #[test]
    fn planner_premature_hold_when_virtual_target_reached_before_arm() {
        let mut planner = JointPositionPlanner::new_for_target(0.05, 0.1);
        planner.q_traj = 0.1;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(planner_premature_hold(&planner, 0.052, 0.1));
        assert!(!planner_drifted_from_measurement(
            &planner, 0.052, 0.1, 0.10
        ));
        reopen_planner_from_premature_hold(&mut planner, 0.052, 0.1, 0.15);
        assert_eq!(planner.phase(), TrapezoidPhase::Cruise);
        assert!((planner.q_traj - 0.1).abs() < 1e-12);
        assert!((planner.dq_traj - 0.15).abs() < 1e-12);
    }

    #[test]
    fn planner_reopens_when_overshot_past_hold_while_moving() {
        let mut planner = JointPositionPlanner::new_for_target(1.5, 1.57);
        planner.q_traj = 1.57;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(planner_overshoot_hold_while_moving(
            &planner, 1.62, 1.57, 0.08, 0.02
        ));
        reopen_planner_from_premature_hold(&mut planner, 1.62, 1.57, 1.65);
        assert_eq!(planner.phase(), TrapezoidPhase::Cruise);
        assert!(planner.dq_traj < 0.0);
    }

    #[test]
    fn mit_kd_engages_on_overshoot_velocity() {
        assert!((position_hold_mit_kd(2.0, 1.62, 1.57, 0.08, 0.02) - 2.0).abs() < 1e-12);
        assert!((position_hold_mit_kd(2.0, 1.62, 1.57, 0.01, 0.02)).abs() < 1e-12);
        assert!((position_hold_mit_kd(2.0, 1.55, 1.57, 0.08, 0.02)).abs() < 1e-12);
    }

    #[test]
    fn planner_drifted_when_hold_latched_but_arm_not_settled() {
        let planner = JointPositionPlanner::new_at(0.0);
        assert!(planner_premature_hold(&planner, 0.087, 0.0));
        assert!(!planner_drifted_from_measurement(
            &planner, 0.087, 0.0, 0.10
        ));
    }

    #[test]
    fn home_premature_hold_when_arm_above_target_within_old_resync_band() {
        let mut planner = JointPositionPlanner::new_at(0.0);
        planner.force_hold_for_test();
        // 0.031 rad was observed on bench-20260620T003624Z — inside 0.03 resync, above 0.005 home band.
        assert!(planner_premature_hold(&planner, 0.031, 0.0));
        assert!(!planner_drifted_from_measurement(
            &planner, 0.031, 0.0, 0.10
        ));
    }

    #[test]
    fn planner_not_drifted_when_arm_runs_ahead_on_approach() {
        let mut planner = JointPositionPlanner::new_for_target(0.88, 1.57);
        planner.q_traj = 0.88;
        assert!(!planner_drifted_from_measurement(
            &planner, 1.02, 1.57, 0.10
        ));
    }

    #[test]
    fn planner_not_drifted_on_small_hold_overshoot() {
        let planner = JointPositionPlanner::new_at(0.1);
        assert!(!planner_drifted_from_measurement(
            &planner, 0.112, 0.1, 0.10
        ));
    }

    #[test]
    fn return_onset_pulls_q_des_below_q_when_stuck() {
        assert!(descent_stuck_mit_pull(
            -0.09, 0.105, 0.015, 0.0, 0.02, false
        ));
        let q_des = clamp_trajectory_setpoint(0.105, 0.105, 0.015, 0.10, None, 0.0)
            .min(0.105 - POSITION_DESCENT_STUCK_LEAD_RAD);
        assert!((q_des - 0.075).abs() < 1e-12);
    }

    #[test]
    fn sub_home_negative_move_does_not_enable_descent_mit_pull() {
        assert!(!descent_stuck_mit_pull(-0.64, 0.0, -0.64, 0.0, 0.02, false));
        assert!(!descent_stuck_mit_pull(
            -0.50, -0.14, -0.64, 0.0, 0.02, false
        ));
    }

    #[test]
    fn onset_mit_velocity_follows_planner_while_arm_stuck() {
        let v = position_hold_mit_velocity(0.0, 0.18, 0.02, 0, true);
        assert!((v - 0.18).abs() < 1e-12);
    }

    #[test]
    fn onset_mit_velocity_zeros_after_onset_window() {
        let v = position_hold_mit_velocity(0.0, 0.18, 0.02, 301, true);
        assert!((v).abs() < 1e-12);
    }

    #[test]
    fn onset_max_lead_boosts_outbound_only() {
        let boosted = position_hold_effective_max_lead(0.10, 0, true, 1.57, 0.0);
        assert!((boosted - 0.15).abs() < 1e-12);
        let mid_span = position_hold_effective_max_lead(0.10, 0, true, 0.69, 0.878);
        assert!((mid_span - 0.10).abs() < 1e-12);
        let home = position_hold_effective_max_lead(0.10, 0, false, -1.57, 1.5);
        assert!((home - 0.10).abs() < 1e-12);
        let small = position_hold_effective_max_lead(0.10, 0, true, 0.02, 0.0);
        assert!((small - 0.10).abs() < 1e-12);
        let return_high = position_hold_effective_max_lead(0.10, 0, true, -1.545, 1.645);
        assert!(
            (return_high - 0.15).abs() < 1e-12,
            "return from high q gets onset lead boost"
        );
        let lower_limit = position_hold_effective_max_lead(0.10, 0, true, -0.64, 0.0);
        assert!(
            (lower_limit - 0.10).abs() < 1e-12,
            "limit-directed negative move must not get return assist boost"
        );
    }

    #[test]
    fn planner_resyncs_when_stuck_at_lead_cap() {
        let mut planner = JointPositionPlanner::new_for_target(1.02, 1.57);
        planner.q_traj = 0.92;
        assert!(planner_should_resync_stuck_lead(
            &planner, 1.02, 1.57, 0.0, 0.10, 0.02
        ));
        assert!(!planner_should_resync_stuck_lead(
            &planner, 1.02, 1.57, 0.15, 0.10, 0.02
        ));
    }

    #[test]
    fn planner_no_resync_when_lagging_on_home_return() {
        let mut planner = JointPositionPlanner::new_for_target(1.65, 0.0);
        planner.q_traj = 1.50;
        planner.dq_traj = -0.8;
        assert!(!planner_should_resync_stuck_lead(
            &planner, 1.65, 0.0, 0.0, 0.10, 0.02
        ));
    }

    #[test]
    fn planner_not_drifted_when_arm_lags_on_home_return() {
        let mut planner = JointPositionPlanner::new_for_target(1.65, 0.0);
        planner.q_traj = 1.50;
        planner.dq_traj = -1.2;
        assert!(!planner_drifted_from_measurement(&planner, 1.65, 0.0, 0.10));
    }

    // === FIX A regression tests: effective max_lead in drift/resync thresholds ===
    // These tests lock the fix for the pi/2 hold limit cycle. The onset boost
    // (position_hold_effective_max_lead) raises max_lead from 0.05 to 0.15 during
    // the 300ms post-retarget window. Before FIX A, advance_position_commands
    // passed raw max_lead to the drift/resync checks, dead-lettering the boost:
    // q_traj was reset to q whenever lead > 0.05, so q_des could never reach q+0.15,
    // capping P-torque at 0.4 Nm — insufficient to break static friction.

    #[test]
    fn planner_drift_uses_effective_max_lead_during_onset() {
        // At home (q≈0) stuck with target=π/2, during onset, q_traj=q+0.10 must
        // NOT drift-reset when using effective max_lead (0.15). With raw 0.05 it
        // would — this is the bug FIX A addresses.
        let q = 0.004;
        let target = std::f64::consts::FRAC_PI_2;
        let mut planner = JointPositionPlanner::new_for_target(q, target);
        planner.q_traj = q + 0.10;
        planner.dq_traj = 0.4;
        let raw_max_lead = 0.05;
        let effective = position_hold_effective_max_lead(
            raw_max_lead, 100, true, target - q, q,
        );
        assert!((effective - 0.15).abs() < 1e-9, "onset boost active");
        assert!(
            !planner_drifted_from_measurement(&planner, q, target, effective),
            "FIX A: drift uses effective max_lead; q+0.10 within 0.15 lead → no reset"
        );
        assert!(
            planner_drifted_from_measurement(&planner, q, target, raw_max_lead),
            "bug repro: raw max_lead resets at q+0.10 > 0.05, dead-lettering the boost"
        );
    }

    #[test]
    fn planner_drift_reverts_to_raw_max_lead_after_onset() {
        // After 300ms onset window, effective max_lead reverts to raw; drift
        // reset resumes at the raw threshold. Guards against persistent boost.
        let q = 0.004;
        let target = std::f64::consts::FRAC_PI_2;
        let mut planner = JointPositionPlanner::new_for_target(q, target);
        planner.q_traj = q + 0.10;
        planner.dq_traj = 0.4;
        let raw_max_lead = 0.05;
        let effective = position_hold_effective_max_lead(
            raw_max_lead, 301, true, target - q, q,
        );
        assert!((effective - raw_max_lead).abs() < 1e-9, "onset expired");
        assert!(
            planner_drifted_from_measurement(&planner, q, target, effective),
            "drift reset resumes at raw max_lead after onset expires"
        );
    }

    #[test]
    fn planner_resync_uses_effective_max_lead_during_onset() {
        // Resync_stuck_lead must also use effective max_lead. During onset,
        // q_traj=q+0.10 is within 0.15 lead → no resync. Raw 0.05 would resync.
        let q = 0.004;
        let target = std::f64::consts::FRAC_PI_2;
        let mut planner = JointPositionPlanner::new_for_target(q, target);
        planner.q_traj = q + 0.10;
        planner.dq_traj = 0.4;
        let raw_max_lead = 0.05;
        let effective = position_hold_effective_max_lead(
            raw_max_lead, 100, true, target - q, q,
        );
        assert!(
            !planner_should_resync_stuck_lead(
                &planner, q, target, 0.0, effective, 0.02
            ),
            "FIX A: resync uses effective max_lead; q+0.10 within 0.15 → no resync"
        );
        assert!(
            planner_should_resync_stuck_lead(
                &planner, q, target, 0.0, raw_max_lead, 0.02
            ),
            "bug repro: raw max_lead resyncs at q+0.10 > 0.05"
        );
    }

    #[test]
    fn onset_lead_boost_expires_at_300ms_boundary() {
        // The onset boost must expire at POSITION_HOLD_ONSET_MS (300ms).
        // At 300ms: boost active (0.15). At 301ms: expired (raw 0.05).
        let raw = 0.05;
        let q = 0.004;
        let settle_error = std::f64::consts::FRAC_PI_2 - q;
        let at_boundary = position_hold_effective_max_lead(raw, 300, true, settle_error, q);
        assert!((at_boundary - 0.15).abs() < 1e-9, "boost active at 300ms");
        let after_boundary = position_hold_effective_max_lead(raw, 301, true, settle_error, q);
        assert!((after_boundary - raw).abs() < 1e-9, "boost expired at 301ms");
    }

    #[test]
    fn descent_mit_pull_clears_after_breakaway_latch() {
        assert!(!descent_stuck_mit_pull(
            -0.09, 0.105, 0.015, 0.0, 0.02, true
        ));
        assert!(descent_stuck_mit_pull(
            -0.09, 0.105, 0.015, 0.0, 0.02, false
        ));
        assert!(descent_breakaway_confirmed(-0.09, -0.026, 0.02));
        // Cruise motion alone must not imply breakaway without a stuck episode.
        assert!(!descent_stuck_mit_pull(
            -0.09, 0.08, 0.015, -0.03, 0.02, false
        ));
    }

    #[test]
    fn home_final_approach_stuck_enables_mit_pull_and_unfreezes_planner() {
        use crate::position_setpoint::{
            home_final_approach_stuck, home_final_approach_stuck_pull_rad,
            POSITION_HOME_FINAL_PULL_THROUGH_RAD,
        };
        assert!(home_final_approach_stuck(0.031, 0.0));
        assert!(!home_final_approach_stuck(0.003, 0.0));
        assert!(!home_final_approach_stuck(0.06, 0.0));
        assert!(descent_stuck_mit_pull(-0.031, 0.031, 0.0, 0.0, 0.02, false));
        assert!(
            (home_final_approach_stuck_pull_rad(0.031, 0.0)
                - (0.031 + POSITION_HOME_FINAL_PULL_THROUGH_RAD))
                .abs()
                < 1e-9
        );
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 0.031, -0.031, 0.031, -0.15, 0.0, 0.02, 0.10,
        ));
        assert!(!planner_should_freeze_on_descent(
            true, 0.0, 0.031, -0.031, 0.031, -0.15, 0.0, 0.02, 0.10,
        ));
    }

    #[test]
    fn planner_freeze_skipped_in_home_final_approach_band() {
        let deadband = 0.02;
        // MIT pull-through replaces planner freeze in the 5–50 mrad home band.
        for q in [0.01_f64, 0.04, 0.05] {
            assert!(!planner_should_freeze_on_descent(
                false, 0.0, q, -q, q, -0.10, 0.0, deadband, 0.10
            ));
            assert!(!planner_should_freeze_on_descent(
                true, 0.0, q, -q, q, -0.10, -0.024, deadband, 0.10
            ));
        }
    }

    #[test]
    fn planner_freeze_skips_far_from_home_overshoot_return() {
        let deadband = 0.02;
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 0.102, -0.102, 0.030, -0.15, 0.0, deadband, 0.10
        ));
    }

    #[test]
    fn planner_freeze_skips_intermediate_overshoot_and_synced_return() {
        let deadband = 0.02;
        // Overshoot past 0.8 rad hold — same lag signature, must not freeze.
        assert!(!planner_should_freeze_on_descent(
            false, 0.8, 0.878, -0.078, 0.059, -0.062, 0.0, deadband, 0.10
        ));
        // Return nearly synced (~10 mrad lead) — must not latch forever at rest.
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 1.518, -1.508, 0.010, -0.180, 0.0, deadband, 0.10
        ));
        assert!(!planner_should_freeze_on_descent(
            true, 0.0, 1.518, -1.508, 0.010, -0.180, 0.0, deadband, 0.10
        ));
    }

    #[test]
    fn planner_freeze_skips_high_angle_return_overshoot() {
        let deadband = 0.02;
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 1.645, -1.645, 0.032, -0.27, 0.0, deadband, 0.10
        ));
    }

    #[test]
    fn position_hold_trajectory_damping_tracks_dq_error() {
        let tau_d = crate::position_trajectory::trajectory_damping_torque(0.1, 0.2, 2.0);
        assert!((tau_d - 0.2).abs() < 1e-12);
    }

    #[test]
    fn slew_max_lead_clamps_command_ahead_of_measured_q() {
        let mut loop_ctrl = test_loop();
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 1.2)
            .expect("hold-at");
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.tick(None).expect("tick");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "shoulder_pitch")
            .expect("joint");
        let cmd = loop_ctrl.position_hold_commands().expect("cmd")[i];
        assert!(cmd < 0.2, "first tick must not jump to 1.2 rad target");
    }

    #[test]
    fn hold_at_ramps_command_not_instant_setpoint() {
        let mut loop_ctrl = test_loop();
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), 1.2)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "shoulder_pitch")
            .expect("joint index");
        let target = loop_ctrl.position_setpoints().expect("target")[i];
        assert!((target - 1.2).abs() < 1e-9);
        let cmd0 = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(cmd0.abs() < 1e-6, "command starts at measured q≈0");
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.tick(None).expect("tick");
        let cmd1 = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(
            cmd1 > cmd0 && cmd1 < target,
            "first tick slews toward target"
        );
    }

    #[test]
    fn planner_drifted_detects_stale_init_before_large_hold_at() {
        let stale = JointPositionPlanner::new_at(0.0);
        assert!(planner_premature_hold(&stale, 2.9, 0.0));
        assert!(!planner_drifted_from_measurement(&stale, 2.9, 0.0, 0.15));
        let aligned = JointPositionPlanner::new_for_target(2.9, 0.0);
        assert!(!planner_drifted_from_measurement(&aligned, 2.9, 0.0, 0.15));
    }

    #[test]
    fn trapezoid_trajectory_accumulates_independently_of_max_lead() {
        let target = 1.2;
        let max_lead = 0.03;
        let mut planner = JointPositionPlanner::new_for_target(0.0, target);
        let dt = 0.005;
        for _ in 0..100 {
            planner.tick(target, dt, 0.08, 0.36);
        }
        assert!(
            planner.q_traj > 0.01,
            "planner trajectory must advance at trapezoid rate even when q lags"
        );
        let q_des = clamp_trajectory_setpoint(planner.q_traj, 0.0, target, max_lead, None, 0.0);
        assert!(
            q_des > 0.0,
            "MIT setpoint follows planner when measured q lags"
        );
        assert!(
            planner.q_traj >= q_des,
            "trajectory runs ahead of or meets clamped MIT command"
        );
    }

    fn shoulder_limit_policy() -> JointLimitPolicy {
        JointLimitPolicy {
            bounds: armee_kinematics::JointLimitBounds::from_hard_and_soft(
                -0.9,
                3.17,
                Some(-0.872665),
                Some(3.141593),
            ),
            margin: armee_kinematics::LimitMarginConfig {
                min_rad: 0.01,
                k_v_s: 0.02,
                k_stop: 0.5,
                velocity_deadband_rad_s: 0.02,
                measured_fault_slack_rad: 0.005,
                decel_rad_s2: 4.8,
            },
            velocity: 2.0,
            effort: 10.0,
            tau_ff_max: 5.0,
        }
    }

    #[test]
    fn overshoot_past_soft_bottom_stays_above_hard_envelope() {
        let policy = shoulder_limit_policy();
        let target = -0.872665;
        let q = -0.92;
        let q_des = clamp_trajectory_setpoint(-0.877, q, target, 0.10, Some(&policy), -1.5);
        assert!(
            q_des >= policy.hard_lower(),
            "q_des {q_des} must not command past hard lower {}",
            policy.hard_lower()
        );
    }

    #[test]
    fn limit_clamped_sub_home_target_does_not_seed_downward_return() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("shoulder_pitch"), -0.85)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "shoulder_pitch")
            .expect("joint index");
        let target = loop_ctrl.position_setpoints().expect("setpoints")[i];
        assert!(
            target > -0.85,
            "requested lower-limit probe must clamp before planner reset"
        );
        let planner = &loop_ctrl.position_planners.as_ref().expect("planner")[i];
        assert!(
            planner.dq_traj.abs() < 1e-12,
            "clamped negative target must not seed downward velocity"
        );
        loop_ctrl.tick(None).expect("tick");
        let cmd = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(
            cmd > -0.20,
            "first tick must ramp from measured q, not jump to clamped target: {cmd}"
        );
        assert!(
            cmd > target,
            "planner must approach the clamped target gradually: cmd={cmd} target={target}"
        );
    }

    #[test]
    fn layer2_hold_at_uses_slew_profile_not_trajectory() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        let joint = "shoulder_pitch";
        let cfg = loop_ctrl
            .supervisor
            .control
            .control
            .joints
            .get(joint)
            .expect("joint cfg");
        let slew = cfg.position_slew_rad_s;
        let trajectory_v = cfg.position_trajectory_velocity_rad_s;
        let threshold = cfg.position_trajectory_threshold_rad;
        let v_small =
            crate::position_profile::position_profile_v_max(0.1, slew, trajectory_v, threshold);
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.1)
            .expect("hold-at");
        let mut max_dq: f64 = 0.0;
        for _ in 0..120 {
            let (q_traj, dq_traj) = loop_ctrl.test_planner_state(joint).unwrap_or((0.0, 0.0));
            max_dq = max_dq.max(dq_traj.abs());
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, q_traj as f32, dq_traj as f32)
                .expect("feedback");
            loop_ctrl.tick(None).expect("tick");
        }
        assert!(
            max_dq <= v_small * 1.2 + 0.02,
            "0.1 rad hold-at must use small-move profile (v_max≈{v_small}), got peak dq_traj={max_dq}"
        );
    }

    #[test]
    fn layer2_replay_return_home_stays_on_slew_profile() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        let joint = "shoulder_pitch";
        let cfg = loop_ctrl
            .supervisor
            .control
            .control
            .joints
            .get(joint)
            .expect("joint cfg");
        let slew = cfg.position_slew_rad_s;
        let trajectory_v = cfg.position_trajectory_velocity_rad_s;
        let threshold = cfg.position_trajectory_threshold_rad;
        let v_small =
            crate::position_profile::position_profile_v_max(0.1, slew, trajectory_v, threshold);
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.1)
            .expect("hold-at");
        for _ in 0..400 {
            let (q_traj, dq_traj) = loop_ctrl.test_planner_state(joint).unwrap_or((0.0, 0.0));
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, q_traj as f32, dq_traj as f32)
                .expect("feedback");
            loop_ctrl.tick(None).expect("tick");
        }
        loop_ctrl
            .set_joint_position_setpoint(joint, 0.0)
            .expect("return home");
        let mut max_dq: f64 = 0.0;
        let mut max_q_des_step: f64 = 0.0;
        let mut prev_q_des = loop_ctrl
            .test_planner_state(joint)
            .map(|(q, _)| q)
            .unwrap_or(0.0);
        for _ in 0..400 {
            let (q_traj, dq_traj) = loop_ctrl.test_planner_state(joint).unwrap_or((0.0, 0.0));
            max_dq = max_dq.max(dq_traj.abs());
            let step = (q_traj - prev_q_des).abs();
            max_q_des_step = max_q_des_step.max(step);
            prev_q_des = q_traj;
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, q_traj as f32, dq_traj as f32)
                .expect("feedback");
            loop_ctrl.tick(None).expect("tick");
        }
        assert!(
            max_dq <= v_small * 1.2 + 0.02,
            "return home must use small-move profile (v_max≈{v_small}), peak dq_traj={max_dq}"
        );
        assert!(
            max_q_des_step < 0.05,
            "planner reference must not jump per tick on return, max step={max_q_des_step}"
        );
    }
}
