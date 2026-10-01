//! Deliberate known-v2 recovery is new-capability conformance, not a baseline red.
//! An independently proved WAL row must survive a completed backup and separate output.
#![allow(clippy::expect_used)]

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use marengo_store::{
    recover_known_v2, LogEventInsert, RecoveryReceipt, Store, StoreError, StructuredLogQuery,
};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

const FIXTURE: &str = r#"
CREATE TABLE settings(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_ms INTEGER NOT NULL);
CREATE TABLE log_events(id INTEGER PRIMARY KEY,ts_ms INTEGER NOT NULL,level TEXT NOT NULL,
 target TEXT NOT NULL,message TEXT NOT NULL,session_id TEXT,fields_json TEXT);
CREATE INDEX log_events_ts ON log_events(ts_ms);
CREATE INDEX log_events_level ON log_events(level);
CREATE INDEX log_events_target ON log_events(target);
CREATE INDEX log_events_session ON log_events(session_id);
CREATE TABLE log_sessions(id TEXT PRIMARY KEY,label TEXT,started_ms INTEGER NOT NULL,ended_ms INTEGER,
 bench_blob TEXT,candump_blob TEXT,trace_blob TEXT,candump_frame_count INTEGER,candump_bytes INTEGER);
CREATE INDEX log_sessions_started ON log_sessions(started_ms);
CREATE TABLE config_overrides(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,
 updated_ms INTEGER NOT NULL,source TEXT NOT NULL);
CREATE VIRTUAL TABLE log_events_fts USING fts5(message,target,fields_json,
 content='log_events',content_rowid='id');
CREATE TRIGGER log_events_ai AFTER INSERT ON log_events BEGIN
 INSERT INTO log_events_fts(rowid,message,target,fields_json)
 VALUES(new.id,new.message,new.target,COALESCE(new.fields_json,'')); END;
CREATE TRIGGER log_events_ad AFTER DELETE ON log_events BEGIN
 INSERT INTO log_events_fts(log_events_fts,rowid,message,target,fields_json)
 VALUES('delete',old.id,old.message,old.target,COALESCE(old.fields_json,'')); END;
CREATE TRIGGER log_events_au AFTER UPDATE ON log_events BEGIN
 INSERT INTO log_events_fts(log_events_fts,rowid,message,target,fields_json)
 VALUES('delete',old.id,old.message,old.target,COALESCE(old.fields_json,''));
 INSERT INTO log_events_fts(rowid,message,target,fields_json)
 VALUES(new.id,new.message,new.target,COALESCE(new.fields_json,'')); END;
INSERT INTO settings VALUES('schema_version','1',17),('log_archive_days','7',23),
 ('log_disk_budget_bytes','123456',29),('operator_neighbor','keep',31);
INSERT INTO log_events VALUES(11,100,'info','historic','historicalmessage','capture',
 '{"detail":"historicalfield"}');
INSERT INTO log_sessions VALUES('capture','literal session',41,42,NULL,NULL,NULL,2,19);
INSERT INTO config_overrides VALUES('neighbor_config','true',37,'operator');
INSERT INTO log_events_fts(log_events_fts) VALUES('rebuild');
"#;

const WAL_ROW: &str = r#"
INSERT INTO log_events VALUES(29,300,'warn','wal','walmessage','capture',
 '{"detail":"walfield"}');
"#;

const HISTORIC_TOKENS: [&str; 4] = [
    "historicalmessage",
    "historicalfield",
    "walmessage",
    "walfield",
];

type Rows = Vec<Vec<Value>>;

#[derive(Clone, Debug, PartialEq)]
struct View {
    schema: Rows,
    settings: Rows,
    events: Rows,
    sessions: Rows,
    config: Rows,
    fts: Vec<Vec<i64>>,
    integrity: Rows,
}

