//! Deliberate recovery of one fully recognized historical schema (ADR0030).

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, ToSql, TransactionBehavior};
use same_file::Handle;
use sha2::{Digest, Sha256};

use crate::error::{Result, StoreError};
use crate::migrations;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const WORK_LIMIT: Duration = Duration::from_secs(30);
const SIDECARS: [&str; 4] = ["", "-wal", "-shm", "-journal"];

/// Completed, verified standalone artifacts from an explicit recovery.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RecoveryReceipt {
    pub source_path: PathBuf,
    pub backup_path: PathBuf,
    pub output_path: PathBuf,
    pub backup_sha256: String,
    pub output_sha256: String,
    pub backup_bytes: u64,
    pub output_bytes: u64,
    pub source_version: u32,
    pub output_version: u32,
}

/// Recover a complete known v2 schema whose stored marker is exactly `1`.
///
/// Source remains logically unchanged. Both destinations and their SQLite
/// sidecars must be fresh; parents must already exist. Portable ASCII filenames
/// avoid platform-dependent case, stream and device-name ambiguity. A completed
/// backup is retained on every later failure. Work uses a 30-second cooperative
/// budget, with individual SQLite busy waits of at most five seconds, and a
/// 256 MiB limit for the source namespace and each image. OS I/O is not forcibly
/// cancellable; this is not a strict wall-clock deadline. Sync and verification
/// do not establish physical media durability.
pub fn recover_known_v2(
    source: impl AsRef<Path>,
    backup: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<RecoveryReceipt> {
    let budget = Budget(Instant::now());
    let paths = RecoveryPaths::resolve(source.as_ref(), backup.as_ref(), output.as_ref(), &budget)
        .map_err(|error| StoreError::msg(format!("known-v2 recovery refused during filesystem preflight: {error}; source={}; backup={}; output={}", source.as_ref().display(), backup.as_ref().display(), output.as_ref().display())))?;
    let mut backup_stage = None;
    let mut output_stage = None;
    let mut backup_published = false;
    let mut output_published = false;
    let mut stage = "recognition";
    let result = (|| {
        let expected = with_connection(None, &budget, |conn| {
            conn.execute_batch(migrations::MIGRATION_001)?;
            conn.execute_batch(migrations::MIGRATION_002)?;
            schema(conn, None, &budget)
        })?;
        let preserved = with_connection(Some(FileBinding::Source(&paths)), &budget, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
            let work = (|| {
                recognize(&tx, &expected, "1", &budget)?;
                integrity(&tx)?;
                let preserved = Preservation::read(&tx, &expected, &budget)?;
                stage = "backup creation";
                backup_stage = Some(OwnedStage::new(&paths.backup)?);
                let owned = backup_stage
                    .as_mut()
                    .ok_or_else(|| StoreError::msg("missing owned backup stage"))?;
                owned.create_file()?;
                with_connection(
                    Some(owned.binding(&owned.database, false)),
                    &budget,
                    |destination| {
                        standalone_mode(destination)?;
                        online_backup(&tx, destination, &budget)?;
                        standalone_mode(destination)?;
                        recognize(destination, &expected, "1", &budget)?;
                        integrity(destination)?;
                        if fingerprint(destination, &expected, None, &budget)? != preserved.complete
                        {
                            return Err(StoreError::msg(
                                "backup did not preserve the pinned source rows",
                            ));
                        }
                        fts_integrity(destination)?;
                        Ok(())
                    },
                )?;
                stage = "backup close/reopen verification";
                verify_image(
                    owned.binding(&owned.database, true),
                    &expected,
                    "1",
                    preserved.complete,
                    None,
                    &budget,
                )?;
                let (hash, bytes) = file_digest(&owned.database, &budget)?;
                owned.sync()?;
                stage = "backup publication";
                paths.backup.fresh(&budget)?;
                owned.publish(&paths.backup.path)?;
                backup_published = true;
                owned.verify_link(&paths.backup.path)?;
                verify_image(
                    owned.binding(&paths.backup.path, true),
                    &expected,
                    "1",
                    preserved.complete,
                    None,
                    &budget,
                )?;
                if file_digest(&paths.backup.path, &budget)? != (hash.clone(), bytes) {
                    return Err(StoreError::msg("published backup identity/content changed"));
                }
                Ok((preserved, hash, bytes))
            })();
            finish(work, tx.rollback().map_err(StoreError::from))
        })?;
        let (mut preserved, backup_sha256, backup_bytes) = preserved;
        stage = "output creation";
        output_stage = Some(OwnedStage::new(&paths.output)?);
        let owned = output_stage
            .as_mut()
            .ok_or_else(|| StoreError::msg("missing owned output stage"))?;
        owned.create_file()?;
        let completed = backup_stage
            .as_ref()
            .ok_or_else(|| StoreError::msg("missing completed backup identity"))?;
        copy_image(&paths.backup.path, completed, owned, &budget)?;
        let now = crate::store::now_ms();
        preserved.recovery_now = Some(now);
        stage = "output marker repair and normal migration";
        with_connection(
            Some(owned.binding(&owned.database, false)),
            &budget,
            |conn| {
                standalone_mode(conn)?;
                recognize(conn, &expected, "1", &budget)?;
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let work = (|| {
                    let changed = tx.execute(
                        "UPDATE settings SET value_json='2' WHERE key='schema_version' AND value_json='1'",
                        [],
                    )?;
                    if changed != 1 {
                        return Err(StoreError::msg(
                            "recognized stale marker was not promoted exactly once",
                        ));
                    }
                    marker(&tx, "2")
                })();
                match work {
                    Ok(()) => tx.commit()?,
                    Err(error) => {
                        return finish(Err(error), tx.rollback().map_err(StoreError::from))
                    }
                }
                migrations::migrate(conn, now)?;
                recognize(conn, &expected, "3", &budget)?;
                integrity(conn)?;
                if fingerprint(conn, &expected, Some(&preserved), &budget)? != preserved.recovered {
                    return Err(StoreError::msg(
                        "recovered output did not preserve historical rows/settings",
                    ));
                }
                preserved.verify_defaults(conn, now)?;
                fts_integrity(conn)?;
                standalone_mode(conn)
            },
        )?;
        stage = "output close/reopen verification";
        verify_image(
            owned.binding(&owned.database, true),
            &expected,
            "3",
            preserved.recovered,
            Some(&preserved),
            &budget,
        )?;
        let (output_sha256, output_bytes) = file_digest(&owned.database, &budget)?;
        owned.sync()?;
        stage = "output publication";
        paths.output.fresh(&budget)?;
        owned.publish(&paths.output.path)?;
        output_published = true;
        owned.verify_link(&paths.output.path)?;
        verify_image(
            owned.binding(&paths.output.path, true),
            &expected,
            "3",
            preserved.recovered,
            Some(&preserved),
            &budget,
        )?;
        if file_digest(&paths.output.path, &budget)? != (output_sha256.clone(), output_bytes) {
            return Err(StoreError::msg("published output identity/content changed"));
        }
        Ok(RecoveryReceipt {
            source_path: paths.source.path.clone(),
            backup_path: paths.backup.path.clone(),
            output_path: paths.output.path.clone(),
            backup_sha256,
            output_sha256,
            backup_bytes,
            output_bytes,
            source_version: 1,
            output_version: 3,
        })
    })();
    let mut cleanup_errors = Vec::new();
    for owned in [output_stage, backup_stage].into_iter().flatten() {
        if let Err(error) = owned.cleanup(&paths.source_files) {
            cleanup_errors.push(error.to_string());
        }
    }
    let result = finish(result, budget.check());
    if result.is_err() || !cleanup_errors.is_empty() {
        let cause = result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_else(|| "artifacts verified but staging cleanup failed".into());
        return Err(StoreError::msg(format!(
            "known-v2 recovery failed during {stage}: {cause}; source={}; retained published backup={}; retained published output={}; cleanup errors={cleanup_errors:?}",
            paths.source.path.display(),
            if backup_published { paths.backup.path.display().to_string() } else { "none".into() },
            if output_published { paths.output.path.display().to_string() } else { "none".into() },
        )));
    }
    result
}

