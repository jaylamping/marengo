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

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command, ActionEvent, ActuatorCommand, ActuatorLimitSnapshot, JointActuatorLimit,
    LimitPatchCommand, OperatorCommand, PersistStatus, TuningChange, TuningChangeEvent, TuningTier,
};
use berthier::{ControlLoop, ControlMode, GainOverride};
use chappe::Bus;
use davout::{MotorBus, Supervisor};
use marengo_config::{
    apply_joint_config_param, ensure_soft_inset, load_command_joint_allowlist_from, motor_type_key,
    profile_content_revision, resolve_command_joint, validate_joint_gains_against_motor_type,
    CommandJointAllowlist, LimitPatch,
};
use thiserror::Error;
use tracing::warn;

use crate::limit_persist::{next_audit_revision, PersistError, PersistRequest};

pub use crate::limit_persist::ConfigPersistQueue;

pub const TOPIC_ACTUATOR_COMMAND: &str = "robot/actuator/command";
pub const TOPIC_ACTUATOR_LIMITS: &str = "robot/actuator/limits";
pub const TOPIC_AUDIT_TUNING: &str = "robot/audit/tuning";
pub const TOPIC_AUDIT_ACTION: &str = "robot/audit/action";

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
    #[error(transparent)]
    PersistQueue(#[from] PersistError),
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
                        revision: next_audit_revision(),
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
        let mut patch = LimitPatch {
            joint: joint.to_string(),
            position_lower_rad: patch_cmd.position_lower_rad,
            position_upper_rad: patch_cmd.position_upper_rad,
            torque_limit_nm: patch_cmd.torque_limit_nm,
            position_soft_lower_rad: patch_cmd.position_soft_lower_rad,
            position_soft_upper_rad: patch_cmd.position_soft_upper_rad,
            velocity_max_rad_s: patch_cmd.velocity_max_rad_s,
        };
        ensure_soft_inset(&mut patch);

        // Snapshot for rollback if the persist queue is dead after a successful live apply.
        let motors_before = loop_ctrl.supervisor().motors.clone();
        let control_before = loop_ctrl.supervisor().control.clone();
        let urdf_before = loop_ctrl.supervisor().urdf_robot().clone();
        loop_ctrl
            .supervisor_mut()
            .apply_limit_patch(&patch)
            .map_err(|e| {
                OverlayError::Config(marengo_config::ConfigError::Parse {
                    path: config_dir.to_path_buf(),
                    message: e.to_string(),
                })
            })?;

        let motors = loop_ctrl.supervisor().motors.clone();
        let control = loop_ctrl.supervisor().control.clone();
        if let Err(error) = self.persist.enqueue(PersistRequest {
            config_dir: config_dir.to_path_buf(),
            motors: Some(motors),
            control,
            timestamp_ms: operator.timestamp_ms,
            session_id: operator.session_id.clone(),
            operator_id: operator.operator_id.clone(),
            joint: joint.to_string(),
            param: "limit_patch".into(),
        }) {
            loop_ctrl
                .supervisor_mut()
                .restore_limit_snapshot(motors_before, control_before, urdf_before)
                .map_err(|e| {
                    OverlayError::Config(marengo_config::ConfigError::Parse {
                        path: config_dir.to_path_buf(),
                        message: format!(
                            "limit_patch rollback failed after enqueue error ({error}): {e}"
                        ),
                    })
                })?;
            return Err(error.into());
        }
        warn!(
            joint = %joint,
            session = %operator.session_id,
            "limit_patch applied live; async motors+control+URDF persist queued"
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
        revision: next_audit_revision(),
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
#[path = "overlay_tests.rs"]
mod tests;
