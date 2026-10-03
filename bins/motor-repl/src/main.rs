//! Interactive motor exercise REPL — all motion goes through Davout.
//!
//! `disable` and the exit stop are deliberately independent of Davout: see
//! [`stop`].

mod stop;

use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use armee_dynamics::{check_gravity_range, GravityRangeVerdict};
use berthier::{ControlLoop, ControlMode};
use davout::{JointCommand, SpeedCommand};
use marengo_config::{
    load_control_config, load_motor_stop_targets, load_motors_config, resolve_config_dir,
    resolve_reference_journal_path, resolve_repo_root,
};
use robstride::RuntimeBus;
use tracing::info;
fn repo_root() -> PathBuf {
    resolve_repo_root()
}

/// Pre-flight gravity saturation check: refuse enable if max(|tau_g|) over the
/// joint range exceeds the motor torque limit; warn above 80%.
/// Returns `Ok(())` or `Err(exit_code)`.
fn preflight_gravity_saturation(loop_ctrl: &mut ControlLoop<RuntimeBus>) -> Result<(), i32> {
    // Collect per-joint range + torque limit from the supervisor first, so the
    // &mut supervisor borrow ends before we borrow loop_ctrl for the dynamics model.
    let joint_names: Vec<String> = loop_ctrl.joint_names().to_vec();
    let joint_specs: Vec<(String, usize, f64, f64, f64)> = {
        let supervisor = loop_ctrl.supervisor_mut();
        let motors = &supervisor.motors.motors;
        joint_names
            .iter()
            .enumerate()
            .filter_map(|(i, joint)| {
                let motor = motors.iter().find(|m| &m.joint == joint)?;
                let policy = supervisor.joint_limit_policy(joint)?;
                Some((
                    joint.clone(),
                    i,
                    motor.bench.position_lower_rad,
                    motor.bench.position_upper_rad,
                    policy.tau_ff_max,
                ))
            })
            .collect()
    };
    let model = loop_ctrl.dynamics_model();
    let mut saturated = false;
    for (joint, i, q_min, q_max, motor_tau_limit) in &joint_specs {
        match check_gravity_range(model, *i, *q_min, *q_max, *motor_tau_limit, 20) {
            GravityRangeVerdict::Within { .. } => {}
            GravityRangeVerdict::Near { tau_max_nm } => eprintln!(
                "WARN: gravity torque {tau_max_nm:.3} Nm is >80% of motor limit {motor_tau_limit:.3} Nm for joint {joint}"
            ),
            GravityRangeVerdict::Saturated { tau_max_nm } => {
                eprintln!(
                    "ERROR: gravity torque {tau_max_nm:.3} Nm exceeds motor limit {motor_tau_limit:.3} Nm for joint {joint}. Use --force to override."
                );
                saturated = true;
            }
            GravityRangeVerdict::Unevaluable(error) => {
                eprintln!(
                    "ERROR: gravity model could not be evaluated for joint {joint}: {error}. Use --force to override."
                );
                saturated = true;
            }
        }
    }
    if saturated {
        Err(1)
    } else {
        Ok(())
    }
}

fn usage() {
    eprintln!(
        "motor-repl — bench motor exercise (Davout → robstride)\n\
         Usage:\n  \
         motor-repl [--config-dir PATH] [--can-interface can0] status\n  \
           motor-repl homing-status\n  \
           motor-repl home\n  \
           motor-repl enable <operator_id> [--force]\n  \
           motor-repl disable\n  \
           motor-repl jog <joint> <position_rad>\n  \
           motor-repl speed <joint> <rad_s>\n  \
           motor-repl speed-stop <joint>\n  \
           motor-repl set-zero <joint> [--sign-tested]\n  \
           motor-repl gravity-on\n  \
           motor-repl gravity-off\n  \
           motor-repl torque-cmd <joint> <nm>\n  \
           motor-repl gravity-preview [q...]  (robot.yaml joint order)\n\
         Homing: saved calibration is history; every fresh process starts Unhomed.\n\
         set-zero runs the qualified physical reference workflow (ADR 0036); its current grant\n\
         ends with this process, so enable after homing from one long-running marengo-pi\n\
         (stdin `home <joint>... sign-tested`).\n\
         disable reads only the drive addresses in motors.yaml and sends one Disable to each (exit 1 if any drive was not reached).\n\
         enable/jog/speed/speed-stop/set-zero disable every drive on SIGTERM/SIGINT/SIGHUP and on any error exit.\n\
         Uses SocketCAN; prefer test harness or simulation before live CAN.\n\
         Env: MARENGO_ROOT, MARENGO_CONFIG_DIR (e.g. config/bringup/shoulder_pitch_dual)"
    );
}

