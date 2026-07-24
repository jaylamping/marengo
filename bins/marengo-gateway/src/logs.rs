//! Log persistence, ring buffer, and HTTP handlers for marengo-gateway.

use std::sync::Arc;

use armee_proto::prost::Message;
use armee_proto::LogEvent;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use marengo_store::{
    LogEventInsert, LogRingBuffer, Store, StructuredLogQuery, DEFAULT_RING_CAPACITY,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::state::SharedState;

const BATCH_MAX: usize = 100;
const BATCH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
#[cfg(test)]
static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Upper bound on log inserts queued for the DB writer. Bounded (not unbounded)
/// so a runaway producer applies backpressure / drops instead of growing the
/// queue until the process is OOM-killed.
const BATCH_QUEUE_CAPACITY: usize = 16_384;

#[cfg(test)]
pub(crate) fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Serialize)]
pub struct StructuredLogListJson {
    entries: Vec<StructuredLogEntryJson>,
    total: u32,
}

#[derive(Serialize)]
pub struct StructuredLogEntryJson {
    id: u64,
    timestamp_ms: u64,
    level: String,
    target: String,
    message: String,
    session_id: String,
    fields_json: String,
}

#[derive(Serialize)]
pub struct LogSessionListJson {
    sessions: Vec<LogSessionMetaJson>,
}

#[derive(Serialize)]
pub struct LogSessionMetaJson {
    id: String,
    label: String,
    started_ms: u64,
    ended_ms: u64,
    has_bench: bool,
    has_candump: bool,
    has_trace: bool,
    candump_bytes: u64,
    candump_frame_count: u64,
}

#[derive(Serialize)]
pub struct CandumpPageJson {
    frames: Vec<CandumpFrameJson>,
    total_frames: u32,
    offset: u32,
}

#[derive(Serialize)]
pub struct CandumpFrameJson {
    delta_s: f64,
    interface: String,
    can_id: String,
    data: String,
    line_no: u32,
}

#[derive(Serialize)]
pub struct CandumpSummaryJson {
    frame_count: u32,
    bytes: u64,
    duration_s: f64,
    approx_hz: f64,
    interfaces: Vec<String>,
    top_ids: Vec<String>,
}

pub struct LogServices {
    pub store: Arc<Store>,
    pub ring: Arc<LogRingBuffer>,
    batch_tx: mpsc::Sender<LogEventInsert>,
    dropped: Arc<std::sync::atomic::AtomicU64>,
}

impl LogServices {
    pub fn open(store: Store) -> Self {
        let store = Arc::new(store);
        let ring = Arc::new(LogRingBuffer::new(DEFAULT_RING_CAPACITY));
        let (batch_tx, batch_rx) = mpsc::channel(BATCH_QUEUE_CAPACITY);
        let dropped = Arc::new(std::sync::atomic::AtomicU64::new(0));
        spawn_batch_writer(Arc::clone(&store), batch_rx);
        if let Ok(recent) = store.recent_log_events(DEFAULT_RING_CAPACITY as u32) {
            let preload: Vec<LogEventInsert> = recent
                .into_iter()
                .rev()
                .map(|row| LogEventInsert {
                    ts_ms: row.ts_ms,
                    level: row.level,
                    target: row.target,
                    message: row.message,
                    session_id: row.session_id,
                    fields_json: row.fields_json,
                })
                .collect();
            ring.preload(preload);
        }
        Self {
            store,
            ring,
            batch_tx,
            dropped,
        }
    }

