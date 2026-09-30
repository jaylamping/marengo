//! Startup history admission and calibration-path composition through public Supervisor APIs.
//! All output is recorded locally; no hardware transport is opened.
#![allow(clippy::expect_used)]

#[path = "../../marengo-homing/tests/support/mod.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use davout::{DavoutError, JointHomingState, OperationalMode, Supervisor};
use marengo_config::{load_motors_config, MotorEntry};
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorBus, ReceiveAttempt};
use support::TestDirectory;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn write_history(path: &Path, motors: &[MotorEntry], operator: &str) -> Vec<u8> {
    // Literal legacy YAML, independent of the registry's serialization implementation.
    let mut history = String::from("joints:\n");
    for motor in motors {
        history.push_str(&format!(
            "  - joint: {}\n    device_id: {}\n    can_interface: {}\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-30T12:00:00Z'\n    config_revision: '6ff2ed82191d9e602aa230cedb80ae10259cce71'\n    operator: {}\n",
            motor.joint, motor.device_id, motor.can_interface, operator,
        ));
    }
    fs::write(path, history.as_bytes()).expect("history fixture");
    history.into_bytes()
}

fn assert_unhomed<B: MotorBus>(supervisor: &Supervisor<B>) {
    for motor in &supervisor.motors.motors {
        assert_eq!(
            supervisor.joint_homing_state(&motor.joint),
            JointHomingState::Unhomed,
            "historical row cannot grant current reference for {}",
            motor.joint,
        );
    }
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
}

#[test]
fn historical_rows_cannot_authorize_checked_home_or_normal_enable() {
    let directory = TestDirectory::new("davout-history-readiness");
    let history_path = directory.path().join("history.yaml");
    let motors = load_motors_config(repo_root()).expect("motor fixture");
    let original = write_history(&history_path, &motors.motors, "historical_operator");
    let mut supervisor = Supervisor::from_repo_with_calibration_record_path(
        repo_root(),
        MemoryBus::default(),
        &history_path,
    )
    .expect("supervisor with inspectable history");

    assert_unhomed(&supervisor);
    assert_eq!(
        supervisor.homing_registry().calibration().joints.len(),
        motors.motors.len(),
    );
    assert!(matches!(
        supervisor.set_homing_complete(),
        Err(DavoutError::Homing { .. })
    ));
    assert!(matches!(
        supervisor.request_enable(true),
        Err(DavoutError::Homing { .. })
    ));
    assert_unhomed(&supervisor);
    // Master policy permits startup diagnostic reporting. Only energizing/motion
    // output is prohibited by this history admission contract.
    assert!(supervisor
        .bus_mut()
        .tx
        .iter()
        .all(|frame| !matches!(frame.id >> 24, 1 | 3)));
    assert_eq!(
        fs::read(&history_path).expect("preserved history"),
        original
    );
}

#[test]
fn missing_history_is_empty_and_does_not_create_a_resource() {
    let directory = TestDirectory::new("davout-missing-history");
    let history_path = directory.path().join("missing.yaml");
    let mut supervisor = Supervisor::from_repo_with_calibration_record_path(
        repo_root(),
        MemoryBus::default(),
        &history_path,
    )
    .expect("missing history is ordinary empty history");
    assert_unhomed(&supervisor);
    assert!(supervisor.homing_registry().calibration().joints.is_empty());
    assert!(matches!(
        supervisor.request_enable(true),
        Err(DavoutError::Homing { .. })
    ));
    assert!(!history_path.exists());
}

#[derive(Clone, Default)]
struct SharedRecordingBus {
    transmitted: Arc<Mutex<Vec<CanFrame>>>,
}

impl CanBus for SharedRecordingBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.transmitted
            .lock()
            .expect("recorded output")
            .push(frame.clone());
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        Ok(ReceiveAttempt::Idle)
    }
}

impl MotorBus for SharedRecordingBus {}

