//! Real CLI composition; the independent historical-schema oracle lives in Store tests.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use marengo_store::{LogEventInsert, Store, StoreError, StructuredLogQuery};

fn run(
    root: &Path,
    configured_db: &Path,
    source: &Path,
    backup: &Path,
    output: Option<&Path>,
) -> Output {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("marengo-log-cli"));
    command
        .arg("--root")
        .arg(root)
        .arg("--db")
        .arg(configured_db)
        .arg("recover-known-v2")
        .arg("--source")
        .arg(source)
        .arg("--backup")
        .arg(backup);
    if let Some(output) = output {
        command.arg("--output").arg(output);
    }
    command
        .output()
        .expect("actual recovery CLI child completed")
}

fn refused(path: &Path, root: &Path) -> bool {
    matches!(Store::open(path, root), Err(StoreError::Message(message))
        if message.contains("historic partial v2"))
}

fn output_history(path: &Path, root: &Path) -> Result<bool, String> {
    let store = Store::open(path, root).map_err(|error| error.to_string())?;
    let version = store
        .get_setting("schema_version")
        .map_err(|error| error.to_string())?;
    let neighbor = store
        .get_setting("operator_neighbor")
        .map_err(|error| error.to_string())?;
    let (events, total) = store
        .query_structured_logs(&StructuredLogQuery {
            from_ms: None,
            to_ms: None,
            level: None,
            target: None,
            session_id: None,
            q: Some("clihistory".into()),
            offset: 0,
            limit: 10,
        })
        .map_err(|error| error.to_string())?;
    Ok(version.as_deref() == Some("3")
        && neighbor.as_deref() == Some("keep")
        && total == 1
        && events.len() == 1
        && events[0].message == "clihistory"
        && events[0].fields_json.as_deref() == Some(r#"{"detail":"clifield"}"#))
}

#[test]
fn explicit_recovery_cli_requires_paths_preserves_collisions_and_reports_completed_artifacts() {
    let directory = tempfile::Builder::new()
        .prefix("marengo-recovery-cli-")
        .tempdir()
        .expect("exclusive CLI fixture");
    let root = directory.path().to_path_buf();
    let source = root.join("historic.sqlite");
    let backup = root.join("backup.sqlite");
    let output = root.join("output.sqlite");
    let unused_root = root.join("configured-root-must-stay-absent");
    let unused_db = unused_root.join("configured.sqlite");
    // v3 has the same schema as this recognized v2 profile after cache removal.
    // This supported bootstrap is a CLI-composition fixture dependency, not an
    // independent migration/schema oracle or an original new-command red.
    let store = Store::open(&source, &root).expect("supported composition fixture");
    store
        .set_setting("operator_neighbor", "keep", 31)
        .expect("literal neighbor");
    store
        .insert_log_events(&[LogEventInsert {
            ts_ms: 501,
            level: "info".into(),
            target: "cli_fixture".into(),
            message: "clihistory".into(),
            session_id: None,
            fields_json: Some(r#"{"detail":"clifield"}"#.into()),
        }])
        .expect("literal searchable history");
    store
        .set_setting("schema_version", "1", 17)
        .expect("stale marker composition fixture");
    drop(store);
    let before = fs::read(&source).expect("closed source before CLI");
    let normal_refused = refused(&source, &root);
    let missing = run(&unused_root, &unused_db, &source, &backup, None);
    let after_missing = fs::read(&source).map_err(|error| error.to_string());
    fs::write(&backup, b"existing backup evidence").expect("owned collision sentinel");
    let collision = run(&unused_root, &unused_db, &source, &backup, Some(&output));
    let collision_bytes = fs::read(&backup).map_err(|error| error.to_string());
    let after_collision = fs::read(&source).map_err(|error| error.to_string());
    let output_after_collision = output.try_exists();
    fs::remove_file(&backup).expect("remove only owned test sentinel");
    let success = run(&unused_root, &unused_db, &source, &backup, Some(&output));
    let receipt = serde_json::from_slice::<serde_json::Value>(&success.stdout);
    let after_success = fs::read(&source).map_err(|error| error.to_string());
    let backup_bytes = fs::metadata(&backup).map(|metadata| metadata.len());
    let output_bytes = fs::metadata(&output).map(|metadata| metadata.len());
    let canonical = |path: &Path| fs::canonicalize(path).ok();
    let receipt_ok = receipt.as_ref().is_ok_and(|value| {
        let path_matches = |key: &str, path: &Path| {
            value[key].as_str().is_some_and(|returned| {
                canonical(Path::new(returned)).is_some()
                    && canonical(Path::new(returned)) == canonical(path)
            })
        };
        path_matches("source_path", &source)
            && path_matches("backup_path", &backup)
            && path_matches("output_path", &output)
            && value["source_version"] == 1
            && value["output_version"] == 3
            && value["backup_bytes"].as_u64() == backup_bytes.ok()
            && value["output_bytes"].as_u64() == output_bytes.ok()
            && ["backup_sha256", "output_sha256"].into_iter().all(|key| {
                value[key].as_str().is_some_and(|hash| {
                    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            })
    });
    let backup_refused = refused(&backup, &root);
    let preserved = output_history(&output, &root);
    let unused_absent = unused_root.try_exists().map(|exists| !exists);
    let cleanup = directory.close().map_err(|error| error.to_string());
    let absent = root.try_exists().map(|exists| !exists);
    assert!(
        normal_refused && missing.status.code() == Some(2) && missing.stdout.is_empty()
            && String::from_utf8_lossy(&missing.stderr).contains("--output")
            && after_missing.as_deref() == Ok(before.as_slice())
            && collision.status.code() == Some(1) && collision.stdout.is_empty()
            && String::from_utf8_lossy(&collision.stderr).contains("fresh")
            && collision_bytes.as_deref() == Ok(b"existing backup evidence".as_slice())
            && after_collision.as_deref() == Ok(before.as_slice())
            && matches!(output_after_collision, Ok(false)) && success.status.success()
            && receipt_ok && after_success.as_deref() == Ok(before.as_slice())
            && backup_refused && preserved == Ok(true) && matches!(unused_absent, Ok(true))
            && cleanup.is_ok() && matches!(absent, Ok(true)),
        "explicit recovery CLI contract failed: missing={missing:?}; collision={collision:?}; success={success:?}; receipt={receipt:?}; receipt_ok={receipt_ok}; history={preserved:?}; cleanup={cleanup:?}; absent={absent:?}"
    );
}
