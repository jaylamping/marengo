//! Existing-public historical commit consistency, not current motor permission.
//!
//! Overlay this file as crates/marengo-homing/tests/history_commit_atomicity.rs
//! in an independently archived checkout. It uses the actual existing crate and
//! that archive's existing tests/support/mod.rs, with no production extraction,
//! new API, dependency, hardware transport or private state mutation.
//!
//! Capture failure before retry; perform the writable controls and remove the
//! exclusive directory before every outcome assertion. Setup failures are not
//! behavioral regression proof.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use marengo_config::{HomingConfigFile, HomingMethod, MotorEntry, MotorsConfigFile};
use marengo_homing::{CalibrationRecord, HomingRegistry, JointHomingState, RegistryError};

#[path = "support/mod.rs"]
mod support;
use support::TestDirectory;

const POSITION_RAD: f64 = 0.01;
const REVISION_METADATA: &str = "history-commit-fixture";
const BLOCKER_BYTES: &[u8] = b"exclusively owned regular-file parent blocker\n";

#[derive(Debug)]
struct PublicSnapshot {
    calibration: CalibrationRecord,
    state: JointHomingState,
    out_of_limits: bool,
    all_verified: bool,
    ready: Result<(), RegistryError>,
}

fn snapshot(registry: &HomingRegistry, motor: &MotorEntry) -> PublicSnapshot {
    PublicSnapshot {
        calibration: registry.calibration().clone(),
        state: registry.joint_state(&motor.joint),
        out_of_limits: registry.is_out_of_limits(&motor.joint),
        all_verified: registry.all_verified(),
        ready: registry.require_ready(),
    }
}

fn record(
    registry: &mut HomingRegistry,
    motor: &MotorEntry,
    home_offset_rad: f64,
    operator: &str,
) -> Result<(), RegistryError> {
    // Opaque legacy audit metadata, not a qualified hardware/config binding.
    registry.record_verification(
        motor,
        "manual_reference",
        home_offset_rad,
        POSITION_RAD,
        true,
        operator,
        Some(REVISION_METADATA.into()),
    )
}

fn assert_saved_row(bytes: &[u8], motor: &MotorEntry, home_offset_rad: f64, operator: &str) {
    // Read actual bytes independently of HomingRegistry's load/upsert helpers.
    let yaml: serde_yaml::Value = serde_yaml::from_slice(bytes).expect("real history YAML");
    let rows = yaml["joints"].as_sequence().expect("actual history rows");
    assert_eq!(rows.len(), 1, "one real saved legacy row");
    let row = &rows[0];
    assert_eq!(row["joint"].as_str(), Some(motor.joint.as_str()));
    assert_eq!(row["device_id"].as_u64(), Some(u64::from(motor.device_id)));
    assert_eq!(
        row["can_interface"].as_str(),
        Some(motor.can_interface.as_str())
    );
    assert_eq!(row["method"].as_str(), Some("manual_reference"));
    assert_eq!(row["home_offset_rad"].as_f64(), Some(home_offset_rad));
    assert_eq!(row["verified_position_rad"].as_f64(), Some(POSITION_RAD));
    assert_eq!(row["sign_test_passed"].as_bool(), Some(true));
    assert_eq!(row["operator"].as_str(), Some(operator));
    assert_eq!(row["config_revision"].as_str(), Some(REVISION_METADATA));
    let timestamp = row["timestamp_utc"]
        .as_str()
        .expect("actual audit timestamp");
    assert!(
        chrono::DateTime::parse_from_rfc3339(timestamp).is_ok(),
        "writer must emit a valid legacy audit timestamp"
    );
}

