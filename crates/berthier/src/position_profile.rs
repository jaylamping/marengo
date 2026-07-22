//! Position-hold cruise `v_max` selection and planner event tags.

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
}
