//! Trapezoidal position trajectory for position-hold retargets (ADR 0007).

const POSITION_TOLERANCE_RAD: f64 = 1e-4;

/// Trajectory phase for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapezoidPhase {
    Accelerate,
    Cruise,
    Decelerate,
    Hold,
}

/// One joint's trapezoidal planner toward a latched target.
#[derive(Debug, Clone)]
pub struct JointPositionPlanner {
    pub q_traj: f64,
    pub dq_traj: f64,
    phase: TrapezoidPhase,
}

impl JointPositionPlanner {
    pub fn new_at(q: f64) -> Self {
        Self {
            q_traj: q,
            dq_traj: 0.0,
            phase: TrapezoidPhase::Hold,
        }
    }

    pub fn new_for_target(q: f64, target: f64) -> Self {
        let mut planner = Self::new_at(q);
        if (target - q).abs() > POSITION_TOLERANCE_RAD {
            planner.phase = TrapezoidPhase::Accelerate;
        }
        planner
    }

    pub fn phase(&self) -> TrapezoidPhase {
        self.phase
    }

    pub fn reset_target(&mut self, q: f64, target: f64) {
        self.q_traj = q;
        self.dq_traj = 0.0;
        self.phase = if (target - q).abs() > POSITION_TOLERANCE_RAD {
            TrapezoidPhase::Accelerate
        } else {
            TrapezoidPhase::Hold
        };
    }

    /// After `reset_target`, seed downward cruise speed when returning from well above `target`.
    /// At high `q`, gravity FF exceeds friction until `dq_traj` reaches velocity deadband — causes
    /// a long return hitch from approach overshoot (e.g. weighted 0.1 rad gate).
    pub fn seed_downward_return_if_needed(
        &mut self,
        q: f64,
        target: f64,
        seed_threshold_rad: f64,
        v_seed: f64,
    ) {
        if target >= q - seed_threshold_rad || v_seed <= POSITION_TOLERANCE_RAD {
            return;
        }
        self.dq_traj = -v_seed;
        self.phase = TrapezoidPhase::Accelerate;
    }

