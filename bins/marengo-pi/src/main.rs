//! Marengo Pi runtime: CAN I/O, control loop, Chappe telemetry, operator commands.

mod enable_gate;
#[cfg(test)]
mod enable_gate_tests;
mod host_metrics;
#[cfg(all(target_os = "linux", feature = "linux-i2c"))]
mod imu;
mod limit_persist;
mod overlay;
#[cfg(test)]
mod reference_busy_overlay_tests;
#[cfg(test)]
mod reference_dispatch_tests;
#[cfg(test)]
mod reference_journal_shutdown_tests;
mod reference_queue;
#[cfg(test)]
mod reference_shutdown_tests;
#[cfg(test)]
mod safety_publication_tests;
#[cfg(test)]
mod safety_receive_diagnostic_tests;
#[cfg(test)]
mod shutdown_tests;

use std::collections::BTreeSet;
use std::env;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use armee_dynamics::{check_gravity_range, GravityRangeVerdict};
use armee_proto::prost::Message;
use armee_proto::{
    ActiveReportingLeaseAction, ActiveReportingLeaseRequest, EnableRequest, Fault, FaultSeverity,
    Heartbeat, HomingComplete, MitCommandBatch, MotorStatusPollRequest,
    OperationalMode as ProtoOpMode, SafetyState, SetZeroRequest,
};
use berthier::{
    proto_control_mode, ControlLoop, ControlMode, GainOverride, LoopError, TickPhaseAverages,
};
use chappe::Bus;
use davout::{
    DavoutError, MotorBus, OperationalMode, ReferenceHandle, ReferenceTerminal, StopReport,
    DEFAULT_LEASE_TTL,
};
use enable_gate::{emit, EnableGate};
use marengo_config::{
    load_control_config, load_motors_config, resolve_config_dir, resolve_repo_root,
    resolve_urdf_path,
};
use robstride::RuntimeBus;
use tracing::{debug, error, info, warn};

use crate::limit_persist::{PersistDrainReport, PersistDrainStatus};
use crate::reference_queue::{ReferenceEvent, ReferenceQueue};

/// Stdin reference queue: Davout handles in flight, stdin commands deferred.
type PiReferenceQueue = ReferenceQueue<ReferenceHandle, PiCommand>;

/// Audit operator for stdin `home <joint>... sign-tested`.
const STDIN_REFERENCE_OPERATOR: &str = "bench";

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
    /// Plain `home`: readiness check only (no reference acquisition).
    Home,
    /// `home <joint>... sign-tested`: queue physical reference per joint.
    HomeJoints {
        joints: Vec<String>,
        sign_tested: bool,
    },
    Enable {
        operator_id: String,
        force: bool,
    },
    Disable,
    GravityOn,
    GravityOff,
    TorqueCmd {
        joint: String,
        tau_nm: f64,
    },
    ImpedanceOn,
    ImpedanceOff,
    HoldOn,
    HoldAt {
        joint: Option<String>,
        position_rad: f64,
    },
    Wave {
        joint: String,
        min_rad: f64,
        max_rad: f64,
        cycles: u32,
        half_period_sec: f64,
    },
    HoldOff,
    Status,
    Quit,
}