struct Budget(Instant);

impl Budget {
    fn check(&self) -> Result<()> {
        if self.0.elapsed() >= WORK_LIMIT {
            return Err(StoreError::msg(
                "known-v2 recovery exceeded its 30-second local work bound",
            ));
        }
        Ok(())
    }
}

fn finish<T>(work: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (work, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(StoreError::msg(format!(
            "{error}; cleanup failed: {cleanup}"
        ))),
    }
}

fn with_connection<T>(
    file: Option<FileBinding<'_>>,
    budget: &Budget,
    work: impl FnOnce(&mut Connection) -> Result<T>,
) -> Result<T> {
    budget.check()?;
    if let Some(file) = &file {
        file.validate()?;
    }
    let mut conn = match &file {
        None => Connection::open_in_memory()?,
        Some(file) => Connection::open_with_flags(
            file.path(),
            if file.readonly() {
                OpenFlags::SQLITE_OPEN_READ_ONLY
            } else {
                OpenFlags::SQLITE_OPEN_READ_WRITE
            },
        )?,
    };
    let result = (|| {
        if let Some(file) = &file {
            file.validate()?;
        }
        conn.busy_timeout(Duration::from_secs(5))?;
        let start = budget.0;
        conn.progress_handler(1_000, Some(move || start.elapsed() >= WORK_LIMIT));
        let result = work(&mut conn);
        finish(result, file.as_ref().map_or(Ok(()), FileBinding::validate))
    })();
    let close = conn.close().map_err(|(conn, error)| {
        drop(conn);
        StoreError::from(error)
    });
    let result = finish(result, close);
    let result = finish(result, file.as_ref().map_or(Ok(()), FileBinding::validate));
    finish(result, budget.check())
}

