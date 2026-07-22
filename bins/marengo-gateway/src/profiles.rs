//! Bringup profile inventory, snapshots, and validated actuator config transactions.

use std::path::Path as FsPath;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command, ActionEvent, ActuatorCommand, Envelope, LimitPatchCommand, OperatorCommand,
    PersistStatus as ProtoPersistStatus,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use marengo_config::{
    add_joint_from_source, joint_in_motors, joint_in_profile_urdf, limit_patch_from_motor,
    membership_slugs_for_joint, profile_content_revision, profile_slug_for_preset,
    resolve_bringup_dir, upsert_joint_limits, LimitPatch, BRINGUP_PROFILE_SLUGS,
    PRESET_PROFILE_MAP,
};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

use crate::config::{authorize_config_mutation, snapshot_from_dir, ConfigSnapshotJson};
use crate::state::{SharedState, TOPIC_ACTUATOR_COMMAND, TOPIC_AUDIT_ACTION};

const LIVE_APPLY_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Serialize)]
pub struct ProfileRevisionJson {
    pub slug: String,
    pub revision: String,
}

#[derive(Debug, Serialize)]
pub struct PresetProfileJson {
    pub preset_id: String,
    pub profile_slug: String,
}

#[derive(Debug, Serialize)]
pub struct ProfilesJson {
    pub active_slug: String,
    pub profiles: Vec<ProfileRevisionJson>,
    pub presets: Vec<PresetProfileJson>,
}