fn parse_command(line: &str) -> Option<PiCommand> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "home" => {
            let mut joints = Vec::new();
            let mut sign_tested = false;
            for tok in parts {
                if tok == "sign-tested" || tok == "--sign-tested" {
                    sign_tested = true;
                } else {
                    joints.push(tok.to_string());
                }
            }
            if joints.is_empty() && !sign_tested {
                Some(PiCommand::Home)
            } else {
                Some(PiCommand::HomeJoints {
                    joints,
                    sign_tested,
                })
            }
        }
        "enable" => {
            let mut force = false;
            let mut operator_id = "bench".to_string();
            for tok in parts {
                if tok == "force" || tok == "--force" {
                    force = true;
                } else if operator_id == "bench" {
                    operator_id = tok.to_string();
                }
            }
            Some(PiCommand::Enable { operator_id, force })
        }
        "disable" => Some(PiCommand::Disable),
        "gravity-on" | "gravity_on" => Some(PiCommand::GravityOn),
        "gravity-off" | "gravity_off" => Some(PiCommand::GravityOff),
        "torque-cmd" | "torque_cmd" => {
            let joint = parts.next()?.to_string();
            let tau_nm = parts.next()?.parse().ok()?;
            Some(PiCommand::TorqueCmd { joint, tau_nm })
        }
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
        "wave" => {
            let tokens: Vec<_> = parts.collect();
            match tokens.as_slice() {
                [joint, min, max, cycles] => {
                    let min_rad = min.parse().ok()?;
                    let max_rad = max.parse().ok()?;
                    let cycles = cycles.parse().ok()?;
                    Some(PiCommand::Wave {
                        joint: joint.to_string(),
                        min_rad,
                        max_rad,
                        cycles,
                        half_period_sec: 0.4,
                    })
                }
                [joint, min, max, cycles, half_period] => {
                    let min_rad = min.parse().ok()?;
                    let max_rad = max.parse().ok()?;
                    let cycles = cycles.parse().ok()?;
                    let half_period_sec = half_period.parse().ok()?;
                    Some(PiCommand::Wave {
                        joint: joint.to_string(),
                        min_rad,
                        max_rad,
                        cycles,
                        half_period_sec,
                    })
                }
                _ => {
                    eprintln!(
                        "wave usage: wave <joint> <min_rad> <max_rad> <cycles> [half_period_sec]"
                    );
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
         home                                   (readiness check only)\n  \
         home <joint> [<joint>...] sign-tested  (physical reference, one joint at a time;\n  \
         \x20                                       other commands wait; disable/quit cancel)\n  \
         enable [operator_id] [force]\n  \
         disable\n  \
         gravity-on | gravity-off\n  \
         torque-cmd <joint> <nm>\n  \
         impedance-on | impedance-off\n  \
         hold-on | hold-at [joint] <rad> | hold-off\n  \
         wave <joint> <min_rad> <max_rad> <cycles> [half_period_sec]\n  \
         status\n  \
         quit"
    );
}

fn is_configured_joint<B: MotorBus>(supervisor: &davout::Supervisor<B>, joint: &str) -> bool {
    supervisor
        .motors
        .motors
        .iter()
        .any(|motor| motor.joint == joint)
}

/// Print the stdout contract lines and mirror them as structured logs.
fn emit_reference_events(events: Vec<ReferenceEvent>) {
    for event in events {
        println!("{event}");
        match &event {
            ReferenceEvent::Current {
                joint,
                position_rad,
            } => info!(joint = %joint, position_rad, "reference acquired current"),
            ReferenceEvent::Failed { joint, message } => {
                warn!(joint = %joint, message = %message, "reference acquisition failed");
            }
            ReferenceEvent::Skipped { joint } => {
                warn!(joint = %joint, "reference skipped after earlier failure");
            }
            ReferenceEvent::DeferredDiscarded { count } => {
                warn!(
                    count,
                    "deferred stdin commands discarded on reference cancel"
                );
            }
        }
    }
}

/// While the reference queue is busy, every stdin command except
/// `home <joints>` (refused as busy), Disable and Quit (cancel) waits.
fn defers_while_referencing(cmd: &PiCommand) -> bool {
    !matches!(
        cmd,
        PiCommand::HomeJoints { .. } | PiCommand::Disable | PiCommand::Quit
    )
}

fn dispatch_stdin_command<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    gate: &mut EnableGate,
    cmd: PiCommand,
    config_dir: &Path,
) -> bool {
    if queue.is_busy() && defers_while_referencing(&cmd) {
        queue.defer(cmd);
        return true;
    }
    handle_command(loop_ctrl, queue, gate, cmd, config_dir)
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

fn handle_chappe_enable<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    request: &EnableRequest,
) -> Result<(), String> {
    if request.enable {
        // A same-tick enable must not flip ACTIVE under a queued reference.
        if queue.is_busy() {
            return Err("reference queue busy; enable refused".into());
        }
        // Never call set_homing_complete on enable — Verified is Set Zero only.
        let targets = loop_ctrl
            .supervisor()
            .resolve_enable_targets(repo_root())
            .map_err(|e| e.to_string())?;
        loop_ctrl
            .supervisor_mut()
            .enable_targets(&targets)
            .map_err(|e| e.to_string())?;
        info!(
            operator = %request.operator_id,
            target_count = targets.len(),
            targets = ?targets,
            "enable via Chappe (targeted)"
        );
    } else {
        let result = loop_ctrl.supervisor_mut().disable_all();
        emit_reference_events(queue.cancel());
        result.map_err(|e| e.to_string())?;
        loop_ctrl.set_control_mode(ControlMode::Disabled);
        info!(operator = %request.operator_id, "disable via Chappe");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn drain_chappe_commands<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    enable_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    homing_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    set_zero_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    lease_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    status_poll_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    shutdown: &AtomicBool,
) {
    // Set-zero before enable so its queue admission is visible to a same-tick
    // enable(true), which is then refused instead of flipping ACTIVE.
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        match set_zero_rx.try_recv() {
            Ok(bytes) => {
                let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
                    continue;
                };
                let Ok(request) = SetZeroRequest::decode(envelope.payload.as_slice()) else {
                    continue;
                };
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }
                if let Err(e) = handle_chappe_set_zero(loop_ctrl, queue, &request) {
                    warn!(
                        joint = %request.joint,
                        error = %e,
                        "Chappe set-zero failed"
                    );
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                warn!(
                    skipped = n,
                    "Chappe set_zero lagged; dropped oldest commands"
                );
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        match lease_rx.try_recv() {
            Ok(bytes) => {
                let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
                    continue;
                };
                let Ok(request) = ActiveReportingLeaseRequest::decode(envelope.payload.as_slice())
                else {
                    continue;
                };
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }
                if let Err(e) = handle_chappe_active_reporting_lease(loop_ctrl, &request) {
                    warn!(
                        joint = %request.joint,
                        lease_id = %request.lease_id,
                        error = %e,
                        "Chappe active-reporting lease failed"
                    );
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                warn!(
                    skipped = n,
                    "Chappe active_reporting_lease lagged; dropped oldest commands"
                );
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }
    // Collapse bursts: only the latest solicit matters before the next control tick.
    let mut status_poll_payloads: Vec<Vec<u8>> = Vec::new();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        match status_poll_rx.try_recv() {
            Ok(bytes) => status_poll_payloads.push(bytes),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                warn!(
                    skipped = n,
                    "Chappe motor_status_poll lagged; dropped oldest commands"
                );
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }
    if let Some(request) = latest_motor_status_poll(status_poll_payloads.iter().map(Vec::as_slice))
    {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        if let Err(e) = loop_ctrl.supervisor_mut().solicit_status_feedback() {
            warn!(
                operator = %request.operator_id,
                error = %e,
                "Chappe motor status poll failed"
            );
        }
    }
    while !shutdown.load(Ordering::SeqCst) {
        let Ok(bytes) = enable_rx.try_recv() else {
            break;
        };
        let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
            continue;
        };
        let Ok(request) = EnableRequest::decode(envelope.payload.as_slice()) else {
            continue;
        };
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        if let Err(e) = handle_chappe_enable(loop_ctrl, queue, &request) {
            warn!(error = %e, "Chappe enable request failed");
        }
    }
    while !shutdown.load(Ordering::SeqCst) {
        let Ok(bytes) = homing_rx.try_recv() else {
            break;
        };
        let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
            continue;
        };
        let Ok(_homing) = HomingComplete::decode(envelope.payload.as_slice()) else {
            continue;
        };
        // Operator HomingComplete / Testing Home retired — ignore wire (compat drain).
        warn!("ignoring retired HomingComplete on robot/homing (use Hardware Set Zero)");
    }
}

