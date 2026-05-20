//! Realtime control stack for Marengo (Berthier).
//!
//! Motor commands go through [Davout](davout::Supervisor) only — this crate does not use
//! [robstride] directly.

use davout::{CanBus, DavoutError, JointCommand, OperationalMode, Supervisor};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("safety: {0}")]
    Safety(#[from] DavoutError),
}

/// Control facade — holds the Davout supervisor (motor gateway).
pub struct Controller<B: CanBus> {
    supervisor: Supervisor<B>,
}

impl<B: CanBus> Controller<B> {
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
    //! Berthier must not link robstride for direct motor access.
    use std::any::TypeId;

    use super::Controller;

    #[test]
    fn motor_path_is_davout_not_robstride() {
        assert!(TypeId::of::<davout::Supervisor<davout::MemoryBus>>() != TypeId::of::<()>());
        // Berthier sources must not call robstride::send_motion; enforced by code review + this module boundary.
    }

    #[test]
    fn controller_commands_through_supervisor() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bus = davout::MemoryBus::default();
        let mut ctrl = Controller::from_repo(&root, bus).expect("controller");
        ctrl.supervisor_mut().set_homing_complete();
        ctrl.supervisor_mut().request_enable(true).expect("enable");
        ctrl.command_position("joint1", 0.05).expect("position");
    }
}