    /// Advance one control tick toward `target` with peak velocity `v_max`.
    pub fn tick(&mut self, target: f64, dt: f64, v_max: f64, a_max: f64) {
        let (q, v, phase) = trapezoid_step(self.q_traj, self.dq_traj, target, v_max, a_max, dt);
        self.q_traj = q;
        self.dq_traj = v;
        self.phase = phase;
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
    let q_new = q + v_new * dt;

    if dir > 0.0 && q_new >= q_target {
        return (q_target, 0.0, TrapezoidPhase::Hold);
    }
    if dir < 0.0 && q_new <= q_target {
        return (q_target, 0.0, TrapezoidPhase::Hold);
    }

    (q_new, v_new, phase)
}

/// Damping from commanded vs measured velocity while tracking a trajectory.
#[allow(dead_code)] // unit tests in this crate
pub fn trajectory_damping_torque(dq: f64, dq_des: f64, kd: f64) -> f64 {
    kd * (dq_des - dq)
}

/// EMA weight for measured velocity used only in position-hold damping FF (200 Hz bench).
pub const POSITION_DAMPING_DQ_FILTER_ALPHA: f64 = 0.25;

/// Above this planner-tracking overspeed, blend braking FF toward [`POSITION_DAMPING_SPIKE_BRAKE_CAP_NM`].
pub const POSITION_DAMPING_SPIKE_OVERSPEED_RAD_S: f64 = 0.04;

/// Max braking torque from a single-tick velocity overshoot while still approaching target.
pub const POSITION_DAMPING_SPIKE_BRAKE_CAP_NM: f64 = 0.04;

/// Single-pole low-pass on measured `dq` for damping FF (not MIT velocity).
pub fn filter_dq_ema(prev: f64, dq_raw: f64, alpha: f64) -> f64 {
    alpha * dq_raw + (1.0 - alpha) * prev
}

/// Planner-aligned overspeed: measured ahead of commanded, same direction.
fn damping_overspeed_rad_s(dq: f64, dq_traj: f64) -> f64 {
    if dq_traj >= 0.0 {
        (dq - dq_traj).max(0.0)
    } else if dq_traj < 0.0 {
        (dq_traj - dq).max(0.0)
    } else {
        0.0
    }
}

/// Damping FF for position hold: filtered `dq`, spike brake cap while approaching target.
pub fn position_hold_damping_torque(
    dq_filtered: f64,
    dq_traj: f64,
    kd: f64,
    velocity_deadband: f64,
    approaching_target: bool,
) -> f64 {
    let tau_d = kd * (dq_traj - dq_filtered);
    if tau_d >= 0.0 || !approaching_target {
        return tau_d;
    }
    let overspeed = damping_overspeed_rad_s(dq_filtered, dq_traj);
    let soft = velocity_deadband * 0.5;
    if overspeed <= soft {
        return tau_d;
    }
    let span = (POSITION_DAMPING_SPIKE_OVERSPEED_RAD_S - soft).max(1e-6);
    let blend = ((overspeed - soft) / span).clamp(0.0, 1.0);
    let capped = -POSITION_DAMPING_SPIKE_BRAKE_CAP_NM;
    tau_d * (1.0 - blend) + capped * blend
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
    fn seed_downward_return_when_well_above_target() {
        let mut planner = JointPositionPlanner::new_at(0.12);
        planner.seed_downward_return_if_needed(0.12, 0.0, 0.05, 0.10);
        assert!((planner.dq_traj + 0.10).abs() < 1e-12);
        assert_eq!(planner.phase, TrapezoidPhase::Accelerate);
    }

    #[test]
    fn seed_downward_return_skips_small_delta() {
        let mut planner = JointPositionPlanner::new_at(0.03);
        planner.seed_downward_return_if_needed(0.03, 0.0, 0.05, 0.10);
        assert!((planner.dq_traj).abs() < 1e-12);
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
    fn planner_always_uses_trapezoid_for_nontrivial_move() {
        let p = JointPositionPlanner::new_for_target(0.0, 0.1);
        assert_eq!(p.phase(), TrapezoidPhase::Accelerate);
    }

    #[test]
    fn small_move_uses_low_v_max_in_tick() {
        let mut p = JointPositionPlanner::new_for_target(0.0, 0.02);
        let dt = 0.005;
        p.tick(0.02, dt, 0.10, 0.36);
        assert!(matches!(p.phase(), TrapezoidPhase::Accelerate));
        assert!((p.dq_traj - 0.0018).abs() < 1e-9);
    }

    #[test]
    fn trajectory_damping_tracks_velocity_error() {
        let tau = trajectory_damping_torque(0.1, 0.2, 2.0);
        assert!((tau - 0.2).abs() < 1e-12);
    }

    #[test]
    fn dq_ema_softens_breakaway_spike() {
        let filtered = filter_dq_ema(0.0, 0.144, POSITION_DAMPING_DQ_FILTER_ALPHA);
        assert!((filtered - 0.036).abs() < 1e-9);
    }

    #[test]
    fn damping_spike_cap_limits_mid_travel_brake() {
        let unfiltered = trajectory_damping_torque(0.144, 0.0612, 1.25);
        assert!(
            unfiltered < -0.08,
            "unfiltered spike should brake hard: {unfiltered}"
        );
        let filtered = position_hold_damping_torque(0.036, 0.0612, 1.25, 0.02, true);
        assert!(
            filtered > 0.0,
            "filtered should assist not brake: {filtered}"
        );
        let capped = position_hold_damping_torque(0.12, 0.0612, 1.25, 0.02, true);
        assert!(capped >= -POSITION_DAMPING_SPIKE_BRAKE_CAP_NM - 1e-9);
        assert!(
            capped > unfiltered,
            "cap should reduce braking: capped={capped} raw={unfiltered}"
        );
    }

    #[test]
    fn damping_no_spike_cap_when_crossed_target() {
        let tau = position_hold_damping_torque(0.12, 0.10, 1.25, 0.02, false);
        assert!((tau + 0.025).abs() < 1e-9);
    }
}
