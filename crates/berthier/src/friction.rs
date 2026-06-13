//! Joint friction model (OpenArm teleop style).

use marengo_config::FrictionGains;

const POSITION_HOLD_ERROR_DEADBAND_RAD: f64 = 1e-4;
const POSITION_HOLD_FRICTION_FADE_RAD: f64 = 0.02;
/// Extra Coulomb while fully ramped and joint still stuck (mid-travel stiction).
const POSITION_STUCK_BREAKAWAY_BOOST: f64 = 1.25;
/// Stay in stuck assist until |dq| exceeds deadband × this (reduces traj_vel churn).
pub const POSITION_STUCK_EXIT_VELOCITY_RATIO: f64 = 1.25;

/// Which position-hold friction recipe produced `tau_f` (bench diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionFrictionMode {
    /// `trajectory_friction`: |dq_traj| above velocity deadband → full ±fc.
    TrajectoryVelocity,
    /// `trajectory_friction`: |dq_traj| within deadband → lead/settle fade.
    TrajectoryNearHold,
    /// `position_settle_friction_torque` after planner latched at target.
    Settle,
    /// `position_slew_friction_torque` stuck breakaway ramp vs dq_traj.
    SlewRamp,
    /// `position_hold_friction_torque` on MIT lead while slewing.
    SlewLead,
    /// Faded assist toward latched target after crossing it while planner still moves.
    TargetRecover,
}

impl PositionFrictionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrajectoryVelocity => "traj_vel",
            Self::TrajectoryNearHold => "traj_hold",
            Self::Settle => "settle",
            Self::SlewRamp => "slew_ramp",
            Self::SlewLead => "slew_lead",
            Self::TargetRecover => "target_recover",
        }
    }
}

/// Planner still commands motion but joint crossed the latched target (overshoot).
pub fn planner_crossed_target(dq_traj: f64, settle_error: f64, velocity_deadband: f64) -> bool {
    dq_traj.abs() > velocity_deadband
        && settle_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_traj * settle_error <= 0.0
}

/// Joint not moving (`|dq|` low) while planner still commands motion toward `target`.
pub fn joint_stuck_in_move_direction(
    dq: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
) -> bool {
    dq.abs() <= velocity_deadband
        && dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && settle_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_traj * settle_error > 0.0
}

/// Hysteresis on [`joint_stuck_in_move_direction`] so noisy |dq| at the deadband does not
/// flip between `SlewRamp` and `TrajectoryVelocity` during stick-creep.
pub fn joint_stuck_with_hysteresis(
    dq: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
    was_stuck: bool,
) -> bool {
    if joint_stuck_in_move_direction(dq, dq_traj, settle_error, velocity_deadband) {
        return true;
    }
    was_stuck
        && dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && settle_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_traj * settle_error > 0.0
        && dq.abs() <= velocity_deadband * POSITION_STUCK_EXIT_VELOCITY_RATIO
}

