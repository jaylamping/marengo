//! The motor-repl stop path runs the real binary against deliberately broken
//! configuration. None of these tests touch hardware: the default build has no
//! SocketCAN, so every drive reports a per-drive open failure, which is exactly
//! the "CAN interface down" case the stop must survive.
#![allow(clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

const MOTORS_ONLY: &str = "motors:\n  - {joint: a, can_interface: can0, device_id: 1}\n  - {joint: b, can_interface: can0, device_id: 2}\n  - {joint: c, can_interface: can1, device_id: 5}\n";

fn repl(config_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_motor-repl"));
    command
        .env("MARENGO_ROOT", config_dir)
        .env("MARENGO_CONFIG_DIR", config_dir)
        .env_remove("MARENGO_CAN_INTERFACE")
        .env_remove("RUST_LOG");
    command
}

#[test]
fn disable_needs_only_motors_yaml_and_reports_every_drive() {
    // No control.yaml, robot.yaml, homing.yaml, URDF or calibration history.
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("motors.yaml"), MOTORS_ONLY).expect("write motors.yaml");
    // A directory where the zero registry would be must not matter either.
    std::fs::create_dir(dir.path().join("zero_registry.yaml")).expect("mkdir");

    let output = repl(dir.path()).arg("disable").output().expect("run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    for address in ["can0:1", "can0:2", "can1:5"] {
        assert!(
            stdout.contains(&format!("disable {address} FAILED")),
            "{address} missing a per-drive outcome:\n{stdout}\n{stderr}"
        );
    }
    assert!(
        !output.status.success(),
        "unreached drives must exit non-zero"
    );
    assert!(
        stderr.contains("disable INCOMPLETE: 3 of 3 drives"),
        "{stderr}"
    );
    for config_failure in ["control.yaml", "control loop", "robot.yaml", "urdf"] {
        assert!(
            !stderr.to_lowercase().contains(config_failure),
            "disable depended on {config_failure}:\n{stderr}"
        );
    }
}

#[test]
fn disable_without_drive_addresses_says_no_stop_was_sent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = repl(dir.path()).arg("disable").output().expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("NO stop frame was sent"), "{stderr}");
}

#[test]
fn set_zero_refuses_to_start_without_an_armable_exit_stop() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = repl(dir.path()).arg("set-zero").output().expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("cannot arm the exit stop"), "{stderr}");
}

#[test]
fn removed_commands_are_rejected_before_owner_or_exit_stop_setup() {
    let dir = tempfile::tempdir().expect("tempdir");
    for command in [
        "home",
        "enable",
        "jog",
        "speed",
        "speed-stop",
        "gravity-on",
        "gravity-off",
        "torque-cmd",
    ] {
        let output = repl(dir.path()).arg(command).output().expect("run");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{command}: {stderr}");
        assert!(
            stderr.contains("unsupported command"),
            "{command}: {stderr}"
        );
        assert!(!stderr.contains("exit stop armed"), "{command}: {stderr}");
        assert!(!stderr.contains("control.yaml"), "{command}: {stderr}");
    }
}

#[test]
fn error_exit_of_a_drive_touching_command_runs_the_stop() {
    // motors.yaml is present, control.yaml is not: set-zero fails during setup.
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("motors.yaml"), MOTORS_ONLY).expect("write motors.yaml");
    let output = repl(dir.path())
        .args(["set-zero", "a"])
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("exit stop armed for 3 drives"), "{stderr}");
    assert!(
        stderr.contains("exit stop, disabling every drive"),
        "{stderr}"
    );
    for address in ["can0:1", "can0:2", "can1:5"] {
        assert!(
            stderr.contains(&format!("disable {address}")),
            "{address} not addressed by the exit stop:\n{stderr}"
        );
    }
}

#[test]
fn read_only_commands_do_not_arm_or_run_the_exit_stop() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("motors.yaml"), MOTORS_ONLY).expect("write motors.yaml");
    let output = repl(dir.path()).arg("status").output().expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("exit stop"), "{stderr}");
}

/// SIGTERM while the command is blocked (here: reading control.yaml from a FIFO
/// with no writer) must still disable every drive and exit 143.
#[cfg(unix)]
#[test]
fn sigterm_disables_every_drive_then_exits_143() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("motors.yaml"), MOTORS_ONLY).expect("write motors.yaml");
    let fifo = dir.path().join("control.yaml");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo")
        .success());

    let mut child = repl(dir.path())
        .args(["set-zero", "a"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stderr = BufReader::new(child.stderr.take().expect("stderr pipe"));
    let mut seen = String::new();
    // The handler is installed before the "armed" line is printed.
    loop {
        let mut line = String::new();
        let n = stderr.read_line(&mut line).expect("read stderr");
        assert!(n > 0, "child exited before arming:\n{seen}");
        seen.push_str(&line);
        if line.contains("exit stop armed") {
            break;
        }
    }
    // In-process signal: the CI dev image has no procps `kill` binary.
    let pid = i32::try_from(child.id()).expect("pid fits pid_t");
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid),
        nix::sys::signal::Signal::SIGTERM,
    )
    .expect("send SIGTERM");
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut stderr, &mut rest).expect("drain stderr");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(143), "{seen}{rest}");
    assert!(rest.contains("signal 15, disabling every drive"), "{rest}");
    for address in ["can0:1", "can0:2", "can1:5"] {
        assert!(
            rest.contains(&format!("disable {address}")),
            "{address} not addressed on SIGTERM:\n{rest}"
        );
    }
}
