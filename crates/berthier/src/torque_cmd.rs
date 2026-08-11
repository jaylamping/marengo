//! TorqueCmdLatch — per-joint latched open-loop `τ_cmd` for [`ControlMode::TorqueOnly`].
//!
//! Owns storage only. [`crate::r#loop::ControlLoop`] owns mode enter/leave policy
//! (`enter_torque_only_zero`, finite/joint checks, leave-clear).

use std::collections::HashMap;

/// Per-joint latched operator torque command (Nm) for TorqueOnly.
///
/// Unset joints read as `0.0`. Cleared by [`Self::on_leave_torque_only`] when
/// leaving TorqueOnly (caller invokes from mode transition).
#[derive(Debug, Default, Clone)]
pub struct TorqueCmdLatch {
    cmds: HashMap<String, f64>,
}

impl TorqueCmdLatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Latched `τ_cmd` for a joint, or `0.0` when unset.
    pub fn get(&self, joint_name: &str) -> f64 {
        self.cmds.get(joint_name).copied().unwrap_or(0.0)
    }

    /// Latch a torque command. Caller validates joint name and finiteness.
    pub fn set(&mut self, joint_name: &str, tau_nm: f64) {
        self.cmds.insert(joint_name.to_string(), tau_nm);
    }

    /// Remove one joint's latch (reverts to 0).
    pub fn clear(&mut self, joint_name: &str) {
        self.cmds.remove(joint_name);
    }

    /// Clear all latched torque commands.
    pub fn clear_all(&mut self) {
        self.cmds.clear();
    }

    /// Mode-leave hook: drop every latch so the next TorqueOnly entry starts at 0.
    pub fn on_leave_torque_only(&mut self) {
        self.clear_all();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn defaults_zero_and_latches() {
        let mut latch = TorqueCmdLatch::new();
        assert!((latch.get("j0")).abs() < 1e-12);
        latch.set("j0", 0.25);
        assert!((latch.get("j0") - 0.25).abs() < 1e-12);
        latch.clear("j0");
        assert!((latch.get("j0")).abs() < 1e-12);
    }

    #[test]
    fn on_leave_clears_all() {
        let mut latch = TorqueCmdLatch::new();
        latch.set("a", 0.1);
        latch.set("b", 0.2);
        latch.on_leave_torque_only();
        assert!((latch.get("a")).abs() < 1e-12);
        assert!((latch.get("b")).abs() < 1e-12);
    }
}
