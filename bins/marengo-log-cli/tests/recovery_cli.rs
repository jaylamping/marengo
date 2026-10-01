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

#[cfg(target_os = "linux")]
#[test]
fn recovery_cli_reports_completed_artifacts_when_stdout_is_full() {
    use std::io::Write as _;
    use std::os::unix::fs::FileTypeExt as _;
    use std::process::Stdio;

    type EventRow = (
        i64,
        i64,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    );

    #[derive(Debug, PartialEq)]
    struct Snapshot {
        settings: Vec<(String, String, i64)>,
        events: Vec<EventRow>,
        searches: Vec<Vec<i64>>,
        integrity: Vec<String>,
    }

    fn snapshot(store: &Store, schema: &str) -> Result<Snapshot, String> {
        let conn = store.connection();
        let read = || -> marengo_store::Result<Snapshot> {
            let mut settings = conn.prepare(&format!(
                "SELECT key,value_json,updated_ms FROM {schema}.settings ORDER BY key"
            ))?;
            let settings = settings
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            let mut events = conn.prepare(&format!(
                "SELECT id,ts_ms,level,target,message,session_id,fields_json FROM {schema}.log_events ORDER BY id"
            ))?;
            let events = events
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut searches = Vec::new();
            for token in ["clihistory", "clifield"] {
                let mut statement = conn.prepare(&format!(
                    "SELECT rowid FROM {schema}.log_events_fts WHERE log_events_fts MATCH ?1 ORDER BY rowid"
                ))?;
                searches.push(
                    statement
                        .query_map([token], |row| row.get(0))?
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            let mut integrity = conn.prepare(&format!("PRAGMA {schema}.integrity_check"))?;
            let integrity = integrity
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Snapshot {
                settings,
                events,
                searches,
                integrity,
            })
        };
        read().map_err(|error| error.to_string())
    }

    let directory = tempfile::Builder::new()
        .prefix("marengo-recovery-cli-full-")
        .tempdir()
        .expect("exclusive stdout-failure fixture");
    let root = directory.path().to_path_buf();
    let source = root.join("historic.sqlite");
    let backup = root.join("backup.sqlite");
    let output = root.join("output.sqlite");
    let unused_root = root.join("configured-root-must-stay-absent");
    let unused_db = unused_root.join("configured.sqlite");
    // As in the existing CLI test, public Store bootstrap is a stated composition
    // fixture dependency; Store's literal fixtures own historical recognition.
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
        .expect("stale marker fixture");
    let logical_before = snapshot(&store, "main");
    drop(store);
    let bytes_before = fs::read(&source).expect("closed source before CLI");
    let normal_refused = refused(&source, &root);
    let bytes_after_refusal = fs::read(&source).map_err(|error| error.to_string());
    let sink_metadata = fs::symlink_metadata("/dev/full")
        .map(|metadata| metadata.file_type().is_char_device())
        .map_err(|error| error.to_string());
    let sink_error = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .and_then(|mut sink| sink.write_all(b"independent ENOSPC control"));
    let sink_is_full = sink_error
        .as_ref()
        .err()
        .is_some_and(|error| error.raw_os_error() == Some(28));
    let fresh = [
        backup.try_exists(),
        output.try_exists(),
        unused_root.try_exists(),
    ];
    let fixture_ok = logical_before.as_ref().is_ok_and(|before| {
        before
            .settings
            .contains(&("schema_version".into(), "1".into(), 17))
            && before
                .settings
                .contains(&("operator_neighbor".into(), "keep".into(), 31))
            && before.events
                == vec![(
                    1,
                    501,
                    "info".into(),
                    "cli_fixture".into(),
                    "clihistory".into(),
                    None,
                    Some(r#"{"detail":"clifield"}"#.into()),
                )]
            && before.searches == vec![vec![1], vec![1]]
            && before.integrity == vec!["ok"]
    });
    let controls = root.is_absolute()
        && fixture_ok
        && normal_refused
        && bytes_after_refusal.as_deref() == Ok(bytes_before.as_slice())
        && sink_metadata == Ok(true)
        && sink_is_full
        && fresh.iter().all(|exists| matches!(exists, Ok(false)));
    let child = if controls {
        let sink = fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("independently verified stdout sink");
        Command::new(assert_cmd::cargo::cargo_bin("marengo-log-cli"))
            .arg("--root")
            .arg(&unused_root)
            .arg("--db")
            .arg(&unused_db)
            .arg("recover-known-v2")
            .arg("--source")
            .arg(&source)
            .arg("--backup")
            .arg(&backup)
            .arg("--output")
            .arg(&output)
            .stdout(Stdio::from(sink))
            .output()
            .map_err(|error| error.to_string())
    } else {
        Err("independent fixture/ENOSPC controls failed; CLI not invoked".into())
    };
    let source_after = fs::read(&source).map_err(|error| error.to_string());
    let backup_before_read = fs::read(&backup).map_err(|error| error.to_string());
    let output_before_read = fs::read(&output).map_err(|error| error.to_string());
    let artifact_paths = [fs::canonicalize(&backup), fs::canonicalize(&output)];
    let completed_images = [&backup_before_read, &output_before_read]
        .iter()
        .all(|image| {
            image.as_ref().is_ok_and(|bytes| {
                bytes.len() > 100
                    && &bytes[..16] == b"SQLite format 3\0"
                    && bytes[18] == 1
                    && bytes[19] == 1
            })
        });
    let observed = (|| -> Result<(Snapshot, Snapshot, Snapshot), String> {
        if !completed_images {
            return Err("CLI did not leave both standalone database images".into());
        }
        let store = Store::open(&output, &root).map_err(|error| error.to_string())?;
        {
            let conn = store.connection();
            // All three images exist; query_only makes these observation
            // attachments incapable of changing their supplied logical state.
            conn.pragma_update(None, "query_only", true)
                .map_err(|error| error.to_string())?;
            conn.execute(
                "ATTACH DATABASE ?1 AS retained_backup",
                [backup.to_str().ok_or("fixture backup path is not UTF8")?],
            )
            .map_err(|error| error.to_string())?;
            conn.execute(
                "ATTACH DATABASE ?1 AS retained_source",
                [source.to_str().ok_or("fixture source path is not UTF8")?],
            )
            .map_err(|error| error.to_string())?;
        }
        let backup_view = snapshot(&store, "retained_backup")?;
        let source_view = snapshot(&store, "retained_source")?;
        let output_view = snapshot(&store, "main")?;
        Ok((backup_view, source_view, output_view))
    })();
    let backup_after_read = fs::read(&backup).map_err(|error| error.to_string());
    let source_after_read = fs::read(&source).map_err(|error| error.to_string());
    let backup_refused = refused(&backup, &root);
    let reopened = output_history(&output, &root);
    let unused_absent = unused_root.try_exists().map(|exists| !exists);
    let preserved = matches!((&logical_before, &observed),
        (Ok(before), Ok((backup_view, source_view, output_view)))
        if backup_view == before && source_view == before
            && output_view.settings.iter().any(|(key,value,_)| key == "schema_version" && value == "3")
            && output_view.settings.iter().filter(|(key,_,_)| key != "schema_version").collect::<Vec<_>>()
                == before.settings.iter().filter(|(key,_,_)| key != "schema_version").collect::<Vec<_>>()
            && output_view.events == before.events && output_view.searches == before.searches
            && output_view.integrity == vec!["ok"])
        && source_after.as_deref() == Ok(bytes_before.as_slice())
        && source_after_read.as_deref() == Ok(bytes_before.as_slice())
        && backup_after_read == backup_before_read
        && backup_refused
        && reopened == Ok(true)
        && matches!(unused_absent, Ok(true));
    let reported = child.as_ref().is_ok_and(|child| {
        let stderr = String::from_utf8_lossy(&child.stderr);
        child.status.code() == Some(1)
            && child.stdout.is_empty()
            && stderr.contains("os error 28")
            && artifact_paths[0].as_ref().is_ok_and(|path| {
                stderr.contains(&format!("retained published backup={}", path.display()))
            })
            && artifact_paths[1].as_ref().is_ok_and(|path| {
                stderr.contains(&format!("retained published output={}", path.display()))
            })
    });
    let cleanup = directory.close().map_err(|error| error.to_string());
    let absent = root
        .try_exists()
        .map(|exists| !exists)
        .map_err(|error| error.to_string());
    let cleaned = cleanup.is_ok() && absent == Ok(true);
    if controls && cleaned {
        println!("CLI_OUTPUT_FAILURE_CONTROLS_OK");
    }
    assert!(controls && cleaned && preserved && reported,
        "recovery CLI stdout failure must report both completed artifacts\ncontrols={controls}; preserved={preserved}; reported={reported}; sink_metadata={sink_metadata:?}; sink_error={sink_error:?}; child={child:?}; logical_before={logical_before:?}; observed={observed:?}; artifact_paths={artifact_paths:?}; source_bytes_preserved={}; backup_unchanged={}; backup_refused={backup_refused}; reopened={reopened:?}; unused_absent={unused_absent:?}; cleanup={cleanup:?}; absent={absent:?}", source_after.as_deref() == Ok(bytes_before.as_slice()) && source_after_read.as_deref() == Ok(bytes_before.as_slice()), backup_after_read == backup_before_read);
}
