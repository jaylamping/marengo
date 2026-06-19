//! Marengo Pi runtime: CAN I/O, control loop, Chappe telemetry, operator commands.

mod host_metrics;
#[cfg(all(target_os = "linux", feature = "linux-i2c"))]
mod imu;

use std::collections::BTreeSet;
use std::env;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use armee_proto::prost::Message;
use armee_proto::{
    EnableRequest, Fault, FaultSeverity, Heartbeat, HomingComplete, OperationalMode as ProtoOpMode,
    SafetyState,
};
use berthier::{proto_control_mode, ControlLoop, ControlMode, TickPhaseAverages};
use chappe::Bus;
use davout::{MotorAddress, OperationalMode};
use marengo_config::{
    load_control_config, load_motors_config, resolve_config_dir, resolve_repo_root,
    resolve_urdf_path,
};
use robstride::RuntimeBus;
use tracing::{debug, error, info, warn};

fn repo_root() -> PathBuf {
    resolve_repo_root()
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn proto_operational_mode(mode: OperationalMode) -> i32 {
    match mode {
        OperationalMode::Disabled => ProtoOpMode::Disabled as i32,
        OperationalMode::Ready => ProtoOpMode::Ready as i32,
        OperationalMode::Active => ProtoOpMode::Active as i32,
    }
}

enum PiCommand {
    Home,
    Enable {
        operator_id: String,
    },
    Disable,
    GravityOn,
    GravityOff,
    ImpedanceOn,
    ImpedanceOff,
    HoldOn,
    HoldAt {
        joint: Option<String>,
        position_rad: f64,
    },
    HoldOff,
    Status,
    Quit,
}

fn parse_command(line: &str) -> Option<PiCommand> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "home" => Some(PiCommand::Home),
        "enable" => {
            let operator_id = parts.next().unwrap_or("bench").to_string();
            Some(PiCommand::Enable { operator_id })
        }
        "disable" => Some(PiCommand::Disable),
        "gravity-on" | "gravity_on" => Some(PiCommand::GravityOn),
        "gravity-off" | "gravity_off" => Some(PiCommand::GravityOff),
        "impedance-on" | "impedance_on" => Some(PiCommand::ImpedanceOn),
        "impedance-off" | "impedance_off" => Some(PiCommand::ImpedanceOff),
        "hold-on" | "hold_on" => Some(PiCommand::HoldOn),
        "hold-off" | "hold_off" => Some(PiCommand::HoldOff),
        "hold-at" | "hold_at" => {
            let tokens: Vec<_> = parts.collect();
            match tokens.as_slice() {
                [rad] => {
                    let position_rad = rad.parse().ok()?;
                    Some(PiCommand::HoldAt {
                        joint: None,
                        position_rad,
                    })
                }
                [joint, rad] => {
                    let position_rad = rad.parse().ok()?;
                    Some(PiCommand::HoldAt {
                        joint: Some(joint.to_string()),
                        position_rad,
                    })
                }
                _ => {
                    eprintln!("hold-at usage: hold-at <rad>  OR  hold-at <joint> <rad>");
                    None
                }
            }
        }
        "status" => Some(PiCommand::Status),
        "quit" | "exit" => Some(PiCommand::Quit),
        "help" => {
            print_usage();
            None
        }
        _ => {
            eprintln!("unknown command: {line} (type help)");
            None
        }
    }
}

fn print_usage() {
    eprintln!(
        "marengo-pi commands (stdin):\n  \
         home\n  \
         enable [operator_id]\n  \
         disable\n  \
         gravity-on | gravity-off\n  \
         impedance-on | impedance-off\n  \
         hold-on | hold-at [joint] <rad> | hold-off\n  \
         status\n  \
         quit"
    );
}

