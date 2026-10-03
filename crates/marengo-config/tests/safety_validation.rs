//! Invalid declarative policy must fail at public loaders and writers, before mutation.

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use marengo_config::{
    apply_joint_config_param, apply_joint_subset, expand_urdf_file_to_cover_motors,
    load_control_config_from, load_homing_config_from, load_motors_config_from,
    load_robot_config_from, validate_control_against_limits, validate_safety_config,
    write_control_config_from, write_motors_and_control, write_motors_control_and_urdf,
    ConfigError, HomingMethod, HomingSensors, SensorInput,
};
use serde_yaml::Value;

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("temp config");
    fs::create_dir(temp.path().join("config")).expect("config dir");
    for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
        fs::copy(
            source_root().join("config").join(name),
            temp.path().join("config").join(name),
        )
        .expect("copy config");
    }
    fs::create_dir_all(temp.path().join("assets/urdf")).expect("assets dir");
    fs::copy(
        source_root().join("assets/urdf/marengo.urdf"),
        temp.path().join("assets/urdf/marengo.urdf"),
    )
    .expect("copy urdf");
    temp
}

fn reject_yaml_cases(file: &str, cases: &[(&str, &str, &str)]) {
    let original: Value = serde_yaml::from_str(
        &fs::read_to_string(source_root().join("config").join(file)).expect("source"),
    )
    .expect("yaml");
    let temp = tempfile::tempdir().expect("temp config");
    let mut accepted = Vec::new();
    for (label, path, replacement) in cases {
        let mut document = original.clone();
        let mut cursor = &mut document;
        for component in path.split('/') {
            cursor = match component.parse::<usize>() {
                Ok(index) => &mut cursor[index],
                Err(_) => &mut cursor[component],
            };
        }
        *cursor = serde_yaml::from_str(replacement).expect("replacement yaml");
        fs::write(
            temp.path().join(file),
            serde_yaml::to_string(&document).expect("serialize"),
        )
        .expect("write case");
        let result = match file {
            "robot.yaml" => load_robot_config_from(temp.path()).map(|_| ()),
            "motors.yaml" => load_motors_config_from(temp.path()).map(|_| ()),
            "control.yaml" => load_control_config_from(temp.path()).map(|_| ()),
            "homing.yaml" => load_homing_config_from(temp.path()).map(|_| ()),
            _ => unreachable!("explicit test fixture filename"),
        };
        if result.is_ok() {
            accepted.push(*label);
        }
    }
    assert!(
        accepted.is_empty(),
        "{file} accepted unsafe policy: {accepted:?}"
    );
}

#[test]
fn robot_loader_rejects_invalid_caps_and_duplicate_identity() {
    reject_yaml_cases(
        "robot.yaml",
        &[
            (
                "NaN bench cap",
                "robot/bench/max_joint_velocity_rad_s",
                ".nan",
            ),
            (
                "negative torque cap",
                "robot/bench/max_joint_torque_nm",
                "-1",
            ),
            ("duplicate joint", "robot/joints/1", "right_shoulder_pitch"),
            ("empty joint", "robot/joints/1", "''"),
        ],
    );
}

#[test]
fn motor_loader_rejects_invalid_transform_and_limits() {
    reject_yaml_cases(
        "motors.yaml",
        &[
            ("invalid direction", "motors/0/direction", "0"),
            ("zero gearing", "motors/0/gear_ratio", "0"),
            ("negative gearing", "motors/0/gear_ratio", "-1"),
            ("NaN gearing", "motors/0/gear_ratio", ".nan"),
            (
                "inverted hard limits",
                "motors/0/bench/position_lower_rad",
                "10",
            ),
            (
                "infinite hard limit",
                "motors/0/bench/position_upper_rad",
                ".inf",
            ),
            (
                "negative velocity cap",
                "motors/0/bench/velocity_limit_rad_s",
                "-1",
            ),
            (
                "negative torque cap",
                "motors/0/bench/torque_limit_nm",
                "-1",
            ),
            (
                "duplicate joint at different address",
                "motors/1/joint",
                "right_shoulder_pitch",
            ),
            ("duplicate address", "motors/1/device_id", "1"),
        ],
    );
}