fn rows(conn: &Connection, sql: &str) -> Result<Rows, String> {
    let mut statement = conn.prepare(sql).map_err(|error| error.to_string())?;
    let mapped = statement
        .query_map([], |row| {
            (0..row.as_ref().column_count())
                .map(|column| row.get::<_, Value>(column))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|error| error.to_string())?;
    mapped
        .collect::<rusqlite::Result<_>>()
        .map_err(|error| error.to_string())
}

fn observe(path: &Path) -> Result<View, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let mut fts = Vec::new();
    for token in HISTORIC_TOKENS {
        let mut statement = conn
            .prepare(
                "SELECT rowid FROM log_events_fts WHERE log_events_fts MATCH ?1 ORDER BY rowid",
            )
            .map_err(|error| error.to_string())?;
        let mapped = statement
            .query_map([token], |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())?;
        fts.push(
            mapped
                .collect::<rusqlite::Result<_>>()
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(View {
        schema: rows(
            &conn,
            "SELECT type,name,COALESCE(sql,'') FROM sqlite_schema ORDER BY type,name",
        )?,
        settings: rows(&conn, "SELECT key,value_json,updated_ms FROM settings ORDER BY key")?,
        events: rows(
            &conn,
            "SELECT id,ts_ms,level,target,message,session_id,fields_json FROM log_events ORDER BY id",
        )?,
        sessions: rows(
            &conn,
            "SELECT id,label,started_ms,ended_ms,bench_blob,candump_blob,trace_blob,candump_frame_count,candump_bytes FROM log_sessions ORDER BY id",
        )?,
        config: rows(
            &conn,
            "SELECT key,value_json,updated_ms,source FROM config_overrides ORDER BY key",
        )?,
        fts,
        integrity: rows(&conn, "PRAGMA integrity_check")?,
    })
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

fn literal_controls(view: &View) -> bool {
    use Value::{Integer, Null};
    view.settings
        == vec![
            vec![text("log_archive_days"), text("7"), Integer(23)],
            vec![text("log_disk_budget_bytes"), text("123456"), Integer(29)],
            vec![text("operator_neighbor"), text("keep"), Integer(31)],
            vec![text("schema_version"), text("1"), Integer(17)],
        ]
        && view.events
            == vec![
                vec![
                    Integer(11),
                    Integer(100),
                    text("info"),
                    text("historic"),
                    text("historicalmessage"),
                    text("capture"),
                    text(r#"{"detail":"historicalfield"}"#),
                ],
                vec![
                    Integer(29),
                    Integer(300),
                    text("warn"),
                    text("wal"),
                    text("walmessage"),
                    text("capture"),
                    text(r#"{"detail":"walfield"}"#),
                ],
            ]
        && view.sessions
            == vec![vec![
                text("capture"),
                text("literal session"),
                Integer(41),
                Integer(42),
                Null,
                Null,
                Null,
                Integer(2),
                Integer(19),
            ]]
        && view.config
            == vec![vec![
                text("neighbor_config"),
                text("true"),
                Integer(37),
                text("operator"),
            ]]
        && view.fts == vec![vec![11], vec![11], vec![29], vec![29]]
        && view.integrity == vec![vec![text("ok")]]
}

fn main_only_lacks_wal_row(main: &View, source: &View) -> bool {
    main.schema == source.schema
        && main.settings == source.settings
        && main.sessions == source.sessions
        && main.config == source.config
        && main.events == source.events[..1]
        && main.fts == vec![vec![11], vec![11], vec![], vec![]]
        && main.integrity == vec![vec![text("ok")]]
}

fn seed_wal(source: &Path) -> (Connection, (i64, i64, i64)) {
    let writer = Connection::open(source).expect("exclusive independent fixture connection");
    writer
        .pragma_update(None, "journal_mode", "WAL")
        .expect("literal source uses WAL");
    writer
        .pragma_update(None, "wal_autocheckpoint", 0_i64)
        .expect("keep committed row in WAL");
    writer
        .execute_batch(FIXTURE)
        .expect("literal complete v2 with stale marker1");
    let checkpoint = writer
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .expect("establish complete base file before WAL-only row");
    writer
        .execute_batch(WAL_ROW)
        .expect("later independently committed WAL row");
    (writer, checkpoint)
}

fn historical_refusal(error: &StoreError) -> bool {
    matches!(error, StoreError::Message(message)
        if message.contains("historic partial v2") && message.contains("backed-up recovery"))
}

fn file_digest(path: &Path) -> Result<(u64, String), String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let length = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        write!(&mut digest, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    Ok((length, digest))
}

fn standalone_image(path: &Path) -> Result<bool, String> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        if Path::new(&sidecar)
            .try_exists()
            .map_err(|error| error.to_string())?
        {
            return Ok(false);
        }
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let journal: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(|error| error.to_string())?;
    Ok(journal.eq_ignore_ascii_case("delete"))
}

fn same_canonical_path(left: &Path, right: &Path) -> Result<bool, String> {
    let left = fs::canonicalize(left).map_err(|error| error.to_string())?;
    let right = fs::canonicalize(right).map_err(|error| error.to_string())?;
    Ok(left == right)
}

fn ordinary_settings(view: &View) -> Rows {
    view.settings
        .iter()
        .filter(|row| row[0] != text("schema_version"))
        .cloned()
        .collect()
}

fn preserved_history(output: &View, source: &View) -> bool {
    output.schema == source.schema
        && ordinary_settings(output) == ordinary_settings(source)
        && output.events == source.events
        && output.sessions == source.sessions
        && output.config == source.config
        && output.fts == source.fts
        && output.integrity == vec![vec![text("ok")]]
        && output
            .settings
            .iter()
            .any(|row| row[0] == text("schema_version") && row[1] == text("3"))
}

#[derive(Debug)]
struct RecoveryEvidence {
    receipt: RecoveryReceipt,
    path_identities: [bool; 3],
    backup: View,
    output: View,
    backup_copy: View,
    output_copy: View,
    reopened: View,
    searches: Vec<(Vec<i64>, u32)>,
    backup_digest: (u64, String),
    output_digest: (u64, String),
    backup_reopen_digest: (u64, String),
    output_reopen_digest: (u64, String),
    copy_digests: [(u64, String); 2],
    standalone_before_after: [bool; 6],
}

fn recover_and_observe(
    source: &Path,
    backup: &Path,
    output: &Path,
    root: &Path,
) -> Result<RecoveryEvidence, String> {
    let receipt = recover_known_v2(source, backup, output).map_err(|error| error.to_string())?;
    let path_identities = [
        same_canonical_path(&receipt.source_path, source)?,
        same_canonical_path(&receipt.backup_path, backup)?,
        same_canonical_path(&receipt.output_path, output)?,
    ];
    let backup_digest = file_digest(backup)?;
    let output_digest = file_digest(output)?;
    let backup_standalone_before = standalone_image(backup)?;
    let output_standalone_before = standalone_image(output)?;
    let backup_view = observe(backup)?;
    let output_view = observe(output)?;
    let backup_standalone_after = standalone_image(backup)?;
    let output_standalone_after = standalone_image(output)?;
    let backup_reopen_digest = file_digest(backup)?;
    let output_reopen_digest = file_digest(output)?;
    let copies = root.join("standalone-copies");
    fs::create_dir(&copies).map_err(|error| error.to_string())?;
    let backup_copy = copies.join("backup.sqlite");
    let output_copy = copies.join("output.sqlite");
    fs::copy(backup, &backup_copy).map_err(|error| error.to_string())?;
    fs::copy(output, &output_copy).map_err(|error| error.to_string())?;
    let backup_copy_standalone = standalone_image(&backup_copy)?;
    let output_copy_standalone = standalone_image(&output_copy)?;
    let backup_copy_view = observe(&backup_copy)?;
    let output_copy_view = observe(&output_copy)?;
    let copy_digests = [file_digest(&backup_copy)?, file_digest(&output_copy)?];
    // Completion is observed before normal Store open can switch output to WAL.
    let store = Store::open(output, root).map_err(|error| error.to_string())?;
    store
        .insert_log_events(&[LogEventInsert {
            ts_ms: 501,
            level: "info".into(),
            target: "public_recovery".into(),
            message: "freshmessage".into(),
            session_id: Some("capture".into()),
            fields_json: Some(r#"{"detail":"freshfield"}"#.into()),
        }])
        .map_err(|error| error.to_string())?;
    drop(store);
    let reopened = Store::open(output, root).map_err(|error| error.to_string())?;
    reopened.migrate().map_err(|error| error.to_string())?;
    let mut searches = Vec::new();
    for token in HISTORIC_TOKENS
        .into_iter()
        .chain(["freshmessage", "freshfield"])
    {
        let (events, total) = reopened
            .query_structured_logs(&StructuredLogQuery {
                from_ms: None,
                to_ms: None,
                level: None,
                target: None,
                session_id: None,
                q: Some(token.into()),
                offset: 0,
                limit: 10,
            })
            .map_err(|error| error.to_string())?;
        searches.push((events.into_iter().map(|row| row.id).collect(), total));
    }
    drop(reopened);
    Ok(RecoveryEvidence {
        receipt,
        path_identities,
        backup: backup_view,
        output: output_view,
        backup_copy: backup_copy_view,
        output_copy: output_copy_view,
        reopened: observe(output)?,
        searches,
        backup_digest,
        output_digest,
        backup_reopen_digest,
        output_reopen_digest,
        copy_digests,
        standalone_before_after: [
            backup_standalone_before,
            output_standalone_before,
            backup_standalone_after,
            output_standalone_after,
            backup_copy_standalone,
            output_copy_standalone,
        ],
    })
}

#[test]
fn known_v2_recovery_completes_backup_and_separate_output_without_changing_source() {
    let directory = tempfile::Builder::new()
        .prefix("marengo-g15-recovery-")
        .tempdir()
        .expect("exclusive fixture");
    let root = directory.path().to_path_buf();
    let source = root.join("historic.sqlite");
    let backup = root.join("completed-backup.sqlite");
    let output = root.join("recovered.sqlite");
    let main_only = root.join("main-only.sqlite");
    let (writer, checkpoint) = seed_wal(&source);
    fs::copy(&source, &main_only).expect("independent main-file-only negative control");
    let wal_bytes = fs::metadata(root.join("historic.sqlite-wal"))
        .map(|metadata| metadata.len())
        .map_err(|error| error.to_string());
    let before = observe(&source);
    let copied_main = observe(&main_only);
    let normal = Store::open(&source, &root);
    let refused = normal.as_ref().err().is_some_and(historical_refusal);
    let refusal_error = normal.as_ref().err().map(|error| format!("{error:?}"));
    drop(normal);
    let after_normal = observe(&source);
    // These fixture/refusal controls do not depend on any new recovery outcome.
    let controls = root.is_absolute()
        && checkpoint == (0, 0, 0)
        && wal_bytes.as_ref().is_ok_and(|bytes| *bytes > 32)
        && before.as_ref().is_ok_and(literal_controls)
        && matches!((&copied_main, &before), (Ok(main), Ok(source))
            if main_only_lacks_wal_row(main, source))
        && refused
        && before.is_ok()
        && after_normal == before;
    let recovered = if controls {
        recover_and_observe(&source, &backup, &output, &root)
    } else {
        Err("independent fixture/refusal controls failed; recovery not invoked".into())
    };
    let after_source = observe(&source);
    let recovery_ok = matches!((&recovered, &before), (Ok(evidence), Ok(original)) if {
        let receipt = &evidence.receipt;
        let mut expected_reopened = evidence.output.clone();
        expected_reopened.events.push(vec![
            Value::Integer(30), Value::Integer(501), text("info"), text("public_recovery"),
            text("freshmessage"), text("capture"), text(r#"{"detail":"freshfield"}"#),
        ]);
        evidence.path_identities == [true; 3] && receipt.source_version == 1
            && receipt.output_version == 3
            && receipt.backup_bytes == evidence.backup_digest.0
            && receipt.backup_sha256 == evidence.backup_digest.1
            && receipt.output_bytes == evidence.output_digest.0
            && receipt.output_sha256 == evidence.output_digest.1
            && evidence.backup_digest == evidence.backup_reopen_digest
            && evidence.output_digest == evidence.output_reopen_digest
            && evidence.copy_digests[0] == evidence.backup_digest
            && evidence.copy_digests[1] == evidence.output_digest
            && evidence.standalone_before_after == [true; 6]
            && evidence.backup == *original
            && evidence.backup_copy == evidence.backup
            && evidence.output_copy == evidence.output
            && preserved_history(&evidence.output, original)
            && evidence.reopened == expected_reopened
            && evidence.searches == vec![
                (vec![11], 1), (vec![11], 1), (vec![29], 1), (vec![29], 1),
                (vec![30], 1), (vec![30], 1),
            ]
    });
    drop(writer);
    let cleanup = directory.close().map_err(|error| error.to_string());
    let absent = root
        .try_exists()
        .map(|exists| !exists)
        .map_err(|error| error.to_string());
    let cleaned = cleanup.is_ok() && absent == Ok(true);
    if controls && cleaned {
        println!("G15_RECOVERY_TRACER_CONTROLS_OK");
    }
    assert!(
        controls && cleaned && recovery_ok && after_source == before,
        "known-v2 recovery backup/output/source/FTS conformance failed\ncleanup={cleanup:?}; absent={absent:?}; controls={controls}; refused={refused}; refusal={refusal_error:?}; recovery_ok={recovery_ok}; checkpoint={checkpoint:?}; wal_bytes={wal_bytes:?}; before={before:?}; main_only={copied_main:?}; after_normal={after_normal:?}; recovered={recovered:?}; after_source={after_source:?}"
    );
}

#[derive(Clone, Copy, Debug)]
enum RefusalCase {
    ExtraTable,
    MissingTrigger,
    ChangedLiteral,
    TriggerComment,
    MissingMarker,
    MalformedMarker,
    FutureMarker,
    CorruptFts,
}

impl RefusalCase {
    fn sql(self) -> &'static str {
        match self {
            Self::ExtraTable => "CREATE TABLE unrecognized_neighbor(value TEXT);",
            Self::MissingTrigger => "DROP TRIGGER log_events_ai;",
            Self::ChangedLiteral => {
                r#"
DROP TRIGGER log_events_ai;
CREATE TRIGGER log_events_ai AFTER INSERT ON log_events BEGIN
 INSERT INTO log_events_fts(rowid,message,target,fields_json)
 VALUES(new.id,new.message,new.target,COALESCE(new.fields_json,' ')); END;
"#
            }
            Self::TriggerComment => {
                r#"
DROP TRIGGER log_events_ai;
CREATE TRIGGER log_events_ai AFTER INSERT ON log_events BEGIN
 INSERT INTO log_events_fts(rowid,message,target,fields_json)
 VALUES(new.id,/* unsupported historical comment */new.message,new.target,
 COALESCE(new.fields_json,'')); END;
"#
            }
            Self::MissingMarker => "DELETE FROM settings WHERE key='schema_version';",
            Self::MalformedMarker => {
                "UPDATE settings SET value_json='1x' WHERE key='schema_version';"
            }
            Self::FutureMarker => "UPDATE settings SET value_json='4' WHERE key='schema_version';",
            Self::CorruptFts => {
                r#"
INSERT INTO log_events_fts(log_events_fts,rowid,message,target,fields_json)
 VALUES('delete',29,'walmessage','wal','{"detail":"walfield"}');
"#
            }
        }
    }

    fn fixture_matches(self, before: &View, original: &View) -> bool {
        let mut expected = original.clone();
        match self {
            Self::ExtraTable => {
                let extra = vec![
                    text("table"),
                    text("unrecognized_neighbor"),
                    text("CREATE TABLE unrecognized_neighbor(value TEXT)"),
                ];
                if before.schema.iter().filter(|row| **row == extra).count() != 1
                    || schema_without(before, "unrecognized_neighbor") != original.schema
                {
                    return false;
                }
                expected.schema = before.schema.clone();
            }
            Self::MissingTrigger => {
                expected.schema = schema_without(original, "log_events_ai");
            }
            Self::ChangedLiteral | Self::TriggerComment => {
                let fragment = match self {
                    Self::ChangedLiteral => "COALESCE(new.fields_json,' ')",
                    _ => "/* unsupported historical comment */",
                };
                let changed = before
                    .schema
                    .iter()
                    .filter(|row| {
                        row[0] == text("trigger")
                            && row[1] == text("log_events_ai")
                            && matches!(&row[2], Value::Text(sql) if sql.contains(fragment))
                    })
                    .count();
                if changed != 1
                    || schema_without(before, "log_events_ai")
                        != schema_without(original, "log_events_ai")
                    || before.schema == original.schema
                {
                    return false;
                }
                expected.schema = before.schema.clone();
            }
            Self::MissingMarker => {
                expected
                    .settings
                    .retain(|row| row[0] != text("schema_version"));
            }
            Self::MalformedMarker | Self::FutureMarker => {
                let value = match self {
                    Self::MalformedMarker => "1x",
                    _ => "4",
                };
                for row in &mut expected.settings {
                    if row[0] == text("schema_version") {
                        row[1] = text(value);
                    }
                }
            }
            Self::CorruptFts => {
                expected.fts = vec![vec![11], vec![11], vec![], vec![]];
            }
        }
        *before == expected
    }

    fn expected_error(self, error: &StoreError) -> bool {
        let phase = match self {
            Self::CorruptFts => "failed during backup creation:",
            _ => "failed during recognition:",
        };
        matches!(error, StoreError::Message(message)
            if message.contains(phase)
                && message.contains("retained published backup=none")
                && message.contains("retained published output=none"))
    }
}

fn schema_without(view: &View, name: &str) -> Rows {
    view.schema
        .iter()
        .filter(|row| row[1] != text(name))
        .cloned()
        .collect()
}

fn directory_names(path: &Path) -> Result<Vec<std::ffi::OsString>, String> {
    let entries = fs::read_dir(path).map_err(|error| error.to_string())?;
    let mut names = entries
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    names.sort();
    Ok(names)
}

fn namespace_absent(path: &Path) -> Result<bool, String> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        match fs::symlink_metadata(Path::new(&name)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
            Ok(_) => return Ok(false),
        }
    }
    Ok(true)
}

fn corrupt_fts_control(writer: &mut Connection) -> (bool, String) {
    match writer.transaction() {
        Err(error) => (false, format!("independent FTS transaction: {error:?}")),
        Ok(transaction) => {
            let integrity = transaction.execute(
                "INSERT INTO log_events_fts(log_events_fts,rank) VALUES('integrity-check',1)",
                [],
            );
            let corrupt = matches!(&integrity, Err(rusqlite::Error::SqliteFailure(code, _))
                if code.code == rusqlite::ErrorCode::DatabaseCorrupt);
            let rollback = transaction.rollback();
            (
                corrupt && rollback.is_ok(),
                format!("rank1={integrity:?}; rollback={rollback:?}"),
            )
        }
    }
}

#[derive(Debug)]
struct RefusalObservation {
    controls: bool,
    cleaned: bool,
    passed: bool,
    details: String,
}

#[test]
fn recovery_refuses_unsupported_profiles_and_unverified_fts_without_artifacts() {
    let mut observations = Vec::new();
    for case in [
        RefusalCase::ExtraTable,
        RefusalCase::MissingTrigger,
        RefusalCase::ChangedLiteral,
        RefusalCase::TriggerComment,
        RefusalCase::MissingMarker,
        RefusalCase::MalformedMarker,
        RefusalCase::FutureMarker,
        RefusalCase::CorruptFts,
    ] {
        let directory = tempfile::Builder::new()
            .prefix("marengo-g15-recovery-refusal-")
            .tempdir()
            .expect("exclusive refusal fixture");
        let root = directory.path().to_path_buf();
        let source = root.join("historic.sqlite");
        let backup = root.join("backup.sqlite");
        let output = root.join("output.sqlite");
        let main_only = root.join("main-only.sqlite");
        let (mut writer, checkpoint) = seed_wal(&source);
        fs::copy(&source, &main_only).expect("independent main-only refusal control");
        let wal_bytes = fs::metadata(root.join("historic.sqlite-wal"))
            .map(|metadata| metadata.len())
            .map_err(|error| error.to_string());
        let original = observe(&source);
        let copied_main = observe(&main_only);
        writer
            .execute_batch(case.sql())
            .expect("literal refusal fixture change");
        let (fts_control, fts_details) = if matches!(case, RefusalCase::CorruptFts) {
            corrupt_fts_control(&mut writer)
        } else {
            (true, "rank1 corruption control not applicable".into())
        };
        let before = observe(&source);
        let entries_before = directory_names(&root);
        let fresh_before = (namespace_absent(&backup), namespace_absent(&output));
        let controls = root.is_absolute()
            && checkpoint == (0, 0, 0)
            && wal_bytes.as_ref().is_ok_and(|bytes| *bytes > 32)
            && original.as_ref().is_ok_and(literal_controls)
            && matches!((&copied_main, &original), (Ok(main), Ok(source))
                if main_only_lacks_wal_row(main, source))
            && matches!((&before, &original), (Ok(before), Ok(original))
                if case.fixture_matches(before, original))
            && fts_control
            && entries_before.is_ok()
            && fresh_before == (Ok(true), Ok(true));
        let result = if controls {
            recover_known_v2(&source, &backup, &output)
        } else {
            Err(StoreError::msg(
                "independent refusal controls failed; recovery not invoked",
            ))
        };
        let refused = result
            .as_ref()
            .err()
            .is_some_and(|error| case.expected_error(error));
        let after = observe(&source);
        let entries_after = directory_names(&root);
        let fresh_after = (namespace_absent(&backup), namespace_absent(&output));
        let passed = refused
            && before.is_ok()
            && after == before
            && entries_after == entries_before
            && fresh_after == (Ok(true), Ok(true));
        drop(writer);
        let cleanup = directory.close().map_err(|error| error.to_string());
        let absent = root
            .try_exists()
            .map(|exists| !exists)
            .map_err(|error| error.to_string());
        let cleaned = cleanup.is_ok() && absent == Ok(true);
        observations.push(RefusalObservation {
            controls, cleaned, passed,
            details: format!("case={case:?}; result={result:?}; checkpoint={checkpoint:?}; wal_bytes={wal_bytes:?}; fts_control={fts_control}; {fts_details}; fresh_before={fresh_before:?}; fresh_after={fresh_after:?}; entries_before={entries_before:?}; entries_after={entries_after:?}; original={original:?}; main_only={copied_main:?}; before={before:?}; after={after:?}; cleanup={cleanup:?}; absent={absent:?}"),
        });
    }
    let controls = observations
        .iter()
        .all(|observation| observation.controls && observation.cleaned);
    let passed = observations.iter().all(|observation| observation.passed);
    let details = observations
        .iter()
        .map(|observation| observation.details.as_str())
        .collect::<Vec<_>>();
    if controls {
        println!("G15_RECOVERY_REFUSAL_CONTROLS_OK");
    }
    assert!(
        controls && passed,
        "known-v2 recovery refusal/backup-verification/source/cleanup failed\n{details:#?}"
    );
}

#[derive(Clone, Copy, Debug)]
enum NamespaceCase {
    MissingSource,
    MissingParent,
    AmbiguousLeaf,
    SourceDestinationAlias,
    DestinationAlias,
    SourceSidecarOverlap,
    ExistingFile,
    ExistingDirectory,
    BackupWalSentinel,
    OutputShmSentinel,
    CaseJournalSentinel,
    HardlinkDestination,
    SourceNamespaceHardlink,
}

struct NamespacePaths {
    source: std::path::PathBuf,
    backup: std::path::PathBuf,
    output: std::path::PathBuf,
    files: Vec<std::path::PathBuf>,
    directories: Vec<std::path::PathBuf>,
    source_alias: Option<std::path::PathBuf>,
    control: Result<bool, String>,
}

impl NamespaceCase {
    fn prepare(self, root: &Path, source: &Path) -> NamespacePaths {
        let mut paths = NamespacePaths {
            source: source.to_path_buf(),
            backup: root.join("backup.sqlite"),
            output: root.join("output.sqlite"),
            files: vec![source.to_path_buf()],
            directories: vec![root.to_path_buf()],
            source_alias: None,
            control: Ok(true),
        };
        match self {
            Self::MissingSource => {
                paths.source = root.join("missing.sqlite");
                paths.control = namespace_absent(&paths.source);
            }
            Self::MissingParent => {
                let missing = root.join("missing-parent");
                paths.backup = missing.join("backup.sqlite");
                paths.control = missing
                    .try_exists()
                    .map(|exists| !exists)
                    .map_err(|error| error.to_string());
            }
            Self::AmbiguousLeaf => {
                paths.backup = root.join("backup.sqlite.");
                paths.control = namespace_absent(&paths.backup);
            }
            Self::SourceDestinationAlias => {
                let parent_alias = root.join("parent-alias");
                fs::create_dir(&parent_alias).expect("owned canonical-parent alias control");
                paths.directories.push(parent_alias.clone());
                paths.backup = parent_alias.join("..").join("historic.sqlite");
                paths.control = same_file::is_same_file(&paths.backup, source)
                    .map_err(|error| error.to_string());
            }
            Self::DestinationAlias => {
                paths.output = paths.backup.clone();
                paths.control = namespace_absent(&paths.backup);
            }
            Self::SourceSidecarOverlap => {
                paths.backup = root.join("historic.sqlite-wal");
                paths.control = fs::metadata(&paths.backup)
                    .map(|metadata| metadata.len() > 32)
                    .map_err(|error| error.to_string());
            }
            Self::ExistingFile => {
                fs::write(&paths.backup, b"owned existing backup sentinel")
                    .expect("owned existing file control");
                paths.files.push(paths.backup.clone());
            }
            Self::ExistingDirectory => {
                fs::create_dir(&paths.output).expect("owned existing directory control");
                let child = paths.output.join("sentinel.txt");
                fs::write(&child, b"owned existing directory child")
                    .expect("owned directory child control");
                paths.files.push(child);
                paths.directories.push(paths.output.clone());
            }
            Self::BackupWalSentinel | Self::OutputShmSentinel | Self::CaseJournalSentinel => {
                let sentinel = root.join(match self {
                    Self::BackupWalSentinel => "backup.sqlite-wal",
                    Self::OutputShmSentinel => "output.sqlite-shm",
                    _ => "OUTPUT.SQLITE-journal",
                });
                fs::write(&sentinel, b"owned SQLite namespace sentinel")
                    .expect("owned sidecar namespace control");
                paths.files.push(sentinel);
            }
            Self::HardlinkDestination => {
                fs::hard_link(source, &paths.backup).expect("owned destination hardlink control");
                paths.control = same_file::is_same_file(source, &paths.backup)
                    .map_err(|error| error.to_string());
                paths.files.push(paths.backup.clone());
            }
            Self::SourceNamespaceHardlink => {
                let alias = root.join("historic.sqlite-journal");
                fs::hard_link(source, &alias).expect("owned source-namespace hardlink control");
                paths.control =
                    same_file::is_same_file(source, &alias).map_err(|error| error.to_string());
                paths.files.push(alias.clone());
                paths.source_alias = Some(alias);
            }
        }
        paths
    }
}

#[derive(Debug, PartialEq)]
struct ProtectedFile {
    identity: same_file::Handle,
    digest: (u64, String),
}

fn protect_files(paths: &[std::path::PathBuf]) -> Result<Vec<ProtectedFile>, String> {
    paths
        .iter()
        .map(|path| {
            let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "sentinel is not a regular file: {}",
                    path.display()
                ));
            }
            Ok(ProtectedFile {
                identity: same_file::Handle::from_path(path).map_err(|error| error.to_string())?,
                digest: file_digest(path)?,
            })
        })
        .collect()
}

