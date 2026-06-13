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

use crate::friction::{friction_torque, position_hold_friction, PositionFrictionMode};
use crate::position_trace::{PositionTrace, PositionTraceRow};
use crate::position_trajectory::{trajectory_damping_torque, JointPositionPlanner, TrapezoidPhase};

const POSITION_SETTLE_TOLERANCE_RAD: f64 = 1e-4;
const POSITION_SMALL_MOVE_VMAX_RAD: f64 = 0.06;

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
        if self.position_retarget_tick.is_none() {
            self.position_retarget_tick = Some(vec![0; self.joint_names.len()]);
        }
        if let Some(ticks) = self.position_retarget_tick.as_mut() {
            ticks[joint_index] = self.tick_count;
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
            let target = self.hold_target_for_joint(&name, position_rad);
            self.position_setpoints = Some(vec![target]);
            self.init_position_planners(&q);
            info!(
                joint = %name,
                q = q[0],
                new_target = target,
                delta = target - q[0],
                tick = self.tick_count,
                "position hold retarget"
            );
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
                for (i, name) in self.joint_names.iter().enumerate() {
                    let (kp, kd, tau_ff, q_des, mit_velocity) = match self.control_mode {
                        ControlMode::GravityComp | ControlMode::TorqueOnly => {
                            (0.0, 0.0, tau_g[i], q[i], 0.0)
                        }
                        ControlMode::Impedance => {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let imp = cfg.map(|c| &c.impedance);
                            let fr = cfg.map(|c| &c.friction);
                            let kp = imp.map(|g| g.kp).unwrap_or(0.0);
                            let kd = imp.map(|g| g.kd).unwrap_or(0.0);
                            let dq = self.joint_velocity(name);
                            let tau_f = fr
                                .map(|f| friction_torque(dq, f.fc, f.fv, f.fo, f.k))
                                .unwrap_or(0.0);
                            (kp, kd, tau_g[i] + tau_f, q[i], 0.0)
                        }
                        ControlMode::Position => {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let kp = cfg.map(|c| c.impedance.kp).unwrap_or(20.0);
                            let kd = cfg.map(|c| c.impedance.kd).unwrap_or(1.0);
                            let max_lead =
                                cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.15);
                            let fr = cfg.map(|c| &c.friction);
                            let planner = self
                                .position_planners
                                .as_ref()
                                .and_then(|p| p.get(i))
                                .ok_or_else(|| LoopError::MissingSetpoint {
                                    joint: name.clone(),
                                })?;
                            let q_traj = planner.q_traj;
                            let dq_traj = planner.dq_traj;
                            let traj_phase = planner.phase();
                            let target = self
                                .position_setpoints
                                .as_deref()
                                .and_then(|sp| sp.get(i).copied())
                                .unwrap_or(q_traj);
                            let q_des = clamp_trajectory_setpoint(q_traj, q[i], target, max_lead);
                            let dq = self.joint_velocity(name);
                            let lead = q_des - q[i];
                            let settle_error = target - q[i];
                            let vel_deadband = cfg
                                .map(|c| c.position_trajectory_velocity_deadband_rad)
                                .unwrap_or(0.02);
                            let settling = matches!(traj_phase, TrapezoidPhase::Hold)
                                && settle_error.abs() <= POSITION_SETTLE_TOLERANCE_RAD;
                            let (friction_mode, tau_f) = fr
                                .map(|f| {
                                    position_hold_friction(
                                        dq,
                                        dq_traj,
                                        settle_error,
                                        vel_deadband,
                                        max_lead,
                                        f,
                                    )
                                })
                                .unwrap_or((PositionFrictionMode::SettleFade, 0.0));
                            let retarget_age_ms = self.position_retarget_age_ms(i);
                            let tau_d = if dq_traj.abs() > vel_deadband {
                                trajectory_damping_torque(dq, dq_traj, kd)
                            } else if settling && dq.abs() > vel_deadband {
                                -kd * dq
                            } else {
                                0.0
                            };
                            let tau_ff_cmd = tau_g[i] + tau_f + tau_d;
                            let tau_meas = self.joint_torque(name);
                            let lead_sat = lead.abs() >= max_lead - 1e-6;
                            let tau_p = kp * lead;
                            let phase_str = format!("{traj_phase:?}");
                            if log_position_diag {
                                info!(
                                    joint = %name,
                                    q = q[i],
                                    dq,
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
                                    dq_mit = dq_traj,
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
                                    dq,
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
                                    "position hold onset"
                                );
                            }
                            if let Some(trace) = self.position_trace.as_mut() {
                                let t_ms =
                                    self.tick_count.saturating_mul(1000) / u64::from(self.loop_hz);
                                let row = PositionTraceRow {
                                    joint: name,
                                    q: q[i],
                                    dq,
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
                                    dq_mit: dq_traj,
                                    kp,
                                    kd,
                                };
                                let _ = trace.maybe_record(self.tick_count, t_ms, &row);
                                if self.tick_count % u64::from(self.loop_hz) == 0 {
                                    let _ = trace.flush();
                                }
                            }
                            (kp, 0.0, tau_ff_cmd, q_des, dq_traj)
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
            if planner_drifted_from_measurement(&planners[i], q[i], targets[i], max_lead) {
                planners[i].reset_target(q[i], targets[i]);
            }
            let move_dist = (targets[i] - q[i]).abs();
            let v_max = if move_dist <= POSITION_SMALL_MOVE_VMAX_RAD {
                slew_rad_s
            } else {
                trajectory_v_max
            };
            planners[i].tick(targets[i], dt, v_max, a_max);
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

/// Trajectory setpoint clamp: brake when `q` outruns `q_traj`, but follow `q_traj` when
/// lagging toward `target` so weighted descent can overcome gravity feedforward.
fn clamp_trajectory_setpoint(q_traj: f64, q: f64, target: f64, max_lead: f64) -> f64 {
    const TOL: f64 = 1e-4;
    let to_target = target - q;
    let lag = q_traj - q;
    if to_target.abs() > TOL && lag.signum() == to_target.signum() && lag.abs() > max_lead {
        if to_target > 0.0 {
            q_traj.clamp(q, target)
        } else {
            q_traj.clamp(target, q)
        }
    } else {
        q_traj.clamp(q - max_lead, q + max_lead)
    }
}

fn planner_drifted_from_measurement(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    max_lead: f64,
) -> bool {
    (q - planner.q_traj).abs() > max_lead && (target - q).abs() > POSITION_SETTLE_TOLERANCE_RAD
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
    fn position_hold_trajectory_damping_tracks_dq_error() {
        let tau_d = trajectory_damping_torque(0.1, 0.2, 2.0);
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
