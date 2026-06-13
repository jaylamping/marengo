//! Periodic control loop (OpenArm-style refresh → compute → MIT send).

use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use armee_proto::{ControlMode as ProtoControlMode, JointState, RobotState};
use chappe::Bus;
use davout::{
    ControlMode, DavoutError, MitJointCommand as DavoutMit, MotorAddress, MotorBus,
    OperationalMode, Supervisor,
};
use marengo_config::{load_robot_config, resolve_urdf_path};
use thiserror::Error;
use tracing::{debug, info};

use crate::friction::{
    friction_torque, position_hold_friction, PositionFrictionMode, POSITION_HOLD_ERROR_DEADBAND_RAD,
    POSITION_STUCK_EXIT_VELOCITY_RATIO,
};
use crate::position_trace::{PositionTrace, PositionTraceRow};
use crate::position_trajectory::{
    filter_dq_ema, position_hold_damping_torque, JointPositionPlanner, TrapezoidPhase,
    POSITION_DAMPING_DQ_FILTER_ALPHA,
};

const POSITION_SETTLE_TOLERANCE_RAD: f64 = 1e-4;
const POSITION_SMALL_MOVE_VMAX_RAD: f64 = 0.06;
/// Descent retarget from above this delta seeds planner speed so FF beats gravity at high q.
const POSITION_RETURN_DESCENT_SEED_RAD: f64 = 0.05;
/// Resync planner only when arm is far from latched target (not small hold overshoot).
const POSITION_RETURN_RESYNC_RAD: f64 = 0.03;
/// MIT pull-down lead while stuck on descent (until breakaway latch clears).
const POSITION_DESCENT_STUCK_LEAD_RAD: f64 = 0.03;

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
}