#[derive(Debug, PartialEq)]
struct ProtectedDirectory {
    identity: same_file::Handle,
    names: Vec<std::ffi::OsString>,
}

fn protect_directories(paths: &[std::path::PathBuf]) -> Result<Vec<ProtectedDirectory>, String> {
    paths
        .iter()
        .map(|path| {
            Ok(ProtectedDirectory {
                identity: same_file::Handle::from_path(path).map_err(|error| error.to_string())?,
                names: directory_names(path)?,
            })
        })
        .collect()
}

#[test]
fn recovery_refuses_path_collisions_without_changing_source_or_sentinels() {
    let mut observations = Vec::new();
    let mut all_controls = true;
    let mut all_refusals = true;
    for case in [
        NamespaceCase::MissingSource,
        NamespaceCase::MissingParent,
        NamespaceCase::AmbiguousLeaf,
        NamespaceCase::SourceDestinationAlias,
        NamespaceCase::DestinationAlias,
        NamespaceCase::SourceSidecarOverlap,
        NamespaceCase::ExistingFile,
        NamespaceCase::ExistingDirectory,
        NamespaceCase::BackupWalSentinel,
        NamespaceCase::OutputShmSentinel,
        NamespaceCase::CaseJournalSentinel,
        NamespaceCase::HardlinkDestination,
        NamespaceCase::SourceNamespaceHardlink,
    ] {
        let directory = tempfile::Builder::new()
            .prefix("marengo-g15-recovery-paths-")
            .tempdir()
            .expect("exclusive namespace fixture");
        let root = directory.path().to_path_buf();
        let source = root.join("historic.sqlite");
        let main_only = root.join("main-only.sqlite");
        let (writer, checkpoint) = seed_wal(&source);
        fs::copy(&source, &main_only).expect("independent namespace main-only control");
        let wal_bytes = fs::metadata(root.join("historic.sqlite-wal"))
            .map(|metadata| metadata.len())
            .map_err(|error| error.to_string());
        let before = observe(&source);
        let copied_main = observe(&main_only);
        let paths = case.prepare(&root, &source);
        let files_before = protect_files(&paths.files);
        let directories_before = protect_directories(&paths.directories);
        let controls = root.is_absolute()
            && checkpoint == (0, 0, 0)
            && wal_bytes.as_ref().is_ok_and(|bytes| *bytes > 32)
            && before.as_ref().is_ok_and(literal_controls)
            && matches!((&copied_main, &before), (Ok(main), Ok(source))
                if main_only_lacks_wal_row(main, source))
            && paths.control == Ok(true)
            && files_before.is_ok()
            && directories_before.is_ok();
        let result = if controls {
            recover_known_v2(&paths.source, &paths.backup, &paths.output)
        } else {
            Err(StoreError::msg(
                "independent namespace controls failed; recovery not invoked",
            ))
        };
        let refused = matches!(&result, Err(StoreError::Message(message))
            if message.contains("refused during filesystem preflight:"));
        let files_after = protect_files(&paths.files);
        let directories_after = protect_directories(&paths.directories);
        let files_preserved = files_before.is_ok() && files_after == files_before;
        let directories_preserved =
            directories_before.is_ok() && directories_after == directories_before;
        let file_details = format!("before={files_before:?}; after={files_after:?}");
        let directory_details =
            format!("before={directories_before:?}; after={directories_after:?}");
        // Drop identity handles before fixture-owned link removal and Windows cleanup.
        drop(files_before);
        drop(files_after);
        drop(directories_before);
        drop(directories_after);
        // The collision's presence/identity was collected above. Remove only our own
        // fabricated journal alias before opening SQLite for source observation.
        let alias_cleanup = match &paths.source_alias {
            Some(alias) => fs::remove_file(alias).map_err(|error| error.to_string()),
            None => Ok(()),
        };
        let after = observe(&source);
        let passed = refused
            && files_preserved
            && directories_preserved
            && alias_cleanup.is_ok()
            && before.is_ok()
            && after == before;
        drop(writer);
        let cleanup = directory.close().map_err(|error| error.to_string());
        let absent = root
            .try_exists()
            .map(|exists| !exists)
            .map_err(|error| error.to_string());
        let cleaned = cleanup.is_ok() && absent == Ok(true);
        all_controls &= controls && cleaned && alias_cleanup.is_ok();
        all_refusals &= passed;
        observations.push(format!("case={case:?}; controls={controls}; cleaned={cleaned}; passed={passed}; source={}; backup={}; output={}; case_control={:?}; result={result:?}; checkpoint={checkpoint:?}; wal_bytes={wal_bytes:?}; file protection: {file_details}; directory protection: {directory_details}; alias_cleanup={alias_cleanup:?}; before={before:?}; main_only={copied_main:?}; after={after:?}; cleanup={cleanup:?}; absent={absent:?}",
            paths.source.display(), paths.backup.display(), paths.output.display(), paths.control));
    }
    if all_controls {
        println!("G15_RECOVERY_NAMESPACE_CONTROLS_OK");
    }
    assert!(
        all_controls && all_refusals,
        "known-v2 recovery namespace/source/sentinel preservation failed\n{observations:#?}"
    );
}