/// Select position-hold friction path and torque (logged for bench onset debugging).
///
/// **Stuck breakaway first:** when measured `|dq|` is within the velocity deadband but
/// the planner still commands motion toward the latched target, always ramp Coulomb
/// assist with `dq_traj` (`SlewRamp`). That applies equally to small slew moves and
/// large trapezoid segments at motion onset (and mid-move stick-slip) without snapping
/// to full `fc` via [`PositionFrictionMode::TrajectoryVelocity`].
///
/// Once the joint is actually moving (`|dq|` above deadband), trajectory / moving-reference
/// paths resume — cruise tracking on large moves is unchanged.
pub fn position_hold_friction_assist(
    dq: f64,
    lead: f64,
    dq_traj: f64,
    settle_error: f64,
    on_trajectory: bool,
    settling: bool,
    moving_reference: bool,
    max_lead: f64,
    velocity_deadband: f64,
    stuck: bool,
    gains: &FrictionGains,
) -> (PositionFrictionMode, f64) {
    if stuck {
        return (
            PositionFrictionMode::SlewRamp,
            position_slew_friction_torque(
                dq,
                lead,
                dq_traj,
                settle_error,
                velocity_deadband,
                gains.fc,
                gains.fv,
                gains.fo,
                gains.k,
            ),
        );
    }
    if planner_crossed_target(dq_traj, settle_error, velocity_deadband) {
        return (
            PositionFrictionMode::TargetRecover,
            position_settle_friction_torque(
                dq,
                settle_error,
                max_lead,
                gains.fc,
                gains.fv,
                gains.fo,
                gains.k,
            ),
        );
    }
    if on_trajectory || moving_reference {
        if dq_traj.abs() > velocity_deadband {
            (
                PositionFrictionMode::TrajectoryVelocity,
                trajectory_friction_torque(dq, dq_traj, settle_error, velocity_deadband, gains),
            )
        } else {
            (
                PositionFrictionMode::TrajectoryNearHold,
                trajectory_friction_torque(dq, dq_traj, settle_error, velocity_deadband, gains),
            )
        }
    } else if settling {
        (
            PositionFrictionMode::Settle,
            position_settle_friction_torque(
                dq,
                settle_error,
                max_lead,
                gains.fc,
                gains.fv,
                gains.fo,
                gains.k,
            ),
        )
    } else {
        (
            PositionFrictionMode::SlewLead,
            position_hold_friction_torque(dq, lead, gains.fc, gains.fv, gains.fo, gains.k),
        )
    }
}

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