/// Realtime control loop facade.
pub struct ControlLoop<B: MotorBus> {
    supervisor: Supervisor<B>,
    dynamics: UrdfGravityModel,
    joint_names: Vec<String>,
    control_mode: ControlMode,
    /// Final joint-space targets for [`ControlMode::Position`] (hold-on / hold-at).
    position_setpoints: Option<Vec<f64>>,
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
        })
    }

    /// Capture current joint positions as hold targets and commands (no ramp).
    pub fn latch_position_setpoints(&mut self) {
        let q = self.refresh_joint_positions();
        self.position_setpoints = Some(q.clone());
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
        let target = self.hold_target_for_joint(joint, position_rad);
        let Some(setpoints) = self.position_setpoints.as_mut() else {
            return Err(LoopError::MissingSetpoint {
                joint: joint.to_string(),
            });
        };
        let old_target = setpoints[i];
        setpoints[i] = target;
        let q_now = self.refresh_joint_positions();
        if self.position_planners.is_none() {
            self.init_position_planners(&q_now);
        }
        if let Some(planners) = self.position_planners.as_mut() {
            planners[i].reset_target(q_now[i], target);
            if (old_target - target).abs() > 1e-6 {
                if let Some(cfg) = self.supervisor.control.control.joints.get(joint) {
                    let move_dist = (target - q_now[i]).abs();
                    let v_max = if move_dist <= POSITION_SMALL_MOVE_VMAX_RAD {
                        cfg.position_slew_rad_s
                    } else {
                        cfg.position_trajectory_velocity_rad_s
                    };
                    planners[i].seed_downward_return_if_needed(
                        q_now[i],
                        target,
                        POSITION_RETURN_DESCENT_SEED_RAD,
                        cfg.position_slew_rad_s.min(v_max),
                    );
                }
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
        let filtered = self.position_dq_filtered.as_mut().expect("init above");
        let prev = filtered[joint_index];
        let next = filter_dq_ema(prev, dq_raw, POSITION_DAMPING_DQ_FILTER_ALPHA);
        filtered[joint_index] = next;
        next
    }

    fn hold_target_for_joint(&self, joint: &str, position_rad: f64) -> f64 {
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

    pub fn clear_position_hold(&mut self) {
        self.position_setpoints = None;
        self.position_planners = None;
        self.position_retarget_tick = None;
        self.position_dq_filtered = None;
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
        if mode != ControlMode::Position {
            self.position_setpoints = None;
            self.position_planners = None;
            self.position_retarget_tick = None;
            self.position_dq_filtered = None;
            self.position_planner_frozen = None;
            self.position_descent_breakaway = None;
            self.position_descent_was_stuck = None;
            self.last_position_diag = None;
        }
        self.control_mode = mode;
        self.supervisor.set_control_mode(mode);
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

    /// One control cycle: recv → compute → send → optional Chappe publish.
    pub fn tick(&mut self, chappe: Option<&Bus>) -> Result<(), LoopError> {
        self.supervisor.refresh_feedback()?;

        let q = self.read_positions();

        if self.supervisor.mode() == OperationalMode::Active {
            if self.control_mode != ControlMode::Disabled {
                let tau_g = self.dynamics.gravity_torques(&q)?;

                if self.control_mode == ControlMode::Position {
                    self.advance_position_commands(&q)?;
                }

                let log_position_diag = self.control_mode == ControlMode::Position
                    && self
                        .last_position_diag
                        .map(|t| t.elapsed() >= Duration::from_secs(1))
                        .unwrap_or(true);
                let mut batch = Vec::new();
                for i in 0..self.joint_names.len() {
                    let name = self.joint_names[i].clone();
                    let (kp, kd, tau_ff, q_des, mit_velocity) = match self.control_mode {
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
                            let mut breakaway = self
                                .position_descent_breakaway
                                .as_ref()
                                .and_then(|l| l.get(i).copied())
                                .unwrap_or(false);
                            let mut q_des =
                                clamp_trajectory_setpoint(q_traj, q[i], target, max_lead);
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
                            let joint_stuck =
                                stuck_now && !breakaway;
                            if joint_stuck {
                                q_des = q_des.min(q[i] - POSITION_DESCENT_STUCK_LEAD_RAD);
                            }
                            let planner_frozen = self
                                .position_planner_frozen
                                .as_ref()
                                .and_then(|f| f.get(i).copied())
                                .unwrap_or(false);
                            let lead = q_des - q[i];
                            let settle_error = target - q[i];
                            let settling = matches!(traj_phase, TrapezoidPhase::Hold)
                                && settle_error.abs() <= POSITION_SETTLE_TOLERANCE_RAD;
                            let approaching_target =
                                dq_traj * settle_error > POSITION_HOLD_ERROR_DEADBAND_RAD;
                            let (friction_mode, tau_f) = friction
                                .map(|f| {
                                    position_hold_friction(
                                        dq_raw,
                                        dq_traj,
                                        settle_error,
                                        vel_deadband,
                                        max_lead,
                                        &f,
                                    )
                                })
                                .unwrap_or((PositionFrictionMode::SettleFade, 0.0));
                            let tau_d = if dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD {
                                position_hold_damping_torque(
                                    dq,
                                    dq_traj,
                                    kd,
                                    vel_deadband,
                                    approaching_target,
                                )
                            } else if settling && dq.abs() > vel_deadband {
                                -kd * dq
                            } else {
                                0.0
                            };
                            let tau_ff_cmd = tau_g[i] + tau_f + tau_d;
                            let tau_meas = self.joint_torque(&name);
                            let lead_sat = lead.abs() >= max_lead - 1e-6;
                            let tau_p = kp * lead;
                            let mit_velocity = if dq_raw.abs() < vel_deadband {
                                0.0
                            } else {
                                dq_traj
                            };
                            let phase_str = format!("{traj_phase:?}");
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
                                    kd_mit = 0.0,
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
                                let t_ms =
                                    self.tick_count.saturating_mul(1000) / u64::from(self.loop_hz);
                                let row = PositionTraceRow {
                                    joint: &name,
                                    q: q[i],
                                    dq: dq_raw,
                                    q_traj,
                                    dq_traj,
                                    q_des,
                                    target,
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
                                };
                                let _ = trace.maybe_record(self.tick_count, t_ms, &row);
                                if self.tick_count % u64::from(self.loop_hz) == 0 {
                                    let _ = trace.flush();
                                }
                            }
                            (kp, 0.0, tau_ff_cmd, q_des, mit_velocity)
                        }
                        ControlMode::Disabled => continue,
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

                self.supervisor.send_mit_batch(batch)?;
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
                self.supervisor.send_mit_batch(batch)?;
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
            }
        }

        self.tick_count += 1;
        debug!(tick = self.tick_count, ?self.control_mode, "control tick");
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
        let Some(planners) = self.position_planners.as_mut() else {
            return Ok(());
        };
        let dt = self.loop_period.as_secs_f64();
        for (i, name) in self.joint_names.iter().enumerate() {
            let cfg = self.supervisor.control.control.joints.get(name);
            let slew_rad_s = cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25);
            let trajectory_v_max = cfg
                .map(|c| c.position_trajectory_velocity_rad_s)
                .unwrap_or(0.30);
            let a_max = cfg
                .map(|c| c.position_trajectory_accel_rad_s2)
                .unwrap_or(0.20);
            let max_lead = cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.10);
            let vel_deadband = cfg
                .map(|c| c.position_trajectory_velocity_deadband_rad)
                .unwrap_or(POSITION_HOLD_ERROR_DEADBAND_RAD);
            let move_dist = (targets[i] - q[i]).abs();
            let v_max = if move_dist <= POSITION_SMALL_MOVE_VMAX_RAD {
                slew_rad_s
            } else {
                trajectory_v_max
            };
            if planner_drifted_from_measurement(&planners[i], q[i], targets[i], max_lead) {
                planners[i].reset_target(q[i], targets[i]);
                if let Some(cfg) = self.supervisor.control.control.joints.get(name) {
                    planners[i].seed_downward_return_if_needed(
                        q[i],
                        targets[i],
                        POSITION_RETURN_DESCENT_SEED_RAD,
                        cfg.position_slew_rad_s.min(v_max),
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
                to_target,
                lag,
                planners[i].dq_traj,
                dq_filtered,
                vel_deadband,
                max_lead,
            );
            if let Some(frozen) = self.position_planner_frozen.as_mut() {
                frozen[i] = freeze;
            }
            if !freeze {
                planners[i].tick(targets[i], dt, v_max, a_max);
            }
        }
        Ok(())
    }

    /// Poll bus feedback, then read cached joint positions for planner init.
    fn refresh_joint_positions(&mut self) -> Vec<f64> {
        let _ = self.supervisor.refresh_feedback();
        self.read_positions()
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
        Ok(self.dynamics.gravity_torques(q)?)
    }
}

fn descent_breakaway_confirmed(to_target: f64, dq_filtered: f64, velocity_deadband: f64) -> bool {
    to_target < -POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_filtered <= -velocity_deadband * POSITION_STUCK_EXIT_VELOCITY_RATIO
}

/// MIT pull-down while descending and stuck (cleared by [`descent_breakaway_confirmed`]).
fn descent_stuck_mit_pull(
    to_target: f64,
    q: f64,
    target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
    breakaway_confirmed: bool,
) -> bool {
    !breakaway_confirmed
        && to_target < -POSITION_HOLD_ERROR_DEADBAND_RAD
        && (q - target) > POSITION_RETURN_DESCENT_SEED_RAD
        && dq_filtered.abs() < velocity_deadband
}

/// Freeze planner while arm lags on descent; hysteresis exit on filtered downward motion.
fn planner_should_freeze_on_descent(
    was_frozen: bool,
    to_target: f64,
    lag: f64,
    dq_traj: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
    max_lead: f64,
) -> bool {
    let lagging = to_target < -POSITION_HOLD_ERROR_DEADBAND_RAD
        && lag > POSITION_HOLD_ERROR_DEADBAND_RAD
        && lag < max_lead
        && dq_traj < -POSITION_HOLD_ERROR_DEADBAND_RAD;
    if !lagging {
        return false;
    }
    let exit_v = velocity_deadband * POSITION_STUCK_EXIT_VELOCITY_RATIO;
    if was_frozen {
        dq_filtered > -exit_v
    } else {
        dq_filtered.abs() < velocity_deadband
    }
}

/// Trajectory setpoint clamp: brake when `q` outruns `q_traj`, but follow `q_traj` when
/// lagging toward `target` so weighted descent can overcome gravity feedforward.
///
/// When the arm is only slightly ahead of the planner while still approaching `target`,
/// do not command `q_des` behind measured `q` — MIT stiffness pulls back and causes
/// mid-travel stick-slip stalls on the weighted bench.
fn clamp_trajectory_setpoint(q_traj: f64, q: f64, target: f64, max_lead: f64) -> f64 {
    const TOL: f64 = 1e-4;
    let to_target = target - q;
    let lag = q_traj - q;
    let mut q_des = if to_target.abs() > TOL
        && lag.signum() == to_target.signum()
        && lag.abs() > max_lead
    {
        if to_target > 0.0 {
            q_traj.clamp(q, target)
        } else {
            q_traj.clamp(target, q)
        }
    } else {
        q_traj.clamp(q - max_lead, q + max_lead)
    };

    if to_target.abs() > TOL {
        if to_target > 0.0 && q > q_traj && (q - q_traj) < max_lead {
            q_des = q_des.max(q);
        } else if to_target < 0.0 && q < q_traj && (q_traj - q) < max_lead {
            q_des = q_des.min(q);
        }
    }

    // Overshoot: command at least `target` but never past measured `q` (avoids MIT pull-back mid-travel).
    let settle_error = target - q;
    if settle_error < -TOL && q_traj <= target + TOL {
        q_des = q_des.max(target).min(q);
    } else if settle_error > TOL && q_traj >= target - TOL {
        q_des = q_des.min(target).max(q);
    }
    q_des
}

fn planner_drifted_from_measurement(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    max_lead: f64,
) -> bool {
    if (q - planner.q_traj).abs() > max_lead && (target - q).abs() > POSITION_SETTLE_TOLERANCE_RAD {
        return true;
    }
    // Planner latched at target while arm still far (return incomplete — not hold overshoot).
    planner.phase() == TrapezoidPhase::Hold
        && (q - target).abs() > POSITION_RETURN_RESYNC_RAD
        && (q - planner.q_traj).abs() > POSITION_SETTLE_TOLERANCE_RAD
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
    #![allow(clippy::expect_used)]

    use super::*;
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
        let q_des = clamp_trajectory_setpoint(0.11, 1.10, 1.57, 0.03);
        assert!((q_des - 1.07).abs() < 1e-12);
    }

    #[test]
    fn clamp_does_not_pull_back_when_slightly_ahead_approaching() {
        let q_des = clamp_trajectory_setpoint(0.035, 0.058, 0.1, 0.10);
        assert!((q_des - 0.058).abs() < 1e-12);
    }

    #[test]
    fn clamp_caps_ahead_setpoint_at_target_on_approach() {
        let q_des = clamp_trajectory_setpoint(0.091, 0.105, 0.1, 0.10);
        assert!((q_des - 0.1).abs() < 1e-12);
    }

    #[test]
    fn planner_drifted_when_hold_latched_but_arm_not_settled() {
        let planner = JointPositionPlanner::new_at(0.0);
        assert!(planner_drifted_from_measurement(&planner, 0.087, 0.0, 0.10));
    }

    #[test]
    fn planner_not_drifted_on_small_hold_overshoot() {
        let planner = JointPositionPlanner::new_at(0.1);
        assert!(!planner_drifted_from_measurement(&planner, 0.112, 0.1, 0.10));
    }

    #[test]
    fn return_onset_pulls_q_des_below_q_when_stuck() {
        assert!(descent_stuck_mit_pull(-0.09, 0.105, 0.0, 0.0, 0.02, false));
        let q_des = clamp_trajectory_setpoint(0.105, 0.105, 0.0, 0.10)
            .min(0.105 - POSITION_DESCENT_STUCK_LEAD_RAD);
        assert!((q_des - 0.075).abs() < 1e-12);
    }

    #[test]
    fn descent_mit_pull_clears_after_breakaway_latch() {
        assert!(!descent_stuck_mit_pull(-0.09, 0.105, 0.0, 0.0, 0.02, true));
        assert!(descent_stuck_mit_pull(-0.09, 0.105, 0.0, 0.0, 0.02, false));
        assert!(descent_breakaway_confirmed(-0.09, -0.026, 0.02));
        // Cruise motion alone must not imply breakaway without a stuck episode.
        assert!(!descent_stuck_mit_pull(-0.09, 0.08, 0.0, -0.03, 0.02, false));
    }

    #[test]
    fn planner_freeze_hysteresis_ignores_dq_noise_while_frozen() {
        let deadband = 0.02;
        assert!(planner_should_freeze_on_descent(
            false, -0.08, 0.03, -0.10, 0.0, deadband, 0.10
        ));
        assert!(planner_should_freeze_on_descent(
            true, -0.08, 0.03, -0.10, -0.024, deadband, 0.10
        ));
        assert!(!planner_should_freeze_on_descent(
            true, -0.08, 0.03, -0.10, -0.026, deadband, 0.10
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
        let stale = JointPositionPlanner::new_for_target(0.0, 0.0);
        assert!(planner_drifted_from_measurement(&stale, 2.9, 0.0, 0.15));
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
        let q_des = clamp_trajectory_setpoint(planner.q_traj, 0.0, target, max_lead);
        assert!(
            q_des > 0.0,
            "MIT setpoint follows planner when measured q lags"
        );
        assert!(
            planner.q_traj >= q_des,
            "trajectory runs ahead of or meets clamped MIT command"
        );
    }
}
