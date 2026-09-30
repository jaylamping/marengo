use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use marengo_config::MotorEntry;
use thiserror::Error;

use crate::calibration::{CalibrationRecord, JointCalibration};
use crate::sensor::{check_sensor_health_at_boot, SensorHealth, SensorProvider, ThreeHallInputs};
use crate::JointHomingState;

#[derive(Debug, Error, PartialEq)]
pub enum RegistryError {
    #[error("io {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("parse {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("joint {joint} not configured for homing")]
    UnknownJoint { joint: String },
    #[error("joint {joint}: {message}")]
    Joint { joint: String, message: String },
}

/// Runtime homing registry — joint states + calibration persistence.
pub struct HomingRegistry {
    record_path: PathBuf,
    calibration: CalibrationRecord,
    joint_states: HashMap<String, JointHomingState>,
    out_of_limits: HashMap<String, bool>,
    sensor_health: HashMap<String, crate::sensor::SensorHealth>,
    configured_joints: Vec<String>,
    zero_tolerance_rad: f64,
}

impl HomingRegistry {
    pub fn new(
        repo_root: impl AsRef<Path>,
        record_rel_path: &str,
        configured_joints: Vec<String>,
        zero_tolerance_rad: f64,
    ) -> Result<Self, RegistryError> {
        let record_path = std::env::var("MARENGO_CALIBRATION_RECORD")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repo_root.as_ref().join(record_rel_path));
        let calibration = if record_path.is_file() {
            load_calibration(&record_path).unwrap_or_default()
        } else {
            CalibrationRecord::default()
        };
        let mut joint_states: HashMap<String, JointHomingState> = configured_joints
            .iter()
            .map(|j| (j.clone(), JointHomingState::Unhomed))
            .collect();
        for entry in &calibration.joints {
            if joint_states.contains_key(&entry.joint) {
                joint_states.insert(entry.joint.clone(), JointHomingState::Verified);
            }
        }
        Ok(Self {
            record_path,
            calibration,
            joint_states,
            out_of_limits: HashMap::new(),
            sensor_health: HashMap::new(),
            configured_joints,
            zero_tolerance_rad,
        })
    }

    pub fn joint_state(&self, joint: &str) -> JointHomingState {
        self.joint_states
            .get(joint)
            .copied()
            .unwrap_or(JointHomingState::Unhomed)
    }

    pub fn is_out_of_limits(&self, joint: &str) -> bool {
        self.out_of_limits.get(joint).copied().unwrap_or(false)
    }

    pub fn mark_out_of_limits(&mut self, joint: &str) {
        if self.configured_joints.iter().any(|j| j == joint) {
            self.out_of_limits.insert(joint.to_string(), true);
        }
    }

    pub fn clear_out_of_limits(&mut self, joint: &str) {
        self.out_of_limits.remove(joint);
    }

    pub fn all_verified(&self) -> bool {
        self.configured_joints
            .iter()
            .all(|j| self.joint_state(j) == JointHomingState::Verified)
    }

    pub fn any_faulted(&self) -> bool {
        self.joint_states
            .values()
            .any(|s| *s == JointHomingState::Faulted)
    }

    pub fn set_state(&mut self, joint: &str, state: JointHomingState) {
        if self.configured_joints.iter().any(|j| j == joint) {
            self.joint_states.insert(joint.to_string(), state);
        }
    }

    pub fn mark_fault(&mut self, joint: &str, message: &str) {
        self.set_state(joint, JointHomingState::Faulted);
        let _ = message;
    }

    pub fn calibration(&self) -> &CalibrationRecord {
        &self.calibration
    }

    pub fn zero_tolerance_rad(&self) -> f64 {
        self.zero_tolerance_rad
    }

    pub fn configured_joints(&self) -> &[String] {
        &self.configured_joints
    }

    pub fn sensor_health(&self, joint: &str) -> crate::sensor::SensorHealth {
        self.sensor_health
            .get(joint)
            .copied()
            .unwrap_or(crate::sensor::SensorHealth::Unknown)
    }

    pub fn set_sensor_health(&mut self, joint: &str, health: crate::sensor::SensorHealth) {
        self.sensor_health.insert(joint.to_string(), health);
    }

    /// Startup sensor health for a joint with configured Hall inputs.
    pub fn check_sensor_health(
        &mut self,
        joint: &str,
        provider: &impl SensorProvider,
        inputs: &ThreeHallInputs,
        allow_overlap: bool,
    ) -> Result<SensorHealth, crate::sensor::SensorError> {
        let health = check_sensor_health_at_boot(provider, inputs, allow_overlap)?;
        self.set_sensor_health(joint, health);
        Ok(health)
    }

    /// Record successful manual or Hall homing calibration.
    #[allow(clippy::too_many_arguments)]
    pub fn record_verification(
        &mut self,
        motor: &MotorEntry,
        method: &str,
        home_offset_rad: f64,
        verified_position_rad: f64,
        sign_test_passed: bool,
        operator: &str,
        config_revision: Option<String>,
    ) -> Result<(), RegistryError> {
        let entry = JointCalibration {
            joint: motor.joint.clone(),
            device_id: motor.device_id,
            can_interface: motor.can_interface.clone(),
            method: method.to_string(),
            home_offset_rad,
            verified_position_rad,
            sign_test_passed,
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            config_revision,
            operator: operator.to_string(),
        };
        self.calibration.upsert(entry);
        self.persist()?;
        self.set_state(&motor.joint, JointHomingState::Verified);
        self.clear_out_of_limits(&motor.joint);
        Ok(())
    }

    pub fn persist(&self) -> Result<(), RegistryError> {
        if let Some(parent) = self.record_path.parent() {
            fs::create_dir_all(parent).map_err(|e| RegistryError::Io {
                path: parent.to_path_buf(),
                message: e.to_string(),
            })?;
        }
        let text = serde_yaml::to_string(&self.calibration).map_err(|e| RegistryError::Parse {
            path: self.record_path.clone(),
            message: e.to_string(),
        })?;
        fs::write(&self.record_path, text).map_err(|e| RegistryError::Io {
            path: self.record_path.clone(),
            message: e.to_string(),
        })
    }

    pub fn require_ready(&self) -> Result<(), RegistryError> {
        if self.any_faulted() {
            return Err(RegistryError::Joint {
                joint: "*".to_string(),
                message: "one or more joints faulted".to_string(),
            });
        }
        for joint in &self.configured_joints {
            if self.joint_state(joint) != JointHomingState::Verified {
                return Err(RegistryError::Joint {
                    joint: joint.clone(),
                    message: format!(
                        "not verified (state {:?}); run set-zero and home first",
                        self.joint_state(joint)
                    ),
                });
            }
        }
        Ok(())
    }

    /// Mark configured joints verified without a live encoder check (unit tests; no disk write).
    pub fn bench_mark_all_verified(&mut self, motors: &[MotorEntry]) -> Result<(), RegistryError> {
        for motor in motors {
            if !self.configured_joints.iter().any(|j| j == &motor.joint) {
                continue;
            }
            let entry = JointCalibration {
                joint: motor.joint.clone(),
                device_id: motor.device_id,
                can_interface: motor.can_interface.clone(),
                method: "bench_test".to_string(),
                home_offset_rad: 0.0,
                verified_position_rad: 0.0,
                sign_test_passed: true,
                timestamp_utc: chrono::Utc::now().to_rfc3339(),
                config_revision: None,
                operator: "bench_test".to_string(),
            };
            self.calibration.upsert(entry);
            self.set_state(&motor.joint, JointHomingState::Verified);
        }
        Ok(())
    }
}

