//! Import systemd journal lines into `log_events` (Pi maintenance).

#[cfg(target_os = "linux")]
use std::io::{BufRead, Read};
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};

#[cfg(any(target_os = "linux", test))]
use serde::Deserialize;

#[cfg(any(target_os = "linux", test))]
use crate::model::LogEventInsert;
use crate::store::Store;
use crate::Result;
#[cfg(target_os = "linux")]
use crate::{now_ms, StoreError};

#[cfg(target_os = "linux")]
const JOURNAL_CURSOR_KEY: &str = "journal_import_cursor";
/// Pre-cursor ledger values stored milliseconds here; still honored once.
#[cfg(target_os = "linux")]
const LEGACY_CURSOR_KEY: &str = "journal_import_ts_ms";
#[cfg(target_os = "linux")]
const DEFAULT_LOOKBACK_MS: u64 = 24 * 60 * 60 * 1000;
/// Events per insert transaction; bounds import memory on a busy journal.
#[cfg(target_os = "linux")]
const IMPORT_BATCH: usize = 500;
#[cfg(target_os = "linux")]
const MAX_LINE_BYTES: usize = 1024 * 1024;
/// Single journal lines past this are skipped (their cursors are unknown, so
/// nothing advances for the skipped line); bounds one huge line from
/// ballooning import memory.
/// Units imported into structured logs (`target` prefix `systemd:`).
pub const JOURNAL_UNITS: &[&str] = &["marengo-pi", "marengo-can", "marengo-gateway"];

/// Pull journal lines since the stored cursor and insert as structured log rows.
pub fn import_journal(store: &Store, units: &[&str]) -> Result<u32> {
    #[cfg(target_os = "linux")]
    {
        import_journal_linux(store, units)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (store, units);
        Ok(0)
    }
}

/// Shared by the Linux importer and the cross-platform parser tests.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Deserialize)]
struct JournalEntry {
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    realtime_us: Option<String>,
    #[serde(rename = "__CURSOR")]
    cursor: Option<String>,
    #[serde(rename = "MESSAGE")]
    message: Option<String>,
    #[serde(rename = "_SYSTEMD_UNIT")]
    systemd_unit: Option<String>,
    #[serde(rename = "SYSLOG_IDENTIFIER")]
    syslog_id: Option<String>,
    #[serde(rename = "PRIORITY")]
    priority: Option<String>,
}

/// One accepted journal line: insert payload plus the cursor to advance to.
#[cfg(any(target_os = "linux", test))]
struct ParsedJournalLine {
    event: LogEventInsert,
    cursor: String,
}

/// Pure parse of one `journalctl --output=json` line. `None` skips the line
/// (unparsable, no timestamp/cursor, or empty message) without failing the
/// import; the caller still advances past it.
#[cfg(any(target_os = "linux", test))]
fn parse_journal_line(line: &str) -> Option<ParsedJournalLine> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let entry: JournalEntry = serde_json::from_str(line).ok()?;
    let cursor = entry.cursor.filter(|cursor| !cursor.is_empty())?;
    let us = entry.realtime_us.as_deref()?.parse::<u64>().ok()?;
    let ts_ms = us / 1000;
    let message = entry.message.unwrap_or_default();
    if message.is_empty() {
        return None;
    }
    let unit = entry
        .systemd_unit
        .as_deref()
        .unwrap_or(entry.syslog_id.as_deref().unwrap_or("unknown"))
        .trim_end_matches(".service");
    Some(ParsedJournalLine {
        event: LogEventInsert {
            ts_ms,
            level: journal_priority_level(entry.priority.as_deref()),
            target: format!("systemd:{unit}"),
            message,
            session_id: None,
            fields_json: None,
        },
        cursor,
    })
}