fn spawn_stdin_commands(tx: Sender<PiCommand>) {
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else {
                break;
            };
            let Some(cmd) = parse_command(line.trim()) else {
                continue;
            };
            if matches!(cmd, PiCommand::Quit) {
                let _ = tx.send(cmd);
                break;
            }
            if tx.send(cmd).is_err() {
                break;
            }
        }
    });
}

fn handle_chappe_enable(
    loop_ctrl: &mut ControlLoop<RuntimeBus>,
    request: &EnableRequest,
) -> Result<(), String> {
    if request.enable {
        if loop_ctrl.supervisor_mut().mode() != davout::OperationalMode::Ready {
            loop_ctrl
                .supervisor_mut()
                .set_homing_complete()
                .map_err(|e| e.to_string())?;
        }
        loop_ctrl
            .supervisor_mut()
            .request_enable(true)
            .map_err(|e| e.to_string())?;
        info!(operator = %request.operator_id, "enable via Chappe");
    } else {
        loop_ctrl
            .supervisor_mut()
            .disable_all()
            .map_err(|e| e.to_string())?;
        loop_ctrl.set_control_mode(ControlMode::Disabled);
        info!(operator = %request.operator_id, "disable via Chappe");
    }
    Ok(())
}

fn drain_chappe_commands(
    loop_ctrl: &mut ControlLoop<RuntimeBus>,
    enable_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    homing_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
) {
    while let Ok(bytes) = enable_rx.try_recv() {
        let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
            continue;
        };
        let Ok(request) = EnableRequest::decode(envelope.payload.as_slice()) else {
            continue;
        };
        if let Err(e) = handle_chappe_enable(loop_ctrl, &request) {
            warn!(error = %e, "Chappe enable request failed");
        }
    }
    while let Ok(bytes) = homing_rx.try_recv() {
        let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
            continue;
        };
        let Ok(_homing) = HomingComplete::decode(envelope.payload.as_slice()) else {
            continue;
        };
        if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
            warn!(error = %e, "Chappe homing rejected");
        } else {
            info!("homing verified via Chappe");
        }
    }
}

fn publish_safety(
    chappe: &Bus,
    mode: OperationalMode,
    active_fault: Option<&str>,
) -> Result<(), chappe::BusError> {
    let mut faults = Vec::new();
    if let Some(message) = active_fault {
        faults.push(Fault {
            code: "runtime".to_string(),
            message: message.to_string(),
            severity: FaultSeverity::Fault as i32,
            joint: String::new(),
        });
    }
    let state = SafetyState {
        timestamp_ms: timestamp_ms(),
        mode: proto_operational_mode(mode),
        hardware_estop_asserted: false,
        software_estop_latched: active_fault.is_some(),
        active_faults: faults,
    };
    chappe.publish(
        "robot/safety",
        "marengo-pi",
        "marengo.v1.SafetyState",
        &state,
    )
}

fn publish_heartbeat(chappe: &Bus) -> Result<(), chappe::BusError> {
    chappe.publish(
        "robot/heartbeat",
        "marengo-pi",
        "marengo.v1.Heartbeat",
        &Heartbeat {
            timestamp_ms: timestamp_ms(),
            node_id: "marengo-pi".to_string(),
        },
    )
}

fn print_status(loop_ctrl: &mut ControlLoop<RuntimeBus>, config_dir: &Path) {
    let control_mode = loop_ctrl.control_mode();
    let supervisor = loop_ctrl.supervisor_mut();
    let operational = supervisor.mode();
    println!(
        "config: {}\noperational: {:?}\ncontrol: {:?}",
        config_dir.display(),
        operational,
        control_mode,
    );
    for motor in &supervisor.motors.motors {
        let address = MotorAddress::from(motor);
        match supervisor.motor_states().get(&address) {
            Some(state) => println!(
                "{} ({}/id{}): pos={:.4} rad vel={:.4} rad/s torque={:.4} Nm fault={:#06x}",
                motor.joint,
                motor.can_interface,
                motor.device_id,
                state.position_rad,
                state.velocity_rad_s,
                state.torque_nm,
                state.fault,
            ),
            None => println!(
                "{} ({}/id{}): no feedback yet",
                motor.joint, motor.can_interface, motor.device_id,
            ),
        }
    }
}