#[derive(Serialize)]
pub struct ProfileSnapshotJson {
    #[serde(flatten)]
    pub snapshot: ConfigSnapshotJson,
    pub revision: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyOperation {
    UpsertLimits,
    AddJoint,
    Preview,
}

#[derive(Debug, Deserialize)]
pub struct ApplyActuatorJson {
    pub target_profile: String,
    pub expected_revision: Option<String>,
    pub operator_id: String,
    pub op: ApplyOperation,
    pub joint: String,
    pub position_lower_rad: Option<f64>,
    pub position_upper_rad: Option<f64>,
    pub torque_limit_nm: Option<f64>,
    pub position_soft_lower_rad: Option<f64>,
    pub position_soft_upper_rad: Option<f64>,
    pub velocity_max_rad_s: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyDecision {
    Add,
    Overwrite,
    Noop,
    UnmappedPreset,
    UnsupportedMembership,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PersistStatus {
    Durable,
    #[allow(dead_code)]
    Pending,
    Failed,
    #[serde(rename = "n/a")]
    NotApplicable,
}

#[derive(Debug, Serialize)]
pub struct ApplyActuatorResultJson {
    pub ok: bool,
    pub message: String,
    pub applied_live: bool,
    pub restart_required: bool,
    pub revision: Option<String>,
    pub persist_status: PersistStatus,
    pub decision: Option<ApplyDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<LimitPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<LimitPatch>,
}

pub async fn get_profiles() -> Result<Json<ProfilesJson>, StatusCode> {
    let repo_root = marengo_config::resolve_repo_root();
    let active_dir = marengo_config::resolve_config_dir(&repo_root);
    let mut profiles = Vec::with_capacity(BRINGUP_PROFILE_SLUGS.len());
    for slug in BRINGUP_PROFILE_SLUGS {
        let dir =
            resolve_bringup_dir(&repo_root, slug).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let revision =
            profile_content_revision(&dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        profiles.push(ProfileRevisionJson {
            slug: (*slug).to_string(),
            revision,
        });
    }
    let presets = PRESET_PROFILE_MAP
        .iter()
        .map(|mapping| PresetProfileJson {
            preset_id: mapping.preset_id.to_string(),
            profile_slug: mapping.profile_slug.to_string(),
        })
        .collect();
    Ok(Json(ProfilesJson {
        active_slug: profile_slug_from_dir(&repo_root, &active_dir),
        profiles,
        presets,
    }))
}

pub async fn get_profile_snapshot(
    Path(slug): Path<String>,
) -> Result<Json<ProfileSnapshotJson>, StatusCode> {
    let repo_root = marengo_config::resolve_repo_root();
    let config_dir = resolve_bringup_dir(&repo_root, &slug).map_err(|_| StatusCode::NOT_FOUND)?;
    let snapshot = snapshot_from_dir(&config_dir)?;
    let revision =
        profile_content_revision(&config_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(ProfileSnapshotJson { snapshot, revision }))
}

pub async fn post_apply_actuator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ApplyActuatorJson>,
) -> Result<(StatusCode, Json<ApplyActuatorResultJson>), StatusCode> {
    authorize_config_mutation(&headers)?;
    let repo_root = marengo_config::resolve_repo_root();
    let active_dir = marengo_config::resolve_config_dir(&repo_root);
    Ok(apply_actuator_async(state, &repo_root, &active_dir, request).await)
}

pub(crate) async fn apply_actuator_async(
    state: SharedState,
    repo_root: &FsPath,
    active_dir: &FsPath,
    request: ApplyActuatorJson,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    let target_input = request.target_profile.trim();
    let Some(target_slug) = resolve_target_slug(target_input) else {
        let decision = target_input
            .starts_with("bench_")
            .then_some(ApplyDecision::UnmappedPreset);
        return apply_error(
            StatusCode::BAD_REQUEST,
            format!("unknown bringup profile or preset: {target_input}"),
            PersistStatus::NotApplicable,
            decision,
            None,
            None,
        );
    };
    let is_active = target_slug == profile_slug_from_dir(repo_root, active_dir);
    if is_active && matches!(request.op, ApplyOperation::UpsertLimits) {
        let slug = target_slug.to_string();
        return apply_active_limit_patch(state, repo_root, active_dir, &slug, request).await;
    }
    apply_actuator(repo_root, active_dir, request)
}

fn apply_actuator(
    repo_root: &FsPath,
    active_dir: &FsPath,
    request: ApplyActuatorJson,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    let target_input = request.target_profile.trim();
    let Some(target_slug) = resolve_target_slug(target_input) else {
        let decision = target_input
            .starts_with("bench_")
            .then_some(ApplyDecision::UnmappedPreset);
        return apply_error(
            StatusCode::BAD_REQUEST,
            format!("unknown bringup profile or preset: {target_input}"),
            PersistStatus::NotApplicable,
            decision,
            None,
            None,
        );
    };
    let target_dir = match resolve_bringup_dir(repo_root, target_slug) {
        Ok(dir) => dir,
        Err(error) => {
            return apply_error(
                StatusCode::NOT_FOUND,
                error.to_string(),
                PersistStatus::NotApplicable,
                None,
                None,
                None,
            );
        }
    };
    let joint = request.joint.trim();
    if joint.is_empty() || request.operator_id.trim().is_empty() {
        return apply_error(
            StatusCode::BAD_REQUEST,
            "joint and operator_id are required".to_string(),
            PersistStatus::NotApplicable,
            None,
            None,
            None,
        );
    }

    match request.op {
        ApplyOperation::Preview => preview_apply(repo_root, active_dir, &target_dir, joint),
        ApplyOperation::UpsertLimits => upsert_limits_apply(
            repo_root,
            active_dir,
            target_slug,
            &target_dir,
            joint,
            &request,
        ),
        ApplyOperation::AddJoint => add_joint_apply(
            repo_root,
            active_dir,
            target_slug,
            &target_dir,
            joint,
            &request,
        ),
    }
}

fn preview_apply(
    repo_root: &FsPath,
    active_dir: &FsPath,
    target_dir: &FsPath,
    joint: &str,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    let source = match limit_patch_from_motor(active_dir, joint) {
        Ok(source) => source,
        Err(error) => {
            return apply_error(
                StatusCode::BAD_REQUEST,
                format!("joint is not available from the active profile: {error}"),
                PersistStatus::NotApplicable,
                Some(ApplyDecision::UnsupportedMembership),
                None,
                None,
            );
        }
    };
    let before = limit_patch_from_motor(target_dir, joint).ok();
    let target_supports_joint =
        joint_in_profile_urdf(repo_root, target_dir, joint).unwrap_or(false);
    let decision = classify_preview(before.as_ref(), &source, target_supports_joint);
    let revision = profile_content_revision(target_dir).ok();
    let ok = decision != ApplyDecision::UnsupportedMembership;
    let message = if ok {
        format!("Previewed {joint} for {}", profile_name(target_dir))
    } else {
        unsupported_membership_message(repo_root, joint, target_dir)
    };
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        },
        Json(ApplyActuatorResultJson {
            ok,
            message,
            applied_live: false,
            restart_required: false,
            revision,
            persist_status: PersistStatus::NotApplicable,
            decision: Some(decision),
            before,
            after: Some(source),
        }),
    )
}

fn upsert_limits_apply(
    repo_root: &FsPath,
    active_dir: &FsPath,
    target_slug: &str,
    target_dir: &FsPath,
    joint: &str,
    request: &ApplyActuatorJson,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    let before = match limit_patch_from_motor(target_dir, joint) {
        Ok(before) => before,
        Err(_) => {
            return apply_error(
                StatusCode::BAD_REQUEST,
                format!("joint {joint} is not in the target profile; use op=add_joint first"),
                PersistStatus::Failed,
                Some(ApplyDecision::UnsupportedMembership),
                None,
                None,
            );
        }
    };
    let after = merge_request_limits(&before, request);
    let current_revision = match profile_content_revision(target_dir) {
        Ok(revision) => revision,
        Err(error) => {
            return apply_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                PersistStatus::Failed,
                None,
                Some(before),
                Some(after),
            );
        }
    };
    if let Err(message) =
        verify_expected_revision(&current_revision, request.expected_revision.as_deref())
    {
        return apply_error(
            StatusCode::CONFLICT,
            message,
            PersistStatus::Failed,
            Some(ApplyDecision::Noop),
            Some(before),
            Some(after),
        );
    }
    let decision = if before == after {
        ApplyDecision::Noop
    } else {
        ApplyDecision::Overwrite
    };
    let revision = if decision == ApplyDecision::Noop {
        current_revision
    } else {
        match upsert_joint_limits(target_dir, &after, request.expected_revision.as_deref()) {
            Ok(result) => result.revision,
            Err(error) => {
                return apply_error(
                    StatusCode::BAD_REQUEST,
                    error.to_string(),
                    PersistStatus::Failed,
                    Some(decision),
                    Some(before),
                    Some(after),
                );
            }
        }
    };
    let _ = (repo_root, active_dir);
    (
        StatusCode::OK,
        Json(ApplyActuatorResultJson {
            ok: true,
            message: format!("Updated {joint} in {target_slug}"),
            applied_live: false,
            restart_required: false,
            revision: Some(revision),
            persist_status: PersistStatus::Durable,
            decision: Some(decision),
            before: Some(before),
            after: Some(after),
        }),
    )
}

