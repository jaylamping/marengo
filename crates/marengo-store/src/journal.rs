//! Import systemd journal lines into `log_events` (Pi maintenance).

#[cfg(target_os = "linux")]
use std::process::Command;

// Linux production code, also compiled for tests everywhere so the journal
// line parser is covered on macOS CI too.
#[cfg(any(target_os = "linux", test))]
use serde::Deserialize;

#[cfg(any(target_os = "linux", test))]
use crate::model::LogEventInsert;
use crate::store::Store;
use crate::Result;
#[cfg(target_os = "linux")]
use crate::{now_ms, StoreError};

#[cfg(target_os = "linux")]
const JOURNAL_CURSOR_KEY: &str = "journal_import_ts_ms";
#[cfg(target_os = "linux")]
const DEFAULT_LOOKBACK_MS: u64 = 24 * 60 * 60 * 1000;

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

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Deserialize)]
struct JournalEntry {
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    realtime_us: Option<String>,
    #[serde(rename = "MESSAGE")]
    message: Option<String>,
    #[serde(rename = "_SYSTEMD_UNIT")]
    systemd_unit: Option<String>,
    #[serde(rename = "SYSLOG_IDENTIFIER")]
    syslog_id: Option<String>,
    #[serde(rename = "PRIORITY")]
    priority: Option<String>,
}

#[cfg(target_os = "linux")]
fn import_journal_linux(store: &Store, units: &[&str]) -> Result<u32> {
    let since_ms = store
        .get_setting(JOURNAL_CURSOR_KEY)?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(|| now_ms().saturating_sub(DEFAULT_LOOKBACK_MS));

    let since_sec = since_ms / 1000;
    let mut cmd = Command::new("journalctl");
    cmd.args([
        "--no-pager",
        "--output",
        "json",
        "--since",
        &format!("@{since_sec}"),
    ]);
    for unit in units {
        cmd.args(["-u", unit]);
    }

    let output = cmd.output().map_err(StoreError::Io)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No entries") {
            return Ok(0);
        }
        return Err(StoreError::msg(format!("journalctl failed: {stderr}")));
    }

    let mut events = Vec::new();
    let mut max_ts = since_ms;

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some((event, ts_ms)) = parse_journal_line(line, since_ms) {
            max_ts = max_ts.max(ts_ms);
            events.push(event);
        }
    }

    let count = events.len() as u32;
    if !events.is_empty() {
        store.insert_log_events(&events)?;
        store.set_setting(JOURNAL_CURSOR_KEY, &max_ts.to_string(), now_ms())?;
    }
    Ok(count)
}

/// Map a journal `PRIORITY` digit to a log level (pure; tested everywhere).
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

/// Parse one `journalctl --output json` line into a log event newer than the
/// stored cursor. Pure (no `journalctl`, no clock); tested on every platform.
/// Returns the event and its millisecond timestamp.
#[cfg(any(target_os = "linux", test))]
fn parse_journal_line(line: &str, since_ms: u64) -> Option<(LogEventInsert, u64)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let entry: JournalEntry = serde_json::from_str(line).ok()?;
    let us = entry.realtime_us?.parse::<u64>().ok()?;
    let ts_ms = us / 1000;
    if ts_ms <= since_ms {
        return None;
    }
    let unit = entry
        .systemd_unit
        .as_deref()
        .unwrap_or(entry.syslog_id.as_deref().unwrap_or("unknown"))
        .trim_end_matches(".service");
    let message = entry.message.unwrap_or_default();
    if message.is_empty() {
        return None;
    }
    Some((
        LogEventInsert {
            ts_ms,
            level: journal_priority_level(entry.priority.as_deref()),
            target: format!("systemd:{unit}"),
            message,
            session_id: None,
            fields_json: None,
        },
        ts_ms,
    ))
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
    fn parse_accepts_a_newer_event() {
        let (event, ts) = parse_journal_line(
            r#"{"__REALTIME_TIMESTAMP":"1700000000123456","MESSAGE":"tick ok","_SYSTEMD_UNIT":"marengo-pi.service","PRIORITY":"6"}"#,
            1_700_000_000_000,
        )
        .expect("newer line parses");
        assert_eq!(ts, 1_700_000_000_123);
        assert_eq!(event.ts_ms, 1_700_000_000_123);
        assert_eq!(event.level, "info");
        assert_eq!(event.target, "systemd:marengo-pi");
        assert_eq!(event.message, "tick ok");
    }

    #[test]
    fn parse_drops_stale_same_ms_and_malformed_lines() {
        // Same-ms entries never advance the cursor (import is cursor-ordered).
        assert!(
            parse_journal_line(
                r#"{"__REALTIME_TIMESTAMP":"1700000000000000","MESSAGE":"dup","_SYSTEMD_UNIT":"marengo-pi.service","PRIORITY":"6"}"#,
                1_700_000_000_000,
            )
            .is_none()
        );
        // An absent MESSAGE defaults to empty and is skipped, never stored
        // as a blank row.
        assert!(
            parse_journal_line(
                r#"{"__REALTIME_TIMESTAMP":"1700000000123456","_SYSTEMD_UNIT":"marengo-pi.service","PRIORITY":"3"}"#,
                0,
            )
            .is_none()
        );
        assert!(parse_journal_line("not json", 0).is_none());
        assert!(parse_journal_line("   ", 0).is_none());
        assert!(parse_journal_line(
            r#"{"MESSAGE":"no timestamp","_SYSTEMD_UNIT":"x.service"}"#,
            0,
        )
        .is_none());
    }

    #[test]
    fn parse_falls_back_to_syslog_id_and_unknown_unit() {
        let (event, _) = parse_journal_line(
            r#"{"__REALTIME_TIMESTAMP":"1700000000123456","MESSAGE":"boot","SYSLOG_IDENTIFIER":"kernel","PRIORITY":"4"}"#,
            0,
        )
        .expect("syslog fallback parses");
        assert_eq!(event.target, "systemd:kernel");
        assert_eq!(event.level, "warn");
        let (event, _) = parse_journal_line(
            r#"{"__REALTIME_TIMESTAMP":"1700000000123456","MESSAGE":"boot","PRIORITY":"2"}"#,
            0,
        )
        .expect("unit fallback parses");
        assert_eq!(event.target, "systemd:unknown");
        assert_eq!(event.level, "error");
    }
}
