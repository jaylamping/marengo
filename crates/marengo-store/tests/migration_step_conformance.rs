//! G15: per-step commits, current metadata preservation and future schema refusal.
//! Literal historical SQL/data fixtures exercise the public Store owner.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::bounded;
use std::path::{Path, PathBuf};

use marengo_store::{Store, StoreError, StructuredLogQuery};
use rusqlite::{Connection, Row};

const BASE: &str = r"
CREATE TABLE settings(key TEXT PRIMARY KEY, value_json TEXT NOT NULL, updated_ms INTEGER NOT NULL);
CREATE TABLE log_events(id INTEGER PRIMARY KEY, ts_ms INTEGER NOT NULL, level TEXT NOT NULL,
  target TEXT NOT NULL, message TEXT NOT NULL, session_id TEXT);
CREATE INDEX log_events_ts ON log_events(ts_ms);
CREATE INDEX log_events_level ON log_events(level);
CREATE INDEX log_events_target ON log_events(target);
CREATE INDEX log_events_session ON log_events(session_id);
CREATE VIRTUAL TABLE log_events_fts USING fts5(message,target,content='log_events',content_rowid='id');
CREATE TABLE log_sessions(id TEXT PRIMARY KEY,label TEXT,started_ms INTEGER NOT NULL,ended_ms INTEGER,
  bench_blob TEXT,candump_blob TEXT,trace_blob TEXT,candump_frame_count INTEGER,candump_bytes INTEGER);
CREATE INDEX log_sessions_started ON log_sessions(started_ms);
CREATE TABLE config_overrides(key TEXT PRIMARY KEY,value_json TEXT NOT NULL,
  updated_ms INTEGER NOT NULL,source TEXT NOT NULL);
CREATE TABLE candump_frame_index(session_id TEXT,ordinal INTEGER,evidence TEXT);
INSERT INTO candump_frame_index VALUES('capture',1,'obsolete cache evidence');
INSERT INTO settings VALUES('schema_version','1',17),('log_archive_days','7',23),
  ('log_disk_budget_bytes','123456',29),('operator_neighbor','keep',31);
INSERT INTO log_events VALUES(11,100,'info','legacy','archivedalpha','capture'),
  (29,300,'error','neighbor','neighboromega',NULL);
INSERT INTO log_sessions VALUES('capture','authoritative label',41,42,
  NULL,NULL,NULL,NULL,NULL);
INSERT INTO config_overrides VALUES('neighbor_config','true',37,'operator');
INSERT INTO log_events_fts(log_events_fts) VALUES('rebuild');
CREATE TRIGGER log_events_ai AFTER INSERT ON log_events BEGIN
  INSERT INTO log_events_fts(rowid) VALUES(new.id); END;
CREATE TRIGGER log_events_ad AFTER DELETE ON log_events BEGIN
  INSERT INTO log_events_fts(log_events_fts,rowid) VALUES('delete',old.id); END;
CREATE TRIGGER log_events_au AFTER UPDATE ON log_events BEGIN
  INSERT INTO log_events_fts(log_events_fts,rowid) VALUES('delete',old.id);
  INSERT INTO log_events_fts(rowid) VALUES(new.id); END;
";

const V2_STEPS: [&str; 10] = [
    "ALTER TABLE log_events ADD COLUMN fields_json TEXT;",
    "DROP TRIGGER log_events_ai;",
    "DROP TRIGGER log_events_ad;",
    "DROP TRIGGER log_events_au;",
    "DROP TABLE log_events_fts;",
    "CREATE VIRTUAL TABLE log_events_fts USING fts5(message,target,fields_json,content='log_events',content_rowid='id');",
    "CREATE TRIGGER log_events_ai AFTER INSERT ON log_events BEGIN INSERT INTO log_events_fts(rowid,message,target,fields_json) VALUES(new.id,new.message,new.target,COALESCE(new.fields_json,'')); END;",
    "CREATE TRIGGER log_events_ad AFTER DELETE ON log_events BEGIN INSERT INTO log_events_fts(log_events_fts,rowid,message,target,fields_json) VALUES('delete',old.id,old.message,old.target,COALESCE(old.fields_json,'')); END;",
    "CREATE TRIGGER log_events_au AFTER UPDATE ON log_events BEGIN INSERT INTO log_events_fts(log_events_fts,rowid,message,target,fields_json) VALUES('delete',old.id,old.message,old.target,COALESCE(old.fields_json,'')); INSERT INTO log_events_fts(rowid,message,target,fields_json) VALUES(new.id,new.message,new.target,COALESCE(new.fields_json,'')); END;",
    "INSERT INTO log_events_fts(log_events_fts) VALUES('rebuild');",
];