async fn apply_active_limit_patch(
    state: SharedState,
    _repo_root: &FsPath,
    active_dir: &FsPath,
    target_slug: &str,
    request: ApplyActuatorJson,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    let joint = request.joint.trim();
    let before = match limit_patch_from_motor(active_dir, joint) {
        Ok(before) => before,
        Err(error) => {
            return apply_error(
                StatusCode::BAD_REQUEST,
                format!("joint {joint} not in active profile: {error}"),
                PersistStatus::Failed,
                Some(ApplyDecision::UnsupportedMembership),
                None,
                None,
            );
        }
    };
    let after = merge_request_limits(&before, &request);
    let current_revision = match profile_content_revision(active_dir) {
        Ok(revision) => revision,
        Err(error) => {
            return apply_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                PersistStatus::Failed,
                None,
                Some(before),
                Some(after),
            );
        }
    };
    if let Err(message) =
        verify_expected_revision(&current_revision, request.expected_revision.as_deref())
    {
        return apply_error(
            StatusCode::CONFLICT,
            message,
            PersistStatus::Failed,
            Some(ApplyDecision::Noop),
            Some(before),
            Some(after),
        );
    }
    if before == after {
        return (
            StatusCode::OK,
            Json(ApplyActuatorResultJson {
                ok: true,
                message: format!("No limit changes for {joint}"),
                applied_live: false,
                restart_required: false,
                revision: Some(current_revision),
                persist_status: PersistStatus::Durable,
                decision: Some(ApplyDecision::Noop),
                before: Some(before),
                after: Some(after),
            }),
        );
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
        return apply_error(
            StatusCode::BAD_GATEWAY,
            format!("failed to publish limit_patch: {error}"),
            PersistStatus::Failed,
            Some(ApplyDecision::Overwrite),
            Some(before),
            Some(after),
        );
    }

    let wait = timeout(
        LIVE_APPLY_TIMEOUT,
        recv_live_limit_patch_ack(&mut rx, &session_id, joint, &request.operator_id),
    )
    .await;

    match wait {
        Ok(event) if event.accepted => {
            let persist_status = match ProtoPersistStatus::try_from(event.persist_status) {
                Ok(ProtoPersistStatus::Durable) => PersistStatus::Durable,
                Ok(ProtoPersistStatus::Failed) => PersistStatus::Failed,
                Ok(ProtoPersistStatus::Pending) => PersistStatus::Pending,
                _ => PersistStatus::Pending,
            };
            let revision = if event.config_revision.is_empty() {
                current_revision
            } else {
                event.config_revision
            };
            (
                StatusCode::OK,
                Json(ApplyActuatorResultJson {
                    ok: true,
                    message: format!("Applied live limits for {joint} on {target_slug}"),
                    applied_live: true,
                    restart_required: false,
                    revision: Some(revision),
                    persist_status,
                    decision: Some(ApplyDecision::Overwrite),
                    before: Some(before),
                    after: Some(after),
                }),
            )
        }
        Ok(event) => apply_error(
            StatusCode::CONFLICT,
            if event.reject_reason.is_empty() {
                format!("Pi rejected limit_patch for {joint}")
            } else {
                event.reject_reason
            },
            PersistStatus::Failed,
            Some(ApplyDecision::Overwrite),
            Some(before),
            Some(after),
        ),
        Err(_) => apply_error(
            StatusCode::GATEWAY_TIMEOUT,
            format!("timed out waiting for Pi ACK on limit_patch for {joint}"),
            PersistStatus::Failed,
            Some(ApplyDecision::Overwrite),
            Some(before),
            Some(after),
        ),
    }
}

