//! Position-hold feedforward composition: gravity + friction + damping.

use marengo_config::FrictionGains;

use crate::friction::{
    position_hold_friction, PositionFrictionMode, POSITION_HOLD_ERROR_DEADBAND_RAD,
};
use crate::position_setpoint::POSITION_SETTLE_TOLERANCE_RAD;
use crate::position_trajectory::{position_hold_damping_torque, TrapezoidPhase};

/// Per-tick position-hold feedforward outputs.
#[derive(Debug, Clone, Copy)]
pub struct PositionHoldFeedforward {
    pub friction_mode: PositionFrictionMode,
    pub tau_f: f64,
    pub tau_d: f64,
    pub tau_ff_cmd: f64,
}

/// Compose `tau_g + tau_f + tau_d` for position hold.
#[allow(clippy::too_many_arguments)]
pub fn compose_position_hold_feedforward(
    tau_g: f64,
    kd: f64,
    dq_filtered: f64,
    dq_traj: f64,
    settle_error: f64,
    vel_deadband: f64,
    effective_max_lead: f64,
    retarget_age_ms: u64,
    traj_phase: TrapezoidPhase,
    friction: Option<&FrictionGains>,
    approaching_target: bool,
) -> PositionHoldFeedforward {
    let settling = matches!(traj_phase, TrapezoidPhase::Hold)
        && settle_error.abs() <= POSITION_SETTLE_TOLERANCE_RAD;
    let (friction_mode, tau_f) = friction
        .map(|f| {
            position_hold_friction(
                dq_filtered,
                dq_traj,
                settle_error,
                vel_deadband,
                effective_max_lead,
                retarget_age_ms,
                f,
            )
        })
        .unwrap_or((PositionFrictionMode::SettleFade, 0.0));
    let tau_d = if dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD {
        position_hold_damping_torque(dq_filtered, dq_traj, kd, vel_deadband, approaching_target)
    } else if settling && dq_filtered.abs() > vel_deadband {
        -kd * dq_filtered
    } else {
        0.0
    };
    PositionHoldFeedforward {
        friction_mode,
        tau_f,
        tau_d,
        tau_ff_cmd: tau_g + tau_f + tau_d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marengo_config::FrictionGains;

    #[test]
    fn compose_feedforward_sums_components() {
        let friction = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let out = compose_position_hold_feedforward(
            1.0,
            1.0,
            0.05,
            0.1,
            0.08,
            0.02,
            0.10,
            0,
            TrapezoidPhase::Cruise,
            Some(&friction),
            true,
        );
        assert!((out.tau_ff_cmd - (1.0 + out.tau_f + out.tau_d)).abs() < 1e-12);
    }
}