fn handle_chappe_active_reporting_lease<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    request: &ActiveReportingLeaseRequest,
) -> Result<(), DavoutError> {
    let action = ActiveReportingLeaseAction::try_from(request.action)
        .unwrap_or(ActiveReportingLeaseAction::Unspecified);
    match action {
        ActiveReportingLeaseAction::Acquire => {
            loop_ctrl.supervisor_mut().acquire_active_reporting_lease(
                &request.joint,
                &request.client_id,
                &request.lease_id,
                DEFAULT_LEASE_TTL,
            )
        }
        ActiveReportingLeaseAction::Renew => {
            loop_ctrl.supervisor_mut().renew_active_reporting_lease(
                &request.joint,
                &request.client_id,
                &request.lease_id,
                DEFAULT_LEASE_TTL,
            )
        }
        ActiveReportingLeaseAction::Release => loop_ctrl
            .supervisor_mut()
            .release_active_reporting_lease(&request.joint, &request.lease_id),
        ActiveReportingLeaseAction::Unspecified => Err(DavoutError::Homing {
            message: "active-reporting lease action unspecified".into(),
        }),
    }
}

/// Queue a Consul Set Zero on the shared reference queue after explicit
/// operator checks (confirm, sign-test attestation, configured joint, idle
/// queue). Admission is not verification: the queue requests the physical
/// transaction after the next tick and reports `reference <joint> ...` lines.
/// No control-mode change is forced: Berthier discards motion intent every
/// tick while reference work is pending and the transaction stops the drives.
fn handle_chappe_set_zero<B: MotorBus>(
    loop_ctrl: &ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    request: &SetZeroRequest,
) -> Result<(), String> {
    if !request.confirm {
        return Err("set-zero requires confirm=true".into());
    }
    let operator = if request.operator_id.is_empty() {
        "consul"
    } else {
        request.operator_id.as_str()
    };
    let joint = request.joint.trim().to_owned();
    let supervisor = loop_ctrl.supervisor();
    queue.admit(
        std::slice::from_ref(&joint),
        request.sign_test_passed,
        operator,
        |name| is_configured_joint(supervisor, name),
    )?;
    info!(joint = %joint, operator, "set-zero queued via Chappe");
    Ok(())
}

