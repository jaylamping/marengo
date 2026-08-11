//! Interactive motor exercise REPL — all motion goes through Davout.

use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use armee_dynamics::max_gravity_torque_over_range;
use berthier::{ControlLoop, ControlMode};
use davout::{JointCommand, SpeedCommand};
use marengo_config::{load_control_config, load_motors_config, resolve_repo_root};
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
        let tau_max = max_gravity_torque_over_range(model, *i, *q_min, *q_max, 20).unwrap_or(0.0);
        if tau_max > *motor_tau_limit {
            eprintln!(
                "ERROR: gravity torque {tau_max:.3} Nm exceeds motor limit {motor_tau_limit:.3} Nm for joint {joint}. Use --force to override."
            );
            saturated = true;
        } else if tau_max > 0.8 * *motor_tau_limit {
            eprintln!(
                "WARN: gravity torque {tau_max:.3} Nm is >80% of motor limit {motor_tau_limit:.3} Nm for joint {joint}"
            );
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
           motor-repl gravity-preview [q0 q1 q2 q3]\n\
         Homing: set-zero each joint at mechanical reference, then home, then enable.\n\
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
    let control = match load_control_config(&root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("control.yaml: {e}");
            std::process::exit(1);
        }
    };
    let motors = match load_motors_config(&root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("motors.yaml: {e}");
            std::process::exit(1);
        }
    };
    let bus = match can_interface.as_deref() {
        Some(interface) => match RuntimeBus::socketcan(interface) {
            Ok(bus) => bus,
            Err(e) => {
                eprintln!("open SocketCAN {interface}: {e}");
                std::process::exit(1);
            }
        },
        None => match RuntimeBus::socketcan_from_motors(&motors) {
            Ok(bus) => bus,
            Err(e) => {
                eprintln!("open SocketCAN from motors.yaml: {e}");
                std::process::exit(1);
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
    let mut loop_ctrl = match ControlLoop::from_repo(
        &root,
        bus,
        control.control.loop_hz,
        control.control.chappe_state_hz,
    ) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control loop: {e}");
            std::process::exit(1);
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
                std::process::exit(1);
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
                    eprintln!("run set-zero for each joint, then home");
                    std::process::exit(1);
                }
            }
            if !force {
                if let Err(code) = preflight_gravity_saturation(&mut loop_ctrl) {
                    std::process::exit(code);
                }
            }
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                std::process::exit(1);
            }
            println!("enabled (operator={op})");
        }
        "disable" => {
            if let Err(e) = loop_ctrl.supervisor_mut().disable_all() {
                eprintln!("disable failed: {e}");
                std::process::exit(1);
            }
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            println!("disabled");
        }
        "jog" => {
            let joint = args.get(2).map(String::as_str).unwrap_or_else(|| {
                eprintln!("missing joint name");
                std::process::exit(1);
            });
            let pos: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                eprintln!("missing or invalid position_rad");
                std::process::exit(1);
            });
            if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                eprintln!("jog blocked: {e}");
                std::process::exit(1);
            }
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                std::process::exit(1);
            }
            if let Err(e) = loop_ctrl.supervisor_mut().send_joint_command(JointCommand {
                joint: joint.to_string(),
                position_rad: pos,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            }) {
                eprintln!("jog failed: {e}");
                std::process::exit(1);
            }
            println!("jog {joint} → {pos} rad (SocketCAN {bus_label})");
        }
        "speed" => {
            let joint = args.get(2).map(String::as_str).unwrap_or_else(|| {
                eprintln!("missing joint name");
                std::process::exit(1);
            });
            let velocity: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                eprintln!("missing or invalid rad_s");
                std::process::exit(1);
            });
            if !control.control.bench.allow_firmware_speed_mode {
                eprintln!("firmware speed mode disabled: set control.bench.allow_firmware_speed_mode=true for bench diagnostics");
                std::process::exit(1);
            }
            if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                eprintln!("speed blocked: {e}");
                std::process::exit(1);
            }
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                std::process::exit(1);
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
                    std::process::exit(1);
                }
            }
        }
        "speed-stop" => {
            let joint = args.get(2).map(String::as_str).unwrap_or_else(|| {
                eprintln!("missing joint name");
                std::process::exit(1);
            });
            if let Err(e) = loop_ctrl.supervisor_mut().stop_speed_command(joint) {
                eprintln!("speed-stop failed: {e}");
                std::process::exit(1);
            }
            println!("speed {joint} → 0 rad/s (SocketCAN {bus_label})");
        }
        "set-zero" => {
            let joint = args.get(2).map(String::as_str).unwrap_or_else(|| {
                eprintln!("missing joint name");
                std::process::exit(1);
            });
            let sign_tested = args.iter().any(|a| a == "--sign-tested");
            if loop_ctrl.supervisor_mut().mode() != davout::OperationalMode::Active {
                if let Err(e) = loop_ctrl.supervisor_mut().request_enable_for_calibration() {
                    eprintln!("enable for set-zero failed: {e}");
                    std::process::exit(1);
                }
            }
            if let Err(e) = loop_ctrl.supervisor_mut().set_zero_position(joint) {
                eprintln!("set-zero failed: {e}");
                std::process::exit(1);
            }
            let _ = loop_ctrl.supervisor_mut().refresh_feedback();
            match loop_ctrl
                .supervisor_mut()
                .verify_zero_after_set(joint, "bench", sign_tested)
            {
                Ok(pos) => {
                    println!("set-zero {joint} verified pos={pos:.4} rad (SocketCAN {bus_label})");
                }
                Err(e) => {
                    eprintln!("set-zero verify failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        "gravity-on" => {
            loop_ctrl.set_control_mode(ControlMode::GravityComp);
            println!("control mode → GravityComp (use marengo-pi or tick loop on bench)");
        }
        "gravity-off" => {
            loop_ctrl.set_control_mode(ControlMode::TorqueOnly);
            println!("control mode → TorqueOnly (τ_cmd≡0; use torque-cmd for nonzero steps)");
        }
        "torque-cmd" => {
            if args.len() < 4 {
                eprintln!("usage: motor-repl torque-cmd <joint> <nm>");
                std::process::exit(1);
            }
            let joint = &args[2];
            let tau: f64 = args[3].parse().unwrap_or_else(|_| {
                eprintln!("invalid torque Nm: {}", args[3]);
                std::process::exit(1);
            });
            match loop_ctrl.set_torque_cmd(joint, tau) {
                Ok(()) => {
                    if loop_ctrl.control_mode() != ControlMode::TorqueOnly {
                        loop_ctrl.set_control_mode(ControlMode::TorqueOnly);
                    }
                    println!("τ_cmd {joint} = {tau:.4} Nm (mode=TorqueOnly)");
                }
                Err(e) => {
                    eprintln!("torque-cmd failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        "gravity-preview" => {
            let joint_count = loop_ctrl.supervisor_mut().motors.motors.len();
            let q: Vec<f64> = if args.len() >= 2 + joint_count {
                args[2..2 + joint_count]
                    .iter()
                    .map(|s| {
                        s.parse::<f64>().unwrap_or_else(|_| {
                            eprintln!("invalid joint angle: {s}");
                            std::process::exit(1);
                        })
                    })
                    .collect()
            } else {
                vec![0.0; joint_count]
            };
            let tau = match loop_ctrl.preview_gravity_torques(&q) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("tau_g: {e}");
                    std::process::exit(1);
                }
            };
            for (name, t) in loop_ctrl
                .supervisor_mut()
                .motors
                .motors
                .iter()
                .map(|m| &m.joint)
                .zip(tau.iter())
            {
                println!("{name}: tau_g = {t:.4} Nm");
            }
        }
        _ => {
            usage();
            std::process::exit(1);
        }
    }
}
