use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::error::{Result, StoreError};

pub const SCHEMA_VERSION: i64 = 3;

pub const MIGRATION_001: &str = r"
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS log_events (
  id INTEGER PRIMARY KEY,
  ts_ms INTEGER NOT NULL,
  level TEXT NOT NULL,
  target TEXT NOT NULL,
  message TEXT NOT NULL,
  session_id TEXT
);
CREATE INDEX IF NOT EXISTS log_events_ts ON log_events(ts_ms);
CREATE INDEX IF NOT EXISTS log_events_level ON log_events(level);
CREATE INDEX IF NOT EXISTS log_events_target ON log_events(target);
CREATE INDEX IF NOT EXISTS log_events_session ON log_events(session_id);

CREATE VIRTUAL TABLE IF NOT EXISTS log_events_fts USING fts5(
  message,
  target,
  content='log_events',
  content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS log_events_ai AFTER INSERT ON log_events BEGIN
  INSERT INTO log_events_fts(rowid) VALUES (new.id);
END;
CREATE TRIGGER IF NOT EXISTS log_events_ad AFTER DELETE ON log_events BEGIN
  INSERT INTO log_events_fts(log_events_fts, rowid) VALUES('delete', old.id);
END;
CREATE TRIGGER IF NOT EXISTS log_events_au AFTER UPDATE ON log_events BEGIN
  INSERT INTO log_events_fts(log_events_fts, rowid) VALUES('delete', old.id);
  INSERT INTO log_events_fts(rowid) VALUES (new.id);
END;

CREATE TABLE IF NOT EXISTS log_sessions (
  id TEXT PRIMARY KEY,
  label TEXT,
  started_ms INTEGER NOT NULL,
  ended_ms INTEGER,
  bench_blob TEXT,
  candump_blob TEXT,
  trace_blob TEXT,
  candump_frame_count INTEGER,
  candump_bytes INTEGER
);
CREATE INDEX IF NOT EXISTS log_sessions_started ON log_sessions(started_ms);

CREATE TABLE IF NOT EXISTS config_overrides (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_ms INTEGER NOT NULL,
  source TEXT NOT NULL
);
";

pub const MIGRATION_002: &str = r"
ALTER TABLE log_events ADD COLUMN fields_json TEXT;

DROP TRIGGER IF EXISTS log_events_ai;
DROP TRIGGER IF EXISTS log_events_ad;
DROP TRIGGER IF EXISTS log_events_au;
DROP TABLE IF EXISTS log_events_fts;

CREATE VIRTUAL TABLE log_events_fts USING fts5(
  message,
  target,
  fields_json,
  content='log_events',
  content_rowid='id'
);

CREATE TRIGGER log_events_ai AFTER INSERT ON log_events BEGIN
  INSERT INTO log_events_fts(rowid, message, target, fields_json)
  VALUES (new.id, new.message, new.target, COALESCE(new.fields_json, ''));
END;
CREATE TRIGGER log_events_ad AFTER DELETE ON log_events BEGIN
  INSERT INTO log_events_fts(log_events_fts, rowid, message, target, fields_json)
  VALUES('delete', old.id, old.message, old.target, COALESCE(old.fields_json, ''));
END;
CREATE TRIGGER log_events_au AFTER UPDATE ON log_events BEGIN
  INSERT INTO log_events_fts(log_events_fts, rowid, message, target, fields_json)
  VALUES('delete', old.id, old.message, old.target, COALESCE(old.fields_json, ''));
  INSERT INTO log_events_fts(rowid, message, target, fields_json)
  VALUES (new.id, new.message, new.target, COALESCE(new.fields_json, ''));
END;

INSERT INTO log_events_fts(log_events_fts) VALUES('rebuild');
";

pub const MIGRATION_003: &str = r"
DROP TABLE IF EXISTS candump_frame_index;
";

pub(crate) fn migrate(conn: &mut Connection, now_ms: u64) -> Result<()> {
    let journal_mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if journal_mode.eq_ignore_ascii_case("off") {
        return Err(StoreError::msg(
            "migration refused: SQLite rollback journaling is disabled; restore a journal before retrying",
        ));
    }
    let now = i64::try_from(now_ms)
        .map_err(|_| StoreError::msg("migration timestamp exceeds SQLite milliseconds range"))?;
    loop {
        // Read only after acquiring the writer reservation. Other Store instances
        // have their own mutexes and may have completed a step while we waited.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = schema_version(&tx)?;
        match version {
            0 => tx.execute_batch(MIGRATION_001)?,
            1 => {
                let fields_present: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('log_events') WHERE name='fields_json')",
                    [],
                    |row| row.get(0),
                )?;
                if fields_present {
                    return Err(StoreError::msg(
                        "historic partial v2 schema requires backed-up recovery; no migration applied",
                    ));
                }
                tx.execute_batch(MIGRATION_002)?;
            }
            2 => tx.execute_batch(MIGRATION_003)?,
            SCHEMA_VERSION => {
                for (key, value) in [
                    (
                        "log_archive_days",
                        crate::paths::DEFAULT_ARCHIVE_DAYS.to_string(),
                    ),
                    (
                        "log_disk_budget_bytes",
                        crate::paths::DEFAULT_LOG_DISK_BUDGET_BYTES.to_string(),
                    ),
                ] {
                    tx.execute(
                        "INSERT INTO settings(key, value_json, updated_ms)
                         SELECT ?1, ?2, ?3 WHERE NOT EXISTS(SELECT 1 FROM settings WHERE key=?1)
                         ON CONFLICT(key) DO NOTHING",
                        params![key, value, now],
                    )?;
                }
                tx.commit()?;
                return Ok(());
            }
            _ => return Err(StoreError::msg("unsupported store schema version")),
        }
        let next_version = version + 1;
        tx.execute(
            "INSERT INTO settings(key, value_json, updated_ms) VALUES ('schema_version', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_ms=excluded.updated_ms",
            params![next_version.to_string(), now],
        )?;
        tx.commit()?;
    }
}

fn schema_version(conn: &Connection) -> Result<i64> {
    let has_settings: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='settings')",
        [],
        |row| row.get(0),
    )?;
    let marker: Option<String> = if has_settings {
        conn.query_row(
            "SELECT value_json FROM settings WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?
    } else {
        None
    };
    if let Some(marker) = marker {
        let version = marker.parse::<i64>().map_err(|_| {
            StoreError::msg(format!(
                "invalid store schema version {marker:?}; preserve this database for recovery"
            ))
        })?;
        if !(1..=SCHEMA_VERSION).contains(&version) {
            return Err(StoreError::msg(format!(
                "unsupported store schema version {marker:?}; this binary supports versions 1 through {SCHEMA_VERSION}"
            )));
        }
        return Ok(version);
    }
    let nonempty: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*')",
        [],
        |row| row.get(0),
    )?;
    if nonempty {
        return Err(StoreError::msg(
            "nonempty store has no schema version; backed-up recovery is required before migration",
        ));
    }
    Ok(0)
}