fn drain_testing_commands<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    testing_cmd_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    shutdown: &AtomicBool,
) {
    while !shutdown.load(Ordering::SeqCst) {
        let Ok(bytes) = testing_cmd_rx.try_recv() else {
            break;
        };
        let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
            continue;
        };
        let Ok(batch) = MitCommandBatch::decode(envelope.payload.as_slice()) else {
            continue;
        };
        // Proto ControlMode::POSITION = 4 (see marengo.proto).
        let want_position = batch.mode == 4;
        for joint in &batch.joints {
            if shutdown.load(Ordering::SeqCst) {
                return;
            }
            // Consul Wave: `wave:<joint>:<min>:<max>:<cycles>:<half_period_sec>`
            // starts Berthier in-loop triangle (continuous; no endpoint holds).
            if let Some(wave) = parse_testing_wave_command(&joint.name) {
                match loop_ctrl.start_position_wave(
                    wave.joint,
                    wave.min_rad,
                    wave.max_rad,
                    wave.cycles,
                    wave.half_period_sec,
                ) {
                    Ok(duration_sec) => {
                        info!(
                            joint = wave.joint,
                            min_rad = wave.min_rad,
                            max_rad = wave.max_rad,
                            cycles = wave.cycles,
                            half_period_sec = wave.half_period_sec,
                            duration_sec,
                            "testing position wave started"
                        );
                    }
                    Err(e) => {
                        warn!(
                            joint = %joint.name,
                            error = %e,
                            "testing position wave rejected"
                        );
                    }
                }
                continue;
            }
            let has_gain = joint.kp != 0.0 || joint.kd != 0.0 || joint.ki != 0.0 || joint.fc != 0.0;
            if has_gain {
                if let Err(error) = loop_ctrl.apply_gain_override(
                    &joint.name,
                    GainOverride {
                        kp: joint.kp,
                        kd: joint.kd,
                        ki: joint.ki,
                        fc: joint.fc,
                    },
                ) {
                    warn!(joint = %joint.name, error = %error, "testing gains rejected");
                    continue;
                }
            } else {
                // Use control.yaml impedance gains for Position mode.
                loop_ctrl.clear_gain_override(&joint.name);
            }
            if want_position {
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }
                // Re-arm first; a fresh Enable refuses the target until its
                // session feedback arrives (a later batch retries).
                let result = if loop_ctrl.control_mode() == ControlMode::Position {
                    loop_ctrl.ensure_active_for_motion().and_then(|()| {
                        loop_ctrl.set_joint_position_setpoint(joint.name.as_str(), joint.position)
                    })
                } else {
                    loop_ctrl.enter_position_hold_at(Some(joint.name.as_str()), joint.position)
                };
                if let Err(e) = result {
                    warn!(
                        joint = %joint.name,
                        position = joint.position,
                        error = %e,
                        "testing position hold rejected"
                    );
                }
            }
        }
    }
}

struct TestingWaveCommand<'a> {
    joint: &'a str,
    min_rad: f64,
    max_rad: f64,
    cycles: u32,
    half_period_sec: f64,
}

fn parse_testing_wave_command(name: &str) -> Option<TestingWaveCommand<'_>> {
    let rest = name.strip_prefix("wave:")?;
    let mut parts = rest.split(':');
    let joint = parts.next()?;
    let min_rad: f64 = parts.next()?.parse().ok()?;
    let max_rad: f64 = parts.next()?.parse().ok()?;
    let cycles: u32 = parts.next()?.parse().ok()?;
    let half_period_sec: f64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || joint.is_empty() || cycles == 0 || half_period_sec <= 0.0 {
        return None;
    }
    Some(TestingWaveCommand {
        joint,
        min_rad,
        max_rad,
        cycles,
        half_period_sec,
    })
}

fn publish_safety<B: MotorBus>(
    chappe: &Bus,
    supervisor: &davout::Supervisor<B>,
    active_fault: Option<&str>,
) -> Result<(), chappe::BusError> {
    let snapshot = supervisor.safety_snapshot();
    let mut faults: Vec<_> = snapshot
        .faults
        .iter()
        .map(|fault| {
            let mut message = DavoutError::FaultLatched {
                id: fault.id,
                class: fault.class,
                joint: fault.joint.clone(),
                message: fault.message.clone(),
            }
            .to_string();
            if let Some(first_frame) = &fault.receive.first_frame {
                message.push_str(&format!("; first receive: {first_frame}"));
            }
            Fault {
                code: "runtime".to_string(),
                message,
                severity: if fault.class == davout::FaultClass::HardwareEstop {
                    FaultSeverity::Estop
                } else {
                    FaultSeverity::Fault
                } as i32,
                joint: fault.joint.clone().unwrap_or_default(),
            }
        })
        .collect();
    if faults.is_empty() {
        if let Some(message) = active_fault {
            faults.push(Fault {
                code: "runtime".to_string(),
                message: message.to_string(),
                severity: FaultSeverity::Fault as i32,
                joint: String::new(),
            });
        }
    }
    let state = SafetyState {
        timestamp_ms: timestamp_ms(),
        mode: proto_operational_mode(supervisor.mode()),
        hardware_estop_asserted: snapshot.hardware_estop_asserted,
        software_estop_latched: !faults.is_empty(),
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

fn print_status<B: MotorBus>(loop_ctrl: &mut ControlLoop<B>, config_dir: &Path) {
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
        let (homing_state, drive_active, out_of_limits) =
            supervisor.joint_commissioning_wire(&motor.joint);
        match supervisor.joint_feedback(&motor.joint) {
            Some(state) => println!(
                "{} ({}/id{}): pos={:.4} rad vel={:.4} rad/s torque={:.4} Nm fault={:#06x} \
                 homing={} drive_active={} out_of_limits={}",
                motor.joint,
                motor.can_interface,
                motor.device_id,
                state.position_rad,
                state.velocity_rad_s,
                state.torque_nm,
                state.fault,
                homing_state,
                drive_active,
                out_of_limits,
            ),
            None => println!(
                "{} ({}/id{}): no feedback yet homing={} drive_active={} out_of_limits={}",
                motor.joint,
                motor.can_interface,
                motor.device_id,
                homing_state,
                drive_active,
                out_of_limits,
            ),
        }
    }
}

fn preflight_gravity_saturation<B: MotorBus>(loop_ctrl: &mut ControlLoop<B>) -> Result<(), ()> {
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
            GravityRangeVerdict::Near { tau_max_nm } => warn!(
                joint = %joint,
                tau_max = tau_max_nm, motor_tau_limit = *motor_tau_limit,
                "gravity torque >80% of motor limit"
            ),
            GravityRangeVerdict::Saturated { tau_max_nm } => {
                error!(
                    joint = %joint,
                    tau_max = tau_max_nm, motor_tau_limit = *motor_tau_limit,
                    "gravity saturation: tau_g exceeds motor torque limit"
                );
                saturated = true;
            }
            GravityRangeVerdict::Unevaluable(error) => {
                error!(
                    joint = %joint,
                    %error,
                    "gravity preflight: model could not be evaluated over the joint range; refusing enable"
                );
                saturated = true;
            }
        }
    }
    if saturated {
        Err(())
    } else {
        Ok(())
    }
}

