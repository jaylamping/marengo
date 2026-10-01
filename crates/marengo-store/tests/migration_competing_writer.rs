//! Qualify marker reread after real WAL writer contention through public Store::migrate.
#![allow(clippy::expect_used, clippy::panic)]

use marengo_store::Store;
use rusqlite::{types::Value, Connection};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const WORKER: &str = "MARENGO_G15_WRITER_WORKER";
const TEST: &str = "migration_rereads_marker_after_competing_writer_commits";
const WAIT: Duration = Duration::from_secs(15);
static CALLBACKS: AtomicUsize = AtomicUsize::new(0);
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
INSERT INTO settings VALUES('schema_version','3',17),('log_archive_days','7',23),
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
"#;

fn message(text: &str) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "G15:{text}").expect("worker protocol write");
    out.flush().expect("worker protocol flush");
}

fn command(expected: &str) {
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .expect("worker protocol read");
    assert_eq!(line.trim(), expected, "worker protocol command");
}

fn busy(_: i32) -> bool {
    // SQLite resets its argument for each locking event; count across the child.
    if CALLBACKS.fetch_add(1, Ordering::SeqCst) != 0 {
        return false;
    }
    // No SQLite access while inside SQLite's callback.
    message("BUSY");
    command("COMMITTED");
    true
}

fn worker(db: &Path) {
    let store = Store::open(db, db.parent().expect("fixture parent")).expect("worker open");
    {
        let conn = store.connection();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .expect("WAL");
        assert_eq!(mode, "wal");
        assert!(conn.is_autocommit(), "idle worker has no read transaction");
        conn.busy_handler(Some(busy))
            .expect("install caller busy policy");
    }
    message("READY");
    command("MIGRATE");
    let result = store.migrate().map_err(|e| e.to_string());
    let calls = CALLBACKS.load(Ordering::SeqCst);
    drop(store);
    message(&format!("RESULT:{calls}:{result:?}"));
}

struct OwnedWorker {
    child: Child,
    input: Option<ChildStdin>,
    lines: mpsc::Receiver<String>,
    reader: Option<std::thread::JoinHandle<()>>,
    reaped: bool,
}

impl OwnedWorker {
    fn start(db: &Path) -> Self {
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", TEST, "--nocapture"])
            .env(WORKER, db)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("owned worker");
        let input = child.stdin.take();
        let output = child.stdout.take().expect("worker stdout");
        let (tx, lines) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if let Some(protocol) = line.strip_prefix("G15:") {
                    if tx.send(protocol.to_owned()).is_err() {
                        break;
                    }
                }
            }
        });
        Self {
            child,
            input,
            lines,
            reader: Some(reader),
            reaped: false,
        }
    }

    fn receive(&self) -> Result<String, String> {
        self.lines
            .recv_timeout(WAIT)
            .map_err(|e| format!("protocol containment failure: {e}"))
    }

    fn send(&mut self, text: &str) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("closed worker input")?;
        writeln!(input, "{text}")
            .and_then(|()| input.flush())
            .map_err(|e| e.to_string())
    }

    fn finish(&mut self) -> Result<(), String> {
        self.input.take();
        // Wait for EOF with a bounded channel wait before wait()/join().
        match self.lines.recv_timeout(WAIT) {
            Err(mpsc::RecvTimeoutError::Disconnected) => (),
            other => return Err(format!("unexpected worker completion: {other:?}")),
        }
        let deadline = Instant::now() + WAIT;
        let status = loop {
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if Instant::now() >= deadline {
                return Err("worker exit containment deadline".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        self.reaped = true;
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "reader panicked")?;
        }
        if status.success() {
            Ok(())
        } else {
            Err(format!("worker failed: {status}"))
        }
    }
}

impl Drop for OwnedWorker {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.input.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn rows(conn: &Connection, sql: &str) -> rusqlite::Result<Vec<Vec<Value>>> {
    let mut statement = conn.prepare(sql)?;
    let columns = statement.column_count();
    let mapped = statement.query_map([], |row| (0..columns).map(|i| row.get(i)).collect())?;
    mapped.collect()
}

fn history(conn: &Connection) -> rusqlite::Result<Vec<Vec<Vec<Value>>>> {
    [
        "SELECT * FROM settings WHERE key!='schema_version' ORDER BY key",
        "SELECT * FROM log_events ORDER BY id",
        "SELECT * FROM log_sessions ORDER BY id",
        "SELECT * FROM config_overrides ORDER BY key",
    ]
    .into_iter()
    .map(|sql| rows(conn, sql))
    .collect()
}

fn marker(conn: &Connection) -> rusqlite::Result<(String, i64)> {
    conn.query_row(
        "SELECT value_json,updated_ms FROM settings WHERE key='schema_version'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

fn cache(conn: &Connection) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='candump_frame_index')",
        [],
        |r| r.get(0),
    )
}