#[cfg(target_os = "linux")]
fn proc_stages(path: &Path) -> Result<Vec<std::ffi::OsString>, String> {
    Ok(directory_names(path)?
        .into_iter()
        .filter(|name| {
            name.to_str()
                .is_some_and(|name| name.starts_with(".marengo-recovery-"))
        })
        .collect())
}

#[cfg(target_os = "linux")]
#[test]
fn output_creation_failure_retains_completed_backup_and_allows_fresh_retry() {
    let directory = tempfile::Builder::new()
        .prefix("marengo-g15-recovery-retry-")
        .tempdir()
        .expect("exclusive later-failure fixture");
    let root = directory.path().to_path_buf();
    let source = root.join("historic.sqlite");
    let first_backup = root.join("retained-backup.sqlite");
    let retry_backup = root.join("retry-backup.sqlite");
    let retry_output = root.join("retry-output.sqlite");
    let main_only = root.join("main-only.sqlite");
    let proc_parent = fs::canonicalize("/proc/self/fd").expect("existing Linux fd parent");
    let failed_output = proc_parent.join("marengo-recovery-output.sqlite");
    let (writer, checkpoint) = seed_wal(&source);
    fs::copy(&source, &main_only).expect("independent retry main-only control");
    let wal_bytes = fs::metadata(root.join("historic.sqlite-wal"))
        .map(|metadata| metadata.len())
        .map_err(|error| error.to_string());
    let before = observe(&source);
    let copied_main = observe(&main_only);
    let proc_directory = fs::symlink_metadata(&proc_parent)
        .map(|metadata| metadata.file_type().is_dir())
        .map_err(|error| error.to_string());
    let stages_before = proc_stages(&proc_parent);
    let entries_before = directory_names(&root);
    let fresh_before = [
        namespace_absent(&first_backup),
        namespace_absent(&retry_backup),
        namespace_absent(&retry_output),
        namespace_absent(&failed_output),
    ];
    let controls = root.is_absolute()
        && proc_parent.is_absolute()
        && proc_directory == Ok(true)
        && stages_before == Ok(Vec::new())
        && checkpoint == (0, 0, 0)
        && wal_bytes.as_ref().is_ok_and(|bytes| *bytes > 32)
        && before.as_ref().is_ok_and(literal_controls)
        && matches!((&copied_main, &before), (Ok(main), Ok(source))
            if main_only_lacks_wal_row(main, source))
        && entries_before.is_ok()
        && fresh_before.iter().all(|fresh| *fresh == Ok(true));
    let failed = if controls {
        recover_known_v2(&source, &first_backup, &failed_output)
    } else {
        Err(StoreError::msg(
            "independent retry controls failed; recovery not invoked",
        ))
    };
    let retained_path = fs::canonicalize(&first_backup).map_err(|error| error.to_string());
    let reported = matches!((&failed, &retained_path), (Err(StoreError::Message(message)), Ok(path))
        if message.contains("failed during output creation:")
            && message.contains(&format!("retained published backup={}", path.display()))
            && message.contains("retained published output=none"));
    let retained_before = protect_files(std::slice::from_ref(&first_backup));
    let standalone_before = standalone_image(&first_backup);
    let retained_view = observe(&first_backup);
    let retained_reopened = protect_files(std::slice::from_ref(&first_backup));
    let standalone_reopened = standalone_image(&first_backup);
    let after_failure = observe(&source);
    let failed_namespace = namespace_absent(&failed_output);
    let stages_after_failure = proc_stages(&proc_parent);
    let entries_after_failure = directory_names(&root);
    let mut expected_first_entries = entries_before.clone();
    if let Ok(names) = &mut expected_first_entries {
        names.push("retained-backup.sqlite".into());
        names.sort();
    }
    let retained_complete = reported
        && standalone_before == Ok(true)
        && standalone_reopened == Ok(true)
        && retained_before.is_ok()
        && retained_reopened == retained_before
        && retained_view == before
        && after_failure == before
        && failed_namespace == Ok(true)
        && stages_after_failure == Ok(Vec::new())
        && entries_after_failure == expected_first_entries;
    let retry = if controls {
        recover_known_v2(&source, &retry_backup, &retry_output)
    } else {
        Err(StoreError::msg(
            "independent retry controls failed; retry not invoked",
        ))
    };
    let retry_paths = match &retry {
        Ok(receipt) => vec![
            same_canonical_path(&receipt.source_path, &source),
            same_canonical_path(&receipt.backup_path, &retry_backup),
            same_canonical_path(&receipt.output_path, &retry_output),
        ],
        Err(_) => Vec::new(),
    };
    let retry_images = protect_files(&[retry_backup.clone(), retry_output.clone()]);
    let retry_standalone = [
        standalone_image(&retry_backup),
        standalone_image(&retry_output),
    ];
    let retry_backup_view = observe(&retry_backup);
    let retry_output_view = observe(&retry_output);
    let retry_complete = matches!((&retry, &retry_images, &before, &retry_backup_view, &retry_output_view),
        (Ok(receipt), Ok(images), Ok(original), Ok(backup), Ok(output))
        if images.len() == 2 && receipt.source_version == 1 && receipt.output_version == 3
            && receipt.backup_bytes == images[0].digest.0
            && receipt.backup_sha256 == images[0].digest.1
            && receipt.output_bytes == images[1].digest.0
            && receipt.output_sha256 == images[1].digest.1
            && backup == original && preserved_history(output, original))
        && retry_paths.len() == 3
        && retry_paths.iter().all(|identity| *identity == Ok(true))
        && retry_standalone
            .iter()
            .all(|standalone| *standalone == Ok(true));
    let retained_after_retry = protect_files(std::slice::from_ref(&first_backup));
    let retained_standalone_after_retry = standalone_image(&first_backup);
    let retained_view_after_retry = observe(&first_backup);
    let after_retry = observe(&source);
    let failed_namespace_after_retry = namespace_absent(&failed_output);
    let stages_after_retry = proc_stages(&proc_parent);
    let entries_after_retry = directory_names(&root);
    let mut expected_retry_entries = entries_before.clone();
    if let Ok(names) = &mut expected_retry_entries {
        for name in [
            "retained-backup.sqlite",
            "retry-backup.sqlite",
            "retry-output.sqlite",
        ] {
            names.push(name.into());
        }
        names.sort();
    }
    let first_preserved = retained_before.is_ok()
        && retained_after_retry == retained_before
        && retained_standalone_after_retry == Ok(true)
        && retained_view_after_retry == before;
    let passed = retained_complete
        && retry_complete
        && first_preserved
        && after_retry == before
        && failed_namespace_after_retry == Ok(true)
        && stages_after_retry == Ok(Vec::new())
        && entries_after_retry == expected_retry_entries;
    let details = format!("controls={controls}; passed={passed}; retained_complete={retained_complete}; retry_complete={retry_complete}; first_preserved={first_preserved}; proc_parent={}; failed_output={}; proc_directory={proc_directory:?}; stages_before={stages_before:?}; fresh_before={fresh_before:?}; failed={failed:?}; retained_path={retained_path:?}; reported={reported}; retained_before={retained_before:?}; retained_reopened={retained_reopened:?}; retained_after_retry={retained_after_retry:?}; standalone_before={standalone_before:?}; standalone_reopened={standalone_reopened:?}; retained_standalone_after_retry={retained_standalone_after_retry:?}; failed_namespace={failed_namespace:?}; failed_namespace_after_retry={failed_namespace_after_retry:?}; retry={retry:?}; retry_paths={retry_paths:?}; retry_images={retry_images:?}; retry_standalone={retry_standalone:?}; stages_after_failure={stages_after_failure:?}; stages_after_retry={stages_after_retry:?}; entries_before={entries_before:?}; entries_after_failure={entries_after_failure:?}; expected_first_entries={expected_first_entries:?}; entries_after_retry={entries_after_retry:?}; expected_retry_entries={expected_retry_entries:?}; before={before:?}; main_only={copied_main:?}; retained_view={retained_view:?}; retained_view_after_retry={retained_view_after_retry:?}; after_failure={after_failure:?}; after_retry={after_retry:?}; retry_backup_view={retry_backup_view:?}; retry_output_view={retry_output_view:?}",
        proc_parent.display(), failed_output.display());
    drop(retained_before);
    drop(retained_reopened);
    drop(retained_after_retry);
    drop(retry_images);
    drop(writer);
    let cleanup = directory.close().map_err(|error| error.to_string());
    let absent = root
        .try_exists()
        .map(|exists| !exists)
        .map_err(|error| error.to_string());
    let cleaned = cleanup.is_ok() && absent == Ok(true);
    if controls && cleaned {
        println!("G15_RECOVERY_RETRY_CONTROLS_OK");
    }
    assert!(controls && cleaned && passed,
        "known-v2 recovery retained-backup/output-failure/retry failed\n{details}\ncleanup={cleanup:?}; absent={absent:?}");
}