/// Live apply ACK only. Write-behind uses `limit_patch_persist` so Failed/Durable
/// cannot be mistaken for a live reject when the Pending event is lagged away.
const LIVE_LIMIT_PATCH_ACTION: &str = "limit_patch";

fn is_live_limit_patch_ack(session_id: &str, event: &ActionEvent) -> bool {
    event.session_id == session_id && event.action == LIVE_LIMIT_PATCH_ACTION
}

async fn recv_live_limit_patch_ack(
    rx: &mut tokio::sync::broadcast::Receiver<(String, Vec<u8>)>,
    session_id: &str,
    joint: &str,
    operator_id: &str,
) -> ActionEvent {
    loop {
        match rx.recv().await {
            Ok((topic, bytes)) if topic == TOPIC_AUDIT_ACTION => {
                if let Ok(event) = decode_action_event(&bytes) {
                    if is_live_limit_patch_ack(session_id, &event) {
                        return event;
                    }
                }
            }
            Ok(_) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                return ActionEvent {
                    timestamp_ms: 0,
                    session_id: session_id.to_string(),
                    operator_id: operator_id.to_string(),
                    joint: joint.to_string(),
                    action: LIVE_LIMIT_PATCH_ACTION.to_string(),
                    revision: 0,
                    accepted: false,
                    reject_reason: "audit channel closed".to_string(),
                    persist_status: ProtoPersistStatus::Failed as i32,
                    config_revision: String::new(),
                };
            }
        }
    }
}

fn decode_action_event(envelope_bytes: &[u8]) -> Result<ActionEvent, ()> {
    let env = Envelope::decode(envelope_bytes).map_err(|_| ())?;
    ActionEvent::decode(env.payload.as_slice()).map_err(|_| ())
}

#[cfg(test)]
fn encode_action_event_envelope(event: &ActionEvent) -> Vec<u8> {
    Envelope {
        timestamp_ms: event.timestamp_ms,
        source_node: "test".to_string(),
        message_type: "marengo.v1.ActionEvent".to_string(),
        payload: event.encode_to_vec(),
    }
    .encode_to_vec()
}

