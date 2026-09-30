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

/// Historical calibration and legacy local policy state.
///
/// Davout owns current output permission independently. This registry cannot
/// mint it, including after a successful scalar history record.
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
    /// Bind the configured history resource deterministically. Environment
    /// selection belongs to runtime composition, not this library.
    pub fn new(
        repo_root: impl AsRef<Path>,
        record_rel_path: &str,
        configured_joints: Vec<String>,
        zero_tolerance_rad: f64,
    ) -> Result<Self, RegistryError> {
        Self::with_record_path(
            repo_root.as_ref().join(record_rel_path),
            configured_joints,
            zero_tolerance_rad,
        )
    }

    /// Load historical calibration without granting a current reference. A
    /// missing resource is empty history; every other read/parse failure returns.
    /// Construction never rewrites the resource. The path is used as supplied.
    pub fn with_record_path(
        record_path: impl AsRef<Path>,
        configured_joints: Vec<String>,
        zero_tolerance_rad: f64,
    ) -> Result<Self, RegistryError> {
        let record_path = record_path.as_ref().to_path_buf();
        let calibration = load_calibration(&record_path)?;
        let joint_states = configured_joints
            .iter()
            .map(|j| (j.clone(), JointHomingState::Unhomed))
            .collect();
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

    pub(crate) fn set_state(&mut self, joint: &str, state: JointHomingState) {
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

    /// Record supplied legacy history and local policy state.
    /// This does not prove device reference or grant Davout output permission.
    /// Failed writing leaves the previous in-memory history and local state intact.
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
        let mut staged = self.calibration.clone();
        staged.upsert(entry);
        self.persist_record(&staged)?;
        self.calibration = staged;
        self.set_state(&motor.joint, JointHomingState::Verified);
        self.clear_out_of_limits(&motor.joint);
        Ok(())
    }

    pub fn persist(&self) -> Result<(), RegistryError> {
        self.persist_record(&self.calibration)
    }

    fn persist_record(&self, calibration: &CalibrationRecord) -> Result<(), RegistryError> {
        if let Some(parent) = self.record_path.parent() {
            fs::create_dir_all(parent).map_err(|e| RegistryError::Io {
                path: parent.to_path_buf(),
                message: e.to_string(),
            })?;
        }
        let text = serde_yaml::to_string(calibration).map_err(|e| RegistryError::Parse {
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
                        "reference is not verified in this process (state {:?})",
                        self.joint_state(joint)
                    ),
                });
            }
        }
        Ok(())
    }
}

pub fn load_calibration(path: &Path) -> Result<CalibrationRecord, RegistryError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CalibrationRecord::default());
        }
        Err(error) => {
            return Err(RegistryError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
    };
    serde_yaml::from_str(&text).map_err(|e| RegistryError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}
