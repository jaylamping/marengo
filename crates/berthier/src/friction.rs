//! Joint friction model (OpenArm teleop style).

const POSITION_HOLD_LEAD_DEADBAND_RAD: f64 = 1e-4;

/// `tau_f = Fc*tanh(k*dq) + Fv*dq + Fo`
pub fn friction_torque(dq: f64, fc: f64, fv: f64, fo: f64, k: f64) -> f64 {
    fc * (k * dq).tanh() + fv * dq + fo
}

/// Position hold uses the commanded lead for Coulomb direction so breakaway
/// torque does not flip with noisy near-zero velocity feedback.
pub fn position_hold_friction_torque(dq: f64, lead: f64, fc: f64, fv: f64, fo: f64, k: f64) -> f64 {
    let coulomb = if lead.abs() > POSITION_HOLD_LEAD_DEADBAND_RAD {
        fc * lead.signum()
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
    fn position_hold_uses_lead_direction_for_breakaway() {
        let positive = position_hold_friction_torque(-0.2, 0.01, 0.25, 0.0, 0.0, 10.0);
        let negative = position_hold_friction_torque(0.2, -0.01, 0.25, 0.0, 0.0, 10.0);

        assert!((positive - 0.25).abs() < 1e-6);
        assert!((negative + 0.25).abs() < 1e-6);
    }

    #[test]
    fn position_hold_falls_back_to_velocity_at_target() {
        let tau = position_hold_friction_torque(0.1, 0.0, 1.0, 0.0, 0.0, 10.0);
        assert!((tau - friction_torque(0.1, 1.0, 0.0, 0.0, 10.0)).abs() < 1e-6);
    }
}
