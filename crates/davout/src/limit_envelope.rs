//! Live Set Limits: expand-only URDF hard + motors/control soft, rebuild policy.

use armee_kinematics::expand_urdf_joint_hard;
use marengo_config::{
    apply_limit_patch_to_control, apply_limit_patch_to_motor, ensure_soft_inset,
    validate_control_against_limits, validate_limit_patch, validate_safety_config,
    ControlConfigFile, LimitPatch, MotorsConfigFile,
};

use crate::{build_limits, DavoutError, MotorBus, OperationalMode, Supervisor};

impl<B: MotorBus> Supervisor<B> {
    /// Apply a validated per-joint limit patch and atomically rebuild runtime policy.
    ///
    /// Expands in-memory URDF hard limits expand-only when the patch exceeds the current
    /// URDF envelope (bench Set Limits). Soft defaults to hard ± ADR 0009 inset when omitted.
    pub fn apply_limit_patch(&mut self, patch: &LimitPatch) -> Result<(), DavoutError> {
        self.refuse_reference_interference("apply_limit_patch")?;
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::LimitPatchActive);
        }
        self.reference_binding_valid();
        validate_limit_patch(patch)?;
        let mut patch = patch.clone();
        ensure_soft_inset(&mut patch);

        let mut urdf_robot = self.urdf_robot.clone();
        expand_urdf_joint_hard(
            &mut urdf_robot,
            &patch.joint,
            patch.position_lower_rad,
            patch.position_upper_rad,
        )
        .map_err(|error| DavoutError::Limit {
            joint: patch.joint.clone(),
            message: error.to_string(),
        })?;

        let mut motors = self.motors.clone();
        let mut control = self.control.clone();
        let motor = motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == patch.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: patch.joint.clone(),
            })?;
        apply_limit_patch_to_motor(motor, &patch)?;
        let control_entry = control
            .control
            .joints
            .get_mut(&patch.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: patch.joint.clone(),
            })?;
        apply_limit_patch_to_control(control_entry, &patch)?;

        validate_control_against_limits(&self.robot, &motors, &control)?;
        let limits = build_limits(&self.robot, &motors, &control, &urdf_robot)?;
        let policy = limits
            .get(&patch.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: patch.joint.clone(),
            })?;
        if let Some(sample) = self.last_feedback_samples.get(&patch.joint) {
            if sample.position_rad < policy.hard_lower()
                || sample.position_rad > policy.hard_upper()
            {
                return Err(DavoutError::Limit {
                    joint: patch.joint.clone(),
                    message: format!(
                        "measured position {} outside proposed hard [{}, {}]",
                        sample.position_rad,
                        policy.hard_lower(),
                        policy.hard_upper()
                    ),
                });
            }
        }

        let installed_model = self.installed_model.replacement(&self.robot, &urdf_robot)?;
        // Installed model/limit generation changes cannot preserve reference.
        self.reference_authority.revoke();
        self.installed_model = installed_model;
        self.urdf_robot = urdf_robot;
        self.motors = motors;
        self.control = control;
        self.limits = limits;
        Ok(())
    }

    /// In-memory URDF model (bench Set Limits may expand hard envelopes).
    pub fn urdf_robot(&self) -> &urdf_rs::Robot {
        &self.urdf_robot
    }

    /// Atomically restore validated motors/control/URDF and derived limits.
    /// Refuses Active before any mutation; rejected candidates preserve installed
    /// policy/model/reference. Successful replacement revokes current reference.
    pub fn restore_limit_snapshot(
        &mut self,
        motors: MotorsConfigFile,
        control: ControlConfigFile,
        urdf_robot: urdf_rs::Robot,
    ) -> Result<(), DavoutError> {
        self.refuse_reference_interference("model restore")?;
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::LimitPatchActive);
        }
        self.reference_binding_valid();
        validate_safety_config(&self.robot, &motors, &control, &self.homing_config)?;
        let limits = build_limits(&self.robot, &motors, &control, &urdf_robot)?;
        let installed_model = self.installed_model.replacement(&self.robot, &urdf_robot)?;
        self.reference_authority.revoke();
        self.installed_model = installed_model;
        self.motors = motors;
        self.control = control;
        self.urdf_robot = urdf_robot;
        self.limits = limits;
        Ok(())
    }
}