    pub fn ingest_log_event(&self, event: &LogEvent) {
        let insert = LogEventInsert {
            ts_ms: event.timestamp_ms,
            level: event.level.clone(),
            target: event.target.clone(),
            message: event.message.clone(),
            session_id: if event.session_id.is_empty() {
                None
            } else {
                Some(event.session_id.clone())
            },
            fields_json: if event.fields_json.is_empty() {
                None
            } else {
                Some(event.fields_json.clone())
            },
        };
        self.ring.push(insert.clone());
        // Non-blocking: under a runaway producer we drop the DB write rather than
        // grow the queue unbounded. Deliberately not logged here — emitting a log
        // on the log-ingest path would feed straight back into this pipeline.
        if self.batch_tx.try_send(insert).is_err() {
            self.dropped
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Count of structured log inserts dropped due to DB-writer backpressure.
    pub fn dropped_log_inserts(&self) -> u64 {
        self.dropped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn spawn_batch_writer(store: Arc<Store>, mut rx: mpsc::Receiver<LogEventInsert>) {
    tokio::spawn(async move {
        let mut buf = Vec::with_capacity(BATCH_MAX);
        let mut interval = tokio::time::interval(BATCH_INTERVAL);
        loop {
            tokio::select! {
                item = rx.recv() => {
                    match item {
                        Some(entry) => {
                            buf.push(entry);
                            if buf.len() >= BATCH_MAX {
                                flush_batch_async(&store, &mut buf).await;
                            }
                        }
                        None => {
                            flush_batch_async(&store, &mut buf).await;
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    flush_batch_async(&store, &mut buf).await;
                }
            }
        }
    });
}

async fn flush_batch_async(store: &Arc<Store>, buf: &mut Vec<LogEventInsert>) {
    if buf.is_empty() {
        return;
    }
    let batch = std::mem::take(buf);
    let store = Arc::clone(store);
    let result = tokio::task::spawn_blocking(move || store.insert_log_events(&batch)).await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "log batch insert failed"),
        Err(e) => tracing::warn!(error = %e, "log batch insert task failed"),
    }
}

pub fn log_token_from_env() -> Option<String> {
    std::env::var("MARENGO_GATEWAY_LOG_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn authorize_logs(headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = log_token_from_env() else {
        return Ok(());
    };
    let provided = headers
        .get("x-marengo-log-token")
        .and_then(|v| v.to_str().ok());
    if provided == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Deserialize)]
pub struct RecentQuery {
    #[serde(default = "default_recent_limit")]
    limit: u32,
}

fn default_recent_limit() -> u32 {
    5000
}

pub async fn snapshot_logs_recent(
    State(state): State<SharedState>,
    Query(query): Query<RecentQuery>,
    headers: HeaderMap,
) -> Result<Json<StructuredLogListJson>, StatusCode> {
    authorize_logs(&headers)?;
    let limit = query.limit.clamp(1, 10_000);
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let rows = logs
        .store
        .recent_log_events(limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let entries: Vec<StructuredLogEntryJson> = rows
        .into_iter()
        .map(|e| StructuredLogEntryJson {
            id: e.id as u64,
            timestamp_ms: e.ts_ms,
            level: e.level,
            target: e.target,
            message: e.message,
            session_id: e.session_id.unwrap_or_default(),
            fields_json: e.fields_json.unwrap_or_default(),
        })
        .collect();
    let total = entries.len() as u32;
    Ok(Json(StructuredLogListJson { entries, total }))
}

#[derive(Deserialize)]
pub struct SessionsQuery {
    from_ms: Option<u64>,
    to_ms: Option<u64>,
    label: Option<String>,
    #[serde(default = "default_session_limit")]
    limit: u32,
}

fn default_session_limit() -> u32 {
    50
}

pub async fn list_sessions(
    State(state): State<SharedState>,
    Query(query): Query<SessionsQuery>,
    headers: HeaderMap,
) -> Result<Json<LogSessionListJson>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let rows = logs
        .store
        .list_sessions(
            query.from_ms,
            query.to_ms,
            query.label.as_deref(),
            query.limit,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let sessions = rows
        .into_iter()
        .map(|s| LogSessionMetaJson {
            id: s.id,
            label: s.label.unwrap_or_default(),
            started_ms: s.started_ms,
            ended_ms: s.ended_ms.unwrap_or(0),
            has_bench: s.bench_blob.is_some(),
            has_candump: s.candump_blob.is_some(),
            has_trace: s.trace_blob.is_some(),
            candump_bytes: s.candump_bytes.unwrap_or(0),
            candump_frame_count: s.candump_frame_count.unwrap_or(0),
        })
        .collect();
    Ok(Json(LogSessionListJson { sessions }))
}

#[derive(Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_page_limit")]
    limit: u32,
}

fn default_page_limit() -> u32 {
    200
}

pub async fn session_bench(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (lines, total) = logs
        .store
        .read_bench_page(&id, query.offset, query.limit)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({ "lines": lines, "total": total })))
}

pub async fn session_trace(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (lines, total) = logs
        .store
        .read_trace_page(&id, query.offset, query.limit)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({ "lines": lines, "total": total })))
}

pub async fn session_candump(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Json<CandumpPageJson>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (frames, total) = logs
        .store
        .read_candump_page(&id, query.offset, query.limit)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(CandumpPageJson {
        total_frames: u32::try_from(total).unwrap_or(u32::MAX),
        offset: query.offset,
        frames: frames
            .into_iter()
            .map(|f| CandumpFrameJson {
                delta_s: f.offset.as_secs_f64(),
                interface: f.interface,
                can_id: f.can_id.to_canonical_hex(),
                data: f.data.iter().map(|b| format!("{b:02X}")).collect(),
                line_no: u32::try_from(f.source_line.get()).unwrap_or(u32::MAX),
            })
            .collect(),
    }))
}

