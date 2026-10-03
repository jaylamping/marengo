//! Marengo Pi runtime: CAN I/O, control loop, Chappe telemetry, operator commands.

mod enable_gate;
#[cfg(test)]
mod enable_gate_tests;
mod host_metrics;
#[cfg(all(target_os = "linux", feature = "linux-i2c"))]
mod imu;
#[cfg(test)]
mod ipc_wiring_tests;
mod limit_persist;
mod motion_owner;
#[cfg(test)]
mod motion_owner_chappe_tests;
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
#[cfg(test)]
mod stop_path_tests;
/// Berthier's feedback/fixture helpers, loaded once for every test module.
#[cfg(test)]
#[path = "../../../crates/berthier/tests/support/mod.rs"]
mod test_support;

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
    ActiveReportingLeaseAction, ActiveReportingLeaseRequest, ControlMode as ProtoControlMode,
    EnableRequest, Fault, FaultSeverity, Heartbeat, MitCommandBatch, MitJointCommand,
    MotorStatusPollRequest, OperationalMode as ProtoOpMode, SafetyState, SetZeroRequest,
};
use berthier::{
    proto_control_mode, ControlLoop, ControlMode, GainOverride, LoopError, TickPhaseAverages,
};
use chappe::topics::{
    TOPIC_ACTIVE_REPORTING_LEASE, TOPIC_ENABLE, TOPIC_HEARTBEAT, TOPIC_MOTOR_STATUS_POLL,
    TOPIC_SAFETY, TOPIC_SET_ZERO, TOPIC_TESTING_MIT_BATCH,
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
use crate::motion_owner::{
    classify_stdin, publish_audit, report_refusal, resolve_owner, CommandClass, CommandSource,
    MotionLease, MOTION_OWNER_ENV,
};
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
    /// `force`/`--force` tokens are accepted and ignored: nothing bypasses the
    /// gravity saturation preflight (docs/safety.md).
    Enable {
        operator_id: String,
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

fn parse_number<T>(token: &str, command: &str, field: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    token.parse().map_err(|_| {
        let error = format!("{command}: invalid {field} value {token:?}");
        eprintln!("{error}");
        error
    })
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
            let mut operator_id = "bench".to_string();
            for tok in parts {
                if tok == "force" || tok == "--force" {
                    continue;
                }
                if operator_id == "bench" {
                    operator_id = tok.to_string();
                }
            }
            Some(PiCommand::Enable { operator_id })
        }
        "disable" => Some(PiCommand::Disable),
        "gravity-on" | "gravity_on" => Some(PiCommand::GravityOn),
        "gravity-off" | "gravity_off" => Some(PiCommand::GravityOff),
        "torque-cmd" | "torque_cmd" => {
            let joint = parts.next()?.to_string();
            let tau_nm = parse_number::<f64>(parts.next()?, "torque-cmd", "torque_nm").ok()?;
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
                    let position_rad = parse_number::<f64>(rad, "hold-at", "position_rad").ok()?;
                    Some(PiCommand::HoldAt {
                        joint: None,
                        position_rad,
                    })
                }
                [joint, rad] => {
                    let position_rad = parse_number::<f64>(rad, "hold-at", "position_rad").ok()?;
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
                    let min_rad = parse_number::<f64>(min, "wave", "min_rad").ok()?;
                    let max_rad = parse_number::<f64>(max, "wave", "max_rad").ok()?;
                    let cycles = parse_number::<u32>(cycles, "wave", "cycles").ok()?;
                    Some(PiCommand::Wave {
                        joint: joint.to_string(),
                        min_rad,
                        max_rad,
                        cycles,
                        half_period_sec: 0.4,
                    })
                }
                [joint, min, max, cycles, half_period] => {
                    let min_rad = parse_number::<f64>(min, "wave", "min_rad").ok()?;
                    let max_rad = parse_number::<f64>(max, "wave", "max_rad").ok()?;
                    let cycles = parse_number::<u32>(cycles, "wave", "cycles").ok()?;
                    let half_period_sec =
                        parse_number::<f64>(half_period, "wave", "half_period_sec").ok()?;
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
                    "deferred stdin commands discarded when reference ended"
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
        if !queue.defer(cmd) {
            eprintln!("reference in progress: deferred command limit reached; command refused");
        }
        return true;
    }
    handle_command(loop_ctrl, queue, gate, cmd, config_dir)
}

/// Admit a stdin command against the motion lease. Stop and observe commands
/// always pass; a motion command from a non-owner stdin is refused with a
/// reason on the console and as a published event.
fn admit_stdin_command(lease: MotionLease, chappe: &Bus, cmd: &PiCommand) -> bool {
    let (class, name) = classify_stdin(cmd);
    match lease.admit(CommandSource::Stdin, class, name) {
        Ok(()) => true,
        Err(refusal) => {
            let reason = refusal.to_string();
            if name.starts_with("home") {
                // `home failed:` is the stdout contract MCP scripts wait on.
                println!("home failed: {reason}");
            } else {
                eprintln!("{reason}");
            }
            report_refusal(chappe, STDIN_REFERENCE_OPERATOR, "", name, &reason);
            false
        }
    }
}

fn read_stdin_commands(reader: impl BufRead, tx: Sender<PiCommand>) {
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("stdin read failed: {error}");
                break;
            }
        };
        let Some(cmd) = parse_command(line.trim()) else {
            continue;
        };
        if matches!(cmd, PiCommand::Quit) {
            let _ = tx.send(cmd);
            return;
        }
        if tx.send(cmd).is_err() {
            return;
        }
    }
    // Use normal shutdown so EOF also performs reference cleanup and the
    // configured drive stop.
    let _ = tx.send(PiCommand::Quit);
}

