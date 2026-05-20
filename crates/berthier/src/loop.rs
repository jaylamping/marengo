//! Periodic control loop (OpenArm-style refresh → compute → MIT send).

use std::path::Path;
use std::time::{Duration, Instant};

use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use armee_proto::{ControlMode as ProtoControlMode, JointState, RobotState};
use chappe::Bus;
use davout::{
    ControlMode, DavoutError, MitJointCommand as DavoutMit, MotorBus, OperationalMode, Supervisor,
};
use marengo_config::{load_robot_config, resolve_urdf_path};
use thiserror::Error;
use tracing::debug;

use crate::friction::friction_torque;

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
}

/// Realtime control loop facade.
pub struct ControlLoop<B: MotorBus> {
    supervisor: Supervisor<B>,
    dynamics: UrdfGravityModel,
    joint_names: Vec<String>,
    control_mode: ControlMode,
    loop_period: Duration,
    chappe_publish_period: Duration,
    last_chappe: Option<Instant>,
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
            loop_period: Duration::from_secs_f64(1.0 / f64::from(loop_hz.max(1))),
            chappe_publish_period: Duration::from_secs_f64(1.0 / f64::from(chappe_hz.max(1))),
            last_chappe: None,
            tick_count: 0,
        })
    }

    pub fn supervisor_mut(&mut self) -> &mut Supervisor<B> {
        &mut self.supervisor
    }

    pub fn set_control_mode(&mut self, mode: ControlMode) {
        self.control_mode = mode;
        self.supervisor.set_control_mode(mode);
    }

    pub fn control_mode(&self) -> ControlMode {
        self.control_mode
    }

    pub fn loop_period(&self) -> Duration {
        self.loop_period
    }

    /// One control cycle: recv → compute → send → optional Chappe publish.
    pub fn tick(&mut self, chappe: Option<&Bus>) -> Result<(), LoopError> {
        let _ = self.supervisor.refresh_feedback()?;
        if self.supervisor.mode() != OperationalMode::Active {
            return Ok(());
        }
        if self.control_mode == ControlMode::Disabled {
            return Ok(());
        }

        let q = self.read_positions();
        let tau_g = self.dynamics.gravity_torques(&q)?;

        let mut batch = Vec::new();
        for (i, name) in self.joint_names.iter().enumerate() {
            let (kp, kd, tau_ff) = match self.control_mode {
                ControlMode::GravityComp | ControlMode::TorqueOnly => (0.0, 0.0, tau_g[i]),
                ControlMode::Impedance => {
                    let cfg = self
                        .supervisor
                        .control
                        .control
                        .joints
                        .get(name);
                    let imp = cfg.map(|c| &c.impedance);
                    let fr = cfg.map(|c| &c.friction);
                    let kp = imp.map(|g| g.kp).unwrap_or(0.0);
                    let kd = imp.map(|g| g.kd).unwrap_or(0.0);
                    let dq = self.joint_velocity(name);
                    let tau_f = fr
                        .map(|f| friction_torque(dq, f.fc, f.fv, f.fo, f.k))
                        .unwrap_or(0.0);
                    (kp, kd, tau_g[i] + tau_f)
                }
                ControlMode::Position => {
                    let cfg = self.supervisor.control.control.joints.get(name);
                    let kp = cfg.map(|c| c.impedance.kp).unwrap_or(20.0);
                    let kd = cfg.map(|c| c.impedance.kd).unwrap_or(1.0);
                    (kp, kd, tau_g[i])
                }
                ControlMode::Disabled => continue,
            };
            batch.push(DavoutMit {
                joint: name.clone(),
                kp,
                kd,
                position_rad: q[i],
                velocity_rad_s: 0.0,
                torque_ff_nm: tau_ff,
            });
        }

        self.supervisor.send_mit_batch(batch)?;

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

    fn read_positions(&self) -> Vec<f64> {
        self.joint_names
            .iter()
            .map(|name| {
                self.supervisor
                    .motors
                    .motors
                    .iter()
                    .find(|m| &m.joint == name)
                    .and_then(|m| self.supervisor.motor_states().get(&m.device_id))
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
            .and_then(|m| self.supervisor.motor_states().get(&m.device_id))
            .map(|s| f64::from(s.velocity_rad_s))
            .unwrap_or(0.0)
    }

    fn publish_robot_state(&self, chappe: &Bus, q: &[f64]) -> Result<(), LoopError> {
        let joints: Vec<JointState> = self
            .joint_names
            .iter()
            .zip(q.iter())
            .map(|(name, &position)| JointState {
                name: name.clone(),
                position,
                velocity: 0.0,
                effort: 0.0,
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
        chappe.publish(
            "robot/state",
            "berthier",
            "marengo.v1.RobotState",
            &state,
        )?;
        Ok(())
    }

    /// Preview gravity torques without sending (for motor-repl status).
    pub fn preview_gravity_torques(&self, q: &[f64]) -> Result<Vec<f64>, LoopError> {
        Ok(self.dynamics.gravity_torques(q)?)
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
