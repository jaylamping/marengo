//! Trapezoidal position trajectory for large `hold-at` moves (ADR 0007).

const POSITION_TOLERANCE_RAD: f64 = 1e-4;

/// Trajectory phase for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapezoidPhase {
    Accelerate,
    Cruise,
    Decelerate,
    Hold,
}

/// One joint's planner: either rate-limited slew (small moves) or trapezoid (large moves).
#[derive(Debug, Clone)]
pub struct JointPositionPlanner {
    pub q_traj: f64,
    pub dq_traj: f64,
    trapezoid_target: Option<f64>,
    phase: TrapezoidPhase,
}

impl JointPositionPlanner {
    pub fn new_slew(q: f64) -> Self {
        Self {
            q_traj: q,
            dq_traj: 0.0,
            trapezoid_target: None,
            phase: TrapezoidPhase::Hold,
        }
    }

    pub fn new_for_target(q: f64, target: f64, threshold_rad: f64) -> Self {
        if (target - q).abs() > threshold_rad {
            Self {
                q_traj: q,
                dq_traj: 0.0,
                trapezoid_target: Some(target),
                phase: TrapezoidPhase::Accelerate,
            }
        } else {
            Self::new_slew(q)
        }
    }

    pub fn uses_trajectory(&self) -> bool {
        self.trapezoid_target.is_some()
    }

    pub fn phase(&self) -> TrapezoidPhase {
        self.phase
    }

    pub fn reset_target(&mut self, q: f64, target: f64, threshold_rad: f64) {
        self.q_traj = q;
        self.dq_traj = 0.0;
        if (target - q).abs() > threshold_rad {
            self.trapezoid_target = Some(target);
            self.phase = TrapezoidPhase::Accelerate;
        } else {
            self.trapezoid_target = None;
            self.phase = TrapezoidPhase::Hold;
        }
    }

    /// Advance one control tick toward `target`.
    pub fn tick(
        &mut self,
        target: f64,
        q_measured: f64,
        dt: f64,
        slew_rad_s: f64,
        v_max: f64,
        a_max: f64,
    ) {
        if let Some(trap_target) = self.trapezoid_target {
            if (trap_target - target).abs() > POSITION_TOLERANCE_RAD {
                self.trapezoid_target = Some(target);
            }
            let (q, v, phase) = trapezoid_step(
                self.q_traj,
                self.dq_traj,
                trap_target,
                v_max,
                a_max,
                dt,
            );
            self.q_traj = q;
            self.dq_traj = v;
            self.phase = phase;
            if phase == TrapezoidPhase::Hold {
                self.trapezoid_target = None;
            }
            return;
        }

        let max_step = slew_rad_s * dt;
        let prev = self.q_traj;
        self.q_traj = slew_toward(self.q_traj, target, max_step);
        self.dq_traj = if dt > 0.0 {
            (self.q_traj - prev) / dt
        } else {
            0.0
        };
        self.phase = if (target - self.q_traj).abs() <= POSITION_TOLERANCE_RAD {
            TrapezoidPhase::Hold
        } else if self.dq_traj.abs() > POSITION_TOLERANCE_RAD {
            TrapezoidPhase::Cruise
        } else {
            TrapezoidPhase::Hold
        };
        let _ = q_measured;
    }
}

/// Move `current` toward `target` by at most `max_step` (rad).
pub fn slew_toward(current: f64, target: f64, max_step: f64) -> f64 {
    let err = target - current;
    if err.abs() <= max_step {
        target
    } else {
        current + max_step.copysign(err)
    }
}

