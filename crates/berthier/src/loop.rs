//! Periodic control loop (OpenArm-style refresh → compute → MIT send).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use armee_kinematics::clamp_hold_target;
use armee_proto::{ControlMode as ProtoControlMode, JointState, RobotState};
use chappe::Bus;
use davout::{
    ControlMode, DavoutError, MitJointCommand as DavoutMit, MotorBus, OperationalMode, Supervisor,
};
use marengo_config::{
    load_robot_config, motor_type_key, resolve_urdf_path, ModeGains, MotorTypeDefaults,
};
use thiserror::Error;
use tracing::{debug, info};

use crate::friction::POSITION_HOLD_ERROR_DEADBAND_RAD;
use crate::gain_runtime::{
    target_gains_from_yaml, GainClampLimits, GainOverride, GainRuntime, JointModeGains,
};
use crate::mit_feedforward::{MitFeedforward, MitFfJointIn};
use crate::position_hold::{
    HoldError, HoldJointParams, HoldRetarget, HoldWorld, PositionHold, ADVANCE_MAX_LEAD_DEFAULT,
};
use crate::position_profile::position_profile_v_max;
use crate::position_setpoint::{downward_return_seed_velocity, envelope_dq_cmd_for_hold_clamp};
use crate::position_trace::{PositionTrace, PositionTraceRow};
use crate::position_wave::PositionWave;

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
    #[error("position wave: min_rad must be less than max_rad")]
    InvalidWaveRange,
    #[error("position wave: cycles must be at least 1")]
    InvalidWaveCycles,
    #[error("position wave: half_period_sec must be positive")]
    InvalidWavePeriod,
    #[error("missing motor feedback for joint {joint}")]
    MissingFeedback { joint: String },
    #[error("position hold: ascent stall on {joint} — planner frozen ahead of arm for {ms} ms")]
    AscentStall { joint: String, ms: u64 },
}

impl From<HoldError> for LoopError {
    fn from(err: HoldError) -> Self {
        match err {
            HoldError::AscentStall { joint, ms } => Self::AscentStall { joint, ms },
            HoldError::MissingSetpoint { joint } => Self::MissingSetpoint { joint },
            HoldError::LenMismatch => Self::MissingSetpoint {
                joint: "len_mismatch".to_string(),
            },
        }
    }
}