fn spawn_stdin_commands(tx: Sender<PiCommand>) {
    thread::spawn(move || read_stdin_commands(io::stdin().lock(), tx));
}

/// One non-blocking read of a Chappe command channel.
enum Polled {
    Message(Vec<u8>),
    /// The channel dropped its `n` oldest messages because this loop fell behind.
    Lagged(u64),
    Empty,
}

fn poll_channel(rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>) -> Polled {
    use tokio::sync::broadcast::error::TryRecvError;
    match rx.try_recv() {
        Ok(bytes) => Polled::Message(bytes),
        Err(TryRecvError::Lagged(skipped)) => Polled::Lagged(skipped),
        Err(TryRecvError::Empty | TryRecvError::Closed) => Polled::Empty,
    }
}

/// Decode `Envelope` → `M`; an undecodable command is logged, never silent.
fn decode_chappe_payload<M: Message + Default>(bytes: &[u8], topic: &str) -> Option<M> {
    let envelope = match armee_proto::Envelope::decode(bytes) {
        Ok(envelope) => envelope,
        Err(error) => {
            warn!(topic, %error, "undecodable Chappe envelope dropped");
            return None;
        }
    };
    match M::decode(envelope.payload.as_slice()) {
        Ok(message) => Some(message),
        Err(error) => {
            warn!(topic, %error, "undecodable Chappe payload dropped");
            None
        }
    }
}

