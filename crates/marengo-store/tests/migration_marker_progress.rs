//! Public Store rollback and retry contract for a rewritten migration marker.
//! A literal supported-v2 fixture bounds a marker rewrite with a SQL tripwire.
#![allow(clippy::expect_used, clippy::panic)]

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use marengo_store::{LogEventInsert, Store, StoreError, StructuredLogQuery};
use rusqlite::{Connection, Row};

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
CREATE TABLE candump_frame_index(session_id TEXT,ordinal INTEGER,evidence TEXT);
INSERT INTO candump_frame_index VALUES('capture',1,'preserved obsolete cache evidence');
INSERT INTO settings VALUES('schema_version','2',17),('log_archive_days','7',23),
 ('log_disk_budget_bytes','123456',29),('operator_neighbor','keep',31);
INSERT INTO log_events VALUES(11,100,'info','historic','historicneedle','capture',
 '{"detail":"fieldneedle"}'),(29,300,'error','neighbor','neighborneedle',NULL,NULL);
INSERT INTO log_sessions VALUES('capture','literal session',41,42,NULL,NULL,NULL,2,19);
INSERT INTO config_overrides VALUES('neighbor_config','true',37,'operator');
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
INSERT INTO log_events_fts(log_events_fts) VALUES('rebuild');
CREATE TABLE g15_marker_attempts(attempts INTEGER NOT NULL CHECK(attempts>=0));
INSERT INTO g15_marker_attempts VALUES(0);
CREATE TRIGGER g15_rewrite_final_marker AFTER UPDATE OF value_json ON settings
WHEN NEW.key='schema_version' AND NEW.value_json='3' BEGIN
 UPDATE g15_marker_attempts SET attempts=attempts+1;
 SELECT CASE WHEN (SELECT attempts FROM g15_marker_attempts)>=2
 THEN RAISE(ABORT,'g15 bounded marker retry tripwire') END;
 UPDATE settings SET value_json='2',updated_ms=17 WHERE key='schema_version';
END;
"#;

fn bounded(name: &str, worker: fn()) {
    const ENV: &str = "MARENGO_G15_CONTRACT_WORKER";
    if std::env::var(ENV).ok().as_deref() == Some(name) {
        worker();
        return;
    }
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", name, "--nocapture"])
        .env(ENV, name)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("bounded marker progress child");
    let mut stdout = child.stdout.take().expect("child output pipe");
    let (finished, completion) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        let read = stdout.read_to_string(&mut output);
        let _ = finished.send((read, output));
    });
    match completion.recv_timeout(Duration::from_secs(15)) {
        Ok((read, output)) => {
            read.expect("child pipe read");
            let status = child.wait().expect("reap completed child");
            reader.join().expect("completed output reader");
            assert!(status.success(), "G15 progress child failed: {output}");
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            panic!("G15 progress child exceeded safety deadline: {error}");
        }
    }
}

type Setting = (String, String, i64);
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
type Config = (String, String, i64, String);

#[derive(Debug, PartialEq, Eq)]
struct View {
    schema: Vec<(String, String, String)>,
    settings: Vec<Setting>,
    events: Vec<Event>,
    sessions: Vec<Session>,
    config: Vec<Config>,
    cache: Vec<(String, i64, String)>,
    attempts: i64,
    history_fts: Vec<i64>,
    integrity: Vec<String>,
}

fn collect<T, F>(conn: &Connection, sql: &str, mapper: F) -> rusqlite::Result<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([], mapper)?;
    rows.collect()
}