const FINAL_MARKER_ABORT: &str = r"
CREATE TRIGGER g15_refuse_final_insert BEFORE INSERT ON settings
WHEN NEW.key='schema_version' AND NEW.value_json='3'
BEGIN SELECT RAISE(ABORT,'g15 final marker refusal'); END;
CREATE TRIGGER g15_refuse_final_update BEFORE UPDATE OF value_json ON settings
WHEN NEW.key='schema_version' AND NEW.value_json='3'
BEGIN SELECT RAISE(ABORT,'g15 final marker refusal'); END;
";
const V2_FIELD: &str = r#"{"detail":"v2fieldneedle"}"#;

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("marengo-g15-step-")
            .tempdir()
            .expect("exclusive runner-configured directory");
        let root = directory.path().to_path_buf();
        assert!(
            root.is_absolute(),
            "all fixture references must be absolute"
        );
        let db = root.join("historic.sqlite");
        Self {
            directory,
            root,
            db,
        }
    }

    fn seed(&self, current: bool) {
        let conn = Connection::open(&self.db).expect("independent seed connection");
        conn.execute_batch(BASE)
            .expect("literal historic v1 fixture");
        let bench = self.root.join("archive").join("bench.log");
        let trace = self.root.join("archive").join("trace.csv");
        conn.execute(
            "UPDATE log_sessions SET bench_blob=?1,trace_blob=?2 WHERE id='capture'",
            rusqlite::params![
                bench.to_string_lossy().as_ref(),
                trace.to_string_lossy().as_ref()
            ],
        )
        .expect("absolute fixture-local artifact references");
        if current {
            for statement in V2_STEPS {
                conn.execute_batch(statement)
                    .expect("literal independent v2 fixture");
            }
            conn.execute_batch(
                "DROP TABLE candump_frame_index; UPDATE settings SET value_json='3',updated_ms=101 WHERE key='schema_version';",
            ).expect("literal current v3 marker and timestamp, no Store bootstrap");
        }
    }

    fn close(self) -> bool {
        self.directory.close().expect("actual fixture cleanup");
        !self.root.try_exists().expect("actual cleanup readback")
    }
}

type Event = (
    i64,
    i64,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);
type Session = (
    String,
    Option<String>,
    i64,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
);

#[derive(Clone, Debug, PartialEq, Eq)]
struct View {
    objects: Vec<(String, String)>,
    columns: Vec<String>,
    fts_columns: Vec<String>,
    settings: Vec<(String, String, i64)>,
    events: Vec<Event>,
    sessions: Vec<Session>,
    config: Vec<(String, String, i64, String)>,
    obsolete: Vec<String>,
    future: Vec<(i64, String)>,
    integrity: Vec<String>,
    alpha: Result<Vec<i64>, String>,
}

fn collect<T, F>(conn: &Connection, sql: &str, mapper: F) -> rusqlite::Result<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([], mapper)?;
    rows.collect()
}

fn fts(conn: &Connection, token: &str) -> Result<Vec<i64>, String> {
    let result = (|| -> rusqlite::Result<Vec<i64>> {
        let mut statement = conn.prepare(
            "SELECT rowid FROM log_events_fts WHERE log_events_fts MATCH ?1 ORDER BY rowid",
        )?;
        let rows = statement.query_map([token], |row| row.get(0))?;
        rows.collect()
    })();
    result.map_err(|error| error.to_string())
}