pub fn load_calibration(path: &Path) -> Result<CalibrationRecord, RegistryError> {
    let text = fs::read_to_string(path).map_err(|e| RegistryError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    serde_yaml::from_str(&text).map_err(|e| RegistryError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::env;

    use marengo_config::MotorType;

    use super::*;

    fn temp_record_path() -> PathBuf {
        let dir = env::temp_dir().join(format!("marengo-homing-test-{}", std::process::id()));
        dir.join("zero_registry.yaml")
    }

    fn sample_motor() -> MotorEntry {
        MotorEntry {
            joint: "shoulder_pitch".to_string(),
            driver: "robstride".to_string(),
            motor_type: MotorType::Rs03,
            can_interface: "can0".to_string(),
            device_id: 12,
            direction: 1,
            gear_ratio: 1.0,
            recv_can_id: 0,
            firmware_version: "0.3.1.42".to_string(),
            bench: marengo_config::MotorBenchLimits {
                position_lower_rad: -0.9,
                position_upper_rad: 3.17,
                velocity_limit_rad_s: 0.5,
                torque_limit_nm: 5.0,
            },
        }
    }

    #[test]
    fn require_ready_fails_when_unhomed() {
        let root = env::temp_dir();
        let rel = format!(
            "marengo-homing-test-{}/zero_registry.yaml",
            std::process::id()
        );
        let reg = HomingRegistry::new(&root, &rel, vec!["shoulder_pitch".to_string()], 0.05)
            .expect("registry");
        let err = reg.require_ready().expect_err("not ready");
        assert!(matches!(err, RegistryError::Joint { .. }));
    }

    #[test]
    fn record_verification_marks_joint_verified() {
        let root = env::temp_dir();
        let path = temp_record_path();
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let mut reg = HomingRegistry::new(&root, &rel, vec!["shoulder_pitch".to_string()], 0.05)
            .expect("registry");
        reg.record_verification(
            &sample_motor(),
            "manual_reference",
            0.0,
            0.01,
            true,
            "test",
            None,
        )
        .expect("record");
        assert_eq!(
            reg.joint_state("shoulder_pitch"),
            JointHomingState::Verified
        );
        reg.require_ready().expect("ready");
        let _ = fs::remove_file(&path);
    }
}
