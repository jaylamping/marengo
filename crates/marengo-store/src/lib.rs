//! Historical logs, capture/session metadata and operator settings in SQLite.
//!
//! Store owns its connection mutex; the private migration owner reserves SQLite's
//! writer before inspecting a version and commits each schema step with its marker.
//! Archive/file operations and candump inspection remain separate from migration
//! transactions. This crate does not authorize robot motion or repair arbitrary
//! damaged databases; callers own request scheduling and historic recovery policy.
//! Allowed dependencies are SQLite, compression/time/serialization and ordinary
//! storage utilities, plus marengo-candump inspection. No control-stack dependency.

mod error;
mod journal;
mod migrations;
mod model;
mod paths;
mod ring;
mod store;

pub use error::{Result, StoreError};
pub use journal::{import_journal, JOURNAL_UNITS};
pub use marengo_candump::{Frame as CandumpFrame, Summary as CandumpSummary};
pub use model::{
    LegacyImportSummary, LogEventInsert, LogEventRow, LogSessionRow, SessionArtifact,
    StructuredLogQuery,
};
pub use paths::{
    blob_dir, default_db_path, log_dir, resolve_db_path, resolve_marengo_root,
    DEFAULT_ARCHIVE_DAYS, DEFAULT_HOT_KEEP, DEFAULT_LOG_DISK_BUDGET_BYTES,
};
pub use ring::{LogRingBuffer, DEFAULT_RING_CAPACITY};
pub use store::{now_ms, Store};