fn handle_command(
    loop_ctrl: &mut ControlLoop<RuntimeBus>,
    cmd: PiCommand,
    config_dir: &Path,
) -> bool {
    match cmd {
        PiCommand::Home => match loop_ctrl.supervisor_mut().set_homing_complete() {
            Ok(()) => println!("homing verified → Ready"),
            Err(e) => eprintln!("home failed: {e}"),
        },
        PiCommand::Enable { operator_id } => {
            if loop_ctrl.supervisor_mut().mode() != davout::OperationalMode::Ready {
                if let Err(e) = loop_ctrl.supervisor_mut().set_homing_complete() {
                    eprintln!("enable blocked: {e}");
                    return true;
                }
            }
            match loop_ctrl.supervisor_mut().request_enable(true) {
                Ok(()) => println!("enabled (operator={operator_id})"),
                Err(e) => eprintln!("enable failed: {e}"),
            }
        }
        PiCommand::Disable => {
            if let Err(e) = loop_ctrl.supervisor_mut().disable_all() {
                eprintln!("disable failed: {e}");
            }
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            println!("disabled");
        }
        PiCommand::GravityOn => {
            loop_ctrl.set_control_mode(ControlMode::GravityComp);
            println!(
                "control mode → GravityComp (operational={:?})",
                loop_ctrl.supervisor_mut().mode()
            );
        }
        PiCommand::GravityOff => {
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            println!("control mode → Disabled");
        }
        PiCommand::ImpedanceOn => {
            loop_ctrl.set_control_mode(ControlMode::Impedance);
            println!(
                "control mode → Impedance (operational={:?})",
                loop_ctrl.supervisor_mut().mode()
            );
        }
        PiCommand::ImpedanceOff => {
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            println!("control mode → Disabled");
        }
        PiCommand::HoldOn => match loop_ctrl.enter_position_hold() {
            Ok(()) => {
                println!(
                    "control mode → Position hold (operational={:?})",
                    loop_ctrl.supervisor_mut().mode()
                );
                if let Some(sp) = loop_ctrl.position_setpoints() {
                    for (name, &q) in loop_ctrl.joint_names().iter().zip(sp) {
                        println!("  hold {name} = {q:.4} rad");
                    }
                }
            }
            Err(e) => eprintln!("hold-on failed: {e}"),
        },
        PiCommand::HoldAt {
            joint,
            position_rad,
        } => match loop_ctrl.enter_position_hold_at(joint.as_deref(), position_rad) {
            Ok(()) => {
                println!(
                    "control mode → Position hold → target {position_rad:.4} rad (ramping, operational={:?})",
                    loop_ctrl.supervisor_mut().mode()
                );
                if let Some(j) = joint.as_deref() {
                    println!("  joint {j}");
                }
            }
            Err(e) => eprintln!("hold-at failed: {e}"),
        },
        PiCommand::HoldOff => {
            loop_ctrl.clear_position_hold();
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            println!("hold-off → Disabled");
        }
        PiCommand::Status => print_status(loop_ctrl, config_dir),
        PiCommand::Quit => return false,
    }
    true
}

fn usage() {
    eprintln!(
        "marengo-pi — Pi control runtime (Berthier → Davout → SocketCAN)\n\
         Usage: marengo-pi [--config-dir PATH] [--no-stdin-ctl]\n\
         Env:  MARENGO_ROOT, MARENGO_CONFIG_DIR — override repo/config paths\n\
         Bring-up: MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_dual"
    );
}

