//! Interactive motor exercise REPL — all motion goes through Davout.

use std::env;
use std::path::PathBuf;

use berthier::{ControlLoop, ControlMode};
use davout::{JointCommand, SpeedCommand};
use marengo_config::{load_control_config, load_motors_config};
use robstride::RuntimeBus;
use tracing::info;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn usage() {
    eprintln!(
        "motor-repl — bench motor exercise (Davout → robstride)\n\
         Usage:\n  \
           motor-repl [--bus memory|socketcan] [--can-interface can0] status\n  \
           motor-repl home\n  \
           motor-repl enable <operator_id>\n  \
           motor-repl disable\n  \
           motor-repl jog <joint> <position_rad>\n  \
           motor-repl speed <joint> <rad_s>\n  \
           motor-repl speed-stop <joint>\n  \
           motor-repl set-zero <joint>\n  \
           motor-repl gravity-on\n  \
           motor-repl gravity-off\n  \
           motor-repl gravity-preview [q0 q1 q2 q3]\n\
         Prefer vcan or simulation before live CAN."
    );
}

fn parse_bus_args(args: Vec<String>, default_interface: String) -> (String, String, Vec<String>) {
    let mut command_args = vec![args[0].clone()];
    let mut bus_kind = env::var("MARENGO_MOTOR_BUS").unwrap_or_else(|_| "memory".to_string());
    let mut can_interface = env::var("MARENGO_CAN_INTERFACE").unwrap_or(default_interface);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--bus" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--bus requires memory or socketcan");
                    std::process::exit(1);
                };
                bus_kind = value.clone();
                i += 2;
            }
            "--can-interface" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--can-interface requires an interface name");
                    std::process::exit(1);
                };
                can_interface = value.clone();
                i += 2;
            }
            _ => {
                command_args.extend_from_slice(&args[i..]);
                break;
            }
        }
    }
    (bus_kind, can_interface, command_args)
}

fn main() {
    marengo_support::init_tracing();
    let args: Vec<String> = env::args().collect();
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
    let default_interface = motors
        .motors
        .first()
        .map(|m| m.can_interface.clone())
        .unwrap_or_else(|| "can0".to_string());
    let (bus_kind, can_interface, args) = parse_bus_args(args, default_interface);
    if args.len() < 2 {
        usage();
        std::process::exit(1);
    }
    let bus = match bus_kind.as_str() {
        "memory" => RuntimeBus::memory(),
        "socketcan" => match RuntimeBus::socketcan(&can_interface) {
            Ok(bus) => bus,
            Err(e) => {
                eprintln!("open SocketCAN {can_interface}: {e}");
                std::process::exit(1);
            }
        },
        other => {
            eprintln!("unknown bus {other}; use memory or socketcan");
            std::process::exit(1);
        }
    };
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
                "operational: {:?}, control: {:?}, bus: {}, interface: {}",
                loop_ctrl.supervisor_mut().mode(),
                loop_ctrl.control_mode(),
                bus_kind,
                can_interface
            );
        }
        "home" => {
            loop_ctrl.supervisor_mut().set_homing_complete();
            println!("homing complete → Ready");
        }
        "enable" => {
            let op = args.get(2).map(String::as_str).unwrap_or("bench");
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
            if let Err(e) = loop_ctrl.supervisor_mut().send_joint_command(JointCommand {
                joint: joint.to_string(),
                position_rad: pos,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            }) {
                eprintln!("jog failed: {e}");
                std::process::exit(1);
            }
            println!("jog {joint} → {pos} rad ({bus_kind} bus)");
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
            loop_ctrl.supervisor_mut().set_homing_complete();
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                std::process::exit(1);
            }
            match loop_ctrl.supervisor_mut().send_speed_command(SpeedCommand {
                joint: joint.to_string(),
                velocity_rad_s: velocity,
            }) {
                Ok(sent) => {
                    println!("speed {joint} → {sent} rad/s (firmware mode 2, {bus_kind} bus)");
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
            println!("speed {joint} → 0 rad/s ({bus_kind} bus)");
        }
        "set-zero" => {
            let joint = args.get(2).map(String::as_str).unwrap_or_else(|| {
                eprintln!("missing joint name");
                std::process::exit(1);
            });
            loop_ctrl.supervisor_mut().set_homing_complete();
            if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
                eprintln!("enable failed: {e}");
                std::process::exit(1);
            }
            if let Err(e) = loop_ctrl.supervisor_mut().set_zero_position(joint) {
                eprintln!("set-zero failed: {e}");
                std::process::exit(1);
            }
            println!("set-zero {joint} ({bus_kind} bus)");
        }
        "gravity-on" => {
            loop_ctrl.set_control_mode(ControlMode::GravityComp);
            println!("control mode → GravityComp (use marengo-pi or tick loop on bench)");
        }
        "gravity-off" => {
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            println!("control mode → Disabled");
        }
        "gravity-preview" => {
            let q: Vec<f64> = if args.len() >= 6 {
                let mut parsed = Vec::new();
                for s in &args[2..6] {
                    match s.parse::<f64>() {
                        Ok(v) => parsed.push(v),
                        Err(_) => {
                            eprintln!("invalid joint angle: {s}");
                            std::process::exit(1);
                        }
                    }
                }
                parsed
            } else {
                vec![0.0, 0.0, 0.0, 0.0]
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