#[test]
fn control_loader_rejects_nonfinite_negative_and_inconsistent_policy() {
    reject_yaml_cases(
        "control.yaml",
        &[
            ("zero loop rate", "control/loop_hz", "0"),
            ("zero state rate", "control/chappe_state_hz", "0"),
            (
                "state faster than control",
                "control/chappe_state_hz",
                "201",
            ),
            ("zero watchdog", "control/comm_watchdog_ms", "0"),
            (
                "negative FF slew",
                "control/tau_ff_rate_limit_nm_per_s",
                "-1",
            ),
            ("NaN FF slew", "control/tau_ff_rate_limit_nm_per_s", ".nan"),
            (
                "poll longer than tick",
                "control/feedback_poll_budget_us",
                "6000",
            ),
            (
                "quiet longer than poll",
                "control/feedback_drain_quiet_us",
                "4000",
            ),
            (
                "negative max gain",
                "control/motor_type_defaults/rs03/kp_max",
                "-1",
            ),
            (
                "NaN type cap",
                "control/motor_type_defaults/rs03/velocity_max_rad_s",
                ".nan",
            ),
            (
                "negative FF cap",
                "control/motor_type_defaults/rs03/tau_ff_max_nm",
                "-1",
            ),
            (
                "NaN group cap",
                "control/actuator_groups/shoulder_pitch/velocity_max_rad_s",
                ".nan",
            ),
            (
                "negative impedance",
                "control/joints/right_shoulder_pitch/impedance/kp",
                "-1",
            ),
            (
                "nonfinite impedance",
                "control/joints/right_shoulder_pitch/impedance/kd",
                ".inf",
            ),
            (
                "over max impedance",
                "control/joints/right_shoulder_pitch/impedance/kp",
                "5001",
            ),
            (
                "over max integral gain",
                "control/joints/right_shoulder_pitch/impedance/ki",
                "5001",
            ),
            (
                "negative viscous gain",
                "control/joints/right_shoulder_pitch/friction/fv",
                "-1",
            ),
            (
                "negative friction slope",
                "control/joints/right_shoulder_pitch/friction/k",
                "-1",
            ),
            (
                "nonfinite friction bias",
                "control/joints/right_shoulder_pitch/friction/fo",
                ".nan",
            ),
            (
                "NaN envelope margin",
                "control/joints/right_shoulder_pitch/position_limit_margin_min_rad",
                ".nan",
            ),
            (
                "NaN soft limit",
                "control/joints/right_shoulder_pitch/position_soft_lower_rad",
                ".nan",
            ),
            (
                "negative position slew",
                "control/joints/right_shoulder_pitch/position_slew_rad_s",
                "-1",
            ),
            (
                "zero trajectory acceleration",
                "control/joints/right_shoulder_pitch/position_trajectory_accel_rad_s2",
                "0",
            ),
            (
                "infinite trim",
                "control/joints/right_shoulder_pitch/position_hold_trim_rad",
                ".inf",
            ),
            (
                "unknown danger action",
                "control/danger_zones/0/action",
                "silent_typo",
            ),
            (
                "unimplemented fault action",
                "control/danger_zones/0/action",
                "fault",
            ),
            (
                "torque clamp without a torque cap",
                "control/danger_zones/0/action",
                "clamp_torque",
            ),
            (
                "unknown danger joint",
                "control/danger_zones/0/joint",
                "not_a_joint",
            ),
            (
                "NaN danger threshold",
                "control/danger_zones/0/velocity_below_rad_s",
                ".nan",
            ),
            (
                "negative danger cap",
                "control/danger_zones/0/max_velocity_rad_s",
                "-1",
            ),
            (
                "invalid expected sign",
                "control/wrong_sign_watchdog/expected_sign_at_positive_q",
                "0",
            ),
            (
                "zero opposition duration",
                "control/wrong_sign_watchdog/min_opposition_ticks",
                "0",
            ),
        ],
    );
}

