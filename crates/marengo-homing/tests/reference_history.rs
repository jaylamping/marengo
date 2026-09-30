//! Public history admission and real persistence contracts. No hardware or grants
//! from history; existing same-process verification remains a separate legacy API.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::process::Command;

use marengo_config::{MotorBenchLimits, MotorEntry, MotorType};
use marengo_homing::{HomingRegistry, JointCalibration, JointHomingState, RegistryError};

mod support;
use support::TestDirectory;

fn row() -> JointCalibration {
    JointCalibration {
        joint: "right_shoulder_pitch".into(),
        device_id: 1,
        can_interface: "can0".into(),
        method: "manual_reference".into(),
        home_offset_rad: 0.0,
        verified_position_rad: 0.0,
        sign_test_passed: true,
        timestamp_utc: "2026-09-30T12:00:00Z".into(),
        config_revision: Some("current-config".into()),
        operator: "history-fixture".into(),
    }
}

fn write_row(directory: &TestDirectory, row: &JointCalibration) -> Vec<u8> {
    let text = format!(
        "joints:\n  - joint: {}\n    device_id: {}\n    can_interface: {}\n    method: {}\n    home_offset_rad: {}\n    verified_position_rad: {}\n    sign_test_passed: {}\n    timestamp_utc: '{}'\n    config_revision: '{}'\n    operator: {}\n",
        row.joint, row.device_id, row.can_interface, row.method, row.home_offset_rad,
        row.verified_position_rad, row.sign_test_passed, row.timestamp_utc,
        row.config_revision.as_deref().expect("fixture revision"), row.operator,
    );
    fs::write(directory.path().join("history.yaml"), text.as_bytes()).expect("historical fixture");
    text.into_bytes()
}

fn assert_history_does_not_grant(row: JointCalibration) {
    let directory = TestDirectory::new("history-refusal");
    let bytes = write_row(&directory, &row);
    // Existing public constructor/path, same admission boundary as baseline reds.
    let registry = HomingRegistry::new(
        directory.path(),
        "history.yaml",
        vec![row.joint.clone()],
        0.05,
    )
    .expect("well-formed history remains available");
    assert_eq!(registry.calibration().joints, [row.clone()]);
    assert_eq!(
        fs::read(directory.path().join("history.yaml")).expect("history bytes"),
        bytes
    );
    assert_eq!(registry.joint_state(&row.joint), JointHomingState::Unhomed);
    assert!(matches!(
        registry.require_ready(),
        Err(RegistryError::Joint { .. })
    ));
}

#[test]
fn exact_history_cannot_grant_a_current_startup_reference() {
    assert_history_does_not_grant(row());
}

#[test]
fn mismatched_history_cannot_grant_a_current_startup_reference() {
    for field in ["device", "interface", "sign", "position", "revision"] {
        let mut row = row();
        match field {
            "device" => row.device_id = 2,
            "interface" => row.can_interface = "can1".into(),
            "sign" => row.sign_test_passed = false,
            "position" => row.verified_position_rad = 0.6,
            "revision" => row.config_revision = Some("old-config".into()),
            _ => unreachable!(),
        }
        assert_history_does_not_grant(row);
    }
}

#[test]
fn malformed_existing_history_is_rejected_without_changing_its_bytes() {
    let directory = TestDirectory::new("malformed-history");
    let path = directory.path().join("history.yaml");
    let bytes = b"joints: [broken legacy YAML\n";
    fs::write(&path, bytes).expect("corrupt existing resource");
    let result = HomingRegistry::new(directory.path(), "history.yaml", vec![row().joint], 0.05);
    assert_eq!(fs::read(&path).expect("original bytes"), bytes);
    assert!(matches!(result, Err(RegistryError::Parse { path: actual, .. }) if actual == path));
}

#[test]
fn missing_history_is_empty_unhomed_and_is_not_created_by_loading() {
    let directory = TestDirectory::new("missing-history");
    let path = directory.path().join("nested/history.yaml");
    let registry = HomingRegistry::new(
        directory.path(),
        "nested/history.yaml",
        vec![row().joint.clone()],
        0.05,
    )
    .expect("missing resource is ordinary empty history");
    assert!(registry.calibration().joints.is_empty());
    assert_eq!(
        registry.joint_state(&row().joint),
        JointHomingState::Unhomed
    );
    assert!(registry.require_ready().is_err());
    assert!(!path.exists());
    assert!(
        !directory.path().join("nested").exists(),
        "read-only startup creates no parent directory"
    );
}

#[test]
fn existing_directory_and_non_utf8_history_return_typed_io_without_mutation() {
    let directory = TestDirectory::new("directory-history");
    let path = directory.path().join("history.yaml");
    fs::create_dir(&path).expect("existing directory at configured resource");
    let marker = path.join("preserved.txt");
    fs::write(&marker, b"preserve directory contents").expect("marker");
    let result = HomingRegistry::with_record_path(&path, vec![row().joint.clone()], 0.05);
    assert!(matches!(result, Err(RegistryError::Io { path: actual, .. }) if actual == path));
    assert_eq!(
        fs::read(marker).expect("existing contents"),
        b"preserve directory contents"
    );

    let directory = TestDirectory::new("non-utf8-history");
    let path = directory.path().join("history.yaml");
    let bytes = [0xff, 0xfe, 0x80, 0x00];
    fs::write(&path, bytes).expect("existing non-UTF8 resource");
    let result = HomingRegistry::with_record_path(&path, vec![row().joint], 0.05);
    assert!(matches!(result, Err(RegistryError::Io { path: actual, .. }) if actual == path));
    assert_eq!(fs::read(&path).expect("preserved bytes"), bytes);
}

