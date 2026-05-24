//! Marengo Pi runtime: control loop, CAN, and Chappe bridge.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use berthier::{ControlLoop, ControlMode};
use chappe::Bus;
use marengo_config::{load_control_config, load_motors_config};
use marengo_support::init_tracing;
use robstride::RuntimeBus;
use tracing::info;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() {
    init_tracing();
    let root = repo_root();
    let control = match load_control_config(&root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {e}");
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

    let bus_kind = std::env::var("MARENGO_MOTOR_BUS").unwrap_or_else(|_| "memory".to_string());
    let can_interface = std::env::var("MARENGO_CAN_INTERFACE").unwrap_or_else(|_| {
        motors
            .motors
            .first()
            .map(|m| m.can_interface.clone())
            .unwrap_or_else(|| "can0".to_string())
    });
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
            eprintln!("unknown MARENGO_MOTOR_BUS={other}; use memory or socketcan");
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

    let chappe = Arc::new(Bus::default());
    info!(
        hz = control.control.loop_hz,
        bus = %bus_kind,
        can_interface = %can_interface,
        "marengo-pi starting gravity-comp scaffold"
    );

    loop_ctrl.supervisor_mut().set_homing_complete();
    if let Err(e) = loop_ctrl.supervisor_mut().request_enable(true) {
        eprintln!("enable failed (run motor-repl home/enable on bench): {e}");
    }
    loop_ctrl.set_control_mode(ControlMode::GravityComp);

    let period = loop_ctrl.loop_period();
    loop {
        let tick_start = Instant::now();
        if let Err(e) = loop_ctrl.tick(Some(chappe.as_ref())) {
            tracing::error!("control tick: {e}");
            if control.control.disable_on_exit {
                let _ = loop_ctrl.supervisor_mut().disable_all();
            }
            break;
        }
        let elapsed = tick_start.elapsed();
        if elapsed < period {
            std::thread::sleep(period - elapsed);
        }
    }
}