enum FileBinding<'a> {
    Source(&'a RecoveryPaths),
    Owned {
        owner: &'a OwnedStage,
        path: &'a Path,
        readonly: bool,
    },
}

impl FileBinding<'_> {
    fn path(&self) -> &Path {
        match self {
            Self::Source(paths) => &paths.source.path,
            Self::Owned { path, .. } => path,
        }
    }

    fn readonly(&self) -> bool {
        match self {
            Self::Source(_) => true,
            Self::Owned { readonly, .. } => *readonly,
        }
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Source(paths) => {
                paths.source.parent_unchanged()?;
                regular(&paths.source.path)?;
                if paths.source_files.first() != Some(&Handle::from_path(&paths.source.path)?) {
                    return Err(StoreError::msg(
                        "source database identity changed before/during SQLite access",
                    ));
                }
                Ok(())
            }
            Self::Owned { owner, path, .. } => owner.verify_link(path),
        }
    }
}

fn online_backup(source: &Connection, destination: &mut Connection, budget: &Budget) -> Result<()> {
    let backup = Backup::new(source, destination)?;
    loop {
        budget.check()?;
        match backup.step(64)? {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => return Err(StoreError::msg("SQLite online backup was busy/locked; retry after releasing the competing connection")),
            _ => return Err(StoreError::msg("unexpected SQLite online backup result")),
        }
    }
}

fn standalone_mode(conn: &Connection) -> Result<()> {
    let mode: String = conn.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err(StoreError::msg(
            "owned image could not enter standalone DELETE journal mode",
        ));
    }
    conn.pragma_update(None, "synchronous", "FULL")?;
    Ok(())
}

fn integrity(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("PRAGMA integrity_check")?;
    let mut rows = statement.query([])?;
    if rows
        .next()?
        .map(|row| row.get::<_, String>(0))
        .transpose()?
        != Some("ok".into())
        || rows.next()?.is_some()
    {
        return Err(StoreError::msg("SQLite integrity check failed"));
    }
    Ok(())
}

fn fts_integrity(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let work = tx
        .execute(
            "INSERT INTO log_events_fts(log_events_fts,rank) VALUES('integrity-check',1)",
            [],
        )
        .map(|_| ())
        .map_err(StoreError::from);
    finish(work, tx.rollback().map_err(StoreError::from))
}

fn marker(conn: &Connection, expected: &str) -> Result<()> {
    let stored: String = conn.query_row(
        "SELECT value_json FROM settings WHERE key='schema_version'",
        [],
        |row| row.get(0),
    )?;
    if stored != expected {
        return Err(StoreError::msg(format!(
            "known-v2 profile requires marker {expected:?}, found {stored:?}"
        )));
    }
    Ok(())
}

