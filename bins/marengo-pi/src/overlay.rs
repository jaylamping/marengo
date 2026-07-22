//! Actuator harness overlay: Chappe tuning → Berthier `GainOverride` + optional control.yaml persist.
//!
//! Runtime MIT uses the existing Testing-page gain path (kp/kd/ki/fc only). It never
//! overrides position, velocity, or torque_ff — those stay under Berthier's planner /
//! gravity law so live control quality is preserved.
//!
//! # ConfigOverlay persist
//!
//! `persist=true` applies the draft to live memory on the control thread, then queues an
//! async YAML write on a background worker (same class of hazard as `PositionTrace::flush`
//! — never block the 200 Hz path on SD I/O). Bursts coalesce: latest draft wins.
//!
//! **Trust boundary:** anyone who can publish `robot/actuator/command` already has
//! enable/testing authority on this Chappe bus. `persist=true` is the durable-write
//! escalation within that same trust — not a separate auth gate.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(test)]
use std::time::Instant;

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command, ActionEvent, ActuatorCommand, ActuatorLimitSnapshot, JointActuatorLimit,
    LimitPatchCommand, OperatorCommand, PersistStatus, TuningChange, TuningChangeEvent,
    TuningTier,
};
use berthier::{ControlLoop, ControlMode, GainOverride};
use chappe::Bus;
use davout::{MotorBus, Supervisor};
use marengo_config::{
    apply_joint_config_param, load_command_joint_allowlist_from, motor_type_key,
    profile_content_revision, resolve_command_joint, validate_joint_gains_against_motor_type,
    write_control_config_from, write_motors_and_control, CommandJointAllowlist, ControlConfigFile,
    LimitPatch, MotorsConfigFile,
};
use thiserror::Error;
use tracing::{info, warn};

pub const TOPIC_ACTUATOR_COMMAND: &str = "robot/actuator/command";
pub const TOPIC_ACTUATOR_LIMITS: &str = "robot/actuator/limits";
pub const TOPIC_AUDIT_TUNING: &str = "robot/audit/tuning";
pub const TOPIC_AUDIT_ACTION: &str = "robot/audit/action";

static AUDIT_REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("joint not command-eligible: {0}")]
    NotWired(String),
    #[error("missing ActuatorCommand")]
    MissingCommand,
    #[error("unsupported tuning tier")]
    UnsupportedTier,
    #[error("unsupported tuning param: {0}")]
    UnsupportedParam(String),
    #[error("non-finite tuning value for {0}")]
    NonFinite(String),
    #[error("persist queue unavailable: {0}")]
    PersistQueue(String),
    #[error("config: {0}")]
    Config(#[from] marengo_config::ConfigError),
    #[error("chappe: {0}")]
    Chappe(#[from] chappe::BusError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayOutcome {
    Tuning(TuningChangeEvent),
    Action(ActionEvent),
}

/// Coalescing background writer for `control.yaml` (never runs on the control tick).
pub struct ConfigPersistQueue {
    pending: Arc<Mutex<PersistSlot>>,
    wake_tx: SyncSender<()>,
}

struct PersistSlot {
    request: Option<PersistRequest>,
    writing: bool,
}

struct PersistRequest {
    config_dir: PathBuf,
    /// When set, write motors.yaml + control.yaml atomically; otherwise control-only.
    motors: Option<MotorsConfigFile>,
    control: ControlConfigFile,
    timestamp_ms: u64,
    session_id: String,
    operator_id: String,
    joint: String,
    param: String,
}

impl ConfigPersistQueue {
    /// Spawn a worker that serializes YAML writes; latest enqueued draft wins.
    pub fn spawn(chappe: Arc<Bus>, shutdown: Arc<AtomicBool>) -> Self {
        let (wake_tx, wake_rx) = sync_channel::<()>(1);
        let pending = Arc::new(Mutex::new(PersistSlot {
            request: None,
            writing: false,
        }));
        let pending_worker = Arc::clone(&pending);
        thread::spawn(move || persist_worker(pending_worker, wake_rx, chappe, shutdown));
        Self { pending, wake_tx }
    }

    /// Queue a durable write. Replaces any not-yet-started request (coalesce).
    ///
    /// Fail-closed: if the worker is dead or the lock is poisoned, returns `Err` so the
    /// caller must not claim persist succeeded (and should not apply live for persist).
    fn enqueue(&self, request: PersistRequest) -> Result<(), OverlayError> {
        {
            let mut slot = self
                .pending
                .lock()
                .map_err(|_| OverlayError::PersistQueue("lock poisoned".into()))?;
            slot.request = Some(request);
        }
        match self.wake_tx.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Ok(()),
            Err(TrySendError::Disconnected(())) => {
                Err(OverlayError::PersistQueue("worker disconnected".into()))
            }
        }
    }

    /// True while a write is queued or in flight (restart must drain first).
    pub fn is_busy(&self) -> bool {
        self.pending
            .lock()
            .map(|slot| slot.request.is_some() || slot.writing)
            .unwrap_or(true)
    }

    /// Block until the persist queue is idle or `timeout` elapses.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        #[cfg(test)]
        let start = Instant::now();
        #[cfg(not(test))]
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if !self.is_busy() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[cfg(test)]
    fn wait_idle_for_test(&self, timeout: Duration) -> bool {
        self.wait_idle(timeout)
    }

    #[cfg(test)]
    fn pending_count_for_test(&self) -> usize {
        self.pending
            .lock()
            .map(|s| usize::from(s.request.is_some()))
            .unwrap_or(0)
    }
}