/// Realtime control loop facade.
pub struct ControlLoop<B: MotorBus> {
    supervisor: Supervisor<B>,
    dynamics: UrdfGravityModel,
    /// Repo root for commissioning-scope / robot.yaml resolution on re-arm.
    repo_root: PathBuf,
    joint_names: Vec<String>,
    control_mode: ControlMode,
    /// Position-hold lifecycle + control law ([`PositionHold`]).
    position_hold: PositionHold,
    loop_period: Duration,
    chappe_publish_period: Duration,
    last_chappe: Option<Instant>,
    last_position_diag: Option<Instant>,
    tick_count: u64,
    loop_hz: u32,
    position_trace: Option<PositionTrace>,
    /// Cumulative per-tick phase times for 1 Hz diagnostics (`take_tick_phase_averages`).
    tick_phase: TickPhaseAccumulator,
    /// Previous supervisor mode (detect Active transition for feedback grace).
    last_operational_mode: OperationalMode,
    /// Ticks allowed without joint feedback immediately after enable (post-homing tick_count > 0).
    active_feedback_grace_ticks: u8,
    /// Testing overrides + mode-transition kp/kd ramp + per-tick resolve.
    gains: GainRuntime,
    /// Per-joint latched open-loop torque command (Nm) for [`ControlMode::TorqueOnly`].
    /// Default unset → 0. Cleared when leaving TorqueOnly.
    torque_cmds: HashMap<String, f64>,
    /// In-loop triangle wave on one joint while others hold fixed setpoints.
    position_wave: Option<PositionWave>,
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
        let n_joints = joint_names.len();
        Ok(Self {
            supervisor,
            dynamics,
            repo_root: root.to_path_buf(),
            joint_names,
            control_mode: ControlMode::Disabled,
            position_hold: PositionHold::new(n_joints),
            loop_period: Duration::from_secs_f64(1.0 / f64::from(loop_hz)),
            chappe_publish_period: Duration::from_secs_f64(1.0 / f64::from(chappe_hz.max(1))),
            last_chappe: None,
            last_position_diag: None,
            tick_count: 0,
            loop_hz,
            position_trace: PositionTrace::from_env(loop_hz),
            tick_phase: TickPhaseAccumulator::default(),
            last_operational_mode: OperationalMode::Disabled,
            active_feedback_grace_ticks: 0,
            gains: GainRuntime::new(),
            torque_cmds: HashMap::new(),
            position_wave: None,
        })
    }

    /// Mean per-tick phase times since the last call; resets the accumulator.
    pub fn take_tick_phase_averages(&mut self) -> Option<TickPhaseAverages> {
        std::mem::take(&mut self.tick_phase).into_averages()
    }

    /// Capture current joint positions as hold targets and commands (no ramp).
    pub fn latch_position_setpoints(&mut self) {
        let q = self.refresh_joint_positions();
        self.position_hold.arm(&q, &q, self.tick_count);
        for (i, name) in self.joint_names.iter().enumerate() {
            let dq = self.joint_velocity(name);
            self.position_hold.seed_dq_filter(i, dq);
        }
    }

    pub fn position_setpoints(&self) -> Option<&[f64]> {
        self.position_hold.targets()
    }

    /// Planner trajectory reference (for status / tests), not the clamped MIT setpoint.
    pub fn position_hold_commands(&self) -> Option<Vec<f64>> {
        self.position_hold.q_traj()
    }

    pub fn set_joint_position_setpoint(
        &mut self,
        joint: &str,
        position_rad: f64,
    ) -> Result<(), LoopError> {
        if !self.position_hold.is_armed() {
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
        let slew = self
            .supervisor
            .control
            .control
            .joints
            .get(joint)
            .map(|c| c.position_slew_rad_s)
            .unwrap_or(0.15);
        let dq_envelope = envelope_dq_cmd_for_hold_clamp(
            self.supervisor.joint_limit_policy(joint),
            q_now[i],
            requested,
            dq_cmd,
            slew,
        );
        let target = self.clamp_hold_target(joint, q_now[i], requested, dq_envelope);
        let old_target = self
            .position_hold
            .targets()
            .and_then(|sp| sp.get(i).copied())
            .unwrap_or(q_now[i]);
        if (requested - target).abs() > 1e-6 {
            info!(
                joint = %joint,
                requested,
                clamped = target,
                q = q_now[i],
                "hold-at target clamped to limit envelope"
            );
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
        let target_changed = self.position_hold.apply_retarget(HoldRetarget {
            joint_idx: i,
            clamped: target,
            requested,
            q: q_now[i],
            tick: self.tick_count,
            dq_seed: Some(self.joint_velocity(joint)),
            downward_seed: downward_seed_rate,
        });
        // Same-target hold-at must not cancel an in-loop wave (idempotent operator path).
        if target_changed {
            self.position_wave = None;
            info!(
                joint = %joint,
                q = q_now[i],
                old_target,
                new_target = target,
                delta = target - q_now[i],
                tick = self.tick_count,
                "position hold retarget"
            );
        }
        Ok(())
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
        self.position_hold.clear();
        self.position_wave = None;
    }

    /// Start a continuous triangle wave on one joint (Position mode).
    ///
    /// Other joints keep their current latched setpoints. The wave runs in-loop for
    /// `cycles` full min→max→min periods without stdin pacing.
    pub fn start_position_wave(
        &mut self,
        joint: &str,
        min_rad: f64,
        max_rad: f64,
        cycles: u32,
        half_period_sec: f64,
    ) -> Result<f64, LoopError> {
        if min_rad >= max_rad {
            return Err(LoopError::InvalidWaveRange);
        }
        if cycles == 0 {
            return Err(LoopError::InvalidWaveCycles);
        }
        if half_period_sec <= 0.0 {
            return Err(LoopError::InvalidWavePeriod);
        }
        let Some(i) = self.joint_names.iter().position(|n| n == joint) else {
            return Err(LoopError::UnknownJoint {
                joint: joint.to_string(),
            });
        };
        let q = self.refresh_joint_positions();
        let was_armed = self.position_hold.is_armed();
        self.position_hold.ensure_armed_from_q(&q, self.tick_count);
        if !was_armed {
            for (i, name) in self.joint_names.iter().enumerate() {
                self.position_hold
                    .seed_dq_filter(i, self.joint_velocity(name));
            }
        }
        self.ensure_active_for_motion()?;
        self.set_control_mode(ControlMode::Position);
        let half_period_ticks = (half_period_sec * f64::from(self.loop_hz)).round().max(1.0) as u64;
        let wave = PositionWave::new(
            i,
            min_rad,
            max_rad,
            self.tick_count,
            half_period_ticks,
            cycles,
        );
        let duration_sec = wave.duration_sec(self.loop_hz);
        self.position_wave = Some(wave);
        info!(
            joint = %joint,
            min_rad,
            max_rad,
            cycles,
            half_period_sec,
            duration_sec,
            "position wave started"
        );
        Ok(duration_sec)
    }

    pub fn position_wave_active(&self) -> bool {
        self.position_wave
            .as_ref()
            .is_some_and(|w| !w.is_finished())
    }

    /// Latch current `q` and enter [`ControlMode::Position`] (gravity FF + impedance gains).
    pub fn enter_position_hold(&mut self) -> Result<(), LoopError> {
        self.latch_position_setpoints();
        self.ensure_active_for_motion()?;
        self.set_control_mode(ControlMode::Position);
        Ok(())
    }

    /// Re-arm drives after a safety disable via scoped commissioning Enable.
    ///
    /// Never calls [`Supervisor::set_homing_complete`] — Verified is Set Zero only.
    /// Targets come from [`Supervisor::resolve_enable_targets`] (persisted scope or
    /// full-master Robot Ready).
    pub fn ensure_active_for_motion(&mut self) -> Result<(), LoopError> {
        if self.supervisor.mode() == OperationalMode::Active {
            return Ok(());
        }
        let targets = self.supervisor.resolve_enable_targets(&self.repo_root)?;
        self.supervisor.enable_targets(&targets)?;
        Ok(())
    }

    /// MIT / MissingFeedback apply only to Davout `active_joints` while Active.
    fn filter_mit_to_active(&self, batch: Vec<DavoutMit>) -> Vec<DavoutMit> {
        let active = self.supervisor.active_joints();
        batch
            .into_iter()
            .filter(|cmd| active.contains(&cmd.joint))
            .collect()
    }

    /// Enter position hold with an explicit setpoint (single-joint bench: one angle; multi-joint: joint name required).
    pub fn enter_position_hold_at(
        &mut self,
        joint: Option<&str>,
        position_rad: f64,
    ) -> Result<(), LoopError> {
        let q = self.refresh_joint_positions();
        if !self.position_hold.is_armed() {
            self.position_hold.arm(&q, &q, self.tick_count);
            for (i, name) in self.joint_names.iter().enumerate() {
                self.position_hold
                    .seed_dq_filter(i, self.joint_velocity(name));
            }
        }
        if self.joint_names.len() == 1 {
            let name = self.joint_names[0].clone();
            self.set_joint_position_setpoint(&name, position_rad)?;
        } else {
            let joint = joint.ok_or(LoopError::JointNameRequired)?;
            self.set_joint_position_setpoint(joint, position_rad)?;
        }
        self.ensure_active_for_motion()?;
        self.set_control_mode(ControlMode::Position);
        Ok(())
    }

    pub fn supervisor_mut(&mut self) -> &mut Supervisor<B> {
        &mut self.supervisor
    }

    pub fn supervisor(&self) -> &Supervisor<B> {
        &self.supervisor
    }

    /// Current Testing/actuator runtime gain override for `joint`, if any.
    pub fn gain_override(&self, joint_name: &str) -> Option<&GainOverride> {
        self.gains.get(joint_name)
    }

    pub fn set_control_mode(&mut self, mode: ControlMode) {
        let previous = self.control_mode;
        // Capture ramp endpoints before mutating mode or clearing overrides.
        let from = {
            let prev_targets = self.target_gains_for_mode(previous);
            self.gains
                .wire_gains_now(previous, &self.joint_names, &prev_targets)
        };
        let to = self.target_gains_for_mode(mode);
        if mode != ControlMode::Position {
            self.position_hold.clear();
            self.last_position_diag = None;
            self.position_wave = None;
        }
        if previous == ControlMode::TorqueOnly && mode != ControlMode::TorqueOnly {
            self.torque_cmds.clear();
        }
        self.control_mode = mode;
        self.supervisor.set_control_mode(mode);
        if previous != mode {
            info!(?previous, ?mode, "control mode transition");
        }
        let arm_ramp = mode != ControlMode::Disabled && previous != ControlMode::Disabled;
        self.gains.on_mode_enter(previous, mode, &from, &to);
        if arm_ramp {
            // Seed the tau_ff rate limiter with current measured torque so the rate
            // limiter slews from the correct starting point (NOT cleared — clearing
            // causes unclamped torque step via unwrap_or(target)).
            self.supervisor.seed_tau_ff_rate_limiter();
        }
    }

    /// Latch a per-joint open-loop torque command for [`ControlMode::TorqueOnly`].
    ///
    /// Values persist until cleared or until leaving TorqueOnly. Default when unset is 0.
    pub fn set_torque_cmd(&mut self, joint_name: &str, tau_nm: f64) -> Result<(), LoopError> {
        if !self.joint_names.iter().any(|n| n == joint_name) {
            return Err(LoopError::UnknownJoint {
                joint: joint_name.to_string(),
            });
        }
        self.torque_cmds.insert(joint_name.to_string(), tau_nm);
        Ok(())
    }

    /// Remove one joint's latched torque command (reverts to 0).
    pub fn clear_torque_cmd(&mut self, joint_name: &str) {
        self.torque_cmds.remove(joint_name);
    }

    /// Clear all latched TorqueOnly torque commands.
    pub fn clear_torque_cmds(&mut self) {
        self.torque_cmds.clear();
    }

    /// Latched `τ_cmd` for a joint, or `0.0` when unset.
    pub fn torque_cmd(&self, joint_name: &str) -> f64 {
        self.torque_cmds.get(joint_name).copied().unwrap_or(0.0)
    }

    /// Target (kp, kd) per joint from YAML (`gravity_comp` / `impedance`).
    fn target_gains_for_mode(&self, mode: ControlMode) -> Vec<(f64, f64)> {
        const ZERO: ModeGains = ModeGains {
            kp: 0.0,
            kd: 0.0,
            ki: 0.0,
        };
        self.joint_names
            .iter()
            .map(|name| {
                let cfg = self.supervisor.control.control.joints.get(name);
                target_gains_from_yaml(
                    mode,
                    cfg.map(|c| &c.gravity_comp).unwrap_or(&ZERO),
                    cfg.map(|c| &c.impedance).unwrap_or(&ZERO),
                )
            })
            .collect()
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

    // -- Gain override API -------------------------------------------

    /// Apply a per-joint gain override, clamped to motor-type safety limits.
    ///
    /// No-op under GravityComp / TorqueOnly / Disabled so Testing cannot stash
    /// stiffness that snaps back on Impedance/Position enter.
    pub fn apply_gain_override(&mut self, joint_name: &str, gain_override: GainOverride) {
        let limits = self.clamp_limits_for(joint_name);
        self.gains
            .apply(self.control_mode, joint_name, gain_override, limits);
    }

    /// Batch-apply gain overrides for multiple joints.
    ///
    /// No-op under GravityComp / TorqueOnly / Disabled (same policy as
    /// [`Self::apply_gain_override`]).
    pub fn apply_gain_overrides(&mut self, overrides: &HashMap<String, GainOverride>) {
        // Precompute limits: apply_batch's closure cannot borrow `self` while `gains` is mut.
        let limits: HashMap<String, GainClampLimits> = overrides
            .keys()
            .map(|j| (j.clone(), self.clamp_limits_for(j)))
            .collect();
        self.gains
            .apply_batch(self.control_mode, overrides, &limits);
    }

    /// Remove the gain override for a single joint, reverting to config gains.
    pub fn clear_gain_override(&mut self, joint_name: &str) {
        self.gains.clear(joint_name);
    }

    /// Remove all gain overrides, reverting to config gains for every joint.
    pub fn clear_all_overrides(&mut self) {
        self.gains.clear_all();
    }

    /// Resolve [MotorTypeDefaults] for a joint from the control config.
    fn resolve_motor_type_defaults(&self, joint_name: &str) -> Option<&MotorTypeDefaults> {
        let joint_cfg = self.supervisor.control.control.joints.get(joint_name)?;
        let key = motor_type_key(joint_cfg.motor_type);
        self.supervisor.control.control.motor_type_defaults.get(key)
    }

    fn clamp_limits_for(&self, joint_name: &str) -> GainClampLimits {
        let defaults = self.resolve_motor_type_defaults(joint_name);
        GainClampLimits {
            kp_max: defaults.map(|d| d.kp_max).unwrap_or(f64::MAX),
            kd_max: defaults.map(|d| d.kd_max).unwrap_or(f64::MAX),
            tau_ff_max: defaults.map(|d| d.tau_ff_max_nm).unwrap_or(f64::MAX),
        }
    }

    fn joint_mode_gains_yaml(
        &self,
        missing_impedance: &'static ModeGains,
    ) -> Vec<JointModeGains<'_>> {
        static ZERO_STATIC: ModeGains = ModeGains {
            kp: 0.0,
            kd: 0.0,
            ki: 0.0,
        };
        self.joint_names
            .iter()
            .map(|name| {
                let cfg = self.supervisor.control.control.joints.get(name);
                JointModeGains {
                    gravity_comp: cfg.map(|c| &c.gravity_comp).unwrap_or(&ZERO_STATIC),
                    impedance: cfg.map(|c| &c.impedance).unwrap_or(missing_impedance),
                }
            })
            .collect()
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
        let active_names: Vec<&String> = self.supervisor.active_joints().iter().collect();
        let all_have_feedback = active_names
            .iter()
            .all(|name| self.has_joint_feedback(name));
        // First tick (or first ticks after enable) may run before CAN status arrives.
        let feedback_bootstrap = needs_joint_feedback
            && !all_have_feedback
            && (self.tick_count == 0 || self.active_feedback_grace_ticks > 0);
        if needs_joint_feedback && !feedback_bootstrap {
            for name in &active_names {
                if !self.has_joint_feedback(name) {
                    return Err(LoopError::MissingFeedback {
                        joint: (*name).clone(),
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

                let log_position_diag = self.control_mode == ControlMode::Position
                    && self
                        .last_position_diag
                        .map(|t| t.elapsed() >= Duration::from_secs(1))
                        .unwrap_or(true);
                let mut batch = Vec::new();
                let mut trace_us_this_tick = 0u64;

                if self.control_mode == ControlMode::Position {
                    static POSITION_DEFAULT_IMPEDANCE: ModeGains = ModeGains {
                        kp: 20.0,
                        kd: 1.0,
                        ki: 0.0,
                    };
                    let yaml = self.joint_mode_gains_yaml(&POSITION_DEFAULT_IMPEDANCE);
                    let resolved =
                        self.gains
                            .resolve_all(self.control_mode, &self.joint_names, &yaml);
                    let dq_meas: Vec<f64> = self
                        .joint_names
                        .iter()
                        .map(|name| self.joint_velocity(name))
                        .collect();
                    let joint_params: Vec<HoldJointParams> = self
                        .joint_names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let r = &resolved[i];
                            let mut friction = cfg.map(|c| c.friction.clone());
                            if let Some(fc) = r.law_fc {
                                if let Some(ref mut f) = friction {
                                    f.fc = fc;
                                }
                            }
                            HoldJointParams {
                                kp: r.law_kp,
                                kd: r.law_kd,
                                ki: r.law_ki,
                                max_lead: cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.15),
                                vel_deadband: cfg
                                    .map(|c| c.position_trajectory_velocity_deadband_rad)
                                    .unwrap_or(0.02),
                                advance_max_lead: cfg
                                    .map(|c| c.position_slew_max_lead_rad)
                                    .unwrap_or(ADVANCE_MAX_LEAD_DEFAULT),
                                advance_vel_deadband: cfg
                                    .map(|c| c.position_trajectory_velocity_deadband_rad)
                                    .unwrap_or(POSITION_HOLD_ERROR_DEADBAND_RAD),
                                slew_rad_s: cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25),
                                trajectory_v_max: cfg
                                    .map(|c| c.position_trajectory_velocity_rad_s)
                                    .unwrap_or(0.30),
                                trajectory_threshold_rad: cfg
                                    .map(|c| c.position_trajectory_threshold_rad)
                                    .unwrap_or(0.15),
                                a_max: cfg
                                    .map(|c| c.position_trajectory_accel_rad_s2)
                                    .unwrap_or(0.20),
                                velocity_cap: self.supervisor.joint_velocity_cap(name),
                                friction,
                                limit_policy: self.supervisor.joint_limit_policy(name).cloned(),
                                tau_meas: self.joint_torque(name),
                            }
                        })
                        .collect();
                    let hold_out = {
                        let world = HoldWorld {
                            q: &q,
                            dq_meas: &dq_meas,
                            tau_g: tau_g.as_slice(),
                            joints: &joint_params,
                            joint_names: &self.joint_names,
                            dt: self.loop_period.as_secs_f64(),
                            hz: self.loop_hz,
                            tick_count: self.tick_count,
                            wave: &mut self.position_wave,
                        };
                        self.position_hold.tick(world)?
                    };
                    (phase.planner_us, t) = phase_elapsed_us(t);
                    for (i, mut cmd) in hold_out.mit.into_iter().enumerate() {
                        // Position MIT wire: compose owns `kd` (`kd_mit`, usually 0).
                        // Resolved `wire_kp` may scale kp (override > ramp > law);
                        // never rewrite `kd`.
                        cmd.kp = resolved[i].wire_kp;
                        batch.push(cmd);
                    }
                    for (i, d) in hold_out.diag.iter().enumerate() {
                        let name = &self.joint_names[i];
                        if log_position_diag {
                            info!(
                                joint = %name,
                                q = d.q,
                                dq = d.dq_raw,
                                dq_filt = d.dq_filt,
                                q_traj = d.q_traj,
                                dq_traj = d.dq_traj,
                                q_des = d.q_des,
                                target = d.target,
                                lead = d.lead,
                                lead_sat = d.lead_sat,
                                settle_error = d.settle_error,
                                settling = d.settling,
                                friction_mode = d.friction_mode,
                                retarget_age_ms = d.retarget_age_ms,
                                joint_stuck = d.joint_stuck,
                                planner_frozen = d.planner_frozen,
                                phase = %d.phase,
                                kp = d.kp,
                                kd = d.kd,
                                tau_g = d.tau_g,
                                tau_f = d.tau_f,
                                tau_d = d.tau_d,
                                tau_ff_cmd = d.tau_ff_cmd,
                                tau_meas = d.tau_meas,
                                tau_err = d.tau_meas - d.tau_ff_cmd,
                                tau_p = d.tau_p,
                                dq_mit = d.mit_velocity,
                                kp_mit = d.kp,
                                kd_mit = d.mit_kd,
                                move_dist = d.move_dist,
                                v_max_eff = d.v_max_eff,
                                "position hold command"
                            );
                        }
                        if should_log_position_onset(d.retarget_tick, self.tick_count, self.loop_hz)
                        {
                            debug!(
                                joint = %name,
                                retarget_age_ms = d.retarget_age_ms,
                                q = d.q,
                                dq = d.dq_raw,
                                dq_filt = d.dq_filt,
                                q_traj = d.q_traj,
                                dq_traj = d.dq_traj,
                                lead = d.lead,
                                settle_error = d.settle_error,
                                settling = d.settling,
                                friction_mode = d.friction_mode,
                                tau_f = d.tau_f,
                                tau_d = d.tau_d,
                                tau_ff_cmd = d.tau_ff_cmd,
                                phase = %d.phase,
                                joint_stuck = d.joint_stuck,
                                planner_frozen = d.planner_frozen,
                                move_dist = d.move_dist,
                                v_max_eff = d.v_max_eff,
                                "position hold onset"
                            );
                        }
                        if let Some(trace) = self.position_trace.as_mut() {
                            let trace_start = Instant::now();
                            let t_ms =
                                self.tick_count.saturating_mul(1000) / u64::from(self.loop_hz);
                            let row = PositionTraceRow {
                                joint: name,
                                q: d.q,
                                dq: d.dq_raw,
                                q_traj: d.q_traj,
                                dq_traj: d.dq_traj,
                                q_des: d.q_des,
                                target: d.target,
                                target_raw: d.target_raw,
                                q_env_lo: d.q_env_lo,
                                q_env_hi: d.q_env_hi,
                                lead: d.lead,
                                lead_sat: d.lead_sat,
                                settle_error: d.settle_error,
                                phase: &d.phase,
                                friction_mode: d.friction_mode,
                                tau_p: d.tau_p,
                                tau_g: d.tau_g,
                                tau_f: d.tau_f,
                                tau_d: d.tau_d,
                                tau_ff_cmd: d.tau_ff_cmd,
                                tau_meas: d.tau_meas,
                                dq_mit: d.mit_velocity,
                                kp: d.kp,
                                kd: d.kd,
                                joint_stuck: d.joint_stuck,
                                planner_frozen: d.planner_frozen,
                                retarget_age_ms: d.retarget_age_ms,
                                planner_event: d.planner_event.as_str(),
                            };
                            let _ = trace.maybe_record(self.tick_count, t_ms, &row);
                            trace_us_this_tick = trace_us_this_tick.saturating_add(
                                u64::try_from(trace_start.elapsed().as_micros())
                                    .unwrap_or(u64::MAX),
                            );
                        }
                    }
                    if log_position_diag {
                        self.last_position_diag = Some(Instant::now());
                    }
                } else {
                    (phase.planner_us, t) = phase_elapsed_us(t);
                    static ZERO_IMPEDANCE: ModeGains = ModeGains {
                        kp: 0.0,
                        kd: 0.0,
                        ki: 0.0,
                    };
                    let yaml = self.joint_mode_gains_yaml(&ZERO_IMPEDANCE);
                    let resolved =
                        self.gains
                            .resolve_all(self.control_mode, &self.joint_names, &yaml);
                    let ff_joints: Vec<MitFfJointIn> = self
                        .joint_names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let r = &resolved[i];
                            MitFfJointIn {
                                name: name.clone(),
                                q: q[i],
                                dq: self.joint_velocity(name),
                                tau_g: tau_g[i],
                                tau_cmd: self.torque_cmd(name),
                                friction: cfg.map(|c| c.friction.clone()),
                                wire_kp: r.wire_kp,
                                wire_kd: r.wire_kd,
                                fc: r.law_fc,
                            }
                        })
                        .collect();
                    batch = MitFeedforward::compose(self.control_mode, &ff_joints);
                }
                phase.trace_us = trace_us_this_tick;
                (phase.compose_us, t) = phase_elapsed_us(t);

                let batch = self.filter_mit_to_active(batch);
                self.supervisor.send_mit_batch(batch)?;
                (phase.send_us, t) = phase_elapsed_us(t);
                let _ = self.supervisor.drain_feedback();
                self.gains.advance_tick();
            } else {
                // Robstride only streams status after MIT frames; hold current q with zero
                // gains/torque so comm watchdog stays fresh between enable and gravity-on.
                // Scoped Enable: keepalive only for Davout active_joints.
                let active = self.supervisor.active_joints();
                let batch: Vec<DavoutMit> = self
                    .joint_names
                    .iter()
                    .zip(q.iter())
                    .filter(|(name, _)| active.contains(*name))
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

    /// Poll bus feedback, then read cached joint positions for planner init.
    fn refresh_joint_positions(&mut self) -> Vec<f64> {
        let _ = self.supervisor.drain_feedback();
        self.read_positions()
    }

    fn has_joint_feedback(&self, joint: &str) -> bool {
        self.supervisor.joint_feedback(joint).is_some()
    }

    fn read_positions(&self) -> Vec<f64> {
        self.joint_names
            .iter()
            .map(|name| {
                self.supervisor
                    .joint_feedback(name)
                    .map(|s| s.position_rad)
                    .unwrap_or(0.0)
            })
            .collect()
    }

    fn joint_velocity(&self, joint: &str) -> f64 {
        self.supervisor
            .joint_feedback(joint)
            .map(|s| s.velocity_rad_s)
            .unwrap_or(0.0)
    }

    fn joint_torque(&self, joint: &str) -> f64 {
        self.supervisor
            .joint_feedback(joint)
            .map(|s| s.torque_nm)
            .unwrap_or(0.0)
    }

    fn publish_robot_state(&self, chappe: &Bus, q: &[f64]) -> Result<(), LoopError> {
        // Presence = fresh CAN feedback. Davout omits stale free-drive samples
        // (FREE_DRIVE_FEEDBACK_TTL) so Consul Online tracks recent RX, not sticky cache.
        let joints: Vec<JointState> = self
            .joint_names
            .iter()
            .zip(q.iter())
            .filter_map(|(name, &position)| {
                let state = self.supervisor.joint_feedback(name)?;
                let (homing_state, drive_active, out_of_limits) =
                    self.supervisor.joint_commissioning_wire(name);
                Some(JointState {
                    name: name.clone(),
                    position,
                    velocity: state.velocity_rad_s,
                    effort: state.torque_nm,
                    temperature_c: state.temperature_c,
                    fault: u32::from(state.fault),
                    homing_state,
                    drive_active,
                    out_of_limits,
                })
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
        self.position_hold.planner_state(i)
    }

    /// Test-only: force Hold at `q_traj` (residual lead-follow / AscentStall scenarios).
    #[cfg(test)]
    pub fn test_force_planner_hold_at(
        &mut self,
        joint: &str,
        q_traj: f64,
    ) -> Result<(), LoopError> {
        let i = self
            .joint_names
            .iter()
            .position(|n| n == joint)
            .ok_or_else(|| LoopError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        self.position_hold.force_planner_hold_at(i, q_traj)?;
        Ok(())
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
    use crate::position_hold::POSITION_ASCENT_STALL_FAULT_MS;
    use crate::position_setpoint::{
        apply_lead_follow_hold_short, approach_stuck_mit_pull, clamp_trajectory_setpoint,
        descent_breakaway_confirmed, descent_stuck_mit_pull, planner_drifted_from_measurement,
        planner_overshoot_hold_while_moving, planner_premature_hold,
        planner_should_freeze_on_ascent_stall, planner_should_freeze_on_descent,
        planner_should_latch_on_overshoot_hold, planner_should_lead_follow_hold_short,
        planner_should_reopen_premature_hold, planner_should_resync_stuck_lead,
        position_hold_effective_max_lead, position_hold_mit_kd, position_hold_mit_velocity,
        reopen_planner_from_premature_hold,
    };
    use crate::position_trajectory::{JointPositionPlanner, TrapezoidPhase};
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
        // Stay Ready (do not call enter_position_hold*, which re-arms Active).
        loop_ctrl.tick(None).expect("tick");
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
    fn tick_partial_enable_sends_mit_only_for_active_joints() {
        let mut loop_ctrl = test_loop();
        let motors = loop_ctrl.supervisor_mut().motors.motors.clone();
        loop_ctrl
            .supervisor_mut()
            .homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("verify");
        let one = motors[0].joint.clone();
        loop_ctrl
            .supervisor_mut()
            .enable_targets(&[one.clone()])
            .expect("scoped enable");
        loop_ctrl.supervisor_mut().seed_synthetic_feedback();
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Active);
        loop_ctrl.supervisor_mut().bus_mut().tx.clear();
        loop_ctrl.tick(None).expect("keepalive tick");
        assert_eq!(
            loop_ctrl.supervisor_mut().bus_mut().tx.len(),
            1,
            "keepalive MIT must cover only active_joints"
        );
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        loop_ctrl.supervisor_mut().bus_mut().tx.clear();
        loop_ctrl.tick(None).expect("gravity tick");
        assert_eq!(
            loop_ctrl.supervisor_mut().bus_mut().tx.len(),
            1,
            "GravityComp MIT must cover only active_joints"
        );
        assert!(loop_ctrl.supervisor().active_joints().contains(&one));
    }

    #[test]
    fn hold_at_reenables_after_safety_disable() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        loop_ctrl.supervisor_mut().disable_all().expect("disable");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Disabled);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.0)
            .expect("hold-at home");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Active);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    }

    #[test]
    fn position_mode_without_feedback_errors_when_active() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.tick(None).expect("seed tick with feedback");
        loop_ctrl.supervisor_mut().clear_motor_states();
        loop_ctrl.set_control_mode(ControlMode::Position);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
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
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
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
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.enter_position_hold().expect("hold");
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
        let sp = loop_ctrl.position_setpoints().expect("latched setpoints");
        assert_eq!(sp.len(), loop_ctrl.joint_names.len());
    }

    #[test]
    fn hold_at_sets_joint_target() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.42)
            .expect("hold-at");
        let sp = loop_ctrl.position_setpoints().expect("setpoints");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint index");
        assert!((sp[i] - 0.42).abs() < 1e-9);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    }

    #[test]
    fn same_target_hold_at_preserves_active_wave() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        let joint = "right_shoulder_pitch";
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.1)
            .expect("hold-at");
        loop_ctrl
            .start_position_wave(joint, 0.0, 0.2, 2, 0.5)
            .expect("wave");
        assert!(loop_ctrl.position_wave_active());
        loop_ctrl
            .set_joint_position_setpoint(joint, 0.1)
            .expect("same-target hold-at");
        assert!(
            loop_ctrl.position_wave_active(),
            "same-target hold-at must not cancel an in-loop wave"
        );
        loop_ctrl
            .set_joint_position_setpoint(joint, 0.25)
            .expect("changed hold-at");
        assert!(
            !loop_ctrl.position_wave_active(),
            "changed target must cancel the wave"
        );
    }

    #[test]
    fn leaving_position_mode_clears_setpoints() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.enter_position_hold().expect("hold");
        assert!(loop_ctrl.position_setpoints().is_some());
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(loop_ctrl.position_setpoints().is_none());
        assert_eq!(loop_ctrl.control_mode(), ControlMode::GravityComp);
    }

    #[test]
    fn torque_cmd_defaults_zero_and_latches() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        assert!((loop_ctrl.torque_cmd(joint)).abs() < 1e-12);
        loop_ctrl.set_torque_cmd(joint, 0.25).expect("set");
        assert!((loop_ctrl.torque_cmd(joint) - 0.25).abs() < 1e-12);
        loop_ctrl.clear_torque_cmd(joint);
        assert!((loop_ctrl.torque_cmd(joint)).abs() < 1e-12);
    }

    #[test]
    fn leaving_torque_only_clears_torque_cmds() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        loop_ctrl.set_control_mode(ControlMode::TorqueOnly);
        loop_ctrl.set_torque_cmd(joint, 0.4).expect("set");
        assert!((loop_ctrl.torque_cmd(joint) - 0.4).abs() < 1e-12);
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(
            (loop_ctrl.torque_cmd(joint)).abs() < 1e-12,
            "leave TorqueOnly must clear τ_cmd latch"
        );
    }

    #[test]
    fn set_torque_cmd_rejects_unknown_joint() {
        let mut loop_ctrl = test_loop();
        let err = loop_ctrl
            .set_torque_cmd("not_a_joint", 0.1)
            .expect_err("unknown");
        assert!(matches!(err, LoopError::UnknownJoint { .. }));
    }

    #[test]
    fn gravity_comp_enter_clears_sticky_gain_overrides() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        loop_ctrl.apply_gain_override(
            "right_shoulder_pitch",
            GainOverride {
                kp: 50.0,
                kd: 5.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_some());
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(
            loop_ctrl.gain_override("right_shoulder_pitch").is_none(),
            "Impedance→GravityComp must clear Testing overrides"
        );
    }

    #[test]
    fn apply_gain_override_ignored_under_gravity_comp() {
        let mut loop_ctrl = test_loop();
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        loop_ctrl.apply_gain_override(
            "right_shoulder_pitch",
            GainOverride {
                kp: 50.0,
                kd: 5.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        assert!(
            loop_ctrl.gain_override("right_shoulder_pitch").is_none(),
            "must not stash overrides under GravityComp"
        );
        // Enter Impedance must not resurrect planted stiffness.
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
    }

    #[test]
    fn apply_gain_override_ignored_under_disabled() {
        let mut loop_ctrl = test_loop();
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
        loop_ctrl.apply_gain_override(
            "right_shoulder_pitch",
            GainOverride {
                kp: 50.0,
                kd: 5.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
    }

    #[test]
    fn position_hold_tick_runs_when_active() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Active);
    }

    #[test]
    fn position_hold_advances_trapezoid_on_first_tick() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
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
    fn clamp_always_bounds_lead_when_lag_exceeds_max_lead() {
        // Bench grind case: q stuck, q_traj already at target — must not open-loop to 0.15.
        let q_des = clamp_trajectory_setpoint(0.15, 0.02, 0.15, 0.10, None, 0.0);
        assert!(
            (q_des - 0.12).abs() < 1e-12,
            "q_des must be q+max_lead=0.12, got {q_des}"
        );
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
        // Descent retarget start (wave reverse): q_traj still above target — must NOT latch.
        assert!(!planner_should_latch_on_overshoot_hold(
            0.76, 0.76, 0.4, 0.0, 0.02
        ));
    }

    #[test]
    fn clamp_does_not_jump_on_descent_retarget_start() {
        // Wave reverse: q_traj still at measured q above a lower target — follow lead, don't snap.
        let q_des = clamp_trajectory_setpoint(0.76, 0.76, 0.4, 0.12, None, 0.0);
        assert!(
            (q_des - 0.76).abs() < 0.13,
            "descent retarget must not command target before planner arrives: {q_des}"
        );
        assert!(
            (q_des - 0.4).abs() > 0.2,
            "q_des must stay near q, not jump to 0.4: {q_des}"
        );
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
    fn mit_kd_engages_on_velocity_during_motion() {
        // Moving toward or past target with dq > deadband → kd engages
        assert!((position_hold_mit_kd(2.0, 1.62, 1.57, 0.08, 0.02) - 2.0).abs() < 1e-12);
        // Below deadband → kd remains 0.0
        assert!((position_hold_mit_kd(2.0, 1.62, 1.57, 0.01, 0.02)).abs() < 1e-12);
        // Moving toward target (not yet past) → kd engages (new behavior)
        assert!((position_hold_mit_kd(2.0, 1.55, 1.57, 0.08, 0.02) - 2.0).abs() < 1e-12);
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
        use crate::position_setpoint::POSITION_DESCENT_STUCK_LEAD_RAD;
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
        assert!(
            (small - 0.10).abs() < 1e-12,
            "small outbound below descent-seed threshold: onset-only, no sustained knee boost"
        );
        let return_high = position_hold_effective_max_lead(0.10, 0, true, -1.545, 1.645);
        assert!(
            (return_high - 0.15).abs() < 1e-12,
            "return from high q gets onset lead boost"
        );
        let outbound_knee = position_hold_effective_max_lead(0.10, 500, true, 0.05, 0.10);
        assert!(
            (outbound_knee - 0.10).abs() < 1e-12,
            "outbound knee after onset: no sustained lead boost"
        );
        let staging_descent = position_hold_effective_max_lead(0.10, 500, false, -0.15, 0.25);
        assert!(
            (staging_descent - 0.15).abs() < 1e-12,
            "sustained boost on staged descent near 15 deg"
        );
        let lower_limit = position_hold_effective_max_lead(0.10, 0, true, -0.64, 0.0);
        assert!(
            (lower_limit - 0.10).abs() < 1e-12,
            "limit-directed negative move must not get return assist boost"
        );
    }

    #[test]
    fn outbound_hold_at_015_no_sustained_lead_boost_after_onset() {
        let lead = position_hold_effective_max_lead(0.10, 500, true, 0.05, 0.10);
        assert!(
            (lead - 0.10).abs() < 1e-12,
            "hold-at 0.15 after onset must keep max_lead=0.10 so stuck-lead resync can fire"
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
        let effective = position_hold_effective_max_lead(raw_max_lead, 100, true, target - q, q);
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
        let effective = position_hold_effective_max_lead(raw_max_lead, 301, true, target - q, q);
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
        let effective = position_hold_effective_max_lead(raw_max_lead, 100, true, target - q, q);
        assert!(
            !planner_should_resync_stuck_lead(&planner, q, target, 0.0, effective, 0.02),
            "FIX A: resync uses effective max_lead; q+0.10 within 0.15 → no resync"
        );
        assert!(
            planner_should_resync_stuck_lead(&planner, q, target, 0.0, raw_max_lead, 0.02),
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
        assert!(
            (after_boundary - raw).abs() < 1e-9,
            "boost expired at 301ms"
        );
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
    fn approach_stuck_mit_pull_disabled_on_ascent() {
        use crate::position_setpoint::{
            approach_stuck_mit_pull_lead_rad, outbound_low_angle_stuck,
            outbound_low_angle_stuck_pull_rad,
        };
        let q = 0.18;
        let target = 0.262;
        let to_target = target - q;
        let lag = 0.15;
        let max_lead = 0.15;
        let deadband = 0.02;
        assert!(outbound_low_angle_stuck(
            q, target, to_target, 0.0, deadband, lag, max_lead
        ));
        assert!(!outbound_low_angle_stuck(
            q, target, to_target, 0.0, deadband, 0.10, max_lead
        ));
        // Ascent pull-harder disabled — helpers remain but must not enable.
        assert!(!approach_stuck_mit_pull(
            to_target,
            q,
            target,
            q + lag,
            0.0,
            0.10,
            deadband,
            max_lead,
        ));
        assert!(!approach_stuck_mit_pull(
            to_target, 0.35, 0.45, 0.50, 0.0, 0.10, deadband, max_lead,
        ));
        assert!((outbound_low_angle_stuck_pull_rad(to_target, max_lead) - to_target).abs() < 1e-9);
        assert!(
            (approach_stuck_mit_pull_lead_rad(to_target, lag, max_lead) - to_target).abs() < 1e-9
        );
    }

    #[test]
    fn ascent_stall_freezes_planner() {
        let q = 0.02;
        let target = 0.15;
        let q_traj = 0.125;
        let to_target = target - q;
        let deadband = 0.02;
        assert!(planner_should_freeze_on_ascent_stall(
            false, target, q, q_traj, to_target, 0.0, deadband
        ));
        // Synced → exit
        assert!(!planner_should_freeze_on_ascent_stall(
            true,
            target,
            q,
            q + 0.01,
            to_target,
            0.0,
            deadband
        ));
        // Motion toward target → exit
        assert!(!planner_should_freeze_on_ascent_stall(
            true, target, q, q_traj, to_target, 0.03, deadband
        ));
        // Home return owned by descent freeze
        assert!(!planner_should_freeze_on_ascent_stall(
            false, 0.0, 0.08, 0.02, -0.08, 0.0, deadband
        ));
    }

    #[test]
    fn ascent_stall_faults_tick_after_sustained_freeze() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        let joint = "right_shoulder_pitch";
        loop_ctrl
            .supervisor_mut()
            .set_synthetic_joint_feedback(joint, 0.02, 0.0)
            .expect("feedback");
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.15)
            .expect("hold-at");
        let mut faulted = false;
        for _ in 0..600 {
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, 0.02, 0.0)
                .expect("feedback");
            match loop_ctrl.tick(None) {
                Ok(()) => {}
                Err(err) => {
                    assert!(
                        matches!(
                            &err,
                            LoopError::AscentStall { joint: j, ms }
                                if j == joint && *ms >= POSITION_ASCENT_STALL_FAULT_MS
                        ),
                        "unexpected tick error: {err:?}"
                    );
                    faulted = true;
                    break;
                }
            }
        }
        assert!(
            faulted,
            "stuck outbound ascent must AscentStall within ~3s of freeze"
        );
    }

    #[test]
    fn ascent_stall_counter_resets_on_progress_toward_target() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        let joint = "right_shoulder_pitch";
        loop_ctrl
            .supervisor_mut()
            .set_synthetic_joint_feedback(joint, 0.02, 0.0)
            .expect("feedback");
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.15)
            .expect("hold-at");
        // Build freeze / partial stall (~0.5 s stuck).
        for _ in 0..100 {
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, 0.02, 0.0)
                .expect("feedback");
            loop_ctrl.tick(None).expect("pre-progress tick");
        }
        // Crawl toward target below exit_v — must reset fuse, not disable.
        for _ in 0..500 {
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, 0.02, 0.01)
                .expect("feedback");
            loop_ctrl
                .tick(None)
                .expect("progress toward target must not AscentStall");
        }
    }

    #[test]
    fn lead_follow_residual_does_not_ascent_stall() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        let joint = "right_shoulder_pitch";
        // Remaining 0.05 ∈ (resync 0.03, max_lead] — lead-follow residual, not true stall.
        loop_ctrl
            .supervisor_mut()
            .set_synthetic_joint_feedback(joint, 0.10, 0.0)
            .expect("feedback");
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.15)
            .expect("hold-at");
        loop_ctrl
            .test_force_planner_hold_at(joint, 0.15)
            .expect("force Hold@target");
        for _ in 0..500 {
            loop_ctrl
                .supervisor_mut()
                .set_synthetic_joint_feedback(joint, 0.10, 0.0)
                .expect("feedback");
            loop_ctrl
                .tick(None)
                .expect("lead-follow residual must not AscentStall");
        }
    }

    #[test]
    fn hold_short_of_target_lead_follows_from_measured_q() {
        // Bench creep: Hold at 0.15 with q≈0.13 (inside premature 30 mrad band).
        let mut planner = JointPositionPlanner::new_for_target(0.13, 0.15);
        planner.q_traj = 0.15;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(
            !planner_premature_hold(&planner, 0.13, 0.15),
            "0.02 rad short is inside return_settle_band — premature path does not fire"
        );
        assert!(planner_should_lead_follow_hold_short(
            &planner, 0.13, 0.15, 0.0, 0.02
        ));
        assert!(!planner_should_lead_follow_hold_short(
            &planner, 0.148, 0.15, 0.0, 0.02
        ));
        apply_lead_follow_hold_short(&mut planner, 0.13, 0.15, 0.12, 0.35);
        assert!(
            (planner.q_traj - 0.15).abs() < 1e-12,
            "residual < max_lead ⇒ q_traj at target (full remaining lead)"
        );
        assert!(
            planner.dq_traj > 0.0,
            "non-zero cruise dq keeps traj friction during residual finish"
        );
        assert_eq!(planner.phase(), TrapezoidPhase::Cruise);
        // Must keep lead-following while Cruise@target so tick stays frozen.
        assert!(planner_should_lead_follow_hold_short(
            &planner, 0.13, 0.15, 0.0, 0.02
        ));
    }

    #[test]
    fn home_return_lead_follows_from_above_not_past_home() {
        // Bench 1.57 return: stuck Hold at q≈0.018 with q_traj=0 — must finish downward.
        let mut above = JointPositionPlanner::new_for_target(0.018, 0.0);
        above.q_traj = 0.0;
        above.dq_traj = 0.0;
        above.force_hold_for_test();
        assert!(
            planner_should_lead_follow_hold_short(&above, 0.018, 0.0, 0.0, 0.02),
            "home residual from above must lead-follow downward"
        );
        apply_lead_follow_hold_short(&mut above, 0.018, 0.0, 0.12, 0.35);
        assert!(
            above.dq_traj < 0.0,
            "home residual cruise must be toward home (negative)"
        );

        // Past-home (pull-through undershoot): must NOT lead-follow back up — that oscillates.
        let mut past = JointPositionPlanner::new_for_target(-0.006, 0.0);
        past.q_traj = 0.0;
        past.dq_traj = 0.0;
        past.force_hold_for_test();
        assert!(
            !planner_should_lead_follow_hold_short(&past, -0.006, 0.0, 0.0, 0.02),
            "past-home Hold must not lead-follow upward"
        );
        // Active Cruise@target with +dq (the oscillating bench state) also rejected.
        past.resume_cruise_toward(0.0, 0.35);
        assert!(
            !planner_should_lead_follow_hold_short(&past, -0.006, 0.0, 0.08, 0.02),
            "past-home Cruise@target +dq must not keep lead-following"
        );
    }

    #[test]
    fn stuck_premature_hold_skips_reopen() {
        let mut planner = JointPositionPlanner::new_for_target(0.02, 0.15);
        planner.q_traj = 0.15;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(planner_premature_hold(&planner, 0.02, 0.15));
        assert!(
            !planner_should_reopen_premature_hold(&planner, 0.02, 0.15, 0.0, 0.02),
            "stuck premature hold must not reopen (Reset oscillator)"
        );
        assert!(
            planner_should_lead_follow_hold_short(&planner, 0.02, 0.15, 0.0, 0.02),
            "large shortfall lead-follows from measured q"
        );
        // Moving toward target may reopen
        assert!(planner_should_reopen_premature_hold(
            &planner, 0.02, 0.15, 0.05, 0.02
        ));
        // Overshoot while moving still reopens
        let mut overshoot = JointPositionPlanner::new_for_target(1.5, 1.57);
        overshoot.q_traj = 1.57;
        overshoot.dq_traj = 0.0;
        overshoot.force_hold_for_test();
        assert!(planner_should_reopen_premature_hold(
            &overshoot, 1.62, 1.57, 0.08, 0.02
        ));
    }

    #[test]
    fn stuck_premature_hold_does_not_thrash_to_cruise_at_target() {
        // Integration-style gate: q_traj=target=0.15, q=0.02, dq=0.
        // Old path reopened Cruise with q_traj still 0.15 → Hold → Reset storm.
        let mut planner = JointPositionPlanner::new_for_target(0.02, 0.15);
        planner.q_traj = 0.15;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        let q = 0.02;
        let target = 0.15;
        let max_lead = 0.10;
        let deadband = 0.02;
        assert!(!planner_should_reopen_premature_hold(
            &planner, q, target, 0.0, deadband
        ));
        assert!(planner_should_resync_stuck_lead(
            &planner, q, target, 0.0, max_lead, deadband
        ));
        planner.reset_target(q, target);
        assert!(
            (planner.q_traj - q).abs() < 1e-12,
            "resync must snap q_traj to measured q, not leave it at target"
        );
        assert!(
            (planner.q_traj - target).abs() > 1e-6,
            "must not keep q_traj parked at target while arm is stuck short"
        );
        assert_ne!(
            planner.phase(),
            TrapezoidPhase::Cruise,
            "resync must not reopen Cruise at the latched target"
        );
    }

    #[test]
    fn envelope_hold_clamp_allows_home_target_from_high_q() {
        use crate::position_setpoint::{envelope_dq_cmd_for_hold_clamp, POSITION_HOME_SETTLE_RAD};
        use armee_kinematics::{JointLimitBounds, JointLimitPolicy, LimitMarginConfig};

        let policy = JointLimitPolicy {
            bounds: JointLimitBounds::from_hard_and_soft(0.0, 3.14159, None, None),
            margin: LimitMarginConfig {
                min_rad: 0.01,
                k_v_s: 0.02,
                k_stop: 0.5,
                decel_rad_s2: 4.5,
                velocity_deadband_rad_s: 0.02,
                measured_fault_slack_rad: 0.005,
            },
            velocity: 1.25,
            effort: 5.0,
            tau_ff_max: 5.0,
        };
        let q = 0.286;
        let requested = 0.0;
        let dq_cruise = -1.25;
        let slew = 0.15;
        let dq_env = envelope_dq_cmd_for_hold_clamp(Some(&policy), q, requested, dq_cruise, slew);
        assert!(
            dq_env.abs() <= slew + 1e-9,
            "envelope dq should use slew, got {dq_env}"
        );
        let clamped = armee_kinematics::clamp_hold_target(&policy, q, dq_env, requested);
        assert!(
            clamped.abs() <= POSITION_HOME_SETTLE_RAD,
            "home hold-at must not clamp to margin band: got {clamped}"
        );
    }

    #[test]
    fn home_final_approach_stuck_enables_mit_pull_and_unfreezes_planner() {
        use crate::position_setpoint::{
            home_final_approach_stuck, home_final_approach_stuck_pull_rad,
            POSITION_HOME_FINAL_PULL_THROUGH_RAD,
        };
        assert!(home_final_approach_stuck(0.031, 0.0));
        assert!(!home_final_approach_stuck(0.003, 0.0));
        assert!(home_final_approach_stuck(0.06, 0.0));
        assert!(home_final_approach_stuck(0.14, 0.0));
        assert!(!home_final_approach_stuck(0.16, 0.0));
        assert!(descent_stuck_mit_pull(-0.031, 0.031, 0.0, 0.0, 0.02, false));
        assert!(descent_stuck_mit_pull(-0.14, 0.14, 0.0, 0.0, 0.02, false));
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
        // MIT pull-through replaces planner freeze in the 5–150 mrad home band.
        for q in [0.01_f64, 0.04, 0.05, 0.12, 0.15] {
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
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 1.2)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint");
        let cmd = loop_ctrl.position_hold_commands().expect("cmd")[i];
        assert!(cmd < 0.2, "first tick must not jump to 1.2 rad target");
    }

    #[test]
    fn hold_at_ramps_command_not_instant_setpoint() {
        let mut loop_ctrl = test_loop();
        bench_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 1.2)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint index");
        let target = loop_ctrl.position_setpoints().expect("target")[i];
        assert!((target - 1.2).abs() < 1e-9);
        let cmd0 = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(cmd0.abs() < 1e-6, "command starts at measured q≈0");
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
        // Past soft/hard lower (~-0.87/-0.9) so clamp_hold_target must raise the goal.
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), -0.95)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint index");
        let target = loop_ctrl.position_setpoints().expect("setpoints")[i];
        assert!(
            target > -0.95,
            "requested lower-limit probe must clamp before planner reset"
        );
        let (_q_traj, dq_traj) = loop_ctrl
            .test_planner_state("right_shoulder_pitch")
            .expect("planner");
        assert!(
            dq_traj.abs() < 1e-12,
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
        let joint = "right_shoulder_pitch";
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
        let joint = "right_shoulder_pitch";
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

    // ════════════════════════════════════════════════════════════════
    // Gain override tests
    // ════════════════════════════════════════════════════════════════

    fn apply_override_in_impedance(
        loop_ctrl: &mut ControlLoop<MemoryBus>,
        joint: &str,
        ov: GainOverride,
    ) {
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        loop_ctrl.apply_gain_override(joint, ov);
    }

    #[test]
    fn apply_gain_override_clamps_kp_to_kp_max() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 6000.0,
                kd: 1.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!(
            (stored.kp - 5000.0).abs() < 1e-9,
            "kp clamped: {}",
            stored.kp
        );
    }

    #[test]
    fn apply_gain_override_clamps_kd_to_kd_max() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 10.0,
                kd: 200.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!(
            (stored.kd - 100.0).abs() < 1e-9,
            "kd clamped: {}",
            stored.kd
        );
    }

    #[test]
    fn apply_gain_override_clamps_fc_to_tau_ff_max_nm() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 10.0,
                kd: 1.0,
                ki: 0.0,
                fc: 10.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!((stored.fc - 5.0).abs() < 1e-9, "fc clamped: {}", stored.fc);
    }

    #[test]
    fn apply_gain_override_clamps_ki_to_kp_max() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 10.0,
                kd: 1.0,
                ki: 10000.0,
                fc: 1.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!(
            (stored.ki - 5000.0).abs() < 1e-9,
            "ki clamped: {}",
            stored.ki
        );
    }

    #[test]
    fn clear_gain_override_removes_entry() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 100.0,
                kd: 10.0,
                ki: 0.0,
                fc: 2.0,
            },
        );
        assert!(loop_ctrl.gain_override(joint).is_some());
        loop_ctrl.clear_gain_override(joint);
        assert!(loop_ctrl.gain_override(joint).is_none());
    }

    #[test]
    fn clear_all_overrides_removes_all() {
        let mut loop_ctrl = test_loop();
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        loop_ctrl.apply_gain_override(
            "right_shoulder_pitch",
            GainOverride {
                kp: 100.0,
                kd: 10.0,
                ki: 0.0,
                fc: 2.0,
            },
        );
        loop_ctrl.apply_gain_override(
            "right_shoulder_roll",
            GainOverride {
                kp: 200.0,
                kd: 20.0,
                ki: 0.0,
                fc: 3.0,
            },
        );
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_some());
        assert!(loop_ctrl.gain_override("right_shoulder_roll").is_some());
        loop_ctrl.clear_all_overrides();
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
        assert!(loop_ctrl.gain_override("right_shoulder_roll").is_none());
    }

    #[test]
    fn no_override_config_gains_unchanged() {
        let loop_ctrl = test_loop();
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
        let cfg = loop_ctrl
            .supervisor
            .control
            .control
            .joints
            .get("right_shoulder_pitch")
            .expect("joint cfg exists");
        // Config gains exist and are non-zero (exact values depend on repo config).
        assert!(
            cfg.impedance.kp > 0.0,
            "kp must be > 0: {}",
            cfg.impedance.kp
        );
        assert!(
            cfg.impedance.kd > 0.0,
            "kd must be > 0: {}",
            cfg.impedance.kd
        );
    }

    #[test]
    fn apply_gain_override_stores_within_limits_as_is() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 50.0,
                kd: 5.0,
                ki: 0.5,
                fc: 1.5,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!((stored.kp - 50.0).abs() < 1e-9);
        assert!((stored.kd - 5.0).abs() < 1e-9);
        assert!((stored.ki - 0.5).abs() < 1e-9);
        assert!((stored.fc - 1.5).abs() < 1e-9);
    }
}
