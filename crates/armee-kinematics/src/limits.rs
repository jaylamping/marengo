//! Velocity-scaled position limit envelope (ADR 0009).

use thiserror::Error;

/// Why a set of joint bounds is unusable. Every variant is a fail-closed refusal: a joint
/// with no sane range gets no policy rather than a clamped guess.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum LimitBoundsError {
    #[error("hard limits [{lower}, {upper}] must be finite")]
    NonFiniteHard { lower: f64, upper: f64 },
    #[error("hard limits [{lower}, {upper}] must satisfy lower < upper")]
    InvertedHard { lower: f64, upper: f64 },
    #[error("soft limits [{lower}, {upper}] must be finite")]
    NonFiniteSoft { lower: f64, upper: f64 },
}

/// Hard and soft position bounds for one joint (rad).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointLimitBounds {
    pub hard_lower: f64,
    pub hard_upper: f64,
    pub soft_lower: f64,
    pub soft_upper: f64,
}

impl JointLimitBounds {
    /// Hard bounds plus optional soft bounds (default: the hard bounds), with soft clamped
    /// into hard.
    ///
    /// Errors instead of panicking (`f64::clamp` panics on `min > max` or NaN) when the hard
    /// range is non-finite or not `lower < upper`, or when a provided soft bound is
    /// non-finite. A URDF/config typo therefore reaches the caller as an error, not a library
    /// panic in supervisor construction.
    pub fn from_hard_and_soft(
        hard_lower: f64,
        hard_upper: f64,
        soft_lower: Option<f64>,
        soft_upper: Option<f64>,
    ) -> Result<Self, LimitBoundsError> {
        if !hard_lower.is_finite() || !hard_upper.is_finite() {
            return Err(LimitBoundsError::NonFiniteHard {
                lower: hard_lower,
                upper: hard_upper,
            });
        }
        if hard_lower >= hard_upper {
            return Err(LimitBoundsError::InvertedHard {
                lower: hard_lower,
                upper: hard_upper,
            });
        }
        let soft_lower = soft_lower.unwrap_or(hard_lower);
        let soft_upper = soft_upper.unwrap_or(hard_upper);
        if !soft_lower.is_finite() || !soft_upper.is_finite() {
            return Err(LimitBoundsError::NonFiniteSoft {
                lower: soft_lower,
                upper: soft_upper,
            });
        }
        Ok(Self {
            hard_lower,
            hard_upper,
            soft_lower: soft_lower.clamp(hard_lower, hard_upper),
            soft_upper: soft_upper.clamp(hard_lower, hard_upper),
        })
    }
}

/// Per-joint margin tuning from `control.yaml`. There is deliberately no `Default`: the
/// defaults live in `marengo-config` (one table), and a missing joint entry is a config error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitMarginConfig {
    pub min_rad: f64,
    pub k_v_s: f64,
    pub k_stop: f64,
    pub velocity_deadband_rad_s: f64,
    pub measured_fault_slack_rad: f64,
    pub decel_rad_s2: f64,
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
///
/// Always returns `lo <= hi`, both inside the soft range. When the kinetic margin consumes the
/// whole soft range (`lo > hi`) the envelope collapses to a single point: the soft-range point
/// nearest the measured `q` (the midpoint only when `q` is not finite), so a clamp holds
/// position instead of commanding a jump across the range.
pub fn effective_command_bounds(policy: &JointLimitPolicy, q: f64, dq_cmd: f64) -> (f64, f64) {
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
    if lo <= hi {
        return (lo, hi);
    }
    let (soft_lo, soft_hi) = (
        b.soft_lower.min(b.soft_upper),
        b.soft_lower.max(b.soft_upper),
    );
    let point = if q.is_finite() {
        q.max(soft_lo).min(soft_hi)
    } else {
        0.5 * (soft_lo + soft_hi)
    };
    (point, point)
}

/// Clamp `position` into `[lo, hi]` (`lo <= hi`). A non-finite `position` is replaced by the
/// finite `anchor` (hold where we are), or by the interval midpoint when that is not finite
/// either: `f64::clamp` would pass NaN through and `f64::max/min` would silently turn it into
/// a bound.
fn clamp_finite(position: f64, anchor: f64, lo: f64, hi: f64) -> f64 {
    let position = if position.is_finite() {
        position
    } else if anchor.is_finite() {
        anchor
    } else {
        0.5 * (lo + hi)
    };
    position.max(lo).min(hi)
}

/// Clamp `position_rad` into the effective envelope.
///
/// Never returns a non-finite value: a non-finite `position_rad` falls back to `q`, then to
/// the envelope midpoint.
pub fn clamp_position_in_envelope(
    policy: &JointLimitPolicy,
    q: f64,
    dq_cmd: f64,
    position_rad: f64,
) -> f64 {
    let (lo, hi) = effective_command_bounds(policy, q, dq_cmd);
    clamp_finite(position_rad, q, lo, hi)
}

/// Operator hold-at within this band of a hard stop (or of the zero-home pose) is a trajectory
/// goal, not an instant MIT setpoint.
const HOLD_TARGET_STOP_TOLERANCE_RAD: f64 = 0.005;