fn parse_args() -> (Option<PathBuf>, bool) {
    let mut args = env::args().skip(1);
    let mut config_dir = None;
    let mut stdin_ctl = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config-dir" => {
                let Some(path) = args.next() else {
                    eprintln!("--config-dir requires a path");
                    std::process::exit(1);
                };
                config_dir = Some(PathBuf::from(path));
            }
            "--no-stdin-ctl" => stdin_ctl = false,
            "--help" | "-h" => {
                usage();
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                usage();
                std::process::exit(1);
            }
        }
    }
    (config_dir, stdin_ctl)
}

fn main() {
    let (cli_config_dir, stdin_ctl) = parse_args();
    let root = repo_root();
    if let Some(dir) = cli_config_dir {
        env::set_var("MARENGO_CONFIG_DIR", dir);
    }
    let config_dir = resolve_config_dir(&root);

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
    let robot = match marengo_config::load_robot_config(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("robot.yaml: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = resolve_urdf_path(&root, &robot) {
        eprintln!("urdf: {e}");
        std::process::exit(1);
    }

    let can_interfaces: BTreeSet<_> = motors
        .motors
        .iter()
        .map(|motor| motor.can_interface.as_str())
        .collect();

    let bus = match RuntimeBus::socketcan_from_motors(&motors) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("open SocketCAN from motors.yaml: {e}");
            eprintln!("build with: cargo build -p marengo-pi --features socketcan");
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
    chappe::tracing_layer::init_subscriber(Some(Arc::clone(&chappe)), "marengo-pi");
    #[cfg(unix)]
    if let Some(socket_path) = chappe::ipc::socket_path_from_env() {
        match chappe::ipc::IpcFanout::spawn_client(socket_path.clone(), (*chappe).clone()) {
            Ok(fanout) => {
                chappe.set_ipc_fanout(fanout);
                info!(path = %socket_path.display(), "Chappe IPC fanout enabled");
            }
            Err(e) => warn!(error = %e, "Chappe IPC fanout disabled"),
        }
    }
    let mut enable_rx = chappe.subscribe("robot/enable");
    let mut homing_rx = chappe.subscribe("robot/homing");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown);
    if ctrlc::set_handler(move || {
        shutdown_flag.store(true, Ordering::SeqCst);
    })
    .is_err()
    {
        eprintln!("failed to install ctrl-c handler");
        std::process::exit(1);
    }

    #[cfg(all(target_os = "linux", feature = "linux-i2c"))]
    imu::spawn_imu_publisher(Arc::clone(&chappe), Arc::clone(&shutdown));

    #[cfg(target_os = "linux")]
    host_metrics::spawn_host_metrics_publisher(Arc::clone(&chappe), Arc::clone(&shutdown));

    let (cmd_tx, cmd_rx): (Sender<PiCommand>, Receiver<PiCommand>) = mpsc::channel();
    if stdin_ctl {
        spawn_stdin_commands(cmd_tx);
        print_usage();
    }

    info!(
        hz = control.control.loop_hz,
        chappe_hz = control.control.chappe_state_hz,
        motor_count = motors.motors.len(),
        interfaces = ?can_interfaces,
        config = %config_dir.display(),
        "marengo-pi starting (Disabled — run home/enable/gravity-on when ready)"
    );

    let mut runtime = ControlLoopRuntime {
        config_dir: &config_dir,
        chappe_state_hz: control.control.chappe_state_hz,
        chappe: &chappe,
        cmd_rx: &cmd_rx,
        enable_rx: &mut enable_rx,
        homing_rx: &mut homing_rx,
        shutdown: &shutdown,
    };
    run_control_loop(&mut loop_ctrl, &mut runtime);

    if control.control.disable_on_exit {
        if let Err(e) = loop_ctrl.supervisor_mut().disable_all() {
            warn!(error = %e, "disable_all on shutdown failed");
        }
    }
    info!("marengo-pi stopped");
}

struct ControlLoopRuntime<'a> {
    config_dir: &'a Path,
    chappe_state_hz: u32,
    chappe: &'a Arc<Bus>,
    cmd_rx: &'a Receiver<PiCommand>,
    enable_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    homing_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    shutdown: &'a Arc<AtomicBool>,
}

