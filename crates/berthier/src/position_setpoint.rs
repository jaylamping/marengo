//! Position-hold setpoint mapping: planner reference → MIT `q_des`.

use armee_kinematics::{clamp_position_in_envelope, effective_command_bounds, JointLimitPolicy};

use crate::position_trajectory::JointPositionPlanner;

pub(crate) const POSITION_SETTLE_TOLERANCE_RAD: f64 = 1e-4;
/// Tighter settle band at `target == 0` — matches Layer 2 `--require-home-start`.
pub(crate) const POSITION_HOME_SETTLE_RAD: f64 = 0.005;
/// Descent retarget from above this delta seeds planner speed so FF beats gravity at high q.
pub(crate) const POSITION_RETURN_DESCENT_SEED_RAD: f64 = 0.05;
/// Resync planner only when arm is far from latched target (not small hold overshoot).
pub(crate) const POSITION_RETURN_RESYNC_RAD: f64 = 0.03;

/// Settle/resync band: home returns use [`POSITION_HOME_SETTLE_RAD`], other targets use resync.
pub fn return_settle_band(target: f64) -> f64 {
    if target.abs() <= POSITION_SETTLE_TOLERANCE_RAD {
        POSITION_HOME_SETTLE_RAD
    } else {
        POSITION_RETURN_RESYNC_RAD
    }
}
/// Return planner-freeze only below this |q| — high-angle descent needs continuous q_ref.
pub(crate) const POSITION_RETURN_FREEZE_Q_MAX_RAD: f64 = 0.12;
/// MIT pull-down lead while stuck on descent (until breakaway latch clears).
pub(crate) const POSITION_DESCENT_STUCK_LEAD_RAD: f64 = 0.03;
/// Command past home while stuck in the final band — extra P torque to break static friction.
pub(crate) const POSITION_HOME_FINAL_PULL_THROUGH_RAD: f64 = 0.02;
/// Outbound-only lead cap during post-retarget onset (weighted breakaway).
pub(crate) const POSITION_HOLD_ONSET_MAX_LEAD_RAD: f64 = 0.15;

use crate::friction::{POSITION_HOLD_ERROR_DEADBAND_RAD, POSITION_HOLD_ONSET_MS};
use crate::position_trajectory::{is_descent_return, is_gravity_assisted_return, TrapezoidPhase};

/// Effective max lead including outbound/return breakaway boost during onset window.
pub fn position_hold_effective_max_lead(
    max_lead: f64,
    retarget_age_ms: u64,
    approaching_target: bool,
    settle_error: f64,
    q: f64,
) -> f64 {
    let breakaway_from_home = q.abs() <= POSITION_RETURN_DESCENT_SEED_RAD;
    let outbound_breakaway = breakaway_from_home
        && approaching_target
        && settle_error > POSITION_RETURN_DESCENT_SEED_RAD;
    let target = q + settle_error;
    let return_breakaway =
        is_gravity_assisted_return(q, target) && settle_error < -POSITION_RETURN_DESCENT_SEED_RAD;
    if retarget_age_ms <= POSITION_HOLD_ONSET_MS
        && settle_error.abs() > POSITION_RETURN_DESCENT_SEED_RAD
        && (outbound_breakaway || return_breakaway)
    {
        max_lead.max(POSITION_HOLD_ONSET_MAX_LEAD_RAD)
    } else {
        max_lead
    }
}

/// Downward planner speed after retarget/reset — large home returns seed closer to cruise.
pub fn downward_return_seed_velocity(slew_rad_s: f64, v_max: f64, q: f64, target: f64) -> f64 {
    let base = slew_rad_s.min(v_max);
    if !is_descent_return(q, target, POSITION_RETURN_DESCENT_SEED_RAD) {
        return base;
    }
    let span = (q - target).abs();
    if span <= POSITION_RETURN_DESCENT_SEED_RAD * 4.0 {
        return base;
    }
    v_max.min(base.max(v_max * 0.35))
}