#[test]
fn corrupt_or_directory_history_fails_before_any_startup_transmission() {
    let directory = TestDirectory::new("davout-invalid-history-resource");
    let corrupt_path = directory.path().join("corrupt.yaml");
    let corrupt_bytes = b"joints: [\n";
    fs::write(&corrupt_path, corrupt_bytes).expect("malformed fixture");
    let directory_path = directory.path().join("directory.yaml");
    fs::create_dir(&directory_path).expect("directory resource fixture");

    for (path, expected_error) in [(&corrupt_path, "parse"), (&directory_path, "io")] {
        let bus = SharedRecordingBus::default();
        let witness = bus.clone();
        let result = Supervisor::from_repo_with_calibration_record_path(repo_root(), bus, path);
        let error = result
            .err()
            .expect("invalid history must refuse supervisor construction");
        assert!(matches!(error, DavoutError::Homing { .. }));
        assert!(error.to_string().contains(expected_error));
        assert!(error
            .to_string()
            .contains(path.to_str().expect("fixture path")));
        assert!(witness
            .transmitted
            .lock()
            .expect("constructor output")
            .is_empty());
    }
    assert_eq!(
        fs::read(corrupt_path).expect("original bytes"),
        corrupt_bytes
    );
    assert!(directory_path.is_dir());
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
    let homing =
        fs::read_to_string(source.join("config/homing.yaml")).expect("homing fixture source");
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
    fs::write(destination.join("config/homing.yaml"), homing).expect("fixture homing policy");
    fs::create_dir_all(destination.join("assets/urdf")).expect("fixture model directory");
    fs::copy(
        source.join("assets/urdf/marengo.urdf"),
        destination.join("assets/urdf/marengo.urdf"),
    )
    .expect("fixture model");
}

#[test]
fn environment_path_precedence_runs_in_isolated_processes() {
    if let Ok(case) = std::env::var("DAVOUT_HISTORY_PATH_CASE") {
        assert_environment_path_case(&case);
        return;
    }
    let motors = load_motors_config(repo_root()).expect("motor fixture");
    for case in [
        "explicit",
        "relative_explicit",
        "legacy",
        "relative_override",
        "configured_default",
    ] {
        let directory = TestDirectory::new(case);
        let fixture_root = directory.path().join("repo");
        copy_config_fixture(&fixture_root);
        write_history(
            &fixture_root.join("configured-history.yaml"),
            &motors.motors,
            "configured_default",
        );
        write_history(
            &directory.path().join("explicit.yaml"),
            &motors.motors,
            "explicit",
        );
        write_history(
            &directory.path().join("legacy.yaml"),
            &motors.motors,
            "legacy",
        );
        let invalid_override = directory.path().join("invalid-override.yaml");
        fs::write(&invalid_override, "joints: [\n").expect("invalid environment resource");
        let mut child = Command::new(std::env::current_exe().expect("test executable"));
        child
            .args([
                "--exact",
                "environment_path_precedence_runs_in_isolated_processes",
                "--nocapture",
            ])
            .current_dir(directory.path())
            .env("DAVOUT_HISTORY_PATH_CASE", case)
            .env("DAVOUT_HISTORY_FIXTURE_ROOT", &fixture_root)
            .env(
                "DAVOUT_HISTORY_EXPLICIT_PATH",
                directory.path().join("explicit.yaml"),
            )
            .env("MARENGO_CONFIG_DIR", fixture_root.join("config"))
            .env_remove("MARENGO_ROOT")
            .env_remove("MARENGO_JOINT_SUBSET")
            .env_remove("MARENGO_CALIBRATION_RECORD");
        match case {
            "explicit" | "relative_explicit" => {
                child.env("MARENGO_CALIBRATION_RECORD", invalid_override);
                if case == "relative_explicit" {
                    child.env("DAVOUT_HISTORY_EXPLICIT_PATH", "explicit.yaml");
                }
            }
            "legacy" => {
                child.env(
                    "MARENGO_CALIBRATION_RECORD",
                    directory.path().join("legacy.yaml"),
                );
            }
            "relative_override" => {
                child.env("MARENGO_CALIBRATION_RECORD", "legacy.yaml");
            }
            _ => {}
        }
        let output = child.output().expect("isolated environment test");
        assert!(
            output.status.success(),
            "case {case} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
    }
}

fn assert_environment_path_case(case: &str) {
    let fixture_root =
        PathBuf::from(std::env::var_os("DAVOUT_HISTORY_FIXTURE_ROOT").expect("child fixture root"));
    let supervisor = if matches!(case, "explicit" | "relative_explicit") {
        let path = std::env::var_os("DAVOUT_HISTORY_EXPLICIT_PATH").expect("explicit child path");
        Supervisor::from_repo_with_calibration_record_path(
            &fixture_root,
            MemoryBus::default(),
            PathBuf::from(path),
        )
    } else {
        Supervisor::from_repo(&fixture_root, MemoryBus::default())
    }
    .expect("selected history resource");
    assert!(matches!(
        case,
        "explicit" | "relative_explicit" | "legacy" | "relative_override" | "configured_default"
    ));
    let expected_operator = match case {
        "relative_override" => "legacy",
        "relative_explicit" => "explicit",
        _ => case,
    };
    let history = &supervisor.homing_registry().calibration().joints;
    assert!(!history.is_empty());
    assert!(history.iter().all(|row| row.operator == expected_operator));
    assert_unhomed(&supervisor);
}
