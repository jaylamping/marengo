//! One-shot motor CLI for bench inspection, independent stop, and physical zeroing.
//!
//! `disable` and the exit stop are deliberately independent of Davout: see
//! [`stop`].

mod stop;

use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use armee_dynamics::{gravity_model_from_urdf, DynamicsModel};
use berthier::ControlLoop;
use marengo_config::{
    load_control_config, load_motor_stop_targets, load_motors_config, load_robot_config,
    resolve_config_dir, resolve_reference_journal_path, resolve_repo_root, resolve_urdf_path,
};
use robstride::RuntimeBus;
use tracing::info;

fn repo_root() -> PathBuf {
    resolve_repo_root()
}

fn usage() {
    eprintln!(
        "motor-repl — one-shot bench motor CLI (Davout → robstride)\n\
         Usage:\n  \
         motor-repl [--config-dir PATH] [--can-interface can0] status\n  \
           motor-repl disable\n  \
           motor-repl set-zero <joint> [--sign-tested]\n  \
           motor-repl protocol-inspect [joint...]  (all joints when none listed)\n  \
           motor-repl gravity-preview [q...]  (robot.yaml joint order)\n\
         Reference grants are process-local; reference and enable in one long-running marengo-pi process.\n\
         set-zero runs the qualified physical reference workflow (ADR 0036); its current grant\n\
         ends with this process.\n\
         disable reads only drive addresses from motors.yaml and sends one Disable to each.\n\
         protocol-inspect (ADR 0037) stops every drive, then reads firmware version, MCU id and\n\
         registers (incl. 0x7028 CAN timeout) one query at a time; it never enables or writes.\n\
         set-zero arms an independent exit stop on SIGTERM/SIGINT/SIGHUP and error exit.\n\
         Uses SocketCAN; prefer test harness or simulation before live CAN.\n\
         Env: MARENGO_ROOT, MARENGO_CONFIG_DIR (e.g. config/bringup/shoulder_pitch_dual)"
    );
}

#[derive(Debug, PartialEq, Eq)]
struct BusArgs {
    can_interface: Option<String>,
    config_dir: Option<PathBuf>,
    command_args: Vec<String>,
}

fn parse_bus_args(args: &[String]) -> Result<BusArgs, String> {
    let Some(program) = args.first() else {
        return Err("missing program name".into());
    };
    let mut parsed = BusArgs {
        can_interface: env::var("MARENGO_CAN_INTERFACE").ok(),
        config_dir: env::var("MARENGO_CONFIG_DIR").ok().map(PathBuf::from),
        command_args: vec![program.clone()],
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--can-interface" => {
                let value = args
                    .get(i + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or("--can-interface requires an interface name")?;
                parsed.can_interface = Some(value.clone());
                i += 2;
            }
            "--config-dir" => {
                let value = args
                    .get(i + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or("--config-dir requires a path")?;
                parsed.config_dir = Some(PathBuf::from(value));
                i += 2;
            }
            value if value.starts_with("--") => {
                return Err(format!("global option {value} must precede the subcommand"));
            }
            _ => {
                if let Some(option) = args[i..]
                    .iter()
                    .find(|arg| matches!(arg.as_str(), "--can-interface" | "--config-dir"))
                {
                    return Err(format!(
                        "global option {option} must precede the subcommand"
                    ));
                }
                parsed.command_args.extend_from_slice(&args[i..]);
                break;
            }
        }
    }
    Ok(parsed)
}

fn parse_gravity_pose(args: &[String], joint_count: usize) -> Result<Vec<f64>, String> {
    let values = args.get(2..).ok_or("missing gravity-preview command")?;
    if values.is_empty() {
        return Ok(vec![0.0; joint_count]);
    }
    if values.len() != joint_count {
        return Err(format!(
            "expected either zero or {joint_count} joint angles, got {}",
            values.len()
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| format!("invalid joint angle: {value}"))
        })
        .collect()
}

