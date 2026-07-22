//! Operator config snapshot + patch (Consul; ADR 0012).
//!
//! Active live limits go through Pi ACK (`/config/actuators/apply`). `/config/patch`
//! is a thin facade for Set Limits compatibility.

use std::path::{Path, PathBuf};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use marengo_config::{
    load_control_config_from, load_motors_config_from, load_robot_config_from,
    profile_content_revision, resolve_joint_velocity_cap, MotorType,
};
use serde::{Deserialize, Serialize};

use crate::logs::log_token_from_env;
use crate::profiles::{self, ApplyActuatorJson, ApplyOperation};
use crate::state::SharedState;

#[derive(Serialize)]
pub struct MotorBenchLimitsJson {
    pub position_lower_rad: f64,
    pub position_upper_rad: f64,
    pub torque_limit_nm: f64,
}

#[derive(Serialize)]
pub struct MotorConfigEntryJson {
    pub joint: String,
    pub can_interface: String,
    pub device_id: u8,
    pub direction: i8,
    pub motor_type: String,
    pub bench: MotorBenchLimitsJson,
}

#[derive(Serialize)]
pub struct JointControlLimitsJson {
    pub joint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_soft_lower_rad: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_soft_upper_rad: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub velocity_max_rad_s: Option<f64>,
}

#[derive(Serialize)]
pub struct ConfigSnapshotJson {
    pub profile: String,
    pub config_dir: String,
    pub joints: Vec<String>,
    pub motors: Vec<MotorConfigEntryJson>,
    pub control_limits: Vec<JointControlLimitsJson>,
    /// Content hash of bringup YAML (CAS token).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub revision: String,
    /// False when write-behind failed after a live apply (not NeedsRestart).
    pub persist_ok: bool,
}

#[derive(Deserialize, Serialize)]
pub struct ConfigPatchJson {
    pub joint: String,
    pub device_id: Option<u8>,
    pub can_interface: Option<String>,
    pub direction: Option<i8>,
    pub position_lower_rad: Option<f64>,
    pub position_upper_rad: Option<f64>,
    pub torque_limit_nm: Option<f64>,
    pub position_soft_lower_rad: Option<f64>,
    pub position_soft_upper_rad: Option<f64>,
    pub velocity_max_rad_s: Option<f64>,
    #[serde(default)]
    pub operator_id: String,
}

#[derive(Serialize)]
pub struct ConfigPatchResultJson {
    pub ok: bool,
    pub message: String,
    pub restart_required: bool,
    /// `durable` | `pending` | `failed` | `n/a` — Durable required before local git sync.
    pub persist_status: String,
}

fn resolve_repo_root() -> PathBuf {
    marengo_config::resolve_repo_root()
}

fn resolve_config_dir() -> PathBuf {
    marengo_config::resolve_config_dir(resolve_repo_root())
}

fn profile_name(config_dir: &Path) -> String {
    config_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string()
}

fn motor_type_label(motor_type: MotorType) -> String {
    match motor_type {
        MotorType::Rs00 => "rs00".to_string(),
        MotorType::Rs02 => "rs02".to_string(),
        MotorType::Rs03 => "rs03".to_string(),
        MotorType::Rs04 => "rs04".to_string(),
    }
}

pub fn snapshot_from_dir(config_dir: &Path) -> Result<ConfigSnapshotJson, StatusCode> {
    let robot = load_robot_config_from(config_dir).map_err(|_| StatusCode::NOT_FOUND)?;
    let motors = load_motors_config_from(config_dir).map_err(|_| StatusCode::NOT_FOUND)?;
    let control = load_control_config_from(config_dir).map_err(|_| StatusCode::NOT_FOUND)?;

    let motor_rows: Vec<MotorConfigEntryJson> = motors
        .motors
        .iter()
        .map(|m| MotorConfigEntryJson {
            joint: m.joint.clone(),
            can_interface: m.can_interface.clone(),
            device_id: m.device_id,
            direction: m.direction,
            motor_type: motor_type_label(m.motor_type),
            bench: MotorBenchLimitsJson {
                position_lower_rad: m.bench.position_lower_rad,
                position_upper_rad: m.bench.position_upper_rad,
                torque_limit_nm: m.bench.torque_limit_nm,
            },
        })
        .collect();

    // ADR 0010: caps resolve joint > actuator_groups > motor_type_defaults.
    // Per-joint YAML often omits velocity_max_rad_s (bench uses actuator_groups).
    let control_limits: Vec<JointControlLimitsJson> = robot
        .robot
        .joints
        .iter()
        .filter_map(|joint| {
            let entry = control.control.joints.get(joint)?;
            let motor_type = motors
                .motors
                .iter()
                .find(|m| m.joint == *joint)
                .map(|m| m.motor_type);
            let velocity_max_rad_s = match motor_type {
                Some(mt) => resolve_joint_velocity_cap(joint, mt, &control.control)
                    .ok()
                    .or(entry.velocity_max_rad_s),
                None => entry.velocity_max_rad_s,
            };
            Some(JointControlLimitsJson {
                joint: joint.clone(),
                position_soft_lower_rad: entry.position_soft_lower_rad,
                position_soft_upper_rad: entry.position_soft_upper_rad,
                velocity_max_rad_s,
            })
        })
        .collect();

    let revision = profile_content_revision(config_dir).unwrap_or_default();
    Ok(ConfigSnapshotJson {
        profile: profile_name(config_dir),
        config_dir: config_dir.display().to_string(),
        joints: robot.robot.joints,
        motors: motor_rows,
        control_limits,
        revision,
        persist_ok: true,
    })
}

