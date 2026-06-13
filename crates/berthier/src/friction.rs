//! Joint friction model (OpenArm teleop style).

use marengo_config::FrictionGains;

pub const POSITION_HOLD_ERROR_DEADBAND_RAD: f64 = 1e-4;
pub const POSITION_HOLD_FRICTION_FADE_RAD: f64 = 0.02;
/// Post-retarget window for onset MIT velocity and full-Coulomb breakaway pulse.
pub const POSITION_HOLD_ONSET_MS: u64 = 300;
/// Hysteresis band when scaling Coulomb assist at stuck onset.
pub const POSITION_STUCK_EXIT_VELOCITY_RATIO: f64 = 1.25;

/// Which position-hold friction recipe produced `tau_f` (bench diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionFrictionMode {
    /// Commanded trajectory velocity drives Coulomb direction.
    TrajectoryVelocity,
    /// Near target or planner latched — faded settle assist.
    SettleFade,
}

impl PositionFrictionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrajectoryVelocity => "traj_vel",
            Self::SettleFade => "settle",
        }
    }
}

/// Two-rule position-hold friction (ADR 0007): trajectory velocity or settle fade.
pub fn position_hold_friction(
    dq: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
    fade_rad: f64,
    retarget_age_ms: u64,
    gains: &FrictionGains,
) -> (PositionFrictionMode, f64) {
    if dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD && dq_traj * settle_error > 0.0 {
        // Ramp Coulomb with planner speed while the joint is stuck — not with settle_error
        // (full move distance would apply fc immediately and cause an approach hitch).
        // During the post-retarget onset window, apply full fc while stuck so breakaway
        // torque is not gated on dq_traj crossing the velocity deadband.
        let stuck = dq.abs() <= velocity_deadband * POSITION_STUCK_EXIT_VELOCITY_RATIO;
        let in_onset = retarget_age_ms <= POSITION_HOLD_ONSET_MS;
        let scale = if !stuck || in_onset {
            1.0
        } else {
            (dq_traj.abs() / velocity_deadband).clamp(0.0, 1.0)
        };
        let tau = gains.fc * dq_traj.signum() * scale + gains.fv * dq + gains.fo;
        return (PositionFrictionMode::TrajectoryVelocity, tau);
    }

    let fade = if settle_error.abs() > POSITION_HOLD_FRICTION_FADE_RAD {
        POSITION_HOLD_FRICTION_FADE_RAD
    } else {
        fade_rad.max(POSITION_HOLD_FRICTION_FADE_RAD)
    };
    let tau = position_settle_friction_torque(
        dq,
        settle_error,
        fade,
        gains.fc,
        gains.fv,
        gains.fo,
        gains.k,
    );
    (PositionFrictionMode::SettleFade, tau)
}

/// `tau_f = Fc*tanh(k*dq) + Fv*dq + Fo`
pub fn friction_torque(dq: f64, fc: f64, fv: f64, fo: f64, k: f64) -> f64 {
    fc * (k * dq).tanh() + fv * dq + fo
}

/// Settle-phase Coulomb assist after the planner reaches the latched target.
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
    gains: &FrictionGains,
) -> f64 {
    if dq_des.abs() > velocity_deadband {
        if dq_des * settle_error > 0.0 {
            gains.fc * dq_des.signum() + gains.fv * dq + gains.fo
        } else {
            position_settle_friction_torque(
                dq,
                settle_error,
                POSITION_HOLD_FRICTION_FADE_RAD,
                gains.fc,
                gains.fv,
                gains.fo,
                gains.k,
            )
        }
    } else if dq_des * settle_error > 0.0 {
        let scale = (dq_des.abs() / velocity_deadband).clamp(0.0, 1.0);
        gains.fc * dq_des.signum() * scale + gains.fv * dq + gains.fo
    } else {
        position_settle_friction_torque(
            dq,
            settle_error,
            POSITION_HOLD_FRICTION_FADE_RAD,
            gains.fc,
            gains.fv,
            gains.fo,
            gains.k,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_gains() -> FrictionGains {
        FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        }
    }

    #[test]
    fn zero_velocity_only_offset() {
        assert!((friction_torque(0.0, 1.0, 0.1, 0.5, 10.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn trajectory_velocity_full_fc_when_moving() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(0.08, 0.10, 0.50, 0.02, 0.10, 500, &gains);
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.25).abs() < 1e-6);
    }

    #[test]
    fn stuck_onset_ramps_coulomb_with_dq_traj_not_settle_error() {
        let gains = FrictionGains {
            fc: 0.35,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction(0.0, 0.018, 0.098, 0.02, 0.10, 500, &gains);
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.315).abs() < 1e-6);
    }

    #[test]
    fn breakaway_pulse_applies_full_fc_during_onset_while_stuck() {
        let gains = FrictionGains {
            fc: 0.35,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction(0.0, 0.00475, 0.98, 0.02, 0.10, 0, &gains);
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.35).abs() < 1e-6);
    }

    #[test]
    fn stuck_onset_reaches_full_fc_at_velocity_deadband() {
        let gains = FrictionGains {
            fc: 0.5,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction(0.0, 0.03, 0.08, 0.02, 0.10, 500, &gains);
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.5).abs() < 1e-6);
    }

    #[test]
    fn settle_fade_when_planner_done() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(0.0, 0.0, 0.02, 0.02, 0.10, 500, &gains);
        assert_eq!(mode, PositionFrictionMode::SettleFade);
        assert!((tau - 0.05).abs() < 1e-6);
    }

    #[test]
    fn cross_target_uses_settle_fade() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(0.08, 0.10, -0.003, 0.02, 0.10, 500, &gains);
        assert_eq!(mode, PositionFrictionMode::SettleFade);
        assert!(tau.abs() < 0.05);
    }

    #[test]
    fn settle_friction_scales_with_settle_error() {
        let tau = position_settle_friction_torque(0.0, 0.02, 0.05, 1.8, 0.0, 0.0, 10.0);
        assert!((tau - 0.72).abs() < 1e-6);
    }

    #[test]
    fn trajectory_friction_ramps_below_velocity_deadband() {
        let gains = test_gains();
        let tau = trajectory_friction_torque(0.0, 0.01, 0.08, 0.02, &gains);
        assert!((tau - 0.125).abs() < 1e-6);
    }

    #[test]
    fn trajectory_friction_fades_when_crossed_target() {
        let gains = test_gains();
        let toward = trajectory_friction_torque(0.08, 0.10, 0.05, 0.02, &gains);
        let crossed = trajectory_friction_torque(0.08, 0.10, -0.0028, 0.02, &gains);
        assert!((toward - 0.25).abs() < 1e-6);
        assert!((crossed + 0.035).abs() < 1e-6);
    }

    #[test]
    fn settle_uses_short_fade_when_far_from_target() {
        let tau = position_settle_friction_torque(0.0, -0.0226, 0.02, 0.35, 0.0, 0.0, 10.0);
        assert!((tau + 0.35).abs() < 1e-6);
    }
}
