//! Existing-row replacement consistency on an actual pre-write I/O failure.
//!
//! Overlay as crates/marengo-homing/tests/history_replacement_atomicity.rs in
//! an independently archived checkout. Only public existing APIs are used.
//! This does not qualify crash-safe replacement, a journal or Davout permission.
//! The prior history is parked within the exclusive fixture, never deleted.
//! Every meaningful outcome assertion follows retry/reload and fixture cleanup.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use marengo_config::{HomingConfigFile, HomingMethod, MotorEntry, MotorsConfigFile};
use marengo_homing::{CalibrationRecord, HomingRegistry, JointHomingState, RegistryError};

#[path = "support/mod.rs"]
mod support;
use support::TestDirectory;

const PRIOR_POSITION: f64 = 0.01;
const REPLACEMENT_POSITION: f64 = 0.02;
const REVISION_METADATA: &str = "history-replacement-fixture";
const BLOCKER_BYTES: &[u8] = b"exclusive regular-file parent blocks replacement\n";
const INITIAL_HISTORY: &str = "joints:
  - joint: right_elbow_pitch
    device_id: 4
    can_interface: can0
    method: manual_reference
    home_offset_rad: 0.0
    verified_position_rad: 0.005
    sign_test_passed: true
    timestamp_utc: '2026-09-29T12:00:00Z'
    config_revision: seed-target-revision
    operator: literal-seed-target
  - joint: right_shoulder_pitch
    device_id: 1
    can_interface: can0
    method: manual_reference
    home_offset_rad: 0.0
    verified_position_rad: 0.01
    sign_test_passed: true
    timestamp_utc: '2026-09-29T13:00:00Z'
    config_revision: unrelated-original-revision
    operator: literal-unrelated-original
";

#[derive(Debug)]
struct PublicSnapshot {
    calibration: CalibrationRecord,
    target_state: JointHomingState,
    target_out_of_limits: bool,
    peer_state: JointHomingState,
    peer_out_of_limits: bool,
    all_verified: bool,
    ready: Result<(), RegistryError>,
}

fn snapshot(registry: &HomingRegistry, target: &MotorEntry, peer: &MotorEntry) -> PublicSnapshot {
    PublicSnapshot {
        calibration: registry.calibration().clone(),
        target_state: registry.joint_state(&target.joint),
        target_out_of_limits: registry.is_out_of_limits(&target.joint),
        peer_state: registry.joint_state(&peer.joint),
        peer_out_of_limits: registry.is_out_of_limits(&peer.joint),
        all_verified: registry.all_verified(),
        ready: registry.require_ready(),
    }
}

fn record_target(
    registry: &mut HomingRegistry,
    target: &MotorEntry,
    position: f64,
    operator: &str,
) -> Result<(), RegistryError> {
    registry.record_verification(
        target,
        "manual_reference",
        0.0,
        position,
        true,
        operator,
        Some(REVISION_METADATA.into()),
    )
}

fn rows(bytes: &[u8]) -> Vec<serde_yaml::Value> {
    let yaml: serde_yaml::Value = serde_yaml::from_slice(bytes).expect("actual saved YAML");
    yaml["joints"]
        .as_sequence()
        .expect("actual saved history rows")
        .clone()
}