#[test]
fn failed_history_commit_preserves_public_state_and_can_be_retried() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("compiled actual crate manifest");
    if let Some(expected) = std::env::var_os("HISTORY_ATOMICITY_EXPECTED_MANIFEST") {
        assert_eq!(
            manifest,
            PathBuf::from(expected)
                .canonicalize()
                .expect("independently bound archived manifest"),
            "probe must compile against the selected actual crate archive"
        );
    }
    let root = manifest.join("../..");
    // Read the archive's configured input directly. Process environment cannot
    // substitute a different motors/homing tree for this finite fixture.
    let motors: MotorsConfigFile = serde_yaml::from_slice(
        &fs::read(root.join("config/motors.yaml")).expect("archived motor policy"),
    )
    .expect("configured motor schema");
    let homing: HomingConfigFile = serde_yaml::from_slice(
        &fs::read(root.join("config/homing.yaml")).expect("archived homing policy"),
    )
    .expect("configured homing schema");
    let motor = motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_elbow_pitch")
        .expect("configured finite elbow fixture")
        .clone();
    let policy = homing
        .homing
        .effective_joint(&motor.joint)
        .expect("configured manual policy");
    let tolerance = homing.homing.zero_verify_tolerance_rad;
    assert_eq!(policy.method, HomingMethod::ManualReference);
    assert!(policy.home_offset_rad.is_finite());
    assert!(tolerance.is_finite() && tolerance > POSITION_RAD);
    assert!(motor.bench.position_lower_rad.is_finite());
    assert!(motor.bench.position_upper_rad.is_finite());
    assert!(motor.bench.position_lower_rad < POSITION_RAD);
    assert!(POSITION_RAD < motor.bench.position_upper_rad);
    assert!(!motor.can_interface.is_empty() && motor.device_id != 0);

    let directory = TestDirectory::new("public-history-commit-atomicity");
    let directory_path = directory.path().to_path_buf();
    let neighbor_path = directory_path.join("writable/history.yaml");
    let blocker_path = directory_path.join("blocked");
    let blocked_history_path = blocker_path.join("history.yaml");

    let mut neighbor =
        HomingRegistry::with_record_path(&neighbor_path, vec![motor.joint.clone()], tolerance)
            .expect("fresh writable public registry");
    let neighbor_before = snapshot(&neighbor, &motor);
    let neighbor_result = record(
        &mut neighbor,
        &motor,
        policy.home_offset_rad,
        "writable-neighbor",
    );
    let neighbor_after = snapshot(&neighbor, &motor);
    let neighbor_bytes = fs::read(&neighbor_path);
    let neighbor_reload =
        HomingRegistry::with_record_path(&neighbor_path, vec![motor.joint.clone()], tolerance);
    let neighbor_reloaded = neighbor_reload
        .as_ref()
        .ok()
        .map(|registry| snapshot(registry, &motor));
    drop(neighbor_reload);

    // Constructor must see genuinely missing history before the exclusive
    // regular-file parent is installed; this is an actual pre-write I/O failure.
    let mut blocked = HomingRegistry::with_record_path(
        &blocked_history_path,
        vec![motor.joint.clone()],
        tolerance,
    )
    .expect("fresh missing public history before blocking parent");
    let blocked_before = snapshot(&blocked, &motor);
    let path_missing_before = !blocked_history_path.exists() && !blocker_path.exists();
    fs::write(&blocker_path, BLOCKER_BYTES).expect("owned regular-file blocker");
    let blocker_is_file = fs::metadata(&blocker_path)
        .expect("actual blocker")
        .is_file();

    let failed_result = record(
        &mut blocked,
        &motor,
        policy.home_offset_rad,
        "blocked-then-retried",
    );
    let failed_snapshot = snapshot(&blocked, &motor);
    let failed_blocker_bytes = fs::read(&blocker_path);
    let failed_history_read = fs::read(&blocked_history_path);
    let history_resource_absent_after_failure = !blocked_history_path.exists();

    // Remove only the known regular file inside this exclusively owned guard.
    // Retry the SAME public request; snapshotting the failed state first means
    // the successful retry cannot hide an uncommitted in-memory row.
    fs::remove_file(&blocker_path).expect("remove exclusively owned parent blocker");
    let retry_before = snapshot(&blocked, &motor);
    let retry_result = record(
        &mut blocked,
        &motor,
        policy.home_offset_rad,
        "blocked-then-retried",
    );
    let retry_after = snapshot(&blocked, &motor);
    let retry_bytes = fs::read(&blocked_history_path);
    let retry_reload = HomingRegistry::with_record_path(
        &blocked_history_path,
        vec![motor.joint.clone()],
        tolerance,
    );
    let retry_reloaded = retry_reload
        .as_ref()
        .ok()
        .map(|registry| snapshot(registry, &motor));
    drop(retry_reload);

    drop(neighbor);
    drop(blocked);
    // Existing guard validates its resolved direct-child target before removal.
    drop(directory);
    let cleanup_complete = !directory_path
        .try_exists()
        .expect("actual exclusive cleanup query");

    println!(
        "history atomicity controls: neighbor={neighbor_result:?}; failed={failed_result:?}; \
         before={blocked_before:?}; failed_snapshot={failed_snapshot:?}; \
         retry={retry_result:?}; cleanup_complete={cleanup_complete}"
    );
    assert!(
        cleanup_complete,
        "exclusive fixture must be cleaned before outcome assertions"
    );

    assert!(neighbor_before.calibration.joints.is_empty());
    assert_eq!(neighbor_before.state, JointHomingState::Unhomed);
    assert!(
        neighbor_result.is_ok(),
        "writable neighbor must reach real persistence: {neighbor_result:?}"
    );
    assert_eq!(neighbor_after.state, JointHomingState::Verified);
    assert!(neighbor_after.all_verified && neighbor_after.ready.is_ok());
    assert_saved_row(
        &neighbor_bytes.expect("actual writable-neighbor bytes"),
        &motor,
        policy.home_offset_rad,
        "writable-neighbor",
    );
    let neighbor_reloaded = neighbor_reloaded.expect("actual writable-neighbor reconstruction");
    assert_eq!(neighbor_reloaded.calibration, neighbor_after.calibration);
    assert_eq!(neighbor_reloaded.state, JointHomingState::Unhomed);
    assert!(!neighbor_reloaded.all_verified && neighbor_reloaded.ready.is_err());

    assert!(path_missing_before && blocker_is_file);
    assert!(blocked_before.calibration.joints.is_empty());
    assert_eq!(blocked_before.state, JointHomingState::Unhomed);
    assert!(
        matches!(
            &failed_result,
            Err(RegistryError::Io { path, message })
                if path == &blocker_path && !message.is_empty()
        ),
        "regular-file parent must cause the actual typed pre-write I/O error: {failed_result:?}"
    );
    assert_eq!(
        failed_blocker_bytes.expect("preserved real blocker"),
        BLOCKER_BYTES
    );
    assert!(failed_history_read.is_err() && history_resource_absent_after_failure);
    assert_eq!(failed_snapshot.state, blocked_before.state);
    assert_eq!(failed_snapshot.out_of_limits, blocked_before.out_of_limits);
    assert_eq!(failed_snapshot.all_verified, blocked_before.all_verified);
    assert_eq!(failed_snapshot.ready, blocked_before.ready);
    assert_eq!(retry_before.calibration, failed_snapshot.calibration);
    assert_eq!(retry_before.state, failed_snapshot.state);
    assert_eq!(retry_before.out_of_limits, failed_snapshot.out_of_limits);

    assert!(
        retry_result.is_ok(),
        "same public request must reach a real successful retry: {retry_result:?}"
    );
    assert_eq!(retry_after.state, JointHomingState::Verified);
    assert!(retry_after.all_verified && retry_after.ready.is_ok());
    assert_saved_row(
        &retry_bytes.expect("actual successful-retry bytes"),
        &motor,
        policy.home_offset_rad,
        "blocked-then-retried",
    );
    let retry_reloaded = retry_reloaded.expect("actual successful-retry reconstruction");
    assert_eq!(retry_reloaded.calibration, retry_after.calibration);
    assert_eq!(retry_reloaded.state, JointHomingState::Unhomed);
    assert!(!retry_reloaded.all_verified && retry_reloaded.ready.is_err());

    // Decisive regression oracle comes only after real failure, neighbor, retry,
    // reconstruction and cleanup controls have executed successfully.
    assert_eq!(
        failed_snapshot.calibration, blocked_before.calibration,
        "failed persistence must not publish an uncommitted calibration row"
    );
}