type SchemaObject = (String, String, String, Option<Vec<String>>);

#[derive(PartialEq, Eq)]
struct Schema {
    objects: Vec<SchemaObject>,
    constraints: [u8; 32],
}

fn schema(conn: &Connection, trusted: Option<&Schema>, budget: &Budget) -> Result<Schema> {
    let mut statement =
        conn.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")?;
    let mut rows = statement.query([])?;
    let mut objects: Vec<SchemaObject> = Vec::new();
    while let Some(row) = rows.next()? {
        budget.check()?;
        let sql: Option<String> = row.get(3)?;
        objects.push((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            sql.as_deref().map(sql_tokens).transpose()?,
        ));
    }
    if trusted.is_some_and(|expected| objects != expected.objects) {
        return Err(StoreError::msg(
            "historical objects do not match the complete trusted v2 profile",
        ));
    }
    let mut constraints = Sha256::new();
    for (kind, name, _, _) in &objects {
        match kind.as_str() {
            "table" => {
                hash_rows(
                    conn,
                    "SELECT * FROM pragma_table_xinfo(?1) ORDER BY cid",
                    &[name],
                    &mut constraints,
                    budget,
                )?;
                hash_rows(conn, "SELECT name,\"unique\",origin,partial FROM pragma_index_list(?1) ORDER BY name", &[name], &mut constraints, budget)?;
                hash_rows(
                    conn,
                    "SELECT * FROM pragma_foreign_key_list(?1) ORDER BY id,seq",
                    &[name],
                    &mut constraints,
                    budget,
                )?;
            }
            "index" => hash_rows(
                conn,
                "SELECT * FROM pragma_index_xinfo(?1) ORDER BY seqno",
                &[name],
                &mut constraints,
                budget,
            )?,
            "trigger" => {}
            _ => {
                return Err(StoreError::msg(
                    "unsupported schema object in known-v2 profile",
                ))
            }
        }
    }
    Ok(Schema {
        objects,
        constraints: constraints.finalize().into(),
    })
}

// Tokens preserve lexeme boundaries. Quoted bytes, including escaped delimiters,
// remain exact; neither comments nor unknown syntax are normalized into matches.
fn sql_tokens(sql: &str) -> Result<Vec<String>> {
    let bytes = sql.as_bytes();
    let mut at = 0;
    let mut tokens = Vec::new();
    while at < bytes.len() {
        if bytes[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        let start = at;
        match bytes[at] {
            b'\'' | b'"' | b'`' | b'[' => {
                let opener = bytes[at];
                let end = if opener == b'[' { b']' } else { opener };
                at += 1;
                loop {
                    if at == bytes.len() {
                        return Err(StoreError::msg("unterminated quoted schema token"));
                    }
                    if bytes[at] == end {
                        at += 1;
                        if opener != b'[' && bytes.get(at) == Some(&end) {
                            at += 1;
                        } else {
                            break;
                        }
                    } else {
                        at += 1;
                    }
                }
                tokens.push(sql[start..at].to_owned());
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                at += 1;
                while bytes
                    .get(at)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    at += 1;
                }
                tokens.push(sql[start..at].to_ascii_lowercase());
            }
            b'0'..=b'9' => {
                at += 1;
                while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                    at += 1;
                }
                tokens.push(sql[start..at].to_owned());
            }
            b'(' | b')' | b',' | b'.' | b'=' | b';' => {
                at += 1;
                tokens.push(sql[start..at].to_owned());
            }
            _ => {
                return Err(StoreError::msg(
                    "comment or unsupported token in historical schema",
                ))
            }
        }
    }
    Ok(tokens)
}

