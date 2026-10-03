//! Concrete bounded noncoalescing SQLite worker. No caller-supplied writer.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};

use super::reference_codec::BODY_CAPACITY;
use super::reference_journal_event::{Body, Input, ReferenceHistoryRecord};

pub(super) const CREDIT_CAPACITY: usize = 8;
const EVENT_CAPACITY: i64 = 4096;
const PAGE_CAPACITY: i64 = 16_384;
const DATABASE_CAPACITY: u64 = 64 * 1024 * 1024;
// With cache_spill=OFF, rollback has one sector header and at most the capped
// existing pages plus eight-byte page-number/checksum overhead. The pinned pager caps sectors at
// 65536 bytes. This bound is distinct from journal_size_limit (a retention cap).
const ROLLBACK_CAPACITY: u64 = 65 * 1024 * 1024;
const APPLICATION_ID: i64 = 0x4d525246;
const META_SQL: &str = "CREATE TABLE reference_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1), version INTEGER NOT NULL CHECK(version=1), next_session INTEGER NOT NULL CHECK(next_session>=0))";
const EVENTS_SQL: &str = "CREATE TABLE reference_events(session INTEGER NOT NULL CHECK(session>0), job BLOB NOT NULL CHECK(typeof(job)='blob' AND length(job)=8), body BLOB NOT NULL CHECK(typeof(body)='blob' AND length(body) BETWEEN 1 AND 1048576), checksum BLOB NOT NULL CHECK(typeof(checksum)='blob' AND length(checksum)=32), PRIMARY KEY(session,job)) WITHOUT ROWID";

