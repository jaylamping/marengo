//! Operator speed fraction for a `hold-at` move (ADR 0040 amendment, 2026-10-28).
//!
//! A share `f` of the joint's configured planner speeds (`position_slew_rad_s`,
//! `position_trajectory_velocity_rad_s` and the velocity cap) for one move. It can only slow a
//! move: `0 < f ≤ 1`. Every other value (zero, negative, above one, NaN, ±∞) is refused, never
//! clamped into range, so every command surface (stdin, the gateway, marengo-pi's Testing batch
//! and Berthier) shares this one rule.

use std::fmt;

use thiserror::Error;

/// A validated speed fraction in `(0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SpeedFraction(f64);

/// A speed fraction outside `(0, 1]` (or not a finite number).
#[derive(Debug, Clone, Copy, PartialEq, Error)]
#[error("speed fraction {value} refused: it must be a number in (0, 1] (it can only slow a move)")]
pub struct InvalidSpeedFraction {
    pub value: f64,
}

impl SpeedFraction {
    /// The configured planner speed (what a hold-at without a fraction runs at).
    pub const FULL: Self = Self(1.0);

    /// Accept `value` only when `0 < value ≤ 1`; NaN and infinities are refused.
    pub fn new(value: f64) -> Result<Self, InvalidSpeedFraction> {
        if value.is_finite() && value > 0.0 && value <= 1.0 {
            Ok(Self(value))
        } else {
            Err(InvalidSpeedFraction { value })
        }
    }

    /// An optional request: absent is [`Self::FULL`], present must pass [`Self::new`].
    pub fn from_optional(value: Option<f64>) -> Result<Self, InvalidSpeedFraction> {
        value.map_or(Ok(Self::FULL), Self::new)
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

impl Default for SpeedFraction {
    fn default() -> Self {
        Self::FULL
    }
}

impl fmt::Display for SpeedFraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_half_open_unit_interval() {
        for value in [1.0, 0.25, 0.5, f64::MIN_POSITIVE] {
            assert_eq!(SpeedFraction::new(value).map(SpeedFraction::get), Ok(value));
        }
        for value in [
            0.0,
            -0.0,
            -0.25,
            1.0 + f64::EPSILON,
            1.5,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert!(
                SpeedFraction::new(value).is_err(),
                "{value} must be refused"
            );
        }
    }

    #[test]
    fn absent_is_full_speed_and_present_is_validated() {
        assert_eq!(SpeedFraction::from_optional(None), Ok(SpeedFraction::FULL));
        assert!(SpeedFraction::from_optional(Some(0.0)).is_err());
        assert_eq!(
            SpeedFraction::from_optional(Some(0.25)).map(SpeedFraction::get),
            Ok(0.25)
        );
    }
}