/// A failed control tick stops every drive and discards motion intent, and the
/// stop result is reported, never swallowed. Berthier already discards intent
/// for every `LoopError::Safety` (loop.rs `tick`), so there is no hold to
/// preserve across a `CommWatchdog`; the mode is set Disabled unconditionally
/// as a second line. Returns the fault text published in `SafetyState`; when
/// the stop itself was not delivered it says so (Davout also latches
/// `StopDelivery`).
fn stop_after_tick_error<B: MotorBus>(loop_ctrl: &mut ControlLoop<B>, error: &LoopError) -> String {
    error!(error = %error, "control tick failed");
    let stop = loop_ctrl.supervisor_mut().disable_all();
    loop_ctrl.set_control_mode(ControlMode::Disabled);
    match stop {
        Ok(()) => error.to_string(),
        Err(stop_error) => {
            error!(
                error = %stop_error,
                "stop after control tick failure was NOT delivered to every drive"
            );
            format!("{error}; stop after tick failure not delivered: {stop_error}")
        }
    }
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
        preflight_gravity_saturation(loop_ctrl)
            .map_err(|()| "gravity saturation preflight refused enable".to_string())?;
        // Never call set_homing_complete on enable — Verified is Set Zero only.
        let targets = loop_ctrl
            .supervisor()
            .resolve_enable_targets(repo_root())
            .map_err(|e| e.to_string())?;
        loop_ctrl
            .supervisor_mut()
            .enable_targets(&targets)
            .map_err(|e| e.to_string())?;
        // An explicit enable is what lets later motion commands re-arm drives.
        loop_ctrl.allow_implicit_enable();
        info!(
            operator = %request.operator_id,
            target_count = targets.len(),
            targets = ?targets,
            "enable via Chappe (targeted)"
        );
    } else {
        // An operator disable stands until an explicit enable (L-berthier-28).
        loop_ctrl.forbid_implicit_enable();
        let result = loop_ctrl.supervisor_mut().disable_all();
        // Intent is discarded whether or not the stop was delivered: a failed
        // stop must never leave GravityComp/Position armed (L-marengo-pi-16).
        loop_ctrl.set_control_mode(ControlMode::Disabled);
        emit_reference_events(queue.cancel());
        result.map_err(|e| e.to_string())?;
        info!(operator = %request.operator_id, "disable via Chappe");
    }
    Ok(())
}

/// Most queued `robot/enable` messages discarded after a lag in one tick.
const LAGGED_ENABLE_DISCARD_BOUND: usize = 4096;