/// `disable`: the independent stop. It reads only the drive addresses from
/// `motors.yaml` and sends one Disable to each, so a missing `control.yaml`,
/// URDF, corrupt calibration history or a down CAN interface cannot prevent it
/// from reaching the drives it can reach. Exit 0 only when every drive's
/// Disable was accepted by its interface.
fn run_disable(root: &std::path::Path, interface: Option<&str>) -> i32 {
    let targets = match load_motor_stop_targets(root) {
        Ok(targets) => targets,
        Err(error) => {
            eprintln!("disable: cannot read drive addresses from motors.yaml: {error}");
            eprintln!("disable: NO stop frame was sent; use the physical E-stop");
            return 1;
        }
    };
    let addresses = stop::stop_addresses(&targets, interface);
    if addresses.is_empty() {
        eprintln!("disable: motors.yaml lists no drives; NO stop frame was sent");
        return 1;
    }
    let report = stop::disable_drives_socketcan(&addresses);
    print!("{report}");
    if report.all_sent() {
        println!(
            "disabled: Disable frame sent to all {} drives (queued on the bus, not drive-confirmed)",
            report.drives.len()
        );
        0
    } else {
        eprintln!(
            "disable INCOMPLETE: {} of {} drives were not reached; use the physical E-stop",
            report.failed(),
            report.drives.len()
        );
        1
    }
}

/// Everything the exit stop needs, resolved before any drive can be touched.
struct ExitStop {
    addresses: Vec<robstride::MotorAddress>,
}

impl ExitStop {
    /// Resolve the drive addresses and install the signal-driven stop. A
    /// command that can leave a drive enabled refuses to start without both.
    fn arm(root: &std::path::Path, interface: Option<&str>) -> Result<Self, String> {
        let targets = load_motor_stop_targets(root)
            .map_err(|error| format!("cannot read drive addresses from motors.yaml: {error}"))?;
        let addresses = stop::stop_addresses(&targets, interface);
        if addresses.is_empty() {
            return Err("motors.yaml lists no drives".into());
        }
        stop::install_signal_stop(addresses.clone())?;
        eprintln!(
            "motor-repl: exit stop armed for {} drives (SIGTERM/SIGINT/SIGHUP and error exit)",
            addresses.len()
        );
        Ok(Self { addresses })
    }

    fn run(&self) {
        eprintln!("motor-repl: exit stop, disabling every drive");
        let report = stop::disable_drives_socketcan(&self.addresses);
        eprint!("{report}");
        if !report.all_sent() {
            eprintln!(
                "motor-repl: exit stop INCOMPLETE ({} of {} drives not reached); use the physical E-stop",
                report.failed(),
                report.drives.len()
            );
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let parsed = match parse_bus_args(&args) {
        Ok(parsed) if parsed.command_args.len() >= 2 => parsed,
        Ok(_) => {
            marengo_support::init_tracing();
            usage();
            std::process::exit(1);
        }
        Err(error) => {
            marengo_support::init_tracing();
            eprintln!("motor-repl: {error}");
            usage();
            std::process::exit(1);
        }
    };
    if let Some(dir) = parsed.config_dir {
        env::set_var("MARENGO_CONFIG_DIR", dir);
    }
    marengo_support::init_tracing();

    let can_interface = parsed.can_interface;
    let args = parsed.command_args;
    let root = repo_root();
    let command = args[1].clone();
    if !is_supported_command(&command) {
        eprintln!("unsupported command: {command}");
        usage();
        std::process::exit(1);
    }
    if command == "disable" {
        std::process::exit(run_disable(&root, can_interface.as_deref()));
    }

    let exit_stop = if stop::arms_exit_stop(&command) {
        match ExitStop::arm(&root, can_interface.as_deref()) {
            Ok(exit_stop) => Some(exit_stop),
            Err(error) => {
                eprintln!("{command}: refused, cannot arm the exit stop: {error}");
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    let code = run_command(&root, can_interface, &args);
    if let Some(exit_stop) = &exit_stop {
        if stop::exit_stop_required(&command, code) {
            exit_stop.run();
        }
    }
    std::process::exit(code);
}

fn run_status(root: &std::path::Path, interface: Option<&str>) -> i32 {
    let control = match load_control_config(root) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("control.yaml: {error}");
            return 1;
        }
    };
    let motors = match load_motors_config(root) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("motors.yaml: {error}");
            return 1;
        }
    };
    let bus = match interface {
        Some(interface) => RuntimeBus::socketcan(interface),
        None => RuntimeBus::socketcan_from_motors(&motors),
    };
    match bus {
        Ok(_bus) => {}
        Err(error) => {
            eprintln!("open SocketCAN: {error}");
            return 1;
        }
    }
    println!(
        "configuration: {} joints, loop_hz={}, SocketCAN opened (no Supervisor constructed)",
        motors.motors.len(),
        control.control.loop_hz
    );
    0
}

fn run_gravity_preview(root: &std::path::Path, args: &[String]) -> i32 {
    let robot = match load_robot_config(root) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("robot.yaml: {error}");
            return 1;
        }
    };
    let urdf = match resolve_urdf_path(root, &robot) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("URDF: {error}");
            return 1;
        }
    };
    let model = match gravity_model_from_urdf(&urdf, &robot.robot.joints) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("gravity model: {error}");
            return 1;
        }
    };
    let q = match parse_gravity_pose(args, robot.robot.joints.len()) {
        Ok(q) => q,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let tau = match model.gravity_torques(&q) {
        Ok(tau) => tau,
        Err(error) => {
            eprintln!("tau_g: {error}");
            return 1;
        }
    };
    for (name, torque) in robot.robot.joints.iter().zip(tau.iter()) {
        println!("{name}: tau_g = {torque:.4} Nm");
    }
    0
}