fn assert_saved_history(bytes: &[u8], target_position: f64, target_operator: &str) {
    // Literal schema/field oracle, independent of upsert/load/serializer helpers.
    let actual = rows(bytes);
    assert_eq!(
        actual.len(),
        2,
        "target replacement must retain unrelated history"
    );
    let target = actual
        .iter()
        .find(|row| row["joint"].as_str() == Some("right_elbow_pitch"))
        .expect("saved target");
    assert_eq!(target["device_id"].as_u64(), Some(4));
    assert_eq!(target["can_interface"].as_str(), Some("can0"));
    assert_eq!(target["method"].as_str(), Some("manual_reference"));
    assert_eq!(target["home_offset_rad"].as_f64(), Some(0.0));
    assert_eq!(
        target["verified_position_rad"].as_f64(),
        Some(target_position)
    );
    assert_eq!(target["sign_test_passed"].as_bool(), Some(true));
    assert_eq!(target["config_revision"].as_str(), Some(REVISION_METADATA));
    assert_eq!(target["operator"].as_str(), Some(target_operator));
    assert!(
        chrono::DateTime::parse_from_rfc3339(
            target["timestamp_utc"]
                .as_str()
                .expect("real audit timestamp")
        )
        .is_ok(),
        "real replacement writer must emit a valid timestamp"
    );
    let peer = actual
        .iter()
        .find(|row| row["joint"].as_str() == Some("right_shoulder_pitch"))
        .expect("saved unrelated row");
    let literal_peer = rows(INITIAL_HISTORY.as_bytes())
        .into_iter()
        .find(|row| row["joint"].as_str() == Some("right_shoulder_pitch"))
        .expect("independent literal unrelated oracle");
    assert_eq!(
        peer, &literal_peer,
        "every unrelated saved field must survive"
    );
}

fn assert_owned_child(root: &Path, source: &Path, destination: &Path) {
    let source = source.canonicalize().expect("owned source directory");
    assert_eq!(
        source.parent(),
        Some(root),
        "rename source must stay in the owned root"
    );
    assert_eq!(
        destination.parent(),
        Some(root),
        "rename destination must stay in the owned root"
    );
    assert!(
        !destination
            .try_exists()
            .expect("unoccupied owned rename target"),
        "do not replace any existing rename target"
    );
}

