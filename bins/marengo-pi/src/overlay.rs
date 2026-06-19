//! Actuator harness overlay: Chappe command subscriber → Berthier gains + control.yaml persist.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command, ActionEvent, ActuatorCommand, ActuatorLimitSnapshot, JointActuatorLimit,
    OperatorCommand, TuningChange, TuningChangeEvent, TuningTier,
};
use berthier::{ControlLoop, RuntimeMitOverlay};
use chappe::Bus;
use davout::{MitJointCommand, MotorBus, Supervisor};
use marengo_config::{
    apply_joint_config_param, is_wired_bench_joint, load_control_config_from,
    motor_for_joint, motor_type_key, resolve_command_joint, write_control_config_from,
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
}

impl Default for ActuatorOverlay {
    fn default() -> Self {
        Self {
            limits_dirty: true,
        }
    }
}

impl ActuatorOverlay {
    pub fn new() -> Self {
        Self::default()
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
        while let Ok(bytes) = rx.try_recv() {
            let Ok(envelope) = armee_proto::Envelope::decode(bytes.as_slice()) else {
                continue;
            };
            let Ok(operator) = OperatorCommand::decode(envelope.payload.as_slice()) else {
                continue;
            };
            match self.apply_operator_command(loop_ctrl, config_dir, chappe, &operator) {
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
                    warn!(error = %e, session = %operator.session_id, "actuator overlay rejected command");
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
    }

    pub fn apply_operator_command<B: MotorBus>(
        &mut self,
        loop_ctrl: &mut ControlLoop<B>,
        config_dir: &Path,
        chappe: &Arc<Bus>,
        operator: &OperatorCommand,
    ) -> Result<Vec<OverlayOutcome>, OverlayError> {
        let cmd = operator
            .command
            .as_ref()
            .ok_or(OverlayError::MissingCommand)?;
        if !resolve_command_joint(&cmd.joint).is_some_and(|j| j == cmd.joint.as_str()) {
            return Err(OverlayError::NotWired(cmd.joint.clone()));
        }

        match cmd.payload {
            Some(actuator_command::Payload::Tuning(ref tuning)) => {
                let event = apply_tuning_change(loop_ctrl, config_dir, operator, cmd, tuning)?;
                let _ = chappe;
                Ok(vec![OverlayOutcome::Tuning(event)])
            }
            Some(actuator_command::Payload::Enable(_))
            | Some(actuator_command::Payload::Mode(_))
            | Some(actuator_command::Payload::Jog(_))
            | Some(actuator_command::Payload::Hold(_))
            | Some(actuator_command::Payload::Preset(_)) => {
                let event = action_event(
                    operator,
                    &cmd.joint,
                    action_label(cmd),
                    false,
                    Some("motion commands gated until PR-5 feasibility".to_string()),
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
        let snapshot = build_limit_snapshot(supervisor, timestamp_ms);
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
    cmd: &ActuatorCommand,
    tuning: &TuningChange,
) -> Result<TuningChangeEvent, OverlayError> {
    let joint = cmd.joint.as_str();
    match TuningTier::try_from(tuning.tier) {
        Ok(TuningTier::RuntimeMit) => {
            let before = runtime_param_before(loop_ctrl, joint, &tuning.param)?;
            apply_runtime_param(loop_ctrl, joint, &tuning.param, tuning.value)?;
            Ok(tuning_event(
                operator,
                joint,
                tuning,
                before,
                tuning.value,
            ))
        }
        Ok(TuningTier::ConfigOverlay) => {
            let supervisor = loop_ctrl.supervisor_mut();
            let entry = supervisor
                .control
                .control
                .joints
                .get_mut(joint)
                .ok_or_else(|| OverlayError::NotWired(joint.to_string()))?;
            let before = apply_joint_config_param(entry, &tuning.param, tuning.value)?;
            if tuning.persist {
                write_control_config_from(config_dir, &supervisor.control)?;
                let reloaded = load_control_config_from(config_dir)?;
                supervisor.control = reloaded;
            }
            Ok(tuning_event(
                operator,
                joint,
                tuning,
                before,
                tuning.value,
            ))
        }
        _ => Err(OverlayError::UnsupportedTier),
    }
}

fn runtime_param_before<B: MotorBus>(
    loop_ctrl: &ControlLoop<B>,
    joint: &str,
    param: &str,
) -> Result<f64, OverlayError> {
    if let Some(overlay) = loop_ctrl.runtime_overlay(joint) {
        if let Some(v) = runtime_overlay_field(overlay, param) {
            return Ok(v);
        }
    }
    let cfg = loop_ctrl
        .supervisor()
        .control
        .control
        .joints
        .get(joint)
        .ok_or_else(|| OverlayError::NotWired(joint.to_string()))?;
    match param {
        "kp" => Ok(cfg.impedance.kp),
        "kd" => Ok(cfg.impedance.kd),
        "pos" => Ok(0.0),
        "vel" => Ok(0.0),
        "torque_ff" => Ok(0.0),
        other => Err(OverlayError::UnsupportedParam(other.to_string())),
    }
}

fn runtime_overlay_field(overlay: &RuntimeMitOverlay, param: &str) -> Option<f64> {
    match param {
        "kp" => overlay.kp,
        "kd" => overlay.kd,
        "pos" => overlay.position_rad,
        "vel" => overlay.velocity_rad_s,
        "torque_ff" => overlay.torque_ff_nm,
        _ => None,
    }
}

fn apply_runtime_param<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    joint: &str,
    param: &str,
    value: f64,
) -> Result<(), OverlayError> {
    let mut overlay = loop_ctrl
        .runtime_overlay(joint)
        .cloned()
        .unwrap_or_default();
    match param {
        "kp" => overlay.kp = Some(value),
        "kd" => overlay.kd = Some(value),
        "pos" => overlay.position_rad = Some(value),
        "vel" => overlay.velocity_rad_s = Some(value),
        "torque_ff" => overlay.torque_ff_nm = Some(value),
        other => return Err(OverlayError::UnsupportedParam(other.to_string())),
    }
    loop_ctrl.set_runtime_overlay(joint, overlay);
    Ok(())
}

pub fn build_limit_snapshot<B: MotorBus>(
    supervisor: &Supervisor<B>,
    timestamp_ms: u64,
) -> ActuatorLimitSnapshot {
    let mut joints = Vec::new();
    for motor in &supervisor.motors.motors {
        let joint = motor.joint.as_str();
        let type_key = motor_type_key(motor.motor_type);
        let Some(defaults) = supervisor
            .control
            .control
            .motor_type_defaults
            .get(type_key)
        else {
            continue;
        };
        let policy = supervisor.joint_limit_policy(joint);
        let velocity = supervisor
            .joint_velocity_cap(joint)
            .unwrap_or(defaults.velocity_max_rad_s);
        let (pos_lower, pos_upper) = policy
            .map(|p| (p.hard_lower(), p.hard_upper()))
            .unwrap_or((0.0, 0.0));
        let tau_ff_max = policy.map(|p| p.tau_ff_max).unwrap_or(defaults.tau_ff_max_nm);
        joints.push(JointActuatorLimit {
            joint: joint.to_string(),
            kp_max: defaults.kp_max,
            kd_max: defaults.kd_max,
            velocity_max_rad_s: velocity,
            tau_ff_max_nm: tau_ff_max,
            pos_lower_rad: pos_lower,
            pos_upper_rad: pos_upper,
            wired: is_wired_bench_joint(joint),
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

/// Returns true when Davout rejects `kp` above motor-type max (overlay must not bypass Davout).
pub fn davout_rejects_excessive_kp<B: MotorBus>(
    supervisor: &mut Supervisor<B>,
    joint: &str,
    kp: f64,
) -> bool {
    let Some(motor) = motor_for_joint(&supervisor.motors, joint).cloned() else {
        return true;
    };
    supervisor
        .filter_mit_command(
            MitJointCommand {
                joint: joint.to_string(),
                kp,
                kd: 0.0,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_ff_nm: 0.0,
            },
            &motor,
        )
        .is_err()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Arc;

    use armee_proto::actuator_command::Payload;
    use berthier::ControlLoop;
    use chappe::Bus;
    use davout::{MemoryBus, Supervisor};

    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn test_loop() -> ControlLoop<MemoryBus> {
        ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50).expect("loop")
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
    fn runtime_overlay_applies_kp_to_berthier() {
        let mut overlay = ActuatorOverlay::new();
        let mut loop_ctrl = test_loop();
        let chappe = Arc::new(Bus::default());
        let op = tuning_operator("elbow", "kp", 88.0, TuningTier::RuntimeMit as i32);
        let outcomes = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &chappe, &op)
            .expect("apply");
        assert_eq!(outcomes.len(), 1);
        let OverlayOutcome::Tuning(event) = &outcomes[0] else {
            panic!("expected tuning event");
        };
        assert!((event.after - 88.0).abs() < 1e-9);
        assert!((event.before - 20.0).abs() < 1e-9);
        let rt = loop_ctrl.runtime_overlay("elbow").expect("overlay");
        assert!((rt.kp.expect("kp") - 88.0).abs() < 1e-9);
    }

    #[test]
    fn config_overlay_writes_control_yaml_and_reloads() {
        let root = repo_root();
        let src = root.join("config");
        let tmp = tempfile::tempdir().expect("tempdir");
        for name in ["control.yaml", "robot.yaml", "motors.yaml", "homing.yaml"] {
            std::fs::copy(src.join(name), tmp.path().join(name)).expect("copy");
        }
        let mut overlay = ActuatorOverlay::new();
        let mut loop_ctrl =
            ControlLoop::from_repo(&root, MemoryBus::default(), 200, 50).expect("loop");
        let chappe = Arc::new(Bus::default());
        let op = tuning_operator("elbow", "impedance.kp", 33.0, TuningTier::ConfigOverlay as i32);
        overlay
            .apply_operator_command(&mut loop_ctrl, tmp.path(), &chappe, &op)
            .expect("apply");
        let reloaded = load_control_config_from(tmp.path()).expect("reload disk");
        assert!((reloaded.control.joints["elbow"].impedance.kp - 33.0).abs() < 1e-9);
        assert!(
            (loop_ctrl.supervisor_mut().control.control.joints["elbow"].impedance.kp - 33.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn davout_clamps_overlay_raised_kp_above_motor_type_max() {
        let bus = MemoryBus::default();
        let mut sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        assert!(
            davout_rejects_excessive_kp(&mut sup, "elbow", 600.0),
            "rs02 kp_max is 500 — Davout must reject 600"
        );
        assert!(
            !davout_rejects_excessive_kp(&mut sup, "elbow", 100.0),
            "in-range kp must pass Davout filter"
        );
    }

    #[test]
    fn rejects_unwired_joint_with_not_wired_error() {
        let mut overlay = ActuatorOverlay::new();
        let mut loop_ctrl = test_loop();
        let chappe = Arc::new(Bus::default());
        let op = tuning_operator("left_knee", "kp", 1.0, TuningTier::RuntimeMit as i32);
        let err = overlay
            .apply_operator_command(&mut loop_ctrl, &repo_root().join("config"), &chappe, &op)
            .expect_err("unwired");
        assert!(matches!(err, OverlayError::NotWired(_)));
    }

    #[test]
    fn limit_snapshot_marks_wired_joints() {
        let bus = MemoryBus::default();
        let sup = Supervisor::from_repo(repo_root(), bus).expect("supervisor");
        let snap = build_limit_snapshot(&sup, 42);
        let elbow = snap
            .joints
            .iter()
            .find(|j| j.joint == "elbow")
            .expect("elbow");
        assert!(elbow.wired);
        assert!(elbow.kp_max > 0.0);
    }
}
