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
    let target = q + settle_error;
    let in_low_angle_band = q.abs() <= POSITION_HOME_BREAKAWAY_Q_MAX_RAD;
    let outbound_breakaway =
        in_low_angle_band && approaching_target && settle_error > POSITION_RETURN_DESCENT_SEED_RAD;
    let return_breakaway =
        is_gravity_assisted_return(q, target) && settle_error < -POSITION_RETURN_DESCENT_SEED_RAD;
    if low_angle_breakaway_active(q, target, settle_error, approaching_target) {
        return max_lead.max(POSITION_HOLD_ONSET_MAX_LEAD_RAD);
    }
    if retarget_age_ms <= POSITION_HOLD_ONSET_MS
        && settle_error.abs() > POSITION_RETURN_DESCENT_SEED_RAD
        && (outbound_breakaway || return_breakaway)
    {
        max_lead.max(POSITION_HOLD_ONSET_MAX_LEAD_RAD)
    } else {
        max_lead
    }
}

/// Sustained lead boost in the ~0–30° friction-knee band (outlasts the 300 ms onset window).
///
/// Outbound (`approaching_target`) never sustains — lead boost there is onset-only via
/// [`position_hold_effective_max_lead`]. Home-final and return/descent sides still boost.
pub fn low_angle_breakaway_active(
    q: f64,
    target: f64,
    settle_error: f64,
    approaching_target: bool,
) -> bool {
    const LOW_ANGLE_SPAN_MAX_RAD: f64 = 0.30;
    if q > LOW_ANGLE_SPAN_MAX_RAD {
        return false;
    }
    if home_final_approach_stuck(q, target) {
        return true;
    }
    // Outbound sustained boost disabled — prevents lead-unlimited hold-at that blocks
    // stuck-lead resync (bench grind: q stuck, q_traj at target).
    if approaching_target {
        return false;
    }
    if target > LOW_ANGLE_SPAN_MAX_RAD {
        return false;
    }
    settle_error < -POSITION_HOME_SETTLE_RAD
}

/// Downward planner speed after retarget/reset — large home returns seed closer to cruise.
/// Cap `dq_cmd` used only for limit-envelope hold-target clamping near hard stops.
///
/// Cruise retarget velocity inflates kinetic margin (~0.12 rad at 1.25 rad/s), which blocks
/// `hold-at 0` on joints whose lower limit is 0 — the target gets clamped to `hard_lower + margin`.
///
/// When `hard_lower` is slightly negative (bench roll −0.05), operator home is still `0`, so the
/// gate is `max(hard_lower + band, band)` — not `hard_lower + band` alone.
pub fn envelope_dq_cmd_for_hold_clamp(
    policy: Option<&JointLimitPolicy>,
    q: f64,
    requested_rad: f64,
    dq_cmd: f64,
    slew_rad_s: f64,
) -> f64 {
    let Some(policy) = policy else {
        return dq_cmd;
    };
    let home_band = POSITION_HOME_SETTLE_RAD;
    let lower_home_gate = (policy.hard_lower() + home_band).max(home_band);
    if requested_rad <= lower_home_gate && q > requested_rad + home_band && dq_cmd < 0.0 {
        return -dq_cmd.abs().min(slew_rad_s);
    }
    if requested_rad >= policy.hard_upper() - home_band
        && q < requested_rad - home_band
        && dq_cmd > 0.0
    {
        return dq_cmd.abs().min(slew_rad_s);
    }
    dq_cmd
}

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

/// Outbound friction-knee stall: lead saturated, no motion, target still in low-angle band.
pub fn outbound_low_angle_stuck(
    q: f64,
    target: f64,
    to_target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
    lag: f64,
    effective_max_lead: f64,
) -> bool {
    const LOW_ANGLE_SPAN_MAX_RAD: f64 = 0.30;
    to_target > POSITION_RETURN_DESCENT_SEED_RAD
        && target <= LOW_ANGLE_SPAN_MAX_RAD
        && q > POSITION_HOME_SETTLE_RAD
        && q <= LOW_ANGLE_SPAN_MAX_RAD
        && dq_filtered.abs() < velocity_deadband
        && lag >= effective_max_lead - 1e-6
}

/// MIT pull-up lead while stuck in [`outbound_low_angle_stuck`].
pub fn outbound_low_angle_stuck_pull_rad(to_target: f64, effective_max_lead: f64) -> f64 {
    to_target
        .min(effective_max_lead)
        .max(POSITION_DESCENT_STUCK_LEAD_RAD)
}