fn handle_command<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    gate: &mut EnableGate,
    cmd: PiCommand,
    config_dir: &Path,
) -> bool {
    match cmd {
        PiCommand::Home => match loop_ctrl.supervisor_mut().set_homing_complete() {
            Ok(()) => println!("homing verified → Ready"),
            Err(e) => eprintln!("home failed: {e}"),
        },
        PiCommand::HomeJoints {
            joints,
            sign_tested,
        } => {
            let supervisor = loop_ctrl.supervisor();
            if let Err(message) =
                queue.admit(&joints, sign_tested, STDIN_REFERENCE_OPERATOR, |joint| {
                    is_configured_joint(supervisor, joint)
                })
            {
                println!("home failed: {message}");
            }
        }
        PiCommand::Enable { operator_id, force } => {
            if !force {
                if let Err(()) = preflight_gravity_saturation(loop_ctrl) {
                    eprintln!("enable refused: gravity saturation exceeds motor limit (use 'enable <operator> force' to override)");
                    return true;
                }
            }
            match loop_ctrl.supervisor().resolve_enable_targets(repo_root()) {
                Ok(targets) => match loop_ctrl.supervisor_mut().enable_targets(&targets) {
                    // `enabled` is printed once enable completes (EnableGate::poll).
                    Ok(()) => emit(gate.begin_enable(operator_id, targets, Instant::now())),
                    Err(e) => eprintln!("enable failed: {e}"),
                },
                Err(e) => eprintln!("enable blocked: {e}"),
            }
        }
        PiCommand::Disable => {
            // Davout's disable_all also cancels a live reference transaction.
            if let Err(e) = loop_ctrl.supervisor_mut().disable_all() {
                eprintln!("disable failed: {e}");
            }
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            emit_reference_events(queue.cancel());
            emit(gate.cancel());
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
            loop_ctrl.enter_torque_only_zero();
            println!(
                "control mode → TorqueOnly (τ_cmd≡0; operational={:?})",
                loop_ctrl.supervisor_mut().mode()
            );
        }
        PiCommand::TorqueCmd { joint, tau_nm } => match loop_ctrl.set_torque_cmd(&joint, tau_nm) {
            Ok(()) => {
                println!(
                    "τ_cmd {joint} = {tau_nm:.4} Nm (mode=TorqueOnly, operational={:?})",
                    loop_ctrl.supervisor_mut().mode()
                );
            }
            Err(e) => eprintln!("torque-cmd failed: {e}"),
        },
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
        cmd @ (PiCommand::HoldOn | PiCommand::HoldAt { .. } | PiCommand::Wave { .. }) => {
            emit(gate.submit(loop_ctrl, cmd, Instant::now()));
        }
        PiCommand::HoldOff => {
            loop_ctrl.clear_position_hold();
            loop_ctrl.set_control_mode(ControlMode::Disabled);
            emit(gate.cancel_arms("cancelled by hold-off"));
            println!("hold-off → Disabled");
        }
        PiCommand::Status => print_status(loop_ctrl, config_dir),
        PiCommand::Quit => {
            // Owner shutdown performs the mandatory live-reference cleanup.
            emit_reference_events(queue.cancel());
            return false;
        }
    }
    true
}

