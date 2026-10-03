//! G15: real migration-marker failures preserve the prior schema and permit retry.
//! Independently authored historic SQLite/FTS/data fixtures exercise public Store APIs.
//! Fixed-cutoff purge and disposable absolute artifact references keep fixtures local.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::bounded;
use std::path::{Path, PathBuf};

use marengo_store::{LogEventInsert, Store, StoreError, StructuredLogQuery};
use rusqlite::{Connection, OptionalExtension};

// Literal historical contracts: do not import production migration constants.
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

const MARKER_ABORT: &str = r"
CREATE TRIGGER g15_refuse_marker_insert BEFORE INSERT ON settings
WHEN NEW.key='schema_version' AND NEW.value_json <>
  COALESCE((SELECT value_json FROM settings WHERE key='schema_version'),'')
BEGIN SELECT RAISE(ABORT,'g15 marker refusal'); END;
CREATE TRIGGER g15_refuse_marker_update BEFORE UPDATE OF value_json ON settings
WHEN NEW.key='schema_version' AND NEW.value_json <> OLD.value_json
BEGIN SELECT RAISE(ABORT,'g15 marker refusal'); END;
";

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("marengo-g15-")
            .tempdir()
            .expect("exclusive runner-configured temporary directory");
        let root = directory.path().to_path_buf();
        assert!(
            root.is_absolute(),
            "fixture references must remain absolute and local"
        );
        let db = root.join("historic.sqlite");
        Self {
            directory,
            root,
            db,
        }
    }

    fn seed(&self, version: i64) {
        let conn = Connection::open(&self.db).expect("independent fixture connection");
        conn.execute_batch(BASE)
            .expect("literal v1 schema and independent rows");
        let bench = self.root.join("archive").join("bench.log");
        let trace = self.root.join("archive").join("trace.csv");
        conn.execute(
            "UPDATE log_sessions SET bench_blob=?1,trace_blob=?2 WHERE id='capture'",
            rusqlite::params![
                bench.to_string_lossy().as_ref(),
                trace.to_string_lossy().as_ref()
            ],
        )
        .expect("fixture-absolute references; public purge cannot leave fixture root");
        if version == 2 {
            for statement in V2_STEPS {
                conn.execute_batch(statement)
                    .expect("literal fixture transition");
            }
            conn.execute(
                "UPDATE settings SET value_json='2' WHERE key='schema_version'",
                [],
            )
            .expect("literal prior version marker");
        }
    }

    fn open(&self) -> Result<Store, StoreError> {
        Store::open(&self.db, &self.root)
    }

    fn close(self) -> bool {
        self.directory
            .close()
            .expect("actual SQLite fixture cleanup");
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

#[derive(Debug, PartialEq, Eq)]
struct Raw {
    schema: Vec<(String, String, String)>,
    settings: Vec<(String, String, i64)>,
    events: Vec<Event>,
    sessions: Vec<Session>,
    overrides: Vec<(String, String, i64, String)>,
    obsolete: Option<String>,
    integrity: Vec<String>,
    alpha_hits: Result<Vec<i64>, String>,
}

fn raw_fts(conn: &Connection, token: &str) -> Result<Vec<i64>, String> {
    let result = (|| -> rusqlite::Result<Vec<i64>> {
        let mut statement = conn.prepare(
            "SELECT rowid FROM log_events_fts WHERE log_events_fts MATCH ?1 ORDER BY rowid",
        )?;
        let rows = statement.query_map([token], |row| row.get(0))?;
        rows.collect()
    })();
    result.map_err(|error| error.to_string())
}

fn raw(path: &Path) -> Raw {
    let conn = Connection::open(path).expect("independent observation connection");
    let mut schema_stmt = conn.prepare(
        "SELECT type,name,COALESCE(sql,'') FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
    ).expect("schema observation");
    let schema = schema_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("schema rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("schema values");
    let has_fields: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('log_events') WHERE name='fields_json')",
            [],
            |row| row.get(0),
        )
        .expect("actual column observation");
    let fields = if has_fields { "fields_json" } else { "NULL" };
    let mut events_stmt = conn
        .prepare(&format!(
            "SELECT id,ts_ms,level,target,message,session_id,{fields} FROM log_events ORDER BY id",
        ))
        .expect("row observation");
    let events = events_stmt
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
        })
        .expect("event rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("event values");
    let mut settings_stmt = conn
        .prepare("SELECT key,value_json,updated_ms FROM settings ORDER BY key")
        .expect("settings observation");
    let settings = settings_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("settings rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("settings values");
    let mut session_stmt = conn.prepare(
        "SELECT id,label,started_ms,ended_ms,bench_blob,candump_blob,trace_blob,candump_frame_count,candump_bytes FROM log_sessions ORDER BY id",
    ).expect("session observation");
    let sessions = session_stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .expect("session rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("session values");
    let mut override_stmt = conn
        .prepare("SELECT key,value_json,updated_ms,source FROM config_overrides ORDER BY key")
        .expect("override observation");
    let overrides = override_stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("override rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("override values");
    let has_obsolete: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='candump_frame_index' AND type='table')",
        [], |row| row.get(0),
    ).expect("obsolete table observation");
    let obsolete = if has_obsolete {
        conn.query_row("SELECT evidence FROM candump_frame_index", [], |row| {
            row.get(0)
        })
        .optional()
        .expect("obsolete evidence observation")
    } else {
        None
    };
    let mut integrity_stmt = conn
        .prepare("PRAGMA integrity_check")
        .expect("integrity observation");
    let integrity = integrity_stmt
        .query_map([], |row| row.get(0))
        .expect("integrity rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("integrity values");
    Raw {
        schema,
        settings,
        events,
        sessions,
        overrides,
        obsolete,
        integrity,
        alpha_hits: raw_fts(&conn, "archivedalpha"),
    }
}

fn search(store: &Store, token: &str) -> Result<(Vec<(i64, String)>, u32), String> {
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
        .map_err(|error| format!("public final-schema search: {error}"))?;
    Ok((
        rows.into_iter().map(|row| (row.id, row.message)).collect(),
        total,
    ))
}

#[derive(Debug)]
struct Usable {
    initial_rows: bool,
    initial_search: bool,
    metadata: bool,
    settings: bool,
    inserted_fields: bool,
    updated_index: bool,
    purge: bool,
    clean_index: bool,
}

fn exercise(store: &Store, root: &Path) -> Result<Usable, String> {
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
    let rows = store
        .recent_log_events(10)
        .map_err(|error| format!("preserved public rows: {error}"))?;
    let initial_rows = rows.len() == 2
        && rows[0].id == 29
        && rows[0].ts_ms == 300
        && rows[0].message == "neighboromega"
        && rows[1].id == 11
        && rows[1].ts_ms == 100
        && rows[1].message == "archivedalpha"
        && rows[1].session_id.as_deref() == Some("capture")
        && rows[1].fields_json.is_none();
    let initial_search = search(store, "archivedalpha")? == (vec![(11, "archivedalpha".into())], 1);
    let session = store
        .get_session("capture")
        .map_err(|error| format!("preserved session: {error}"))?;
    let metadata = session.is_some_and(|row| {
        row.label.as_deref() == Some("authoritative label")
            && row.started_ms == 41
            && row.ended_ms == Some(42)
            && row.bench_blob.as_deref() == Some(bench.as_str())
            && row.candump_blob.is_none()
            && row.trace_blob.as_deref() == Some(trace.as_str())
    });
    let settings = store
        .get_setting("schema_version")
        .map_err(|error| error.to_string())?
        == Some("3".into())
        && store
            .get_setting("log_archive_days")
            .map_err(|error| error.to_string())?
            == Some("7".into())
        && store
            .get_setting("log_disk_budget_bytes")
            .map_err(|error| error.to_string())?
            == Some("123456".into())
        && store
            .get_setting("operator_neighbor")
            .map_err(|error| error.to_string())?
            == Some("keep".into())
        && store
            .connection()
            .query_row(
                "SELECT value_json FROM config_overrides WHERE key='neighbor_config'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("independent override readback: {error}"))?
            == "true";
    store
        .insert_log_events(&[LogEventInsert {
            ts_ms: 500,
            level: "warn".into(),
            target: "runtime".into(),
            message: "livebeta".into(),
            session_id: Some("capture".into()),
            fields_json: Some(r#"{"detail":"insertneedle"}"#.into()),
        }])
        .map_err(|error| format!("final-schema insertion: {error}"))?;
    let inserted = search(store, "insertneedle")?;
    let inserted_fields = inserted.1 == 1 && inserted.0 == vec![(30, "livebeta".into())];
    store
        .connection()
        .execute(
            "UPDATE log_events SET message='changedomega',fields_json=?1 WHERE id=29",
            [r#"{"detail":"updatedneedle"}"#],
        )
        .map_err(|error| format!("actual update trigger operation: {error}"))?;
    let updated_index = search(store, "neighboromega")? == (Vec::new(), 0)
        && search(store, "changedomega")? == (vec![(29, "changedomega".into())], 1)
        && search(store, "updatedneedle")? == (vec![(29, "changedomega".into())], 1);
    // Independent literal cutoff: all three logs and the session precede 501.
    // Stored artifact references are absolute paths within the disposable fixture.
    let purge = store
        .purge_before(501)
        .map_err(|error| format!("public fixed-cutoff purge: {error}"))?
        == (3, 1)
        && store
            .recent_log_events(10)
            .map_err(|error| error.to_string())?
            .is_empty();
    let clean_index = search(store, "archivedalpha")? == (Vec::new(), 0)
        && search(store, "insertneedle")? == (Vec::new(), 0);
    Ok(Usable {
        initial_rows,
        initial_search,
        metadata,
        settings,
        inserted_fields,
        updated_index,
        purge,
        clean_index,
    })
}

fn usable(observation: &Usable) -> bool {
    observation.initial_rows
        && observation.initial_search
        && observation.metadata
        && observation.settings
        && observation.inserted_fields
        && observation.updated_index
        && observation.purge
        && observation.clean_index
}

fn constraint(error: &StoreError) -> bool {
    matches!(error, StoreError::Sqlite(rusqlite::Error::SqliteFailure(code, Some(message)))
        if code.code == rusqlite::ErrorCode::ConstraintViolation && message.contains("g15 marker refusal"))
}

fn remove_marker_abort(db: &Path) {
    Connection::open(db)
        .expect("remove only fixture injection")
        .execute_batch(
            "DROP TRIGGER g15_refuse_marker_insert; DROP TRIGGER g15_refuse_marker_update;",
        )
        .expect("actual retry guard removal");
}

fn retry(fixture: &Fixture) -> Result<Usable, String> {
    // Every public retry operation returns collected behavior. In particular the
    // original duplicate-column open refusal cannot panic before the rollback
    // observation/cleanup/control marker/decisive oracle have been classified.
    let store = fixture
        .open()
        .map_err(|error| format!("writable retry open: {error}"))?;
    let observed = exercise(&store, &fixture.root)?;
    drop(store);
    let reopened = fixture
        .open()
        .map_err(|error| format!("writable retry reopen: {error}"))?;
    let version = reopened
        .get_setting("schema_version")
        .map_err(|error| error.to_string())?;
    let rows = reopened
        .recent_log_events(10)
        .map_err(|error| error.to_string())?;
    if version != Some("3".into()) || !rows.is_empty() {
        return Err("successful retry did not reopen as coherent final schema".into());
    }
    Ok(observed)
}

#[test]
fn independently_seeded_v1_and_v2_upgrade_preserve_data_and_working_fts() {
    bounded(
        "independently_seeded_v1_and_v2_upgrade_preserve_data_and_working_fts",
        || {
            let mut observations = Vec::new();
            for version in [1, 2] {
                let fixture = Fixture::new();
                fixture.seed(version);
                let before = raw(&fixture.db);
                let observed = retry(&fixture);
                let after = raw(&fixture.db);
                let cleaned = fixture.close();
                observations.push((version, before, observed, after, cleaned));
            }
            for (version, before, observed, after, cleaned) in observations {
                assert!(cleaned, "actual cleanup for prior v{version}");
                assert_eq!(
                    before.alpha_hits,
                    Ok(vec![11]),
                    "independent prior FTS control"
                );
                assert_eq!(
                    before.integrity,
                    vec!["ok"],
                    "independent prior integrity control"
                );
                assert!(
                    observed.as_ref().is_ok_and(usable),
                    "public upgrade control v{version}: {observed:?}"
                );
                assert!(after.obsolete.is_none(), "only obsolete cache is removed");
                assert_eq!(after.integrity, vec!["ok"]);
            }
        },
    );
}

fn rollback_case(version: i64) {
    let fixture = Fixture::new();
    fixture.seed(version);
    Connection::open(&fixture.db)
        .expect("fixture marker injector")
        .execute_batch(MARKER_ABORT)
        .expect("real SQLite marker-abort triggers");
    let before = raw(&fixture.db);
    let attempt = fixture.open();
    let refused = attempt.as_ref().err().is_some_and(constraint);
    let reported = attempt.as_ref().err().map(ToString::to_string);
    drop(attempt);
    let after_failure = raw(&fixture.db);
    remove_marker_abort(&fixture.db);
    let retried = retry(&fixture);
    let final_state = raw(&fixture.db);
    let cleaned = fixture.close();
    assert!(cleaned, "all SQLite handles close before fixture cleanup");
    assert_eq!(
        before.alpha_hits,
        Ok(vec![11]),
        "reachable pre-migration FTS control"
    );
    assert_eq!(before.obsolete.as_deref(), Some("obsolete cache evidence"));
    assert!(
        refused,
        "real typed marker refusal must be reached: {reported:?}"
    );
    println!("G15_ATOMICITY_CONTROLS_OK");
    assert_eq!(
        after_failure, before,
        "v{version} DDL/data/index/version must roll back together"
    );
    assert!(
        retried.as_ref().is_ok_and(usable),
        "actual writable retry/reopen: {retried:?}"
    );
    assert_eq!(final_state.integrity, vec!["ok"]);
    assert!(final_state.obsolete.is_none());
}

#[test]
fn v2_marker_failure_rolls_back_completed_ddl_and_allows_real_retry() {
    bounded(
        "v2_marker_failure_rolls_back_completed_ddl_and_allows_real_retry",
        || rollback_case(1),
    );
}

#[test]
fn v3_marker_failure_preserves_prior_schema_and_obsolete_cache_until_retry() {
    bounded(
        "v3_marker_failure_preserves_prior_schema_and_obsolete_cache_until_retry",
        || rollback_case(2),
    );
}