/// One trapezoidal velocity step toward `q_target`.
pub fn trapezoid_step(
    q: f64,
    v: f64,
    q_target: f64,
    v_max: f64,
    a_max: f64,
    dt: f64,
) -> (f64, f64, TrapezoidPhase) {
    let remaining = q_target - q;
    let dir = remaining.signum();
    let dist = remaining.abs();

    if dist <= POSITION_TOLERANCE_RAD {
        return (q_target, 0.0, TrapezoidPhase::Hold);
    }

    let v_along = v * dir;
    let v_mag = v.abs();
    let stop_dist = if v_mag > 0.0 {
        v_mag * v_mag / (2.0 * a_max)
    } else {
        0.0
    };

    let (v_along_new, phase) = if stop_dist >= dist {
        let v_cap = (2.0 * a_max * dist).sqrt();
        let decel = (v_along - a_max * dt).max(0.0);
        (decel.min(v_cap), TrapezoidPhase::Decelerate)
    } else if v_along < v_max {
        let accel = (v_along + a_max * dt).min(v_max);
        let phase = if accel >= v_max - 1e-9 {
            TrapezoidPhase::Cruise
        } else {
            TrapezoidPhase::Accelerate
        };
        (accel, phase)
    } else {
        (v_max, TrapezoidPhase::Cruise)
    };

    let v_new = dir * v_along_new;
    let mut q_new = q + v_new * dt;

    if dir > 0.0 && q_new >= q_target {
        return (q_target, 0.0, TrapezoidPhase::Hold);
    }
    if dir < 0.0 && q_new <= q_target {
        return (q_target, 0.0, TrapezoidPhase::Hold);
    }

    (q_new, v_new, phase)
}

/// Damping from commanded vs measured velocity while tracking a trajectory.
pub fn trajectory_damping_torque(dq: f64, dq_des: f64, kd: f64) -> f64 {
    kd * (dq_des - dq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trapezoid_reaches_target_without_exceeding_v_max() {
        let v_max = 0.30;
        let a_max = 0.20;
        let dt = 0.005;
        let target = 1.57;
        let mut q = 0.0;
        let mut v = 0.0;
        let mut peak_v: f64 = 0.0;
        for _ in 0..5000 {
            let (q_new, v_new, phase) = trapezoid_step(q, v, target, v_max, a_max, dt);
            q = q_new;
            v = v_new;
            peak_v = peak_v.max(v.abs());
            if phase == TrapezoidPhase::Hold {
                break;
            }
        }
        assert!((q - target).abs() < 1e-3, "q={q}");
        assert!(peak_v <= v_max + 1e-6, "peak_v={peak_v}");
    }

    #[test]
    fn trapezoid_negative_move() {
        let v_max = 0.30;
        let a_max = 0.20;
        let dt = 0.005;
        let target = -0.85;
        let mut q = 0.0;
        let mut v = 0.0;
        for _ in 0..5000 {
            let (q_new, v_new, phase) = trapezoid_step(q, v, target, v_max, a_max, dt);
            q = q_new;
            v = v_new;
            if phase == TrapezoidPhase::Hold {
                break;
            }
        }
        assert!((q - target).abs() < 1e-3);
    }

    #[test]
    fn deceleration_starts_before_target() {
        let v_max = 0.30;
        let a_max = 0.20;
        let dt = 0.005;
        let target = 1.0;
        let mut q = 0.0;
        let mut v = 0.0;
        let mut saw_decel = false;
        let mut reached = false;
        for _ in 0..5000 {
            let (q_new, v_new, phase) = trapezoid_step(q, v, target, v_max, a_max, dt);
            if phase == TrapezoidPhase::Decelerate {
                saw_decel = true;
            }
            q = q_new;
            v = v_new;
            if phase == TrapezoidPhase::Hold {
                reached = true;
                break;
            }
        }
        assert!(reached);
        assert!(saw_decel, "must decelerate before hold");
    }

    #[test]
    fn planner_selects_trajectory_for_large_hold_at() {
        let p = JointPositionPlanner::new_for_target(0.0, 1.57, 0.15);
        assert!(p.uses_trajectory());
    }

    #[test]
    fn planner_uses_slew_for_small_hold_at() {
        let p = JointPositionPlanner::new_for_target(0.0, 0.1, 0.15);
        assert!(!p.uses_trajectory());
    }

    #[test]
    fn trajectory_damping_tracks_velocity_error() {
        let tau = trajectory_damping_torque(0.1, 0.2, 2.0);
        assert!((tau - 0.2).abs() < 1e-12);
    }
}