fn usage() {
    eprintln!(
        "marengo-pi — Pi control runtime (Berthier → Davout → SocketCAN)\n\
         Usage: marengo-pi [--config-dir PATH] [--no-stdin-ctl]\n\
         Env:  MARENGO_ROOT, MARENGO_CONFIG_DIR (default /opt/marengo/config on Pi)\n\
         Dev:  unset MARENGO_CONFIG_DIR to use <repo>/config"
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

    let reference_journal = match marengo_config::resolve_reference_journal_path(&root, &config_dir)
    {
        Ok(path) => path,
        Err(e) => {
            eprintln!("reference journal: {e}");
            std::process::exit(1);
        }
    };
    let bus = match RuntimeBus::socketcan_from_motors(&motors) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("open SocketCAN from motors.yaml: {e}");
            eprintln!("build with: cargo build -p marengo-pi --features socketcan");
            std::process::exit(1);
        }
    };

    let mut loop_ctrl = match ControlLoop::from_repo_with_physical_reference(
        &root,
        bus,
        &reference_journal,
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
    let mut set_zero_rx = chappe.subscribe("robot/set_zero");
    let mut lease_rx = chappe.subscribe("robot/active_reporting_lease");
    let mut status_poll_rx = chappe.subscribe("robot/motor_status_poll");
    let mut testing_cmd_rx = chappe.subscribe("robot/testing/mit_command_batch");
    let mut actuator_rx = chappe.subscribe(overlay::TOPIC_ACTUATOR_COMMAND);

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

    #[cfg(unix)]
    if signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&shutdown)).is_err() {
        eprintln!("failed to install SIGTERM handler");
        std::process::exit(1);
    }

    let persist_queue = overlay::ConfigPersistQueue::spawn(Arc::clone(&chappe), root.clone());
    let mut actuator_overlay =
        match overlay::ActuatorOverlay::from_config_dir(&config_dir, persist_queue) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("actuator overlay allowlist: {e}");
                std::process::exit(1);
            }
        };
    actuator_overlay.mark_limits_dirty();

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
        reference_journal = %reference_journal.display(),
        "marengo-pi starting Disabled; physical Robstride reference installed \
         (stdin `home <joint>... sign-tested`, Consul Set Zero); calibration history \
         cannot grant current reference (docs/homing.md)"
    );

    let mut runtime = ControlLoopRuntime {
        config_dir: &config_dir,
        chappe_state_hz: control.control.chappe_state_hz,
        chappe: &chappe,
        cmd_rx: &cmd_rx,
        enable_rx: &mut enable_rx,
        homing_rx: &mut homing_rx,
        set_zero_rx: &mut set_zero_rx,
        lease_rx: &mut lease_rx,
        status_poll_rx: &mut status_poll_rx,
        testing_cmd_rx: &mut testing_cmd_rx,
        actuator_rx: &mut actuator_rx,
        actuator_overlay: &mut actuator_overlay,
        shutdown: &shutdown,
    };
    run_control_loop(&mut loop_ctrl, &mut runtime);

    let outcome = finish_owner_shutdown(
        &mut loop_ctrl,
        &actuator_overlay,
        control.control.disable_on_exit,
        Duration::from_secs(5),
        #[cfg(test)]
        None,
    );
    if let ExitStopOutcome::Attempted { result, report } = &outcome.stop {
        info!(
            writes_accepted = result.is_ok(),
            ?report,
            "shutdown stop delivery evidence; physical stop unconfirmed"
        );
    }
    if let Some(reference) = &outcome.mandatory_reference {
        info!(
            ?reference,
            "mandatory reference cleanup preceded storage drain; physical stop unconfirmed"
        );
    }
    info!(
        persist_idle = outcome.persist_idle,
        status = ?outcome.persist.status,
        successful_requests = outcome.persist.successful_writes,
        failed_requests = outcome.persist.failed_writes,
        publication_failures = outcome.persist.publication_failures,
        pending_requests = outcome.persist.pending_requests,
        in_flight = outcome.persist.in_flight,
        worker_terminated = outcome.persist.worker_terminated,
        worker_error = ?outcome.persist.worker_error,
        coalesced_requests = outcome.persist.coalesced_requests,
        reference_journal = ?outcome.reference_journal,
        ?outcome,
        "owner shutdown outcome"
    );
    info!("marengo-pi stopped");
}

#[derive(Debug)]
enum ExitStopOutcome {
    Skipped,
    Attempted {
        result: Result<(), DavoutError>,
        report: Option<StopReport>,
    },
}

#[derive(Debug)]
struct ShutdownOutcome {
    persist_idle: bool,
    stop: ExitStopOutcome,
    mandatory_reference: Option<ReferenceTerminal>,
    persist: PersistDrainReport,
    reference_journal: davout::ReferenceJournalDrain,
}

#[cfg(test)]
type BeforePersistWait<'a, B> = dyn Fn(&ControlLoop<B>) + 'a;

