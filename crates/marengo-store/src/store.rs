use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::{Result, StoreError};
use crate::migrations::{MIGRATION_001, MIGRATION_002, MIGRATION_003, SCHEMA_VERSION};
use crate::model::{LogEventInsert, LogEventRow, LogSessionRow, StructuredLogQuery};
use crate::paths::{blob_dir, log_dir};

pub struct Store {
    conn: Mutex<Connection>,
    marengo_root: PathBuf,
    candump: marengo_candump::Candump,
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
        let conn = self.connection();
        let deleted_logs = conn.execute(
            "DELETE FROM log_events WHERE ts_ms < ?1",
            params![cutoff as i64],
        )? as u64;

        let old_sessions: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id FROM log_sessions WHERE started_ms < ?1")?;
            let rows = stmt.query_map(params![cutoff as i64], |row| row.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(StoreError::from)?
        };

        for id in &old_sessions {
            if let Some(session) = self.get_session(id)? {
                for path in [session.bench_blob, session.candump_blob, session.trace_blob]
                    .into_iter()
                    .flatten()
                {
                    let _ = fs::remove_file(&path);
                }
            }
            conn.execute("DELETE FROM log_sessions WHERE id = ?1", params![id])?;
        }

        let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
        Ok((deleted_logs, old_sessions.len() as u64))
    }

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
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               label = excluded.label,
               ended_ms = excluded.ended_ms,
               bench_blob = excluded.bench_blob,
               candump_blob = excluded.candump_blob,
               trace_blob = excluded.trace_blob",
            params![
                id,
                label,
                started_ms as i64,
                now_ms() as i64,
                bench.map(|p| p.display().to_string()),
                candump.map(|p| p.display().to_string()),
                trace.map(|p| p.display().to_string()),
            ],
        )?;
        Ok(())
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
        fs::create_dir_all(blob_dir(&self.marengo_root))?;
        let mut archived = 0u32;
        for pattern in ["bench-*.log", "candump-*.log", "position-trace-*.csv"] {
            archived += self.archive_pattern(&hot, pattern, keep)?;
        }
        Ok(archived)
    }

    fn archive_pattern(&self, hot_dir: &Path, pattern: &str, keep: usize) -> Result<u32> {
        let mut files = list_timestamped_files(hot_dir, pattern)?;
        files.sort_by(|a, b| b.1.cmp(&a.1));
        let mut archived = 0u32;
        for (path, _mtime) in files.into_iter().skip(keep) {
            if path.is_symlink() {
                continue;
            }
            let session_id = extract_session_id(&path);
            let gz_path = self.gzip_to_blob(&path, &session_id)?;
            self.update_session_blob(&session_id, &path, &gz_path)?;
            if pattern.contains("candump") {
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

    fn update_session_blob(&self, session_id: &str, source: &Path, gz_path: &Path) -> Result<()> {
        let fname = source.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let col = if fname.starts_with("bench-") {
            "bench_blob"
        } else if fname.starts_with("candump-") {
            "candump_blob"
        } else if fname.starts_with("position-trace-") {
            "trace_blob"
        } else {
            return Ok(());
        };
        let sql = format!(
            "INSERT INTO log_sessions (id, label, started_ms, {col})
             VALUES (?1, NULL, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET {col} = excluded.{col}, ended_ms = ?4"
        );
        let started = session_id_to_ms(session_id).unwrap_or_else(now_ms);
        self.connection().execute(
            &sql,
            params![
                session_id,
                started as i64,
                gz_path.display().to_string(),
                now_ms() as i64
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

    pub fn import_legacy_hot(&self, keep: usize) -> Result<u32> {
        let hot = log_dir(&self.marengo_root);
        if !hot.is_dir() {
            return Ok(0);
        }
        let mut registered = 0u32;
        for entry in fs::read_dir(&hot)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_symlink() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(session_id) = extract_session_id_from_name(name) {
                let started = session_id_to_ms(&session_id).unwrap_or_else(now_ms);
                let label = None;
                let bench = if name.starts_with("bench-") {
                    Some(path.clone())
                } else {
                    None
                };
                let candump = if name.starts_with("candump-") {
                    Some(path.clone())
                } else {
                    None
                };
                let trace = if name.starts_with("position-trace-") {
                    Some(path.clone())
                } else {
                    None
                };
                if bench.is_some() || candump.is_some() || trace.is_some() {
                    self.register_session(
                        &session_id,
                        label,
                        started,
                        bench.as_deref(),
                        candump.as_deref(),
                        trace.as_deref(),
                    )?;
                    registered += 1;
                }
            }
        }
        self.archive_hot_sessions(keep)?;
        Ok(registered)
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

fn list_timestamped_files(dir: &Path, pattern: &str) -> Result<Vec<(PathBuf, i64)>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !matches_hot_pattern(name, pattern) {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        out.push((path, mtime));
    }
    Ok(out)
}

fn matches_hot_pattern(name: &str, pattern: &str) -> bool {
    if name.contains("latest") {
        return false;
    }
    match pattern {
        "bench-*.log" => name.starts_with("bench-") && name.ends_with(".log"),
        "candump-*.log" => name.starts_with("candump-") && name.ends_with(".log"),
        "position-trace-*.csv" => name.starts_with("position-trace-") && name.ends_with(".csv"),
        _ => false,
    }
}

fn extract_session_id(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(extract_session_id_from_name)
        .unwrap_or_else(|| "unknown".to_string())
}

fn extract_session_id_from_name(name: &str) -> Option<String> {
    for prefix in ["bench-", "candump-", "position-trace-"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            let id = rest
                .strip_suffix(".log")
                .or_else(|| rest.strip_suffix(".csv"))
                .unwrap_or(rest);
            return Some(id.to_string());
        }
    }
    None
}

fn session_id_to_date(session_id: &str) -> String {
    if session_id.len() >= 8 {
        format!(
            "{}-{}-{}",
            &session_id[0..4],
            &session_id[4..6],
            &session_id[6..8]
        )
    } else {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect()
    }
}

fn session_id_to_ms(session_id: &str) -> Option<u64> {
    let parsed =
        time::format_description::parse("[year][month][day]T[hour][minute][second]Z").ok()?;
    let dt = time::OffsetDateTime::parse(session_id, &parsed).ok()?;
    Some((dt.unix_timestamp_nanos() / 1_000_000) as u64)
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
