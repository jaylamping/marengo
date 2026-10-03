//! Legacy calibration history is never read: construction succeeds with
//! corrupt, directory-shaped or missing history, starts Unreferenced, and
//! creates no history resource. All output is recorded locally; no hardware
//! transport is opened.
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use davout::{JointHomingState, OperationalMode, Supervisor};
use robstride::MemoryBus;
use support::TestDirectory;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_config_fixture(destination: &Path) {
    let source = repo_root();
    fs::create_dir_all(destination.join("config")).expect("fixture config directory");
    for filename in ["robot.yaml", "motors.yaml", "control.yaml"] {
        fs::copy(
            source.join("config").join(filename),
            destination.join("config").join(filename),
        )
        .expect("copy fixture policy");
    }
    // Point the reserved history location inside the fixture so the test never
    // touches the absolute bench path from master homing.yaml.
    let homing = fs::read_to_string(source.join("config/homing.yaml")).expect("homing source");
    let homing = homing
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("calibration_record_path:") {
                "  calibration_record_path: configured-history.yaml"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(destination.join("config/homing.yaml"), homing).expect("fixture homing");
    fs::create_dir_all(destination.join("assets/urdf")).expect("fixture model directory");
    fs::copy(
        source.join("assets/urdf/marengo.urdf"),
        destination.join("assets/urdf/marengo.urdf"),
    )
    .expect("fixture model");
}

fn reserved_history_path(fixture_root: &Path) -> PathBuf {
    fixture_root.join("configured-history.yaml")
}

fn assert_unhomed<B: robstride::MotorBus>(supervisor: &Supervisor<B>) {
    for motor in &supervisor.motors.motors {
        assert_eq!(
            supervisor.joint_homing_state(&motor.joint),
            JointHomingState::Unhomed,
            "no history row can grant current reference for {}",
            motor.joint,
        );
    }
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
}

#[test]
fn corrupt_or_directory_history_is_ignored_at_startup() {
    let directory = TestDirectory::new("davout-ignored-history-resource");
    let fixture_root = directory.path().join("repo");
    copy_config_fixture(&fixture_root);
    let corrupt_path = reserved_history_path(&fixture_root);
    fs::create_dir_all(corrupt_path.parent().expect("history parent"))
        .expect("history parent directory");
    fs::write(&corrupt_path, b"joints: [\n").expect("malformed fixture");
    let directory_path = directory.path().join("directory.yaml");
    fs::create_dir(&directory_path).expect("directory resource fixture");

    let supervisor = Supervisor::from_repo(&fixture_root, MemoryBus::default())
        .expect("legacy history must not refuse supervisor construction");
    assert_unhomed(&supervisor);
    // The malformed bytes are preserved byte-for-byte: nothing parses them and
    // nothing overwrites them.
    assert_eq!(
        fs::read(&corrupt_path).expect("original bytes"),
        b"joints: [\n"
    );
    assert!(directory_path.is_dir());
}

#[test]
fn missing_history_is_empty_and_does_not_create_a_resource() {
    let directory = TestDirectory::new("davout-missing-history");
    let fixture_root = directory.path().join("repo");
    copy_config_fixture(&fixture_root);
    let supervisor = Supervisor::from_repo(&fixture_root, MemoryBus::default())
        .expect("missing history is ordinary empty history");
    assert_unhomed(&supervisor);
    assert!(!reserved_history_path(&fixture_root).exists());
}

#[test]
fn legacy_rows_never_grant_reference_even_when_well_formed() {
    let directory = TestDirectory::new("davout-legacy-rows");
    let fixture_root = directory.path().join("repo");
    copy_config_fixture(&fixture_root);
    let history_path = reserved_history_path(&fixture_root);
    fs::create_dir_all(history_path.parent().expect("history parent"))
        .expect("history parent directory");
    fs::write(
        &history_path,
        "joints:\n  - joint: right_elbow_pitch\n    device_id: 4\n    can_interface: can0\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-30T12:00:00Z'\n    operator: historical_operator\n",
    )
    .expect("well-formed legacy fixture");
    let mut supervisor = Supervisor::from_repo(&fixture_root, MemoryBus::default())
        .expect("legacy rows must not refuse supervisor construction");
    assert_unhomed(&supervisor);
    assert!(supervisor.set_homing_complete().is_err());
}