fn observe(path: &Path) -> Result<View, String> {
    let result = (|| -> rusqlite::Result<View> {
        let conn = Connection::open(path)?;
        let schema = collect(
            &conn,
            "SELECT type,name,COALESCE(sql,'') FROM sqlite_schema ORDER BY type,name",
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let settings = collect(
            &conn,
            "SELECT key,value_json,updated_ms FROM settings ORDER BY key",
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let events = collect(&conn,
            "SELECT id,ts_ms,level,target,message,session_id,fields_json FROM log_events ORDER BY id",
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
        let sessions = collect(&conn,
            "SELECT id,label,started_ms,ended_ms,bench_blob,candump_blob,trace_blob,candump_frame_count,candump_bytes FROM log_sessions ORDER BY id",
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)))?;
        let config = collect(
            &conn,
            "SELECT key,value_json,updated_ms,source FROM config_overrides ORDER BY key",
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let has_cache = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='candump_frame_index')",
            [], |r| r.get::<_, bool>(0))?;
        let cache = if has_cache {
            collect(&conn,
                "SELECT session_id,ordinal,evidence FROM candump_frame_index ORDER BY session_id,ordinal",
                |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?
        } else {
            Vec::new()
        };
        let attempts =
            conn.query_row("SELECT attempts FROM g15_marker_attempts", [], |r| r.get(0))?;
        let history_fts = collect(&conn,
            "SELECT rowid FROM log_events_fts WHERE log_events_fts MATCH 'fieldneedle' ORDER BY rowid",
            |r| r.get(0))?;
        let integrity = collect(&conn, "PRAGMA integrity_check", |r| r.get(0))?;
        Ok(View {
            schema,
            settings,
            events,
            sessions,
            config,
            cache,
            attempts,
            history_fts,
            integrity,
        })
    })();
    result.map_err(|error| error.to_string())
}

fn validate_tripwire(path: &Path) -> Result<bool, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| e.to_string())?;
    let first = conn.execute(
        "UPDATE settings SET value_json='3',updated_ms=101 WHERE key='schema_version'",
        [],
    );
    let state = conn.query_row(
        "SELECT value_json,updated_ms,(SELECT attempts FROM g15_marker_attempts) FROM settings WHERE key='schema_version'",
        [], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?)));
    let second = conn.execute(
        "UPDATE settings SET value_json='3',updated_ms=103 WHERE key='schema_version'",
        [],
    );
    let tripwire = match &second {
        Err(rusqlite::Error::SqliteFailure(code, Some(message))) => {
            code.code == rusqlite::ErrorCode::ConstraintViolation
                && message == "g15 bounded marker retry tripwire"
        }
        _ => false,
    };
    let rollback = conn.execute_batch("ROLLBACK");
    Ok(first.is_ok()
        && matches!(state, Ok((ref marker,17,1)) if marker=="2")
        && tripwire
        && rollback.is_ok())
}

fn ordinary_settings(view: &View) -> Vec<Setting> {
    view.settings
        .iter()
        .filter(|row| row.0 != "schema_version")
        .cloned()
        .collect()
}

fn literal_controls(view: &View) -> bool {
    view.settings
        == vec![
            ("log_archive_days".into(), "7".into(), 23),
            ("log_disk_budget_bytes".into(), "123456".into(), 29),
            ("operator_neighbor".into(), "keep".into(), 31),
            ("schema_version".into(), "2".into(), 17),
        ]
        && view.events.len() == 2
        && view.events[0].0 == 11
        && view.events[1].0 == 29
        && view.sessions.len() == 1
        && view.sessions[0].0 == "capture"
        && view.config
            == vec![(
                "neighbor_config".into(),
                "true".into(),
                37,
                "operator".into(),
            )]
        && view.cache
            == vec![(
                "capture".into(),
                1,
                "preserved obsolete cache evidence".into(),
            )]
        && view.attempts == 0
        && view.history_fts == vec![11]
        && view.integrity == vec!["ok"]
}

type Search = (Vec<(i64, String)>, u32);

fn public_search(store: &Store, token: &str) -> Result<Search, String> {
    store
        .query_structured_logs(&StructuredLogQuery {
            from_ms: None,
            to_ms: None,
            level: None,
            target: None,
            session_id: None,
            q: Some(token.into()),
            offset: 0,
            limit: 20,
        })
        .map(|(rows, total)| {
            (
                rows.into_iter().map(|row| (row.id, row.message)).collect(),
                total,
            )
        })
        .map_err(|error| error.to_string())
}

#[derive(Debug)]
struct Retry {
    view: View,
    historic: Search,
    fresh: Search,
}