#[derive(Debug, thiserror::Error)]
#[error("reference journal: {message}")]
pub struct ReferenceJournalError {
    message: String,
}
fn error(message: impl std::fmt::Display) -> ReferenceJournalError {
    ReferenceJournalError {
        message: crate::bounded_message(&message.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceJournalResult {
    Pending,
    DurableHistory {
        diagnostic_session: u64,
        job_sequence: u64,
        checksum: [u8; 32],
    },
    Failed {
        message: String,
    },
    Uncertain {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceJournalDrain {
    pub admission_closed: bool,
    pub queued: usize,
    pub in_flight: bool,
    pub completed_unconsumed: usize,
    pub accepted_credits: usize,
    pub durable_writes: u64,
    pub failed_writes: u64,
    pub uncertain_writes: u64,
    pub worker_terminated: bool,
    pub worker_error: Option<String>,
}
impl ReferenceJournalDrain {
    pub fn is_complete(&self) -> bool {
        self.worker_terminated
            && self.queued == 0
            && !self.in_flight
            && self.completed_unconsumed == 0
            && self.accepted_credits == 0
    }
    pub(super) fn absent() -> Self {
        Self {
            admission_closed: true,
            queued: 0,
            in_flight: false,
            completed_unconsumed: 0,
            accepted_credits: 0,
            durable_writes: 0,
            failed_writes: 0,
            uncertain_writes: 0,
            worker_terminated: true,
            worker_error: None,
        }
    }
}

/// The nonce never appears in disk data or a public snapshot.
#[derive(Clone)]
pub(super) struct JobIdentity {
    pub(super) nonce: Arc<()>,
    pub(super) sequence: u64,
}
impl JobIdentity {
    fn matches(&self, other: &Self) -> bool {
        self.sequence == other.sequence && Arc::ptr_eq(&self.nonce, &other.nonce)
    }
}
struct Job {
    identity: JobIdentity,
    input: Input,
}
pub(super) struct Completed {
    pub(super) identity: JobIdentity,
    pub(super) result: ReferenceJournalResult,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Progress {
    Opening,
    Encoding,
    Writing,
    CommitAttempted,
    Committed,
}
struct Flight {
    identity: JobIdentity,
    progress: Progress,
}
#[derive(Default)]
struct State {
    closed: bool,
    queued: VecDeque<Job>,
    flight: Option<Flight>,
    completed: VecDeque<Completed>,
    credits: usize,
    durable: u64,
    failed: u64,
    uncertain: u64,
    finished: bool,
    worker_error: Option<String>,
}
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poison| {
            let mut state = poison.into_inner();
            state.closed = true;
            state
                .worker_error
                .get_or_insert_with(|| "journal state lock poisoned".into());
            state
        })
    }
    fn progress(&self, progress: Progress) {
        if let Some(flight) = &mut self.lock().flight {
            flight.progress = progress;
        }
    }
    fn complete(&self, identity: JobIdentity, result: ReferenceJournalResult) {
        let mut state = self.lock();
        match &result {
            ReferenceJournalResult::DurableHistory { .. } => {
                state.durable = state.durable.saturating_add(1)
            }
            ReferenceJournalResult::Failed { .. } => state.failed = state.failed.saturating_add(1),
            ReferenceJournalResult::Uncertain { .. } => {
                state.uncertain = state.uncertain.saturating_add(1)
            }
            ReferenceJournalResult::Pending => {}
        }
        // Admission reserved this slot, including after owner cancellation.
        state.completed.push_back(Completed { identity, result });
        state.flight = None;
        self.changed.notify_all();
    }
}

pub(super) struct Journal {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Journal {
    pub(super) fn spawn(
        path: PathBuf,
        #[cfg(any(test, feature = "reference-journal-test-support"))] pause: Option<
            test_support::WorkerPause,
        >,
    ) -> Result<Self, ReferenceJournalError> {
        checked_absolute_path(&path)?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let owner = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("reference-journal".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker_loop(
                        &owner,
                        &path,
                        #[cfg(any(test, feature = "reference-journal-test-support"))]
                        pause.as_ref(),
                    );
                }));
                if result.is_err() {
                    let mut state = owner.lock();
                    state.closed = true;
                    state.worker_error = Some("reference journal worker unwound".into());
                    if let Some(flight) = state.flight.take() {
                        let result = if flight.progress >= Progress::Writing {
                            ReferenceJournalResult::Uncertain {
                                message: "worker unwound during/after transaction".into(),
                            }
                        } else {
                            ReferenceJournalResult::Failed {
                                message: "worker unwound before event transaction".into(),
                            }
                        };
                        drop(state);
                        owner.complete(flight.identity, result);
                        state = owner.lock();
                    }
                    while let Some(job) = state.queued.pop_front() {
                        drop(state);
                        owner.complete(
                            job.identity,
                            ReferenceJournalResult::Failed {
                                message: "worker terminated before queued event".into(),
                            },
                        );
                        state = owner.lock();
                    }
                }
                owner.lock().finished = true;
                owner.changed.notify_all();
            })
            .map_err(error)?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub(super) fn submit(
        &self,
        identity: JobIdentity,
        input: Input,
    ) -> Result<(), ReferenceJournalError> {
        let mut state = self.shared.lock();
        if state.closed || state.finished || state.worker_error.is_some() {
            return Err(error("journal admission closed"));
        }
        if state.credits == CREDIT_CAPACITY {
            return Err(error("journal completion credits full"));
        }
        state.credits += 1;
        state.queued.push_back(Job { identity, input });
        self.shared.changed.notify_all();
        Ok(())
    }
    pub(super) fn has_credit(&self) -> Result<bool, ReferenceJournalError> {
        let state = self.shared.lock();
        if state.closed || state.finished || state.worker_error.is_some() {
            return Err(error("journal admission closed"));
        }
        Ok(state.credits < CREDIT_CAPACITY)
    }
    pub(super) fn take_matching(&self, identity: &JobIdentity) -> Option<Completed> {
        let mut state = self.shared.lock();
        let index = state
            .completed
            .iter()
            .position(|item| item.identity.matches(identity))?;
        let completed = state.completed.remove(index)?;
        state.credits = state.credits.saturating_sub(1);
        self.shared.changed.notify_all();
        Some(completed)
    }
    pub(super) fn close(&self) {
        self.shared.lock().closed = true;
        self.shared.changed.notify_all();
    }
    fn join_finished(&mut self) -> bool {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            return false;
        }
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                self.shared
                    .lock()
                    .worker_error
                    .get_or_insert_with(|| "journal thread panicked".into());
            }
        }
        true
    }
    pub(super) fn wait_until(&mut self, deadline: Instant) -> ReferenceJournalDrain {
        self.close();
        loop {
            let terminated = self.join_finished();
            let state = self.shared.lock();
            let remaining = deadline.saturating_duration_since(Instant::now());
            if terminated || remaining.is_zero() {
                return ReferenceJournalDrain {
                    admission_closed: state.closed,
                    queued: state.queued.len(),
                    in_flight: state.flight.is_some(),
                    completed_unconsumed: state.completed.len(),
                    accepted_credits: state.credits,
                    durable_writes: state.durable,
                    failed_writes: state.failed,
                    uncertain_writes: state.uncertain,
                    worker_terminated: terminated,
                    worker_error: state.worker_error.clone(),
                };
            }
            let wait = if state.finished {
                remaining.min(Duration::from_millis(1))
            } else {
                remaining
            };
            let result = self.shared.changed.wait_timeout(state, wait);
            if let Err(poison) = result {
                let (mut state, _) = poison.into_inner();
                state.closed = true;
                state
                    .worker_error
                    .get_or_insert_with(|| "journal wait lock poisoned".into());
            }
        }
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        self.close();
    }
}