/// Ascent MIT pull-harder — **disabled**.
///
/// Increasing lead while stuck ascending worsens open-loop grind. Ascent stalls freeze the
/// planner via [`planner_should_freeze_on_ascent_stall`]; descent uses [`descent_stuck_mit_pull`].
/// Helpers [`outbound_low_angle_stuck`] / [`approach_stuck_mit_pull_lead_rad`] remain for tests.
#[allow(clippy::too_many_arguments)]
pub fn approach_stuck_mit_pull(
    _to_target: f64,
    _q: f64,
    _target: f64,
    _q_traj: f64,
    _dq_filtered: f64,
    _dq_traj: f64,
    _velocity_deadband: f64,
    _effective_max_lead: f64,
) -> bool {
    false
}

/// Pull-up lead for disabled [`approach_stuck_mit_pull`] (kept for helper/tests).
pub fn approach_stuck_mit_pull_lead_rad(to_target: f64, lag: f64, effective_max_lead: f64) -> f64 {
    if lag >= effective_max_lead - 1e-6 {
        outbound_low_angle_stuck_pull_rad(to_target, effective_max_lead)
    } else {
        (lag + POSITION_HOME_FINAL_PULL_THROUGH_RAD)
            .clamp(POSITION_DESCENT_STUCK_LEAD_RAD, effective_max_lead)
    }
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

/// Upper q bound for home breakaway MIT pull — covers low-angle stall above the 50 mrad seed band.
pub(crate) const POSITION_HOME_BREAKAWAY_Q_MAX_RAD: f64 = 0.15;

/// Last ~5–150 mrad above home — `is_gravity_assisted_return` is false at `target == 0`.
pub fn home_final_approach_stuck(q: f64, target: f64) -> bool {
    target.abs() <= POSITION_SETTLE_TOLERANCE_RAD
        && q > POSITION_HOME_SETTLE_RAD
        && q <= POSITION_HOME_BREAKAWAY_Q_MAX_RAD
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
///
/// Requires the planner reference to be *at* the target (`|q_traj - target| ≤ tol`), not merely
/// on the far side of it. Using `q_traj >= target` for positive targets falsely latches at the
/// start of a descent retarget (e.g. wave reverse 0.8 → 0.4) and commands a MIT jump.
pub fn planner_should_latch_on_overshoot_hold(
    q: f64,
    q_traj: f64,
    target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> bool {
    let planner_at_target = (q_traj - target).abs() <= POSITION_SETTLE_TOLERANCE_RAD;
    let overshot = if target >= 0.0 {
        q > target + POSITION_RETURN_RESYNC_RAD
    } else {
        q < target - POSITION_RETURN_RESYNC_RAD
    };
    target.abs() > POSITION_RETURN_FREEZE_Q_MAX_RAD
        && planner_at_target
        && overshot
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

/// Freeze planner while stuck ascending with `q_traj` ahead of measured `q`.
///
/// Owns outbound stalls (`target` not ≈0). Home return uses [`planner_should_freeze_on_descent`].
/// Exit when synced (`|q_traj − q| < POSITION_RETURN_RESYNC_RAD`) or motion resumes toward target.
pub fn planner_should_freeze_on_ascent_stall(
    was_frozen: bool,
    target: f64,
    q: f64,
    q_traj: f64,
    to_target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> bool {
    // Home return descent freeze owns target≈0.
    if target.abs() <= POSITION_SETTLE_TOLERANCE_RAD {
        return false;
    }
    let settle_band = return_settle_band(target);
    if (q - target).abs() <= settle_band {
        return false;
    }
    let lead = q_traj - q;
    if was_frozen && lead.abs() < POSITION_RETURN_RESYNC_RAD {
        return false;
    }
    let planner_ahead = to_target > POSITION_HOLD_ERROR_DEADBAND_RAD
        && lead > POSITION_RETURN_RESYNC_RAD;
    if !planner_ahead {
        return false;
    }
    let exit_v = velocity_deadband * crate::friction::POSITION_STUCK_EXIT_VELOCITY_RATIO;
    let moving_toward_target = if to_target > 0.0 {
        dq_filtered >= exit_v
    } else {
        dq_filtered <= -exit_v
    };
    if was_frozen {
        !moving_toward_target
    } else {
        dq_filtered.abs() < velocity_deadband
    }
}

/// Reopen premature/overshoot Hold only when the arm is moving.
///
/// Stuck premature hold must not reopen — that keeps `q_traj` at target, flashes Cruise, then
/// re-Holds → Reset oscillator. Prefer ascent-stall freeze / stuck-lead resync instead.
pub fn planner_should_reopen_premature_hold(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    dq_filtered: f64,
    velocity_deadband: f64,
) -> bool {
    if planner_overshoot_hold_while_moving(planner, q, target, dq_filtered, velocity_deadband) {
        return true;
    }
    if !planner_premature_hold(planner, q, target) {
        return false;
    }
    dq_filtered.abs() >= velocity_deadband
}

/// Hold while measured `q` is still short of target — lead-follow from `q`.
///
/// Parking `q_traj` at target produces dead P-creep; restarting a full trapezoid races `q_traj`
/// to target and thrashes (bench: 9 resets, q stuck ~0.13). Lead-follow keeps
/// `q_traj = q ± lead` so P stays engaged while the reference tracks the arm. Applies whenever
/// Hold and short (including after a prior lead-follow park). Skip overshoot.
pub fn planner_should_lead_follow_hold_short(
    planner: &JointPositionPlanner,
    q: f64,
    target: f64,
    _dq_filtered: f64,
    _velocity_deadband: f64,
) -> bool {
    if planner.phase() != TrapezoidPhase::Hold {
        return false;
    }
    let short = if target >= 0.0 {
        q < target - POSITION_HOME_SETTLE_RAD
    } else {
        q > target + POSITION_HOME_SETTLE_RAD
    };
    if !short {
        return false;
    }
    // Overshoot past target — latch/overshoot paths own that.
    if target >= 0.0 {
        q <= target + POSITION_RETURN_RESYNC_RAD
    } else {
        q >= target - POSITION_RETURN_RESYNC_RAD
    }
}

/// Finish a short Hold with reference at/near target and non-zero cruise `dq_traj`.
///
/// Open-loop tick stays frozen (caller). Non-zero `dq_traj` keeps trajectory-velocity friction
/// engaged so the last ~20 mrad does not feel like a dead stop + P-creep. When remaining is
/// below `max_lead`, `q_traj` equals `target` — that is geometrically required for full lead.
pub fn apply_lead_follow_hold_short(
    planner: &mut JointPositionPlanner,
    q: f64,
    target: f64,
    max_lead: f64,
    slew_rad_s: f64,
) {
    let dir = (target - q).signum();
    if dir == 0.0 {
        planner.latch_at_target(target);
        return;
    }
    let remaining = (target - q).abs();
    if remaining <= POSITION_HOME_SETTLE_RAD {
        planner.latch_at_target(target);
        return;
    }
    let lead = remaining.min(max_lead);
    let q_ref = q + dir * lead;
    let v = slew_rad_s.max(POSITION_HOME_SETTLE_RAD);
    planner.resume_cruise_toward(q_ref, dir * v);
}

/// Trajectory setpoint clamp: brake when `q` outruns `q_traj`, follow when lagging toward target.
///
/// Always lead-bounded: `q_des` starts as `q_traj` clamped to `[q − max_lead, q + max_lead]`.
/// Never drops the lead bound when `|q_traj − q| > max_lead` (open-loop follow is forbidden).
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
    let mut q_des = q_traj.clamp(q - max_lead, q + max_lead);

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
    // Snap to target only when the planner has actually arrived near it — not when a
    // fresh descent retarget still has q_traj ≫ target (wave reverse fault).
    let planner_near_target = (q_traj - target).abs() <= max_lead + TOL;
    let overshot_past_target = if target >= 0.0 {
        q > target + POSITION_RETURN_RESYNC_RAD && planner_near_target && q_traj >= target - TOL
    } else {
        q < target - POSITION_RETURN_RESYNC_RAD && planner_near_target && q_traj <= target + TOL
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
    // Planner reference finished at target; measured q has not arrived yet (lag on ascent or descent).
    (planner.q_traj - target).abs() <= tol
}

/// Virtual trapezoid latched at target while measured `q` overshot — planner at target, arm past.
pub fn planner_overshot_at_hold(planner: &JointPositionPlanner, q: f64, target: f64) -> bool {
    if planner.phase() != TrapezoidPhase::Hold {
        return false;
    }
    let tol = POSITION_SETTLE_TOLERANCE_RAD;
    if (planner.q_traj - target).abs() > tol {
        return false;
    }
    if target >= 0.0 {
        q > target + POSITION_RETURN_RESYNC_RAD
    } else {
        q < target - POSITION_RETURN_RESYNC_RAD
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
