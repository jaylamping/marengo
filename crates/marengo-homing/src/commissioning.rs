//! Commissioning facets: proto mapping, OutOfLimits classification, Ready aggregation.

use armee_proto::JointHomingState as ProtoJointHomingState;

use crate::verify::VerifyError;
use crate::JointHomingState;

/// Map runtime homing registry state onto the Chappe proto enum.
///
/// Always yields a non-[`ProtoJointHomingState::Unspecified`] value so Consul can
/// distinguish live wire from old publishers that omit `homing_state`.
pub fn to_proto_homing_state(state: JointHomingState) -> ProtoJointHomingState {
    match state {
        JointHomingState::Unhomed => ProtoJointHomingState::Unhomed,
        JointHomingState::Homing => ProtoJointHomingState::Homing,
        JointHomingState::Verified => ProtoJointHomingState::Verified,
        JointHomingState::Faulted => ProtoJointHomingState::Faulted,
    }
}

/// Decode a wire `homing_state` ordinal. `Unspecified` / unknown stay unspecified.
pub fn from_proto_homing_state(raw: i32) -> Option<JointHomingState> {
    match ProtoJointHomingState::try_from(raw) {
        Ok(ProtoJointHomingState::Unspecified) => None,
        Ok(ProtoJointHomingState::Unhomed) => Some(JointHomingState::Unhomed),
        Ok(ProtoJointHomingState::Homing) => Some(JointHomingState::Homing),
        Ok(ProtoJointHomingState::Verified) => Some(JointHomingState::Verified),
        Ok(ProtoJointHomingState::Faulted) => Some(JointHomingState::Faulted),
        Err(_) => None,
    }
}

/// True when verification failed specifically for hard-limit breach.
pub fn verify_error_is_out_of_limits(err: &VerifyError) -> bool {
    matches!(err, VerifyError::OutOfLimits { .. })
}

/// Per-joint inputs for Joint → Limb → Robot Ready aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JointFacetInput {
    pub name: String,
    pub homing_state: JointHomingState,
    pub online: bool,
    pub motor_mapped: bool,
    pub fault: bool,
    pub out_of_limits: bool,
    pub drive_active: bool,
}

impl JointFacetInput {
    /// Built = online feedback or motors.yaml mapping. Unbuilt Offline does not block Ready.
    pub fn is_built(&self) -> bool {
        self.online || self.motor_mapped
    }

    /// Joint Ready = reference Verified only (Fault/OutOfLimits are separate health facets).
    pub fn is_ready(&self) -> bool {
        self.homing_state == JointHomingState::Verified
    }

    /// Healthy enough to count toward Robot/Limb Ready aggregates.
    pub fn is_ready_healthy(&self) -> bool {
        self.is_ready() && !self.fault && !self.out_of_limits
    }
}

/// Limb Ready: every built member is Ready+healthy; unbuilt Offline members do not block.
pub fn limb_ready(members: &[JointFacetInput]) -> bool {
    let built: Vec<&JointFacetInput> = members.iter().filter(|j| j.is_built()).collect();
    if built.is_empty() {
        return false;
    }
    built.iter().all(|j| j.is_ready_healthy())
}

/// Robot Ready over master actuated joints: every built joint Ready+healthy.
/// Unbuilt Offline inventory does not block. Scope must not be used here.
pub fn robot_ready(master_joints: &[JointFacetInput]) -> bool {
    let built: Vec<&JointFacetInput> = master_joints.iter().filter(|j| j.is_built()).collect();
    if built.is_empty() {
        return false;
    }
    built.iter().all(|j| j.is_ready_healthy())
}

/// Wire-gate helper: old publishers leave `homing_state` at UNSPECIFIED (0).
pub fn wire_homing_is_unspecified(raw: i32) -> bool {
    matches!(
        ProtoJointHomingState::try_from(raw),
        Ok(ProtoJointHomingState::Unspecified) | Err(_)
    )
}

/// Joint is eligible for Enable: Verified, Online, not Fault, not OutOfLimits.
pub fn is_enable_eligible(joint: &JointFacetInput) -> bool {
    joint.online && joint.is_ready_healthy()
}