#[tracing::instrument(skip(loop_ctrl, runtime))]
fn run_control_loop(loop_ctrl: &mut ControlLoop<RuntimeBus>, runtime: &mut ControlLoopRuntime<'_>) {
    let period = loop_ctrl.loop_period();
    let chappe_period = Duration::from_secs_f64(1.0 / f64::from(runtime.chappe_state_hz.max(1)));
    let mut last_chappe = Instant::now();
    let mut last_heartbeat = Instant::now();
    let mut active_fault: Option<String>;
    let mut timing = LoopTimingWindow::new(loop_ctrl.tick_count());

    while !runtime.shutdown.load(Ordering::SeqCst) {
        let tick_start = Instant::now();
        let mut outer_stdin_us = 0u64;
        let mut outer_chappe_drain_us = 0u64;

        let mut t = tick_start;
        while let Ok(cmd) = runtime.cmd_rx.try_recv() {
            if !handle_command(loop_ctrl, cmd, runtime.config_dir) {
                runtime.shutdown.store(true, Ordering::SeqCst);
                break;
            }
        }
        (outer_stdin_us, t) = phase_elapsed_us(t);

        drain_chappe_commands(loop_ctrl, runtime.enable_rx, runtime.homing_rx);
        (outer_chappe_drain_us, t) = phase_elapsed_us(t);

        active_fault = match loop_ctrl.tick(Some(runtime.chappe.as_ref())) {
            Ok(()) => None,
            Err(e) => {
                error!(error = %e, "control tick failed");
                let _ = loop_ctrl.supervisor_mut().disable_all();
                loop_ctrl.set_control_mode(ControlMode::Disabled);
                Some(e.to_string())
            }
        };

        let elapsed = tick_start.elapsed();
        timing.record_tick(
            elapsed,
            period,
            loop_ctrl.supervisor_mut().last_refresh_frame_count(),
            outer_stdin_us,
            outer_chappe_drain_us,
        );

        let now = Instant::now();
        if now.duration_since(last_chappe) >= chappe_period {
            let mode = loop_ctrl.supervisor_mut().mode();
            if let Err(e) = publish_safety(runtime.chappe.as_ref(), mode, active_fault.as_deref()) {
                warn!(error = %e, "failed to publish SafetyState");
            }
            last_chappe = now;
        }
        if now.duration_since(last_heartbeat) >= Duration::from_secs(1) {
            if let Err(e) = publish_heartbeat(runtime.chappe.as_ref()) {
                warn!(error = %e, "failed to publish heartbeat");
            }
            debug_status(loop_ctrl, &mut timing);
            last_heartbeat = now;
        }

        if elapsed < period {
            thread::sleep(period - elapsed);
        }
    }
}

fn phase_elapsed_us(since: Instant) -> (u64, Instant) {
    let now = Instant::now();
    let us = u64::try_from(now.duration_since(since).as_micros()).unwrap_or(u64::MAX);
    (us, now)
}

/// Wall-clock loop stats accumulated between 1 Hz heartbeats.
struct LoopTimingWindow {
    window_start: Instant,
    tick_count_start: u64,
    iterations: u32,
    tick_elapsed_max_us: u64,
    tick_elapsed_sum_us: u64,
    overruns: u32,
    refresh_frames_sum: u32,
    outer_stdin_us_sum: u64,
    outer_chappe_drain_us_sum: u64,
}

impl LoopTimingWindow {
    fn new(tick_count: u64) -> Self {
        Self {
            window_start: Instant::now(),
            tick_count_start: tick_count,
            iterations: 0,
            tick_elapsed_max_us: 0,
            tick_elapsed_sum_us: 0,
            overruns: 0,
            refresh_frames_sum: 0,
            outer_stdin_us_sum: 0,
            outer_chappe_drain_us_sum: 0,
        }
    }

