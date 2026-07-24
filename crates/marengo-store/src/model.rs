use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEventRow {
    pub id: i64,
    pub ts_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
    pub session_id: Option<String>,
    pub fields_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEventInsert {
    pub ts_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
    pub session_id: Option<String>,
    pub fields_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSessionRow {
    pub id: String,
    pub label: Option<String>,
    pub started_ms: u64,
    pub ended_ms: Option<u64>,
    pub bench_blob: Option<String>,
    pub candump_blob: Option<String>,
    pub trace_blob: Option<String>,
    pub candump_frame_count: Option<u64>,
    pub candump_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredLogQuery {
    pub from_ms: Option<u64>,
    pub to_ms: Option<u64>,
    pub level: Option<String>,
    pub target: Option<String>,
    pub session_id: Option<String>,
    pub q: Option<String>,
    pub offset: u32,
    pub limit: u32,
}

// Candump frame/summary types live in marengo-candump; re-exported from lib.rs.