fn recognize(conn: &Connection, expected: &Schema, version: &str, budget: &Budget) -> Result<()> {
    budget.check()?;
    let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if !["delete", "truncate", "persist", "wal"]
        .iter()
        .any(|allowed| mode.eq_ignore_ascii_case(allowed))
    {
        return Err(StoreError::msg(
            "historical source has no supported rollback-capable journal mode",
        ));
    }
    let pages: u64 = conn.pragma_query_value(None, "page_count", |row| row.get(0))?;
    let size: u64 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
    if pages
        .checked_mul(size)
        .is_none_or(|bytes| bytes > MAX_BYTES)
    {
        return Err(StoreError::msg(
            "historical image exceeds the 256 MiB recovery bound",
        ));
    }
    // Check the complete object signature before querying object-specific pragmas.
    if schema(conn, Some(expected), budget)? != *expected {
        return Err(StoreError::msg("historical schema is not the complete trusted v2 profile; preserve it for separate recovery"));
    }
    marker(conn, version)
}

struct Preservation {
    complete: [u8; 32],
    recovered: [u8; 32],
    has_days: bool,
    has_budget: bool,
    recovery_now: Option<u64>,
}

impl Preservation {
    fn read(conn: &Connection, expected: &Schema, budget: &Budget) -> Result<Self> {
        let present = |key: &str| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",
                [key],
                |row| row.get(0),
            )
        };
        let mut value = Self {
            complete: fingerprint(conn, expected, None, budget)?,
            recovered: [0; 32],
            has_days: present("log_archive_days")?,
            has_budget: present("log_disk_budget_bytes")?,
            recovery_now: None,
        };
        value.recovered = fingerprint(conn, expected, Some(&value), budget)?;
        Ok(value)
    }

    fn verify_defaults(&self, conn: &Connection, now: u64) -> Result<()> {
        let now = i64::try_from(now)
            .map_err(|_| StoreError::msg("recovery timestamp exceeds SQLite range"))?;
        for (present, key, expected) in [
            (
                self.has_days,
                "log_archive_days",
                crate::paths::DEFAULT_ARCHIVE_DAYS.to_string(),
            ),
            (
                self.has_budget,
                "log_disk_budget_bytes",
                crate::paths::DEFAULT_LOG_DISK_BUDGET_BYTES.to_string(),
            ),
        ] {
            if !present {
                let row: (String, i64) = conn.query_row(
                    "SELECT value_json,updated_ms FROM settings WHERE key=?1",
                    [key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if row != (expected, now) {
                    return Err(StoreError::msg(
                        "normal owner did not insert the expected missing default",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn fingerprint(
    conn: &Connection,
    expected: &Schema,
    repaired: Option<&Preservation>,
    budget: &Budget,
) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    for (kind, name, _, _) in &expected.objects {
        if kind != "table" || name == "log_events_fts" {
            continue;
        }
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        // Names are already equal to the trusted reference, not caller input.
        let mut sql = format!("SELECT * FROM \"{name}\"");
        if let ("settings", Some(preserved)) = (name.as_str(), repaired) {
            sql.push_str(" WHERE key<>'schema_version'");
            if !preserved.has_days {
                sql.push_str(" AND key<>'log_archive_days'");
            }
            if !preserved.has_budget {
                sql.push_str(" AND key<>'log_disk_budget_bytes'");
            }
        }
        let columns = conn.prepare(&sql)?.column_count();
        sql.push_str(" ORDER BY ");
        sql.push_str(
            &(1..=columns)
                .map(|column| column.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        hash_rows(conn, &sql, &[], &mut hash, budget)?;
    }
    Ok(hash.finalize().into())
}

fn hash_rows(
    conn: &Connection,
    sql: &str,
    parameters: &[&dyn ToSql],
    hash: &mut Sha256,
    budget: &Budget,
) -> Result<()> {
    let mut statement = conn.prepare(sql)?;
    let columns = statement.column_count();
    hash.update((columns as u64).to_le_bytes());
    let mut rows = statement.query(parameters)?;
    while let Some(row) = rows.next()? {
        budget.check()?;
        hash.update([1]);
        for column in 0..columns {
            match row.get_ref(column)? {
                ValueRef::Null => hash.update([0]),
                ValueRef::Integer(value) => {
                    hash.update([1]);
                    hash.update(value.to_le_bytes());
                }
                ValueRef::Real(value) => {
                    hash.update([2]);
                    hash.update(value.to_bits().to_le_bytes());
                }
                ValueRef::Text(value) | ValueRef::Blob(value) => {
                    hash.update([if matches!(row.get_ref(column)?, ValueRef::Text(_)) {
                        3
                    } else {
                        4
                    }]);
                    hash.update((value.len() as u64).to_le_bytes());
                    hash.update(value);
                }
            }
        }
    }
    hash.update([0]);
    Ok(())
}

fn verify_image(
    file: FileBinding<'_>,
    expected: &Schema,
    version: &str,
    digest: [u8; 32],
    repaired: Option<&Preservation>,
    budget: &Budget,
) -> Result<()> {
    file.validate()?;
    let path = file.path().to_path_buf();
    standalone_files(&path)?;
    with_connection(Some(file), budget, |conn| {
        let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("delete") {
            return Err(StoreError::msg(
                "closed artifact is not a standalone DELETE image",
            ));
        }
        recognize(conn, expected, version, budget)?;
        integrity(conn)?;
        if fingerprint(conn, expected, repaired, budget)? != digest {
            return Err(StoreError::msg(
                "reopened artifact did not preserve verified rows",
            ));
        }
        if let Some(preserved) = repaired {
            preserved.verify_defaults(
                conn,
                preserved
                    .recovery_now
                    .ok_or_else(|| StoreError::msg("missing expected recovery timestamp"))?,
            )?;
        }
        Ok(())
    })?;
    standalone_files(&path)
}

fn standalone_files(path: &Path) -> Result<()> {
    for sidecar in namespace(path).into_iter().skip(1) {
        absent(&sidecar)?;
    }
    let mut header = [0; 20];
    File::open(path)?.read_exact(&mut header)?;
    if &header[..16] != b"SQLite format 3\0" || header[18..20] != [1, 1] {
        return Err(StoreError::msg(
            "closed SQLite image still requires WAL or has an invalid header",
        ));
    }
    Ok(())
}

fn file_digest(path: &Path, budget: &Budget) -> Result<(String, u64)> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    if length > MAX_BYTES {
        return Err(StoreError::msg("artifact exceeds the 256 MiB bound"));
    }
    let mut hash = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        budget.check()?;
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > MAX_BYTES {
            return Err(StoreError::msg("artifact grew beyond the recovery bound"));
        }
        hash.update(&buffer[..read]);
    }
    if count != length {
        return Err(StoreError::msg("artifact size changed while hashing"));
    }
    Ok((format!("{:x}", hash.finalize()), count))
}

fn copy_image(
    source: &Path,
    source_owner: &OwnedStage,
    destination: &OwnedStage,
    budget: &Budget,
) -> Result<()> {
    source_owner.verify_link(source)?;
    destination.verify_link(&destination.database)?;
    let mut source = File::open(source)?;
    let mut target = OpenOptions::new().write(true).open(&destination.database)?;
    source_owner.verify_file(&source)?;
    destination.verify_file(&target)?;
    if target.metadata()?.len() != 0 {
        return Err(StoreError::msg("output stage was not empty"));
    }
    let mut buffer = [0; 64 * 1024];
    let mut copied = 0_u64;
    loop {
        budget.check()?;
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        copied += read as u64;
        if copied > MAX_BYTES {
            return Err(StoreError::msg("backup grew beyond the recovery bound"));
        }
        target.write_all(&buffer[..read])?;
    }
    target.sync_all()?;
    destination.verify_link(&destination.database)
}

struct Destination {
    path: PathBuf,
    parent: PathBuf,
    parent_identity: Handle,
}

impl Destination {
    fn resolve(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StoreError::msg("recovery requires a named portable database file"))?;
        portable_name(name)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = parent.canonicalize()?;
        if !fs::metadata(&parent)?.is_dir() {
            return Err(StoreError::msg(
                "recovery destination parent is not an existing directory",
            ));
        }
        let parent_identity = Handle::from_path(&parent)?;
        Ok(Self {
            path: parent.join(name),
            parent,
            parent_identity,
        })
    }

    fn parent_unchanged(&self) -> Result<()> {
        if Handle::from_path(&self.parent)? != self.parent_identity {
            return Err(StoreError::msg(format!(
                "recovery parent identity changed: {}",
                self.parent.display()
            )));
        }
        Ok(())
    }

    fn fresh(&self, budget: &Budget) -> Result<()> {
        self.parent_unchanged()?;
        for path in namespace(&self.path) {
            absent(&path)?;
            no_case_neighbor(&path, false, budget)?;
        }
        Ok(())
    }
}

struct RecoveryPaths {
    source: Destination,
    backup: Destination,
    output: Destination,
    source_files: Vec<Handle>,
}

impl RecoveryPaths {
    fn resolve(source: &Path, backup: &Path, output: &Path, budget: &Budget) -> Result<Self> {
        budget.check()?;
        let source = Destination::resolve(source)?;
        let backup = Destination::resolve(backup)?;
        let output = Destination::resolve(output)?;
        let destinations = [&source, &backup, &output];
        for (index, left) in destinations.iter().enumerate() {
            for right in destinations.iter().skip(index + 1) {
                if left.parent_identity == right.parent_identity {
                    for a in namespace(&left.path) {
                        for b in namespace(&right.path) {
                            if a.file_name()
                                .and_then(|name| name.to_str())
                                .zip(b.file_name().and_then(|name| name.to_str()))
                                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
                            {
                                return Err(StoreError::msg(
                                    "source/backup/output SQLite namespaces alias",
                                ));
                            }
                        }
                    }
                }
            }
        }
        regular(&source.path)?;
        let mut source_files = Vec::new();
        let mut source_bytes = 0_u64;
        for path in namespace(&source.path) {
            no_case_neighbor(&path, true, budget)?;
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    regular(&path)?;
                    source_bytes = source_bytes
                        .checked_add(metadata.len())
                        .ok_or_else(|| StoreError::msg("source namespace size overflow"))?;
                    if source_bytes > MAX_BYTES {
                        return Err(StoreError::msg(
                            "source SQLite namespace exceeds the 256 MiB bound",
                        ));
                    }
                    let handle = Handle::from_path(&path)?;
                    if source_files.contains(&handle) {
                        return Err(StoreError::msg(
                            "source SQLite namespace contains hardlink aliases",
                        ));
                    }
                    source_files.push(handle);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        backup.fresh(budget)?;
        output.fresh(budget)?;
        Ok(Self {
            source,
            backup,
            output,
            source_files,
        })
    }
}

fn portable_name(name: &str) -> Result<()> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|tail| tail.len() == 1 && matches!(tail.as_bytes()[0], b'1'..=b'9'))
        });
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || device
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(StoreError::msg("ambiguous recovery filename; use ASCII letters/digits/dot/underscore/hyphen and no Windows device name or trailing dot"));
    }
    Ok(())
}

fn namespace(path: &Path) -> Vec<PathBuf> {
    SIDECARS
        .iter()
        .map(|suffix| {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            PathBuf::from(name)
        })
        .collect()
}

fn absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(StoreError::msg(format!(
            "recovery path must be fresh, including dangling symlinks: {}",
            path.display()
        ))),
    }
}