    fn record_tick(
        &mut self,
        elapsed: Duration,
        period: Duration,
        refresh_frames: usize,
        outer_stdin_us: u64,
        outer_chappe_drain_us: u64,
    ) {
        self.iterations += 1;
        let us = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
        self.tick_elapsed_sum_us = self.tick_elapsed_sum_us.saturating_add(us);
        self.tick_elapsed_max_us = self.tick_elapsed_max_us.max(us);
        if elapsed > period {
            self.overruns += 1;
        }
        self.refresh_frames_sum = self
            .refresh_frames_sum
            .saturating_add(u32::try_from(refresh_frames).unwrap_or(u32::MAX));
        self.outer_stdin_us_sum = self.outer_stdin_us_sum.saturating_add(outer_stdin_us);
        self.outer_chappe_drain_us_sum = self
            .outer_chappe_drain_us_sum
            .saturating_add(outer_chappe_drain_us);
    }

    fn log_and_reset(&mut self, loop_ctrl: &mut ControlLoop<RuntimeBus>) {
        let wall_s = self.window_start.elapsed().as_secs_f64();
        if wall_s <= f64::EPSILON || self.iterations == 0 {
            *self = Self::new(loop_ctrl.tick_count());
            return;
        }
        let wall_hz = f64::from(self.iterations) / wall_s;
        let avg_us = self.tick_elapsed_sum_us / u64::from(self.iterations);
        let nominal_ticks = loop_ctrl.tick_count().saturating_sub(self.tick_count_start);
        let outer_stdin_avg_us = self.outer_stdin_us_sum / u64::from(self.iterations);
        let outer_chappe_drain_avg_us =
            self.outer_chappe_drain_us_sum / u64::from(self.iterations);
        debug!(
            configured_hz = loop_ctrl.configured_loop_hz(),
            wall_hz,
            nominal_ticks,
            tick_elapsed_avg_us = avg_us,
            tick_elapsed_max_us = self.tick_elapsed_max_us,
            overruns = self.overruns,
            refresh_frames_per_sec = f64::from(self.refresh_frames_sum) / wall_s,
            outer_stdin_avg_us,
            outer_chappe_drain_avg_us,
            "loop timing"
        );
        if let Some(phase) = loop_ctrl.take_tick_phase_averages() {
            log_tick_phase_averages(phase);
        }
        *self = Self::new(loop_ctrl.tick_count());
    }
}

fn log_tick_phase_averages(phase: TickPhaseAverages) {
    let accounted = phase.feedback_us
        + phase.gravity_us
        + phase.planner_us
        + phase.compose_us
        + phase.send_us
        + phase.chappe_us;
    debug!(
        phase_ticks = phase.ticks,
        feedback_us = phase.feedback_us,
        gravity_us = phase.gravity_us,
        planner_us = phase.planner_us,
        compose_us = phase.compose_us,
        trace_us = phase.trace_us,
        send_us = phase.send_us,
        chappe_us = phase.chappe_us,
        accounted_us = accounted,
        "tick phase timing"
    );
}

fn debug_status(loop_ctrl: &mut ControlLoop<RuntimeBus>, timing: &mut LoopTimingWindow) {
    timing.log_and_reset(loop_ctrl);
    let control_mode = loop_ctrl.control_mode();
    let supervisor = loop_ctrl.supervisor_mut();
    let operational = supervisor.mode();
    for motor in &supervisor.motors.motors {
        let address = MotorAddress::from(motor);
        if let Some(state) = supervisor.motor_states().get(&address) {
            debug!(
                joint = %motor.joint,
                interface = %motor.can_interface,
                device_id = motor.device_id,
                pos = f64::from(state.position_rad),
                vel = f64::from(state.velocity_rad_s),
                torque = f64::from(state.torque_nm),
                operational = ?operational,
                control = ?proto_control_mode(control_mode),
                "feedback"
            );
        }
    }
}
