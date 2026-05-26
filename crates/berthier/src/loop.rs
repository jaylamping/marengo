//! Periodic control loop (OpenArm-style refresh → compute → MIT send).

use std::path::Path;
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

use crate::friction::{friction_torque, position_hold_friction_torque};

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
    /// Ramped MIT position commands — slews toward [`Self::position_setpoints`] each tick.
    position_commands: Option<Vec<f64>>,
    loop_period: Duration,
    chappe_publish_period: Duration,
    last_chappe: Option<Instant>,
    last_position_diag: Option<Instant>,
    tick_count: u64,
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
        let supervisor = Supervisor::from_repo(root, bus)?;
        Ok(Self {
            supervisor,
            dynamics,
            joint_names,
            control_mode: ControlMode::Disabled,
            position_setpoints: None,
            position_commands: None,
            loop_period: Duration::from_secs_f64(1.0 / f64::from(loop_hz.max(1))),
            chappe_publish_period: Duration::from_secs_f64(1.0 / f64::from(chappe_hz.max(1))),
            last_chappe: None,
            last_position_diag: None,
            tick_count: 0,
        })
    }

    /// Capture current joint positions as hold targets and commands (no ramp).
    pub fn latch_position_setpoints(&mut self) {
        let q = self.read_positions();
        self.position_setpoints = Some(q.clone());
        self.position_commands = Some(q);
    }

    pub fn position_setpoints(&self) -> Option<&[f64]> {
        self.position_setpoints.as_deref()
    }

    /// Ramped MIT position commands (for status / tests); slews toward [`Self::position_setpoints`].
    pub fn position_hold_commands(&self) -> Option<&[f64]> {
        self.position_commands.as_deref()
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
        setpoints[i] = target;
        Ok(())
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
        self.position_commands = None;
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
        let q = self.read_positions();
        if self.joint_names.len() == 1 {
            let name = self.joint_names[0].clone();
            let target = self.hold_target_for_joint(&name, position_rad);
            self.position_setpoints = Some(vec![target]);
            self.position_commands = Some(q);
        } else {
            let joint = joint.ok_or(LoopError::JointNameRequired)?;
            if self.position_setpoints.is_none() {
                self.position_setpoints = Some(q.clone());
            }
            if self.position_commands.is_none() {
                self.position_commands = Some(q);
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
            self.position_commands = None;
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

                let q_cmd = self.position_commands.as_deref();
                let log_position_diag = self.control_mode == ControlMode::Position
                    && self
                        .last_position_diag
                        .map(|t| t.elapsed() >= Duration::from_secs(1))
                        .unwrap_or(true);
                let mut batch = Vec::new();
                for (i, name) in self.joint_names.iter().enumerate() {
                    let (kp, kd, tau_ff, q_des) = match self.control_mode {
                        ControlMode::GravityComp | ControlMode::TorqueOnly => {
                            (0.0, 0.0, tau_g[i], q[i])
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
                            (kp, kd, tau_g[i] + tau_f, q[i])
                        }
                        ControlMode::Position => {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let kp = cfg.map(|c| c.impedance.kp).unwrap_or(20.0);
                            let kd = cfg.map(|c| c.impedance.kd).unwrap_or(1.0);
                            let fr = cfg.map(|c| &c.friction);
                            let q_des =
                                q_cmd.and_then(|sp| sp.get(i).copied()).ok_or_else(|| {
                                    LoopError::MissingSetpoint {
                                        joint: name.clone(),
                                    }
                                })?;
                            let dq = self.joint_velocity(name);
                            let lead = q_des - q[i];
                            let tau_f = fr
                                .map(|f| {
                                    position_hold_friction_torque(dq, lead, f.fc, f.fv, f.fo, f.k)
                                })
                                .unwrap_or(0.0);
                            if log_position_diag {
                                let target = self
                                    .position_setpoints
                                    .as_deref()
                                    .and_then(|sp| sp.get(i).copied())
                                    .unwrap_or(q_des);
                                let position_error = target - q[i];
                                let estimated_tau = tau_g[i] + tau_f + kp * lead - kd * dq;
                                info!(
                                    joint = %name,
                                    q = q[i],
                                    dq,
                                    q_des,
                                    target,
                                    lead,
                                    position_error,
                                    kp,
                                    kd,
                                    tau_g = tau_g[i],
                                    tau_f,
                                    estimated_tau,
                                    "position hold command"
                                );
                            }
                            (kp, kd, tau_g[i] + tau_f, q_des)
                        }
                        ControlMode::Disabled => continue,
                    };
                    batch.push(DavoutMit {
                        joint: name.clone(),
                        kp,
                        kd,
                        position_rad: q_des,
                        velocity_rad_s: 0.0,
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

    /// Ramp commanded hold poses toward latched targets for this tick.
    fn advance_position_commands(&mut self, q: &[f64]) -> Result<(), LoopError> {
        let Some(targets) = self.position_setpoints.as_ref() else {
            return Ok(());
        };
        if self.position_commands.is_none() {
            self.position_commands = Some(q.to_vec());
        }
        let Some(commands) = self.position_commands.as_mut() else {
            return Ok(());
        };
        let dt = self.loop_period.as_secs_f64();
        for (i, name) in self.joint_names.iter().enumerate() {
            let cfg = self.supervisor.control.control.joints.get(name);
            let slew_rad_s = cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25);
            let max_lead = cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.15);
            let max_step = slew_rad_s * dt;
            commands[i] = slew_toward(commands[i], targets[i], max_step);
            commands[i] = commands[i].clamp(q[i] - max_lead, q[i] + max_lead);
        }
        Ok(())
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

/// Move `current` toward `target` by at most `max_step` (rad).
fn slew_toward(current: f64, target: f64, max_step: f64) -> f64 {
    let err = target - current;
    if err.abs() <= max_step {
        target
    } else {
        current + max_step.copysign(err)
    }
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
    fn slew_toward_steps_and_snaps() {
        assert!((slew_toward(0.0, 1.0, 0.25) - 0.25).abs() < 1e-12);
        assert!((slew_toward(0.9, 1.0, 0.25) - 1.0).abs() < 1e-12);
        assert!((slew_toward(0.0, -1.0, 0.1) - (-0.1)).abs() < 1e-12);
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
}
