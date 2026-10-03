//! Candidate continuity regression through actual R2a inspection/admission.
//! INITIAL is a declared old virtual grant, never acquisition or physical proof.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{
    DavoutError, JointHomingState, OperationalMode, ReferenceError, ReferencePhase,
    ReferenceRequest, Supervisor,
};
use marengo_config::{load_robot_config_from, validate_safety_config};

#[path = "../../berthier/tests/support/mod.rs"]
mod support;

const TARGET: &str = "right_elbow_pitch";
const HISTORY: &[u8] = b"joints:\n  - joint: right_elbow_pitch\n    device_id: 4\n    can_interface: can0\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-01T00:00:00Z'\n    config_revision: historical-fixture\n    operator: historical-operator\n";

#[derive(Debug, Clone, Copy)]
enum ObservationPath {
    Snapshot,
    Begin,
}

struct Case {
    path: ObservationPath,
    initial: Vec<JointHomingState>,
    edited_policy_valid: bool,
    refusal: bool,
    no_reservation: bool,
    at_refusal_writes: usize,
    restored_states: Vec<JointHomingState>,
    restored_mapping: bool,
    ready: Result<(), DavoutError>,
    enable: Result<(), DavoutError>,
    after_mode: OperationalMode,
    after_active: Vec<String>,
    before_cleanup_writes: usize,
    history_before_cleanup: Vec<u8>,
    history_after_cleanup: Vec<u8>,
    cleanup: Result<(), DavoutError>,
    master_unchanged: bool,
    resources_removed: bool,
}

fn exercise_observation(path: ObservationPath) -> Case {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master: Vec<_> = [
        "config/robot.yaml",
        "config/motors.yaml",
        "config/control.yaml",
        "config/homing.yaml",
        "assets/urdf/marengo.urdf",
    ]
    .into_iter()
    .map(|relative| {
        let path = root.join(relative);
        let bytes = std::fs::read(&path).expect("immutable master input");
        (path, bytes)
    })
    .collect();
    let fixture = support::FixtureTree::new("reference-observed-policy", &root);
    let fixture_path = fixture.path().to_path_buf();
    let config = fixture.path().join("config");
    let control_path = config.join("control.yaml");
    let control = std::fs::read_to_string(&control_path).expect("copied control");
    assert_eq!(
        control
            .matches("active_reporting_diagnostics: true")
            .count(),
        1
    );
    std::fs::write(
        control_path,
        control.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("only copied diagnostics disabled before construction");
    let history = fixture.path().join("history.yaml");
    std::fs::write(&history, HISTORY).expect("literal inspection history");
    let robot = load_robot_config_from(&config).expect("actual copied robot policy");
    let mut owner = Supervisor::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
    )
    .expect("real owner with explicit old INITIAL virtual coverage");
    let names: Vec<_> = owner
        .motors
        .motors
        .iter()
        .map(|motor| motor.joint.clone())
        .collect();
    let initial = names
        .iter()
        .map(|joint| owner.joint_homing_state(joint))
        .collect();
    let old_request = ReferenceRequest {
        stamp: owner
            .reference_snapshot()
            .next_stamp
            .expect("fresh stamp before edit"),
        joint: TARGET.into(),
        confirmed: true,
        sign_verified: true,
    };
    let original_mapping = format!("{:?}", owner.motors);
    owner.bus_mut().clear_trace();
    let selected = owner
        .motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == TARGET)
        .expect("installed target");
    assert_eq!(selected.device_id, 4);
    selected.device_id = 6;
    // This remains valid scalar/unique-address configuration. Its defect is
    // continuity with this owner's installed can0:4, not malformed input.
    let edited_policy_valid =
        validate_safety_config(&robot, &owner.motors, &owner.control, &owner.homing_config).is_ok();
    // Do not call any ordinary facet/binding getter while edited: that would
    // hide whether the new API itself observed and revoked the old grant.
    let (refusal, no_reservation) = match path {
        ObservationPath::Snapshot => {
            let snapshot = owner.reference_snapshot();
            (
                snapshot.next_stamp.is_none(),
                snapshot.phase == ReferencePhase::Idle
                    && snapshot.handle.is_none()
                    && snapshot.terminal.is_none(),
            )
        }
        ObservationPath::Begin => {
            let result = owner.begin_reference(old_request);
            (
                matches!(result, Err(ReferenceError::InvalidRequest { .. })),
                !owner.reference_busy(),
            )
        }
    };
    let at_refusal_writes = owner.bus().transmissions().len();
    owner
        .motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == TARGET)
        .expect("same public row")
        .device_id = 4;
    let restored_mapping = format!("{:?}", owner.motors) == original_mapping;
    let restored_states = names
        .iter()
        .map(|joint| owner.joint_homing_state(joint))
        .collect();
    let ready = owner.set_homing_complete();
    let enable = owner.enable_targets(&[TARGET.into()]);
    let after_mode = owner.mode();
    let after_active = owner.active_joints().iter().cloned().collect();
    let before_cleanup_writes = owner.bus().transmissions().len();
    let history_before_cleanup = std::fs::read(&history).expect("history before fallback cleanup");
    // Capture admission/state/TX before fallback. Actual stop after a defective
    // candidate Enable cannot manufacture a refusal or erase the captured TX.
    let cleanup = owner.disable_all();
    let history_after_cleanup = std::fs::read(&history).expect("history after actual cleanup");
    drop(owner);
    drop(fixture);
    let resources_removed = !fixture_path
        .try_exists()
        .expect("exclusive fixture removal");
    let master_unchanged = master
        .iter()
        .all(|(path, bytes)| std::fs::read(path).expect("master input after cleanup") == *bytes);
    Case {
        path,
        initial,
        edited_policy_valid,
        refusal,
        no_reservation,
        at_refusal_writes,
        restored_states,
        restored_mapping,
        ready,
        enable,
        after_mode,
        after_active,
        before_cleanup_writes,
        history_before_cleanup,
        history_after_cleanup,
        cleanup,
        master_unchanged,
        resources_removed,
    }
}

