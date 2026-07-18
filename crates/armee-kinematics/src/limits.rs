//! Velocity-scaled position limit envelope (ADR 0009).

/// Hard and soft position bounds for one joint (rad).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointLimitBounds {
    pub hard_lower: f64,
    pub hard_upper: f64,
    pub soft_lower: f64,
    pub soft_upper: f64,
}

impl JointLimitBounds {
    pub fn from_hard_and_soft(
        hard_lower: f64,
        hard_upper: f64,
        soft_lower: Option<f64>,
        soft_upper: Option<f64>,
    ) -> Self {
        let soft_lower = soft_lower.unwrap_or(hard_lower);
        let soft_upper = soft_upper.unwrap_or(hard_upper);
        Self {
            hard_lower,
            hard_upper,
            soft_lower: soft_lower.clamp(hard_lower, hard_upper),
            soft_upper: soft_upper.clamp(hard_lower, hard_upper),
        }
    }
}

/// Per-joint margin tuning from `control.yaml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitMarginConfig {
    pub min_rad: f64,
    pub k_v_s: f64,
    pub k_stop: f64,
    pub velocity_deadband_rad_s: f64,
    pub measured_fault_slack_rad: f64,
    pub decel_rad_s2: f64,
}

impl Default for LimitMarginConfig {
    fn default() -> Self {
        Self {
            min_rad: 0.01,
            k_v_s: 0.02,
            k_stop: 0.5,
            velocity_deadband_rad_s: 0.02,
            measured_fault_slack_rad: 0.005,
            decel_rad_s2: 0.20,
        }
    }
}

/// Runtime limit policy for one joint (built at supervisor init).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointLimitPolicy {
    pub bounds: JointLimitBounds,
    pub margin: LimitMarginConfig,
    pub velocity: f64,
    pub effort: f64,
    pub tau_ff_max: f64,
}

impl JointLimitPolicy {
    pub fn hard_lower(&self) -> f64 {
        self.bounds.hard_lower
    }

    pub fn hard_upper(&self) -> f64 {
        self.bounds.hard_upper
    }

    pub fn soft_lower(&self) -> f64 {
        self.bounds.soft_lower
    }

    pub fn soft_upper(&self) -> f64 {
        self.bounds.soft_upper
    }
}

/// Kinetic margin from commanded velocity toward a limit.
pub fn limit_margin_rad(dq_cmd: f64, margin: &LimitMarginConfig) -> f64 {
    let v = if dq_cmd.abs() <= margin.velocity_deadband_rad_s {
        0.0
    } else {
        dq_cmd.abs()
    };
    let decel = margin.decel_rad_s2.max(1e-6);
    margin.min_rad + margin.k_v_s * v + margin.k_stop * v * v / (2.0 * decel)
}

/// Effective commandable `[lo, hi]` given motion intent `dq_cmd`.
pub fn effective_command_bounds(policy: &JointLimitPolicy, _q: f64, dq_cmd: f64) -> (f64, f64) {
    let b = policy.bounds;
    let m = limit_margin_rad(dq_cmd, &policy.margin);
    let mut lo = b.hard_lower;
    let mut hi = b.hard_upper;
    if dq_cmd < -policy.margin.velocity_deadband_rad_s {
        lo = (b.hard_lower + m).min(b.hard_upper);
    }
    if dq_cmd > policy.margin.velocity_deadband_rad_s {
        hi = (b.hard_upper - m).max(b.hard_lower);
    }
    lo = lo.max(b.soft_lower);
    hi = hi.min(b.soft_upper);
    if lo > hi {
        let mid = 0.5 * (b.soft_lower + b.soft_upper);
        (mid, mid)
    } else {
        (lo, hi)
    }
}

/// Clamp `position_rad` into the effective envelope.
pub fn clamp_position_in_envelope(
    policy: &JointLimitPolicy,
    q: f64,
    dq_cmd: f64,
    position_rad: f64,
) -> f64 {
    let (lo, hi) = effective_command_bounds(policy, q, dq_cmd);
    position_rad.clamp(lo, hi)
}

/// Operator hold-at within this band of a hard stop is a trajectory goal, not an instant MIT setpoint.
const HOLD_TARGET_STOP_TOLERANCE_RAD: f64 = 0.005;

/// Clamp operator target to soft bounds, then envelope at `dq_cmd`.
pub fn clamp_hold_target(
    policy: &JointLimitPolicy,
    q: f64,
    dq_cmd: f64,
    requested_rad: f64,
) -> f64 {
    let soft_clamped = requested_rad.clamp(policy.soft_lower(), policy.soft_upper());
    if soft_clamped <= policy.hard_lower() + HOLD_TARGET_STOP_TOLERANCE_RAD
        && q > policy.hard_lower()
    {
        return soft_clamped.max(policy.hard_lower());
    }
    if soft_clamped >= policy.hard_upper() - HOLD_TARGET_STOP_TOLERANCE_RAD
        && q < policy.hard_upper()
    {
        return soft_clamped.min(policy.hard_upper());
    }
    clamp_position_in_envelope(policy, q, dq_cmd, soft_clamped)
}