pub async fn latest_candump(
    State(state): State<SharedState>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Json<CandumpPageJson>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (frames, total) = logs
        .store
        .read_hot_candump_page(query.offset, query.limit)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(CandumpPageJson {
        total_frames: u32::try_from(total).unwrap_or(u32::MAX),
        offset: query.offset,
        frames: frames
            .into_iter()
            .map(|f| CandumpFrameJson {
                delta_s: f.offset.as_secs_f64(),
                interface: f.interface,
                can_id: f.can_id.to_canonical_hex(),
                data: f.data.iter().map(|b| format!("{b:02X}")).collect(),
                line_no: u32::try_from(f.source_line.get()).unwrap_or(u32::MAX),
            })
            .collect(),
    }))
}

pub async fn session_candump_summary(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<CandumpSummaryJson>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let summary = logs
        .store
        .candump_summary(&id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(CandumpSummaryJson {
        frame_count: u32::try_from(summary.parsed_frames).unwrap_or(u32::MAX),
        bytes: summary.source_bytes,
        duration_s: summary.duration_s,
        approx_hz: summary.approx_hz.unwrap_or(0.0),
        interfaces: summary.interfaces.into_iter().map(|i| i.name).collect(),
        top_ids: summary
            .top_ids
            .into_iter()
            .map(|c| c.can_id.to_canonical_hex())
            .collect(),
    }))
}

#[derive(Deserialize)]
pub struct StructuredQuery {
    from_ms: Option<u64>,
    to_ms: Option<u64>,
    level: Option<String>,
    target: Option<String>,
    session_id: Option<String>,
    q: Option<String>,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_page_limit")]
    limit: u32,
}

pub async fn structured_logs(
    State(state): State<SharedState>,
    Query(query): Query<StructuredQuery>,
    headers: HeaderMap,
) -> Result<Json<StructuredLogListJson>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (rows, total) = logs
        .store
        .query_structured_logs(&StructuredLogQuery {
            from_ms: query.from_ms,
            to_ms: query.to_ms,
            level: query.level,
            target: query.target,
            session_id: query.session_id,
            q: query.q,
            offset: query.offset,
            limit: query.limit,
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let entries = rows
        .into_iter()
        .map(|e| StructuredLogEntryJson {
            id: e.id as u64,
            timestamp_ms: e.ts_ms,
            level: e.level,
            target: e.target,
            message: e.message,
            session_id: e.session_id.unwrap_or_default(),
            fields_json: e.fields_json.unwrap_or_default(),
        })
        .collect();
    Ok(Json(StructuredLogListJson { entries, total }))
}

#[derive(Serialize)]
pub struct SettingsResponse {
    settings: std::collections::HashMap<String, String>,
}

pub async fn get_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let mut settings = std::collections::HashMap::new();
    for key in [
        "schema_version",
        "log_archive_days",
        "log_disk_budget_bytes",
    ] {
        if let Ok(Some(val)) = logs.store.get_setting(key) {
            settings.insert(key.to_string(), val);
        }
    }
    Ok(Json(SettingsResponse { settings }))
}

pub async fn session_download(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    authorize_logs(&headers)?;
    let logs = state.logs.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let session = logs
        .store
        .get_session(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let path = match query.kind.as_str() {
        "candump" => session.candump_blob,
        "bench" => session.bench_blob,
        "trace" => session.trace_blob,
        _ => None,
    }
    .ok_or(StatusCode::NOT_FOUND)?;
    let bytes = std::fs::read(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/gzip")],
        bytes,
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    #[serde(rename = "type")]
    kind: String,
}

pub fn decode_log_payload(payload: &[u8]) -> Option<LogEvent> {
    let env = armee_proto::Envelope::decode(payload).ok()?;
    LogEvent::decode(env.payload.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_limit_default() {
        assert_eq!(default_recent_limit(), 5000);
    }

    #[test]
    fn log_token_from_env_filters_empty() {
        let _env = lock_test_env();
        const KEY: &str = "MARENGO_GATEWAY_LOG_TOKEN";
        let saved = std::env::var(KEY).ok();
        std::env::set_var(KEY, "");
        assert!(log_token_from_env().is_none());
        std::env::set_var(KEY, "secret");
        assert_eq!(log_token_from_env().as_deref(), Some("secret"));
        match saved {
            Some(v) => std::env::set_var(KEY, v),
            None => std::env::remove_var(KEY),
        }
    }
}