fn observe(path: &Path) -> Result<View, String> {
    let result = (|| -> rusqlite::Result<View> {
        let conn = Connection::open(path)?;
        let objects = collect(
            &conn,
            "SELECT type,name FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let columns: Vec<String> = collect(
            &conn,
            "SELECT name FROM pragma_table_info('log_events') ORDER BY cid",
            |row| row.get(0),
        )?;
        let fts_columns = collect(
            &conn,
            "SELECT name FROM pragma_table_info('log_events_fts') ORDER BY cid",
            |row| row.get(0),
        )?;
        let fields = if columns.iter().any(|column| column == "fields_json") {
            "fields_json"
        } else {
            "NULL"
        };
        let future_field = if columns.iter().any(|column| column == "future_extension") {
            "future_extension"
        } else {
            "NULL"
        };
        let events = collect(&conn, &format!(
            "SELECT id,ts_ms,level,target,message,session_id,{fields},{future_field} FROM log_events ORDER BY id",
        ), |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?)))?;
        let settings = collect(
            &conn,
            "SELECT key,value_json,updated_ms FROM settings ORDER BY key",
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let sessions = collect(&conn,
            "SELECT id,label,started_ms,ended_ms,bench_blob,candump_blob,trace_blob,candump_frame_count,candump_bytes FROM log_sessions ORDER BY id",
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?)))?;
        let config = collect(
            &conn,
            "SELECT key,value_json,updated_ms,source FROM config_overrides ORDER BY key",
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let obsolete = if objects
            .iter()
            .any(|(_, name)| name == "candump_frame_index")
        {
            collect(
                &conn,
                "SELECT evidence FROM candump_frame_index ORDER BY ordinal",
                |row| row.get(0),
            )?
        } else {
            Vec::new()
        };
        let future = if objects.iter().any(|(_, name)| name == "future_history") {
            collect(
                &conn,
                "SELECT id,note FROM future_history ORDER BY id",
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?
        } else {
            Vec::new()
        };
        let integrity = collect(&conn, "PRAGMA integrity_check", |row| row.get(0))?;
        let alpha = fts(&conn, "archivedalpha");
        Ok(View {
            objects,
            columns,
            fts_columns,
            settings,
            events,
            sessions,
            config,
            obsolete,
            future,
            integrity,
            alpha,
        })
    })();
    result.map_err(|error| error.to_string())
}

fn marker(view: &View) -> Option<&str> {
    view.settings
        .iter()
        .find(|(key, _, _)| key == "schema_version")
        .map(|(_, value, _)| value.as_str())
}

fn ordinary_settings(view: &View) -> Vec<(String, String, i64)> {
    view.settings
        .iter()
        .filter(|(key, _, _)| key != "schema_version")
        .cloned()
        .collect()
}

fn literal_controls(view: &View, version: &str, marker_time: i64) -> bool {
    view.settings
        .contains(&("schema_version".into(), version.into(), marker_time))
        && view
            .settings
            .contains(&("log_archive_days".into(), "7".into(), 23))
        && view
            .settings
            .contains(&("log_disk_budget_bytes".into(), "123456".into(), 29))
        && view
            .settings
            .contains(&("operator_neighbor".into(), "keep".into(), 31))
        && view.events.len() == 2
        && view.events[0].0 == 11
        && view.events[1].0 == 29
        && view.sessions.len() == 1
        && view.sessions[0].2 == 41
        && view.sessions[0].3 == Some(42)
        && view.config
            == vec![(
                "neighbor_config".into(),
                "true".into(),
                37,
                "operator".into(),
            )]
        && view.alpha == Ok(vec![11])
        && view.integrity == vec!["ok"]
}

fn preserved(view: &View, source: &View) -> bool {
    ordinary_settings(view) == ordinary_settings(source)
        && view.sessions == source.sessions
        && view.config == source.config
        && view.integrity == vec!["ok"]
        && view.alpha == Ok(vec![11])
}

fn final_refusal(error: &StoreError) -> bool {
    matches!(error, StoreError::Sqlite(rusqlite::Error::SqliteFailure(code, Some(message)))
        if code.code == rusqlite::ErrorCode::ConstraintViolation && message.contains("g15 final marker refusal"))
}

fn write_committed_v2(path: &Path) -> Result<Vec<i64>, String> {
    let conn = Connection::open(path).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO log_events(id,ts_ms,level,target,message,session_id,fields_json) VALUES(41,500,'warn','runtime','committedbeta','capture',?1)",
        [V2_FIELD],
    ).map_err(|error| format!("committed v2 insertion: {error}"))?;
    fts(&conn, "v2fieldneedle")
}