fn public_retry(db: &Path, root: &Path) -> Result<Retry, String> {
    let store = Store::open(db, root).map_err(|e| e.to_string())?;
    store
        .insert_log_events(&[LogEventInsert {
            ts_ms: 501,
            level: "info".into(),
            target: "public_retry".into(),
            message: "retryneedle".into(),
            session_id: Some("capture".into()),
            fields_json: Some(r#"{"detail":"freshneedle"}"#.into()),
        }])
        .map_err(|e| e.to_string())?;
    drop(store);
    let reopened = Store::open(db, root).map_err(|e| e.to_string())?;
    reopened.migrate().map_err(|e| e.to_string())?;
    let historic = public_search(&reopened, "fieldneedle")?;
    let fresh = public_search(&reopened, "freshneedle")?;
    drop(reopened);
    Ok(Retry {
        view: observe(db)?,
        historic,
        fresh,
    })
}

fn retry_preserved(retry: &Retry, before: &View, refused_state: &View) -> bool {
    retry
        .view
        .settings
        .iter()
        .any(|row| row.0 == "schema_version" && row.1 == "3")
        && ordinary_settings(&retry.view) == ordinary_settings(before)
        && retry.view.sessions == before.sessions
        && retry.view.config == before.config
        && retry.view.events.len() == 3
        && retry.view.events[..2] == before.events
        && retry.view.events[2]
            == (
                30,
                501,
                "info".into(),
                "public_retry".into(),
                "retryneedle".into(),
                Some("capture".into()),
                Some(r#"{"detail":"freshneedle"}"#.into()),
            )
        && retry.view.cache.is_empty()
        && retry.view.attempts == refused_state.attempts
        && retry.view.history_fts == vec![11]
        && retry.view.integrity == vec!["ok"]
        && retry.historic == (vec![(11, "historicneedle".into())], 1)
        && retry.fresh == (vec![(30, "retryneedle".into())], 1)
}

#[test]
fn rewritten_final_marker_refuses_before_committing_and_retries_after_trigger_removal() {
    bounded(
        "rewritten_final_marker_refuses_before_committing_and_retries_after_trigger_removal",
        || {
            let directory = tempfile::Builder::new()
                .prefix("marengo-g15-progress-")
                .tempdir()
                .expect("exclusive runner-configured fixture");
            let root = directory.path().to_path_buf();
            assert!(root.is_absolute(), "fixture paths must be absolute");
            let db = root.join("supported-v2.sqlite");
            let conn = Connection::open(&db).expect("independent fixture connection");
            conn.execute_batch(FIXTURE)
                .expect("independent literal supported-v2 fixture");
            drop(conn);
            let initial = observe(&db);
            let tripwire_control = validate_tripwire(&db);
            let before = observe(&db);
            let attempt = if tripwire_control == Ok(true) {
                Store::open(&db, &root)
            } else {
                Err(StoreError::msg(
                    "fixture SQL tripwire control failed; public open not attempted",
                ))
            };
            let refused = matches!(&attempt, Err(StoreError::Message(message))
            if message.contains("schema version") && message.contains("did not advance"));
            let refusal_error = match &attempt {
                Ok(_) => "unexpected successful open".to_string(),
                Err(error) => format!("{error:?}"),
            };
            drop(attempt);
            let after = observe(&db);
            let removal = (|| -> rusqlite::Result<()> {
                let conn = Connection::open(&db)?;
                conn.execute_batch("DROP TRIGGER g15_rewrite_final_marker")
            })()
            .map_err(|error| error.to_string());
            let retry = public_retry(&db, &root);
            let cleanup = directory.close().map_err(|error| error.to_string());
            let absent = root
                .try_exists()
                .map(|exists| !exists)
                .map_err(|error| error.to_string());
            let cleaned = cleanup.is_ok() && absent == Ok(true);
            let controls = cleaned
                && tripwire_control == Ok(true)
                && initial == before
                && before.as_ref().is_ok_and(literal_controls);
            let rollback_ok = refused && before.is_ok() && after == before;
            let retry_ok = match (&retry, &before, &after) {
                (Ok(retry), Ok(before), Ok(after)) => {
                    removal.is_ok() && retry_preserved(retry, before, after)
                }
                _ => false,
            };
            if controls {
                println!("G15_MARKER_PROGRESS_CONTROLS_OK");
            }
            assert!(controls && rollback_ok && retry_ok,
            "marker-progress refusal/rollback/retry failed\ncleanup={cleanup:?}; absent={absent:?}; controls={controls}; rollback_ok={rollback_ok}; retry_ok={retry_ok}; error={refusal_error}; tripwire={tripwire_control:?}; initial={initial:?}; before={before:?}; after={after:?}; removal={removal:?}; retry={retry:?}");
        },
    );
}