fn persist_worker(
    pending: Arc<Mutex<PersistSlot>>,
    wake_rx: Receiver<()>,
    chappe: Arc<Bus>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match wake_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(()) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        while wake_rx.try_recv().is_ok() {}

        loop {
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            let request = {
                let Ok(mut slot) = pending.lock() else {
                    return;
                };
                let Some(req) = slot.request.take() else {
                    break;
                };
                slot.writing = true;
                req
            };

            let write_result = match &request.motors {
                Some(motors) => {
                    write_motors_and_control(&request.config_dir, motors, &request.control)
                }
                None => write_control_config_from(&request.config_dir, &request.control),
            };
            if let Ok(mut slot) = pending.lock() {
                slot.writing = false;
            }

            match write_result {
                Ok(()) => {
                    info!(
                        joint = %request.joint,
                        param = %request.param,
                        session = %request.session_id,
                        persist = true,
                        "config persist write succeeded"
                    );
                    let config_revision =
                        profile_content_revision(&request.config_dir).unwrap_or_default();
                    let event = ActionEvent {
                        timestamp_ms: request.timestamp_ms,
                        session_id: request.session_id,
                        operator_id: request.operator_id,
                        joint: request.joint,
                        action: if request.param == "limit_patch" {
                            "limit_patch".to_string()
                        } else {
                            "config_persist".to_string()
                        },
                        revision: AUDIT_REVISION.fetch_add(1, Ordering::Relaxed),
                        accepted: true,
                        reject_reason: "disk write completed".to_string(),
                        persist_status: PersistStatus::Durable as i32,
                        config_revision,
                    };
                    let _ = publish_action_event(&chappe, &event);
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        joint = %request.joint,
                        param = %request.param,
                        session = %request.session_id,
                        persist = true,
                        "config persist write FAILED; live config may differ from disk"
                    );
                    let event = ActionEvent {
                        timestamp_ms: request.timestamp_ms,
                        session_id: request.session_id,
                        operator_id: request.operator_id,
                        joint: request.joint,
                        action: if request.param == "limit_patch" {
                            "limit_patch".to_string()
                        } else {
                            "config_persist".to_string()
                        },
                        revision: AUDIT_REVISION.fetch_add(1, Ordering::Relaxed),
                        accepted: false,
                        reject_reason: format!(
                            "async config write failed after live apply: {e}"
                        ),
                        persist_status: PersistStatus::Failed as i32,
                        config_revision: String::new(),
                    };
                    let _ = publish_action_event(&chappe, &event);
                }
            }
        }
    }
}

pub struct ActuatorOverlay {
    limits_dirty: bool,
    allowlist: CommandJointAllowlist,
    persist: ConfigPersistQueue,
}

impl ActuatorOverlay {
    pub fn new(allowlist: CommandJointAllowlist, persist: ConfigPersistQueue) -> Self {
        Self {
            limits_dirty: true,
            allowlist,
            persist,
        }
    }

    pub fn from_config_dir(
        config_dir: &Path,
        persist: ConfigPersistQueue,
    ) -> Result<Self, OverlayError> {
        Ok(Self::new(
            load_command_joint_allowlist_from(config_dir)?,
            persist,
        ))
    }

    pub fn mark_limits_dirty(&mut self) {
        self.limits_dirty = true;
    }

