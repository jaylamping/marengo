use marengo_config::{EffectiveHomingJoint, HomingMethod, MotorEntry};
use thiserror::Error;

use crate::registry::HomingRegistry;

#[derive(Debug, Error, PartialEq)]
pub enum VerifyError {
    #[error("joint {joint}: position {position_rad} outside limits [{lower}, {upper}]")]
    OutOfLimits {
        joint: String,
        position_rad: f64,
        lower: f64,
        upper: f64,
    },
    #[error("joint {joint}: |position| {position_rad} exceeds tolerance {tolerance_rad}")]
    ZeroTolerance {
        joint: String,
        position_rad: f64,
        tolerance_rad: f64,
    },
    #[error("joint {joint}: sign test required but not recorded")]
    SignTestRequired { joint: String },
    #[error("registry: {0}")]
    Registry(#[from] crate::registry::RegistryError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerifyOutcome {
    pub joint: String,
    pub verified_position_rad: f64,
    pub within_tolerance: bool,
}

/// Verify manual-reference zero: position within limits and near zero after SetZero.
#[allow(clippy::too_many_arguments)]
pub fn verify_manual_reference(
    registry: &mut HomingRegistry,
    motor: &MotorEntry,
    homing: &EffectiveHomingJoint,
    position_rad: f64,
    lower: f64,
    upper: f64,
    sign_test_passed: bool,
    operator: &str,
    config_revision: Option<String>,
) -> Result<VerifyOutcome, VerifyError> {
    if position_rad < lower || position_rad > upper {
        registry.set_state(&motor.joint, crate::JointHomingState::Faulted);
        registry.mark_out_of_limits(&motor.joint);
        return Err(VerifyError::OutOfLimits {
            joint: motor.joint.clone(),
            position_rad,
            lower,
            upper,
        });
    }
    let tolerance = registry.zero_tolerance_rad();
    if position_rad.abs() > tolerance {
        registry.set_state(&motor.joint, crate::JointHomingState::Faulted);
        return Err(VerifyError::ZeroTolerance {
            joint: motor.joint.clone(),
            position_rad,
            tolerance_rad: tolerance,
        });
    }
    if homing.sign_test_required && !sign_test_passed {
        registry.set_state(&motor.joint, crate::JointHomingState::Faulted);
        return Err(VerifyError::SignTestRequired {
            joint: motor.joint.clone(),
        });
    }
    let method = match homing.method {
        HomingMethod::ManualReference => "manual_reference",
        HomingMethod::HallThreeSensor => "hall_three_sensor",
        HomingMethod::None => "none",
    };
    registry
        .record_verification(
            motor,
            method,
            homing.home_offset_rad,
            position_rad,
            sign_test_passed,
            operator,
            config_revision,
        )
        .map_err(VerifyError::Registry)?;
    Ok(VerifyOutcome {
        joint: motor.joint.clone(),
        verified_position_rad: position_rad,
        within_tolerance: true,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::env;

    use marengo_config::{HomingMethod, MotorType, SearchDirection};

    use super::*;
    use crate::registry::HomingRegistry;

    fn registry() -> HomingRegistry {
        let root = env::temp_dir();
        let rel = format!(
            "marengo-verify-test-{}/zero_registry.yaml",
            std::process::id()
        );
        HomingRegistry::new(&root, &rel, vec!["shoulder_pitch".to_string()], 0.05)
            .expect("registry")
    }

    fn motor() -> MotorEntry {
        MotorEntry {
            joint: "shoulder_pitch".to_string(),
            driver: "robstride".to_string(),
            motor_type: MotorType::Rs03,
            can_interface: "can0".to_string(),
            device_id: 12,
            direction: 1,
            gear_ratio: 1.0,
            recv_can_id: 0,
            firmware_version: "0.3.1.42".to_string(),
            bench: marengo_config::MotorBenchLimits {
                position_lower_rad: -0.9,
                position_upper_rad: 3.17,
                velocity_limit_rad_s: 0.5,
                torque_limit_nm: 5.0,
            },
        }
    }

    fn homing_cfg() -> EffectiveHomingJoint {
        EffectiveHomingJoint {
            joint: "shoulder_pitch".to_string(),
            method: HomingMethod::ManualReference,
            home_offset_rad: 0.0,
            search_direction: SearchDirection::Positive,
            search_velocity_rad_s: 0.15,
            search_torque_nm: 0.5,
            search_timeout_s: 30.0,
            backoff_rad: 0.05,
            sign_test_required: false,
            allow_sensor_overlap: false,
            sensors: None,
        }
    }

    #[test]
    fn verify_accepts_near_zero() {
        let mut reg = registry();
        let out = verify_manual_reference(
            &mut reg,
            &motor(),
            &homing_cfg(),
            0.01,
            -0.9,
            3.17,
            true,
            "test",
            None,
        )
        .expect("verify");
        assert!(out.within_tolerance);
    }

    #[test]
    fn verify_rejects_far_from_zero() {
        let mut reg = registry();
        let err = verify_manual_reference(
            &mut reg,
            &motor(),
            &homing_cfg(),
            0.2,
            -0.9,
            3.17,
            true,
            "test",
            None,
        )
        .expect_err("tolerance");
        assert!(matches!(err, VerifyError::ZeroTolerance { .. }));
        assert!(!reg.is_out_of_limits("shoulder_pitch"));
    }

    #[test]
    fn verify_limit_breach_marks_out_of_limits() {
        let mut reg = registry();
        let err = verify_manual_reference(
            &mut reg,
            &motor(),
            &homing_cfg(),
            5.0,
            -0.9,
            3.17,
            true,
            "test",
            None,
        )
        .expect_err("limits");
        assert!(matches!(err, VerifyError::OutOfLimits { .. }));
        assert!(crate::verify_error_is_out_of_limits(&err));
        assert!(reg.is_out_of_limits("shoulder_pitch"));
        assert_eq!(
            reg.joint_state("shoulder_pitch"),
            crate::JointHomingState::Faulted
        );
    }
}
