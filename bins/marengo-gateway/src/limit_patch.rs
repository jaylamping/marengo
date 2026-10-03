//! Live limit patch via Pi ACK (Set Limits / POST /config/patch).

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command, ActuatorCommand, LimitPatchCommand, OperatorCommand,
    PersistStatus as ProtoPersistStatus,
};
use marengo_config::{limit_patch_from_motor, profile_content_revision, LimitPatch};
use tokio::time::timeout;

use crate::action_ack::{recv_action_ack, LIMIT_PATCH_PERSIST_ACTION, LIVE_LIMIT_PATCH_ACTION};
use crate::state::{SharedState, TOPIC_ACTUATOR_COMMAND};

const LIVE_APPLY_TIMEOUT: Duration = Duration::from_secs(8);
const LIMIT_PATCH_PERSIST_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistStatus {
    Durable,
    Pending,
    Failed,
}

#[derive(Debug)]
pub struct LimitPatchResult {
    pub ok: bool,
    pub message: String,
    pub restart_required: bool,
    pub persist_status: PersistStatus,
}

#[derive(Debug, Clone)]
pub struct LimitPatchRequest {
    pub joint: String,
    pub operator_id: String,
    pub expected_revision: Option<String>,
    pub position_lower_rad: Option<f64>,
    pub position_upper_rad: Option<f64>,
    pub torque_limit_nm: Option<f64>,
    pub position_soft_lower_rad: Option<f64>,
    pub position_soft_upper_rad: Option<f64>,
    pub velocity_max_rad_s: Option<f64>,
}

pub async fn apply_limit_patch_async(
    state: SharedState,
    config_dir: &Path,
    request: LimitPatchRequest,
) -> LimitPatchResult {
    let joint = request.joint.trim();
    if joint.is_empty() {
        return limit_patch_error("joint name required".to_string(), PersistStatus::Failed);
    }
    let before = match limit_patch_from_motor(config_dir, joint) {
        Ok(before) => before,
        Err(error) => {
            return limit_patch_error(
                format!("joint {joint} not in master config: {error}"),
                PersistStatus::Failed,
            );
        }
    };
    let after = merge_request_limits(&before, &request);
    let current_revision = match profile_content_revision(config_dir) {
        Ok(revision) => revision,
        Err(error) => {
            return limit_patch_error(error.to_string(), PersistStatus::Failed);
        }
    };
    if let Some(expected) = request.expected_revision.as_deref() {
        if expected != current_revision {
            return limit_patch_error(
                format!("config revision mismatch: expected {expected}, found {current_revision}"),
                PersistStatus::Failed,
            );
        }
    }
    if before == after {
        return LimitPatchResult {
            ok: true,
            message: format!("No limit changes for {joint}"),
            restart_required: false,
            persist_status: PersistStatus::Durable,
        };
    }

    let session_id = format!(
        "limit-{}-{}",
        joint,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let mut rx = state.subscribe_envelopes();
    let operator = OperatorCommand {
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        session_id: session_id.clone(),
        operator_id: request.operator_id.clone(),
        seq: 1,
        command: Some(ActuatorCommand {
            joint: joint.to_string(),
            payload: Some(actuator_command::Payload::LimitPatch(LimitPatchCommand {
                position_lower_rad: after.position_lower_rad,
                position_upper_rad: after.position_upper_rad,
                torque_limit_nm: after.torque_limit_nm,
                position_soft_lower_rad: after.position_soft_lower_rad,
                position_soft_upper_rad: after.position_soft_upper_rad,
                velocity_max_rad_s: after.velocity_max_rad_s,
                expected_revision: request
                    .expected_revision
                    .clone()
                    .unwrap_or(current_revision.clone()),
            })),
        }),
    };
    let payload = operator.encode_to_vec();
    if let Err(error) = state.publish_command_envelope(
        TOPIC_ACTUATOR_COMMAND,
        "marengo-gateway",
        "marengo.v1.OperatorCommand",
        payload,
    ) {
        return limit_patch_error(
            format!("failed to publish limit_patch: {error}"),
            PersistStatus::Failed,
        );
    }

    let wait = timeout(
        LIVE_APPLY_TIMEOUT,
        recv_action_ack(
            &mut rx,
            &session_id,
            joint,
            &request.operator_id,
            LIVE_LIMIT_PATCH_ACTION,
        ),
    )
    .await;

    match wait {
        Ok(event) if event.accepted => {
            let persist_wait = timeout(
                LIMIT_PATCH_PERSIST_TIMEOUT,
                recv_action_ack(
                    &mut rx,
                    &session_id,
                    joint,
                    &request.operator_id,
                    LIMIT_PATCH_PERSIST_ACTION,
                ),
            )
            .await;
            let persist_event = match persist_wait {
                Ok(ev) => ev,
                Err(_) => {
                    return limit_patch_error(
                        format!(
                            "live limits applied for {joint}, but timed out waiting for durable persist"
                        ),
                        PersistStatus::Pending,
                    );
                }
            };
            let persist_status = match ProtoPersistStatus::try_from(persist_event.persist_status) {
                Ok(ProtoPersistStatus::Durable) => PersistStatus::Durable,
                Ok(ProtoPersistStatus::Failed) => PersistStatus::Failed,
                Ok(ProtoPersistStatus::Pending) => PersistStatus::Pending,
                _ => PersistStatus::Pending,
            };
            if persist_status == PersistStatus::Failed || !persist_event.accepted {
                return limit_patch_error(
                    if persist_event.reject_reason.is_empty() {
                        format!("Pi live-applied {joint} but durable persist failed")
                    } else {
                        persist_event.reject_reason
                    },
                    PersistStatus::Failed,
                );
            }
            LimitPatchResult {
                ok: true,
                message: format!(
                    "Applied live limits for {joint} on master (durable; URDF expanded if needed)"
                ),
                restart_required: false,
                persist_status,
            }
        }
        Ok(event) => limit_patch_error(
            if event.reject_reason.is_empty() {
                format!("Pi rejected limit_patch for {joint}")
            } else {
                event.reject_reason
            },
            PersistStatus::Failed,
        ),
        Err(_) => limit_patch_error(
            format!("timed out waiting for Pi ACK on limit_patch for {joint}"),
            PersistStatus::Failed,
        ),
    }
}

fn merge_request_limits(before: &LimitPatch, request: &LimitPatchRequest) -> LimitPatch {
    LimitPatch {
        joint: before.joint.clone(),
        position_lower_rad: request
            .position_lower_rad
            .unwrap_or(before.position_lower_rad),
        position_upper_rad: request
            .position_upper_rad
            .unwrap_or(before.position_upper_rad),
        torque_limit_nm: request.torque_limit_nm.or(before.torque_limit_nm),
        position_soft_lower_rad: request
            .position_soft_lower_rad
            .or(before.position_soft_lower_rad),
        position_soft_upper_rad: request
            .position_soft_upper_rad
            .or(before.position_soft_upper_rad),
        velocity_max_rad_s: request.velocity_max_rad_s.or(before.velocity_max_rad_s),
    }
}

fn limit_patch_error(message: String, persist_status: PersistStatus) -> LimitPatchResult {
    LimitPatchResult {
        ok: false,
        message,
        restart_required: false,
        persist_status,
    }
}
