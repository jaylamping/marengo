//! Private admission authority. History and scalar verification are not permits.

use std::cell::Cell;
use std::collections::HashSet;
use std::sync::Arc;

use rustc_hash::FxHashSet;

use robstride::MotorAddress;

use crate::reference_model::{InstalledModelStamp, InstalledReferenceModel};

/// Lifetime binding captured only from the actual consumed owner evidence.
/// Neither a cache entry, deadline nor ordinary stop generation is a permit.
pub(super) struct ConsumedReferenceBinding {
    pub(super) job: Arc<()>,
    pub(super) model: InstalledModelStamp,
    pub(super) joint: String,
    pub(super) address: MotorAddress,
    pub(super) device_epoch: u64,
    /// Physical bindings retain the MCU identifier read during acquisition.
    pub(super) uid: Option<robstride::DeviceUid>,
    revoked: Cell<bool>,
}

impl ConsumedReferenceBinding {
    pub(super) fn new(
        job: Arc<()>,
        model: InstalledModelStamp,
        joint: String,
        address: MotorAddress,
        device_epoch: u64,
        uid: Option<robstride::DeviceUid>,
    ) -> Self {
        Self {
            job,
            model,
            joint,
            address,
            device_epoch,
            uid,
            revoked: Cell::new(false),
        }
    }

    pub(super) fn is_revoked(&self) -> bool {
        self.revoked.get()
    }
}

/// How a consumed selection combines with existing current permission.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionScope {
    /// R2b2 virtual rule: the selected joint replaces all prior coverage.
    Replace,
    /// Physical rule (ADR 0036): per-joint grants accumulate under one policy.
    Accumulate,
}

use marengo_config::{
    resolve_joint_velocity_cap, ControlConfigFile, EffectiveHomingJoint, HomingConfigFile,
    JointControlEntry, MotorEntry, MotorsConfigFile,
};

/// Bound-side policy for one installed motor, resolved once from the immutable
/// snapshot so each validation only resolves the live side.
struct BoundMotor {
    control: Option<JointControlEntry>,
    velocity_cap: Option<f64>,
    homing: Option<EffectiveHomingJoint>,
}

fn bind_motors(
    motors: &[MotorEntry],
    homing: &HomingConfigFile,
    control: &ControlConfigFile,
) -> Vec<BoundMotor> {
    motors
        .iter()
        .map(|motor| BoundMotor {
            control: control.control.joints.get(&motor.joint).cloned(),
            velocity_cap: resolve_joint_velocity_cap(
                &motor.joint,
                motor.motor_type,
                &control.control,
            )
            .ok(),
            homing: homing.homing.effective_joint(&motor.joint),
        })
        .collect()
}

/// Nonexported, nonserializable and noncloneable owner-local permission.
pub(crate) struct ReferenceAuthority {
    joints: FxHashSet<String>,
    motors: Vec<MotorEntry>,
    bound: Vec<BoundMotor>,
    homing: Option<HomingConfigFile>,
    control: Option<ControlConfigFile>,
    realm: Option<Arc<()>>,
    consumed: Vec<ConsumedReferenceBinding>,
    accumulates: bool,
    revoked: Cell<bool>,
    generation: Cell<u64>,
}

impl Default for ReferenceAuthority {
    fn default() -> Self {
        Self {
            joints: FxHashSet::default(),
            motors: Vec::new(),
            bound: Vec::new(),
            homing: None,
            control: None,
            realm: None,
            consumed: Vec::new(),
            accumulates: false,
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
            joints: joints.into_iter().collect(),
            motors: motors.motors.clone(),
            bound: bind_motors(&motors.motors, homing, control),
            homing: Some(homing.clone()),
            control: Some(control.clone()),
            realm: Some(realm),
            consumed: Vec::new(),
            accumulates: false,
            revoked: Cell::new(false),
            generation: Cell::new(1),
        }
    }

    pub(crate) fn realm(&self) -> Option<&Arc<()>> {
        self.realm.as_ref()
    }

    pub(crate) fn revoke(&self) {
        if !self.revoked.replace(true) {
            self.bump_generation();
        }
    }

    fn bump_generation(&self) {
        self.generation.set(self.generation.get().saturating_add(1));
    }

    /// Revoke one physical joint grant; peers bound to other devices remain.
    pub(super) fn revoke_binding(&self, binding: &ConsumedReferenceBinding) {
        if !binding.revoked.replace(true) {
            self.bump_generation();
        }
    }

    pub(super) fn revoke_joint(&self, joint: &str) {
        if let Some(binding) = self.consumed.iter().find(|binding| binding.joint == joint) {
            self.revoke_binding(binding);
        }
    }

