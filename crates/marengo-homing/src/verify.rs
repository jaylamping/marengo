use marengo_config::{EffectiveHomingJoint, HomingMethod, MotorEntry};
use thiserror::Error;

use crate::registry::HomingRegistry;

#[derive(Debug, Error, PartialEq)]
pub enum VerifyError {
    #[error("joint {joint}: invalid manual-reference input {field}")]
    InvalidInput { joint: String, field: &'static str },
    #[error("joint {joint}: method {method:?} has no scalar manual-reference workflow")]
    UnsupportedMethod { joint: String, method: HomingMethod },
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

/// Validate supplied manual-reference scalars and record legacy history.
///
/// This does not establish sample freshness, device identity or SetZero causality,
/// and cannot mint Davout's private current-reference permission. Invalid inputs
/// and unsupported methods return before history or registry-state mutation.
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
    let invalid = |field| VerifyError::InvalidInput {
        joint: motor.joint.clone(),
        field,
    };
    if homing.joint != motor.joint {
        return Err(invalid("homing_joint"));
    }
    if !registry.configured_joints().contains(&motor.joint) {
        return Err(invalid("registry_joint"));
    }
    let tolerance = registry.zero_tolerance_rad();
    for (field, value) in [
        ("position_rad", position_rad),
        ("lower", lower),
        ("upper", upper),
        ("zero_tolerance_rad", tolerance),
        ("home_offset_rad", homing.home_offset_rad),
    ] {
        if !value.is_finite() {
            return Err(invalid(field));
        }
    }
    if lower >= upper {
        return Err(invalid("bounds"));
    }
    if tolerance < 0.0 {
        return Err(invalid("zero_tolerance_rad"));
    }
    if homing.method != HomingMethod::ManualReference {
        return Err(VerifyError::UnsupportedMethod {
            joint: motor.joint.clone(),
            method: homing.method,
        });
    }
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
    registry
        .record_verification(
            motor,
            "manual_reference",
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

    use marengo_config::{HomingMethod, MotorType, SearchDirection};

    use super::*;
    use crate::registry::HomingRegistry;
    use crate::test_support::TestDirectory;

    fn registry() -> (TestDirectory, HomingRegistry) {
        let directory = TestDirectory::new("manual-verifier");
        let registry = HomingRegistry::with_record_path(
            directory.path().join("zero_registry.yaml"),
            vec!["shoulder_pitch".to_string()],
            0.05,
        )
        .expect("registry");
        (directory, registry)
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
        let (_directory, mut reg) = registry();
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
        let (_directory, mut reg) = registry();
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
        let (_directory, mut reg) = registry();
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

    #[test]
    fn nonfinite_input_is_a_typed_request_error_without_recording() {
        let (directory, mut reg) = registry();
        let error = verify_manual_reference(
            &mut reg,
            &motor(),
            &homing_cfg(),
            f64::NAN,
            -0.9,
            3.17,
            true,
            "test",
            None,
        )
        .expect_err("nonfinite input");
        assert_eq!(
            error,
            VerifyError::InvalidInput {
                joint: "shoulder_pitch".into(),
                field: "position_rad",
            }
        );
        assert_eq!(
            reg.joint_state("shoulder_pitch"),
            crate::JointHomingState::Unhomed
        );
        assert!(reg.calibration().joints.is_empty());
        assert!(!directory.path().join("zero_registry.yaml").exists());
    }

    #[test]
    fn nonmanual_methods_have_typed_unsupported_results() {
        for method in [HomingMethod::HallThreeSensor, HomingMethod::None] {
            let (directory, mut reg) = registry();
            let mut homing = homing_cfg();
            homing.method = method;
            let error = verify_manual_reference(
                &mut reg,
                &motor(),
                &homing,
                0.01,
                -0.9,
                3.17,
                true,
                "test",
                None,
            )
            .expect_err("not a manual workflow");
            assert_eq!(
                error,
                VerifyError::UnsupportedMethod {
                    joint: "shoulder_pitch".into(),
                    method,
                }
            );
            assert_eq!(
                reg.joint_state("shoulder_pitch"),
                crate::JointHomingState::Unhomed
            );
            assert!(reg.calibration().joints.is_empty());
            assert!(!directory.path().join("zero_registry.yaml").exists());
        }
    }
}