fn expected_default_settings(output: &View, source: &View) -> Option<View> {
    let timestamp = output
        .settings
        .iter()
        .find_map(|row| match row.as_slice() {
            [Value::Text(key), Value::Text(value), Value::Integer(timestamp)]
                if key == "schema_version" && value == "3" =>
            {
                Some(*timestamp)
            }
            _ => None,
        })?;
    let mut expected = source.clone();
    expected.settings = vec![
        vec![
            text("log_archive_days"),
            text("30"),
            Value::Integer(timestamp),
        ],
        vec![
            text("log_disk_budget_bytes"),
            text("5368709120"),
            Value::Integer(timestamp),
        ],
        vec![text("operator_neighbor"), text("keep"), Value::Integer(31)],
        vec![text("schema_version"), text("3"), Value::Integer(timestamp)],
    ];
    Some(expected)
}

#[test]
fn recovery_inserts_missing_defaults_once_with_the_output_marker_timestamp() {
    let directory = tempfile::Builder::new()
        .prefix("marengo-g15-recovery-defaults-")
        .tempdir()
        .expect("exclusive missing-default fixture");
    let root = directory.path().to_path_buf();
    let source = root.join("historic.sqlite");
    let backup = root.join("backup.sqlite");
    let output = root.join("output.sqlite");
    let main_only = root.join("main-only.sqlite");
    let (writer, checkpoint) = seed_wal(&source);
    fs::copy(&source, &main_only).expect("independent missing-default main-only control");
    let wal_bytes = fs::metadata(root.join("historic.sqlite-wal"))
        .map(|metadata| metadata.len())
        .map_err(|error| error.to_string());
    let original = observe(&source);
    let copied_main = observe(&main_only);
    writer
        .execute_batch(
            "DELETE FROM settings WHERE key IN ('log_archive_days','log_disk_budget_bytes');",
        )
        .expect("literal deletion of both optional defaults");
    let before = observe(&source);
    let expected_missing = original.clone().map(|mut view| {
        view.settings.retain(|row| {
            row[0] != text("log_archive_days") && row[0] != text("log_disk_budget_bytes")
        });
        view
    });
    let entries_before = directory_names(&root);
    let fresh_before = [namespace_absent(&backup), namespace_absent(&output)];
    let controls = root.is_absolute()
        && checkpoint == (0, 0, 0)
        && wal_bytes.as_ref().is_ok_and(|bytes| *bytes > 32)
        && original.as_ref().is_ok_and(literal_controls)
        && matches!((&copied_main, &original), (Ok(main), Ok(source))
            if main_only_lacks_wal_row(main, source))
        && before == expected_missing
        && entries_before.is_ok()
        && fresh_before.iter().all(|fresh| *fresh == Ok(true));
    let recovered = if controls {
        recover_known_v2(&source, &backup, &output)
    } else {
        Err(StoreError::msg(
            "independent missing-default controls failed; recovery not invoked",
        ))
    };
    let receipt_paths = match &recovered {
        Ok(receipt) => vec![
            same_canonical_path(&receipt.source_path, &source),
            same_canonical_path(&receipt.backup_path, &backup),
            same_canonical_path(&receipt.output_path, &output),
        ],
        Err(_) => Vec::new(),
    };
    let images_before = protect_files(&[backup.clone(), output.clone()]);
    let standalone_before = [standalone_image(&backup), standalone_image(&output)];
    let backup_view = observe(&backup);
    let output_view = observe(&output);
    let images_reopened = protect_files(&[backup.clone(), output.clone()]);
    let standalone_reopened = [standalone_image(&backup), standalone_image(&output)];
    let closed_entries = directory_names(&root);
    let mut expected_entries = entries_before.clone();
    if let Ok(names) = &mut expected_entries {
        names.push("backup.sqlite".into());
        names.push("output.sqlite".into());
        names.sort();
    }
    let expected_output = match (&output_view, &before) {
        (Ok(output), Ok(source)) => expected_default_settings(output, source),
        _ => None,
    };
    let completed = matches!((&recovered, &images_before, &output_view, &expected_output),
        (Ok(receipt), Ok(images), Ok(actual), Some(expected))
        if images.len() == 2 && receipt.source_version == 1 && receipt.output_version == 3
            && receipt.backup_bytes == images[0].digest.0
            && receipt.backup_sha256 == images[0].digest.1
            && receipt.output_bytes == images[1].digest.0
            && receipt.output_sha256 == images[1].digest.1 && actual == expected)
        && receipt_paths.len() == 3
        && receipt_paths.iter().all(|identity| *identity == Ok(true))
        && images_reopened == images_before
        && backup_view == before
        && standalone_before.iter().all(|image| *image == Ok(true))
        && standalone_reopened.iter().all(|image| *image == Ok(true))
        && closed_entries == expected_entries;
    let opened = Store::open(&output, &root);
    let open_result = opened
        .as_ref()
        .map(|_| ())
        .map_err(|error| error.to_string());
    drop(opened);
    let after_open = observe(&output);
    let reopened = Store::open(&output, &root);
    let migrate_result = match &reopened {
        Ok(store) => store.migrate().map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    drop(reopened);
    let after_migrate = observe(&output);
    let after_source = observe(&source);
    let backup_after = protect_files(std::slice::from_ref(&backup));
    let backup_view_after = observe(&backup);
    let backup_preserved = matches!((&images_before, &backup_after), (Ok(before), Ok(after))
        if before.len() == 2 && after.len() == 1 && before[0] == after[0])
        && backup_view_after == before;
    let passed = completed
        && open_result.is_ok()
        && migrate_result.is_ok()
        && after_open == output_view
        && after_migrate == output_view
        && after_source == before
        && backup_preserved;
    let details = format!("controls={controls}; passed={passed}; completed={completed}; backup_preserved={backup_preserved}; checkpoint={checkpoint:?}; wal_bytes={wal_bytes:?}; recovered={recovered:?}; receipt_paths={receipt_paths:?}; original={original:?}; main_only={copied_main:?}; expected_missing={expected_missing:?}; before={before:?}; backup_view={backup_view:?}; output_view={output_view:?}; expected_output={expected_output:?}; images_before={images_before:?}; images_reopened={images_reopened:?}; standalone_before={standalone_before:?}; standalone_reopened={standalone_reopened:?}; entries_before={entries_before:?}; closed_entries={closed_entries:?}; expected_entries={expected_entries:?}; open_result={open_result:?}; migrate_result={migrate_result:?}; after_open={after_open:?}; after_migrate={after_migrate:?}; after_source={after_source:?}; backup_after={backup_after:?}; backup_view_after={backup_view_after:?}");
    drop(images_before);
    drop(images_reopened);
    drop(backup_after);
    drop(writer);
    let cleanup = directory.close().map_err(|error| error.to_string());
    let absent = root
        .try_exists()
        .map(|exists| !exists)
        .map_err(|error| error.to_string());
    let cleaned = cleanup.is_ok() && absent == Ok(true);
    if controls && cleaned {
        println!("G15_RECOVERY_DEFAULTS_CONTROLS_OK");
    }
    assert!(controls && cleaned && passed,
        "known-v2 recovery missing-default values/timestamps preservation failed\n{details}\ncleanup={cleanup:?}; absent={absent:?}");
}
