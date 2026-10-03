use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;
use time::OffsetDateTime;

use crate::error::{Result, StoreError};
use crate::migrations;
use crate::model::{
    LegacyImportSummary, LogEventInsert, LogEventRow, LogSessionRow, SessionArtifact,
    StructuredLogQuery,
};
use crate::paths::{blob_dir, log_dir};

pub struct Store {
    conn: Mutex<Connection>,
    db_path: PathBuf,
    marengo_root: PathBuf,
    candump: marengo_candump::Candump,
}

struct CaptureFile {
    path: PathBuf,
    session_id: String,
    modified_seconds: i64,
}

/// Process-unique suffix for archive temp files: the CLI and the gateway
/// archive concurrently, so deterministic `.gz.tmp` names would collide.
static ARCHIVE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
        // ADR 0029 bounded five-second busy policy on the normal path too,
        // so the nightly CLI and the gateway batch writer wait out each
        // other's write reservations instead of failing with SQLITE_BUSY.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self {
            conn: Mutex::new(conn),
            db_path: db_path.as_ref().to_path_buf(),
            marengo_root: marengo_root.as_ref().to_path_buf(),
            candump,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn marengo_root(&self) -> &Path {
        &self.marengo_root
    }

    pub fn migrate(&self) -> Result<()> {
        let mut conn = self.connection();
        migrations::migrate(&mut conn, now_ms())
    }

    pub fn set_setting(&self, key: &str, value_json: &str, updated_ms: u64) -> Result<()> {
        self.connection().execute(
            "INSERT INTO settings (key, value_json, updated_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_ms = excluded.updated_ms",
            params![key, value_json, ms_to_sql(updated_ms)?],
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
            params![key, value_json, ms_to_sql(updated_ms)?, source],
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
        insert_events(&tx, events)?;
        tx.commit()?;
        Ok(())
    }

    /// Insert events and advance a settings cursor in one transaction, so a
    /// crash between the two can neither lose events nor re-import them.
    /// Used by journal import; the cursor value is opaque to the store.
    pub fn insert_log_events_with_cursor(
        &self,
        events: &[LogEventInsert],
        cursor_key: &str,
        cursor: &str,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let conn = self.connection();
        let tx = conn.unchecked_transaction()?;
        insert_events(&tx, events)?;
        tx.execute(
            "INSERT INTO settings (key, value_json, updated_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_ms = excluded.updated_ms",
            params![cursor_key, cursor, ms_to_sql(now_ms())?],
        )?;
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
            params_vec.push(Box::new(ms_to_sql(from)?));
        }
        if let Some(to) = query.to_ms {
            where_clauses.push("ts_ms <= ?".to_string());
            params_vec.push(Box::new(ms_to_sql(to)?));
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
            // Sanitize to a quoted phrase-prefix query: embedded quotes and
            // FTS operators then match literally instead of breaking MATCH
            // syntax (previously a `"` in `q` failed the whole query).
            let sanitized: String = q
                .trim()
                .replace('"', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !sanitized.is_empty() {
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
                let fts_q = format!("\"{sanitized}\"*");
                let mut query_params = params_vec;
                query_params.insert(0, Box::new(fts_q));
                let count_refs: Vec<&dyn rusqlite::types::ToSql> =
                    query_params.iter().map(|p| p.as_ref()).collect();
                let conn = self.connection();
                let total: u32 = conn
                    .query_row(&count_sql, count_refs.as_slice(), |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(StoreError::from)
                    .and_then(|n| {
                        u32::try_from(n)
                            .map_err(|_| StoreError::msg("log row count exceeds u32 range"))
                    })?;

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
            .map_err(StoreError::from)
            .and_then(|n| {
                u32::try_from(n).map_err(|_| StoreError::msg("log row count exceeds u32 range"))
            })?;

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
    ///
    /// The newest unfinalized session is never purged: it is the live
    /// capture whose hot files are being written and whose rows are still
    /// arriving. Older abandoned sessions (crashed before finalize) still
    /// purge by age so they cannot pin disk forever.
    /// Artifact files are removed before their rows in one transaction, so a
    /// crash leaves rows pointing at missing files (re-driven on the next
    /// purge) rather than blobs without rows (leaked disk no future purge
    /// can find). File-removal failures are reported, never swallowed, and
    /// do not abort the purge.
    pub fn purge_before(&self, cutoff_ms: u64) -> Result<(u64, u64)> {
        let cutoff = i64::try_from(cutoff_ms)
            .map_err(|_| StoreError::msg("retention cutoff exceeds SQLite milliseconds range"))?;
        let conn = self.connection();

        // Read artifact references on the connection already held here. Calling
        // get_session would try to acquire the same non-reentrant mutex again.
        let old_sessions: Vec<LogSessionRow> = {
            let mut stmt = conn.prepare(
                "SELECT id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob,
                        candump_frame_count, candump_bytes
                 FROM log_sessions WHERE started_ms < ?1
                   AND (ended_ms IS NOT NULL OR id != (
                     SELECT id FROM log_sessions
                     ORDER BY started_ms DESC, id DESC LIMIT 1))",
            )?;
            let rows = stmt.query_map(params![cutoff], map_session_row)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(StoreError::from)?
        };

        for session in &old_sessions {
            remove_session_files(session);
        }

        let tx = conn.unchecked_transaction()?;
        let deleted_logs =
            tx.execute("DELETE FROM log_events WHERE ts_ms < ?1", params![cutoff])? as u64;
        for session in &old_sessions {
            tx.execute(
                "DELETE FROM log_sessions WHERE id = ?1",
                params![session.id],
            )?;
        }
        tx.commit()?;

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
                ms_to_sql(started_ms)?,
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
            params![ms_to_sql(ended_ms)?, id],
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
            params_vec.push(Box::new(ms_to_sql(to)?));
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

    pub fn archive_hot_sessions(&self, keep: usize) -> Result<u32> {
        let hot = log_dir(&self.marengo_root);
        // Group hot captures by session so one session's bench/candump/trace
        // archive together: keeping per artifact kind by mtime could strand
        // a session split across hot and blob. Sessions rank by their newest
        // artifact mtime; the `keep` newest sessions stay hot in full.
        let mut by_session: HashSet<String> = HashSet::new();
        let mut staged: Vec<(SessionArtifact, CaptureFile)> = Vec::new();
        for artifact in [
            SessionArtifact::Bench,
            SessionArtifact::Candump,
            SessionArtifact::Trace,
        ] {
            for file in list_timestamped_files(&hot, artifact)? {
                by_session.insert(file.session_id.clone());
                staged.push((artifact, file));
            }
        }
        let mut sessions: Vec<(String, i64)> = by_session
            .into_iter()
            .map(|id| {
                let newest = staged
                    .iter()
                    .filter(|(_, file)| file.session_id == id)
                    .map(|(_, file)| file.modified_seconds)
                    .max()
                    .unwrap_or(0);
                (id, newest)
            })
            .collect();
        sessions.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let archive_ids: HashSet<&str> = sessions
            .iter()
            .skip(keep)
            .map(|(id, _)| id.as_str())
            .collect();
        let mut groups: Vec<(SessionArtifact, Vec<CaptureFile>)> = Vec::new();
        for artifact in [
            SessionArtifact::Bench,
            SessionArtifact::Candump,
            SessionArtifact::Trace,
        ] {
            let files: Vec<CaptureFile> = staged
                .iter()
                .filter(|(kind, file)| {
                    *kind == artifact && archive_ids.contains(file.session_id.as_str())
                })
                .map(|(_, file)| CaptureFile {
                    path: file.path.clone(),
                    session_id: file.session_id.clone(),
                    modified_seconds: file.modified_seconds,
                })
                .collect();
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
        // Unique temp per process so concurrent CLI/gateway archives never
        // share a temp name; fsync file and directory so a power loss after
        // "archived" cannot resurrect the pre-rename state.
        let unique = ARCHIVE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = dest.with_file_name(format!("{name}.gz.tmp.{}-{unique}", std::process::id()));
        let result = (|| {
            let mut input = File::open(source)?;
            let out = File::create(&tmp)?;
            let mut enc = GzEncoder::new(out, Compression::default());
            std::io::copy(&mut input, &mut enc)?;
            let out = enc.finish()?;
            out.sync_all()?;
            drop(out);
            fs::rename(&tmp, &dest)?;
            let dir = fs::File::open(&dest_dir)?;
            dir.sync_all()?;
            Ok(dest.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
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
            params![
                session_id,
                ms_to_sql(started)?,
                gz_path.display().to_string()
            ],
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
                    // Zero parsed frames resolve no joints.
                    enriched: false,
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
                // Zero parsed frames resolve no joints.
                enriched: false,
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
        crate::disk::log_disk_usage_bytes(&self.marengo_root, &self.db_path)
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

fn insert_events(tx: &rusqlite::Transaction<'_>, events: &[LogEventInsert]) -> Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO log_events (ts_ms, level, target, message, session_id, fields_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for e in events {
        stmt.execute(params![
            ms_to_sql(e.ts_ms)?,
            e.level,
            e.target,
            e.message,
            e.session_id,
            e.fields_json,
        ])?;
    }
    Ok(())
}

fn map_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogEventRow> {
    Ok(LogEventRow {
        id: row.get(0)?,
        ts_ms: u64_from_sql(row.get::<_, i64>(1)?, 1)?,
        level: row.get(2)?,
        target: row.get(3)?,
        message: row.get(4)?,
        session_id: row.get(5)?,
        fields_json: row.get(6)?,
    })
}

pub(crate) fn map_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogSessionRow> {
    Ok(LogSessionRow {
        id: row.get(0)?,
        label: row.get(1)?,
        started_ms: u64_from_sql(row.get::<_, i64>(2)?, 2)?,
        ended_ms: row
            .get::<_, Option<i64>>(3)?
            .map(|v| u64_from_sql(v, 3))
            .transpose()?,
        bench_blob: row.get(4)?,
        candump_blob: row.get(5)?,
        trace_blob: row.get(6)?,
        candump_frame_count: row
            .get::<_, Option<i64>>(7)?
            .map(|v| u64_from_sql(v, 7))
            .transpose()?,
        candump_bytes: row
            .get::<_, Option<i64>>(8)?
            .map(|v| u64_from_sql(v, 8))
            .transpose()?,
    })
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Millisecond timestamps cross into SQLite as signed i64: reject
/// out-of-range values instead of wrapping them silently.
fn ms_to_sql(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| StoreError::msg("timestamp exceeds SQLite millisecond range"))
}

/// Reject negative stored timestamps instead of wrapping them into huge u64s.
fn u64_from_sql(value: i64, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "negative timestamp in store row",
            )),
        )
    })
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

fn open_text_lines(path: &str) -> Result<Box<dyn BufRead>> {
    let gz = path.ends_with(".gz");
    let file = File::open(path)?;
    if gz {
        Ok(Box::new(BufReader::new(GzDecoder::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

/// Read one page of a (possibly compressed) line file without materializing
/// the whole file: stream once to count, then stream again for the window.
/// Two passes over gzip input decompress twice; page requests stay bounded
/// in memory either way. Totals saturate (never wrap) at `u32::MAX`.
fn read_text_page(path: &str, offset: u32, limit: u32) -> Result<(Vec<String>, u32)> {
    let total = count_text_lines(path)?;
    let start = u64::from(offset.min(total));
    let want = u64::from(limit);
    if want == 0 || start >= u64::from(total) {
        return Ok((Vec::new(), total));
    }
    let end = start.saturating_add(want).min(u64::from(total));
    let mut reader = open_text_lines(path)?;
    let mut page = Vec::new();
    let mut line = String::new();
    let mut index = 0u64;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if index >= start && index < end {
            page.push(line.trim_end().to_string());
        }
        index += 1;
        if index >= end {
            break;
        }
    }
    Ok((page, total))
}

#[cfg(test)]
fn walkdir_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walkdir_files(&path));
        } else if let Some(name) = path.to_str() {
            out.push(name.to_string());
        }
    }
    out
}

fn count_text_lines(path: &str) -> Result<u32> {
    let mut reader = open_text_lines(path)?;
    let mut total = 0u64;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        total = total.saturating_add(1);
    }
    Ok(u32::try_from(total).unwrap_or(u32::MAX))
}

/// Best-effort removal of one session's artifact files. Symlinks are never
/// followed. Failures are reported via tracing and never swallowed: callers
/// delete the session row afterwards in the same transaction, so a leftover
/// file is re-driven on the next purge instead of silently leaking.
pub(crate) fn remove_session_files(session: &LogSessionRow) {
    for path in [
        &session.bench_blob,
        &session.candump_blob,
        &session.trace_blob,
    ]
    .into_iter()
    .flatten()
    {
        if let Err(error) = fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(path = %path, %error, "purge could not remove session artifact");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn open_applies_bounded_busy_policy() -> Result<()> {
        let dir = tempdir()?;
        let db = dir.path().join("busy.db");
        let store = Store::open(&db, dir.path())?;
        let ms: i64 = store
            .connection()
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))?;
        assert_eq!(ms, 5_000, "ADR 0029 five-second busy policy");
        Ok(())
    }

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
    fn fts_query_with_quote_matches_literally() -> Result<()> {
        // A `"` in the query must not break MATCH syntax: it is sanitized
        // to a quoted phrase-prefix and matches literally.
        let dir = tempdir()?;
        let db = dir.path().join("fts-quote.db");
        let store = Store::open(&db, dir.path())?;
        store.insert_log_events(&[LogEventInsert {
            ts_ms: 1000,
            level: "info".into(),
            target: "berthier".into(),
            message: "say \"hello\" shoulder".into(),
            session_id: None,
            fields_json: None,
        }])?;
        let (entries, total) = store.query_structured_logs(&StructuredLogQuery {
            from_ms: None,
            to_ms: None,
            target: None,
            session_id: None,
            q: Some("\"hello\"".into()),
            level: None,
            limit: 10,
            offset: 0,
        })?;
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
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
    fn out_of_range_timestamps_fail_closed() -> Result<()> {
        let dir = tempdir()?;
        let db = dir.path().join("range.db");
        let store = Store::open(&db, dir.path())?;
        // Far-future writes never wrap into negative SQLite values.
        let refused = store.insert_log_events(&[LogEventInsert {
            ts_ms: u64::MAX,
            level: "info".into(),
            target: "range".into(),
            message: "too far".into(),
            session_id: None,
            fields_json: None,
        }]);
        assert!(refused.is_err(), "u64::MAX timestamp must be refused");
        // Hand-edited negative rows never wrap into huge u64 timestamps.
        store.connection().execute(
            "INSERT INTO log_events (ts_ms, level, target, message) VALUES (?1, 'info', 't', 'm')",
            rusqlite::params![-1i64],
        )?;
        assert!(
            store.recent_log_events(10).is_err(),
            "negative stored timestamp must be refused"
        );
        Ok(())
    }

    #[test]
    fn text_page_streams_window_without_head_loss() -> Result<()> {
        use std::io::Write;
        let dir = tempdir()?;
        // Paging must address absolute lines: page 2 of 5 single-line rows
        // returns exactly rows 3-4, with the true total.
        let path = dir.path().join("bench.log");
        {
            let mut file = std::fs::File::create(&path)?;
            for index in 0..5 {
                writeln!(file, "row-{index}")?;
            }
        }
        let name = path.display().to_string();
        let (page, total) = read_text_page(&name, 2, 2)?;
        assert_eq!(total, 5);
        assert_eq!(page, vec!["row-2".to_string(), "row-3".to_string()]);
        let (empty, total) = read_text_page(&name, 9, 10)?;
        assert_eq!(total, 5);
        assert!(empty.is_empty(), "offset past end returns no rows");
        Ok(())
    }

    #[test]
    fn archive_leaves_no_temp_and_roundtrips() -> Result<()> {
        use std::io::Read;
        let dir = tempdir()?;
        let db = dir.path().join("archive.db");
        let store = Store::open(&db, dir.path())?;
        let hot = crate::paths::log_dir(dir.path());
        std::fs::create_dir_all(&hot)?;
        let body = b"bench line one\nbench line two\n";
        std::fs::write(hot.join("bench-20260101T000000Z.log"), body)?;
        let archived = store.archive_hot_sessions(0)?;
        assert_eq!(archived, 1);
        let session = store
            .get_session("20260101T000000Z")?
            .expect("archived session");
        let blob = session.bench_blob.expect("bench blob");
        let mut gz = Vec::new();
        std::fs::File::open(&blob)?.read_to_end(&mut gz)?;
        assert_eq!(&gz[0..2], &[0x1f, 0x8b], "blob is gzip");
        let mut plain = String::new();
        flate2::read::GzDecoder::new(&gz[..]).read_to_string(&mut plain)?;
        assert_eq!(plain.as_bytes(), body);
        // No process-unique temp may survive beside the blob.
        let blobs = crate::paths::blob_dir(dir.path());
        let mut leftovers = Vec::new();
        for entry in walkdir_files(&blobs) {
            if entry.contains(".gz.tmp.") {
                leftovers.push(entry);
            }
        }
        assert!(leftovers.is_empty(), "temp leftovers: {leftovers:?}");
        Ok(())
    }

    #[test]
    fn archive_keep_is_session_atomic() -> Result<()> {
        // `keep = 1` must keep one session whole: no session may end up
        // split across hot files and blobs, whatever the mtime order.
        let dir = tempdir()?;
        let db = dir.path().join("keep.db");
        let store = Store::open(&db, dir.path())?;
        let hot = crate::paths::log_dir(dir.path());
        std::fs::create_dir_all(&hot)?;
        std::fs::write(hot.join("bench-20260101T000000Z.log"), b"a-bench")?;
        std::fs::write(hot.join("candump-20260101T000000Z.log"), b"a-candump")?;
        std::fs::write(hot.join("bench-20260102T000000Z.log"), b"b-bench")?;
        store.archive_hot_sessions(1)?;
        let hot_a = hot.join("bench-20260101T000000Z.log").exists() as u8
            + hot.join("candump-20260101T000000Z.log").exists() as u8;
        let hot_b = hot.join("bench-20260102T000000Z.log").exists() as u8;
        assert!(
            (hot_a == 2 && hot_b == 0) || (hot_a == 0 && hot_b == 1),
            "one session stays hot whole: hot_a={hot_a} hot_b={hot_b}"
        );
        // Archived sessions gain rows pointing at every blob; a session
        // that stays hot whole has no blob refs yet (rows are created by
        // archiving, not by capture).
        for (id, hot_left) in [("20260101T000000Z", hot_a), ("20260102T000000Z", hot_b)] {
            match store.get_session(id)? {
                None => assert!(hot_left > 0, "rowless session {id} must be hot"),
                Some(session) => {
                    let blobbed = [session.bench_blob, session.candump_blob]
                        .into_iter()
                        .flatten()
                        .count();
                    assert_eq!(
                        blobbed + usize::from(hot_left),
                        if id.starts_with("20260101") { 2 } else { 1 },
                        "session {id} fully accounted"
                    );
                }
            }
        }
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