    /// Cancelling the owning commit revokes exactly its selected permission.
    pub(super) fn revoke_selected(&self, job: &Arc<()>) {
        let selected = self
            .consumed
            .iter()
            .find(|binding| Arc::ptr_eq(&binding.job, job));
        match selected {
            Some(binding) if self.accumulates => self.revoke_binding(binding),
            Some(_) => self.revoke(),
            None => {}
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// Every new acquisition invalidates prior coverage, including after an
    /// earlier revocation. Refuse exhaustion before reserving or transmitting.
    pub(crate) fn invalidate_for_acquisition(&self) -> Option<u64> {
        let next = self.generation.get().checked_add(1)?;
        self.revoked.set(true);
        self.generation.set(next);
        Some(next)
    }

    /// Physical acquisition replaces only the target's coordinate; other joints'
    /// grants keep their own device bindings and continuity (ADR 0036).
    pub(crate) fn invalidate_joint_for_acquisition(&self, joint: &str) -> Option<u64> {
        let next = self.generation.get().checked_add(1)?;
        if let Some(binding) = self.consumed.iter().find(|binding| binding.joint == joint) {
            binding.revoked.set(true);
        }
        if !self.accumulates {
            self.revoked.set(true);
        }
        self.generation.set(next);
        Some(next)
    }

    pub(crate) fn contains(&self, joint: &str) -> bool {
        !self.revoked.get()
            && self.joints.contains(joint)
            && (self.consumed.is_empty()
                || self
                    .consumed
                    .iter()
                    .any(|binding| binding.joint == joint && !binding.revoked.get()))
    }

    pub(super) fn select_consumed(
        &mut self,
        realm: Arc<()>,
        binding: ConsumedReferenceBinding,
        policy: &crate::reference_journal_event::TypedPolicy,
        scope: SelectionScope,
    ) -> Option<()> {
        let next = self.generation.get().checked_add(1)?;
        let joint = binding.joint.clone();
        let merge = scope == SelectionScope::Accumulate
            && self.accumulates
            && !self.revoked.get()
            && self
                .realm
                .as_ref()
                .is_some_and(|own| Arc::ptr_eq(own, &realm))
            && self.policy_matches(&policy.motors, &policy.homing, &policy.control);
        if merge {
            self.consumed.retain(|existing| existing.joint != joint);
            self.consumed.push(binding);
            self.joints.insert(joint);
            self.generation.set(next);
            return Some(());
        }
        *self = Self {
            joints: FxHashSet::from_iter([joint]),
            motors: policy.motors.motors.clone(),
            bound: bind_motors(&policy.motors.motors, &policy.homing, &policy.control),
            homing: Some(policy.homing.clone()),
            control: Some(policy.control.clone()),
            realm: Some(realm),
            consumed: vec![binding],
            accumulates: scope == SelectionScope::Accumulate,
            revoked: Cell::new(false),
            generation: Cell::new(next),
        };
        Some(())
    }

    pub(super) fn selected_by(&self, job: &Arc<()>) -> bool {
        !self.revoked.get()
            && self
                .consumed
                .iter()
                .any(|binding| !binding.revoked.get() && Arc::ptr_eq(&binding.job, job))
    }

    pub(super) fn consumed_bindings(&self) -> &[ConsumedReferenceBinding] {
        &self.consumed
    }

    pub(super) fn binding_for(&self, joint: &str) -> Option<&ConsumedReferenceBinding> {
        self.consumed
            .iter()
            .find(|binding| binding.joint == joint && !binding.revoked.get())
    }

    pub(super) fn validate_consumed_model(&self, model: &InstalledReferenceModel) -> bool {
        self.consumed
            .iter()
            .all(|binding| model.matches(&binding.model))
    }

    #[cfg(test)]
    pub(super) fn set_generation_for_test(&self, generation: u64) {
        self.generation.set(generation);
    }

    /// Compare without side effects; `validate_binding` revokes on mismatch.
    fn policy_matches(
        &self,
        motors: &MotorsConfigFile,
        homing: &HomingConfigFile,
        control: &ControlConfigFile,
    ) -> bool {
        let (Some(bound_homing), Some(bound_control)) = (&self.homing, &self.control) else {
            return false;
        };
        self.motors.len() == motors.motors.len()
            && self.bound.len() == self.motors.len()
            && self
                .motors
                .iter()
                .zip(&self.bound)
                .zip(&motors.motors)
                .all(|((old, bound), new)| {
                    motor_reference_matches(old, new)
                        && control_reference_matches(old, bound, bound_control, control)
                        && match (&bound.homing, homing.homing.effective_joint_ref(&new.joint)) {
                            (Some(old), Some(new)) => homing_reference_matches(old, &new),
                            _ => false,
                        }
                })
            && bound_homing.homing.zero_verify_tolerance_rad
                == homing.homing.zero_verify_tolerance_rad
            && bound_homing.homing.calibration_record_path == homing.homing.calibration_record_path
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
        let matches = self.policy_matches(motors, homing, control);
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
    bound: &BoundMotor,
    old: &ControlConfigFile,
    new: &ControlConfigFile,
) -> bool {
    let (Some(old_joint), Some(new_joint)) = (&bound.control, new.control.joints.get(&motor.joint))
    else {
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
        && bound.velocity_cap
            == resolve_joint_velocity_cap(&motor.joint, motor.motor_type, &new.control).ok()
}

fn homing_reference_matches(old: &EffectiveHomingJoint, new: &EffectiveHomingJoint<&str>) -> bool {
    fn sensors<J>(effective: &EffectiveHomingJoint<J>) -> Option<[(u8, bool); 3]> {
        effective.sensors.as_ref().map(|sensors| {
            [
                (sensors.home.gpio, sensors.home.active_high),
                (sensors.min_limit.gpio, sensors.min_limit.active_high),
                (sensors.max_limit.gpio, sensors.max_limit.active_high),
            ]
        })
    }
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