fn worker_loop(
    shared: &Shared,
    path: &Path,
    #[cfg(any(test, feature = "reference-journal-test-support"))] pause: Option<
        &test_support::WorkerPause,
    >,
) {
    let mut database: Option<Database> = None;
    loop {
        let job = {
            let mut state = shared.lock();
            while state.queued.is_empty() && !state.closed {
                state = shared
                    .changed
                    .wait(state)
                    .unwrap_or_else(|poison| poison.into_inner());
            }
            let Some(job) = state.queued.pop_front() else {
                return;
            };
            state.flight = Some(Flight {
                identity: job.identity.clone(),
                progress: Progress::Opening,
            });
            job
        };
        #[cfg(any(test, feature = "reference-journal-test-support"))]
        test_support::at(pause, test_support::JournalPausePoint::BeforeOpen);
        let result = (|| {
            if database.is_none() {
                database = Some(Database::open(path, true, true).map_err(|error| {
                    ReferenceJournalResult::Failed {
                        message: error.to_string(),
                    }
                })?);
            }
            let db = database
                .as_mut()
                .ok_or_else(|| ReferenceJournalResult::Failed {
                    message: "journal unavailable".into(),
                })?;
            shared.progress(Progress::Encoding);
            let body = job
                .input
                .encode(db.session, job.identity.sequence)
                .map_err(|error| ReferenceJournalResult::Failed {
                    message: error.to_string(),
                })?;
            db.append(
                job.identity.sequence,
                &body,
                shared,
                #[cfg(any(test, feature = "reference-journal-test-support"))]
                pause,
            )
        })();
        let result = match result {
            Ok(result) => result,
            Err(result) => {
                database = None;
                result
            }
        };
        #[cfg(any(test, feature = "reference-journal-test-support"))]
        test_support::at(pause, test_support::JournalPausePoint::BeforePublication);
        shared.complete(job.identity, result);
        #[cfg(any(test, feature = "reference-journal-test-support"))]
        test_support::at(pause, test_support::JournalPausePoint::AfterPublication);
    }
}

fn checked_absolute_path(path: &Path) -> Result<(), ReferenceJournalError> {
    if !path.is_absolute()
        || path.as_os_str().as_encoded_bytes().len() > 4096
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(error("explicit absolute journal path required"));
    }
    Ok(())
}

