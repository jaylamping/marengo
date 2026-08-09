//! Homing state, sensor health, calibration registry, and verification logic.
//!
//! Pure logic — GPIO/hardware I/O lives in bins via [`SensorProvider`].

mod calibration;
mod registry;
mod sensor;
mod verify;

pub use calibration::{CalibrationRecord, JointCalibration};
pub use registry::HomingRegistry;
pub use sensor::{
    classify_sensor_pattern, MemorySensorProvider, SensorHealth, SensorPattern, SensorProvider,
    SensorSnapshot, ThreeHallInputs,
};
pub use verify::{verify_manual_reference, VerifyError, VerifyOutcome};

use marengo_config::{EffectiveHomingJoint, HomingConfigFile, HomingMethod};

/// Per-joint homing lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointHomingState {
    Unhomed,
    Homing,
    Verified,
    Faulted,
}

/// Build effective homing config for every joint listed in robot config.
pub fn effective_homing_for_robot(
    homing: &HomingConfigFile,
    robot_joints: &[String],
) -> Vec<EffectiveHomingJoint> {
    robot_joints
        .iter()
        .filter_map(|j| homing.homing.effective_joint(j))
        .collect()
}

/// Whether a homing method requires physical Hall inputs.
pub fn method_requires_sensors(method: &HomingMethod) -> bool {
    matches!(method, HomingMethod::HallThreeSensor)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::HashMap;
    use std::path::PathBuf;

    use marengo_config::{load_homing_config, resolve_repo_root};

    use super::*;
    use crate::sensor::MemorySensorProvider;

    fn repo_root() -> PathBuf {
        resolve_repo_root()
    }

    #[test]
    fn effective_homing_lists_robot_joints() {
        let homing = load_homing_config(repo_root()).expect("homing");
        let joints = vec![
            "right_shoulder_roll".to_string(),
            "right_shoulder_pitch".to_string(),
            "right_upper_arm_yaw".to_string(),
            "right_elbow_pitch".to_string(),
        ];
        let eff = effective_homing_for_robot(&homing, &joints);
        assert_eq!(eff.len(), 4);
    }

    #[test]
    fn sensor_pattern_classifies_home_only() {
        let snap = SensorSnapshot {
            home: true,
            min_limit: false,
            max_limit: false,
        };
        let pattern = classify_sensor_pattern(snap, false).expect("home");
        assert_eq!(pattern, SensorPattern::HomeReference);
    }

    #[test]
    fn sensor_pattern_rejects_overlap_by_default() {
        let snap = SensorSnapshot {
            home: true,
            min_limit: true,
            max_limit: false,
        };
        assert!(classify_sensor_pattern(snap, false).is_err());
    }

    #[test]
    fn memory_provider_reads_gpio_state() {
        let mut states = HashMap::new();
        states.insert(23u8, true);
        let provider = MemorySensorProvider::new(states);
        let inputs = ThreeHallInputs {
            home_gpio: 23,
            min_gpio: 24,
            max_gpio: 25,
            active_high: true,
        };
        let snap = provider.read_three_hall(&inputs).expect("read");
        assert!(snap.home);
        assert!(!snap.min_limit);
    }
}