    pub fn drain_commands<B: MotorBus>(
        &mut self,
        loop_ctrl: &mut ControlLoop<B>,
        config_dir: &Path,
        chappe: &Arc<Bus>,
        rx: &mut tokio::sync::broadcast::Receiver<Vec<u8>>,
    ) {
        loop {
            match rx.try_recv() {
                Ok(bytes) => {
                    let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
                        warn!("actuator overlay: undecodable Envelope dropped");
                        continue;
                    };
                    let Ok(operator) = OperatorCommand::decode(envelope.payload.as_slice()) else {
                        warn!("actuator overlay: undecodable OperatorCommand dropped");
                        continue;
                    };
                    match self.apply_operator_command(loop_ctrl, config_dir, &operator) {
                        Ok(outcomes) => {
                            for outcome in outcomes {
                                if let Err(e) = publish_outcome(chappe, &outcome) {
                                    warn!(error = %e, "failed to publish overlay audit event");
                                }
                                match &outcome {
                                    OverlayOutcome::Tuning(_) => self.limits_dirty = true,
                                    OverlayOutcome::Action(event)
                                        if event.action == "limit_patch" && event.accepted =>
                                    {
                                        self.limits_dirty = true;
                                    }
                                    OverlayOutcome::Action(_) => {}
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                error = %e,
                                session = %operator.session_id,
                                "actuator overlay rejected command"
                            );
                            if let Some(cmd) = operator.command.as_ref() {
                                let event = action_event(
                                    &operator,
                                    cmd.joint.as_str(),
                                    action_label(cmd),
                                    false,
                                    Some(e.to_string()),
                                    PersistStatus::NotApplicable,
                                    String::new(),
                                );
                                let _ = publish_action_event(chappe, &event);
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    warn!(
                        skipped = n,
                        "actuator overlay: broadcast lagged; commands dropped"
                    );
                    let event = ActionEvent {
                        timestamp_ms: 0,
                        session_id: String::new(),
                        operator_id: String::new(),
                        joint: String::new(),
                        action: "tuning".to_string(),
                        revision: AUDIT_REVISION.fetch_add(1, Ordering::Relaxed),
                        accepted: false,
                        reject_reason: format!("broadcast lagged; skipped {n} actuator commands"),
                        persist_status: PersistStatus::NotApplicable as i32,
                        config_revision: String::new(),
                    };
                    let _ = publish_action_event(chappe, &event);
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
            }
        }
    }

    pub fn apply_operator_command<B: MotorBus>(
        &mut self,
        loop_ctrl: &mut ControlLoop<B>,
        config_dir: &Path,
        operator: &OperatorCommand,
    ) -> Result<Vec<OverlayOutcome>, OverlayError> {
        let cmd = operator
            .command
            .as_ref()
            .ok_or(OverlayError::MissingCommand)?;
        let joint = resolve_command_joint(&cmd.joint, &self.allowlist)
            .ok_or_else(|| OverlayError::NotWired(cmd.joint.clone()))?
            .to_string();

        match cmd.payload {
            Some(actuator_command::Payload::Tuning(ref tuning)) => {
                self.apply_tuning_change(loop_ctrl, config_dir, operator, &joint, tuning)
            }
            Some(actuator_command::Payload::LimitPatch(ref patch)) => {
                self.apply_limit_patch_command(loop_ctrl, config_dir, operator, &joint, patch)
            }
            Some(actuator_command::Payload::Enable(_))
            | Some(actuator_command::Payload::Mode(_))
            | Some(actuator_command::Payload::Jog(_))
            | Some(actuator_command::Payload::Hold(_))
            | Some(actuator_command::Payload::Preset(_)) => {
                let event = action_event(
                    operator,
                    &joint,
                    action_label(cmd),
                    false,
                    Some("motion commands gated until motion unlock".to_string()),
                    PersistStatus::NotApplicable,
                    String::new(),
                );
                Ok(vec![OverlayOutcome::Action(event)])
            }
            None => Err(OverlayError::MissingCommand),
        }
    }

    /// Drain write-behind before process exit / restart.
    pub fn wait_persist_idle(&self, timeout: Duration) -> bool {
        self.persist.wait_idle(timeout)
    }

    pub fn maybe_publish_limits<B: MotorBus>(
        &mut self,
        supervisor: &Supervisor<B>,
        chappe: &Arc<Bus>,
        timestamp_ms: u64,
    ) -> Result<(), chappe::BusError> {
        if !self.limits_dirty {
            return Ok(());
        }
        let snapshot = build_limit_snapshot(supervisor, &self.allowlist, timestamp_ms);
        chappe.publish(
            TOPIC_ACTUATOR_LIMITS,
            "marengo-pi",
            "marengo.v1.ActuatorLimitSnapshot",
            &snapshot,
        )?;
        self.limits_dirty = false;
        Ok(())
    }
}

impl ActuatorOverlay {
    fn apply_tuning_change<B: MotorBus>(
        &self,
        loop_ctrl: &mut ControlLoop<B>,
        config_dir: &Path,
        operator: &OperatorCommand,
        joint: &str,
        tuning: &TuningChange,
    ) -> Result<Vec<OverlayOutcome>, OverlayError> {
        match TuningTier::try_from(tuning.tier) {
            Ok(TuningTier::RuntimeMit) => {
                let before = runtime_param_before(loop_ctrl, joint, &tuning.param)?;
                apply_runtime_param(loop_ctrl, joint, &tuning.param, tuning.value)?;
                let after = runtime_param_before(loop_ctrl, joint, &tuning.param)?;
                Ok(vec![OverlayOutcome::Tuning(tuning_event(
                    operator, joint, tuning, before, after,
                ))])
            }
            Ok(TuningTier::ConfigOverlay) => {
                // Clone-then-commit: never leave live config mutated if validate/enqueue fails.
                let mut draft = loop_ctrl.supervisor().control.clone();
                let entry = draft
                    .control
                    .joints
                    .get_mut(joint)
                    .ok_or_else(|| OverlayError::NotWired(joint.to_string()))?;
                let before = apply_joint_config_param(entry, &tuning.param, tuning.value)?;
                validate_joint_gains_against_motor_type(&draft, joint)?;

                if tuning.persist {
                    // Trust boundary: publishers on robot/actuator/command already hold
                    // enable/testing authority; persist=true is durable-write escalation
                    // in that same Chappe trust — not a separate auth check.
                    self.persist.enqueue(PersistRequest {
                        config_dir: config_dir.to_path_buf(),
                        motors: None,
                        control: draft.clone(),
                        timestamp_ms: operator.timestamp_ms,
                        session_id: operator.session_id.clone(),
                        operator_id: operator.operator_id.clone(),
                        joint: joint.to_string(),
                        param: tuning.param.clone(),
                    })?;
                    warn!(
                        joint = %joint,
                        param = %tuning.param,
                        session = %operator.session_id,
                        persist = true,
                        "ConfigOverlay persist=true queued (async SD write; live applied next)"
                    );
                }

                // Live-first after enqueue succeeds so we never claim persist when the
                // queue is dead. Background write failures emit ActionEvent accepted=false.
                loop_ctrl.supervisor_mut().control = draft;

                let mut outcomes = vec![OverlayOutcome::Tuning(tuning_event(
                    operator,
                    joint,
                    tuning,
                    before,
                    tuning.value,
                ))];
                if tuning.persist {
                    outcomes.push(OverlayOutcome::Action(action_event(
                        operator,
                        joint,
                        "config_persist",
                        true,
                        Some(
                            "queued async control.yaml write; live config already applied"
                                .to_string(),
                        ),
                        PersistStatus::Pending,
                        String::new(),
                    )));
                }
                Ok(outcomes)
            }
            _ => Err(OverlayError::UnsupportedTier),
        }
    }

    fn apply_limit_patch_command<B: MotorBus>(
        &self,
        loop_ctrl: &mut ControlLoop<B>,
        config_dir: &Path,
        operator: &OperatorCommand,
        joint: &str,
        patch_cmd: &LimitPatchCommand,
    ) -> Result<Vec<OverlayOutcome>, OverlayError> {
        let current_revision = profile_content_revision(config_dir)?;
        if !patch_cmd.expected_revision.is_empty()
            && patch_cmd.expected_revision != current_revision
        {
            return Err(OverlayError::Config(marengo_config::ConfigError::Parse {
                path: config_dir.to_path_buf(),
                message: format!(
                    "profile revision mismatch: expected {}, found {current_revision}",
                    patch_cmd.expected_revision
                ),
            }));
        }
        let patch = LimitPatch {
            joint: joint.to_string(),
            position_lower_rad: patch_cmd.position_lower_rad,
            position_upper_rad: patch_cmd.position_upper_rad,
            torque_limit_nm: patch_cmd.torque_limit_nm,
            position_soft_lower_rad: patch_cmd.position_soft_lower_rad,
            position_soft_upper_rad: patch_cmd.position_soft_upper_rad,
            velocity_max_rad_s: patch_cmd.velocity_max_rad_s,
        };
        loop_ctrl
            .supervisor_mut()
            .apply_limit_patch(&patch)
            .map_err(|e| OverlayError::Config(marengo_config::ConfigError::Parse {
                path: config_dir.to_path_buf(),
                message: e.to_string(),
            }))?;

        let motors = loop_ctrl.supervisor().motors.clone();
        let control = loop_ctrl.supervisor().control.clone();
        self.persist.enqueue(PersistRequest {
            config_dir: config_dir.to_path_buf(),
            motors: Some(motors),
            control,
            timestamp_ms: operator.timestamp_ms,
            session_id: operator.session_id.clone(),
            operator_id: operator.operator_id.clone(),
            joint: joint.to_string(),
            param: "limit_patch".into(),
        })?;
        warn!(
            joint = %joint,
            session = %operator.session_id,
            "limit_patch applied live; async motors+control persist queued"
        );
        Ok(vec![OverlayOutcome::Action(action_event(
            operator,
            joint,
            "limit_patch",
            true,
            Some("applied in memory; persist pending".to_string()),
            PersistStatus::Pending,
            current_revision,
        ))])
    }
}

fn baseline_gain_override<B: MotorBus>(loop_ctrl: &ControlLoop<B>, joint: &str) -> GainOverride {
    if let Some(ov) = loop_ctrl.gain_override(joint) {
        return ov.clone();
    }
    let cfg = loop_ctrl.supervisor().control.control.joints.get(joint);
    GainOverride {
        kp: cfg.map(|c| c.impedance.kp).unwrap_or(0.0),
        kd: cfg.map(|c| c.impedance.kd).unwrap_or(0.0),
        ki: cfg.map(|c| c.impedance.ki).unwrap_or(0.0),
        fc: cfg.map(|c| c.friction.fc).unwrap_or(0.0),
    }
}

fn runtime_param_before<B: MotorBus>(
    loop_ctrl: &ControlLoop<B>,
    joint: &str,
    param: &str,
) -> Result<f64, OverlayError> {
    let ov = baseline_gain_override(loop_ctrl, joint);
    match param {
        "kp" => Ok(ov.kp),
        "kd" => Ok(ov.kd),
        "ki" => Ok(ov.ki),
        "fc" => Ok(ov.fc),
        "pos" | "vel" | "torque_ff" => Err(OverlayError::UnsupportedParam(format!(
            "{param} (runtime MIT is gains-only; use Hold/Jog for motion)"
        ))),
        other => Err(OverlayError::UnsupportedParam(other.to_string())),
    }
}

fn apply_runtime_param<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    joint: &str,
    param: &str,
    value: f64,
) -> Result<(), OverlayError> {
    if !value.is_finite() {
        return Err(OverlayError::NonFinite(param.to_string()));
    }
    if value < 0.0 {
        return Err(OverlayError::UnsupportedParam(format!(
            "{param} must be >= 0"
        )));
    }
    // GravityComp / TorqueOnly require kp=0, kd=0 with tau_g (docs/safety.md). GainOverride
    // is applied after mode selection in Berthier and would defeat that contract.
    match loop_ctrl.control_mode() {
        ControlMode::GravityComp | ControlMode::TorqueOnly => {
            return Err(OverlayError::UnsupportedParam(format!(
                "{param} (RuntimeMit blocked in {:?}; keep kp=0 kd=0)",
                loop_ctrl.control_mode()
            )));
        }
        ControlMode::Disabled | ControlMode::Impedance | ControlMode::Position => {}
    }
    let mut ov = baseline_gain_override(loop_ctrl, joint);
    match param {
        "kp" => ov.kp = value,
        "kd" => ov.kd = value,
        "ki" => ov.ki = value,
        "fc" => ov.fc = value,
        "pos" | "vel" | "torque_ff" => {
            return Err(OverlayError::UnsupportedParam(format!(
                "{param} (runtime MIT is gains-only; use Hold/Jog for motion)"
            )));
        }
        other => return Err(OverlayError::UnsupportedParam(other.to_string())),
    }
    // Clamps to motor_type_defaults inside Berthier — same path as Testing page.
    loop_ctrl.apply_gain_override(joint, ov);
    Ok(())
}

pub fn build_limit_snapshot<B: MotorBus>(
    supervisor: &Supervisor<B>,
    allowlist: &CommandJointAllowlist,
    timestamp_ms: u64,
) -> ActuatorLimitSnapshot {
    let mut joints = Vec::new();
    for motor in &supervisor.motors.motors {
        let joint = motor.joint.as_str();
        let type_key = motor_type_key(motor.motor_type);
        let Some(defaults) = supervisor.control.control.motor_type_defaults.get(type_key) else {
            continue;
        };
        let policy = supervisor.joint_limit_policy(joint);
        let velocity = supervisor
            .joint_velocity_cap(joint)
            .unwrap_or(defaults.velocity_max_rad_s);
        let (pos_lower, pos_upper) = policy
            .map(|p| (p.hard_lower(), p.hard_upper()))
            .unwrap_or((0.0, 0.0));
        let tau_ff_max = policy
            .map(|p| p.tau_ff_max)
            .unwrap_or(defaults.tau_ff_max_nm);
        joints.push(JointActuatorLimit {
            joint: joint.to_string(),
            kp_max: defaults.kp_max,
            kd_max: defaults.kd_max,
            velocity_max_rad_s: velocity,
            tau_ff_max_nm: tau_ff_max,
            pos_lower_rad: pos_lower,
            pos_upper_rad: pos_upper,
            wired: allowlist.contains(joint),
        });
    }
    ActuatorLimitSnapshot {
        timestamp_ms,
        joints,
    }
}

fn tuning_event(
    operator: &OperatorCommand,
    joint: &str,
    tuning: &TuningChange,
    before: f64,
    after: f64,
) -> TuningChangeEvent {
    TuningChangeEvent {
        timestamp_ms: operator.timestamp_ms,
        session_id: operator.session_id.clone(),
        operator_id: operator.operator_id.clone(),
        joint: joint.to_string(),
        tier: tuning.tier,
        param: tuning.param.clone(),
        before,
        after,
        persist: tuning.persist,
    }
}

fn action_event(
    operator: &OperatorCommand,
    joint: &str,
    action: &str,
    accepted: bool,
    reject_reason: Option<String>,
    persist_status: PersistStatus,
    config_revision: String,
) -> ActionEvent {
    ActionEvent {
        timestamp_ms: operator.timestamp_ms,
        session_id: operator.session_id.clone(),
        operator_id: operator.operator_id.clone(),
        joint: joint.to_string(),
        action: action.to_string(),
        revision: AUDIT_REVISION.fetch_add(1, Ordering::Relaxed),
        accepted,
        reject_reason: reject_reason.unwrap_or_default(),
        persist_status: persist_status as i32,
        config_revision,
    }
}

fn action_label(cmd: &ActuatorCommand) -> &'static str {
    match cmd.payload {
        Some(actuator_command::Payload::Enable(_)) => "enable",
        Some(actuator_command::Payload::Mode(_)) => "mode_change",
        Some(actuator_command::Payload::Jog(_)) => "jog",
        Some(actuator_command::Payload::Hold(_)) => "hold",
        Some(actuator_command::Payload::Preset(_)) => "preset",
        Some(actuator_command::Payload::Tuning(_)) => "tuning",
        Some(actuator_command::Payload::LimitPatch(_)) => "limit_patch",
        None => "unknown",
    }
}

fn publish_outcome(chappe: &Arc<Bus>, outcome: &OverlayOutcome) -> Result<(), chappe::BusError> {
    match outcome {
        OverlayOutcome::Tuning(event) => publish_tuning_event(chappe, event),
        OverlayOutcome::Action(event) => publish_action_event(chappe, event),
    }
}

pub fn publish_tuning_event(
    chappe: &Arc<Bus>,
    event: &TuningChangeEvent,
) -> Result<(), chappe::BusError> {
    chappe.publish(
        TOPIC_AUDIT_TUNING,
        "marengo-pi",
        "marengo.v1.TuningChangeEvent",
        event,
    )
}

pub fn publish_action_event(
    chappe: &Arc<Bus>,
    event: &ActionEvent,
) -> Result<(), chappe::BusError> {
    chappe.publish(
        TOPIC_AUDIT_ACTION,
        "marengo-pi",
        "marengo.v1.ActionEvent",
        event,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use armee_proto::actuator_command::Payload;
    use berthier::ControlLoop;
    use davout::MemoryBus;
    use marengo_config::{load_control_config_from, CommandJointAllowlist};

    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn test_loop() -> ControlLoop<MemoryBus> {
        ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop")
    }

    fn allowlist_from_repo() -> CommandJointAllowlist {
        load_command_joint_allowlist_from(repo_root().join("config")).expect("allowlist")
    }

    fn test_persist_queue() -> (ConfigPersistQueue, Arc<AtomicBool>) {
        let shutdown = Arc::new(AtomicBool::new(false));
        let queue = ConfigPersistQueue::spawn(Arc::new(Bus::new(16)), Arc::clone(&shutdown));
        (queue, shutdown)
    }

    fn test_overlay() -> (ActuatorOverlay, Arc<AtomicBool>) {
        let (persist, shutdown) = test_persist_queue();
        (
            ActuatorOverlay::new(allowlist_from_repo(), persist),
            shutdown,
        )
    }

    fn tuning_operator(joint: &str, param: &str, value: f64, tier: i32) -> OperatorCommand {
        OperatorCommand {
            timestamp_ms: 1,
            session_id: "sess-test".to_string(),
            operator_id: "bench".to_string(),
            seq: 1,
            command: Some(ActuatorCommand {
                joint: joint.to_string(),
                payload: Some(Payload::Tuning(TuningChange {
                    tier,
                    param: param.to_string(),
                    value,
                    persist: tier == TuningTier::ConfigOverlay as i32,
                })),
            }),
        }
    }

    #[test]
    fn runtime_overlay_applies_kp_via_gain_override() {
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl = test_loop();
        let op = tuning_operator("elbow", "kp", 88.0, TuningTier::RuntimeMit as i32);
        let outcomes = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect("apply");
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0], OverlayOutcome::Tuning(_)));
        let OverlayOutcome::Tuning(ref event) = outcomes[0] else {
            return;
        };
        assert!((event.after - 88.0).abs() < 1e-9);
        let ov = loop_ctrl.gain_override("elbow").expect("override");
        assert!((ov.kp - 88.0).abs() < 1e-9);
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn runtime_overlay_rejects_pos_vel_torque_ff() {
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl = test_loop();
        for param in ["pos", "vel", "torque_ff"] {
            let op = tuning_operator("elbow", param, 1.0, TuningTier::RuntimeMit as i32);
            let err = overlay
                .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
                .expect_err(param);
            assert!(matches!(err, OverlayError::UnsupportedParam(_)));
        }
        assert!(loop_ctrl.gain_override("elbow").is_none());
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn runtime_overlay_rejects_under_gravity_comp() {
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl = test_loop();
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        let op = tuning_operator("elbow", "kp", 88.0, TuningTier::RuntimeMit as i32);
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect_err("gravity comp");
        assert!(matches!(err, OverlayError::UnsupportedParam(_)));
        assert!(loop_ctrl.gain_override("elbow").is_none());
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn runtime_overlay_rejects_negative_kp() {
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl = test_loop();
        let op = tuning_operator("elbow", "kp", -1.0, TuningTier::RuntimeMit as i32);
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect_err("negative");
        assert!(matches!(err, OverlayError::UnsupportedParam(_)));
        assert!(loop_ctrl.gain_override("elbow").is_none());
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn runtime_overlay_clamps_kp_to_motor_type_max() {
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl = test_loop();
        let op = tuning_operator("elbow", "kp", 600.0, TuningTier::RuntimeMit as i32);
        overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect("apply");
        let ov = loop_ctrl.gain_override("elbow").expect("override");
        // rs02 kp_max is 500 in default config
        assert!(ov.kp <= 500.0 + 1e-9);
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn config_overlay_rejects_over_max_kp_before_persist() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl =
            ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
        let op = tuning_operator(
            "elbow",
            "impedance.kp",
            9999.0,
            TuningTier::ConfigOverlay as i32,
        );
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
            .expect_err("over max");
        assert!(matches!(err, OverlayError::Config(_)));
        let disk = load_control_config_from(tmp.path()).expect("disk");
        assert!(disk.control.joints["elbow"].impedance.kp < 9999.0);
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn config_overlay_queues_persist_and_applies_live() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl =
            ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
        let op = tuning_operator(
            "elbow",
            "impedance.kp",
            33.0,
            TuningTier::ConfigOverlay as i32,
        );
        let outcomes = overlay
            .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
            .expect("apply");
        assert!(outcomes
            .iter()
            .any(|o| matches!(o, OverlayOutcome::Tuning(_))));
        assert!(outcomes.iter().any(|o| {
            matches!(
                o,
                OverlayOutcome::Action(ActionEvent {
                    action,
                    accepted: true,
                    ..
                }) if action == "config_persist"
            )
        }));
        assert!(
            (loop_ctrl.supervisor().control.control.joints["elbow"]
                .impedance
                .kp
                - 33.0)
                .abs()
                < 1e-9,
            "live must apply before disk write completes"
        );
        assert!(
            overlay.persist.wait_idle_for_test(Duration::from_secs(2)),
            "persist worker did not drain"
        );
        let reloaded = load_control_config_from(tmp.path()).expect("reload disk");
        assert!((reloaded.control.joints["elbow"].impedance.kp - 33.0).abs() < 1e-9);
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn persist_queue_coalesces_to_latest_draft() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let (queue, shutdown) = test_persist_queue();
        let mut draft = load_control_config_from(tmp.path()).expect("load");
        draft
            .control
            .joints
            .get_mut("elbow")
            .expect("elbow")
            .impedance
            .kp = 11.0;
        queue
            .enqueue(PersistRequest {
                config_dir: tmp.path().to_path_buf(),
                motors: None,
                control: draft.clone(),
                timestamp_ms: 1,
                session_id: "s".into(),
                operator_id: "o".into(),
                joint: "elbow".into(),
                param: "impedance.kp".into(),
            })
            .expect("enqueue1");
        draft
            .control
            .joints
            .get_mut("elbow")
            .expect("elbow")
            .impedance
            .kp = 44.0;
        queue
            .enqueue(PersistRequest {
                config_dir: tmp.path().to_path_buf(),
                motors: None,
                control: draft,
                timestamp_ms: 2,
                session_id: "s".into(),
                operator_id: "o".into(),
                joint: "elbow".into(),
                param: "impedance.kp".into(),
            })
            .expect("enqueue2");
        // At most one pending slot (latest wins) — may already be writing.
        assert!(queue.pending_count_for_test() <= 1);
        assert!(queue.wait_idle_for_test(Duration::from_secs(2)));
        let reloaded = load_control_config_from(tmp.path()).expect("reload");
        assert!((reloaded.control.joints["elbow"].impedance.kp - 44.0).abs() < 1e-9);
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn rejects_unwired_joint_with_not_wired_error() {
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl = test_loop();
        let op = tuning_operator("left_knee", "kp", 1.0, TuningTier::RuntimeMit as i32);
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect_err("unwired");
        assert!(matches!(err, OverlayError::NotWired(_)));
        shutdown.store(true, Ordering::SeqCst);
    }

    #[test]
    fn limit_snapshot_marks_wired_joints() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let allowlist = allowlist_from_repo();
        let snap = build_limit_snapshot(&sup, &allowlist, 42);
        let elbow = snap
            .joints
            .iter()
            .find(|j| j.joint == "elbow")
            .expect("elbow");
        assert!(elbow.wired);
        assert!(elbow.kp_max > 0.0);
    }

    #[test]
    fn limit_patch_applies_live_and_queues_persist() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let revision = profile_content_revision(tmp.path()).expect("revision");
        let (mut overlay, shutdown) = test_overlay();
        let mut loop_ctrl =
            ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
        let op = OperatorCommand {
            timestamp_ms: 1,
            session_id: "limit-test".into(),
            operator_id: "test".into(),
            seq: 1,
            command: Some(ActuatorCommand {
                joint: "elbow".into(),
                payload: Some(Payload::LimitPatch(LimitPatchCommand {
                    position_lower_rad: 0.1,
                    position_upper_rad: 1.4,
                    torque_limit_nm: Some(2.0),
                    position_soft_lower_rad: Some(0.0),
                    position_soft_upper_rad: Some(2.0),
                    velocity_max_rad_s: None,
                    expected_revision: revision,
                })),
            }),
        };
        let outcomes = overlay
            .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
            .expect("apply");
        assert!(outcomes.iter().any(|o| {
            matches!(
                o,
                OverlayOutcome::Action(ActionEvent {
                    action,
                    accepted: true,
                    persist_status,
                    ..
                }) if action == "limit_patch"
                    && *persist_status == PersistStatus::Pending as i32
            )
        }));
        let policy = loop_ctrl
            .supervisor()
            .joint_limit_policy("elbow")
            .expect("policy");
        assert!((policy.hard_upper() - 1.4).abs() < 1e-9);
        assert!(
            overlay.persist.wait_idle_for_test(Duration::from_secs(2)),
            "persist drain"
        );
        let motors = marengo_config::load_motors_config_from(tmp.path()).expect("motors");
        let elbow = motors
            .motors
            .iter()
            .find(|m| m.joint == "elbow")
            .expect("elbow");
        assert!((elbow.bench.position_upper_rad - 1.4).abs() < 1e-9);
        shutdown.store(true, Ordering::SeqCst);
    }
}
