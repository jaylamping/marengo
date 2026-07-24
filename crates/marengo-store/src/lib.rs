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
pub use model::{LogEventInsert, LogEventRow, LogSessionRow, StructuredLogQuery};
pub use paths::{
    blob_dir, default_db_path, log_dir, resolve_db_path, resolve_marengo_root,
    DEFAULT_ARCHIVE_DAYS, DEFAULT_HOT_KEEP, DEFAULT_LOG_DISK_BUDGET_BYTES,
};
pub use ring::{LogRingBuffer, DEFAULT_RING_CAPACITY};
pub use store::{now_ms, Store};
