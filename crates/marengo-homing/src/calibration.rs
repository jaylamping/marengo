use serde::{Deserialize, Serialize};

/// One joint entry in the host calibration registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JointCalibration {
    pub joint: String,
    pub device_id: u8,
    pub can_interface: String,
    pub method: String,
    pub home_offset_rad: f64,
    pub verified_position_rad: f64,
    pub sign_test_passed: bool,
    pub timestamp_utc: String,
    pub config_revision: Option<String>,
    pub operator: String,
}

/// Full calibration registry file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CalibrationRecord {
    pub joints: Vec<JointCalibration>,
}

impl CalibrationRecord {
    pub fn find_joint(&self, joint: &str) -> Option<&JointCalibration> {
        self.joints.iter().find(|j| j.joint == joint)
    }

    pub fn upsert(&mut self, entry: JointCalibration) {
        if let Some(existing) = self.joints.iter_mut().find(|j| j.joint == entry.joint) {
            *existing = entry;
        } else {
            self.joints.push(entry);
        }
    }
}