pub async fn get_config_snapshot(
    State(state): State<SharedState>,
) -> Result<Json<ConfigSnapshotJson>, StatusCode> {
    // Read-only snapshot is LAN bench telemetry for Consul inventory — do not gate on
    // MARENGO_GATEWAY_LOG_TOKEN (deployed www cannot bake that secret). Mutations stay
    // fail-closed via authorize_config_mutation.
    let _logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config_dir = resolve_config_dir();
    let mut snapshot = snapshot_from_dir(&config_dir)?;
    snapshot.persist_ok = !state.persist_degraded();
    Ok(Json(snapshot))
}

/// Config writes share the gateway token with logs, but fail closed when it is unset.
pub fn authorize_config_mutation(headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = log_token_from_env() else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let provided = headers
        .get("x-marengo-log-token")
        .and_then(|value| value.to_str().ok());
    if provided == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub async fn post_config_patch(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(patch): Json<ConfigPatchJson>,
) -> Result<Json<ConfigPatchResultJson>, StatusCode> {
    authorize_config_mutation(&headers)?;
    let logs = state
        .logs
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .clone();
    if patch.joint.trim().is_empty() {
        return Ok(Json(ConfigPatchResultJson {
            ok: false,
            message: "joint name required".to_string(),
            restart_required: false,
            persist_status: "failed".to_string(),
        }));
    }
    if patch.device_id.is_some() || patch.can_interface.is_some() || patch.direction.is_some() {
        return Ok(Json(ConfigPatchResultJson {
            ok: false,
            message: "motor address and direction changes are not supported by /config/patch"
                .to_string(),
            restart_required: false,
            persist_status: "failed".to_string(),
        }));
    }

    let config_dir = resolve_config_dir();
    let repo_root = resolve_repo_root();
    let active_slug = config_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
        .to_string();
    let revision = profile_content_revision(&config_dir).ok();
    let apply = ApplyActuatorJson {
        target_profile: active_slug,
        expected_revision: revision,
        operator_id: if patch.operator_id.is_empty() {
            "consul".to_string()
        } else {
            patch.operator_id.clone()
        },
        op: ApplyOperation::UpsertLimits,
        joint: patch.joint.clone(),
        position_lower_rad: patch.position_lower_rad,
        position_upper_rad: patch.position_upper_rad,
        torque_limit_nm: patch.torque_limit_nm,
        position_soft_lower_rad: patch.position_soft_lower_rad,
        position_soft_upper_rad: patch.position_soft_upper_rad,
        velocity_max_rad_s: patch.velocity_max_rad_s,
    };
    let (_status, Json(result)) =
        profiles::apply_actuator_async(state, &repo_root, &config_dir, apply).await;

    if result.ok {
        let audit_key = format!("config.patch.{}", patch.joint);
        let audit_json = serde_json::to_string(&patch).unwrap_or_default();
        let source = if patch.operator_id.is_empty() {
            "consul".to_string()
        } else {
            patch.operator_id.clone()
        };
        let _ = logs.store.set_config_override(
            &audit_key,
            &audit_json,
            &source,
            marengo_store::now_ms(),
        );
    }

    Ok(Json(ConfigPatchResultJson {
        ok: result.ok,
        message: result.message,
        restart_required: result.restart_required,
        persist_status: match result.persist_status {
            profiles::PersistStatus::Durable => "durable".to_string(),
            profiles::PersistStatus::Pending => "pending".to_string(),
            profiles::PersistStatus::Failed => "failed".to_string(),
            profiles::PersistStatus::NotApplicable => "n/a".to_string(),
        },
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use marengo_config::{
        validate_control_against_limits, validate_control_config, validate_motors_against_robot,
    };

    use super::*;

    #[test]
    fn arm_3dof_right_profile_loads() {
        let root = marengo_config::resolve_repo_root();
        let config_dir = root.join("config/bringup/arm_3dof_right");
        if !config_dir.is_dir() {
            return;
        }
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let motors = load_motors_config_from(&config_dir).expect("motors");
        let control = load_control_config_from(&config_dir).expect("control");
        validate_control_config(&control).expect("control valid");
        validate_motors_against_robot(&robot, &motors).expect("motors align");
        validate_control_against_limits(&robot, &motors, &control).expect("limits align");
        assert_eq!(robot.robot.joints.len(), 3);
        assert_eq!(motors.motors.len(), 3);
        let urdf = root.join(&robot.robot.urdf);
        assert!(urdf.is_file(), "URDF missing: {}", urdf.display());
    }

    #[test]
    fn arm_4dof_snapshot_resolves_actuator_group_velocity() {
        let root = marengo_config::resolve_repo_root();
        let config_dir = root.join("config/bringup/arm_4dof_right");
        if !config_dir.is_dir() {
            return;
        }
        let snap = snapshot_from_dir(&config_dir).expect("snapshot");
        let pitch = snap
            .control_limits
            .iter()
            .find(|c| c.joint == "right_shoulder_pitch")
            .expect("pitch limits");
        assert_eq!(pitch.velocity_max_rad_s, Some(2.5));
    }
}
