//! Import systemd journal lines into `log_events` (Pi maintenance).

use std::process::Command;

use serde::Deserialize;

use crate::model::LogEventInsert;
use crate::store::Store;
use crate::{now_ms, Result, StoreError};

const JOURNAL_CURSOR_KEY: &str = "journal_import_ts_ms";
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
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: JournalEntry = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(us_raw) = entry.realtime_us else {
            continue;
        };
        let Ok(us) = us_raw.parse::<u64>() else {
            continue;
        };
        let ts_ms = us / 1000;
        if ts_ms <= since_ms {
            continue;
        }
        max_ts = max_ts.max(ts_ms);
        let unit = entry
            .systemd_unit
            .as_deref()
            .unwrap_or(entry.syslog_id.as_deref().unwrap_or("unknown"))
            .trim_end_matches(".service");
        let message = entry.message.unwrap_or_default();
        if message.is_empty() {
            continue;
        }
        events.push(LogEventInsert {
            ts_ms,
            level: journal_priority_level(entry.priority.as_deref()),
            target: format!("systemd:{unit}"),
            message,
            session_id: None,
            fields_json: None,
        });
    }

    let count = events.len() as u32;
    if !events.is_empty() {
        store.insert_log_events(&events)?;
        store.set_setting(JOURNAL_CURSOR_KEY, &max_ts.to_string(), now_ms())?;
    }
    Ok(count)
}

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
    use super::journal_priority_level;

    #[test]
    fn journal_priority_maps() {
        assert_eq!(journal_priority_level(Some("3")), "error");
        assert_eq!(journal_priority_level(Some("4")), "warn");
        assert_eq!(journal_priority_level(Some("6")), "info");
    }
}
