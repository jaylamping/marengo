//! Scalar history validation is not a physical current-reference capability.
#![allow(clippy::expect_used)]

mod support;

use marengo_config::{
    EffectiveHomingJoint, HomingMethod, MotorBenchLimits, MotorEntry, MotorType, SearchDirection,
};
use marengo_homing::{verify_manual_reference, HomingRegistry, JointHomingState};
use support::TestDirectory;

#[derive(Clone, Copy)]
enum Input {
    Position(f64),
    Bounds(f64, f64),
    Tolerance(f64),
    Offset(f64),
    PolicyJoint,
    UnconfiguredJoint,
    Method(HomingMethod),
}

fn motor() -> MotorEntry {
    MotorEntry {
        joint: "test_joint".into(),
        driver: "robstride".into(),
        motor_type: MotorType::Rs03,
        can_interface: "can0".into(),
        device_id: 12,
        direction: 1,
        gear_ratio: 1.0,
        recv_can_id: 0,
        firmware_version: "fixture-label".into(),
        bench: MotorBenchLimits {
            position_lower_rad: -1.0,
            position_upper_rad: 1.0,
            velocity_limit_rad_s: 0.5,
            torque_limit_nm: 1.0,
        },
    }
}

fn policy() -> EffectiveHomingJoint {
    EffectiveHomingJoint {
        joint: "test_joint".into(),
        method: HomingMethod::ManualReference,
        home_offset_rad: 0.0,
        search_direction: SearchDirection::Positive,
        search_velocity_rad_s: 0.15,
        search_torque_nm: 0.5,
        search_timeout_s: 30.0,
        backoff_rad: 0.05,
        sign_test_required: true,
        allow_sensor_overlap: false,
        sensors: None,
    }
}

fn invalid_input_preserves_state_and_history(label: &str, input: Input) {
    let directory = TestDirectory::new(label);
    let path = directory.path().join("history.yaml");
    let original = b"joints: []\n# immutable manual-reference rejection fixture\n";
    std::fs::write(&path, original).expect("history fixture");
    let mut position = 0.01;
    let mut lower = -1.0;
    let mut upper = 1.0;
    let mut tolerance = 0.05;
    let mut homing = policy();
    let mut joints = vec!["test_joint".into()];
    match input {
        Input::Position(value) => position = value,
        Input::Bounds(lo, hi) => {
            lower = lo;
            upper = hi;
        }
        Input::Tolerance(value) => tolerance = value,
        Input::Offset(value) => homing.home_offset_rad = value,
        Input::PolicyJoint => homing.joint = "different_joint".into(),
        Input::UnconfiguredJoint => joints = vec!["different_joint".into()],
        Input::Method(method) => homing.method = method,
    }
    let mut registry = HomingRegistry::with_record_path(&path, joints, tolerance)
        .expect("existing history is readable");
    let result = verify_manual_reference(
        &mut registry,
        &motor(),
        &homing,
        position,
        lower,
        upper,
        true,
        "validation-fixture",
        None,
    );
    let state = registry.joint_state("test_joint");
    let history = std::fs::read(&path).expect("inspect history");
    println!(
        "{label}: result={result:?}, state={state:?}, preserved={}",
        history == original
    );
    assert!(
        result.is_err(),
        "invalid input must refuse before recording"
    );
    assert_eq!(
        state,
        JointHomingState::Unhomed,
        "invalid request is not device fault evidence"
    );
    assert_eq!(history, original, "refusal must preserve existing bytes");
    assert!(
        registry.calibration().joints.is_empty(),
        "no in-memory history mutation"
    );
    assert!(!registry.is_out_of_limits("test_joint"));
}

macro_rules! rejection_cases {
    ($( $name:ident => $input:expr ),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                invalid_input_preserves_state_and_history(stringify!($name), $input);
            }
        )+
    };
}

rejection_cases! {
    nan_position => Input::Position(f64::NAN),
    positive_infinite_position => Input::Position(f64::INFINITY),
    negative_infinite_position => Input::Position(f64::NEG_INFINITY),
    nan_lower_bound => Input::Bounds(f64::NAN, 1.0),
    positive_infinite_lower_bound => Input::Bounds(f64::INFINITY, 1.0),
    negative_infinite_lower_bound => Input::Bounds(f64::NEG_INFINITY, 1.0),
    nan_upper_bound => Input::Bounds(-1.0, f64::NAN),
    positive_infinite_upper_bound => Input::Bounds(-1.0, f64::INFINITY),
    negative_infinite_upper_bound => Input::Bounds(-1.0, f64::NEG_INFINITY),
    reversed_bounds => Input::Bounds(1.0, -1.0),
    empty_bounds => Input::Bounds(0.01, 0.01),
    nan_tolerance => Input::Tolerance(f64::NAN),
    positive_infinite_tolerance => Input::Tolerance(f64::INFINITY),
    negative_infinite_tolerance => Input::Tolerance(f64::NEG_INFINITY),
    negative_tolerance => Input::Tolerance(-0.01),
    nan_offset => Input::Offset(f64::NAN),
    positive_infinite_offset => Input::Offset(f64::INFINITY),
    negative_infinite_offset => Input::Offset(f64::NEG_INFINITY),
    mismatched_policy_joint => Input::PolicyJoint,
    unconfigured_joint => Input::UnconfiguredJoint,
    hall_method_has_no_scalar_manual_workflow => Input::Method(HomingMethod::HallThreeSensor),
    none_method_has_no_scalar_manual_workflow => Input::Method(HomingMethod::None),
}

#[test]
fn finite_manual_history_recording_is_reachable() {
    let directory = TestDirectory::new("finite-manual-validation-control");
    let path = directory.path().join("history.yaml");
    let mut registry = HomingRegistry::with_record_path(&path, vec!["test_joint".into()], 0.05)
        .expect("missing history");
    let result = verify_manual_reference(
        &mut registry,
        &motor(),
        &policy(),
        0.01,
        -1.0,
        1.0,
        true,
        "finite-control",
        None,
    )
    .expect("finite manual history recording");
    assert_eq!(result.verified_position_rad, 0.01);
    let rows = registry.calibration();
    assert_eq!(rows.joints.len(), 1);
    assert_eq!(rows.joints[0].operator, "finite-control");
    let text = std::fs::read_to_string(path).expect("persisted history");
    assert!(text.contains("finite-control"));
    // History recording reachability; Davout's independent private authority owns output.
}