fn add_joint_apply(
    repo_root: &FsPath,
    active_dir: &FsPath,
    target_slug: &str,
    target_dir: &FsPath,
    joint: &str,
    request: &ApplyActuatorJson,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    let current_revision = match profile_content_revision(target_dir) {
        Ok(revision) => revision,
        Err(error) => {
            return apply_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                PersistStatus::Failed,
                None,
                None,
                None,
            );
        }
    };
    if let Err(message) =
        verify_expected_revision(&current_revision, request.expected_revision.as_deref())
    {
        return apply_error(
            StatusCode::CONFLICT,
            message,
            PersistStatus::Failed,
            Some(ApplyDecision::Noop),
            None,
            None,
        );
    }
    if joint_in_motors(target_dir, joint).unwrap_or(false) {
        let current = limit_patch_from_motor(target_dir, joint).ok();
        return (
            StatusCode::OK,
            Json(ApplyActuatorResultJson {
                ok: true,
                message: format!("{joint} is already in {target_slug}"),
                applied_live: false,
                restart_required: false,
                revision: Some(current_revision),
                persist_status: PersistStatus::Durable,
                decision: Some(ApplyDecision::Noop),
                before: current.clone(),
                after: current,
            }),
        );
    }
    if !joint_in_profile_urdf(repo_root, target_dir, joint).unwrap_or(false) {
        return apply_error(
            StatusCode::BAD_REQUEST,
            unsupported_membership_message(repo_root, joint, target_dir),
            PersistStatus::Failed,
            Some(ApplyDecision::UnsupportedMembership),
            None,
            limit_patch_from_motor(active_dir, joint).ok(),
        );
    }
    let source = match limit_patch_from_motor(active_dir, joint) {
        Ok(source) => source,
        Err(error) => {
            return apply_error(
                StatusCode::BAD_REQUEST,
                format!("joint is not available from the active profile: {error}"),
                PersistStatus::Failed,
                Some(ApplyDecision::UnsupportedMembership),
                None,
                None,
            );
        }
    };
    let result = match add_joint_from_source(
        repo_root,
        target_dir,
        active_dir,
        joint,
        request.expected_revision.as_deref(),
    ) {
        Ok(result) => result,
        Err(error) => {
            return apply_error(
                StatusCode::BAD_REQUEST,
                error.to_string(),
                PersistStatus::Failed,
                Some(ApplyDecision::Add),
                None,
                Some(source),
            );
        }
    };
    let is_active = target_slug == profile_slug_from_dir(repo_root, active_dir);
    (
        StatusCode::OK,
        Json(ApplyActuatorResultJson {
            ok: true,
            message: format!("Added {joint} to {target_slug}"),
            applied_live: false,
            restart_required: is_active,
            revision: Some(result.revision),
            persist_status: PersistStatus::Durable,
            decision: Some(ApplyDecision::Add),
            before: None,
            after: Some(source),
        }),
    )
}

fn resolve_target_slug(target: &str) -> Option<&str> {
    let target = target.trim();
    if let Some(slug) = profile_slug_for_preset(target) {
        return Some(slug);
    }
    BRINGUP_PROFILE_SLUGS.contains(&target).then_some(target)
}

fn classify_preview(
    before: Option<&LimitPatch>,
    source: &LimitPatch,
    target_supports_joint: bool,
) -> ApplyDecision {
    match before {
        Some(current) if current == source => ApplyDecision::Noop,
        Some(_) => ApplyDecision::Overwrite,
        None if target_supports_joint => ApplyDecision::Add,
        None => ApplyDecision::UnsupportedMembership,
    }
}