#[test]
fn homing_loader_rejects_invalid_defaults_and_overrides() {
    reject_yaml_cases(
        "homing.yaml",
        &[
            (
                "NaN zero tolerance",
                "homing/zero_verify_tolerance_rad",
                ".nan",
            ),
            (
                "negative zero tolerance",
                "homing/zero_verify_tolerance_rad",
                "-1",
            ),
            (
                "unbounded zero tolerance",
                "homing/zero_verify_tolerance_rad",
                "0.11",
            ),
            (
                "negative search velocity",
                "homing/defaults/search_velocity_rad_s",
                "-1",
            ),
            (
                "negative search torque",
                "homing/defaults/search_torque_nm",
                "-1",
            ),
            ("zero timeout", "homing/defaults/search_timeout_s", "0"),
            (
                "unbounded timeout",
                "homing/defaults/search_timeout_s",
                "301",
            ),
            (
                "infinite timeout",
                "homing/defaults/search_timeout_s",
                ".inf",
            ),
            ("NaN offset", "homing/defaults/home_offset_rad", ".nan"),
            ("negative backoff", "homing/defaults/backoff_rad", "-1"),
            (
                "negative override",
                "homing/joints/right_elbow_pitch/search_velocity_rad_s",
                "-1",
            ),
        ],
    );
}

#[test]
fn all_master_configs_load_and_unknown_keys_are_rejected() {
    let dir = source_root().join("config");
    load_robot_config_from(&dir).expect("master robot config");
    load_motors_config_from(&dir).expect("master motors config");
    load_control_config_from(&dir).expect("master control config");
    load_homing_config_from(&dir).expect("master homing config");

    reject_yaml_cases(
        "robot.yaml",
        &[("unknown robot key", "robot/bench/typo_cap", "1")],
    );
    reject_yaml_cases(
        "motors.yaml",
        &[("unknown motor key", "motors/0/bench/typo_cap", "1")],
    );
    reject_yaml_cases(
        "control.yaml",
        &[(
            "unknown control key",
            "control/joints/right_shoulder_pitch/typo_gain",
            "1",
        )],
    );
    reject_yaml_cases(
        "homing.yaml",
        &[("unknown homing key", "homing/defaults/typo_timeout", "1")],
    );
}

#[test]
fn cross_config_validator_rejects_motor_type_disagreement_and_excluding_soft_bounds() {
    let dir = source_root().join("config");
    let robot = load_robot_config_from(&dir).expect("robot");
    let motors = load_motors_config_from(&dir).expect("motors");
    let original = load_control_config_from(&dir).expect("control");
    let mut wrong_type = original.clone();
    wrong_type
        .control
        .joints
        .get_mut("right_shoulder_pitch")
        .expect("joint")
        .motor_type = marengo_config::MotorType::Rs02;
    assert!(
        matches!(validate_control_against_limits(&robot, &motors, &wrong_type), Err(ConfigError::InvalidSafetyConfig { field, .. }) if field == "control.joints.right_shoulder_pitch.motor_type")
    );
    let mut outside = original;
    outside
        .control
        .joints
        .get_mut("right_shoulder_pitch")
        .expect("joint")
        .position_soft_lower_rad = Some(-50.0);
    assert!(
        matches!(validate_control_against_limits(&robot, &motors, &outside), Err(ConfigError::InvalidLimitMargin { joint, .. }) if joint == "right_shoulder_pitch")
    );
}

