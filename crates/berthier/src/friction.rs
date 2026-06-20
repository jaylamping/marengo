//! Joint friction model (OpenArm teleop style).

use marengo_config::FrictionGains;

use crate::position_trajectory::TrapezoidPhase;

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

/// Measured velocity ahead of commanded trajectory (same direction).
fn trajectory_overspeed_rad_s(dq: f64, dq_traj: f64) -> f64 {
    if dq_traj > POSITION_HOLD_ERROR_DEADBAND_RAD {
        (dq - dq_traj).max(0.0)
    } else if dq_traj < -POSITION_HOLD_ERROR_DEADBAND_RAD {
        (dq_traj - dq).max(0.0)
    } else {
        0.0
    }
}

fn trajectory_overspeed_fade(overspeed: f64, velocity_deadband: f64) -> f64 {
    let deadband = velocity_deadband.max(POSITION_HOLD_ERROR_DEADBAND_RAD);
    if overspeed <= deadband {
        1.0
    } else {
        (1.0 - (overspeed - deadband) / deadband).clamp(0.0, 1.0)
    }
}

/// Two-rule position-hold friction (ADR 0007): trajectory velocity or settle fade.
#[allow(clippy::too_many_arguments)]
pub fn position_hold_friction(
    dq: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
    fade_rad: f64,
    retarget_age_ms: u64,
    traj_phase: TrapezoidPhase,
    gains: &FrictionGains,
) -> (PositionFrictionMode, f64) {
    let toward_target = dq_traj * settle_error > POSITION_HOLD_ERROR_DEADBAND_RAD;
    if dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD {
        if toward_target {
            return trajectory_velocity_friction(
                dq,
                dq_traj,
                settle_error,
                velocity_deadband,
                retarget_age_ms,
                gains,
            );
        }
        if matches!(traj_phase, TrapezoidPhase::Hold)
            || settle_error.abs() >= POSITION_HOLD_FRICTION_FADE_RAD
        {
            // Overshoot at rest or far past target — settle assist.
        } else {
            // Tiny target cross while planner still cruising: taper Coulomb, do not flip sign.
            return cross_target_trajectory_friction(
                dq,
                dq_traj,
                settle_error,
                velocity_deadband,
                fade_rad,
                retarget_age_ms,
                gains,
            );
        }
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

fn trajectory_velocity_friction(
    dq: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
    retarget_age_ms: u64,
    gains: &FrictionGains,
) -> (PositionFrictionMode, f64) {
    let stuck = dq.abs() <= velocity_deadband * POSITION_STUCK_EXIT_VELOCITY_RATIO;
    let in_onset = retarget_age_ms <= POSITION_HOLD_ONSET_MS;
    let gravity_descent = settle_error < -POSITION_HOLD_FRICTION_FADE_RAD
        && dq_traj < -POSITION_HOLD_ERROR_DEADBAND_RAD;
    let opposing_motion = dq.abs() > velocity_deadband * 0.5
        && dq.signum() != dq_traj.signum()
        && dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD;
    let mut scale = if !stuck || (in_onset && opposing_motion) || (in_onset && !gravity_descent) {
        1.0
    } else if in_onset && gravity_descent {
        let vel_scale = (dq_traj.abs() / velocity_deadband).clamp(0.0, 1.0);
        let time_ramp = (retarget_age_ms as f64 / POSITION_HOLD_ONSET_MS as f64).clamp(0.25, 1.0);
        vel_scale * time_ramp
    } else {
        (dq_traj.abs() / velocity_deadband).clamp(0.0, 1.0)
    };
    scale *= trajectory_overspeed_fade(trajectory_overspeed_rad_s(dq, dq_traj), velocity_deadband);
    let tau = gains.fc * dq_traj.signum() * scale + gains.fv * dq + gains.fo;
    (PositionFrictionMode::TrajectoryVelocity, tau)
}

fn cross_target_trajectory_friction(
    dq: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
    fade_rad: f64,
    retarget_age_ms: u64,
    gains: &FrictionGains,
) -> (PositionFrictionMode, f64) {
    let fade = POSITION_HOLD_FRICTION_FADE_RAD.max(fade_rad);
    let taper = (settle_error.abs() / fade).clamp(0.0, 1.0);
    let stuck = dq.abs() <= velocity_deadband * POSITION_STUCK_EXIT_VELOCITY_RATIO;
    let in_onset = retarget_age_ms <= POSITION_HOLD_ONSET_MS;
    let mut scale = taper;
    if stuck && !in_onset {
        scale *= (dq_traj.abs() / velocity_deadband).clamp(0.0, 1.0);
    }
    scale *= trajectory_overspeed_fade(trajectory_overspeed_rad_s(dq, dq_traj), velocity_deadband);
    let tau = gains.fc * dq_traj.signum() * scale + gains.fv * dq + gains.fo;
    (PositionFrictionMode::TrajectoryVelocity, tau)
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
#[allow(dead_code)] // unit tests in this crate
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
    fn gravity_return_onset_friction_ramps_not_full_breakaway() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(
            0.0,
            -0.10,
            -0.08,
            0.02,
            0.10,
            0,
            TrapezoidPhase::Accelerate,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!(tau.abs() > 0.0);
        assert!(tau.abs() < gains.fc - 1e-6);
    }

    #[test]
    fn zero_velocity_only_offset() {
        assert!((friction_torque(0.0, 1.0, 0.1, 0.5, 10.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn trajectory_velocity_full_fc_when_moving() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(
            0.08,
            0.10,
            0.50,
            0.02,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.25).abs() < 1e-6);
    }

    #[test]
    fn trajectory_friction_zeros_when_measured_outruns_planner() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(
            1.50,
            0.02,
            0.50,
            0.02,
            0.10,
            50,
            TrapezoidPhase::Cruise,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!(tau.abs() < 1e-6);
    }

    #[test]
    fn trajectory_friction_fades_overspeed_across_deadband_span() {
        let gains = test_gains();
        let velocity_deadband = 0.02;
        let (_, full) = position_hold_friction(
            0.12,
            0.10,
            0.50,
            velocity_deadband,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );
        let (_, half) = position_hold_friction(
            0.13,
            0.10,
            0.50,
            velocity_deadband,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );
        let (_, zero) = position_hold_friction(
            0.14,
            0.10,
            0.50,
            velocity_deadband,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );

        assert!((full - 0.25).abs() < 1e-6);
        assert!((half - 0.125).abs() < 1e-6);
        assert!(zero.abs() < 1e-6);
    }

    #[test]
    fn trajectory_friction_overspeed_fade_is_monotonic() {
        let gains = test_gains();
        let velocity_deadband = 0.02;
        let mut previous = f64::INFINITY;
        for overspeed_steps in 0..=8 {
            let overspeed = f64::from(overspeed_steps) * 0.005;
            let dq = 0.10 + overspeed;
            let (_, tau) = position_hold_friction(
                dq,
                0.10,
                0.50,
                velocity_deadband,
                0.10,
                500,
                TrapezoidPhase::Cruise,
                &gains,
            );
            assert!(tau <= previous + 1e-9, "tau={tau} previous={previous}");
            previous = tau;
        }
    }

    #[test]
    fn stuck_onset_ramps_coulomb_with_dq_traj_not_settle_error() {
        let gains = FrictionGains {
            fc: 0.35,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction(
            0.0,
            0.018,
            0.098,
            0.02,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );
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
        let (mode, tau) = position_hold_friction(
            0.0,
            0.00475,
            0.98,
            0.02,
            0.10,
            0,
            TrapezoidPhase::Cruise,
            &gains,
        );
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
        let (mode, tau) = position_hold_friction(
            0.0,
            0.03,
            0.08,
            0.02,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.5).abs() < 1e-6);
    }

    #[test]
    fn settle_fade_when_planner_done() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(
            0.0,
            0.0,
            0.02,
            0.02,
            0.10,
            500,
            TrapezoidPhase::Hold,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::SettleFade);
        assert!((tau - 0.05).abs() < 1e-6);
    }

    #[test]
    fn cross_target_tapers_coulomb_while_planner_cruises() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(
            0.08,
            0.10,
            -0.003,
            0.02,
            0.10,
            500,
            TrapezoidPhase::Cruise,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!(tau > 0.0 && tau < 0.25);
    }

    #[test]
    fn cross_target_uses_settle_fade_at_hold() {
        let gains = test_gains();
        let (mode, tau) = position_hold_friction(
            0.08,
            0.10,
            -0.003,
            0.02,
            0.10,
            500,
            TrapezoidPhase::Hold,
            &gains,
        );
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