/// Composition of the installed owner's graceful exit and its write-behind queue.
fn finish_owner_shutdown<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    actuator_overlay: &overlay::ActuatorOverlay,
    disable_on_exit: bool,
    persist_timeout: Duration,
    #[cfg(test)] before_persist_wait: Option<&BeforePersistWait<'_, B>>,
) -> ShutdownOutcome {
    loop_ctrl.inhibit_motion_for_shutdown();
    let mandatory_reference = loop_ctrl.supervisor_mut().cancel_reference_for_shutdown();
    let stop = if disable_on_exit {
        let result = if let Some(reference) = &mandatory_reference {
            let failed_writes = reference.stop.failed_writes();
            if failed_writes > 0 {
                Err(DavoutError::StopDelivery { failed_writes })
            } else {
                Ok(())
            }
        } else {
            loop_ctrl.supervisor_mut().disable_all()
        };
        if let Err(e) = &result {
            warn!(error = %e, "disable_all on shutdown failed");
        }
        let report = mandatory_reference
            .as_ref()
            .map(|reference| reference.stop.clone())
            .or_else(|| loop_ctrl.supervisor().safety_snapshot().last_stop);
        ExitStopOutcome::Attempted { result, report }
    } else {
        ExitStopOutcome::Skipped
    };

    let deadline = Instant::now()
        .checked_add(persist_timeout)
        .unwrap_or_else(Instant::now);
    actuator_overlay.close_persist_admission();
    loop_ctrl.supervisor().close_reference_journal_admission();
    #[cfg(test)]
    if let Some(observer) = before_persist_wait {
        observer(loop_ctrl);
    }
    let persist = actuator_overlay
        .close_persist_and_drain(deadline.saturating_duration_since(Instant::now()));
    let reference_journal = loop_ctrl
        .supervisor_mut()
        .drain_reference_journal_until(deadline);
    if !reference_journal.is_complete()
        || reference_journal.failed_writes > 0
        || reference_journal.uncertain_writes > 0
    {
        warn!(?reference_journal, "reference history shutdown outcome");
    }
    let persist_idle = persist.is_idle();
    match persist.status {
        PersistDrainStatus::Complete => {}
        PersistDrainStatus::CompletedWithFailures => {
            warn!(
                ?persist,
                "config persist shutdown drain completed with failures"
            );
        }
        PersistDrainStatus::TimedOut => {
            warn!(
                ?persist,
                "config persist shutdown drain timed out; work unfinished"
            );
        }
        PersistDrainStatus::WorkerFailed => {
            error!(
                ?persist,
                "config persist worker failed during shutdown drain"
            );
        }
    }
    ShutdownOutcome {
        persist_idle,
        stop,
        mandatory_reference,
        persist,
        reference_journal,
    }
}

struct ControlLoopRuntime<'a> {
    config_dir: &'a Path,
    chappe_state_hz: u32,
    chappe: &'a Arc<Bus>,
    cmd_rx: &'a Receiver<PiCommand>,
    enable_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    homing_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    set_zero_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    lease_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    status_poll_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    testing_cmd_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    actuator_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    actuator_overlay: &'a mut overlay::ActuatorOverlay,
    shutdown: &'a Arc<AtomicBool>,
}

