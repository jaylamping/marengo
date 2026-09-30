//! # Berthier — realtime control (outer loop)
//!
//! Berthier owns **what to command each tick**: read joint state, compute feedforward and
//! gains, assemble MIT setpoints. It does **not** talk to CAN, enforce safety limits, or
//! encode vendor frames.
//!
//! ## Responsibilities
//!
//! - [`ControlLoop::tick`](loop::ControlLoop::tick): `recv` → `q` → `tau_g(q)` → MIT batch → Davout.
//! - Control modes: gravity comp, impedance, position, torque-only ([`ControlMode`]).
//! - Position hold law lives in [`position_hold::PositionHold`] (targets, planners, MIT compose).
//! - MIT feedforward for GravityComp / Impedance / TorqueOnly: [`mit_feedforward::MitFeedforward`].
//! - TorqueOnly operator latch: [`torque_cmd::TorqueCmdLatch`] (`τ_cmd`; cleared on leave).
//! - Optional friction feedforward (`friction` module) in impedance and position modes.
//! - Publish [`RobotState`](armee_proto::RobotState) on Chappe (lower rate than the motor loop).
//! - Legacy [`Controller`]: single-joint position commands through Davout (REPL / bring-up).
//!
//! ## Does not
//!
//! - Open SocketCAN or call `robstride` (motor path is Davout → robstride only).
//! - Apply torque/position limits, E-stop, comm watchdog, or danger zones (Davout).
//! - Parse URDF for limits (uses [`armee-dynamics`] for `tau_g`, config for gains).
//!
//! ## Dependencies (allowed direction)
//!
//! ```text
//! armee-dynamics (tau_g) ──► berthier ──► davout ──► robstride
//! marengo-config (gains) ──┘              chappe (telemetry)
//! armee-proto (wire types)
//! ```
//!
//! ## Two mode enums (read with care)
//!
//! | Enum | Crate | Meaning |
//! |------|-------|---------|
//! | [`davout::OperationalMode`] | Davout | Disabled / Ready / Active — **may motors move?** |
//! | [`ControlMode`] | Berthier + Davout | GravityComp / Impedance / … — **how** to command when Active |
//!
//! See [ADR 0004](../../docs/decisions/0004-control-modes-and-mit.md).

mod friction;
mod gain_runtime;
mod r#loop;
mod mit_feedforward;
mod position_feedforward;
mod position_hold;
mod position_profile;
mod position_setpoint;
mod position_trace;
mod position_trajectory;
mod position_wave;
mod torque_cmd;

#[cfg(test)]
mod mode_isolation;

pub use davout::ControlMode;
pub use gain_runtime::{mode_allows_gain_override, GainOverride};
pub use r#loop::{proto_control_mode, ControlLoop, LoopError, TickPhaseAverages};

use davout::{DavoutError, JointCommand, OperationalMode, Supervisor};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("safety: {0}")]
    Safety(#[from] DavoutError),
}

/// Control facade — holds the Davout supervisor (motor gateway).
pub struct Controller<B: davout::MotorBus> {
    supervisor: Supervisor<B>,
}

impl<B: davout::MotorBus> Controller<B> {
    pub fn new(supervisor: Supervisor<B>) -> Self {
        Self { supervisor }
    }

    pub fn from_repo(repo_root: impl AsRef<std::path::Path>, bus: B) -> Result<Self, ControlError> {
        Ok(Self {
            supervisor: Supervisor::from_repo(repo_root, bus)?,
        })
    }

    pub fn supervisor_mut(&mut self) -> &mut Supervisor<B> {
        &mut self.supervisor
    }

    pub fn mode(&self) -> OperationalMode {
        self.supervisor.mode()
    }

    /// Track a joint position (rad). Requires ACTIVE supervisor.
    pub fn command_position(
        &mut self,
        joint: impl Into<String>,
        position_rad: f64,
    ) -> Result<(), ControlError> {
        let joint = joint.into();
        debug!(%joint, position_rad, "command_position");
        self.supervisor
            .send_joint_command(JointCommand {
                joint,
                position_rad,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            })
            .map_err(ControlError::Safety)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::Controller;

    #[test]
    fn controller_commands_through_supervisor() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bus = davout::MemoryBus::default();
        let mut ctrl = Controller::from_repo(&root, bus).expect("controller");
        let motors = ctrl.supervisor_mut().motors.motors.clone();
        ctrl.supervisor_mut()
            .homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("verify");
        ctrl.supervisor_mut().set_homing_complete().expect("ready");
        ctrl.supervisor_mut().request_enable(true).expect("enable");
        ctrl.command_position("right_shoulder_roll", 0.05)
            .expect("position");
    }
}
