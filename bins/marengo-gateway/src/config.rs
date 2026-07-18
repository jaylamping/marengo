//! Operator config snapshot + patch (Consul; ADR 0012 phase 1).
//!
//! Reads active bringup YAML from `MARENGO_CONFIG_DIR`, applies validated patches
//! to `motors.yaml` / `control.yaml`, and records audit rows in `config_overrides`.

use std::path::{Path, PathBuf};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use marengo_config::{
    load_control_config_from, load_motors_config_from, load_robot_config_from,
    validate_control_against_limits, validate_control_config, validate_motors_against_robot,
    MotorType,
};
use serde::{Deserialize, Serialize};

use crate::logs::authorize_logs;
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

pub async fn get_config_snapshot(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<ConfigSnapshotJson>, StatusCode> {
    authorize_logs(&headers)?;
    let _logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let config_dir = resolve_config_dir();
    let robot = load_robot_config_from(&config_dir).map_err(|_| StatusCode::NOT_FOUND)?;
    let motors = load_motors_config_from(&config_dir).map_err(|_| StatusCode::NOT_FOUND)?;
    let control = load_control_config_from(&config_dir).map_err(|_| StatusCode::NOT_FOUND)?;

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

    let control_limits: Vec<JointControlLimitsJson> = robot
        .robot
        .joints
        .iter()
        .filter_map(|joint| {
            control.control.joints.get(joint).map(|entry| JointControlLimitsJson {
                joint: joint.clone(),
                position_soft_lower_rad: entry.position_soft_lower_rad,
                position_soft_upper_rad: entry.position_soft_upper_rad,
                velocity_max_rad_s: entry.velocity_max_rad_s,
            })
        })
        .collect();

    Ok(Json(ConfigSnapshotJson {
        profile: profile_name(&config_dir),
        config_dir: config_dir.display().to_string(),
        joints: robot.robot.joints.clone(),
        motors: motor_rows,
        control_limits,
    }))
}

pub async fn post_config_patch(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(patch): Json<ConfigPatchJson>,
) -> Result<Json<ConfigPatchResultJson>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if patch.joint.trim().is_empty() {
        return Ok(Json(ConfigPatchResultJson {
            ok: false,
            message: "joint name required".to_string(),
            restart_required: false,
        }));
    }

    let config_dir = resolve_config_dir();
    let motors_path = config_dir.join("motors.yaml");
    let control_path = config_dir.join("control.yaml");

    let mut motors_doc: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&motors_path).map_err(|_| StatusCode::NOT_FOUND)?)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut control_doc: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&control_path).map_err(|_| StatusCode::NOT_FOUND)?)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    apply_motor_patch(&mut motors_doc, &patch.joint, &patch)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    apply_control_patch(&mut control_doc, &patch.joint, &patch)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let motors_text =
        serde_yaml::to_string(&motors_doc).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let control_text =
        serde_yaml::to_string(&control_doc).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    std::fs::write(&motors_path, &motors_text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(&control_path, &control_text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let robot = load_robot_config_from(&config_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let motors = load_motors_config_from(&config_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let control = load_control_config_from(&config_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    validate_control_config(&control).map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_motors_against_robot(&robot, &motors).map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_control_against_limits(&robot, &motors, &control)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let audit_key = format!("config.patch.{}", patch.joint);
    let audit_json = serde_json::to_string(&patch).unwrap_or_default();
    let source = if patch.operator_id.is_empty() {
        "consul".to_string()
    } else {
        patch.operator_id.clone()
    };
    logs.store
        .set_config_override(
            &audit_key,
            &audit_json,
            &source,
            marengo_store::now_ms(),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ConfigPatchResultJson {
        ok: true,
        message: format!("Updated {} in {}", patch.joint, profile_name(&config_dir)),
        restart_required: true,
    }))
}

fn apply_motor_patch(
    doc: &mut serde_yaml::Value,
    joint: &str,
    patch: &ConfigPatchJson,
) -> Result<(), String> {
    let motors = doc
        .get_mut("motors")
        .and_then(|v| v.as_sequence_mut())
        .ok_or_else(|| "motors list missing".to_string())?;
    let entry = motors
        .iter_mut()
        .find(|m| m.get("joint").and_then(|j| j.as_str()) == Some(joint))
        .ok_or_else(|| format!("joint '{joint}' not in motors.yaml"))?;

    if let Some(device_id) = patch.device_id {
        entry
            .as_mapping_mut()
            .ok_or_else(|| "motor entry not a mapping".to_string())?
            .insert(
                serde_yaml::Value::from("device_id"),
                serde_yaml::Value::from(u64::from(device_id)),
            );
    }
    if let Some(can_interface) = &patch.can_interface {
        entry
            .as_mapping_mut()
            .ok_or_else(|| "motor entry not a mapping".to_string())?
            .insert(
                serde_yaml::Value::from("can_interface"),
                serde_yaml::Value::from(can_interface.clone()),
            );
    }
    if let Some(direction) = patch.direction {
        entry
            .as_mapping_mut()
            .ok_or_else(|| "motor entry not a mapping".to_string())?
            .insert(
                serde_yaml::Value::from("direction"),
                serde_yaml::Value::from(i64::from(direction)),
            );
    }

    let bench_keys = [
        ("position_lower_rad", patch.position_lower_rad),
        ("position_upper_rad", patch.position_upper_rad),
        ("torque_limit_nm", patch.torque_limit_nm),
    ];
    for (key, value) in bench_keys {
        if let Some(v) = value {
            let mapping = entry
                .as_mapping_mut()
                .ok_or_else(|| "motor entry not a mapping".to_string())?;
            let bench = mapping
                .entry(serde_yaml::Value::from("bench"))
                .or_insert_with(|| serde_yaml::Mapping::new().into());
            bench
                .as_mapping_mut()
                .ok_or_else(|| "bench not a mapping".to_string())?
                .insert(serde_yaml::Value::from(key), serde_yaml::Value::from(v));
        }
    }
    Ok(())
}

fn apply_control_patch(
    doc: &mut serde_yaml::Value,
    joint: &str,
    patch: &ConfigPatchJson,
) -> Result<(), String> {
    let has_control_patch = patch.position_soft_lower_rad.is_some()
        || patch.position_soft_upper_rad.is_some()
        || patch.velocity_max_rad_s.is_some();
    if !has_control_patch {
        return Ok(());
    }

    let joints = doc
        .get_mut("control")
        .and_then(|c| c.get_mut("joints"))
        .and_then(|j| j.as_mapping_mut())
        .ok_or_else(|| "control.joints missing".to_string())?;
    let entry = joints
        .get_mut(serde_yaml::Value::from(joint))
        .ok_or_else(|| format!("joint '{joint}' not in control.yaml"))?;

    let mapping = entry
        .as_mapping_mut()
        .ok_or_else(|| "control joint entry not a mapping".to_string())?;
    if let Some(v) = patch.position_soft_lower_rad {
        mapping.insert(
            serde_yaml::Value::from("position_soft_lower_rad"),
            serde_yaml::Value::from(v),
        );
    }
    if let Some(v) = patch.position_soft_upper_rad {
        mapping.insert(
            serde_yaml::Value::from("position_soft_upper_rad"),
            serde_yaml::Value::from(v),
        );
    }
    if let Some(v) = patch.velocity_max_rad_s {
        mapping.insert(
            serde_yaml::Value::from("velocity_max_rad_s"),
            serde_yaml::Value::from(v),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_motor_patch_updates_device_and_limits() {
        let yaml = r#"
motors:
  - joint: right_shoulder_roll
    device_id: 1
    bench:
      position_lower_rad: -1.0
      position_upper_rad: 1.0
"#;
        let mut doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let patch = ConfigPatchJson {
            joint: "right_shoulder_roll".to_string(),
            device_id: Some(3),
            can_interface: Some("can0".to_string()),
            direction: None,
            position_lower_rad: Some(-1.57),
            position_upper_rad: Some(1.57),
            torque_limit_nm: Some(5.0),
            position_soft_lower_rad: None,
            position_soft_upper_rad: None,
            velocity_max_rad_s: None,
            operator_id: String::new(),
        };
        apply_motor_patch(&mut doc, "right_shoulder_roll", &patch).unwrap();
        let out = serde_yaml::to_string(&doc).unwrap();
        assert!(out.contains("device_id: 3"));
        assert!(out.contains("position_lower_rad: -1.57"));
    }

    #[test]
    fn arm_2dof_right_profile_loads() {
        let root = marengo_config::resolve_repo_root();
        let config_dir = root.join("config/bringup/arm_2dof_right");
        if !config_dir.is_dir() {
            return;
        }
        let robot = load_robot_config_from(&config_dir).expect("robot");
        let motors = load_motors_config_from(&config_dir).expect("motors");
        let control = load_control_config_from(&config_dir).expect("control");
        validate_control_config(&control).expect("control valid");
        validate_motors_against_robot(&robot, &motors).expect("motors align");
        validate_control_against_limits(&robot, &motors, &control).expect("limits align");
        assert_eq!(robot.robot.joints.len(), 2);
        assert_eq!(motors.motors.len(), 2);
        let urdf = root.join(&robot.robot.urdf);
        assert!(urdf.is_file(), "URDF missing: {}", urdf.display());
    }
}