/// Resolve existing ancestors without creating either explicitly supplied resource.
/// Retain the history's absolute spelling so later work cannot reinterpret its cwd.
pub(super) fn distinct_history_path(
    history: &Path,
    journal: &Path,
) -> Result<PathBuf, ReferenceJournalError> {
    checked_absolute_path(journal)?;
    let history = if history.is_absolute() {
        history.to_owned()
    } else {
        std::env::current_dir().map_err(error)?.join(history)
    };
    if history.as_os_str().as_encoded_bytes().len() > 4096 {
        return Err(error("bounded explicit history path required"));
    }
    let history_is_regular = match existing_metadata(&history)? {
        Some(metadata) if metadata.is_file() => true,
        Some(_) => return Err(error("existing history must be a regular file")),
        None => false,
    };
    let history_slot = canonical_slot(&history)?;
    let history_folded = history_slot.as_os_str().to_string_lossy().to_lowercase();
    // Conservatively refuse case-only spellings too, including on platforms
    // where distinct missing names would resolve to one case-insensitive slot.
    // SQLite owns its sidecars as well as the main file, including deletion of
    // remnant rollback files. None may occupy the supplied calibration slot.
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let mut resource = journal.as_os_str().to_os_string();
        resource.push(suffix);
        let resource = PathBuf::from(resource);
        let journal_slot = canonical_slot(&resource)?;
        if history_slot == journal_slot
            || history_folded == journal_slot.as_os_str().to_string_lossy().to_lowercase()
        {
            return Err(error("history and journal paths must be distinct"));
        }
        // Canonical filenames do not identify existing hard links. Use the
        // portable file identity already pinned by the workspace, and preserve
        // all errors except genuine absence. This opens no SQLite connection.
        // same-file opens paths. Never give it a symlink or special file that
        // could block a read-only identity open. Unsupported journal resources
        // keep their lazy worker refusal rather than becoming an early I/O job.
        let journal_is_regular = existing_metadata(&resource)?.is_some_and(|meta| meta.is_file());
        if history_is_regular && journal_is_regular {
            match same_file::is_same_file(&history, &resource) {
                Ok(true) => return Err(error("history and journal paths must be distinct")),
                Ok(false) => {}
                Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
                Err(cause) => return Err(error(cause)),
            }
        }
    }
    Ok(history)
}

fn existing_metadata(path: &Path) -> Result<Option<fs::Metadata>, ReferenceJournalError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(error(cause)),
    }
}

fn canonical_slot(path: &Path) -> Result<PathBuf, ReferenceJournalError> {
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match ancestor.canonicalize() {
            Ok(mut resolved) => {
                for filename in missing.into_iter().rev() {
                    resolved.push(filename);
                }
                return Ok(resolved);
            }
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| error("resource ancestor cannot be resolved"))?
                        .to_os_string(),
                );
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| error("resource parent absent"))?;
            }
            Err(cause) => return Err(error(cause)),
        }
    }
}

fn checked_resource(
    path: &Path,
    allow_create: bool,
) -> Result<(PathBuf, bool), ReferenceJournalError> {
    checked_absolute_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| error("journal parent absent"))?;
    let metadata = fs::symlink_metadata(parent).map_err(error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(error("journal parent must be a real directory"));
    }
    // Resolve the explicitly named directory once, including normal platform
    // namespace aliases, then use its canonical path for every SQLite operation.
    let parent = parent.canonicalize().map_err(error)?;
    let filename = path
        .file_name()
        .ok_or_else(|| error("journal filename absent"))?;
    let actual = parent.join(filename);
    for suffix in ["-journal", "-wal", "-shm"] {
        let mut sidecar = actual.as_os_str().to_os_string();
        sidecar.push(suffix);
        match fs::symlink_metadata(PathBuf::from(sidecar)) {
            Ok(metadata)
                if suffix == "-journal"
                    && !metadata.file_type().is_symlink()
                    && metadata.is_file()
                    && metadata.len() <= ROLLBACK_CAPACITY => {}
            Ok(_) => return Err(error("unsupported journal sidecar resource")),
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
            Err(cause) => return Err(error(cause)),
        }
    }
    let created = match fs::symlink_metadata(&actual) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() == 0
                || metadata.len() > DATABASE_CAPACITY
            {
                return Err(error("journal is not a supported bounded existing file"));
            }
            false
        }
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound && allow_create => {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            drop(options.open(&actual).map_err(error)?);
            true
        }
        Err(cause) => return Err(error(cause)),
    };
    // SQLite, not application cleanup, owns valid hot-journal recovery.
    Ok((actual, created))
}