/// Clamp operator target to soft bounds, then envelope at `dq_cmd`.
///
/// A target at the lowest commandable point (`soft_lower + 5 mrad`; the soft range is clamped
/// first, so this is the operator's "go to the bottom") or at operator zero-home (`|target| ≤
/// 5 mrad`, e.g. roll with `hard_lower = −0.05`) approached from above is returned as the goal
/// unclamped by the kinetic margin: the planner and the per-tick envelope still bound the
/// setpoint. Any other target is **not** exempt, however negative `hard_lower` is (right pitch
/// `−0.9`): it gets the kinetic margin like every other target.
///
/// A non-finite `requested_rad` holds `q` (clamped into the soft range); the result is always
/// finite.
pub fn clamp_hold_target(
    policy: &JointLimitPolicy,
    q: f64,
    dq_cmd: f64,
    requested_rad: f64,
) -> f64 {
    let soft_clamped = clamp_finite(requested_rad, q, policy.soft_lower(), policy.soft_upper());
    let at_lower_stop = soft_clamped <= policy.soft_lower() + HOLD_TARGET_STOP_TOLERANCE_RAD;
    let at_home = soft_clamped.abs() <= HOLD_TARGET_STOP_TOLERANCE_RAD;
    if (at_lower_stop || at_home) && q > soft_clamped {
        return soft_clamped.max(policy.hard_lower());
    }
    if soft_clamped >= policy.hard_upper() - HOLD_TARGET_STOP_TOLERANCE_RAD
        && q < policy.hard_upper()
    {
        return soft_clamped.min(policy.hard_upper());
    }
    clamp_position_in_envelope(policy, q, dq_cmd, soft_clamped)
}

/// True when measured `q` exceeds hard limits plus fault slack. Fails closed: a non-finite
/// `q` (or non-finite bounds/slack) is a fault.
pub fn measured_position_fault(q: f64, policy: &JointLimitPolicy) -> bool {
    let slack = policy.margin.measured_fault_slack_rad;
    let (lo, hi) = (policy.hard_lower() - slack, policy.hard_upper() + slack);
    !q.is_finite() || lo.is_nan() || hi.is_nan() || q < lo || q > hi
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
            )
            .expect("bounds"),
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
    fn hold_at_home_not_clamped_when_hard_lower_slightly_negative() {
        let p = JointLimitPolicy {
            bounds: JointLimitBounds::from_hard_and_soft(
                -0.05,
                3.14159,
                Some(-0.05),
                Some(3.14159),
            )
            .expect("bounds"),
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
        let clamped = clamp_hold_target(&p, 0.75, -1.25, 0.0);
        assert!(
            clamped.abs() < 1e-9,
            "hold-at 0 with hard_lower=-0.05 must not kinetic-clamp, got {clamped}"
        );
    }

    #[test]
    fn hold_at_home_not_clamped_by_kinetic_margin() {
        let p = JointLimitPolicy {
            bounds: JointLimitBounds::from_hard_and_soft(0.0, 3.14159, None, None).expect("bounds"),
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
    #[test]
    fn bounds_constructor_refuses_instead_of_panicking() {
        // `f64::clamp` panics on min > max and on NaN bounds.
        assert_eq!(
            JointLimitBounds::from_hard_and_soft(1.0, -1.0, None, None),
            Err(LimitBoundsError::InvertedHard {
                lower: 1.0,
                upper: -1.0
            })
        );
        assert!(matches!(
            JointLimitBounds::from_hard_and_soft(0.5, 0.5, None, None),
            Err(LimitBoundsError::InvertedHard { .. })
        ));
        for (lo, hi) in [(f64::NAN, 1.0), (0.0, f64::NAN), (f64::NEG_INFINITY, 1.0)] {
            assert!(matches!(
                JointLimitBounds::from_hard_and_soft(lo, hi, None, None),
                Err(LimitBoundsError::NonFiniteHard { .. })
            ));
        }
        assert!(matches!(
            JointLimitBounds::from_hard_and_soft(0.0, 1.0, Some(f64::NAN), None),
            Err(LimitBoundsError::NonFiniteSoft { .. })
        ));
    }

    #[test]
    fn bounds_constructor_clamps_soft_into_hard() {
        let b =
            JointLimitBounds::from_hard_and_soft(-1.0, 1.0, Some(-2.0), Some(0.5)).expect("bounds");
        assert_eq!((b.soft_lower, b.soft_upper), (-1.0, 0.5));
        let d = JointLimitBounds::from_hard_and_soft(-1.0, 1.0, None, None).expect("bounds");
        assert_eq!((d.soft_lower, d.soft_upper), (-1.0, 1.0));
    }

    #[test]
    fn collapsed_envelope_never_inverts_with_inverted_soft() {
        // Hand-built (public fields) policy with soft_lower > soft_upper must not panic.
        let mut p = shoulder_policy();
        p.bounds.soft_lower = 1.0;
        p.bounds.soft_upper = 0.5;
        let (lo, hi) = effective_command_bounds(&p, 0.7, 0.0);
        assert!(lo <= hi);
        assert!(clamp_position_in_envelope(&p, 0.7, 0.0, 2.0).is_finite());
    }
}