fn public_retry(path: &Path, root: &Path) -> Result<bool, String> {
    let store = Store::open(path, root).map_err(|error| format!("retry open: {error}"))?;
    store
        .migrate()
        .map_err(|error| format!("retry explicit migrate: {error}"))?;
    let rows = store
        .recent_log_events(10)
        .map_err(|error| error.to_string())?;
    let rows_ok = rows.len() == 3
        && rows[0].id == 41
        && rows[0].ts_ms == 500
        && rows[0].message == "committedbeta"
        && rows[0].fields_json.as_deref() == Some(V2_FIELD)
        && rows[1].id == 29
        && rows[2].id == 11;
    let session = store
        .get_session("capture")
        .map_err(|error| error.to_string())?;
    let bench = root
        .join("archive")
        .join("bench.log")
        .to_string_lossy()
        .into_owned();
    let trace = root
        .join("archive")
        .join("trace.csv")
        .to_string_lossy()
        .into_owned();
    let session_ok = session.is_some_and(|row| {
        row.label.as_deref() == Some("authoritative label")
            && row.started_ms == 41
            && row.ended_ms == Some(42)
            && row.bench_blob.as_deref() == Some(bench.as_str())
            && row.candump_blob.is_none()
            && row.trace_blob.as_deref() == Some(trace.as_str())
    });
    let mut searches = Vec::new();
    for token in ["archivedalpha", "v2fieldneedle"] {
        let (rows, total) = store
            .query_structured_logs(&StructuredLogQuery {
                from_ms: None,
                to_ms: None,
                level: None,
                target: None,
                session_id: None,
                q: Some(token.into()),
                limit: 10,
                offset: 0,
            })
            .map_err(|error| format!("retry public FTS: {error}"))?;
        searches.push((
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            total,
        ));
    }
    let version = store
        .get_setting("schema_version")
        .map_err(|error| error.to_string())?;
    drop(store);
    let reopened = Store::open(path, root).map_err(|error| format!("retry reopen: {error}"))?;
    let reopened_rows = reopened
        .recent_log_events(10)
        .map_err(|error| error.to_string())?;
    Ok(rows_ok
        && session_ok
        && searches == vec![(vec![11], 1), (vec![41], 1)]
        && version == Some("3".into())
        && reopened_rows.len() == 3)
}

#[test]
fn marker3_refusal_keeps_committed_v2_and_public_retry_preserves_history() {
    bounded(
        "marker3_refusal_keeps_committed_v2_and_public_retry_preserves_history",
        || {
            let fixture = Fixture::new();
            fixture.seed(false);
            let source = observe(&fixture.db).expect("independent literal v1 observations");
            Connection::open(&fixture.db)
                .expect("injector connection")
                .execute_batch(FINAL_MARKER_ABORT)
                .expect("only final marker is refused");
            let attempt = Store::open(&fixture.db, &fixture.root);
            let refused = attempt.as_ref().err().is_some_and(final_refusal);
            let refusal_error = attempt.as_ref().err().map(ToString::to_string);
            drop(attempt);
            let committed_v2 = observe(&fixture.db);
            let v2_write = write_committed_v2(&fixture.db);
            let removal = Connection::open(&fixture.db)
                .and_then(|conn| {
                    conn.execute_batch(
            "DROP TRIGGER g15_refuse_final_insert; DROP TRIGGER g15_refuse_final_update;",
        )
                })
                .map_err(|error| error.to_string());
            let retry = if removal.is_ok() {
                public_retry(&fixture.db, &fixture.root)
            } else {
                Err("fixture trigger removal failed".into())
            };
            let after_retry = observe(&fixture.db);
            let cleaned = fixture.close();
            let controls = cleaned && literal_controls(&source, "1", 17) && refused;
            if controls {
                println!("G15_STEP_COMMIT_CONTROLS_OK");
            }
            let step_ok = committed_v2.as_ref().is_ok_and(|view| {
                marker(view) == Some("2")
                    && view.events == source.events
                    && preserved(view, &source)
                    && view.columns.iter().any(|column| column == "fields_json")
                    && view.fts_columns == vec!["message", "target", "fields_json"]
                    && view.obsolete == vec!["obsolete cache evidence"]
            });
            let final_ok = after_retry.as_ref().is_ok_and(|view| {
                marker(view) == Some("3")
                    && preserved(view, &source)
                    && view.obsolete.is_empty()
                    && view.events.len() == 3
                    && view.events[..2] == source.events
                    && view.events[2].0 == 41
                    && view.events[2].1 == 500
                    && view.events[2].4 == "committedbeta"
                    && view.events[2].6.as_deref() == Some(V2_FIELD)
            });
            // Candidate-only per-step conformance. A whole-chain rollback must fail
            // step_ok even if a later public retry eventually succeeds.
            assert!(controls && step_ok && v2_write == Ok(vec![41]) && removal.is_ok()
            && retry == Ok(true) && final_ok,
            "per-step commit/retry conformance failed\ncleanup={cleaned}, refusal={refusal_error:?}, source={source:?}, v2={committed_v2:?}, v2_write={v2_write:?}, removal={removal:?}, retry={retry:?}, final={after_retry:?}");
        },
    );
}

