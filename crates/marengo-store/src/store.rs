use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;
use time::OffsetDateTime;

use crate::error::{Result, StoreError};
use crate::migrations::{MIGRATION_001, MIGRATION_002, MIGRATION_003, SCHEMA_VERSION};
use crate::model::{
    LegacyImportSummary, LogEventInsert, LogEventRow, LogSessionRow, SessionArtifact,
    StructuredLogQuery,
};
use crate::paths::{blob_dir, log_dir};

pub struct Store {
    conn: Mutex<Connection>,
    marengo_root: PathBuf,
    candump: marengo_candump::Candump,
}

struct CaptureFile {
    path: PathBuf,
    session_id: String,
    modified_seconds: i64,
}

impl Store {
    pub fn open(db_path: impl AsRef<Path>, marengo_root: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_candump(db_path, marengo_root, marengo_candump::Candump::plain())
    }

    pub fn open_with_candump(
        db_path: impl AsRef<Path>,
        marengo_root: impl AsRef<Path>,
        candump: marengo_candump::Candump,
    ) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path.as_ref())?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self {
            conn: Mutex::new(conn),
            marengo_root: marengo_root.as_ref().to_path_buf(),
            candump,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_default() -> Result<Self> {
        let root = crate::paths::resolve_marengo_root();
        let db = crate::paths::resolve_db_path();
        Self::open(db, root)
    }

    pub fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn marengo_root(&self) -> &Path {
        &self.marengo_root
    }

    pub fn migrate(&self) -> Result<()> {
        self.connection().execute_batch(MIGRATION_001)?;
        let now = now_ms();
        let version = self.schema_version()?.unwrap_or(0);
        if version < 2 {
            self.connection().execute_batch(MIGRATION_002)?;
        }
        if version < 3 {
            self.connection().execute_batch(MIGRATION_003)?;
        }
        self.set_setting("schema_version", &SCHEMA_VERSION.to_string(), now)?;
        if self.get_setting("log_archive_days")?.is_none() {
            self.set_setting(
                "log_archive_days",
                &crate::paths::DEFAULT_ARCHIVE_DAYS.to_string(),
                now,
            )?;
        }
        if self.get_setting("log_disk_budget_bytes")?.is_none() {
            self.set_setting(
                "log_disk_budget_bytes",
                &crate::paths::DEFAULT_LOG_DISK_BUDGET_BYTES.to_string(),
                now,
            )?;
        }
        Ok(())
    }

    fn schema_version(&self) -> Result<Option<i64>> {
        Ok(self
            .get_setting("schema_version")?
            .and_then(|v| v.parse::<i64>().ok()))
    }

    pub fn set_setting(&self, key: &str, value_json: &str, updated_ms: u64) -> Result<()> {
        self.connection().execute(
            "INSERT INTO settings (key, value_json, updated_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_ms = excluded.updated_ms",
            params![key, value_json, updated_ms as i64],
        )?;
        Ok(())
    }

    pub fn set_config_override(
        &self,
        key: &str,
        value_json: &str,
        source: &str,
        updated_ms: u64,
    ) -> Result<()> {
        self.connection().execute(
            "INSERT INTO config_overrides (key, value_json, updated_ms, source)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET
               value_json = excluded.value_json,
               updated_ms = excluded.updated_ms,
               source = excluded.source",
            params![key, value_json, updated_ms as i64, source],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.connection();
        conn.query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn insert_log_events(&self, events: &[LogEventInsert]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let conn = self.connection();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO log_events (ts_ms, level, target, message, session_id, fields_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for e in events {
                stmt.execute(params![
                    e.ts_ms as i64,
                    e.level,
                    e.target,
                    e.message,
                    e.session_id,
                    e.fields_json,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn recent_log_events(&self, limit: u32) -> Result<Vec<LogEventRow>> {
        let conn = self.connection();
        let mut stmt = conn.prepare(
            "SELECT id, ts_ms, level, target, message, session_id, fields_json
             FROM log_events ORDER BY ts_ms DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], map_log_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn query_structured_logs(
        &self,
        query: &StructuredLogQuery,
    ) -> Result<(Vec<LogEventRow>, u32)> {
        let limit = query.limit.clamp(1, 5000);
        let offset = query.offset;
        let mut where_clauses = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(from) = query.from_ms {
            where_clauses.push("ts_ms >= ?".to_string());
            params_vec.push(Box::new(from as i64));
        }
        if let Some(to) = query.to_ms {
            where_clauses.push("ts_ms <= ?".to_string());
            params_vec.push(Box::new(to as i64));
        }
        if let Some(level) = &query.level {
            where_clauses.push("level = ?".to_string());
            params_vec.push(Box::new(level.clone()));
        }
        if let Some(target) = &query.target {
            where_clauses.push("target = ?".to_string());
            params_vec.push(Box::new(target.clone()));
        }
        if let Some(session_id) = &query.session_id {
            where_clauses.push("session_id = ?".to_string());
            params_vec.push(Box::new(session_id.clone()));
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };

        if let Some(q) = &query.q {
            if !q.trim().is_empty() {
                let filter_sql = if where_clauses.is_empty() {
                    String::new()
                } else {
                    format!(
                        " AND {}",
                        where_clauses
                            .iter()
                            .map(|c| format!("e.{c}"))
                            .collect::<Vec<_>>()
                            .join(" AND ")
                    )
                };
                let count_sql = format!(
                    "SELECT COUNT(*)
                     FROM log_events e
                     INNER JOIN log_events_fts f ON f.rowid = e.id
                     WHERE log_events_fts MATCH ?{filter_sql}"
                );
                let select_sql = format!(
                    "SELECT e.id, e.ts_ms, e.level, e.target, e.message, e.session_id, e.fields_json
                     FROM log_events e
                     INNER JOIN log_events_fts f ON f.rowid = e.id
                     WHERE log_events_fts MATCH ?{filter_sql}
                     ORDER BY e.ts_ms DESC LIMIT ? OFFSET ?"
                );
                let fts_q = format!("{}*", q.trim());
                let mut query_params = params_vec;
                query_params.insert(0, Box::new(fts_q));
                let count_refs: Vec<&dyn rusqlite::types::ToSql> =
                    query_params.iter().map(|p| p.as_ref()).collect();
                let conn = self.connection();
                let total: u32 = conn
                    .query_row(&count_sql, count_refs.as_slice(), |row| {
                        row.get::<_, i64>(0)
                    })
                    .map(|n| n as u32)
                    .unwrap_or(0);

                query_params.push(Box::new(limit as i64));
                query_params.push(Box::new(offset as i64));
                let select_refs: Vec<&dyn rusqlite::types::ToSql> =
                    query_params.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&select_sql)?;
                let rows = stmt.query_map(select_refs.as_slice(), map_log_row)?;
                let entries: Vec<LogEventRow> = rows
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(StoreError::from)?;
                return Ok((entries, total));
            }
        }

        let count_sql = format!("SELECT COUNT(*) FROM log_events{where_sql}");
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let conn = self.connection();
        let total: u32 = conn
            .query_row(&count_sql, param_refs.as_slice(), |row| {
                row.get::<_, i64>(0)
            })
            .map(|n| n as u32)
            .unwrap_or(0);

        let select_sql = format!(
            "SELECT id, ts_ms, level, target, message, session_id, fields_json
             FROM log_events{where_sql} ORDER BY ts_ms DESC LIMIT ? OFFSET ?"
        );
        params_vec.push(Box::new(limit as i64));
        params_vec.push(Box::new(offset as i64));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&select_sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), map_log_row)?;
        let entries = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)?;
        Ok((entries, total))
    }

    pub fn purge_older_than_days(&self, days: u32) -> Result<(u64, u64)> {
        let cutoff = now_ms().saturating_sub(u64::from(days) * 86_400_000);
        self.purge_before(cutoff)
    }

    /// Remove events and sessions strictly before the supplied UTC epoch cutoff.
    /// Reject cutoffs outside SQLite's signed millisecond range before deletion.
    pub fn purge_before(&self, cutoff_ms: u64) -> Result<(u64, u64)> {
        let cutoff = i64::try_from(cutoff_ms)
            .map_err(|_| StoreError::msg("retention cutoff exceeds SQLite milliseconds range"))?;
        let conn = self.connection();
        let deleted_logs =
            conn.execute("DELETE FROM log_events WHERE ts_ms < ?1", params![cutoff])? as u64;

        // Read artifact references on the connection already held here. Calling
        // get_session would try to acquire the same non-reentrant mutex again.
        let old_sessions: Vec<LogSessionRow> = {
            let mut stmt = conn.prepare(
                "SELECT id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob,
                        candump_frame_count, candump_bytes
                 FROM log_sessions WHERE started_ms < ?1",
            )?;
            let rows = stmt.query_map(params![cutoff], map_session_row)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(StoreError::from)?
        };

        for session in &old_sessions {
            for path in [
                &session.bench_blob,
                &session.candump_blob,
                &session.trace_blob,
            ]
            .into_iter()
            .flatten()
            {
                let _ = fs::remove_file(path);
            }
            conn.execute(
                "DELETE FROM log_sessions WHERE id = ?1",
                params![session.id],
            )?;
        }

        let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
        Ok((deleted_logs, old_sessions.len() as u64))
    }

    /// Register supplied references. Omitted label/artifacts preserve existing
    /// values; the original capture start/end are unchanged on updates. A changed
    /// candump reference invalidates statistics belonging to the old file.
    /// A new capture has no known end until explicit finalization.
    pub fn register_session(
        &self,
        id: &str,
        label: Option<&str>,
        started_ms: u64,
        bench: Option<&Path>,
        candump: Option<&Path>,
        trace: Option<&Path>,
    ) -> Result<()> {
        self.connection().execute(
            "INSERT INTO log_sessions (id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               label = COALESCE(excluded.label, log_sessions.label),
               candump_frame_count = CASE
                 WHEN excluded.candump_blob IS NOT NULL AND excluded.candump_blob IS NOT log_sessions.candump_blob
                 THEN NULL ELSE log_sessions.candump_frame_count END,
               candump_bytes = CASE
                 WHEN excluded.candump_blob IS NOT NULL AND excluded.candump_blob IS NOT log_sessions.candump_blob
                 THEN NULL ELSE log_sessions.candump_bytes END,
               bench_blob = COALESCE(excluded.bench_blob, log_sessions.bench_blob),
               candump_blob = COALESCE(excluded.candump_blob, log_sessions.candump_blob),
               trace_blob = COALESCE(excluded.trace_blob, log_sessions.trace_blob)",
            params![
                id,
                label,
                started_ms as i64,
                bench.map(|p| p.display().to_string()),
                candump.map(|p| p.display().to_string()),
                trace.map(|p| p.display().to_string()),
            ],
        )?;
        Ok(())
    }

    /// Clear one reference, preserving the file, siblings and capture metadata.
    /// Returns false for an absent session or an already empty reference.
    pub fn clear_session_artifact(&self, id: &str, artifact: SessionArtifact) -> Result<bool> {
        let (column, extra) = match artifact {
            SessionArtifact::Bench => ("bench_blob", ""),
            SessionArtifact::Candump => (
                "candump_blob",
                ", candump_frame_count = NULL, candump_bytes = NULL",
            ),
            SessionArtifact::Trace => ("trace_blob", ""),
        };
        let sql = format!(
            "UPDATE log_sessions SET {column} = NULL{extra} WHERE id = ?1 AND {column} IS NOT NULL"
        );
        Ok(self.connection().execute(&sql, params![id])? != 0)
    }

    pub fn finalize_session(&self, id: &str, ended_ms: u64) -> Result<()> {
        self.connection().execute(
            "UPDATE log_sessions SET ended_ms = ?1 WHERE id = ?2",
            params![ended_ms as i64, id],
        )?;
        Ok(())
    }

    pub fn list_sessions(
        &self,
        from_ms: Option<u64>,
        to_ms: Option<u64>,
        label: Option<&str>,
        limit: u32,
    ) -> Result<Vec<LogSessionRow>> {
        let mut sql =
            String::from("SELECT id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob, candump_frame_count, candump_bytes FROM log_sessions WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(from) = from_ms {
            sql.push_str(" AND started_ms >= ?");
            params_vec.push(Box::new(from as i64));
        }
        if let Some(to) = to_ms {
            sql.push_str(" AND started_ms <= ?");
            params_vec.push(Box::new(to as i64));
        }
        if let Some(lbl) = label {
            sql.push_str(" AND label = ?");
            params_vec.push(Box::new(lbl.to_string()));
        }
        sql.push_str(" ORDER BY started_ms DESC LIMIT ?");
        params_vec.push(Box::new(limit.clamp(1, 500) as i64));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let conn = self.connection();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), map_session_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<LogSessionRow>> {
        let conn = self.connection();
        conn
            .query_row(
                "SELECT id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob, candump_frame_count, candump_bytes
                 FROM log_sessions WHERE id = ?1",
                params![id],
                map_session_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn latest_session(&self) -> Result<Option<LogSessionRow>> {
        let conn = self.connection();
        conn
            .query_row(
                "SELECT id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob, candump_frame_count, candump_bytes
                 FROM log_sessions ORDER BY started_ms DESC LIMIT 1",
                [],
                map_session_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn archive_hot_sessions(&self, keep: usize) -> Result<u32> {
        let hot = log_dir(&self.marengo_root);
        let mut groups = Vec::new();
        for artifact in [
            SessionArtifact::Bench,
            SessionArtifact::Candump,
            SessionArtifact::Trace,
        ] {
            let mut files = list_timestamped_files(&hot, artifact)?;
            files.sort_by(|a, b| b.modified_seconds.cmp(&a.modified_seconds));
            let files: Vec<_> = files.into_iter().skip(keep).collect();
            for file in &files {
                self.capture_started_ms(&file.session_id)?;
            }
            groups.push((artifact, files));
        }
        // Resolve every selected capture before publishing even an earlier kind.
        // This does not roll back later filesystem or SQLite failures.
        fs::create_dir_all(blob_dir(&self.marengo_root))?;
        let mut archived = 0u32;
        for (artifact, files) in groups {
            archived += self.archive_files(files, artifact)?;
        }
        Ok(archived)
    }

    fn archive_files(&self, files: Vec<CaptureFile>, artifact: SessionArtifact) -> Result<u32> {
        let mut archived = 0u32;
        for CaptureFile {
            path, session_id, ..
        } in files
        {
            let gz_path = self.gzip_to_blob(&path, &session_id)?;
            self.update_session_blob(&session_id, artifact, &gz_path)?;
            if artifact == SessionArtifact::Candump {
                let inspection = self.candump.inspect_path(
                    &gz_path,
                    marengo_candump::InspectRequest::summary(marengo_candump::TimestampMode::Delta),
                )?;
                let bytes = inspection.summary.source_bytes;
                self.connection().execute(
                    "UPDATE log_sessions SET candump_frame_count = ?1, candump_bytes = ?2 WHERE id = ?3",
                    params![
                        inspection.summary.parsed_frames as i64,
                        bytes as i64,
                        session_id
                    ],
                )?;
            }
            fs::remove_file(&path)?;
            archived += 1;
        }
        Ok(archived)
    }

    fn gzip_to_blob(&self, source: &Path, session_id: &str) -> Result<PathBuf> {
        let date = session_id_to_date(session_id);
        let dest_dir = blob_dir(&self.marengo_root).join(&date);
        fs::create_dir_all(&dest_dir)?;
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| StoreError::msg("invalid source filename"))?;
        let dest = dest_dir.join(format!("{name}.gz"));
        let tmp = dest.with_extension("gz.tmp");
        {
            let mut input = File::open(source)?;
            let out = File::create(&tmp)?;
            let mut enc = GzEncoder::new(out, Compression::default());
            std::io::copy(&mut input, &mut enc)?;
            enc.finish()?;
        }
        fs::rename(&tmp, &dest)?;
        Ok(dest)
    }

    fn update_session_blob(
        &self,
        session_id: &str,
        artifact: SessionArtifact,
        gz_path: &Path,
    ) -> Result<()> {
        let col = match artifact {
            SessionArtifact::Bench => "bench_blob",
            SessionArtifact::Candump => "candump_blob",
            SessionArtifact::Trace => "trace_blob",
        };
        let sql = format!(
            "INSERT INTO log_sessions (id, label, started_ms, {col})
             VALUES (?1, NULL, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET {col} = excluded.{col}"
        );
        let started = self.capture_started_ms(session_id)?;
        self.connection().execute(
            &sql,
            params![session_id, started as i64, gz_path.display().to_string()],
        )?;
        Ok(())
    }

    pub fn read_bench_page(
        &self,
        session_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<String>, u32)> {
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| StoreError::msg("session not found"))?;
        let path = session
            .bench_blob
            .ok_or_else(|| StoreError::msg("no bench blob"))?;
        read_text_page(&path, offset, limit)
    }

    pub fn read_candump_page(
        &self,
        session_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<marengo_candump::Inspection> {
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| StoreError::msg("session not found"))?;
        let path = session
            .candump_blob
            .ok_or_else(|| StoreError::msg("no candump blob"))?;
        let page = marengo_candump::FramePage::new(u64::from(offset), limit)?;
        Ok(self.candump.inspect_path(
            &path,
            marengo_candump::InspectRequest::page(marengo_candump::TimestampMode::Delta, page),
        )?)
    }

    pub fn read_hot_candump_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<marengo_candump::Inspection> {
        let hot = log_dir(&self.marengo_root).join("candump-latest.log");
        if !hot.exists() {
            return Ok(marengo_candump::Inspection {
                timestamp_mode: marengo_candump::TimestampMode::Delta,
                summary: marengo_candump::Summary {
                    total_lines: 0,
                    parsed_frames: 0,
                    source_bytes: 0,
                    duration_s: 0.0,
                    approx_hz: None,
                    interfaces: Vec::new(),
                    top_ids: Vec::new(),
                },
                frames: Vec::new(),
            });
        }
        let page = marengo_candump::FramePage::new(u64::from(offset), limit)?;
        Ok(self.candump.inspect_path(
            &hot,
            marengo_candump::InspectRequest::page(marengo_candump::TimestampMode::Delta, page),
        )?)
    }

    pub fn candump_summary(&self, session_id: &str) -> Result<marengo_candump::Summary> {
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| StoreError::msg("session not found"))?;
        let path = session
            .candump_blob
            .ok_or_else(|| StoreError::msg("no candump blob"))?;
        Ok(self
            .candump
            .inspect_path(
                &path,
                marengo_candump::InspectRequest::summary(marengo_candump::TimestampMode::Delta),
            )?
            .summary)
    }

    pub fn hot_candump_summary(&self) -> Result<marengo_candump::Summary> {
        let hot = log_dir(&self.marengo_root).join("candump-latest.log");
        if !hot.exists() {
            return Ok(marengo_candump::Summary {
                total_lines: 0,
                parsed_frames: 0,
                source_bytes: 0,
                duration_s: 0.0,
                approx_hz: None,
                interfaces: Vec::new(),
                top_ids: Vec::new(),
            });
        }
        Ok(self
            .candump
            .inspect_path(
                &hot,
                marengo_candump::InspectRequest::summary(marengo_candump::TimestampMode::Delta),
            )?
            .summary)
    }

    pub fn read_trace_page(
        &self,
        session_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<String>, u32)> {
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| StoreError::msg("session not found"))?;
        let path = session
            .trace_blob
            .ok_or_else(|| StoreError::msg("no trace blob"))?;
        read_text_page(&path, offset, limit)
    }

    pub fn log_disk_usage_bytes(&self) -> Result<u64> {
        let mut total = 0u64;
        if let Ok(meta) = fs::metadata(crate::paths::resolve_db_path()) {
            total += meta.len();
        }
        total += dir_size(&log_dir(&self.marengo_root))?;
        Ok(total)
    }

    /// Compatibility entry point returning unique session IDs processed.
    pub fn import_legacy_hot(&self, keep: usize) -> Result<u32> {
        Ok(self.import_legacy_hot_report(keep)?.sessions)
    }

    /// Import hot references and archive beyond `keep`. Counts describe files
    /// processed this call, not newly inserted rows or the total archive.
    pub fn import_legacy_hot_report(&self, keep: usize) -> Result<LegacyImportSummary> {
        let hot = log_dir(&self.marengo_root);
        if !hot.is_dir() {
            return Ok(LegacyImportSummary::default());
        }
        let mut candidates = Vec::new();
        for entry in fs::read_dir(&hot)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some((artifact, session_id)) = capture_name(name) {
                let started = self.capture_started_ms(session_id)?;
                let session_id = session_id.to_string();
                candidates.push((path, artifact, session_id, started));
            }
        }
        let mut registered = HashSet::new();
        let mut artifacts = 0u32;
        for (path, artifact, session_id, started) in candidates {
            self.register_session(
                &session_id,
                None,
                started,
                (artifact == SessionArtifact::Bench).then_some(path.as_path()),
                (artifact == SessionArtifact::Candump).then_some(path.as_path()),
                (artifact == SessionArtifact::Trace).then_some(path.as_path()),
            )?;
            registered.insert(session_id);
            artifacts = artifacts
                .checked_add(1)
                .ok_or_else(|| StoreError::msg("too many legacy artifacts"))?;
        }
        self.archive_hot_sessions(keep)?;
        Ok(LegacyImportSummary {
            sessions: u32::try_from(registered.len())
                .map_err(|_| StoreError::msg("too many legacy sessions"))?,
            artifacts,
        })
    }

    fn capture_started_ms(&self, session_id: &str) -> Result<u64> {
        if let Some(existing) = self.get_session(session_id)? {
            return Ok(existing.started_ms);
        }
        session_id_to_ms(session_id).ok_or_else(|| StoreError::UnknownCaptureDate {
            session_id: session_id.to_string(),
        })
    }
}

fn map_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogEventRow> {
    Ok(LogEventRow {
        id: row.get(0)?,
        ts_ms: row.get::<_, i64>(1)? as u64,
        level: row.get(2)?,
        target: row.get(3)?,
        message: row.get(4)?,
        session_id: row.get(5)?,
        fields_json: row.get(6)?,
    })
}

fn map_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogSessionRow> {
    Ok(LogSessionRow {
        id: row.get(0)?,
        label: row.get(1)?,
        started_ms: row.get::<_, i64>(2)? as u64,
        ended_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
        bench_blob: row.get(4)?,
        candump_blob: row.get(5)?,
        trace_blob: row.get(6)?,
        candump_frame_count: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
        candump_bytes: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
    })
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn list_timestamped_files(dir: &Path, artifact: SessionArtifact) -> Result<Vec<CaptureFile>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let Some((kind, session_id)) = capture_name(name) else {
            continue;
        };
        if kind != artifact {
            continue;
        }
        let session_id = session_id.to_string();
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        out.push(CaptureFile {
            path,
            session_id,
            modified_seconds: mtime,
        });
    }
    Ok(out)
}

fn capture_name(name: &str) -> Option<(SessionArtifact, &str)> {
    for (artifact, prefix, extension) in [
        (SessionArtifact::Bench, "bench-", ".log"),
        (SessionArtifact::Candump, "candump-", ".log"),
        (SessionArtifact::Trace, "position-trace-", ".csv"),
    ] {
        if let Some(id) = name
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(extension))
        {
            if !id.is_empty() && id != "latest" {
                return Some((artifact, id));
            }
        }
    }
    None
}

fn session_id_to_date(session_id: &str) -> String {
    capture_datetime(session_id)
        .map(|dt| dt.date().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn session_id_to_ms(session_id: &str) -> Option<u64> {
    u64::try_from(capture_datetime(session_id)?.unix_timestamp_nanos() / 1_000_000).ok()
}

fn capture_datetime(session_id: &str) -> Option<OffsetDateTime> {
    let canonical = session_id.strip_prefix("profile-").unwrap_or(session_id);
    if canonical.len() != 16
        || !canonical
            .bytes()
            .enumerate()
            .all(|(index, byte)| match index {
                8 => byte == b'T',
                15 => byte == b'Z',
                _ => byte.is_ascii_digit(),
            })
    {
        return None;
    }
    let parsed =
        time::format_description::parse("[year][month][day]T[hour][minute][second]Z").ok()?;
    let dt = time::PrimitiveDateTime::parse(canonical, &parsed)
        .ok()?
        .assume_utc();
    (dt.unix_timestamp() >= 0).then_some(dt)
}

fn read_text_page(path: &str, offset: u32, limit: u32) -> Result<(Vec<String>, u32)> {
    let gz = path.ends_with(".gz");
    let file = File::open(path)?;
    let mut reader: Box<dyn BufRead> = if gz {
        Box::new(BufReader::new(GzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    let mut all = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        all.push(line.trim_end().to_string());
    }
    let total = all.len() as u32;
    let start = offset.min(total) as usize;
    let end = (start + limit as usize).min(all.len());
    Ok((all[start..end].to_vec(), total))
}

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    if path.is_file() {
        return Ok(fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    }
    if !path.is_dir() {
        return Ok(0);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        total += dir_size(&entry.path())?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrate_and_insert_logs() -> Result<()> {
        let dir = tempdir()?;
        let db = dir.path().join("test.db");
        let store = Store::open(&db, dir.path())?;
        store.insert_log_events(&[LogEventInsert {
            ts_ms: 1000,
            level: "info".into(),
            target: "test".into(),
            message: "hello".into(),
            session_id: None,
            fields_json: Some(r#"{"joint":"shoulder_pitch"}"#.into()),
        }])?;
        let recent = store.recent_log_events(10)?;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "hello");
        Ok(())
    }

    #[test]
    fn fts_query_applies_level_filter_and_count() -> Result<()> {
        let dir = tempdir()?;
        let db = dir.path().join("fts.db");
        let store = Store::open(&db, dir.path())?;
        store.insert_log_events(&[
            LogEventInsert {
                ts_ms: 1000,
                level: "info".into(),
                target: "berthier".into(),
                message: "shoulder pitch hold".into(),
                session_id: None,
                fields_json: None,
            },
            LogEventInsert {
                ts_ms: 2000,
                level: "error".into(),
                target: "berthier".into(),
                message: "shoulder pitch fault".into(),
                session_id: None,
                fields_json: None,
            },
        ])?;
        let (entries, total) = store.query_structured_logs(&StructuredLogQuery {
            from_ms: None,
            to_ms: None,
            target: None,
            session_id: None,
            q: Some("shoulder".into()),
            level: Some("error".into()),
            limit: 10,
            offset: 0,
        })?;
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, "error");
        Ok(())
    }

    #[test]
    fn candump_page_delegates_to_deep_module() -> Result<()> {
        let dir = tempdir()?;
        let db = dir.path().join("candump.db");
        let log_dir = dir.path().join("var").join("log");
        fs::create_dir_all(&log_dir)?;
        let hot = log_dir.join("candump-latest.log");
        fs::write(
            &hot,
            "(0.000000) can0 701#AABBCCDD\n(0.010000) can0 702#11223344\n",
        )?;
        let store = Store::open(&db, dir.path())?;
        let inspection = store.read_hot_candump_page(0, 10)?;
        assert_eq!(inspection.summary.parsed_frames, 2, "parsed frame count");
        assert_eq!(inspection.frames.len(), 2);
        assert_eq!(inspection.frames[0].interface, "can0");
        assert_eq!(inspection.frames[0].can_id.get(), 0x701);
        assert_eq!(inspection.frames[0].data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
        Ok(())
    }
}
