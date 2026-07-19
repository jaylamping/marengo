//! Position-hold motion profile selection (small hold vs trajectory vs return).

use crate::position_trajectory::is_gravity_assisted_return;

/// Motion profile for position hold — drives planner speed and setpoint policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionMotionProfile {
    /// Local retarget within `position_trajectory_threshold_rad`.
    SmallHold,
    /// Full trapezoid cruise toward a distant target.
    TrajectoryMove,
    /// Gravity-assisted return toward home.
    ReturnHome,
    /// Cross-home or limit-directed probe (distinct from ordinary return).
    LimitProbe,
}

/// Planner event tag for trace CSV (`planner_event` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerEvent {
    Tick,
    Reset,
    Latch,
    FreezeEnter,
    FreezeExit,
    ResyncStuckLead,
    EnvelopeClamp,
}

impl PlannerEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::Reset => "reset",
            Self::Latch => "latch",
            Self::FreezeEnter => "freeze_enter",
            Self::FreezeExit => "freeze_exit",
            Self::ResyncStuckLead => "resync",
            Self::EnvelopeClamp => "envelope_clamp",
        }
    }
}

/// Select cruise `v_max` from move distance and configured thresholds.
pub fn position_profile_v_max(
    move_dist: f64,
    slew_rad_s: f64,
    trajectory_v_max: f64,
    trajectory_threshold_rad: f64,
) -> f64 {
    if move_dist <= trajectory_threshold_rad {
        slew_rad_s
    } else {
        trajectory_v_max
    }
}

/// Effective `v_max` during planner tick — preserves bleed guard when a small retarget
/// follows a fast segment and the planner is still coasting above slew speed.
pub fn position_hold_v_max(
    move_dist: f64,
    slew_rad_s: f64,
    trajectory_v_max: f64,
    trajectory_threshold_rad: f64,
    planner_speed: f64,
) -> f64 {
    if move_dist <= trajectory_threshold_rad && planner_speed <= slew_rad_s + 1e-9 {
        slew_rad_s
    } else {
        trajectory_v_max
    }
}

/// Classify the active motion profile for diagnostics and future policy gates.
pub fn classify_position_profile(
    q: f64,
    target: f64,
    move_dist: f64,
    trajectory_threshold_rad: f64,
) -> PositionMotionProfile {
    if move_dist <= trajectory_threshold_rad {
        PositionMotionProfile::SmallHold
    } else if is_gravity_assisted_return(q, target) {
        PositionMotionProfile::ReturnHome
    } else if q * target < 0.0 {
        PositionMotionProfile::LimitProbe
    } else {
        PositionMotionProfile::TrajectoryMove
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_v_max_selects_slew_for_layer2_move() {
        assert!((position_profile_v_max(0.1, 0.15, 2.0, 0.15) - 0.15).abs() < 1e-12);
        assert!((position_profile_v_max(1.57, 0.15, 2.0, 0.15) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn profile_v_max_zero_threshold_always_trajectory() {
        assert!((position_profile_v_max(0.1, 0.15, 2.0, 0.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn hold_v_max_bleeds_fast_planner_on_small_retarget() {
        assert!((position_hold_v_max(0.1, 0.15, 2.0, 0.15, 0.0) - 0.15).abs() < 1e-12);
        assert!((position_hold_v_max(0.1, 0.15, 2.0, 0.15, 1.5) - 2.0).abs() < 1e-12);
        assert!((position_hold_v_max(1.57, 0.15, 2.0, 0.15, 0.0) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn classify_small_hold_at_layer2() {
        assert_eq!(
            classify_position_profile(0.0, 0.1, 0.1, 0.15),
            PositionMotionProfile::SmallHold
        );
    }
}