#[test]
fn config_writers_reject_invalid_candidate_without_touching_any_artifact() {
    for writer in ["control", "profile", "urdf-and-profile"] {
        let temp = fixture();
        let dir = temp.path().join("config");
        let mut motors = load_motors_config_from(&dir).expect("motors");
        // A valid envelope expansion must not happen before control policy rejection.
        motors.motors[0].bench.position_lower_rad = -20.0;
        motors.motors[0].bench.position_upper_rad = 20.0;
        let mut control = load_control_config_from(&dir).expect("control");
        control.control.tau_ff_rate_limit_nm_per_s = -1.0;
        let files = [
            "config/robot.yaml",
            "config/motors.yaml",
            "config/control.yaml",
            "config/homing.yaml",
            "assets/urdf/marengo.urdf",
        ];
        let before: Vec<_> = files
            .iter()
            .map(|path| fs::read(temp.path().join(path)).expect("before"))
            .collect();
        let result = match writer {
            "control" => write_control_config_from(&dir, &control),
            "profile" => write_motors_and_control(&dir, &motors, &control),
            _ => write_motors_control_and_urdf(temp.path(), &dir, &motors, &control),
        };
        assert!(result.is_err(), "{writer} committed a negative torque slew");
        for (path, bytes) in files.iter().zip(before) {
            assert_eq!(
                fs::read(temp.path().join(path)).expect("after"),
                bytes,
                "{writer} changed {path}"
            );
        }
        assert!(!dir.join("control.yaml.tmp").exists());
    }
}

#[test]
fn config_parameter_rejection_is_atomic_and_signed_bias_remains_supported() {
    let cfg = load_control_config_from(source_root().join("config")).expect("control");
    let original = &cfg.control.joints["right_elbow_pitch"];
    for param in [
        "impedance.kp",
        "impedance.kd",
        "impedance.ki",
        "friction.fc",
        "friction.fv",
        "friction.k",
    ] {
        let mut entry = original.clone();
        let before = serde_yaml::to_string(&entry).expect("before");
        assert!(
            apply_joint_config_param(&mut entry, param, -1.0).is_err(),
            "accepted {param}"
        );
        assert_eq!(serde_yaml::to_string(&entry).expect("after"), before);
    }
    let mut entry = original.clone();
    assert!(apply_joint_config_param(&mut entry, "friction.fo", -0.05).is_ok());
}

#[test]
fn master_and_narrowed_policy_accept_signed_offsets_and_zero_nonnegative_caps() {
    let dir = source_root().join("config");
    let mut robot = load_robot_config_from(&dir).expect("robot");
    let mut motors = load_motors_config_from(&dir).expect("motors");
    let mut control = load_control_config_from(&dir).expect("control");
    let mut homing = load_homing_config_from(&dir).expect("homing");
    validate_safety_config(&robot, &motors, &control, &homing).expect("master policy");
    let joint = "right_elbow_pitch";
    let entry = control.control.joints.get_mut(joint).expect("joint");
    entry.friction.fo = -0.05;
    entry.position_hold_trim_rad = -0.02;
    entry.position_trajectory_threshold_rad = 0.0;
    robot.robot.bench.max_joint_torque_nm = 0.0;
    control.control.tau_ff_rate_limit_nm_per_s = 0.0;
    homing
        .homing
        .joints
        .get_mut(joint)
        .expect("homing joint")
        .overrides
        .home_offset_rad = Some(-0.02);
    let subset = [joint.to_string()].into_iter().collect();
    apply_joint_subset(&mut robot, &mut motors, &mut control, &subset).expect("subset");
    // Homing still contains inactive rows, and limb inventory still contains unbuilt joints.
    validate_safety_config(&robot, &motors, &control, &homing).expect("narrowed policy");
}

#[test]
fn aggregate_policy_requires_motor_control_and_homing_for_each_active_joint() {
    let dir = source_root().join("config");
    let robot = load_robot_config_from(&dir).expect("robot");
    let motors = load_motors_config_from(&dir).expect("motors");
    let control = load_control_config_from(&dir).expect("control");
    let homing = load_homing_config_from(&dir).expect("homing");
    let joint = "right_elbow_pitch";
    let mut missing_motor = motors.clone();
    missing_motor.motors.retain(|entry| entry.joint != joint);
    assert!(
        matches!(validate_safety_config(&robot, &missing_motor, &control, &homing), Err(ConfigError::UnknownMotorJoint { joint: name }) if name == joint)
    );
    let mut missing_control = control.clone();
    missing_control.control.joints.remove(joint);
    missing_control.control.actuator_groups.remove("elbow");
    assert!(
        matches!(validate_safety_config(&robot, &motors, &missing_control, &homing), Err(ConfigError::MissingControlJoint { joint: name }) if name == joint)
    );
    let mut missing_homing = homing;
    missing_homing.homing.joints.remove(joint);
    assert!(
        matches!(validate_safety_config(&robot, &motors, &control, &missing_homing), Err(ConfigError::InvalidSafetyConfig { field, .. }) if field == "homing.joints")
    );
}

