//! Actuator harness overlay: Chappe tuning → Berthier `GainOverride` + optional control.yaml persist.
//!
//! Runtime MIT uses the existing Testing-page gain path (kp/kd/ki/fc only). It never
//! overrides position, velocity, or torque_ff — those stay under Berthier's planner /
//! gravity law so live control quality is preserved.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command, ActionEvent, ActuatorCommand, ActuatorLimitSnapshot, JointActuatorLimit,
    OperatorCommand, TuningChange, TuningChangeEvent, TuningTier,
};
use berthier::{ControlLoop, GainOverride};
use chappe::Bus;
use davout::{MotorBus, Supervisor};
use marengo_config::{
    apply_joint_config_param, load_command_joint_allowlist_from, load_control_config_from,
    motor_type_key, resolve_command_joint, validate_joint_gains_against_motor_type,
    write_control_config_from, CommandJointAllowlist,
};
use thiserror::Error;
use tracing::warn;

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
}

impl ActuatorOverlay {
    pub fn new(allowlist: CommandJointAllowlist) -> Self {
        Self {
            limits_dirty: true,
            allowlist,
        }
    }

    pub fn from_config_dir(config_dir: &Path) -> Result<Self, OverlayError> {
        Ok(Self::new(load_command_joint_allowlist_from(config_dir)?))
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
                                if matches!(outcome, OverlayOutcome::Tuning(_)) {
                                    self.limits_dirty = true;
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
                let event = apply_tuning_change(loop_ctrl, config_dir, operator, &joint, tuning)?;
                Ok(vec![OverlayOutcome::Tuning(event)])
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
                );
                Ok(vec![OverlayOutcome::Action(event)])
            }
            None => Err(OverlayError::MissingCommand),
        }
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

fn apply_tuning_change<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    config_dir: &Path,
    operator: &OperatorCommand,
    joint: &str,
    tuning: &TuningChange,
) -> Result<TuningChangeEvent, OverlayError> {
    match TuningTier::try_from(tuning.tier) {
        Ok(TuningTier::RuntimeMit) => {
            let before = runtime_param_before(loop_ctrl, joint, &tuning.param)?;
            apply_runtime_param(loop_ctrl, joint, &tuning.param, tuning.value)?;
            let after = runtime_param_before(loop_ctrl, joint, &tuning.param)?;
            Ok(tuning_event(operator, joint, tuning, before, after))
        }
        Ok(TuningTier::ConfigOverlay) => {
            // Clone-then-commit: never leave live config mutated if persist/validate fails.
            let mut draft = loop_ctrl.supervisor().control.clone();
            let entry = draft
                .control
                .joints
                .get_mut(joint)
                .ok_or_else(|| OverlayError::NotWired(joint.to_string()))?;
            let before = apply_joint_config_param(entry, &tuning.param, tuning.value)?;
            validate_joint_gains_against_motor_type(&draft, joint)?;
            if tuning.persist {
                write_control_config_from(config_dir, &draft)?;
                let reloaded = load_control_config_from(config_dir)?;
                loop_ctrl.supervisor_mut().control = reloaded;
            } else {
                loop_ctrl.supervisor_mut().control = draft;
            }
            Ok(tuning_event(operator, joint, tuning, before, tuning.value))
        }
        _ => Err(OverlayError::UnsupportedTier),
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
    use marengo_config::CommandJointAllowlist;

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
        let mut overlay = ActuatorOverlay::new(allowlist_from_repo());
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
    }

    #[test]
    fn runtime_overlay_rejects_pos_vel_torque_ff() {
        let mut overlay = ActuatorOverlay::new(allowlist_from_repo());
        let mut loop_ctrl = test_loop();
        for param in ["pos", "vel", "torque_ff"] {
            let op = tuning_operator("elbow", param, 1.0, TuningTier::RuntimeMit as i32);
            let err = overlay
                .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
                .expect_err(param);
            assert!(matches!(err, OverlayError::UnsupportedParam(_)));
        }
        assert!(loop_ctrl.gain_override("elbow").is_none());
    }

    #[test]
    fn runtime_overlay_clamps_kp_to_motor_type_max() {
        let mut overlay = ActuatorOverlay::new(allowlist_from_repo());
        let mut loop_ctrl = test_loop();
        let op = tuning_operator("elbow", "kp", 600.0, TuningTier::RuntimeMit as i32);
        overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect("apply");
        let ov = loop_ctrl.gain_override("elbow").expect("override");
        // rs02 kp_max is 500 in default config
        assert!(ov.kp <= 500.0 + 1e-9);
    }

    #[test]
    fn config_overlay_rejects_over_max_kp_before_persist() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let mut overlay = ActuatorOverlay::new(allowlist_from_repo());
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
    }

    #[test]
    fn config_overlay_writes_control_yaml_and_reloads() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let mut overlay = ActuatorOverlay::new(allowlist_from_repo());
        let mut loop_ctrl =
            ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
        let op = tuning_operator(
            "elbow",
            "impedance.kp",
            33.0,
            TuningTier::ConfigOverlay as i32,
        );
        overlay
            .apply_operator_command(&mut loop_ctrl, tmp.path(), &op)
            .expect("apply");
        let reloaded = load_control_config_from(tmp.path()).expect("reload disk");
        assert!((reloaded.control.joints["elbow"].impedance.kp - 33.0).abs() < 1e-9);
        assert!(
            (loop_ctrl.supervisor().control.control.joints["elbow"]
                .impedance
                .kp
                - 33.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn rejects_unwired_joint_with_not_wired_error() {
        let mut overlay = ActuatorOverlay::new(allowlist_from_repo());
        let mut loop_ctrl = test_loop();
        let op = tuning_operator("left_knee", "kp", 1.0, TuningTier::RuntimeMit as i32);
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &op)
            .expect_err("unwired");
        assert!(matches!(err, OverlayError::NotWired(_)));
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
}