/// Slew-phase breakaway while the joint is stuck: ramp Coulomb with `dq_traj` so assist
/// does not snap on at the velocity deadband (initial hitch).
pub fn position_slew_friction_torque(
    dq: f64,
    lead: f64,
    dq_traj: f64,
    settle_error: f64,
    velocity_deadband: f64,
    fc: f64,
    fv: f64,
    fo: f64,
    k: f64,
) -> f64 {
    if dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq.abs() <= velocity_deadband
        && settle_error.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_traj * settle_error > 0.0
    {
        let ramp = (dq_traj.abs() / velocity_deadband).clamp(0.0, 1.0);
        let boost = if ramp >= 1.0 - f64::EPSILON {
            POSITION_STUCK_BREAKAWAY_BOOST
        } else {
            1.0
        };
        return fc * dq_traj.signum() * ramp * boost + fv * dq + fo;
    }
    position_hold_friction_torque(dq, lead, fc, fv, fo, k)
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
    gains: &marengo_config::FrictionGains,
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
    } else {
        position_hold_friction_torque(dq, settle_error, gains.fc, gains.fv, gains.fo, gains.k)
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

        assert!((positive - 0.125).abs() < 1e-6);
        assert!((negative + 0.125).abs() < 1e-6);
    }

    #[test]
    fn position_hold_does_not_flip_when_joint_outruns_ramp() {
        let tau = position_hold_friction_torque(-0.2, 0.04, 0.25, 0.0, 0.0, 10.0);

        assert!((tau - 0.25).abs() < 1e-6);
    }

    #[test]
    fn position_hold_fades_breakaway_assist_near_target() {
        let tau = position_hold_friction_torque(0.0, 0.002, 0.5, 0.0, 0.0, 10.0);

        assert!((tau - 0.05).abs() < 1e-6);
    }

    #[test]
    fn position_hold_falls_back_to_velocity_at_target() {
        let tau = position_hold_friction_torque(0.1, 0.0, 1.0, 0.0, 0.0, 10.0);
        assert!((tau - friction_torque(0.1, 1.0, 0.0, 0.0, 10.0)).abs() < 1e-6);
    }

    #[test]
    fn settle_friction_scales_with_settle_error_not_saturated_lead() {
        let lead_assist = position_hold_friction_torque(0.0, 0.05, 1.8, 0.0, 0.0, 10.0);
        let settle_assist = position_settle_friction_torque(0.0, 0.02, 0.05, 1.8, 0.0, 0.0, 10.0);

        assert!((lead_assist - 1.8).abs() < 1e-6);
        assert!((settle_assist - 0.72).abs() < 1e-6);
    }

    #[test]
    fn slew_friction_ramps_breakaway_while_joint_stuck() {
        let deadband = 0.02;
        let half = position_slew_friction_torque(0.0, 0.0, 0.01, 0.08, deadband, 0.5, 0.0, 0.0, 10.0);
        let full = position_slew_friction_torque(0.0, 0.0, 0.02, 0.08, deadband, 0.5, 0.0, 0.0, 10.0);

        assert!((half - 0.25).abs() < 1e-6);
        assert!((full - 0.625).abs() < 1e-6);
    }

    #[test]
    fn slew_friction_does_not_assist_past_target_crossing() {
        let tau = position_slew_friction_torque(0.0, -0.05, 0.01, -0.01, 0.02, 0.5, 0.0, 0.0, 10.0);

        assert!((tau - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn slew_friction_falls_back_when_joint_is_moving() {
        let tau = position_slew_friction_torque(0.05, 0.03, 0.01, 0.08, 0.02, 0.5, 0.0, 0.0, 10.0);

        assert!((tau - 0.5).abs() < 1e-6);
    }

    #[test]
    fn stuck_breakaway_ramps_when_moving_reference_even_on_trajectory() {
        let gains = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction_assist(
            0.0,
            0.0,
            0.01,
            0.08,
            true,
            false,
            true,
            0.10,
            0.02,
            true,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::SlewRamp);
        assert!((tau - 0.125).abs() < 1e-6);
        assert!(joint_stuck_in_move_direction(0.0, 0.01, 0.08, 0.02));
    }

    #[test]
    fn stuck_hysteresis_keeps_slew_ramp_above_deadband_noise() {
        assert!(joint_stuck_with_hysteresis(0.021, 0.10, 0.08, 0.02, true));
        assert!(!joint_stuck_with_hysteresis(0.026, 0.10, 0.08, 0.02, true));
    }

    #[test]
    fn overshoot_uses_faded_target_recover_not_opposing_lead() {
        let gains = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction_assist(
            0.08,
            -0.04,
            0.10,
            -0.0028,
            false,
            false,
            false,
            0.10,
            0.02,
            false,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TargetRecover);
        assert!((tau + 0.007).abs() < 1e-6);
        assert!(planner_crossed_target(0.10, -0.0028, 0.02));
    }

    #[test]
    fn trajectory_friction_fades_when_crossed_target() {
        let gains = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let toward = trajectory_friction_torque(0.08, 0.10, 0.05, 0.02, &gains);
        let crossed = trajectory_friction_torque(0.08, 0.10, -0.0028, 0.02, &gains);

        assert!((toward - 0.25).abs() < 1e-6);
        assert!((crossed + 0.035).abs() < 1e-6);
    }

    #[test]
    fn moving_joint_on_trajectory_uses_trajectory_velocity() {
        let gains = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction_assist(
            0.08,
            0.02,
            0.10,
            0.50,
            true,
            false,
            true,
            0.10,
            0.02,
            false,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::TrajectoryVelocity);
        assert!((tau - 0.25).abs() < 1e-6);
        assert!(!joint_stuck_in_move_direction(0.08, 0.10, 0.50, 0.02));
    }

    #[test]
    fn position_hold_friction_assist_classifies_stuck_as_slew_ramp_not_traj_vel() {
        let gains = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction_assist(
            0.0,
            0.0,
            0.10,
            0.08,
            false,
            false,
            true,
            0.10,
            0.02,
            true,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::SlewRamp);
        assert!((tau - 0.3125).abs() < 1e-6);
        assert!(joint_stuck_in_move_direction(0.0, 0.10, 0.08, 0.02));
    }

    #[test]
    fn position_hold_friction_assist_uses_slew_ramp_when_not_moving_reference() {
        let gains = FrictionGains {
            fc: 0.25,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let (mode, tau) = position_hold_friction_assist(
            0.0,
            0.0,
            0.01,
            0.08,
            false,
            false,
            false,
            0.10,
            0.02,
            true,
            &gains,
        );
        assert_eq!(mode, PositionFrictionMode::SlewRamp);
        assert!((tau - 0.125).abs() < 1e-6);
    }
}