struct Database {
    connection: Connection,
    session: u64,
}
impl Database {
    fn open(
        path: &Path,
        allow_create: bool,
        allocate_session: bool,
    ) -> Result<Self, ReferenceJournalError> {
        let (path, created) = checked_resource(path, allow_create)?;
        if !created {
            // SQLite can create WAL sidecars or change persistent mode while
            // opening/querying an incompatible database. Refuse its format
            // using only a bounded file read before giving SQLite the path.
            let mut header = [0_u8; 20];
            fs::File::open(&path)
                .map_err(error)?
                .read_exact(&mut header)
                .map_err(error)?;
            if &header[..16] != b"SQLite format 3\0" || header[18..20] != [1, 1] {
                return Err(error("unsupported existing journal format"));
            }
        }
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(error)?;
        connection.busy_timeout(Duration::ZERO).map_err(error)?;
        connection.set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            (BODY_CAPACITY + 4096) as i32,
        );
        connection.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH, 8192);
        if !created {
            Self::schema(&connection)?;
        }
        if created {
            connection
                .pragma_update(None, "page_size", 4096)
                .map_err(error)?;
        }
        let pages: i64 = connection
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .map_err(error)?;
        let page_size: i64 = connection
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .map_err(error)?;
        if pages > PAGE_CAPACITY || page_size != 4096 {
            return Err(error("journal page size/capacity mismatch"));
        }
        connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA locking_mode=EXCLUSIVE; PRAGMA cache_size=-512; PRAGMA cache_spill=OFF; PRAGMA mmap_size=0; PRAGMA temp_store=MEMORY; PRAGMA trusted_schema=OFF; PRAGMA max_page_count=16384; PRAGMA journal_size_limit=68157440;").map_err(error)?;
        for (name, expected) in [
            ("synchronous", 3),
            ("cache_size", -512),
            ("cache_spill", 0),
            ("mmap_size", 0),
            ("temp_store", 2),
            ("trusted_schema", 0),
            ("max_page_count", PAGE_CAPACITY),
            ("journal_size_limit", ROLLBACK_CAPACITY as i64),
            ("busy_timeout", 0),
        ] {
            let actual: i64 = connection
                .pragma_query_value(None, name, |row| row.get(0))
                .map_err(error)?;
            if actual != expected {
                return Err(error(format!("journal pragma {name} mismatch")));
            }
        }
        for (name, expected) in [("journal_mode", "delete"), ("locking_mode", "exclusive")] {
            let actual: String = connection
                .pragma_query_value(None, name, |row| row.get(0))
                .map_err(error)?;
            if actual != expected {
                return Err(error(format!("journal pragma {name} mismatch")));
            }
        }
        // A pragma alone does not acquire the lock. This real transaction does.
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(error)?;
        if created {
            tx.execute_batch(&format!("{META_SQL}; {EVENTS_SQL}; INSERT INTO reference_meta VALUES(1,1,0); PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version=1;")).map_err(error)?;
        }
        tx.commit().map_err(error)?;
        Self::schema(&connection)?;
        let integrity: String = connection
            .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
            .map_err(error)?;
        if integrity != "ok" {
            return Err(error("journal integrity failure"));
        }
        Self::validate_history(&connection)?;
        let session = if allocate_session {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Exclusive)
                .map_err(error)?;
            let current: i64 = tx
                .query_row(
                    "SELECT next_session FROM reference_meta WHERE singleton=1 AND version=1",
                    [],
                    |row| row.get(0),
                )
                .map_err(error)?;
            let next = current
                .checked_add(1)
                .filter(|n| *n > 0)
                .ok_or_else(|| error("diagnostic session exhausted"))?;
            if tx
                .execute(
                    "UPDATE reference_meta SET next_session=?1 WHERE singleton=1",
                    [next],
                )
                .map_err(error)?
                != 1
            {
                return Err(error("diagnostic session row absent"));
            }
            tx.commit().map_err(error)?;
            next as u64
        } else {
            0
        };
        Ok(Self {
            connection,
            session,
        })
    }

    fn schema(connection: &Connection) -> Result<(), ReferenceJournalError> {
        let id: i64 = connection
            .pragma_query_value(None, "application_id", |row| row.get(0))
            .map_err(error)?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(error)?;
        if id != APPLICATION_ID || version != 1 {
            return Err(error("incompatible journal identity/version"));
        }
        let mut query = connection
            .prepare(
                "SELECT name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .map_err(error)?;
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(error)?;
        let mut actual = Vec::new();
        for row in rows {
            actual.push(row.map_err(error)?);
            if actual.len() > 2 {
                return Err(error("unexpected journal schema object"));
            }
        }
        if actual
            != [
                ("reference_events".into(), EVENTS_SQL.into()),
                ("reference_meta".into(), META_SQL.into()),
            ]
        {
            return Err(error("exact journal schema mismatch"));
        }
        let count: i64 = connection
            .query_row("SELECT count(*) FROM reference_meta", [], |row| row.get(0))
            .map_err(error)?;
        let valid: i64 = connection.query_row("SELECT count(*) FROM reference_meta WHERE singleton=1 AND version=1 AND next_session>=0", [], |row| row.get(0)).map_err(error)?;
        if count != 1 || valid != 1 {
            return Err(error("journal metadata mismatch"));
        }
        Ok(())
    }

    fn validate_history(connection: &Connection) -> Result<(), ReferenceJournalError> {
        let count: i64 = connection
            .query_row("SELECT count(*) FROM reference_events", [], |row| {
                row.get(0)
            })
            .map_err(error)?;
        if count > EVENT_CAPACITY {
            return Err(error("journal event capacity"));
        }
        let session: i64 = connection
            .query_row("SELECT next_session FROM reference_meta", [], |row| {
                row.get(0)
            })
            .map_err(error)?;
        let mut query = connection.prepare("SELECT session,job,length(body),length(checksum),body,checksum FROM reference_events ORDER BY session,job").map_err(error)?;
        let mut rows = query.query([]).map_err(error)?;
        while let Some(row) = rows.next().map_err(error)? {
            let row_session: i64 = row.get(0).map_err(error)?;
            if row_session <= 0 || row_session > session {
                return Err(error("history session mismatch"));
            }
            let body_length: i64 = row.get(2).map_err(error)?;
            let digest_length: i64 = row.get(3).map_err(error)?;
            if !(1..=BODY_CAPACITY as i64).contains(&body_length) || digest_length != 32 {
                return Err(error("history body length"));
            }
            let job: Vec<u8> = row.get(1).map_err(error)?;
            let body: Vec<u8> = row.get(4).map_err(error)?;
            let checksum: Vec<u8> = row.get(5).map_err(error)?;
            // History integrity must not depend on the current config schema:
            // rows written by older binaries carry retired keys, and the full
            // typed body no longer decodes. Integrity (checksum, row-key
            // identity, session bounds, lengths) uses an identity-only view
            // that skips the policy, robot and URDF payloads. History never
            // grants, so payload semantics are not rechecked here.
            checked_history_row(row_session as u64, &job, &body, &checksum)?;
        }
        Ok(())
    }

    fn append(
        &mut self,
        sequence: u64,
        body: &[u8],
        shared: &Shared,
        #[cfg(any(test, feature = "reference-journal-test-support"))] pause: Option<
            &test_support::WorkerPause,
        >,
    ) -> Result<ReferenceJournalResult, ReferenceJournalResult> {
        let checksum: [u8; 32] = Sha256::digest(body).into();
        let key = sequence.to_be_bytes();
        checked_body(self.session, &key, body, &checksum).map_err(|cause| {
            ReferenceJournalResult::Failed {
                message: cause.to_string(),
            }
        })?;
        shared.progress(Progress::Writing);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(|cause| ReferenceJournalResult::Failed {
                message: cause.to_string(),
            })?;
        let insertion = (|| -> Result<(), ReferenceJournalError> {
            let existing: Option<(Vec<u8>, Vec<u8>)> = tx
                .query_row(
                    "SELECT body,checksum FROM reference_events WHERE session=?1 AND job=?2",
                    params![self.session as i64, key.as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(error)?;
            if let Some((prior, prior_checksum)) = existing {
                if prior != body || prior_checksum != checksum {
                    return Err(error("journal key/body conflict"));
                }
            } else {
                let count: i64 = tx
                    .query_row("SELECT count(*) FROM reference_events", [], |row| {
                        row.get(0)
                    })
                    .map_err(error)?;
                if count >= EVENT_CAPACITY {
                    return Err(error("journal event capacity reached"));
                }
                tx.execute(
                    "INSERT INTO reference_events(session,job,body,checksum) VALUES(?1,?2,?3,?4)",
                    params![
                        self.session as i64,
                        key.as_slice(),
                        body,
                        checksum.as_slice()
                    ],
                )
                .map_err(error)?;
            }
            Ok(())
        })();
        if let Err(cause) = insertion {
            return Err(match tx.rollback() {
                Ok(()) => ReferenceJournalResult::Failed {
                    message: cause.to_string(),
                },
                Err(rollback) => ReferenceJournalResult::Uncertain {
                    message: format!("{cause}; rollback: {rollback}"),
                },
            });
        }
        #[cfg(any(test, feature = "reference-journal-test-support"))]
        test_support::at(pause, test_support::JournalPausePoint::BeforeCommit);
        shared.progress(Progress::CommitAttempted);
        tx.commit()
            .map_err(|cause| ReferenceJournalResult::Uncertain {
                message: cause.to_string(),
            })?;
        shared.progress(Progress::Committed);
        #[cfg(any(test, feature = "reference-journal-test-support"))]
        test_support::at(pause, test_support::JournalPausePoint::AfterCommit);
        let readback: (Vec<u8>, Vec<u8>) = self
            .connection
            .query_row(
                "SELECT body,checksum FROM reference_events WHERE session=?1 AND job=?2",
                params![self.session as i64, key.as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|cause| ReferenceJournalResult::Uncertain {
                message: cause.to_string(),
            })?;
        if readback.0 != body || readback.1 != checksum {
            return Err(ReferenceJournalResult::Uncertain {
                message: "committed event readback mismatch".into(),
            });
        }
        checked_body(self.session, &key, &readback.0, &readback.1).map_err(|cause| {
            ReferenceJournalResult::Uncertain {
                message: cause.to_string(),
            }
        })?;
        Ok(ReferenceJournalResult::DurableHistory {
            diagnostic_session: self.session,
            job_sequence: sequence,
            checksum,
        })
    }
}

fn checked_history_row(
    session: u64,
    job: &[u8],
    body: &[u8],
    checksum: &[u8],
) -> Result<(), ReferenceJournalError> {
    let job: [u8; 8] = job.try_into().map_err(|_| error("job key width"))?;
    let digest: [u8; 32] = checksum.try_into().map_err(|_| error("checksum width"))?;
    if <[u8; 32]>::from(Sha256::digest(body)) != digest {
        return Err(error("body checksum mismatch"));
    }
    let identity = super::reference_codec::decode_identity(body).map_err(error)?;
    if !identity.matches(session, u64::from_be_bytes(job)) {
        return Err(error("event identity/body mismatch"));
    }
    Ok(())
}

fn checked_body(
    session: u64,
    job: &[u8],
    body: &[u8],
    checksum: &[u8],
) -> Result<(), ReferenceJournalError> {
    let job: [u8; 8] = job.try_into().map_err(|_| error("job key width"))?;
    let digest: [u8; 32] = checksum.try_into().map_err(|_| error("checksum width"))?;
    if <[u8; 32]>::from(Sha256::digest(body)) != digest {
        return Err(error("body checksum mismatch"));
    }
    let parsed: Body = super::reference_codec::decode(body).map_err(error)?;
    if !parsed.validate()
        || parsed.diagnostic_session != session
        || parsed.job_sequence != u64::from_be_bytes(job)
    {
        return Err(error("event identity/body mismatch"));
    }
    Ok(())
}

pub(super) fn inspect(
    path: &Path,
    limit: usize,
) -> Result<Vec<ReferenceHistoryRecord>, ReferenceJournalError> {
    if !(1..=CREDIT_CAPACITY).contains(&limit) {
        return Err(error("history inspection limit must be 1..=8"));
    }
    let database = Database::open(path, false, false)?;
    let mut query = database.connection.prepare("SELECT session,job,length(body),length(checksum),body,checksum FROM reference_events ORDER BY session DESC,job DESC LIMIT ?1").map_err(error)?;
    let mut rows = query.query([limit as i64]).map_err(error)?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(error)? {
        let length: i64 = row.get(2).map_err(error)?;
        if !(1..=BODY_CAPACITY as i64).contains(&length)
            || row.get::<_, i64>(3).map_err(error)? != 32
        {
            return Err(error("history body length"));
        }
        let body: Vec<u8> = row.get(4).map_err(error)?;
        let checksum: Vec<u8> = row.get(5).map_err(error)?;
        let digest: [u8; 32] = checksum
            .as_slice()
            .try_into()
            .map_err(|_| error("checksum width"))?;
        let row_session: i64 = row.get(0).map_err(error)?;
        let job: Vec<u8> = row.get(1).map_err(error)?;
        // Inspection stays honest about schema drift: a row whose body no
        // longer decodes under the current typed schema is reported as a
        // legacy record instead of failing the whole listing. Identity and
        // envelope still decode exactly, so corruption keeps failing closed.
        let record = match ReferenceHistoryRecord::from_body(&body, digest) {
            Ok(record) => record,
            Err(_) => {
                checked_history_row(row_session as u64, &job, &body, &checksum)?;
                let identity = super::reference_codec::decode_identity(&body).map_err(error)?;
                ReferenceHistoryRecord::legacy(
                    identity.diagnostic_session,
                    identity.job_sequence,
                    digest,
                )
            }
        };
        records.push(record);
    }
    Ok(records)
}

#[cfg(any(test, feature = "reference-journal-test-support"))]
pub(super) mod test_support {
    use super::*;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum JournalPausePoint {
        BeforeOpen,
        BeforeCommit,
        AfterCommit,
        BeforePublication,
        AfterPublication,
    }
    struct PauseState {
        hits: usize,
        paused: bool,
        released: bool,
    }
    struct Pause {
        point: JournalPausePoint,
        occurrence: usize,
        state: Mutex<PauseState>,
        changed: Condvar,
        #[cfg(test)]
        unwind_on_release: bool,
    }
    pub struct JournalTestPause {
        inner: Arc<Pause>,
    }
    pub(crate) struct WorkerPause {
        inner: Arc<Pause>,
    }
    impl JournalTestPause {
        pub fn new(
            point: JournalPausePoint,
            occurrence: usize,
        ) -> Result<Self, ReferenceJournalError> {
            if !(1..=8).contains(&occurrence) {
                return Err(error("pause occurrence must be 1..=8"));
            }
            Ok(Self {
                inner: Arc::new(Pause {
                    point,
                    occurrence,
                    state: Mutex::new(PauseState {
                        hits: 0,
                        paused: false,
                        released: false,
                    }),
                    changed: Condvar::new(),
                    #[cfg(test)]
                    unwind_on_release: false,
                }),
            })
        }
        pub fn wait_paused(&self, timeout: Duration) -> bool {
            let start = Instant::now();
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            while !state.paused {
                let remaining = timeout.saturating_sub(start.elapsed());
                if remaining.is_zero() {
                    return false;
                }
                state = self
                    .inner
                    .changed
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(|poison| poison.into_inner())
                    .0;
            }
            true
        }
        #[cfg(test)]
        pub(crate) fn with_worker_unwind(
            point: JournalPausePoint,
        ) -> Result<Self, ReferenceJournalError> {
            let mut pause = Self::new(point, 1)?;
            Arc::get_mut(&mut pause.inner)
                .ok_or_else(|| error("unshared test pause required"))?
                .unwind_on_release = true;
            Ok(pause)
        }
        pub fn release(&self) {
            self.inner
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .released = true;
            self.inner.changed.notify_all();
        }
        pub(crate) fn worker(&self) -> WorkerPause {
            WorkerPause {
                inner: Arc::clone(&self.inner),
            }
        }
    }
    impl Drop for JournalTestPause {
        fn drop(&mut self) {
            self.release();
        }
    }
    pub(super) fn at(pause: Option<&WorkerPause>, point: JournalPausePoint) {
        let Some(pause) = pause.filter(|pause| pause.inner.point == point) else {
            return;
        };
        let mut state = pause
            .inner
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.hits = state.hits.saturating_add(1);
        if state.hits != pause.inner.occurrence || state.released {
            return;
        }
        state.paused = true;
        pause.inner.changed.notify_all();
        while !state.released {
            state = pause
                .inner
                .changed
                .wait(state)
                .unwrap_or_else(|poison| poison.into_inner());
        }
        drop(state);
        #[cfg(test)]
        #[allow(
            clippy::panic,
            reason = "fixed unit-test-only worker unwind after real SQL phase"
        )]
        if pause.inner.unwind_on_release {
            panic!("fixed reference journal worker-unwind fixture");
        }
    }
}

#[cfg(test)]
#[path = "reference_journal_storage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "reference_journal_resource_tests.rs"]
mod resource_tests;

#[cfg(test)]
#[path = "reference_journal_namespace_tests.rs"]
mod namespace_tests;

#[cfg(test)]
#[path = "reference_journal_hardlink_tests.rs"]
mod hardlink_tests;

#[cfg(all(test, unix))]
#[path = "reference_journal_filetype_tests.rs"]
mod filetype_tests;