/// Dispatches read-only commands without constructing a Supervisor; all other
/// supported commands load configuration, open CAN and construct their owner.
fn is_read_only_command(command: &str) -> bool {
    matches!(command, "status" | "gravity-preview")
}
fn is_supported_command(command: &str) -> bool {
    matches!(
        command,
        "status" | "disable" | "set-zero" | "protocol-inspect" | "gravity-preview"
    )
}

/// `CanTimeout` (0x7028) counts: 20000 = 1 s, 0 = off.
const CAN_TIMEOUT_COUNTS_PER_SECOND: f64 = 20_000.0;

fn parameter_label(parameter: robstride::ParameterId) -> &'static str {
    use robstride::ParameterId;
    match parameter {
        ParameterId::RunMode => "run_mode",
        ParameterId::MechPos => "mech_pos",
        ParameterId::MechVel => "mech_vel",
        ParameterId::CanTimeout => "can_timeout",
        ParameterId::ZeroSta => "zero_sta",
        ParameterId::AddOffset => "add_offset",
        _ => "register",
    }
}

/// One line per drive; `pi_protocol_inspect` reads `firmware=` and `can_timeout=`.
fn format_inspection(drive: &davout::DriveProtocolInspection) -> String {
    use robstride::{ParameterId, ParameterValue};
    let mut line = format!(
        "inspect {} {}:{} firmware={} uid={}",
        drive.joint, drive.address.interface, drive.address.device_id, drive.firmware, drive.uid
    );
    for (parameter, value) in &drive.parameters {
        let label = parameter_label(*parameter);
        match (parameter, value) {
            (ParameterId::CanTimeout, ParameterValue::U32(0)) => {
                line.push_str(&format!(" {label}=0 (off)"));
            }
            (ParameterId::CanTimeout, ParameterValue::U32(counts)) => line.push_str(&format!(
                " {label}={counts} ({:.3} s)",
                f64::from(*counts) / CAN_TIMEOUT_COUNTS_PER_SECOND
            )),
            (_, ParameterValue::U8(v)) => line.push_str(&format!(" {label}={v}")),
            (_, ParameterValue::U16(v)) => line.push_str(&format!(" {label}={v}")),
            (_, ParameterValue::U32(v)) => line.push_str(&format!(" {label}={v}")),
            (_, ParameterValue::F32(v)) => line.push_str(&format!(" {label}={v:.4}")),
        }
    }
    line
}

