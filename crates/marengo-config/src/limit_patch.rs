//! Typed, validated updates to per-joint hard and soft limits.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{ConfigError, JointControlEntry, MotorEntry};

/// ADR 0009 hard/soft gap (~27 mrad). Set Limits soft is taught-hard inset by this amount.
pub const DEFAULT_SOFT_INSET_RAD: f64 = 0.027;

/// Soft bounds inside hard with a positive inset (never soft≡hard when span allows).
pub fn soft_limits_with_inset(hard_lower: f64, hard_upper: f64, inset: f64) -> (f64, f64) {
    let span = hard_upper - hard_lower;
    if !span.is_finite() || span <= 0.0 {
        return (hard_lower, hard_upper);
    }
    let inset = inset.clamp(0.0, span * 0.25);
    (hard_lower + inset, hard_upper - inset)
}

/// Fill missing soft fields from hard ± [`DEFAULT_SOFT_INSET_RAD`].
pub fn ensure_soft_inset(patch: &mut LimitPatch) {
    if patch.position_soft_lower_rad.is_some() && patch.position_soft_upper_rad.is_some() {
        return;
    }
    let (lo, hi) = soft_limits_with_inset(
        patch.position_lower_rad,
        patch.position_upper_rad,
        DEFAULT_SOFT_INSET_RAD,
    );
    patch.position_soft_lower_rad = Some(lo);
    patch.position_soft_upper_rad = Some(hi);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitPatch {
    pub joint: String,
    pub position_lower_rad: f64,
    pub position_upper_rad: f64,
    #[serde(default)]
    pub torque_limit_nm: Option<f64>,
    #[serde(default)]
    pub position_soft_lower_rad: Option<f64>,
    #[serde(default)]
    pub position_soft_upper_rad: Option<f64>,
    #[serde(default)]
    pub velocity_max_rad_s: Option<f64>,
}

pub fn validate_limit_patch(patch: &LimitPatch) -> Result<(), ConfigError> {
    if patch.joint.trim().is_empty() {
        return Err(invalid_patch("joint must not be empty"));
    }
    if !patch.position_lower_rad.is_finite() || !patch.position_upper_rad.is_finite() {
        return Err(invalid_patch("hard position limits must be finite"));
    }
    if patch.position_lower_rad >= patch.position_upper_rad {
        return Err(invalid_patch(format!(
            "position_lower_rad {} must be less than position_upper_rad {}",
            patch.position_lower_rad, patch.position_upper_rad
        )));
    }
    validate_positive_optional("torque_limit_nm", patch.torque_limit_nm)?;
    validate_finite_optional("position_soft_lower_rad", patch.position_soft_lower_rad)?;
    validate_finite_optional("position_soft_upper_rad", patch.position_soft_upper_rad)?;
    validate_positive_optional("velocity_max_rad_s", patch.velocity_max_rad_s)?;
    if let (Some(lower), Some(upper)) =
        (patch.position_soft_lower_rad, patch.position_soft_upper_rad)
    {
        if lower > upper {
            return Err(invalid_patch(format!(
                "position_soft_lower_rad {lower} must not exceed position_soft_upper_rad {upper}"
            )));
        }
    }
    Ok(())
}

pub fn apply_limit_patch_to_motor(
    motor: &mut MotorEntry,
    patch: &LimitPatch,
) -> Result<(), ConfigError> {
    validate_limit_patch(patch)?;
    if motor.joint != patch.joint {
        return Err(invalid_patch(format!(
            "patch for joint {} cannot update motor for {}",
            patch.joint, motor.joint
        )));
    }
    motor.bench.position_lower_rad = patch.position_lower_rad;
    motor.bench.position_upper_rad = patch.position_upper_rad;
    if let Some(torque_limit_nm) = patch.torque_limit_nm {
        motor.bench.torque_limit_nm = torque_limit_nm;
    }
    Ok(())
}

pub fn apply_limit_patch_to_control(
    entry: &mut JointControlEntry,
    patch: &LimitPatch,
) -> Result<(), ConfigError> {
    validate_limit_patch(patch)?;
    let soft_lower = patch
        .position_soft_lower_rad
        .or(entry.position_soft_lower_rad)
        .map(|lower| lower.clamp(patch.position_lower_rad, patch.position_upper_rad));
    let soft_upper = patch
        .position_soft_upper_rad
        .or(entry.position_soft_upper_rad)
        .map(|upper| upper.clamp(patch.position_lower_rad, patch.position_upper_rad));
    if let (Some(lower), Some(upper)) = (soft_lower, soft_upper) {
        if lower > upper {
            return Err(invalid_patch(format!(
                "effective soft lower {lower} must not exceed effective soft upper {upper}"
            )));
        }
    }
    entry.position_soft_lower_rad = soft_lower;
    entry.position_soft_upper_rad = soft_upper;
    if let Some(velocity_max_rad_s) = patch.velocity_max_rad_s {
        entry.velocity_max_rad_s = Some(velocity_max_rad_s);
    }
    Ok(())
}

fn validate_finite_optional(name: &str, value: Option<f64>) -> Result<(), ConfigError> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(invalid_patch(format!("{name} must be finite")));
    }
    Ok(())
}

fn validate_positive_optional(name: &str, value: Option<f64>) -> Result<(), ConfigError> {
    validate_finite_optional(name, value)?;
    if value.is_some_and(|value| value <= 0.0) {
        return Err(invalid_patch(format!("{name} must be greater than zero")));
    }
    Ok(())
}