fn regular(path: &Path) -> Result<()> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(StoreError::msg(format!(
            "recovery path is not a regular nonsymlink file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn no_case_neighbor(path: &Path, allow_exact: bool, budget: &Budget) -> Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::msg("missing recovery filename"))?;
    let text = name
        .to_str()
        .ok_or_else(|| StoreError::msg("nonportable recovery filename"))?;
    let parent = path
        .parent()
        .ok_or_else(|| StoreError::msg("missing recovery parent"))?;
    for entry in fs::read_dir(parent)? {
        budget.check()?;
        let entry = entry?;
        let found = entry.file_name();
        if found
            .to_str()
            .is_some_and(|found| found.eq_ignore_ascii_case(text))
            && (!allow_exact || found.as_os_str() != name)
        {
            return Err(StoreError::msg(format!(
                "case-ambiguous or existing recovery namespace: {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

struct OwnedStage {
    parent: PathBuf,
    parent_identity: Handle,
    directory: PathBuf,
    directory_identity: Handle,
    database: PathBuf,
    database_identity: Option<Handle>,
}

impl OwnedStage {
    fn binding<'a>(&'a self, path: &'a Path, readonly: bool) -> FileBinding<'a> {
        FileBinding::Owned {
            owner: self,
            path,
            readonly,
        }
    }

    fn new(destination: &Destination) -> Result<Self> {
        destination.parent_unchanged()?;
        let parent = destination.parent.clone();
        let parent_identity = Handle::from_path(&parent)?;
        let directory = tempfile::Builder::new()
            .prefix(".marengo-recovery-")
            .tempdir_in(&parent)?
            .keep();
        let directory_identity = Handle::from_path(&directory).map_err(|error| {
            StoreError::msg(format!(
                "cannot establish staging ownership; retained {}: {error}",
                directory.display()
            ))
        })?;
        let database = directory.join("image.sqlite");
        Ok(Self {
            parent,
            parent_identity,
            directory,
            directory_identity,
            database,
            database_identity: None,
        })
    }

    fn create_file(&mut self) -> Result<()> {
        self.verify_directory()?;
        for path in namespace(&self.database) {
            absent(&path)?;
        }
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&self.database)?;
        self.database_identity = Some(Handle::from_file(file)?);
        Ok(())
    }

    fn verify_directory(&self) -> Result<()> {
        if Handle::from_path(&self.parent)? != self.parent_identity
            || !fs::symlink_metadata(&self.directory)?.file_type().is_dir()
            || Handle::from_path(&self.directory)? != self.directory_identity
        {
            return Err(StoreError::msg(format!(
                "owned staging directory was replaced: {}",
                self.directory.display()
            )));
        }
        Ok(())
    }

    fn verify_link(&self, path: &Path) -> Result<()> {
        self.verify_directory()?;
        regular(path)?;
        if self.database_identity.as_ref() != Some(&Handle::from_path(path)?) {
            return Err(StoreError::msg(format!(
                "owned SQLite image identity changed: {}",
                path.display()
            )));
        }
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        self.verify_link(&self.database)?;
        let file = OpenOptions::new().write(true).open(&self.database)?;
        self.verify_file(&file)?;
        file.sync_all()?;
        Ok(())
    }

    fn verify_file(&self, file: &File) -> Result<()> {
        if self.database_identity.as_ref() != Some(&Handle::from_file(file.try_clone()?)?) {
            return Err(StoreError::msg(
                "opened staging/publication file has the wrong identity",
            ));
        }
        Ok(())
    }

    fn publish(&self, destination: &Path) -> Result<()> {
        self.verify_link(&self.database)?;
        // Both paths share a filesystem. hard_link refuses an existing final;
        // explicit stage cleanup later avoids hidden no-clobber move cleanup.
        fs::hard_link(&self.database, destination)?;
        Ok(())
    }

    fn cleanup(mut self, protected: &[Handle]) -> Result<()> {
        let cleanup = (|| {
            self.verify_directory()?;
            let mut files = Vec::new();
            for entry in fs::read_dir(&self.directory)? {
                let path = entry?.path();
                if path != self.database {
                    // A SQLite-looking name is not evidence that we created it.
                    return Err(StoreError::msg(format!(
                        "unowned staging entry/sidecar retained: {}",
                        path.display()
                    )));
                }
                regular(&path)?;
                let identity = Handle::from_path(&path)?;
                if self.database_identity.as_ref() != Some(&identity)
                    || protected.contains(&identity)
                {
                    return Err(StoreError::msg(
                        "staging database is not the exclusively created unaliased file",
                    ));
                }
                files.push((path, identity));
            }
            drop(self.database_identity.take());
            for (path, identity) in files {
                if Handle::from_path(&path)? != identity {
                    return Err(StoreError::msg(
                        "staging entry identity changed before cleanup",
                    ));
                }
                fs::remove_file(&path)?;
                drop(identity);
            }
            Ok(())
        })();
        if let Err(error) = cleanup {
            return Err(StoreError::msg(format!(
                "owned staging cleanup failed; retained {}: {error}",
                self.directory.display()
            )));
        }
        let directory = self.directory;
        drop(self.directory_identity);
        fs::remove_dir(&directory).map_err(|error| {
            StoreError::msg(format!(
                "owned staging directory cleanup failed; retained {}: {error}",
                directory.display()
            ))
        })
    }
}
