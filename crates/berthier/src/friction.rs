//! Joint friction model (OpenArm teleop style).

const POSITION_HOLD_ERROR_DEADBAND_RAD: f64 = 1e-4;
const POSITION_HOLD_FRICTION_FADE_RAD: f64 = 0.005;

/// `tau_f = Fc*tanh(k*dq) + Fv*dq + Fo`
pub fn friction_torque(dq: f64, fc: f64, fv: f64, fo: f64, k: f64) -> f64 {
    fc * (k * dq).tanh() + fv * dq + fo
}

/// Position hold uses the final target error for Coulomb direction so breakaway
/// torque does not flip when the joint moves ahead of the ramped command.
pub fn position_hold_friction_torque(
    dq: f64,
    position_error: f64,
    fc: f64,
    fv: f64,
    fo: f64,
    k: f64,
) -> f64 {
    let coulomb = if position_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD {
        fc * (position_error / POSITION_HOLD_FRICTION_FADE_RAD).clamp(-1.0, 1.0)
    } else {
        fc * (k * dq).tanh()
    };
    coulomb + fv * dq + fo
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
}