#[tracing::instrument(skip(loop_ctrl, runtime))]
fn run_control_loop<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    runtime: &mut ControlLoopRuntime<'_>,
) {
    let period = loop_ctrl.loop_period();
    let chappe_period = Duration::from_secs_f64(1.0 / f64::from(runtime.chappe_state_hz.max(1)));
    let mut last_chappe = Instant::now();
    let mut last_heartbeat = Instant::now();
    let mut active_fault: Option<String>;
    let mut timing = LoopTimingWindow::new(loop_ctrl.tick_count());
    let mut reference_queue = PiReferenceQueue::new(format!("marengo-pi-{}", std::process::id()));
    let mut enable_gate = EnableGate::default();

    'control: while !runtime.shutdown.load(Ordering::SeqCst) {
        let tick_start = Instant::now();

        let t = tick_start;
        loop {
            if runtime.shutdown.load(Ordering::SeqCst) {
                break 'control;
            }
            // Commands deferred behind a drained reference queue run first, in order.
            let Some(cmd) = reference_queue
                .take_ready_deferred()
                .or_else(|| runtime.cmd_rx.try_recv().ok())
            else {
                break;
            };
            if runtime.shutdown.load(Ordering::SeqCst) {
                break 'control;
            }
            if !dispatch_stdin_command(
                loop_ctrl,
                &mut reference_queue,
                &mut enable_gate,
                cmd,
                runtime.config_dir,
            ) {
                runtime.shutdown.store(true, Ordering::SeqCst);
                break 'control;
            }
        }
        let (outer_stdin_us, t_next) = phase_elapsed_us(t);

        drain_chappe_commands(
            loop_ctrl,
            &mut reference_queue,
            runtime.enable_rx,
            runtime.homing_rx,
            runtime.set_zero_rx,
            runtime.lease_rx,
            runtime.status_poll_rx,
            runtime.shutdown,
        );
        if runtime.shutdown.load(Ordering::SeqCst) {
            break;
        }
        let (outer_chappe_drain_us, t_after_chappe) = phase_elapsed_us(t_next);
        drain_testing_commands(loop_ctrl, runtime.testing_cmd_rx, runtime.shutdown);
        if runtime.shutdown.load(Ordering::SeqCst) {
            break;
        }
        runtime.actuator_overlay.drain_commands_until_shutdown(
            loop_ctrl,
            runtime.config_dir,
            runtime.chappe,
            runtime.actuator_rx,
            runtime.shutdown,
        );
        let (_outer_actuator_us, _t) = phase_elapsed_us(t_after_chappe);

        if runtime.shutdown.load(Ordering::SeqCst) {
            break;
        }
        loop_ctrl.supervisor_mut().tick_active_reporting_leases();
        if runtime.shutdown.load(Ordering::SeqCst) {
            break;
        }

        active_fault = match loop_ctrl.tick(Some(runtime.chappe.as_ref())) {
            Ok(()) => None,
            Err(e) => {
                error!(error = %e, "control tick failed");
                let _ = loop_ctrl.supervisor_mut().disable_all();
                let preserve_position_hold =
                    matches!(&e, LoopError::Safety(DavoutError::CommWatchdog { .. }));
                if !preserve_position_hold {
                    loop_ctrl.set_control_mode(ControlMode::Disabled);
                }
                Some(e.to_string())
            }
        };
        emit_reference_events(reference_queue.pump(loop_ctrl.supervisor_mut()));
        // After the tick: report a completed (or refused) Enable, then retry
        // deferred Position-mode arms.
        emit(enable_gate.poll(loop_ctrl, Instant::now()));
        if runtime.shutdown.load(Ordering::SeqCst) {
            break;
        }

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
            // A healthy Disabled tick cannot clear the authority retained by Davout.
            if let Err(e) = publish_safety(
                runtime.chappe.as_ref(),
                loop_ctrl.supervisor(),
                active_fault.as_deref(),
            ) {
                warn!(error = %e, "failed to publish SafetyState");
            }
            if let Err(e) = runtime.actuator_overlay.maybe_publish_limits(
                loop_ctrl.supervisor(),
                runtime.chappe,
                timestamp_ms(),
            ) {
                warn!(error = %e, "failed to publish ActuatorLimitSnapshot");
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
    // Signal shutdown: unfinished joints are cancelled; owner shutdown performs
    // the mandatory live-reference cleanup.
    emit_reference_events(reference_queue.cancel());
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

    fn log_and_reset<B: MotorBus>(&mut self, loop_ctrl: &mut ControlLoop<B>) {
        let wall_s = self.window_start.elapsed().as_secs_f64();
        if wall_s <= f64::EPSILON || self.iterations == 0 {
            *self = Self::new(loop_ctrl.tick_count());
            return;
        }
        let wall_hz = f64::from(self.iterations) / wall_s;
        let avg_us = self.tick_elapsed_sum_us / u64::from(self.iterations);
        let nominal_ticks = loop_ctrl.tick_count().saturating_sub(self.tick_count_start);
        let outer_stdin_avg_us = self.outer_stdin_us_sum / u64::from(self.iterations);
        let outer_chappe_drain_avg_us = self.outer_chappe_drain_us_sum / u64::from(self.iterations);
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

/// Decode one Chappe envelope carrying `MotorStatusPollRequest` (testable seam).
fn decode_motor_status_poll_envelope(bytes: &[u8]) -> Option<MotorStatusPollRequest> {
    let envelope = armee_proto::Envelope::decode(bytes).ok()?;
    MotorStatusPollRequest::decode(envelope.payload.as_slice()).ok()
}

/// Collapse a burst of status-poll envelope payloads to the latest valid request.
fn latest_motor_status_poll<'a>(
    payloads: impl IntoIterator<Item = &'a [u8]>,
) -> Option<MotorStatusPollRequest> {
    let mut latest = None;
    for bytes in payloads {
        if let Some(request) = decode_motor_status_poll_envelope(bytes) {
            latest = Some(request);
        }
    }
    latest
}

fn debug_status<B: MotorBus>(loop_ctrl: &mut ControlLoop<B>, timing: &mut LoopTimingWindow) {
    timing.log_and_reset(loop_ctrl);
    let control_mode = loop_ctrl.control_mode();
    let supervisor = loop_ctrl.supervisor_mut();
    let operational = supervisor.mode();
    for motor in &supervisor.motors.motors {
        let (homing_state, drive_active, out_of_limits) =
            supervisor.joint_commissioning_wire(&motor.joint);
        if let Some(state) = supervisor.joint_feedback(&motor.joint) {
            debug!(
                joint = %motor.joint,
                interface = %motor.can_interface,
                device_id = motor.device_id,
                pos = state.position_rad,
                vel = state.velocity_rad_s,
                torque = state.torque_nm,
                homing_state,
                drive_active,
                out_of_limits,
                operational = ?operational,
                control = ?proto_control_mode(control_mode),
                "feedback"
            );
        } else {
            debug!(
                joint = %motor.joint,
                homing_state,
                drive_active,
                out_of_limits,
                operational = ?operational,
                "no feedback"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod status_poll_tests {
    use super::*;
    use armee_proto::Envelope;

    fn encode_poll(operator_id: &str, timestamp_ms: u64) -> Vec<u8> {
        let request = MotorStatusPollRequest {
            timestamp_ms,
            operator_id: operator_id.into(),
        };
        let envelope = Envelope {
            timestamp_ms,
            source_node: "gateway".into(),
            message_type: "marengo.v1.MotorStatusPollRequest".into(),
            payload: request.encode_to_vec(),
        };
        envelope.encode_to_vec()
    }

    #[test]
    fn latest_motor_status_poll_keeps_last_valid_envelope() {
        let first = encode_poll("consul-a", 1);
        let garbage = b"not-an-envelope".to_vec();
        let second = encode_poll("consul-b", 2);
        let Some(latest) =
            latest_motor_status_poll([first.as_slice(), garbage.as_slice(), second.as_slice()])
        else {
            panic!("expected latest status poll");
        };
        assert_eq!(latest.operator_id, "consul-b");
        assert_eq!(latest.timestamp_ms, 2);
    }

    #[test]
    fn decode_motor_status_poll_envelope_rejects_garbage() {
        assert!(decode_motor_status_poll_envelope(b"nope").is_none());
    }
}