/// `robot/enable` dropped messages. A Disable may be among them and cannot be
/// recovered, so fail closed: stop every drive, cancel the reference queue and
/// refuse implicit re-enable. The surviving messages cannot be ordered against
/// the lost ones, so they are discarded; an operator who still wants motion
/// enables again explicitly (L-marengo-pi-06).
fn stop_after_lagged_enable<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    chappe: &Bus,
    enable_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    skipped: u64,
) {
    error!(
        skipped,
        "Chappe robot/enable lagged; a disable may have been dropped — stopping drives"
    );
    loop_ctrl.forbid_implicit_enable();
    let result = loop_ctrl.supervisor_mut().disable_all();
    emit_reference_events(queue.cancel());
    loop_ctrl.set_control_mode(ControlMode::Disabled);
    if let Err(e) = &result {
        error!(error = %e, "stop after lagged robot/enable failed");
    }
    let mut discarded = 0usize;
    for _ in 0..LAGGED_ENABLE_DISCARD_BOUND {
        if matches!(poll_channel(enable_rx), Polled::Empty) {
            break;
        }
        discarded += 1;
    }
    publish_audit(
        chappe,
        "stop_on_lag",
        result.is_ok(),
        "marengo-pi",
        "",
        &format!(
            "robot/enable lagged ({skipped} dropped, {discarded} queued discarded); drives \
             stopped fail-closed; enable explicitly to resume"
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn drain_chappe_commands<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &mut PiReferenceQueue,
    lease: MotionLease,
    chappe: &Bus,
    enable_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
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
        match poll_channel(set_zero_rx) {
            Polled::Message(bytes) => {
                let Some(request) = decode_chappe_payload::<SetZeroRequest>(&bytes, TOPIC_SET_ZERO)
                else {
                    continue;
                };
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }
                if let Err(refusal) =
                    lease.admit(CommandSource::Chappe, CommandClass::Motion, "set-zero")
                {
                    report_refusal(
                        chappe,
                        &request.operator_id,
                        &request.joint,
                        "set-zero",
                        &refusal.to_string(),
                    );
                    continue;
                }
                if let Err(e) = handle_chappe_set_zero(loop_ctrl, queue, &request) {
                    warn!(
                        joint = %request.joint,
                        error = %e,
                        "Chappe set-zero failed"
                    );
                }
            }
            Polled::Lagged(n) => {
                warn!(
                    skipped = n,
                    "Chappe set_zero lagged; dropped oldest commands"
                );
                publish_audit(
                    chappe,
                    "chappe_lagged",
                    false,
                    "marengo-pi",
                    "",
                    &format!("robot/set_zero lagged; {n} set-zero commands dropped; resend"),
                );
            }
            Polled::Empty => break,
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
        match poll_channel(enable_rx) {
            Polled::Empty => break,
            Polled::Lagged(skipped) => {
                stop_after_lagged_enable(loop_ctrl, queue, chappe, enable_rx, skipped);
                break;
            }
            Polled::Message(bytes) => {
                let Some(request) = decode_chappe_payload::<EnableRequest>(&bytes, TOPIC_ENABLE)
                else {
                    continue;
                };
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }
                // Disable is a stop: admitted from any source. Enable energises.
                let (class, name) = if request.enable {
                    (CommandClass::Motion, "enable")
                } else {
                    (CommandClass::Stop, "disable")
                };
                if let Err(refusal) = lease.admit(CommandSource::Chappe, class, name) {
                    report_refusal(chappe, &request.operator_id, "", name, &refusal.to_string());
                    continue;
                }
                if let Err(e) = handle_chappe_enable(loop_ctrl, queue, &request) {
                    warn!(error = %e, "Chappe enable request failed");
                }
            }
        }
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

/// Apply (or clear) one Testing joint's gain override. Must run after the
/// control mode has been entered: overrides exist only in Impedance/Position,
/// and Berthier refuses them elsewhere instead of silently dropping them.
fn apply_testing_gains<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    joint: &MitJointCommand,
) -> Result<(), LoopError> {
    let has_gain = joint.kp != 0.0 || joint.kd != 0.0 || joint.ki != 0.0 || joint.fc != 0.0;
    if has_gain {
        loop_ctrl.apply_gain_override(
            &joint.name,
            GainOverride {
                kp: joint.kp,
                kd: joint.kd,
                ki: joint.ki,
                fc: joint.fc,
            },
        )
    } else {
        // Use control.yaml impedance gains for Position mode.
        loop_ctrl.clear_gain_override(&joint.name);
        Ok(())
    }
}

/// Testing (`robot/testing/mit_command_batch`) is motion: it needs the motion
/// lease and an idle reference queue. Batches are refused whole, with a
/// published reason, before any gain, mode or enable side effect.
fn drain_testing_commands<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    queue: &PiReferenceQueue,
    lease: MotionLease,
    chappe: &Bus,
    testing_cmd_rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    shutdown: &AtomicBool,
) {
    const TOPIC: &str = TOPIC_TESTING_MIT_BATCH;
    while !shutdown.load(Ordering::SeqCst) {
        let bytes = match poll_channel(testing_cmd_rx) {
            Polled::Message(bytes) => bytes,
            Polled::Lagged(n) => {
                warn!(skipped = n, "Chappe testing batches lagged; dropped oldest");
                publish_audit(
                    chappe,
                    "chappe_lagged",
                    false,
                    "marengo-pi",
                    "",
                    &format!("{TOPIC} lagged; {n} testing batches dropped; resend"),
                );
                continue;
            }
            Polled::Empty => break,
        };
        let Some(batch) = decode_chappe_payload::<MitCommandBatch>(&bytes, TOPIC) else {
            continue;
        };
        if let Err(refusal) =
            lease.admit(CommandSource::Chappe, CommandClass::Motion, "testing batch")
        {
            report_refusal(chappe, "consul", "", "testing batch", &refusal.to_string());
            continue;
        }
        // Same gate as Chappe enable and stdin commands: nothing may energise,
        // retune or move a drive while a reference acquisition owns the bus.
        if queue.is_busy() || loop_ctrl.supervisor().reference_busy() {
            report_refusal(
                chappe,
                "consul",
                "",
                "testing batch",
                "testing batch refused: reference acquisition in progress",
            );
            continue;
        }
        let want_position = matches!(
            ProtoControlMode::try_from(batch.mode),
            Ok(ProtoControlMode::Position)
        );
        if want_position
            && loop_ctrl.supervisor().mode() != OperationalMode::Active
            && preflight_gravity_saturation(loop_ctrl).is_err()
        {
            report_refusal(
                chappe,
                "consul",
                "",
                "testing batch",
                "gravity saturation preflight refused motion",
            );
            continue;
        }
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
            if want_position {
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
                    continue;
                }
            }
            // Gains last: they only exist once the requested mode is entered
            // (CS19: applied first, a GravityComp → Position batch lost them).
            if let Err(error) = apply_testing_gains(loop_ctrl, joint) {
                warn!(joint = %joint.name, error = %error, "testing gains rejected");
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
    chappe.publish(TOPIC_SAFETY, "marengo-pi", "marengo.v1.SafetyState", &state)
}

fn publish_heartbeat(chappe: &Bus) -> Result<(), chappe::BusError> {
    chappe.publish(
        TOPIC_HEARTBEAT,
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
    const GRID_POINTS: usize = 5;
    const MAX_GRID_SAMPLES: usize = 1_000_000;
    let joint_names = loop_ctrl.joint_names().to_vec();
    let joint_specs: Vec<(String, f64, f64, f64)> = {
        let supervisor = loop_ctrl.supervisor();
        let mut specs = Vec::with_capacity(joint_names.len());
        for joint in &joint_names {
            let Some(_motor) = supervisor.motors.motors.iter().find(|m| &m.joint == joint) else {
                error!(
                    joint,
                    "gravity preflight: no motor configuration; refusing enable"
                );
                return Err(());
            };
            let Some(policy) = supervisor.joint_limit_policy(joint) else {
                error!(
                    joint,
                    "gravity preflight: no live limit policy; refusing enable"
                );
                return Err(());
            };
            let Some(feedback) = supervisor.joint_feedback(joint) else {
                error!(
                    joint,
                    "gravity preflight: no measured position for live envelope"
                );
                return Err(());
            };
            let (q_min, q_max) =
                armee_kinematics::effective_command_bounds(policy, feedback.position_rad, 0.0);
            if !q_min.is_finite() || !q_max.is_finite() || q_min > q_max {
                error!(
                    joint,
                    q_min, q_max, "gravity preflight: invalid live envelope"
                );
                return Err(());
            }
            specs.push((joint.clone(), q_min, q_max, policy.tau_ff_max));
        }
        specs
    };
    let exponent = match u32::try_from(joint_specs.len()) {
        Ok(exponent) => exponent,
        Err(_) => {
            error!("gravity preflight: too many joints to sweep");
            return Err(());
        }
    };
    let Some(sample_count) = GRID_POINTS.checked_pow(exponent) else {
        error!("gravity preflight: sweep size overflow");
        return Err(());
    };
    if sample_count > MAX_GRID_SAMPLES {
        error!(
            sample_count,
            "gravity preflight: sweep exceeds bounded work"
        );
        return Err(());
    }
    let mut maxima = vec![0.0_f64; joint_specs.len()];
    let mut q = vec![0.0_f64; joint_specs.len()];
    for sample in 0..sample_count {
        let mut digits = sample;
        for (i, (_, lower, upper, _)) in joint_specs.iter().enumerate() {
            let point = digits % GRID_POINTS;
            digits /= GRID_POINTS;
            q[i] = lower + (upper - lower) * point as f64 / (GRID_POINTS - 1) as f64;
        }
        let tau = match loop_ctrl.preview_gravity_torques(&q) {
            Ok(tau) => tau,
            Err(error) => {
                error!(%error, "gravity preflight: model could not be evaluated; refusing enable");
                return Err(());
            }
        };
        for (i, value) in tau.iter().enumerate() {
            if !value.is_finite() {
                error!(joint = %joint_names[i], "gravity preflight: non-finite torque");
                return Err(());
            }
            maxima[i] = maxima[i].max(value.abs());
        }
    }
    let mut saturated = false;
    for ((joint, _, _, limit), tau_max_nm) in joint_specs.iter().zip(maxima) {
        if !limit.is_finite() || *limit <= 0.0 || tau_max_nm > *limit {
            error!(
                joint,
                tau_max_nm, limit, "gravity saturation: refusing enable"
            );
            saturated = true;
        } else if tau_max_nm > 0.8 * *limit {
            warn!(
                joint,
                tau_max_nm, limit, "gravity torque >80% of motor limit"
            );
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
        PiCommand::Enable { operator_id } => {
            if let Err(()) = preflight_gravity_saturation(loop_ctrl) {
                eprintln!("enable refused: gravity saturation preflight failed closed");
                return true;
            }
            match loop_ctrl.supervisor().resolve_enable_targets(repo_root()) {
                Ok(targets) => match loop_ctrl.supervisor_mut().enable_targets(&targets) {
                    // `enabled` is printed once enable completes (EnableGate::poll).
                    Ok(()) => {
                        // An explicit enable lets later motion commands re-arm drives.
                        loop_ctrl.allow_implicit_enable();
                        emit(gate.begin_enable(operator_id, targets, Instant::now()));
                    }
                    Err(e) => eprintln!("enable failed: {e}"),
                },
                Err(e) => eprintln!("enable blocked: {e}"),
            }
        }
        PiCommand::Disable => {
            // An operator disable stands until an explicit enable (L-berthier-28).
            loop_ctrl.forbid_implicit_enable();
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
         Usage: marengo-pi [--config-dir PATH] [--no-stdin-ctl] [--motion-owner stdin|chappe]\n\
         Motion owner: exactly one command source may enable/move the robot. Default chappe\n\
         (Consul via the gateway); scripted stdin sessions claim it with `--motion-owner stdin`\n\
         (or {MOTION_OWNER_ENV}=stdin). Disable/stop is accepted from either source.\n\
         Env:  MARENGO_ROOT, MARENGO_CONFIG_DIR (default /opt/marengo/config on Pi)\n\
         Dev:  unset MARENGO_CONFIG_DIR to use <repo>/config"
    );
}

struct CliArgs {
    config_dir: Option<PathBuf>,
    stdin_ctl: bool,
    motion_owner: Option<String>,
}

fn parse_args() -> CliArgs {
    let mut args = env::args().skip(1);
    let mut cli = CliArgs {
        config_dir: None,
        stdin_ctl: true,
        motion_owner: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config-dir" => {
                let Some(path) = args.next() else {
                    eprintln!("--config-dir requires a path");
                    std::process::exit(1);
                };
                cli.config_dir = Some(PathBuf::from(path));
            }
            "--no-stdin-ctl" => cli.stdin_ctl = false,
            "--motion-owner" => {
                let Some(owner) = args.next() else {
                    eprintln!("--motion-owner requires stdin or chappe");
                    std::process::exit(1);
                };
                cli.motion_owner = Some(owner);
            }
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
    cli
}

fn main() {
    let cli = parse_args();
    let stdin_ctl = cli.stdin_ctl;
    let motion_owner = match resolve_owner(
        cli.motion_owner.as_deref(),
        env::var(MOTION_OWNER_ENV).ok().as_deref(),
    ) {
        Ok(owner) => owner,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    if motion_owner == CommandSource::Stdin && !stdin_ctl {
        eprintln!("--motion-owner stdin requires stdin control (drop --no-stdin-ctl)");
        std::process::exit(1);
    }
    let motion = MotionLease::new(motion_owner);
    let root = repo_root();
    if let Some(dir) = cli.config_dir {
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
    match resolve_chappe_socket(std::env::var_os("MARENGO_CHAPPE_SOCKET")) {
        Some(socket_path) => {
            match chappe::ipc::IpcFanout::spawn_client(socket_path.clone(), (*chappe).clone()) {
                Ok(fanout) => {
                    chappe.set_ipc_fanout(fanout);
                    info!(path = %socket_path.display(), "Chappe IPC fanout enabled");
                }
                Err(e) => warn!(error = %e, "Chappe IPC fanout disabled"),
            }
        }
        // Explicit, not a quiet headless run: without the socket the runtime
        // moves motors with no gateway command/telemetry path (L-marengo-pi-08).
        None => warn!(
            "MARENGO_CHAPPE_SOCKET is unset: Chappe IPC fanout disabled; \
             Consul commands and gateway telemetry are unavailable while motors run"
        ),
    }
    let mut enable_rx = chappe.subscribe(TOPIC_ENABLE);
    let mut set_zero_rx = chappe.subscribe(TOPIC_SET_ZERO);
    let mut lease_rx = chappe.subscribe(TOPIC_ACTIVE_REPORTING_LEASE);
    let mut status_poll_rx = chappe.subscribe(TOPIC_MOTOR_STATUS_POLL);
    let mut testing_cmd_rx = chappe.subscribe(TOPIC_TESTING_MIT_BATCH);
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
        eprintln!(
            "motion owner: {motion_owner}{}",
            if motion_owner == CommandSource::Stdin {
                ""
            } else {
                " (stdin may only disable/stop; restart with --motion-owner stdin to command from stdin)"
            }
        );
    }

    info!(
        motion_owner = %motion_owner,
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
        set_zero_rx: &mut set_zero_rx,
        lease_rx: &mut lease_rx,
        status_poll_rx: &mut status_poll_rx,
        testing_cmd_rx: &mut testing_cmd_rx,
        actuator_rx: &mut actuator_rx,
        actuator_overlay: &mut actuator_overlay,
        shutdown: &shutdown,
        motion,
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
    // Trace flush is outside the tick by construction (L-berthier-07).
    loop_ctrl.flush_position_trace();
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
    set_zero_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    lease_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    status_poll_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    testing_cmd_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    actuator_rx: &'a mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    actuator_overlay: &'a mut overlay::ActuatorOverlay,
    shutdown: &'a Arc<AtomicBool>,
    /// Process-lifetime claim on motion; see [`motion_owner`].
    motion: MotionLease,
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
    runtime.actuator_overlay.set_motion_lease(runtime.motion);

    'control: while !runtime.shutdown.load(Ordering::SeqCst) {
        let tick_start = Instant::now();

        let t = tick_start;
        loop {
            if runtime.shutdown.load(Ordering::SeqCst) {
                break 'control;
            }
            // Commands deferred behind a drained reference queue were admitted
            // when first received; they run first, in order.
            let cmd = match reference_queue.take_ready_deferred() {
                Some(cmd) => cmd,
                None => match runtime.cmd_rx.try_recv() {
                    Ok(cmd) => {
                        if !admit_stdin_command(runtime.motion, runtime.chappe.as_ref(), &cmd) {
                            continue;
                        }
                        cmd
                    }
                    Err(_) => break,
                },
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
            runtime.motion,
            runtime.chappe.as_ref(),
            runtime.enable_rx,
            runtime.set_zero_rx,
            runtime.lease_rx,
            runtime.status_poll_rx,
            runtime.shutdown,
        );
        if runtime.shutdown.load(Ordering::SeqCst) {
            break;
        }
        let (outer_chappe_drain_us, t_after_chappe) = phase_elapsed_us(t_next);
        drain_testing_commands(
            loop_ctrl,
            &reference_queue,
            runtime.motion,
            runtime.chappe.as_ref(),
            runtime.testing_cmd_rx,
            runtime.shutdown,
        );
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
            Err(e) => Some(stop_after_tick_error(loop_ctrl, &e)),
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

/// Resolve the Chappe IPC socket from an explicit env value (L-marengo-pi-08).
///
/// Env-only with no silent default: `None` means the runtime moves motors
/// with no gateway command/telemetry path, which the caller must log loudly.
/// Takes the env value as a parameter so the no-default contract is
/// unit-testable without touching the process environment.
#[cfg(unix)]
fn resolve_chappe_socket(env: Option<std::ffi::OsString>) -> Option<PathBuf> {
    env.map(PathBuf::from)
}

/// Wall-clock loop stats accumulated between 1 Hz heartbeats.
struct LoopTimingWindow {
    window_start: Instant,
    tick_count_start: u64,
    iterations: u32,
    tick_elapsed_max_us: u64,
    tick_elapsed_sum_us: u64,
    overruns: u32,
    /// Lifetime overruns across windows (L-marengo-pi-12): the per-window
    /// count alone is debug-only and resets every second.
    total_overruns: u64,
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
            total_overruns: 0,
            refresh_frames_sum: 0,
            outer_stdin_us_sum: 0,
            outer_chappe_drain_us_sum: 0,
        }
    }

    /// Lifetime tick overruns (tick wall time exceeded the period).
    #[cfg(test)]
    fn total_overruns(&self) -> u64 {
        self.total_overruns
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
            self.total_overruns = self.total_overruns.saturating_add(1);
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
            total_overruns = self.total_overruns,
            refresh_frames_per_sec = f64::from(self.refresh_frames_sum) / wall_s,
            outer_stdin_avg_us,
            outer_chappe_drain_avg_us,
            "loop timing"
        );
        if let Some(phase) = loop_ctrl.take_tick_phase_averages() {
            log_tick_phase_averages(phase);
        }
        let total_overruns = self.total_overruns;
        *self = Self::new(loop_ctrl.tick_count());
        self.total_overruns = total_overruns;
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
    // M06 budget evidence at 1 Hz: tick overruns and dropped-telemetry
    // counts alongside the phase averages (L-marengo-pi-12, L-berthier-24).
    debug!(
        tick_overruns = loop_ctrl.tick_overruns(),
        telemetry_failures = loop_ctrl.telemetry_failures(),
        "loop budget counters"
    );
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

#[cfg(test)]
mod stdin_command_tests {
    use std::io::Cursor;
    use std::sync::mpsc;

    use super::{parse_command, parse_number, read_stdin_commands, PiCommand};

    #[test]
    fn invalid_numeric_commands_report_a_parse_error_and_are_rejected() {
        assert!(parse_number::<f64>("abc", "torque-cmd", "torque_nm").is_err());
        assert!(parse_command("torque-cmd joint abc").is_none());
        assert!(parse_command("wave joint 0 abc 2").is_none());
    }

    #[test]
    fn stdin_eof_enqueues_quit_for_normal_shutdown() {
        let (tx, rx) = mpsc::channel();
        read_stdin_commands(Cursor::new("status\n"), tx);

        assert!(matches!(rx.recv(), Ok(PiCommand::Status)));
        assert!(matches!(rx.recv(), Ok(PiCommand::Quit)));
    }

    #[test]
    fn explicit_quit_does_not_enqueue_a_second_shutdown_command() {
        let (tx, rx) = mpsc::channel();
        read_stdin_commands(Cursor::new("quit\n"), tx);

        assert!(matches!(rx.recv(), Ok(PiCommand::Quit)));
        assert!(rx.try_recv().is_err());
    }
}