#[test]
fn observed_mapping_refusal_cannot_restore_initial_permission_after_public_reversion() {
    let cases = [ObservationPath::Snapshot, ObservationPath::Begin].map(exercise_observation);
    // Both real paths and cleanup run before the first outcome classification.
    for case in cases {
        assert!(case.resources_removed && case.master_unchanged);
        case.cleanup.expect("real all-address fallback accepted");
        assert_eq!(
            case.initial,
            vec![JointHomingState::Verified; 5],
            "INITIAL setup control"
        );
        assert!(
            case.edited_policy_valid,
            "unused6 remains a valid scalar/unique mapping"
        );
        assert!(
            case.refusal && case.no_reservation,
            "new API must refuse mismatched installed policy"
        );
        assert_eq!(case.at_refusal_writes, 0);
        assert!(case.restored_mapping);
        assert_eq!(case.history_before_cleanup, HISTORY);
        assert_eq!(case.history_after_cleanup, HISTORY);
        println!(
            "actual observed mismatch path={:?} states={:?} Ready={:?} Enable={:?} writes={}",
            case.path, case.restored_states, case.ready, case.enable, case.before_cleanup_writes
        );
        assert_eq!(case.restored_states, vec![JointHomingState::Unhomed; 5],
            "observed {:?} policy refusal must permanently revoke INITIAL permission before restoring public mapping", case.path);
        assert!(case.ready.is_err() && case.enable.is_err());
        assert_eq!(case.after_mode, OperationalMode::Disabled);
        assert!(case.after_active.is_empty());
        assert_eq!(
            case.before_cleanup_writes, 0,
            "refused restored admission must not Enable"
        );
    }
}