#[derive(Debug)]
struct Observation {
    committed: bool,
    result: String,
    final_marker: (String, i64),
    preserved: bool,
}

impl Observation {
    fn accepted(&self) -> bool {
        self.committed
            && self.result == "RESULT:1:Ok(())"
            && self.final_marker == ("3".into(), 777)
            && self.preserved
    }
}

fn qualification(db: &Path) -> Result<Observation, String> {
    let run = || -> Result<Observation, Box<dyn std::error::Error>> {
        let mut conn = Connection::open(db)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(FIXTURE)?;
        let before = history(&conn)?;
        let mut child = OwnedWorker::start(db);
        if child.receive()? != "READY" {
            return Err("missing idle worker readiness".into());
        }
        let mode: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        if mode != "wal" {
            return Err("fixture is not WAL".into());
        }
        conn.execute_batch(
            "BEGIN IMMEDIATE;
            CREATE TABLE candump_frame_index(session_id TEXT,ordinal INTEGER,evidence TEXT);
            INSERT INTO candump_frame_index VALUES('capture',1,'obsolete');
            UPDATE settings SET value_json='2',updated_ms=17 WHERE key='schema_version'; COMMIT;",
        )?;
        let observer = Connection::open(db)?;
        if marker(&observer)? != ("2".into(), 17)
            || !cache(&observer)?
            || history(&observer)? != before
        {
            return Err("incoherent v2 fixture".into());
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "DROP TABLE candump_frame_index;
            UPDATE settings SET value_json='3',updated_ms=777 WHERE key='schema_version';",
        )?;
        child.send("MIGRATE")?;
        if child.receive()? != "BUSY" {
            return Err("no actual reservation contention".into());
        }
        tx.commit()?;
        // Independent autocommit reader sees the writer commit before acknowledgement.
        let committed = marker(&observer)? == ("3".into(), 777)
            && !cache(&observer)?
            && history(&observer)? == before;
        child.send("COMMITTED")?;
        let result = child.receive()?;
        child.finish()?;
        drop(child);
        drop(observer);
        drop(conn);
        let reopened = Store::open(db, db.parent().ok_or("fixture parent")?)?;
        let conn = reopened.connection();
        let final_marker = marker(&conn)?;
        let preserved = !cache(&conn)?
            && history(&conn)? == before
            && rows(&conn, "SELECT rowid FROM log_events_fts WHERE log_events_fts MATCH 'fieldneedle' ORDER BY rowid")?
                == vec![vec![Value::Integer(11)]]
            && rows(&conn, "SELECT rowid FROM log_events_fts WHERE log_events_fts MATCH 'historicneedle' ORDER BY rowid")?
                == vec![vec![Value::Integer(11)]]
            && rows(&conn, "PRAGMA integrity_check")? == vec![vec![Value::Text("ok".into())]];
        conn.execute_batch(
            "INSERT INTO log_events_fts(log_events_fts,rank) VALUES('integrity-check',1)",
        )?;
        Ok(Observation {
            committed,
            result,
            final_marker,
            preserved,
        })
    };
    run().map_err(|e| e.to_string())
}

#[test]
fn migration_rereads_marker_after_competing_writer_commits() {
    if let Some(db) = std::env::var_os(WORKER) {
        worker(Path::new(&db));
        return;
    }
    let directory = tempfile::Builder::new()
        .prefix("marengo-g15-writer-")
        .tempdir()
        .expect("owned fixture");
    let root = directory.path().to_path_buf();
    let outcome = qualification(&root.join("store.sqlite"));
    let cleanup = directory.close();
    let absent = root.try_exists().map(|exists| !exists);
    println!("G15_WRITER_OBSERVATION: {outcome:?}; cleanup={cleanup:?}; absent={absent:?}");
    assert!(outcome.as_ref().is_ok_and(Observation::accepted) && cleanup.is_ok() && matches!(absent, Ok(true)),
        "competing-writer qualification: outcome={outcome:?}, cleanup={cleanup:?}, absent={absent:?}");
}