/// MIT velocity FF: zero at rest except during post-retarget onset while approaching.
pub fn position_hold_mit_velocity(
    dq_raw: f64,
    dq_traj: f64,
    velocity_deadband: f64,
    retarget_age_ms: u64,
    approaching_target: bool,
) -> f64 {
    if dq_raw.abs() >= velocity_deadband {
        return dq_traj;
    }
    if approaching_target
        && retarget_age_ms <= POSITION_HOLD_ONSET_MS
        && dq_traj.abs() > POSITION_HOLD_ERROR_DEADBAND_RAD
    {
        return dq_traj;
    }
    0.0
}

/// MIT `kd` while arm is moving — firmware velocity damping while the arm has speed.
pub fn position_hold_mit_kd(
    kd: f64,
    _q: f64,
    _target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> f64 {
    if dq_filtered.abs() < velocity_deadband {
        return 0.0;
    }
    kd
}

/// MIT pull while ascending toward target and stuck with planner slightly ahead.
pub fn approach_stuck_mit_pull(
    to_target: f64,
    q: f64,
    q_traj: f64,
    dq_filtered: f64,
    dq_traj: f64,
    velocity_deadband: f64,
) -> bool {
    to_target > POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_traj > POSITION_HOLD_ERROR_DEADBAND_RAD
        && q_traj >= q - POSITION_SETTLE_TOLERANCE_RAD
        && (q_traj - q) < POSITION_RETURN_RESYNC_RAD
        && dq_filtered.abs() < velocity_deadband
}

/// MIT pull-down while descending and stuck (cleared by [`descent_breakaway_confirmed`]).
pub fn descent_stuck_mit_pull(
    to_target: f64,
    q: f64,
    target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
    breakaway_confirmed: bool,
) -> bool {
    !breakaway_confirmed
        && to_target < -POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_filtered.abs() < velocity_deadband
        && (high_angle_descent_stuck(q, target) || home_final_approach_stuck(q, target))
}

/// Weighted return above `POSITION_RETURN_DESCENT_SEED_RAD` — gravity-assisted same-side descent.
fn high_angle_descent_stuck(q: f64, target: f64) -> bool {
    is_gravity_assisted_return(q, target) && (q - target) > POSITION_RETURN_DESCENT_SEED_RAD
}

/// Last ~5–50 mrad above home — `is_gravity_assisted_return` is false at `target == 0`.
pub fn home_final_approach_stuck(q: f64, target: f64) -> bool {
    target.abs() <= POSITION_SETTLE_TOLERANCE_RAD
        && q > POSITION_HOME_SETTLE_RAD
        && q <= POSITION_RETURN_DESCENT_SEED_RAD
}

/// MIT pull-down lead while stuck in [`home_final_approach_stuck`] (includes pull-through past target).
pub fn home_final_approach_stuck_pull_rad(q: f64, target: f64) -> f64 {
    if !home_final_approach_stuck(q, target) {
        return POSITION_DESCENT_STUCK_LEAD_RAD;
    }
    (q - target + POSITION_HOME_FINAL_PULL_THROUGH_RAD).clamp(
        POSITION_DESCENT_STUCK_LEAD_RAD,
        POSITION_HOLD_ONSET_MAX_LEAD_RAD,
    )
}

pub fn descent_breakaway_confirmed(
    to_target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> bool {
    to_target < -POSITION_HOLD_ERROR_DEADBAND_RAD
        && dq_filtered <= -velocity_deadband * crate::friction::POSITION_STUCK_EXIT_VELOCITY_RATIO
}

/// Arm stuck at the lead cap with no motion — snap `q_traj` to measured `q`.
pub fn planner_should_resync_stuck_lead(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    dq_filtered: f64,
    max_lead: f64,
    velocity_deadband: f64,
) -> bool {
    if (target - q).abs() <= POSITION_SETTLE_TOLERANCE_RAD {
        return false;
    }
    if dq_filtered.abs() >= velocity_deadband {
        return false;
    }
    if is_descent_return(q, target, POSITION_RETURN_DESCENT_SEED_RAD)
        && target < q
        && q > planner.q_traj
        && planner.phase() != TrapezoidPhase::Hold
    {
        return false;
    }
    (q - planner.q_traj).abs() > max_lead - 1e-6
}

/// Arm past latched target and at rest — stop trapezoid hunting (hold overshoot wiggle).
pub fn planner_should_latch_on_overshoot_hold(
    q: f64,
    q_traj: f64,
    target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> bool {
    let target_reached_by_planner = if target >= 0.0 {
        q_traj >= target - POSITION_SETTLE_TOLERANCE_RAD
    } else {
        q_traj <= target + POSITION_SETTLE_TOLERANCE_RAD
    };
    target.abs() > POSITION_RETURN_FREEZE_Q_MAX_RAD
        && target_reached_by_planner
        && (q - target).abs() > POSITION_RETURN_RESYNC_RAD
        && dq_filtered.abs() < velocity_deadband
}

/// Freeze planner while arm lags on return-to-home descent; hysteresis exit on filtered downward motion.
#[allow(clippy::too_many_arguments)]
pub fn planner_should_freeze_on_descent(
    was_frozen: bool,
    target: f64,
    q: f64,
    to_target: f64,
    lag: f64,
    dq_traj: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
    max_lead: f64,
) -> bool {
    if target.abs() > POSITION_SETTLE_TOLERANCE_RAD {
        return false;
    }
    if home_final_approach_stuck(q, target) {
        return false;
    }
    if q > POSITION_RETURN_FREEZE_Q_MAX_RAD {
        return false;
    }
    // Final-band stick-slip assist only — not while still descending from approach overshoot.
    if (q - target).abs() > POSITION_RETURN_DESCENT_SEED_RAD {
        return false;
    }
    let settle_band = return_settle_band(target);
    if was_frozen
        && (lag.abs() < POSITION_RETURN_RESYNC_RAD
            || (home_final_approach_stuck(q, target) && dq_filtered.abs() < velocity_deadband))
    {
        return false;
    }
    if (q - target).abs() <= settle_band {
        return false;
    }
    let lagging = to_target < -POSITION_HOLD_ERROR_DEADBAND_RAD
        && lag > POSITION_RETURN_RESYNC_RAD
        && lag < max_lead
        && dq_traj < -POSITION_HOLD_ERROR_DEADBAND_RAD;
    if !lagging {
        return false;
    }
    let exit_v = velocity_deadband * crate::friction::POSITION_STUCK_EXIT_VELOCITY_RATIO;
    if was_frozen {
        dq_filtered > -exit_v
    } else {
        dq_filtered.abs() < velocity_deadband
    }
}

/// Trajectory setpoint clamp: brake when `q` outruns `q_traj`, follow when lagging toward target.
pub fn clamp_trajectory_setpoint(
    q_traj: f64,
    q: f64,
    target: f64,
    max_lead: f64,
    policy: Option<&JointLimitPolicy>,
    dq_traj: f64,
) -> f64 {
    const TOL: f64 = 1e-4;
    let to_target = target - q;
    let lag = q_traj - q;
    let mut q_des =
        if to_target.abs() > TOL && lag.signum() == to_target.signum() && lag.abs() > max_lead {
            if to_target > 0.0 {
                q_traj.clamp(q, target)
            } else {
                q_traj.clamp(target, q)
            }
        } else {
            q_traj.clamp(q - max_lead, q + max_lead)
        };

    if to_target.abs() > TOL {
        if to_target > 0.0 && q > q_traj && (q - q_traj) < POSITION_RETURN_RESYNC_RAD {
            q_des = q_des.max(q);
        } else if to_target < 0.0
            && q < q_traj
            && ((q_traj - q) < POSITION_RETURN_RESYNC_RAD
                || (target.abs() <= TOL && q.abs() < POSITION_RETURN_FREEZE_Q_MAX_RAD))
        {
            if let Some(p) = policy {
                let (lo, _) = effective_command_bounds(p, q, dq_traj);
                q_des = q_des.min(q.max(lo));
            } else {
                q_des = q_des.min(q);
            }
        }
    }

    let settle_error = target - q;
    let overshot_past_target = if target >= 0.0 {
        q > target + POSITION_RETURN_RESYNC_RAD && q_traj >= target - TOL
    } else {
        q < target - POSITION_RETURN_RESYNC_RAD && q_traj <= target + TOL
    };
    if overshot_past_target && target.abs() > POSITION_RETURN_FREEZE_Q_MAX_RAD {
        q_des = target;
        if let Some(p) = policy {
            let (lo, hi) = effective_command_bounds(p, q, dq_traj);
            q_des = q_des.clamp(lo, hi);
        }
    } else if settle_error < -TOL && q_traj <= target + TOL {
        q_des = q_des.max(target);
        if let Some(p) = policy {
            let (lo, _) = effective_command_bounds(p, q, dq_traj);
            q_des = q_des.min(q).max(lo);
        } else {
            q_des = q_des.min(q);
        }
    } else if settle_error > TOL && q_traj >= target - TOL {
        if let Some(p) = policy {
            let (_, hi) = effective_command_bounds(p, q, dq_traj);
            q_des = q_des.min(target).max(q).min(hi);
        } else {
            q_des = q_des.min(target).max(q);
        }
    }
    if let Some(p) = policy {
        q_des = clamp_position_in_envelope(p, q, dq_traj, q_des);
    }
    q_des
}

/// Virtual trapezoid latched at target while measured `q` is still short of target — reopen, do not reset.
pub fn planner_premature_hold(planner: &JointPositionPlanner, q: f64, target: f64) -> bool {
    if planner.phase() != TrapezoidPhase::Hold {
        return false;
    }
    let band = return_settle_band(target);
    if (q - target).abs() <= band {
        return false;
    }
    let tol = POSITION_SETTLE_TOLERANCE_RAD;
    if target >= 0.0 {
        planner.q_traj >= target - tol
    } else {
        planner.q_traj <= target + tol
    }
}

/// Virtual trapezoid latched at target while measured `q` overshot — planner at target, arm past.
pub fn planner_overshot_at_hold(planner: &JointPositionPlanner, q: f64, target: f64) -> bool {
    if planner.phase() != TrapezoidPhase::Hold {
        return false;
    }
    let tol = POSITION_SETTLE_TOLERANCE_RAD;
    if target >= 0.0 {
        q > target + POSITION_RETURN_RESYNC_RAD && planner.q_traj >= target - tol
    } else {
        q < target - POSITION_RETURN_RESYNC_RAD && planner.q_traj <= target + tol
    }
}

/// Virtual trapezoid latched at target while measured `q` overshot and is still moving — reopen.
pub fn planner_overshoot_hold_while_moving(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> bool {
    if dq_filtered.abs() < velocity_deadband {
        return false;
    }
    if !planner_overshot_at_hold(planner, q, target) {
        return false;
    }
    if target >= 0.0 {
        dq_filtered > velocity_deadband
    } else {
        dq_filtered < -velocity_deadband
    }
}

/// Resume cruise toward `target` without snapping `q_traj` back to measured `q`.
pub fn reopen_planner_from_premature_hold(
    planner: &mut JointPositionPlanner,
    q: f64,
    target: f64,
    v_max: f64,
) {
    let dir = (target - q).signum();
    if dir > 0.0 {
        planner.q_traj = planner.q_traj.max(q);
    } else if dir < 0.0 {
        planner.q_traj = planner.q_traj.min(q);
    }
    planner.resume_cruise_toward(planner.q_traj, dir * v_max);
}

pub fn planner_drifted_from_measurement(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    max_lead: f64,
) -> bool {
    if planner_premature_hold(planner, q, target) {
        return false;
    }
    if planner_overshot_at_hold(planner, q, target) {
        return false;
    }
    let to_target = target - q;
    if to_target.abs() > POSITION_SETTLE_TOLERANCE_RAD {
        if to_target > 0.0 && q > planner.q_traj + max_lead {
            return false;
        }
        if to_target < 0.0 && q < planner.q_traj - max_lead {
            return false;
        }
        if is_descent_return(q, target, POSITION_RETURN_DESCENT_SEED_RAD)
            && q > planner.q_traj + max_lead
            && planner.phase() != TrapezoidPhase::Hold
        {
            return false;
        }
    }
    if (q - planner.q_traj).abs() > max_lead && (target - q).abs() > POSITION_SETTLE_TOLERANCE_RAD {
        return true;
    }
    planner.phase() == TrapezoidPhase::Hold
        && (q - target).abs() > return_settle_band(target)
        && (q - planner.q_traj).abs() > POSITION_SETTLE_TOLERANCE_RAD
}