#[test]
fn failed_history_replacement_preserves_prior_rows_state_and_flags_then_retries() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("compiled actual crate manifest");
    if let Some(expected) = std::env::var_os("HISTORY_ATOMICITY_EXPECTED_MANIFEST") {
        assert_eq!(
            manifest,
            PathBuf::from(expected)
                .canonicalize()
                .expect("independently bound archived manifest"),
            "replacement probe must compile against the selected actual archive"
        );
    }
    let root = manifest.join("../..");
    let motors: MotorsConfigFile = serde_yaml::from_slice(
        &fs::read(root.join("config/motors.yaml")).expect("archived configured motors"),
    )
    .expect("configured motor schema");
    let homing: HomingConfigFile = serde_yaml::from_slice(
        &fs::read(root.join("config/homing.yaml")).expect("archived configured homing"),
    )
    .expect("configured homing schema");
    let target = motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_elbow_pitch")
        .expect("configured target")
        .clone();
    let peer = motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_shoulder_pitch")
        .expect("configured unrelated peer")
        .clone();
    let tolerance = homing.homing.zero_verify_tolerance_rad;
    assert!(tolerance.is_finite() && tolerance > REPLACEMENT_POSITION);
    for motor in [&target, &peer] {
        let policy = homing
            .homing
            .effective_joint(&motor.joint)
            .expect("manual policy");
        assert_eq!(policy.method, HomingMethod::ManualReference);
        assert_eq!(policy.home_offset_rad, 0.0);
        assert!(motor.bench.position_lower_rad.is_finite());
        assert!(motor.bench.position_upper_rad.is_finite());
        assert!(motor.bench.position_lower_rad < PRIOR_POSITION);
        assert!(REPLACEMENT_POSITION < motor.bench.position_upper_rad);
        assert_eq!(motor.can_interface, "can0");
    }
    assert_eq!(target.device_id, 4);
    assert_eq!(peer.device_id, 1);

    let directory = TestDirectory::new("public-history-replacement-atomicity");
    let owned_root = directory
        .path()
        .canonicalize()
        .expect("exclusive resolved root");
    let live_parent = owned_root.join("live");
    let parked_parent = owned_root.join("parked-prior");
    let history_path = live_parent.join("history.yaml");
    let parked_history_path = parked_parent.join("history.yaml");
    fs::create_dir(&live_parent).expect("owned prior history directory");
    fs::write(&history_path, INITIAL_HISTORY).expect("independent literal prior history");
    let configured = vec![target.joint.clone(), peer.joint.clone()];
    let mut registry =
        HomingRegistry::with_record_path(&history_path, configured.clone(), tolerance)
            .expect("actual existing history");
    let initial = snapshot(&registry, &target, &peer);

    // Establish meaningful prior local state through a real successful write.
    // These local legacy states are not Davout current-reference permission.
    let prior_result = record_target(
        &mut registry,
        &target,
        PRIOR_POSITION,
        "prior-committed-target",
    );
    registry.mark_out_of_limits(&target.joint);
    registry.mark_out_of_limits(&peer.joint);
    let before_failure = snapshot(&registry, &target, &peer);
    let prior_bytes = fs::read(&history_path);

    // Park the whole exclusively owned directory, preserving every prior byte.
    // Verify both resolved targets before directory moves on any host.
    assert_owned_child(&owned_root, &live_parent, &parked_parent);
    fs::rename(&live_parent, &parked_parent).expect("park exclusively owned prior directory");
    let parked_bytes_before_failure = fs::read(&parked_history_path);
    fs::write(&live_parent, BLOCKER_BYTES).expect("exclusive regular-file parent blocker");
    let blocker_is_file = fs::metadata(&live_parent)
        .expect("actual parent blocker")
        .is_file();
    let failed_result = record_target(
        &mut registry,
        &target,
        REPLACEMENT_POSITION,
        "replacement-target",
    );
    let failed_snapshot = snapshot(&registry, &target, &peer);
    let parked_bytes_after_failure = fs::read(&parked_history_path);
    let blocker_bytes_after_failure = fs::read(&live_parent);
    let failed_destination_read = fs::read(&history_path);

    fs::remove_file(&live_parent).expect("remove only the owned regular-file blocker");
    assert_owned_child(&owned_root, &parked_parent, &live_parent);
    fs::rename(&parked_parent, &live_parent).expect("restore exact prior directory");
    let restored_prior_bytes = fs::read(&history_path);
    let before_retry = snapshot(&registry, &target, &peer);
    let retry_result = record_target(
        &mut registry,
        &target,
        REPLACEMENT_POSITION,
        "replacement-target",
    );
    let after_retry = snapshot(&registry, &target, &peer);
    let retry_bytes = fs::read(&history_path);
    let reloaded = HomingRegistry::with_record_path(&history_path, configured, tolerance);
    let reloaded_snapshot = reloaded
        .as_ref()
        .ok()
        .map(|registry| snapshot(registry, &target, &peer));
    drop(reloaded);
    drop(registry);
    drop(directory);
    let cleanup_complete = !owned_root
        .try_exists()
        .expect("actual exclusive cleanup query");

    println!(
        "replacement controls: prior={prior_result:?}; failed={failed_result:?}; \
         before={before_failure:?}; failed_snapshot={failed_snapshot:?}; \
         retry={retry_result:?}; cleanup_complete={cleanup_complete}"
    );
    assert!(
        cleanup_complete,
        "replacement fixture cleanup must precede outcome assertions"
    );

    assert_eq!(initial.calibration.joints.len(), 2);
    assert_eq!(initial.target_state, JointHomingState::Unhomed);
    assert_eq!(initial.peer_state, JointHomingState::Unhomed);
    assert!(
        prior_result.is_ok(),
        "real prior target commit must be reachable: {prior_result:?}"
    );
    assert_eq!(before_failure.target_state, JointHomingState::Verified);
    assert_eq!(before_failure.peer_state, JointHomingState::Unhomed);
    assert!(before_failure.target_out_of_limits && before_failure.peer_out_of_limits);
    let prior_bytes = prior_bytes.expect("actual prior committed history bytes");
    assert_saved_history(&prior_bytes, PRIOR_POSITION, "prior-committed-target");
    assert_eq!(
        parked_bytes_before_failure.expect("exact prior bytes immediately after parking"),
        prior_bytes
    );
    assert!(blocker_is_file);
    assert!(
        matches!(
            &failed_result,
            Err(RegistryError::Io { path, message }) if path == &live_parent && !message.is_empty()
        ),
        "replacement must reach the actual typed parent I/O failure: {failed_result:?}"
    );
    assert_eq!(
        blocker_bytes_after_failure.expect("preserved blocker"),
        BLOCKER_BYTES
    );
    assert!(failed_destination_read.is_err());
    assert_eq!(
        parked_bytes_after_failure.expect("prior bytes after failure"),
        prior_bytes
    );
    assert_eq!(
        restored_prior_bytes.expect("restored exact history"),
        prior_bytes
    );
    assert_eq!(failed_snapshot.target_state, before_failure.target_state);
    assert_eq!(failed_snapshot.peer_state, before_failure.peer_state);
    assert_eq!(
        failed_snapshot.target_out_of_limits,
        before_failure.target_out_of_limits
    );
    assert_eq!(
        failed_snapshot.peer_out_of_limits,
        before_failure.peer_out_of_limits
    );
    assert_eq!(failed_snapshot.all_verified, before_failure.all_verified);
    assert_eq!(failed_snapshot.ready, before_failure.ready);
    assert_eq!(before_retry.calibration, failed_snapshot.calibration);
    assert_eq!(before_retry.target_state, failed_snapshot.target_state);
    assert_eq!(before_retry.peer_state, failed_snapshot.peer_state);
    assert_eq!(
        before_retry.target_out_of_limits,
        failed_snapshot.target_out_of_limits
    );
    assert_eq!(
        before_retry.peer_out_of_limits,
        failed_snapshot.peer_out_of_limits
    );

    assert!(
        retry_result.is_ok(),
        "same replacement must succeed after restoration: {retry_result:?}"
    );
    assert_eq!(after_retry.target_state, JointHomingState::Verified);
    assert!(
        !after_retry.target_out_of_limits,
        "successful target retry clears its own flag"
    );
    assert_eq!(after_retry.peer_state, JointHomingState::Unhomed);
    assert!(
        after_retry.peer_out_of_limits,
        "target retry must preserve the peer flag"
    );
    assert_saved_history(
        &retry_bytes.expect("actual successful replacement bytes"),
        REPLACEMENT_POSITION,
        "replacement-target",
    );
    let reloaded_snapshot =
        reloaded_snapshot.expect("actual successful replacement reconstruction");
    assert_eq!(reloaded_snapshot.calibration, after_retry.calibration);
    let initial_peer = initial
        .calibration
        .find_joint(&peer.joint)
        .expect("independent literal unrelated row");
    for (stage, calibration) in [
        ("prior commit", &before_failure.calibration),
        ("failed replacement", &failed_snapshot.calibration),
        ("before retry", &before_retry.calibration),
        ("successful retry", &after_retry.calibration),
        ("reconstruction", &reloaded_snapshot.calibration),
    ] {
        assert_eq!(
            calibration.find_joint(&peer.joint),
            Some(initial_peer),
            "every unrelated public history field must survive {stage}"
        );
    }
    assert_eq!(reloaded_snapshot.target_state, JointHomingState::Unhomed);
    assert_eq!(reloaded_snapshot.peer_state, JointHomingState::Unhomed);
    assert!(!reloaded_snapshot.target_out_of_limits && !reloaded_snapshot.peer_out_of_limits);
    assert!(!reloaded_snapshot.all_verified && reloaded_snapshot.ready.is_err());

    // This final named oracle cannot be satisfied by a successful later retry,
    // lost peer history or a writer that never reached the filesystem.
    assert_eq!(
        failed_snapshot.calibration, before_failure.calibration,
        "failed replacement must preserve prior target and unrelated calibration history"
    );
}