#[test]
fn explicit_binding_selects_history_and_leaves_every_configured_joint_unhomed() {
    let first = TestDirectory::new("first-binding");
    let second = TestDirectory::new("second-binding");
    let first_row = row();
    let mut second_row = row();
    second_row.operator = "second-resource".into();
    write_row(&first, &first_row);
    let bytes = write_row(&second, &second_row);
    let registry = HomingRegistry::with_record_path(
        second.path().join("history.yaml"),
        vec![first_row.joint.clone(), "right_shoulder_roll".into()],
        0.05,
    )
    .expect("selected resource");
    assert_eq!(registry.calibration().joints, [second_row]);
    for joint in [first_row.joint.as_str(), "right_shoulder_roll"] {
        assert_eq!(registry.joint_state(joint), JointHomingState::Unhomed);
    }
    assert!(!registry.all_verified());
    assert!(registry.require_ready().is_err());
    assert_eq!(
        fs::read(second.path().join("history.yaml")).expect("selected bytes"),
        bytes
    );
}

#[test]
fn real_persistence_survives_reconstruction_but_same_process_grant_does_not() {
    let directory = TestDirectory::new("persist-reconstruct");
    let path = directory.path().join("history.yaml");
    let motor = MotorEntry {
        joint: row().joint,
        driver: "robstride".into(),
        motor_type: MotorType::Rs03,
        can_interface: "can0".into(),
        device_id: 1,
        direction: 1,
        gear_ratio: 1.0,
        recv_can_id: 0,
        firmware_version: "fixture".into(),
        bench: MotorBenchLimits {
            position_lower_rad: -0.9,
            position_upper_rad: 3.17,
            velocity_limit_rad_s: 0.5,
            torque_limit_nm: 5.0,
        },
    };
    let mut registry = HomingRegistry::with_record_path(&path, vec![motor.joint.clone()], 0.05)
        .expect("empty registry");
    // Existing public API behavior is retained for this bounded slice. It does
    // not establish qualified physical/current reference evidence.
    registry
        .record_verification(
            &motor,
            "manual_reference",
            0.0,
            0.01,
            true,
            "legacy-test",
            None,
        )
        .expect("real file persistence");
    assert_eq!(
        registry.joint_state(&motor.joint),
        JointHomingState::Verified
    );
    registry.require_ready().expect("legacy same-process grant");
    let history = registry.calibration().clone();
    let bytes = fs::read(&path).expect("persisted file");
    drop(registry);
    let reconstructed = HomingRegistry::new(
        directory.path(),
        "history.yaml",
        vec![motor.joint.clone()],
        0.05,
    )
    .expect("retained history");
    assert_eq!(reconstructed.calibration(), &history);
    assert_eq!(
        reconstructed.joint_state(&motor.joint),
        JointHomingState::Unhomed
    );
    assert!(reconstructed.require_ready().is_err());
    assert_eq!(
        fs::read(path).expect("history unchanged after startup"),
        bytes
    );
}

#[test]
fn library_binding_ignores_legacy_environment_in_an_isolated_child() {
    if std::env::var_os("HOMING_BINDING_CHILD").is_some() {
        assert_child_binding();
        return;
    }
    let selected = TestDirectory::new("selected-path");
    let decoy = TestDirectory::new("environment-decoy");
    write_row(&selected, &row());
    // Choosing the environment decoy would fail parsing, making precedence
    // observable through construction rather than a private path getter.
    fs::write(decoy.path().join("history.yaml"), b"joints: [broken\n").expect("decoy");
    let output = Command::new(std::env::current_exe().expect("integration executable"))
        .args([
            "--exact",
            "library_binding_ignores_legacy_environment_in_an_isolated_child",
            "--nocapture",
        ])
        .env(
            "MARENGO_CALIBRATION_RECORD",
            decoy.path().join("history.yaml"),
        )
        .env("HOMING_SELECTED_ROOT", selected.path())
        .env("HOMING_BINDING_CHILD", "1")
        .env("TEMP", selected.path())
        .env("TMP", selected.path())
        .env("TMPDIR", selected.path())
        .output()
        .expect("isolated child");
    assert!(
        output.status.success(),
        "child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_child_binding() {
    let root = std::path::PathBuf::from(
        std::env::var_os("HOMING_SELECTED_ROOT").expect("child selected root"),
    );
    let via_root = HomingRegistry::new(&root, "history.yaml", vec![row().joint.clone()], 0.05)
        .expect("deterministic root binding");
    let via_path = HomingRegistry::with_record_path(
        root.join("history.yaml"),
        vec![row().joint.clone()],
        0.05,
    )
    .expect("explicit path binding");
    assert_eq!(via_root.calibration().joints, [row()]);
    assert_eq!(via_path.calibration(), via_root.calibration());
    assert_eq!(
        via_root.joint_state(&row().joint),
        JointHomingState::Unhomed
    );
    assert_eq!(
        via_path.joint_state(&row().joint),
        JointHomingState::Unhomed
    );
}
