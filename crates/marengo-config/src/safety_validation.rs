//! Declarative safety policy validation, shared by loaders and update boundaries.

use std::collections::HashSet;
use std::time::Duration;

use crate::{
    motor_for_joint, motor_type_key, resolve_joint_velocity_cap, validate_control_against_limits,
    ConfigError, ControlConfigFile, HomingConfigFile, HomingMethod, JointControlEntry,
    MotorsConfigFile, RobotConfigFile,
};

fn invalid(field: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError::InvalidSafetyConfig {
        field: field.into(),
        message: message.into(),
    }
}

fn finite(field: &str, value: f64) -> Result<(), ConfigError> {
    if !value.is_finite() {
        return Err(invalid(field, "must be finite"));
    }
    Ok(())
}

fn nonnegative(field: &str, value: f64) -> Result<(), ConfigError> {
    finite(field, value)?;
    if value < 0.0 {
        return Err(invalid(field, "must be >= 0"));
    }
    Ok(())
}

fn positive(field: &str, value: f64) -> Result<(), ConfigError> {
    finite(field, value)?;
    if value <= 0.0 {
        return Err(invalid(field, "must be > 0"));
    }
    Ok(())
}

fn named(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must not be empty"));
    }
    Ok(())
}

/// Validate robot identity and finite, nonnegative bench caps.
/// Limb inventory may include joints that have not been built yet.
pub fn validate_robot_config(robot: &RobotConfigFile) -> Result<(), ConfigError> {
    let robot = &robot.robot;
    named("robot.name", &robot.name)?;
    named("robot.urdf", &robot.urdf)?;
    nonnegative(
        "robot.bench.max_joint_velocity_rad_s",
        robot.bench.max_joint_velocity_rad_s,
    )?;
    nonnegative(
        "robot.bench.max_joint_torque_nm",
        robot.bench.max_joint_torque_nm,
    )?;
    let mut joints = HashSet::new();
    for joint in &robot.joints {
        named("robot.joints", joint)?;
        if !joints.insert(joint) {
            return Err(invalid("robot.joints", format!("duplicate joint {joint}")));
        }
    }
    Ok(())
}

/// Validate unique motor identity, transforms, and hard bench envelopes.
pub fn validate_motors_config(motors: &MotorsConfigFile) -> Result<(), ConfigError> {
    let mut joints = HashSet::new();
    let mut addresses = HashSet::new();
    for motor in &motors.motors {
        named("motors.joint", &motor.joint)?;
        named("motors.can_interface", &motor.can_interface)?;
        if !joints.insert(&motor.joint) {
            return Err(invalid(
                "motors.joint",
                format!("duplicate joint {}", motor.joint),
            ));
        }
        if !addresses.insert((&motor.can_interface, motor.device_id)) {
            return Err(ConfigError::DuplicateMotorAddress {
                interface: motor.can_interface.clone(),
                device_id: motor.device_id,
            });
        }
        let field = format!("motors.{}", motor.joint);
        if !matches!(motor.direction, -1 | 1) {
            return Err(invalid(format!("{field}.direction"), "must be -1 or +1"));
        }
        positive(&format!("{field}.gear_ratio"), motor.gear_ratio)?;
        finite(
            &format!("{field}.bench.position_lower_rad"),
            motor.bench.position_lower_rad,
        )?;
        finite(
            &format!("{field}.bench.position_upper_rad"),
            motor.bench.position_upper_rad,
        )?;
        if motor.bench.position_lower_rad >= motor.bench.position_upper_rad {
            return Err(invalid(
                format!("{field}.bench"),
                "hard lower must be < hard upper",
            ));
        }
        nonnegative(
            &format!("{field}.bench.velocity_limit_rad_s"),
            motor.bench.velocity_limit_rad_s,
        )?;
        nonnegative(
            &format!("{field}.bench.torque_limit_nm"),
            motor.bench.torque_limit_nm,
        )?;
    }
    Ok(())
}

pub(crate) fn validate_joint_numbers(
    joint: &str,
    entry: &JointControlEntry,
) -> Result<(), ConfigError> {
    let field = format!("control.joints.{joint}");
    named("control.joints", joint)?;
    for (name, value) in [
        ("position_slew_rad_s", entry.position_slew_rad_s),
        (
            "position_slew_max_lead_rad",
            entry.position_slew_max_lead_rad,
        ),
        (
            "position_trajectory_threshold_rad",
            entry.position_trajectory_threshold_rad,
        ),
        (
            "position_trajectory_velocity_deadband_rad",
            entry.position_trajectory_velocity_deadband_rad,
        ),
    ] {
        nonnegative(&format!("{field}.{name}"), value)?;
    }
    positive(
        &format!("{field}.position_trajectory_velocity_rad_s"),
        entry.position_trajectory_velocity_rad_s,
    )?;
    positive(
        &format!("{field}.position_trajectory_accel_rad_s2"),
        entry.position_trajectory_accel_rad_s2,
    )?;
    finite(
        &format!("{field}.position_hold_trim_rad"),
        entry.position_hold_trim_rad,
    )?;
    Ok(())
}