#[cfg(target_os = "linux")]
fn import_journal_linux(store: &Store, units: &[&str]) -> Result<u32> {
    let mut cmd = Command::new("journalctl");
    cmd.args(["--no-pager", "--output", "json"]);
    // Resume from the journal cursor when present: cursors are unique and
    // ordered, so same-millisecond entries are neither dropped nor
    // re-imported. A legacy millisecond value (pre-cursor ledgers) falls
    // back to `--since` once, then the cursor takes over.
    match store.get_setting(JOURNAL_CURSOR_KEY)? {
        Some(cursor) if !cursor.trim().is_empty() => {
            cmd.args(["--after-cursor", cursor.trim()]);
        }
        // Whitespace-only cursor: no position to resume from; fall through
        // to the legacy-millisecond/default lookback below.
        Some(_) | None => match store
            .get_setting(LEGACY_CURSOR_KEY)?
            .and_then(|value| value.parse::<u64>().ok())
        {
            Some(since_ms) => {
                cmd.args(["--since", &format!("@{}", since_ms / 1000)]);
            }
            None => {
                let since_ms = now_ms().saturating_sub(DEFAULT_LOOKBACK_MS);
                cmd.args(["--since", &format!("@{}", since_ms / 1000)]);
            }
        },
    }
    for unit in units {
        cmd.args(["-u", unit]);
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(StoreError::Io)?;
    // Stream stdout instead of buffering it: a 24 h lookback on a busy Pi
    // must not land whole in memory before the first insert.
    let mut reader = std::io::BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| StoreError::msg("journalctl stdout unavailable"))?,
    );
    let mut total = 0u32;
    let mut batch = Vec::with_capacity(IMPORT_BATCH);
    let mut batch_cursor = String::new();
    let mut buf = Vec::with_capacity(8192);
    loop {
        buf.clear();
        let read = reader
            .by_ref()
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut buf)
            .map_err(StoreError::Io)?;
        if read == 0 {
            break;
        }
        if buf.len() > MAX_LINE_BYTES {
            // Oversize line: drain to the newline, skip the entry, and keep
            // the import bounded. Its cursor is unknown, so nothing advances.
            tracing::warn!(
                bytes = buf.len(),
                "journal line exceeds cap; skipping entry"
            );
            loop {
                buf.clear();
                let drained = reader
                    .by_ref()
                    .take((MAX_LINE_BYTES + 1) as u64)
                    .read_until(b'\n', &mut buf)
                    .map_err(StoreError::Io)?;
                if drained == 0 || buf.last() == Some(&b'\n') {
                    break;
                }
            }
            continue;
        }
        let line = String::from_utf8_lossy(&buf);
        let Some(parsed) = parse_journal_line(&line) else {
            continue;
        };
        batch_cursor = parsed.cursor;
        batch.push(parsed.event);
        if batch.len() >= IMPORT_BATCH {
            total += drain_import_batch(store, &mut batch, &batch_cursor)?;
        }
    }
    total += drain_import_batch(store, &mut batch, &batch_cursor)?;
    drop(reader);
    let output = child.wait_with_output().map_err(StoreError::Io)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No entries") {
            return Ok(total);
        }
        return Err(StoreError::msg(format!("journalctl failed: {stderr}")));
    }
    Ok(total)
}

/// Insert one batch and advance the cursor in a single transaction.
#[cfg(target_os = "linux")]
fn drain_import_batch(store: &Store, batch: &mut Vec<LogEventInsert>, cursor: &str) -> Result<u32> {
    if batch.is_empty() {
        return Ok(0);
    }
    let count = batch.len() as u32;
    store.insert_log_events_with_cursor(batch, JOURNAL_CURSOR_KEY, cursor)?;
    batch.clear();
    Ok(count)
}

#[cfg(any(target_os = "linux", test))]
fn journal_priority_level(priority: Option<&str>) -> String {
    match priority.and_then(|p| p.parse::<u8>().ok()) {
        Some(0..=3) => "error".into(),
        Some(4) => "warn".into(),
        Some(5..=6) => "info".into(),
        Some(7) => "debug".into(),
        _ => "info".into(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{journal_priority_level, parse_journal_line};

    #[test]
    fn journal_priority_maps() {
        assert_eq!(journal_priority_level(Some("3")), "error");
        assert_eq!(journal_priority_level(Some("4")), "warn");
        assert_eq!(journal_priority_level(Some("6")), "info");
    }

    #[test]
    fn journal_line_parses_cursor_level_and_unit() {
        let parsed = parse_journal_line(
            r#"{"__CURSOR":"s=abc;i=1","__REALTIME_TIMESTAMP":"1700000000123456","MESSAGE":"hold ok","_SYSTEMD_UNIT":"marengo-pi.service","PRIORITY":"6"}"#,
        )
        .expect("valid line parses");
        assert_eq!(parsed.event.ts_ms, 1_700_000_000_123);
        assert_eq!(parsed.event.level, "info");
        assert_eq!(parsed.event.target, "systemd:marengo-pi");
        assert_eq!(parsed.event.message, "hold ok");
        assert_eq!(parsed.cursor, "s=abc;i=1");
    }

    #[test]
    fn journal_line_skips_without_cursor_or_message() {
        // No cursor: cannot advance, must skip rather than stall or dup.
        assert!(
            parse_journal_line(r#"{"__REALTIME_TIMESTAMP":"1700000000123456","MESSAGE":"x"}"#)
                .is_none()
        );
        // Empty message: nothing worth storing.
        assert!(parse_journal_line(
            r#"{"__CURSOR":"s=abc;i=2","__REALTIME_TIMESTAMP":"1700000000123456","MESSAGE":""}"#
        )
        .is_none());
        // Garbage never fails the import.
        assert!(parse_journal_line("not json").is_none());
        assert!(parse_journal_line("").is_none());
    }
}