/// Resolve Enable targets.
///
/// - `effective_scope = Some(...)`: enable Verified in-scope joints (skip others); does **not**
///   require full-master Robot Ready.
/// - `effective_scope = None` (no persisted scope file): require [`robot_ready`] on
///   `master_joints`, then target eligible loaded joints from `loaded_joints`.
///
/// Returns an error string when the resulting target set is empty or Robot Ready fails.
pub fn select_enable_targets(
    master_joints: &[JointFacetInput],
    loaded_joints: &[JointFacetInput],
    effective_scope: Option<&[String]>,
) -> Result<Vec<String>, String> {
    match effective_scope {
        Some(scope) => {
            let scope_set: std::collections::HashSet<&str> =
                scope.iter().map(String::as_str).collect();
            let mut targets: Vec<String> = loaded_joints
                .iter()
                .filter(|j| scope_set.contains(j.name.as_str()) && is_enable_eligible(j))
                .map(|j| j.name.clone())
                .collect();
            targets.sort();
            if targets.is_empty() {
                return Err("no Verified in-scope joints eligible for enable".into());
            }
            Ok(targets)
        }
        None => {
            if !robot_ready(master_joints) {
                return Err(
                    "Enable requires full-master Robot Ready when no commissioning scope is set"
                        .into(),
                );
            }
            let mut targets: Vec<String> = loaded_joints
                .iter()
                .filter(|j| is_enable_eligible(j))
                .map(|j| j.name.clone())
                .collect();
            targets.sort();
            if targets.is_empty() {
                return Err("no loaded joints eligible for enable".into());
            }
            Ok(targets)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use armee_proto::prost::Message;
    use armee_proto::{JointState, RobotState};

    fn facet(
        name: &str,
        state: JointHomingState,
        online: bool,
        motor_mapped: bool,
        fault: bool,
        out_of_limits: bool,
    ) -> JointFacetInput {
        JointFacetInput {
            name: name.to_string(),
            homing_state: state,
            online,
            motor_mapped,
            fault,
            out_of_limits,
            drive_active: false,
        }
    }

    #[test]
    fn proto_mapping_covers_all_runtime_states() {
        assert_eq!(
            to_proto_homing_state(JointHomingState::Unhomed) as i32,
            ProtoJointHomingState::Unhomed as i32
        );
        assert_eq!(
            to_proto_homing_state(JointHomingState::Homing) as i32,
            ProtoJointHomingState::Homing as i32
        );
        assert_eq!(
            to_proto_homing_state(JointHomingState::Verified) as i32,
            ProtoJointHomingState::Verified as i32
        );
        assert_eq!(
            to_proto_homing_state(JointHomingState::Faulted) as i32,
            ProtoJointHomingState::Faulted as i32
        );
        assert_ne!(
            to_proto_homing_state(JointHomingState::Unhomed) as i32,
            ProtoJointHomingState::Unspecified as i32
        );
    }

    #[test]
    fn unspecified_wire_gates_old_publishers() {
        assert!(wire_homing_is_unspecified(0));
        assert!(wire_homing_is_unspecified(99));
        assert!(!wire_homing_is_unspecified(
            ProtoJointHomingState::Verified as i32
        ));
        assert_eq!(from_proto_homing_state(0), None);
        assert_eq!(
            from_proto_homing_state(ProtoJointHomingState::Verified as i32),
            Some(JointHomingState::Verified)
        );
    }

    #[test]
    fn joint_state_commissioning_fields_round_trip() {
        let msg = RobotState {
            timestamp_ms: 7,
            joints: vec![JointState {
                name: "right_elbow_pitch".to_string(),
                position: 0.25,
                velocity: 0.0,
                effort: 0.1,
                temperature_c: 30.0,
                fault: 0,
                homing_state: ProtoJointHomingState::Verified as i32,
                drive_active: true,
                out_of_limits: false,
            }],
        };
        let bytes = msg.encode_to_vec();
        let decoded = RobotState::decode(bytes.as_slice()).expect("decode");
        assert_eq!(decoded.joints.len(), 1);
        let j = &decoded.joints[0];
        assert_eq!(j.name, "right_elbow_pitch");
        assert_eq!(j.homing_state, ProtoJointHomingState::Verified as i32);
        assert!(j.drive_active);
        assert!(!j.out_of_limits);
    }

    #[test]
    fn omitted_homing_state_decodes_as_unspecified() {
        // Proto3 default: absent enum → 0 (UNSPECIFIED). Old runtimes omit field 7.
        let legacy = JointState {
            name: "right_shoulder_roll".to_string(),
            position: 0.0,
            velocity: 0.0,
            effort: 0.0,
            temperature_c: 0.0,
            fault: 0,
            homing_state: 0,
            drive_active: false,
            out_of_limits: false,
        };
        let bytes = legacy.encode_to_vec();
        let decoded = JointState::decode(bytes.as_slice()).expect("decode");
        assert!(wire_homing_is_unspecified(decoded.homing_state));
        assert!(!decoded.drive_active);
        assert!(!decoded.out_of_limits);
    }

    #[test]
    fn verify_out_of_limits_classifies_only_limit_breach() {
        let ool = VerifyError::OutOfLimits {
            joint: "j".into(),
            position_rad: 9.0,
            lower: -1.0,
            upper: 1.0,
        };
        let tol = VerifyError::ZeroTolerance {
            joint: "j".into(),
            position_rad: 0.2,
            tolerance_rad: 0.05,
        };
        assert!(verify_error_is_out_of_limits(&ool));
        assert!(!verify_error_is_out_of_limits(&tol));
    }

    #[test]
    fn unbuilt_offline_does_not_block_robot_ready() {
        let master = vec![
            facet(
                "right_shoulder_roll",
                JointHomingState::Verified,
                true,
                true,
                false,
                false,
            ),
            facet(
                "right_shoulder_pitch",
                JointHomingState::Verified,
                true,
                true,
                false,
                false,
            ),
            // Unbuilt Offline inventory — must not block.
            facet(
                "left_shoulder_roll",
                JointHomingState::Unhomed,
                false,
                false,
                false,
                false,
            ),
        ];
        assert!(robot_ready(&master));
    }

    #[test]
    fn scope_does_not_fabricate_robot_ready() {
        // Built Unhomed master joint keeps Robot Ready false even if scoped joints are Verified.
        let master = vec![
            facet(
                "right_shoulder_roll",
                JointHomingState::Verified,
                true,
                true,
                false,
                false,
            ),
            facet(
                "right_shoulder_pitch",
                JointHomingState::Unhomed,
                true,
                true,
                false,
                false,
            ),
        ];
        assert!(!robot_ready(&master));
    }

    #[test]
    fn limb_ready_ignores_unbuilt_and_requires_built_verified() {
        let limb = vec![
            facet(
                "right_shoulder_roll",
                JointHomingState::Verified,
                true,
                true,
                false,
                false,
            ),
            facet(
                "right_lower_arm_yaw",
                JointHomingState::Unhomed,
                false,
                false,
                false,
                false,
            ),
        ];
        assert!(limb_ready(&limb));

        let faulted = vec![facet(
            "right_shoulder_roll",
            JointHomingState::Faulted,
            true,
            true,
            true,
            false,
        )];
        assert!(!limb_ready(&faulted));
    }

    #[test]
    fn out_of_limits_blocks_robot_ready_even_when_verified() {
        let master = vec![facet(
            "right_elbow_pitch",
            JointHomingState::Verified,
            true,
            true,
            false,
            true,
        )];
        assert!(!robot_ready(&master));
    }

    #[test]
    fn scoped_enable_skips_unhomed_and_out_of_limits() {
        let loaded = vec![
            facet("a", JointHomingState::Verified, true, true, false, false),
            facet("b", JointHomingState::Unhomed, true, true, false, false),
            facet("c", JointHomingState::Verified, true, true, false, true),
        ];
        let scope = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let targets = select_enable_targets(&loaded, &loaded, Some(&scope)).expect("targets");
        assert_eq!(targets, vec!["a".to_string()]);
    }

    #[test]
    fn scoped_enable_does_not_require_robot_ready() {
        let master = vec![
            facet("a", JointHomingState::Verified, true, true, false, false),
            facet("b", JointHomingState::Unhomed, true, true, false, false),
        ];
        let scope = vec!["a".to_string()];
        let targets = select_enable_targets(&master, &master, Some(&scope)).expect("scoped");
        assert_eq!(targets, vec!["a".to_string()]);
        assert!(!robot_ready(&master));
    }

    #[test]
    fn no_scope_enable_requires_robot_ready() {
        let master = vec![
            facet("a", JointHomingState::Verified, true, true, false, false),
            facet("b", JointHomingState::Unhomed, true, true, false, false),
        ];
        let err = select_enable_targets(&master, &master, None).expect_err("blocked");
        assert!(err.contains("Robot Ready"));
    }

    #[test]
    fn scoped_enable_rejects_empty_eligible_set() {
        let loaded = vec![facet(
            "a",
            JointHomingState::Unhomed,
            true,
            true,
            false,
            false,
        )];
        let scope = vec!["a".to_string()];
        let err = select_enable_targets(&loaded, &loaded, Some(&scope)).expect_err("empty");
        assert!(err.contains("no Verified"));
    }
}