pub(crate) fn validate_control_numbers(cfg: &ControlConfigFile) -> Result<(), ConfigError> {
    let control = &cfg.control;
    // Feedback budgets use whole microseconds; a higher rate would yield a zero-length tick.
    if control.loop_hz == 0 || control.loop_hz > 1_000_000 {
        return Err(invalid(
            "control.loop_hz",
            "must be between 1 and 1000000 Hz",
        ));
    }
    if control.chappe_state_hz == 0 || control.chappe_state_hz > control.loop_hz {
        return Err(invalid(
            "control.chappe_state_hz",
            "must be positive and <= loop_hz",
        ));
    }
    if control.comm_watchdog_ms == 0 {
        return Err(invalid("control.comm_watchdog_ms", "must be > 0"));
    }
    if control.feedback_poll_budget_us == 0
        || control.feedback_poll_budget_us > 1_000_000 / u64::from(control.loop_hz)
    {
        return Err(invalid(
            "control.feedback_poll_budget_us",
            "must be positive and fit within one control tick",
        ));
    }
    if control.feedback_drain_quiet_us == 0
        || control.feedback_drain_quiet_us > control.feedback_poll_budget_us
    {
        return Err(invalid(
            "control.feedback_drain_quiet_us",
            "must be positive and <= feedback_poll_budget_us",
        ));
    }
    nonnegative(
        "control.tau_ff_rate_limit_nm_per_s",
        control.tau_ff_rate_limit_nm_per_s,
    )?;
    for (motor_type, defaults) in &control.motor_type_defaults {
        if !matches!(motor_type.as_str(), "rs00" | "rs02" | "rs03" | "rs04") {
            return Err(invalid(
                "control.motor_type_defaults",
                format!("unsupported motor type {motor_type}"),
            ));
        }
        let field = format!("control.motor_type_defaults.{motor_type}");
        for (name, value) in [
            ("kp_max", defaults.kp_max),
            ("kd_max", defaults.kd_max),
            ("tau_ff_max_nm", defaults.tau_ff_max_nm),
        ] {
            nonnegative(&format!("{field}.{name}"), value)?;
        }
        positive(
            &format!("{field}.velocity_max_rad_s"),
            defaults.velocity_max_rad_s,
        )?;
    }
    for (joint, entry) in &control.joints {
        validate_joint_numbers(joint, entry)?;
        crate::validate_joint_gains_against_motor_type(cfg, joint)?;
    }
    let mut names = HashSet::new();
    for rule in &control.danger_zones {
        named("control.danger_zones.name", &rule.name)?;
        if !names.insert(&rule.name) {
            return Err(invalid(
                "control.danger_zones.name",
                format!("duplicate rule {}", rule.name),
            ));
        }
        let field = format!("control.danger_zones.{}", rule.name);
        if !control.joints.contains_key(&rule.joint) {
            return Err(invalid(
                format!("{field}.joint"),
                "must name a control joint",
            ));
        }
        if !matches!(rule.action.as_str(), "clamp_velocity" | "clamp_torque") {
            return Err(invalid(
                format!("{field}.action"),
                "must be clamp_velocity or clamp_torque",
            ));
        }
        if rule.action == "clamp_torque" && rule.max_torque_nm.is_none() {
            return Err(invalid(
                format!("{field}.max_torque_nm"),
                "clamp_torque requires an explicit torque cap",
            ));
        }
        finite(
            &format!("{field}.position_above_rad"),
            rule.position_above_rad,
        )?;
        finite(
            &format!("{field}.velocity_below_rad_s"),
            rule.velocity_below_rad_s,
        )?;
        nonnegative(
            &format!("{field}.max_velocity_rad_s"),
            rule.max_velocity_rad_s,
        )?;
        if let Some(cap) = rule.max_torque_nm {
            nonnegative(&format!("{field}.max_torque_nm"), cap)?;
        }
    }
    let sign = &control.wrong_sign_watchdog;
    if !matches!(sign.expected_sign_at_positive_q, -1 | 1) {
        return Err(invalid(
            "control.wrong_sign_watchdog.expected_sign_at_positive_q",
            "must be -1 or +1",
        ));
    }
    nonnegative(
        "control.wrong_sign_watchdog.min_velocity_rad_s",
        sign.min_velocity_rad_s,
    )?;
    if sign.min_opposition_ticks == 0 {
        return Err(invalid(
            "control.wrong_sign_watchdog.min_opposition_ticks",
            "must be > 0",
        ));
    }
    Ok(())
}