fn parse_bus_args(args: Vec<String>) -> (Option<String>, Option<PathBuf>, Vec<String>) {
    let mut command_args = vec![args[0].clone()];
    let mut can_interface = env::var("MARENGO_CAN_INTERFACE").ok();
    let mut config_dir = env::var("MARENGO_CONFIG_DIR").ok().map(PathBuf::from);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--can-interface" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--can-interface requires an interface name");
                    std::process::exit(1);
                };
                can_interface = Some(value.clone());
                i += 2;
            }
            "--config-dir" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--config-dir requires a path");
                    std::process::exit(1);
                };
                config_dir = Some(PathBuf::from(value));
                i += 2;
            }
            _ => {
                command_args.extend_from_slice(&args[i..]);
                break;
            }
        }
    }
    (can_interface, config_dir, command_args)
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
    marengo_support::init_tracing();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
        std::process::exit(1);
    }

    let (can_interface, config_dir, args) = parse_bus_args(args);
    if let Some(dir) = config_dir {
        env::set_var("MARENGO_CONFIG_DIR", dir);
    }
    if args.len() < 2 {
        usage();
        std::process::exit(1);
    }

    let root = repo_root();
    let command = args[1].clone();
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

/// Load configuration, open the bus and run one command. Returns the exit
/// code; never exits the process, so `main` can run the exit stop afterwards.
fn run_command(root: &std::path::Path, can_interface: Option<String>, args: &[String]) -> i32 {
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
        "status" => {
            info!(
                mode = ?loop_ctrl.supervisor_mut().mode(),
                control = ?loop_ctrl.control_mode(),
                "motor-repl status"
            );
            println!(
                "operational: {:?}, control: {:?}, SocketCAN: {}",
                loop_ctrl.supervisor_mut().mode(),
                loop_ctrl.control_mode(),
                bus_label
            );
            let joints: Vec<String> = loop_ctrl
                .supervisor_mut()
                .motors
                .motors
                .iter()
                .map(|m| m.joint.clone())
                .collect();
            for joint in &joints {
                let state = loop_ctrl.supervisor_mut().joint_homing_state(joint);
                println!("  homing {joint}: {state:?}");
            }
        }
        "homing-status" => {
            let joints: Vec<String> = loop_ctrl
                .supervisor_mut()
                .motors
                .motors
                .iter()
                .map(|m| m.joint.clone())
                .collect();
            for joint in &joints {
                let state = loop_ctrl.supervisor_mut().joint_homing_state(joint);
                let pos = loop_ctrl.supervisor_mut().joint_position_rad(joint);
                println!(
                    "{joint}: homing={state:?} pos={}",
                    pos.map(|p| format!("{p:.4} rad"))
                        .unwrap_or_else(|| "n/a".into())
                );
            }
        }
        "home" => {
            if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                eprintln!("home failed: {e}");
                return 1;
            }
            println!("homing verified → Ready");
        }
        "enable" => {
            let force = args.iter().any(|a| a == "--force");
            let op = args
                .iter()
                .skip(2)
                .find(|a| *a != "--force")
                .map(String::as_str)
                .unwrap_or("bench");
            if loop_ctrl.supervisor_mut().mode() != davout::OperationalMode::Ready {
                if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                    eprintln!("enable blocked: {e}");
                    eprintln!(
                        "saved history cannot grant current reference; home with marengo-pi `home <joint>... sign-tested` and enable in that process (docs/homing.md)"
                    );
                    return 1;
                }
            }
            if !force {
                if let Err(code) = preflight_gravity_saturation(&mut loop_ctrl) {
                    return code;
                }
            }
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                return 1;
            }
            println!("enabled (operator={op})");
        }
        "jog" => {
            let Some(joint) = args.get(2).map(String::as_str) else {
                eprintln!("missing joint name");
                return 1;
            };
            let Some(pos) = args.get(3).and_then(|s| s.parse::<f64>().ok()) else {
                eprintln!("missing or invalid position_rad");
                return 1;
            };
            if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                eprintln!("jog blocked: {e}");
                return 1;
            }
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                return 1;
            }
            if let Err(e) = loop_ctrl.supervisor_mut().send_joint_command(JointCommand {
                joint: joint.to_string(),
                position_rad: pos,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            }) {
                eprintln!("jog failed: {e}");
                return 1;
            }
            println!("jog {joint} → {pos} rad (SocketCAN {bus_label})");
        }
        "speed" => {
            let Some(joint) = args.get(2).map(String::as_str) else {
                eprintln!("missing joint name");
                return 1;
            };
            let Some(velocity) = args.get(3).and_then(|s| s.parse::<f64>().ok()) else {
                eprintln!("missing or invalid rad_s");
                return 1;
            };
            if !control.control.bench.allow_firmware_speed_mode {
                eprintln!("firmware speed mode disabled: set control.bench.allow_firmware_speed_mode=true for bench diagnostics");
                return 1;
            }
            if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                eprintln!("speed blocked: {e}");
                return 1;
            }
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                return 1;
            }
            match loop_ctrl.supervisor_mut().send_speed_command(SpeedCommand {
                joint: joint.to_string(),
                velocity_rad_s: velocity,
            }) {
                Ok(sent) => {
                    println!(
                        "speed {joint} → {sent} rad/s (firmware mode 2, SocketCAN {bus_label})"
                    );
                }
                Err(e) => {
                    eprintln!("speed failed: {e}");
                    return 1;
                }
            }
        }
        "speed-stop" => {
            let Some(joint) = args.get(2).map(String::as_str) else {
                eprintln!("missing joint name");
                return 1;
            };
            if let Err(e) = loop_ctrl.supervisor_mut().stop_speed_command(joint) {
                eprintln!("speed-stop failed: {e}");
                return 1;
            }
            println!("speed {joint} → 0 rad/s (SocketCAN {bus_label})");
        }
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
        "gravity-on" => {
            loop_ctrl.set_control_mode(ControlMode::GravityComp);
            println!("control mode → GravityComp (use marengo-pi or tick loop on bench)");
        }
        "gravity-off" => {
            loop_ctrl.enter_torque_only_zero();
            println!("control mode → TorqueOnly (τ_cmd≡0; use torque-cmd for nonzero steps)");
        }
        "torque-cmd" => {
            if args.len() < 4 {
                eprintln!("usage: motor-repl torque-cmd <joint> <nm>");
                return 1;
            }
            let joint = &args[2];
            let Ok(tau) = args[3].parse::<f64>() else {
                eprintln!("invalid torque Nm: {}", args[3]);
                return 1;
            };
            match loop_ctrl.set_torque_cmd(joint, tau) {
                Ok(()) => {
                    println!("τ_cmd {joint} = {tau:.4} Nm (mode=TorqueOnly)");
                }
                Err(e) => {
                    eprintln!("torque-cmd failed: {e}");
                    return 1;
                }
            }
        }
        "gravity-preview" => {
            // q / τ vectors follow robot.yaml joint order (same as dynamics), not motors.yaml list order.
            let names = loop_ctrl.joint_names().to_vec();
            let joint_count = names.len();
            let q: Vec<f64> = if args.len() >= 2 + joint_count {
                let mut q = Vec::with_capacity(joint_count);
                for s in &args[2..2 + joint_count] {
                    match s.parse::<f64>() {
                        Ok(value) => q.push(value),
                        Err(_) => {
                            eprintln!("invalid joint angle: {s}");
                            return 1;
                        }
                    }
                }
                q
            } else {
                vec![0.0; joint_count]
            };
            let tau = match loop_ctrl.preview_gravity_torques(&q) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("tau_g: {e}");
                    return 1;
                }
            };
            for (name, t) in names.iter().zip(tau.iter()) {
                println!("{name}: tau_g = {t:.4} Nm");
            }
        }
        _ => {
            usage();
            return 1;
        }
    }
    0
}
