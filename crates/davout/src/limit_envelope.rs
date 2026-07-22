//! Live Set Limits: expand-only URDF hard + motors/control soft, rebuild policy.

use armee_kinematics::expand_urdf_joint_hard;
use marengo_config::{
    apply_limit_patch_to_control, apply_limit_patch_to_motor, ensure_soft_inset,
    validate_control_against_limits, validate_limit_patch, ControlConfigFile, LimitPatch,
    MotorsConfigFile,
};

use crate::{build_limits, DavoutError, MotorBus, OperationalMode, Supervisor};

impl<B: MotorBus> Supervisor<B> {
    /// Apply a validated per-joint limit patch and atomically rebuild runtime policy.
    ///
    /// Expands in-memory URDF hard limits expand-only when the patch exceeds the current
    /// URDF envelope (bench Set Limits). Soft defaults to hard ± ADR 0009 inset when omitted.
    pub fn apply_limit_patch(&mut self, patch: &LimitPatch) -> Result<(), DavoutError> {
        if self.mode == OperationalMode::Active {
            return Err(DavoutError::LimitPatchActive);
        }
        validate_limit_patch(patch)?;
        let mut patch = patch.clone();
        ensure_soft_inset(&mut patch);

        let urdf_before = self.urdf_robot.clone();
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
        let limits = match build_limits(&self.robot, &motors, &control, &urdf_robot) {
            Ok(limits) => limits,
            Err(error) => {
                self.urdf_robot = urdf_before;
                return Err(error);
            }
        };
        let policy = limits
            .get(&patch.joint)
            .ok_or_else(|| DavoutError::UnknownJoint {
                joint: patch.joint.clone(),
            })?;
        if let Some(sample) = self.last_feedback_samples.get(&patch.joint) {
            if sample.position_rad < policy.hard_lower()
                || sample.position_rad > policy.hard_upper()
            {
                self.urdf_robot = urdf_before;
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

    /// Restore motors/control/URDF and rebuild limits (persist enqueue rollback).
    pub fn restore_limit_snapshot(
        &mut self,
        motors: MotorsConfigFile,
        control: ControlConfigFile,
        urdf_robot: urdf_rs::Robot,
    ) -> Result<(), DavoutError> {
        self.motors = motors;
        self.control = control;
        self.urdf_robot = urdf_robot;
        self.rebuild_limits()
    }
}