/// `protocol-inspect [joint...]` (ADR 0037). The owner transmits nothing at
/// construction; Davout stops every drive before and after the read queries.
fn run_protocol_inspect(
    root: &std::path::Path,
    bus: RuntimeBus,
    joints: &[String],
    bus_label: &str,
) -> i32 {
    let mut owner = match davout::Supervisor::from_repo_for_protocol_inspection(root, bus) {
        Ok(owner) => owner,
        Err(e) => {
            eprintln!("protocol-inspect: {e}");
            return 1;
        }
    };
    match owner.inspect_drive_protocol(joints) {
        Ok(drives) => {
            for drive in &drives {
                println!("{}", format_inspection(drive));
            }
            println!(
                "protocol-inspect: {} drives read, all drives Disabled again (SocketCAN {bus_label})",
                drives.len()
            );
            0
        }
        Err(e) => {
            eprintln!("protocol-inspect refused: {e}");
            1
        }
    }
}

fn run_command(root: &std::path::Path, can_interface: Option<String>, args: &[String]) -> i32 {
    let command = args.get(1).map(String::as_str).unwrap_or_default();
    if !is_supported_command(command) {
        eprintln!("unsupported command: {command}");
        return 1;
    }
    if is_read_only_command(command) {
        return match command {
            "status" => run_status(root, can_interface.as_deref()),
            "gravity-preview" => run_gravity_preview(root, args),
            _ => {
                eprintln!("unsupported command: {command}");
                return 1;
            }
        };
    }
    let control = match load_control_config(root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("control.yaml: {e}");
            return 1;
        }
    };
    let motors = match load_motors_config(root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("motors.yaml: {e}");
            return 1;
        }
    };
    let bus = match can_interface.as_deref() {
        Some(interface) => match RuntimeBus::socketcan(interface) {
            Ok(bus) => bus,
            Err(e) => {
                eprintln!("open SocketCAN {interface}: {e}");
                return 1;
            }
        },
        None => match RuntimeBus::socketcan_from_motors(&motors) {
            Ok(bus) => bus,
            Err(e) => {
                eprintln!("open SocketCAN from motors.yaml: {e}");
                return 1;
            }
        },
    };
    let bus_label = can_interface
        .clone()
        .unwrap_or_else(|| "motors.yaml".to_string());
    let can_interfaces: BTreeSet<_> = motors
        .motors
        .iter()
        .map(|motor| motor.can_interface.as_str())
        .collect();
    info!(
        bus = %bus_label,
        motor_count = motors.motors.len(),
        interfaces = ?can_interfaces,
        "motor-repl opened SocketCAN"
    );
    if args[1] == "protocol-inspect" {
        return run_protocol_inspect(root, bus, &args[2..], &bus_label);
    }
    // Only set-zero needs the physical reference owner and its journal; every
    // other command keeps the ordinary owner so a journal resource fault can
    // never block it.
    let built = if args[1] == "set-zero" {
        match resolve_reference_journal_path(root, resolve_config_dir(root)) {
            Ok(journal) => ControlLoop::from_repo_with_physical_reference(
                root,
                bus,
                journal,
                control.control.loop_hz,
                control.control.chappe_state_hz,
            ),
            Err(e) => {
                eprintln!("reference journal path: {e}");
                return 1;
            }
        }
    } else {
        ControlLoop::from_repo(
            root,
            bus,
            control.control.loop_hz,
            control.control.chappe_state_hz,
        )
    };
    let mut loop_ctrl = match built {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control loop: {e}");
            return 1;
        }
    };

    match args[1].as_str() {
        "set-zero" => {
            let Some(joint) = args.get(2).map(String::as_str) else {
                eprintln!("missing joint name");
                return 1;
            };
            let sign_tested = args.iter().any(|a| a == "--sign-tested");
            // Davout owns preflight, the qualified physical acquisition and the
            // journal commit; the drives are stopped before this returns, and
            // `main` runs the exit stop if it returns an error.
            match loop_ctrl
                .supervisor_mut()
                .calibrate_joint_zero(joint, "bench", sign_tested)
            {
                Ok(pos) => {
                    println!("set-zero {joint} verified pos={pos:.4} rad (SocketCAN {bus_label})");
                }
                Err(e) => {
                    eprintln!("set-zero refused: {e}");
                    return 1;
                }
            }
        }
        _ => {
            usage();
            return 1;
        }
    }
    0
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod argument_tests {
    use super::*;

    #[test]
    fn global_options_are_parsed_before_the_subcommand_and_are_not_command_args() {
        let args = strings(&[
            "motor-repl",
            "--config-dir",
            "cfg",
            "--can-interface",
            "can1",
            "set-zero",
            "a",
        ]);
        let parsed = parse_bus_args(&args).expect("valid args");
        assert_eq!(parsed.config_dir, Some(PathBuf::from("cfg")));
        assert_eq!(parsed.can_interface.as_deref(), Some("can1"));
        assert_eq!(parsed.command_args[1..], ["set-zero", "a"]);
    }

    #[test]
    fn global_options_after_the_subcommand_are_rejected() {
        let args = strings(&["motor-repl", "set-zero", "a", "--config-dir", "cfg"]);
        assert!(parse_bus_args(&args)
            .expect_err("late global option")
            .contains("must precede"));
    }

    #[test]
    fn gravity_preview_requires_an_empty_or_complete_pose() {
        let args = strings(&["motor-repl", "gravity-preview", "0.1"]);
        assert!(parse_gravity_pose(&args, 5)
            .expect_err("partial pose")
            .contains("expected either zero or 5"));
        let default_pose = strings(&["motor-repl", "gravity-preview"]);
        assert_eq!(parse_gravity_pose(&default_pose, 2), Ok(vec![0.0, 0.0]));
        let complete_pose = strings(&["motor-repl", "gravity-preview", "0.1", "-0.2"]);
        assert_eq!(parse_gravity_pose(&complete_pose, 2), Ok(vec![0.1, -0.2]));
        let excess_pose = strings(&["motor-repl", "gravity-preview", "0.1", "-0.2", "0.3"]);
        assert!(parse_gravity_pose(&excess_pose, 2)
            .expect_err("excess pose")
            .contains("expected either zero or 2"));
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn read_only_commands_bypass_supervisor_construction() {
        assert!(is_read_only_command("status"));
        assert!(is_read_only_command("gravity-preview"));
        assert!(!is_read_only_command("set-zero"));
    }

    #[test]
    fn argument_parser_rejects_missing_program_or_command() {
        assert!(parse_bus_args(&[]).is_err());
        let program_only = strings(&["motor-repl"]);
        assert_eq!(
            parse_bus_args(&program_only)
                .expect("program name")
                .command_args
                .len(),
            1
        );
    }
    #[test]
    fn obsolete_motion_and_mode_commands_are_not_admitted() {
        for command in [
            "home",
            "homing-status",
            "enable",
            "jog",
            "speed",
            "speed-stop",
            "gravity-on",
            "gravity-off",
            "torque-cmd",
        ] {
            assert!(!is_supported_command(command), "{command}");
        }
        for command in [
            "status",
            "disable",
            "set-zero",
            "protocol-inspect",
            "gravity-preview",
        ] {
            assert!(is_supported_command(command), "{command}");
        }
    }

    #[test]
    fn inspection_line_carries_firmware_and_can_timeout() {
        use robstride::{DeviceUid, FirmwareVersion, MotorAddress, ParameterId, ParameterValue};
        let drive = davout::DriveProtocolInspection {
            joint: "right_elbow_pitch".into(),
            address: MotorAddress::new("can0", 4),
            firmware: FirmwareVersion([0, 2, 3, 34]),
            uid: DeviceUid([4, 0, 0, 0, 0, 0, 0, 0]),
            parameters: vec![
                (ParameterId::CanTimeout, ParameterValue::U32(600)),
                (ParameterId::MechPos, ParameterValue::F32(0.25)),
            ],
        };
        let line = format_inspection(&drive);
        assert!(line.starts_with("inspect right_elbow_pitch can0:4 firmware=0.2.3.34 "));
        assert!(line.contains(" can_timeout=600 (0.030 s)"), "{line}");
        assert!(line.contains(" mech_pos=0.2500"), "{line}");
        let off = davout::DriveProtocolInspection {
            parameters: vec![(ParameterId::CanTimeout, ParameterValue::U32(0))],
            ..drive
        };
        assert!(format_inspection(&off).ends_with(" can_timeout=0 (off)"));
    }
}