/// True when measured `q` exceeds hard limits plus fault slack.
pub fn measured_position_fault(q: f64, policy: &JointLimitPolicy) -> bool {
    let slack = policy.margin.measured_fault_slack_rad;
    q < policy.hard_lower() - slack || q > policy.hard_upper() + slack
}

/// Scale cruise `v_max` when approaching an envelope wall.
pub fn approach_velocity_cap(policy: &JointLimitPolicy, q: f64, dq_cmd: f64, v_max: f64) -> f64 {
    if v_max <= 0.0 {
        return 0.0;
    }
    let margin = limit_margin_rad(dq_cmd, &policy.margin);
    if margin <= 1e-9 {
        return v_max;
    }
    let mut scale: f64 = 1.0;
    if dq_cmd < -policy.margin.velocity_deadband_rad_s {
        let dist = q - (policy.hard_lower() + margin);
        if dist < margin {
            scale = scale.min((dist / margin).clamp(0.0, 1.0));
        }
    }
    if dq_cmd > policy.margin.velocity_deadband_rad_s {
        let dist = (policy.hard_upper() - margin) - q;
        if dist < margin {
            scale = scale.min((dist / margin).clamp(0.0, 1.0));
        }
    }
    v_max * scale
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant, clippy::expect_used)]

    use super::*;

    fn shoulder_policy() -> JointLimitPolicy {
        JointLimitPolicy {
            bounds: JointLimitBounds::from_hard_and_soft(
                -0.9,
                3.17,
                Some(-0.872665),
                Some(3.141593),
            ),
            margin: LimitMarginConfig {
                min_rad: 0.01,
                k_v_s: 0.02,
                k_stop: 0.5,
                velocity_deadband_rad_s: 0.02,
                measured_fault_slack_rad: 0.005,
                decel_rad_s2: 4.8,
            },
            velocity: 2.0,
            effort: 10.0,
            tau_ff_max: 5.0,
        }
    }

    #[test]
    fn hold_at_home_not_clamped_by_kinetic_margin() {
        let p = JointLimitPolicy {
            bounds: JointLimitBounds::from_hard_and_soft(0.0, 3.14159, None, None),
            margin: LimitMarginConfig {
                min_rad: 0.01,
                k_v_s: 0.02,
                k_stop: 0.5,
                velocity_deadband_rad_s: 0.02,
                measured_fault_slack_rad: 0.005,
                decel_rad_s2: 4.5,
            },
            velocity: 1.25,
            effort: 5.0,
            tau_ff_max: 5.0,
        };
        let clamped = clamp_hold_target(&p, 0.286, -1.25, 0.0);
        assert!(
            clamped.abs() < 1e-9,
            "hold-at home must stay at hard lower, got {clamped}"
        );
    }

    #[test]
    fn slow_move_uses_min_margin() {
        let p = shoulder_policy();
        let m = limit_margin_rad(0.0, &p.margin);
        assert!((m - 0.01).abs() < 1e-9);
        let (lo, hi) = effective_command_bounds(&p, 0.0, 0.0);
        assert!((lo - (-0.872665)).abs() < 1e-6);
        assert!((hi - 3.141593).abs() < 1e-6);
    }

    #[test]
    fn fast_descent_margin_exceeds_hard_soft_gap() {
        let p = shoulder_policy();
        let m = limit_margin_rad(-2.0, &p.margin);
        assert!(m > 0.027, "margin {m} should exceed 27 mrad hard-soft gap");
        let (lo, _hi) = effective_command_bounds(&p, 0.0, -2.0);
        assert!(
            lo > -0.872665,
            "effective lower {lo} should stay inside soft when fast"
        );
    }

    #[test]
    fn asymmetric_envelope_moving_up() {
        let p = shoulder_policy();
        let (lo_up, hi_up) = effective_command_bounds(&p, 0.0, 2.0);
        let (lo_idle, hi_idle) = effective_command_bounds(&p, 0.0, 0.0);
        assert!(
            (lo_up - lo_idle).abs() < 1e-9,
            "moving up should not shrink lower bound"
        );
        assert!(hi_up < hi_idle, "moving up should shrink upper bound");
    }

    #[test]
    fn measured_fault_respects_slack() {
        let p = shoulder_policy();
        assert!(!measured_position_fault(-0.903, &p));
        assert!(measured_position_fault(-0.906, &p));
    }

    #[test]
    fn approach_cap_scales_near_lower_wall() {
        let p = shoulder_policy();
        let margin = limit_margin_rad(-2.0, &p.margin);
        let q = p.hard_lower() + margin + 0.01;
        let cap = approach_velocity_cap(&p, q, -2.0, 2.0);
        assert!(cap < 2.0);
        assert!(cap > 0.0);
    }
}