#[test]
fn current_v3_open_and_migrate_preserve_explicit_setting_timestamps() {
    bounded(
        "current_v3_open_and_migrate_preserve_explicit_setting_timestamps",
        || {
            let fixture = Fixture::new();
            fixture.seed(true);
            let source = observe(&fixture.db).expect("literal current v3 fixture observations");
            let opened = Store::open(&fixture.db, &fixture.root);
            let after_open = observe(&fixture.db);
            let migrated = opened
                .as_ref()
                .map_err(|error| error.to_string())
                .and_then(|store| store.migrate().map_err(|error| error.to_string()));
            let opened_ok = opened.is_ok();
            drop(opened);
            let after_migrate = observe(&fixture.db);
            let reopened = Store::open(&fixture.db, &fixture.root);
            let reopened_ok = reopened.is_ok();
            drop(reopened);
            let after_reopen = observe(&fixture.db);
            let cleaned = fixture.close();
            let controls = cleaned && literal_controls(&source, "3", 101);
            if controls {
                println!("G15_CURRENT_MARKER_CONTROLS_OK");
            }
            // An already-current owner must preserve supplied metadata on every open.
            assert!(controls && opened_ok && migrated.is_ok() && reopened_ok
            && after_open == Ok(source.clone())
            && after_migrate == Ok(source.clone())
            && after_reopen == Ok(source.clone()),
            "current marker/settings timestamps changed\ncleanup={cleaned}, open={opened_ok}, migrate={migrated:?}, reopen={reopened_ok}, source={source:?}, after_open={after_open:?}, after_migrate={after_migrate:?}, after_reopen={after_reopen:?}");
        },
    );
}

#[test]
fn future_v4_open_refuses_without_changing_logical_schema_data_or_marker() {
    bounded(
        "future_v4_open_refuses_without_changing_logical_schema_data_or_marker",
        || {
            let fixture = Fixture::new();
            fixture.seed(true);
            Connection::open(&fixture.db)
                .expect("literal future fixture connection")
                .execute_batch(
                    r"
UPDATE settings SET value_json='4',updated_ms=103 WHERE key='schema_version';
CREATE TABLE future_history(id INTEGER PRIMARY KEY,note TEXT NOT NULL);
INSERT INTO future_history VALUES(7,'opaque future evidence');
ALTER TABLE log_events ADD COLUMN future_extension TEXT;
UPDATE log_events SET future_extension='future row evidence' WHERE id=11;
",
                )
                .expect("independent additive future4 schema and explicit marker");
            let source = observe(&fixture.db).expect("literal future fixture observations");
            let attempt = Store::open(&fixture.db, &fixture.root);
            let refused = attempt.is_err();
            let error = attempt.as_ref().err().map(ToString::to_string);
            drop(attempt);
            let after = observe(&fixture.db);
            let cleaned = fixture.close();
            let controls = cleaned
                && literal_controls(&source, "4", 103)
                && source.future == vec![(7, "opaque future evidence".into())]
                && source.events[0].7.as_deref() == Some("future row evidence");
            if controls {
                println!("G15_FUTURE_MARKER_CONTROLS_OK");
            }
            // Logical SQL/data/marker refusal only; WAL setup precedes migration.
            assert!(controls && refused && after.as_ref().is_ok_and(|view| view == &source),
            "future4 was admitted or logically changed\ncleanup={cleaned}, refused={refused}, error={error:?}, source={source:?}, after={after:?}");
        },
    );
}
