pub const SCHEMA_VERSION: i64 = 1;

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

CREATE TABLE IF NOT EXISTS candump_frame_index (
  session_id TEXT NOT NULL,
  line_no INTEGER NOT NULL,
  byte_offset INTEGER NOT NULL,
  PRIMARY KEY (session_id, line_no)
);

CREATE TABLE IF NOT EXISTS config_overrides (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_ms INTEGER NOT NULL,
  source TEXT NOT NULL
);
";
