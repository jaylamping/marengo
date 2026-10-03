//! Folded homing facets (WP-T/B14): the live remainder of `marengo-homing`.
//!
//! Retired with the crate: the scalar verifier (`verify.rs`), the YAML history
//! writer/reader (`record_verification`, `persist`, `load_calibration`,
//! `CalibrationRecord`), the legacy registry lifecycle (`joint_states`,
//! `all_verified`, `require_ready`, ...), the Hall sensor module (`sensor.rs`,
//! deferred per D-4), `limb_ready`, the proto-decode helpers
//! (`from_proto_homing_state`, `wire_homing_is_unspecified` — decode lives in
//! Consul), and the write-only `JointFacetInput.drive_active` facet (D-9).
//!
//! What remains is pure vocabulary + aggregation. Live Ready/Enable/output
//! permission is Davout's private current-reference authority; nothing here
//! grants it.

use std::collections::HashMap;

use armee_proto::JointHomingState as ProtoJointHomingState;

/// Per-joint homing lifecycle state (projection vocabulary only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointHomingState {
    Unhomed,
    Homing,
    Verified,
    Faulted,
}

/// Map runtime homing state onto the Chappe proto enum.
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

/// Process-local OutOfLimits latch set (L-marengo-homing-01).
///
/// Recovery story: this flag has no in-process clear path by design. Every
/// production set goes through Davout's feedback consumer, which returns a
/// `Limit` error that [`record_runtime_error`](super::Supervisor::disable_all)
/// latches as a permanent Feedback-class fault in the same call (faults never
/// clear in-process, ADR 0020), and reference acquisition requires
/// fault-clear, so no fresh grant can exist while this flag is set. Recovery
/// is a process restart, which constructs fresh flags.
#[derive(Debug, Default)]
pub struct OutOfLimitsFlags {
    inner: HashMap<String, bool>,
}

impl OutOfLimitsFlags {
    /// Latch the facet for a configured joint. Unknown joints are ignored.
    pub fn mark(&mut self, joint: &str, configured: bool) {
        if configured {
            self.inner.insert(joint.to_string(), true);
        }
    }

    /// Whether measured feedback for `joint` breached its hard limits (+ slack).
    pub fn is_set(&self, joint: &str) -> bool {
        self.inner.get(joint).copied().unwrap_or(false)
    }
}

/// Per-joint inputs for Joint → Robot Ready aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JointFacetInput {
    pub name: String,
    pub homing_state: JointHomingState,
    pub online: bool,
    pub motor_mapped: bool,
    pub fault: bool,
    pub out_of_limits: bool,
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

    /// Healthy enough to count toward Robot Ready aggregates.
    pub fn is_ready_healthy(&self) -> bool {
        self.is_ready() && !self.fault && !self.out_of_limits
    }
}

/// Robot Ready over master actuated joints: every built joint Ready+healthy.
/// Unbuilt Offline inventory does not block. Scope must not be used here.
///
/// Subset semantics (L-marengo-homing-05): the *loaded* motors define the Ready
/// universe. Davout applies `MARENGO_JOINT_SUBSET` to robot/motors/control at
/// construction, so subset-excluded joints arrive here as unbuilt
/// (`motor_mapped = false`, no feedback) and intentionally do not block Robot
/// Ready. They are inventory outside the loaded robot — not silently healthy
/// joints. Callers must build facets from the same subset-filtered motors that
/// [`select_enable_targets`] draws its targets from.
pub fn robot_ready(master_joints: &[JointFacetInput]) -> bool {
    let built: Vec<&JointFacetInput> = master_joints.iter().filter(|j| j.is_built()).collect();
    if built.is_empty() {
        return false;
    }
    built.iter().all(|j| j.is_ready_healthy())
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
        }
    }

    #[test]
    fn proto_mapping_covers_all_runtime_states() {
        assert_eq!(
            to_proto_homing_state(JointHomingState::Unhomed) as i32,
            ProtoJointHomingState::Unhomed as i32
        );
        assert_eq!(
            to_proto_homing_state(JointHomingState::Verified) as i32,
            ProtoJointHomingState::Verified as i32
        );
        assert_ne!(
            to_proto_homing_state(JointHomingState::Unhomed) as i32,
            ProtoJointHomingState::Unspecified as i32
        );
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
    fn subset_excluded_joints_neither_block_ready_nor_become_targets() {
        let master = vec![
            facet("a", JointHomingState::Verified, true, true, false, false),
            facet(
                "excluded",
                JointHomingState::Unhomed,
                false,
                false,
                false,
                false,
            ),
        ];
        assert!(robot_ready(&master));
        let loaded: Vec<JointFacetInput> =
            master.iter().filter(|j| j.motor_mapped).cloned().collect();
        let targets = select_enable_targets(&master, &loaded, None).expect("targets");
        assert_eq!(targets, vec!["a".to_string()]);
    }

    #[test]
    fn out_of_limits_latch_has_no_clear_and_ignores_unknown_joints() {
        let mut flags = OutOfLimitsFlags::default();
        assert!(!flags.is_set("a"));
        flags.mark("unknown", false);
        assert!(!flags.is_set("unknown"));
        flags.mark("a", true);
        assert!(flags.is_set("a"));
    }
}