fn merge_request_limits(before: &LimitPatch, request: &ApplyActuatorJson) -> LimitPatch {
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

fn verify_expected_revision(actual: &str, expected: Option<&str>) -> Result<(), String> {
    if let Some(expected) = expected {
        if expected != actual {
            return Err(format!(
                "profile revision mismatch: expected {expected}, found {actual}"
            ));
        }
    }
    Ok(())
}

fn profile_slug_from_dir(repo_root: &FsPath, config_dir: &FsPath) -> String {
    let canonical_active =
        std::fs::canonicalize(config_dir).unwrap_or_else(|_| config_dir.to_path_buf());
    for slug in BRINGUP_PROFILE_SLUGS {
        if let Ok(candidate) = resolve_bringup_dir(repo_root, slug) {
            if candidate == canonical_active {
                return (*slug).to_string();
            }
        }
    }
    profile_name(config_dir)
}

fn profile_name(config_dir: &FsPath) -> String {
    config_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("default")
        .to_string()
}

fn unsupported_membership_message(repo_root: &FsPath, joint: &str, target_dir: &FsPath) -> String {
    let memberships = membership_slugs_for_joint(repo_root, joint).unwrap_or_default();
    format!(
        "joint {joint} is not supported by {}; current memberships: {}",
        profile_name(target_dir),
        if memberships.is_empty() {
            "none".to_string()
        } else {
            memberships.join(", ")
        }
    )
}

fn apply_error(
    status: StatusCode,
    message: String,
    persist_status: PersistStatus,
    decision: Option<ApplyDecision>,
    before: Option<LimitPatch>,
    after: Option<LimitPatch>,
) -> (StatusCode, Json<ApplyActuatorResultJson>) {
    (
        status,
        Json(ApplyActuatorResultJson {
            ok: false,
            message,
            applied_live: false,
            restart_required: false,
            revision: None,
            persist_status,
            decision,
            before,
            after,
        }),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn limits(upper: f64) -> LimitPatch {
        LimitPatch {
            joint: "right_elbow_pitch".to_string(),
            position_lower_rad: -0.1,
            position_upper_rad: upper,
            torque_limit_nm: Some(2.0),
            position_soft_lower_rad: Some(-0.05),
            position_soft_upper_rad: Some(upper - 0.05),
            velocity_max_rad_s: Some(0.8),
        }
    }

    #[test]
    fn resolves_preset_id_or_profile_slug() {
        assert_eq!(resolve_target_slug("bench_4dof"), Some("arm_4dof_right"));
        assert_eq!(
            resolve_target_slug("arm_3dof_right"),
            Some("arm_3dof_right")
        );
        assert_eq!(resolve_target_slug("bench_unknown"), None);
        assert_eq!(resolve_target_slug("../arm_4dof_right"), None);
    }

    #[test]
    fn preview_classifies_add_overwrite_noop_and_unsupported() {
        let source = limits(1.2);
        assert_eq!(classify_preview(None, &source, true), ApplyDecision::Add);
        assert_eq!(
            classify_preview(Some(&limits(1.0)), &source, true),
            ApplyDecision::Overwrite
        );
        assert_eq!(
            classify_preview(Some(&source), &source, true),
            ApplyDecision::Noop
        );
        assert_eq!(
            classify_preview(None, &source, false),
            ApplyDecision::UnsupportedMembership
        );
    }

    #[test]
    fn preview_reads_active_limits_without_writing_target() {
        let root = marengo_config::resolve_repo_root();
        let active = resolve_bringup_dir(&root, "arm_4dof_right").expect("active profile");
        let target = resolve_bringup_dir(&root, "arm_3dof_right").expect("target profile");
        let revision_before = profile_content_revision(&target).expect("revision");
        let request = ApplyActuatorJson {
            target_profile: "bench_3dof".to_string(),
            expected_revision: Some(revision_before.clone()),
            operator_id: "test-operator".to_string(),
            op: ApplyOperation::Preview,
            joint: "right_elbow_pitch".to_string(),
            position_lower_rad: None,
            position_upper_rad: None,
            torque_limit_nm: None,
            position_soft_lower_rad: None,
            position_soft_upper_rad: None,
            velocity_max_rad_s: None,
        };

        let (status, Json(result)) = apply_actuator(&root, &active, request);

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(result.decision, Some(ApplyDecision::UnsupportedMembership));
        assert!(result.after.is_some());
        assert_eq!(
            profile_content_revision(&target).expect("unchanged revision"),
            revision_before
        );
    }

    #[tokio::test]
    async fn profile_snapshot_rejects_non_allowlisted_slug() {
        let result = get_profile_snapshot(Path("../config".to_string())).await;
        assert!(matches!(result, Err(StatusCode::NOT_FOUND)));
    }

    #[test]
    fn inactive_upsert_advances_revision_and_rejects_stale_cas() {
        let root = marengo_config::resolve_repo_root();
        let source = resolve_bringup_dir(&root, "arm_3dof_right").expect("3dof");
        let temp = tempfile::tempdir().expect("tempdir");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(source.join(name), temp.path().join(name)).expect("copy");
        }
        // Point MARENGO_CONFIG_DIR away so active != target; write via upsert helper path.
        let before = profile_content_revision(temp.path()).expect("rev");
        let patch = LimitPatch {
            joint: "right_shoulder_pitch".to_string(),
            position_lower_rad: -0.5,
            position_upper_rad: 2.5,
            torque_limit_nm: Some(3.0),
            position_soft_lower_rad: Some(-0.4),
            position_soft_upper_rad: Some(2.4),
            velocity_max_rad_s: None,
        };
        let ok = upsert_joint_limits(temp.path(), &patch, Some(&before)).expect("cas ok");
        assert_ne!(ok.revision, before);
        let stale = upsert_joint_limits(temp.path(), &patch, Some(&before));
        assert!(stale.is_err(), "stale CAS must fail");
    }

    fn sample_action(
        session_id: &str,
        action: &str,
        accepted: bool,
        persist: ProtoPersistStatus,
    ) -> ActionEvent {
        ActionEvent {
            timestamp_ms: 1,
            session_id: session_id.to_string(),
            operator_id: "op".to_string(),
            joint: "right_elbow_pitch".to_string(),
            action: action.to_string(),
            revision: 1,
            accepted,
            reject_reason: if accepted {
                String::new()
            } else {
                "rejected".to_string()
            },
            persist_status: persist as i32,
            config_revision: "rev-1".to_string(),
        }
    }

    #[test]
    fn live_ack_matcher_ignores_write_behind_persist_action() {
        let session = "sess-1";
        assert!(is_live_limit_patch_ack(
            session,
            &sample_action(session, "limit_patch", true, ProtoPersistStatus::Pending)
        ));
        assert!(!is_live_limit_patch_ack(
            session,
            &sample_action(
                session,
                "limit_patch_persist",
                false,
                ProtoPersistStatus::Failed
            )
        ));
        assert!(!is_live_limit_patch_ack(
            "other",
            &sample_action(session, "limit_patch", true, ProtoPersistStatus::Pending)
        ));
    }

    #[tokio::test]
    async fn recv_live_ack_skips_persist_failed_until_pending() {
        let bus = std::sync::Arc::new(chappe::Bus::new(64));
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = state.subscribe_envelopes();
        let session = "sess-live";
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                session,
                "limit_patch_persist",
                false,
                ProtoPersistStatus::Failed,
            )),
        );
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                session,
                "limit_patch",
                true,
                ProtoPersistStatus::Pending,
            )),
        );
        let event = timeout(
            Duration::from_secs(1),
            recv_live_limit_patch_ack(&mut rx, session, "right_elbow_pitch", "op"),
        )
        .await
        .expect("ack");
        assert!(event.accepted);
        assert_eq!(event.action, "limit_patch");
        assert_eq!(event.persist_status, ProtoPersistStatus::Pending as i32);
    }

    #[tokio::test]
    async fn recv_live_ack_surfaces_live_reject() {
        let bus = std::sync::Arc::new(chappe::Bus::new(64));
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = state.subscribe_envelopes();
        let session = "sess-reject";
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                session,
                "limit_patch",
                false,
                ProtoPersistStatus::NotApplicable,
            )),
        );
        let event = timeout(
            Duration::from_secs(1),
            recv_live_limit_patch_ack(&mut rx, session, "right_elbow_pitch", "op"),
        )
        .await
        .expect("ack");
        assert!(!event.accepted);
        assert_eq!(event.reject_reason, "rejected");
    }

    #[tokio::test]
    async fn recv_live_ack_times_out_without_matching_session() {
        let bus = std::sync::Arc::new(chappe::Bus::new(64));
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::clone(&bus)));
        let mut rx = state.subscribe_envelopes();
        state.ingest_runtime_frame(
            TOPIC_AUDIT_ACTION.to_string(),
            encode_action_event_envelope(&sample_action(
                "other-session",
                "limit_patch",
                true,
                ProtoPersistStatus::Pending,
            )),
        );
        let timed_out = timeout(
            Duration::from_millis(50),
            recv_live_limit_patch_ack(&mut rx, "wanted-session", "right_elbow_pitch", "op"),
        )
        .await;
        assert!(
            timed_out.is_err(),
            "mismatched session must not satisfy wait"
        );
    }
}
