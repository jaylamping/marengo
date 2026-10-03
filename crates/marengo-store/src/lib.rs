//! Historical logs, capture/session metadata and operator settings in SQLite.
//!
//! Store owns its connection mutex; the private migration owner reserves SQLite's
//! writer before inspecting a version and commits each schema step with its marker.
//! Archive/file operations and candump inspection remain separate from migration
//! transactions. This crate does not authorize robot motion or repair arbitrary
//! damaged databases. Explicit known-v2 recovery preserves a verified backup and
//! publishes a separate output; callers own request scheduling and recovery consent.
//! Allowed dependencies are SQLite, compression/time/serialization and ordinary
//! storage utilities, plus marengo-candump inspection. No control-stack dependency.

mod disk;
mod error;
mod journal;
mod migrations;
mod model;
mod paths;
mod recovery;
mod retention;
mod store;

pub use disk::log_disk_usage_bytes;
pub use error::{Result, StoreError};
pub use journal::{import_journal, JOURNAL_UNITS};
pub use model::{
    LegacyImportSummary, LogEventInsert, LogEventRow, LogSessionRow, SessionArtifact,
    StructuredLogQuery,
};
pub use paths::{
    log_dir, resolve_db_path, resolve_marengo_root, DEFAULT_ARCHIVE_DAYS, DEFAULT_HOT_KEEP,
    DEFAULT_LOG_DISK_BUDGET_BYTES,
};
pub use recovery::{recover_known_v2, RecoveryReceipt};
pub use retention::{RetentionPolicy, RetentionReport};
pub use store::{now_ms, Store};
