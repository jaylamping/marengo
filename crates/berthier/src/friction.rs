//! Joint friction model (OpenArm teleop style).

const POSITION_HOLD_ERROR_DEADBAND_RAD: f64 = 1e-4;
const POSITION_HOLD_FRICTION_FADE_RAD: f64 = 0.005;

/// `tau_f = Fc*tanh(k*dq) + Fv*dq + Fo`
pub fn friction_torque(dq: f64, fc: f64, fv: f64, fo: f64, k: f64) -> f64 {
    fc * (k * dq).tanh() + fv * dq + fo
}

/// Slew Coulomb assist uses **command tracking error** (`q_des − q`, bounded by
/// `position_slew_max_lead_rad`), not error to the final latched target. That matches
/// joint-impedance practice (KUKA/iiwa-style): feedforward assists along the ramp,
/// not toward a distant goal.
pub fn position_hold_friction_torque(
    dq: f64,
    tracking_error: f64,
    fc: f64,
    fv: f64,
    fo: f64,
    k: f64,
) -> f64 {
    let coulomb = if tracking_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD {
        fc * (tracking_error / POSITION_HOLD_FRICTION_FADE_RAD).clamp(-1.0, 1.0)
    } else {
        fc * (k * dq).tanh()
    };
    coulomb + fv * dq + fo
}

/// Settle-phase Coulomb assist after the planner reaches the latched target (`q_traj ≈
/// target`, `dq_traj ≈ 0`). Uses settle error with a wider fade so saturated MIT lead
/// does not pin breakaway assist at full `fc` during end-of-travel stick-slip.
pub fn position_settle_friction_torque(
    dq: f64,
    settle_error: f64,
    fade_rad: f64,
    fc: f64,
    fv: f64,
    fo: f64,
    k: f64,
) -> f64 {
    let fade = fade_rad.max(POSITION_HOLD_FRICTION_FADE_RAD);
    let coulomb = if settle_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD {
        fc * (settle_error / fade).clamp(-1.0, 1.0)
    } else {
        fc * (k * dq).tanh()
    };
    coulomb + fv * dq + fo
}

/// Trajectory Coulomb assist follows commanded velocity when moving; near hold uses settle error.
pub fn trajectory_friction_torque(
    dq: f64,
    dq_des: f64,
    settle_error: f64,
    velocity_deadband: f64,
    fc: f64,
    fv: f64,
    fo: f64,
    k: f64,
) -> f64 {
    if dq_des.abs() > velocity_deadband {
        fc * dq_des.signum() + fv * dq + fo
    } else {
        position_hold_friction_torque(dq, settle_error, fc, fv, fo, k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_velocity_only_offset() {
        assert!((friction_torque(0.0, 1.0, 0.1, 0.5, 10.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn position_hold_uses_target_error_direction_for_breakaway() {
        let positive = position_hold_friction_torque(-0.2, 0.01, 0.25, 0.0, 0.0, 10.0);
        let negative = position_hold_friction_torque(0.2, -0.01, 0.25, 0.0, 0.0, 10.0);

        assert!((positive - 0.25).abs() < 1e-6);
        assert!((negative + 0.25).abs() < 1e-6);
    }

    #[test]
    fn position_hold_does_not_flip_when_joint_outruns_ramp() {
        let tau = position_hold_friction_torque(-0.2, 0.04, 0.25, 0.0, 0.0, 10.0);

        assert!((tau - 0.25).abs() < 1e-6);
    }

    #[test]
    fn position_hold_fades_breakaway_assist_near_target() {
        let tau = position_hold_friction_torque(0.0, 0.002, 0.5, 0.0, 0.0, 10.0);

        assert!((tau - 0.2).abs() < 1e-6);
    }

    #[test]
    fn position_hold_falls_back_to_velocity_at_target() {
        let tau = position_hold_friction_torque(0.1, 0.0, 1.0, 0.0, 0.0, 10.0);
        assert!((tau - friction_torque(0.1, 1.0, 0.0, 0.0, 10.0)).abs() < 1e-6);
    }

    #[test]
    fn settle_friction_scales_with_settle_error_not_saturated_lead() {
        let lead_assist = position_hold_friction_torque(0.0, 0.05, 1.8, 0.0, 0.0, 10.0);
        let settle_assist =
            position_settle_friction_torque(0.0, 0.02, 0.05, 1.8, 0.0, 0.0, 10.0);

        assert!((lead_assist - 1.8).abs() < 1e-6);
        assert!((settle_assist - 0.72).abs() < 1e-6);
    }
}