fn validate_homing_numbers(
    field: &str,
    offset: f64,
    velocity: f64,
    torque: f64,
    timeout: f64,
    backoff: f64,
) -> Result<(), ConfigError> {
    finite(&format!("{field}.home_offset_rad"), offset)?;
    positive(&format!("{field}.search_velocity_rad_s"), velocity)?;
    nonnegative(&format!("{field}.search_torque_nm"), torque)?;
    positive(&format!("{field}.search_timeout_s"), timeout)?;
    if Duration::try_from_secs_f64(timeout).is_err() {
        return Err(invalid(
            format!("{field}.search_timeout_s"),
            "must fit in a Duration",
        ));
    }
    nonnegative(&format!("{field}.backoff_rad"), backoff)
}

/// Validate homing defaults and the effective values after per-joint overrides.
pub fn validate_homing_config(cfg: &HomingConfigFile) -> Result<(), ConfigError> {
    let homing = &cfg.homing;
    nonnegative(
        "homing.zero_verify_tolerance_rad",
        homing.zero_verify_tolerance_rad,
    )?;
    let defaults = &homing.defaults;
    validate_homing_numbers(
        "homing.defaults",
        defaults.home_offset_rad,
        defaults.search_velocity_rad_s,
        defaults.search_torque_nm,
        defaults.search_timeout_s,
        defaults.backoff_rad,
    )?;
    for joint in homing.configured_joints() {
        named("homing.joints", joint)?;
        let effective = homing
            .effective_joint(joint)
            .ok_or_else(|| invalid("homing.joints", format!("missing joint {joint}")))?;
        let field = format!("homing.joints.{joint}");
        validate_homing_numbers(
            &field,
            effective.home_offset_rad,
            effective.search_velocity_rad_s,
            effective.search_torque_nm,
            effective.search_timeout_s,
            effective.backoff_rad,
        )?;
        if effective.method == HomingMethod::HallThreeSensor && effective.sensors.is_none() {
            return Err(invalid(
                format!("{field}.sensors"),
                "Hall homing requires home, min_limit, and max_limit sensors",
            ));
        }
        if let Some(sensors) = &effective.sensors {
            let pins = [
                sensors.home.gpio,
                sensors.min_limit.gpio,
                sensors.max_limit.gpio,
            ];
            if pins[0] == pins[1] || pins[0] == pins[2] || pins[1] == pins[2] {
                return Err(invalid(
                    format!("{field}.sensors"),
                    "home, min_limit, and max_limit must use distinct GPIOs",
                ));
            }
        }
    }
    Ok(())
}

/// Validate the complete active declarative policy before startup or profile mutation.
///
/// Extra homing rows remain valid when commissioning narrows the active joint subset.
/// This validates borrowed raw config; it does not make subsequent mutation impossible.
pub fn validate_safety_config(
    robot: &RobotConfigFile,
    motors: &MotorsConfigFile,
    control: &ControlConfigFile,
    homing: &HomingConfigFile,
) -> Result<(), ConfigError> {
    validate_control_against_limits(robot, motors, control)?;
    validate_homing_config(homing)?;
    for joint in &robot.robot.joints {
        let effective = homing.homing.effective_joint(joint).ok_or_else(|| {
            invalid(
                "homing.joints",
                format!("active joint {joint} has no homing entry"),
            )
        })?;
        // Only sensor-search methods move autonomously using these values.
        if effective.method == HomingMethod::HallThreeSensor {
            let motor =
                motor_for_joint(motors, joint).ok_or_else(|| ConfigError::UnknownMotorJoint {
                    joint: joint.clone(),
                })?;
            let cap = resolve_joint_velocity_cap(joint, motor.motor_type, &control.control)?;
            if effective.search_velocity_rad_s > cap {
                return Err(invalid(
                    format!("homing.joints.{joint}.search_velocity_rad_s"),
                    "exceeds command velocity cap",
                ));
            }
            let defaults = control
                .control
                .motor_type_defaults
                .get(motor_type_key(motor.motor_type))
                .ok_or_else(|| {
                    invalid(
                        "control.motor_type_defaults",
                        format!("missing defaults for {joint}"),
                    )
                })?;
            let cap = robot
                .robot
                .bench
                .max_joint_torque_nm
                .min(motor.bench.torque_limit_nm)
                .min(defaults.tau_ff_max_nm);
            if effective.search_torque_nm > cap {
                return Err(invalid(
                    format!("homing.joints.{joint}.search_torque_nm"),
                    "exceeds effective torque cap",
                ));
            }
        }
    }
    Ok(())
}
