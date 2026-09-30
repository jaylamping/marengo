//! Private admission authority. History and scalar verification are not permits.

use std::cell::Cell;
use std::collections::HashSet;
use std::sync::Arc;

use marengo_config::{
    resolve_joint_velocity_cap, ControlConfigFile, EffectiveHomingJoint, HomingConfigFile,
    MotorEntry, MotorsConfigFile,
};

/// Nonexported, nonserializable and noncloneable owner-local permission.
pub(crate) struct ReferenceAuthority {
    joints: HashSet<String>,
    motors: Vec<MotorEntry>,
    homing: Option<HomingConfigFile>,
    control: Option<ControlConfigFile>,
    realm: Option<Arc<()>>,
    revoked: Cell<bool>,
    generation: Cell<u64>,
}

impl Default for ReferenceAuthority {
    fn default() -> Self {
        Self {
            joints: HashSet::new(),
            motors: Vec::new(),
            homing: None,
            control: None,
            realm: None,
            revoked: Cell::new(false),
            generation: Cell::new(0),
        }
    }
}

impl ReferenceAuthority {
    pub(crate) fn initial_virtual(
        realm: Arc<()>,
        joints: HashSet<String>,
        motors: &MotorsConfigFile,
        homing: &HomingConfigFile,
        control: &ControlConfigFile,
    ) -> Self {
        Self {
            joints,
            motors: motors.motors.clone(),
            homing: Some(homing.clone()),
            control: Some(control.clone()),
            realm: Some(realm),
            revoked: Cell::new(false),
            generation: Cell::new(1),
        }
    }

    pub(crate) fn realm(&self) -> Option<&Arc<()>> {
        self.realm.as_ref()
    }

    pub(crate) fn revoke(&self) {
        if !self.revoked.replace(true) {
            self.generation.set(self.generation.get().saturating_add(1));
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub(crate) fn contains(&self, joint: &str) -> bool {
        !self.revoked.get() && self.joints.contains(joint)
    }

    pub(crate) fn validate_binding(
        &self,
        motors: &MotorsConfigFile,
        homing: &HomingConfigFile,
        control: &ControlConfigFile,
    ) -> bool {
        if self.revoked.get() || self.realm.is_none() {
            return false;
        }
        let Some(bound_homing) = &self.homing else {
            return false;
        };
        let Some(bound_control) = &self.control else {
            return false;
        };
        let matches = self.motors.len() == motors.motors.len()
            && self.motors.iter().zip(&motors.motors).all(|(old, new)| {
                motor_reference_matches(old, new)
                    && control_reference_matches(old, bound_control, control)
                    && match (
                        bound_homing.homing.effective_joint(&old.joint),
                        homing.homing.effective_joint(&new.joint),
                    ) {
                        (Some(old), Some(new)) => homing_reference_matches(&old, &new),
                        _ => false,
                    }
            })
            && bound_homing.homing.zero_verify_tolerance_rad
                == homing.homing.zero_verify_tolerance_rad
            && bound_homing.homing.calibration_record_path == homing.homing.calibration_record_path;
        if !matches {
            self.revoke();
        }
        matches
    }
}

fn motor_reference_matches(old: &MotorEntry, new: &MotorEntry) -> bool {
    old.joint == new.joint
        && old.driver == new.driver
        && old.motor_type == new.motor_type
        && old.can_interface == new.can_interface
        && old.device_id == new.device_id
        && old.direction == new.direction
        && old.gear_ratio == new.gear_ratio
        && old.recv_can_id == new.recv_can_id
        && old.firmware_version == new.firmware_version
        && old.bench.position_lower_rad == new.bench.position_lower_rad
        && old.bench.position_upper_rad == new.bench.position_upper_rad
        && old.bench.velocity_limit_rad_s == new.bench.velocity_limit_rad_s
}

// Gains, friction, torque caps and watchdog timings do not change the reference.
// The installed envelope/frame and resolved velocity policy do. Output checks
// still validate all scalar policy and apply current torque caps independently.
fn control_reference_matches(
    motor: &MotorEntry,
    old: &ControlConfigFile,
    new: &ControlConfigFile,
) -> bool {
    let (Some(old_joint), Some(new_joint)) = (
        old.control.joints.get(&motor.joint),
        new.control.joints.get(&motor.joint),
    ) else {
        return false;
    };
    old_joint.motor_type == new_joint.motor_type
        && old_joint.position_hold_trim_rad == new_joint.position_hold_trim_rad
        && old_joint.position_soft_lower_rad == new_joint.position_soft_lower_rad
        && old_joint.position_soft_upper_rad == new_joint.position_soft_upper_rad
        && old_joint.position_limit_margin_min_rad == new_joint.position_limit_margin_min_rad
        && old_joint.position_limit_margin_k_v_s == new_joint.position_limit_margin_k_v_s
        && old_joint.position_limit_margin_k_stop == new_joint.position_limit_margin_k_stop
        && old_joint.position_limit_measured_fault_slack_rad
            == new_joint.position_limit_measured_fault_slack_rad
        && old_joint.position_trajectory_velocity_deadband_rad
            == new_joint.position_trajectory_velocity_deadband_rad
        && old_joint.position_trajectory_accel_rad_s2 == new_joint.position_trajectory_accel_rad_s2
        && old.control.wrong_sign_watchdog.expected_sign_at_positive_q
            == new.control.wrong_sign_watchdog.expected_sign_at_positive_q
        && resolve_joint_velocity_cap(&motor.joint, motor.motor_type, &old.control).ok()
            == resolve_joint_velocity_cap(&motor.joint, motor.motor_type, &new.control).ok()
}

fn homing_reference_matches(old: &EffectiveHomingJoint, new: &EffectiveHomingJoint) -> bool {
    let sensors = |effective: &EffectiveHomingJoint| {
        effective.sensors.as_ref().map(|sensors| {
            [
                (sensors.home.gpio, sensors.home.active_high),
                (sensors.min_limit.gpio, sensors.min_limit.active_high),
                (sensors.max_limit.gpio, sensors.max_limit.active_high),
            ]
        })
    };
    old.joint == new.joint
        && old.method == new.method
        && old.home_offset_rad == new.home_offset_rad
        && old.search_direction == new.search_direction
        && old.search_velocity_rad_s == new.search_velocity_rad_s
        && old.search_torque_nm == new.search_torque_nm
        && old.search_timeout_s == new.search_timeout_s
        && old.backoff_rad == new.backoff_rad
        && old.sign_test_required == new.sign_test_required
        && old.allow_sensor_overlap == new.allow_sensor_overlap
        && sensors(old) == sensors(new)
}