#[test]
fn hall_search_requires_sensor_identity_and_respects_effective_motion_caps() {
    let dir = source_root().join("config");
    let robot = load_robot_config_from(&dir).expect("robot");
    let mut motors = load_motors_config_from(&dir).expect("motors");
    let mut control = load_control_config_from(&dir).expect("control");
    let mut homing = load_homing_config_from(&dir).expect("homing");
    let joint = "right_shoulder_pitch";
    control
        .control
        .joints
        .get_mut(joint)
        .expect("joint")
        .velocity_max_rad_s = Some(1.5);
    motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == joint)
        .expect("motor")
        .bench
        .torque_limit_nm = 0.25;
    let entry = homing.homing.joints.get_mut(joint).expect("homing joint");
    entry.method = HomingMethod::HallThreeSensor;
    entry.overrides.search_velocity_rad_s = Some(0.1);
    entry.overrides.search_torque_nm = Some(0.1);
    assert!(
        matches!(validate_safety_config(&robot, &motors, &control, &homing), Err(ConfigError::InvalidSafetyConfig { field, .. }) if field.ends_with(".sensors"))
    );
    homing
        .homing
        .joints
        .get_mut(joint)
        .expect("homing joint")
        .overrides
        .sensors = Some(HomingSensors {
        home: SensorInput {
            gpio: 1,
            active_high: true,
        },
        min_limit: SensorInput {
            gpio: 2,
            active_high: true,
        },
        max_limit: SensorInput {
            gpio: 3,
            active_high: true,
        },
    });
    validate_safety_config(&robot, &motors, &control, &homing).expect("bounded Hall policy");
    let mut excessive_velocity = homing.clone();
    excessive_velocity
        .homing
        .joints
        .get_mut(joint)
        .expect("homing joint")
        .overrides
        .search_velocity_rad_s = Some(2.0);
    assert!(
        matches!(validate_safety_config(&robot, &motors, &control, &excessive_velocity), Err(ConfigError::InvalidSafetyConfig { field, .. }) if field.ends_with(".search_velocity_rad_s"))
    );
    let mut excessive_torque = homing.clone();
    excessive_torque
        .homing
        .joints
        .get_mut(joint)
        .expect("homing joint")
        .overrides
        .search_torque_nm = Some(0.5);
    assert!(
        matches!(validate_safety_config(&robot, &motors, &control, &excessive_torque), Err(ConfigError::InvalidSafetyConfig { field, .. }) if field.ends_with(".search_torque_nm"))
    );
    let mut duplicate_pin = homing;
    duplicate_pin
        .homing
        .joints
        .get_mut(joint)
        .expect("homing joint")
        .overrides
        .sensors
        .as_mut()
        .expect("sensors")
        .min_limit
        .gpio = 1;
    assert!(
        matches!(validate_safety_config(&robot, &motors, &control, &duplicate_pin), Err(ConfigError::InvalidSafetyConfig { field, .. }) if field.ends_with(".sensors"))
    );
}

#[test]
fn urdf_expansion_rejects_invalid_motor_transform_before_rewrite() {
    let temp = fixture();
    let mut motors = load_motors_config_from(temp.path().join("config")).expect("motors");
    motors.motors[0].bench.position_lower_rad = -20.0;
    motors.motors[0].bench.position_upper_rad = 20.0;
    motors.motors[0].gear_ratio = 0.0;
    let path = temp.path().join("assets/urdf/marengo.urdf");
    let before = fs::read(&path).expect("before");
    assert!(expand_urdf_file_to_cover_motors(&path, &motors).is_err());
    assert_eq!(fs::read(&path).expect("after"), before);
    assert!(!path.with_extension("urdf.tmp").exists());
}