fn invalid_patch(message: impl Into<String>) -> ConfigError {
    ConfigError::Parse {
        path: PathBuf::from("limit patch"),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use crate::{
        FrictionGains, JointControlEntry, ModeGains, MotorBenchLimits, MotorEntry, MotorType,
    };

    use super::*;

    fn patch() -> LimitPatch {
        LimitPatch {
            joint: "right_elbow_pitch".to_string(),
            position_lower_rad: -0.2,
            position_upper_rad: 1.4,
            torque_limit_nm: Some(2.5),
            position_soft_lower_rad: Some(-1.0),
            position_soft_upper_rad: Some(2.0),
            velocity_max_rad_s: Some(0.8),
        }
    }

    fn motor() -> MotorEntry {
        MotorEntry {
            joint: "right_elbow_pitch".to_string(),
            driver: "robstride".to_string(),
            motor_type: MotorType::Rs02,
            can_interface: "can0".to_string(),
            device_id: 4,
            direction: 1,
            gear_ratio: 1.0,
            recv_can_id: 0x244,
            firmware_version: "test".to_string(),
            bench: MotorBenchLimits {
                position_lower_rad: -0.05,
                position_upper_rad: 1.2,
                velocity_limit_rad_s: 1.0,
                torque_limit_nm: 3.0,
            },
        }
    }

    fn control() -> JointControlEntry {
        JointControlEntry {
            motor_type: MotorType::Rs02,
            gravity_comp: ModeGains {
                kp: 0.0,
                kd: 0.0,
                ki: 0.0,
            },
            impedance: ModeGains {
                kp: 12.0,
                kd: 1.5,
                ki: 0.0,
            },
            friction: FrictionGains {
                fc: 0.05,
                fv: 0.0,
                fo: 0.0,
                k: 10.0,
            },
            velocity_max_rad_s: None,
            position_slew_rad_s: 0.12,
            position_slew_max_lead_rad: 0.10,
            position_trajectory_threshold_rad: 0.15,
            position_trajectory_velocity_rad_s: 0.8,
            position_trajectory_accel_rad_s2: 2.5,
            position_trajectory_velocity_deadband_rad: 0.02,
            position_hold_trim_rad: 0.0,
            position_limit_margin_min_rad: 0.01,
            position_limit_margin_k_v_s: 0.02,
            position_limit_margin_k_stop: 0.5,
            position_limit_measured_fault_slack_rad: 0.005,
            position_soft_lower_rad: None,
            position_soft_upper_rad: None,
        }
    }

    #[test]
    fn soft_inset_is_inside_hard() {
        let (lo, hi) = soft_limits_with_inset(-0.5, 0.95, DEFAULT_SOFT_INSET_RAD);
        assert!((lo - (-0.5 + DEFAULT_SOFT_INSET_RAD)).abs() < 1e-12);
        assert!((hi - (0.95 - DEFAULT_SOFT_INSET_RAD)).abs() < 1e-12);
        assert!(lo < hi);
    }

    #[test]
    fn ensure_soft_inset_fills_missing_soft() {
        let mut patch = patch();
        patch.position_soft_lower_rad = None;
        patch.position_soft_upper_rad = None;
        ensure_soft_inset(&mut patch);
        assert!(patch.position_soft_lower_rad.is_some());
        assert!(patch.position_soft_upper_rad.is_some());
        assert!(patch.position_soft_lower_rad.unwrap() > patch.position_lower_rad);
        assert!(patch.position_soft_upper_rad.unwrap() < patch.position_upper_rad);
    }

    #[test]
    fn rejects_reversed_hard_limits() {
        let mut patch = patch();
        patch.position_lower_rad = patch.position_upper_rad;
        assert!(validate_limit_patch(&patch).is_err());
    }

    #[test]
    fn rejects_non_positive_optional_caps() {
        let mut patch = patch();
        patch.torque_limit_nm = Some(0.0);
        assert!(validate_limit_patch(&patch).is_err());
        patch.torque_limit_nm = Some(1.0);
        patch.velocity_max_rad_s = Some(f64::NAN);
        assert!(validate_limit_patch(&patch).is_err());
    }

    #[test]
    fn applies_motor_hard_limits_and_optional_torque() {
        let patch = patch();
        let mut motor = motor();
        apply_limit_patch_to_motor(&mut motor, &patch).expect("valid patch");
        assert_eq!(motor.bench.position_lower_rad, -0.2);
        assert_eq!(motor.bench.position_upper_rad, 1.4);
        assert_eq!(motor.bench.torque_limit_nm, 2.5);
        assert_eq!(motor.bench.velocity_limit_rad_s, 1.0);
    }

    #[test]
    fn clamps_soft_limits_into_new_hard_bounds() {
        let patch = patch();
        let mut control = control();
        apply_limit_patch_to_control(&mut control, &patch).expect("valid patch");
        assert_eq!(control.position_soft_lower_rad, Some(-0.2));
        assert_eq!(control.position_soft_upper_rad, Some(1.4));
        assert_eq!(control.velocity_max_rad_s, Some(0.8));
    }

    #[test]
    fn rejects_patch_for_a_different_joint() {
        let patch = patch();
        let mut motor = motor();
        motor.joint = "right_upper_arm_yaw".to_string();
        assert!(apply_limit_patch_to_motor(&mut motor, &patch).is_err());
    }
}
