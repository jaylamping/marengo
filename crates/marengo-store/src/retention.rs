//! Enforcement of the persisted `log_archive_days` and `log_disk_budget_bytes`
//! settings (seeded by migration, shown by the gateway `/settings` route).

use std::fs;

use crate::error::{Result, StoreError};
use crate::model::LogSessionRow;
use crate::store::{map_session_row, remove_session_files, Store};

const ARCHIVE_DAYS_KEY: &str = "log_archive_days";
const DISK_BUDGET_KEY: &str = "log_disk_budget_bytes";

/// Retention limits read from the `settings` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Sessions and log rows older than this many days are purged.
    pub archive_days: u32,
    /// Upper bound for the database plus everything under `var/log`.
    pub disk_budget_bytes: u64,
}

/// What one [`Store::enforce_retention`] call removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetentionReport {
    pub log_rows: u64,
    /// Sessions removed because they were older than `archive_days`.
    pub aged_sessions: u64,
    /// Sessions removed, oldest first, to get back under the disk budget.
    pub budget_sessions: u64,
    /// Log disk usage after enforcement.
    pub usage_bytes: u64,
    pub budget_bytes: u64,
}

impl Store {
    /// Read both retention settings; a missing or malformed value is an error.
    pub fn retention_policy(&self) -> Result<RetentionPolicy> {
        Ok(RetentionPolicy {
            archive_days: self.numeric_setting(ARCHIVE_DAYS_KEY)?,
            disk_budget_bytes: self.numeric_setting(DISK_BUDGET_KEY)?,
        })
    }

    /// Purge by age (`log_archive_days`), then evict the oldest sessions until
    /// log disk usage fits `log_disk_budget_bytes`.
    ///
    /// The newest session is never evicted by the budget pass, so an
    /// undersized budget cannot delete the capture currently being written.
    /// Usage that stays over budget after every older session is gone (for
    /// example a large database) is reported in `usage_bytes`, not an error.
    pub fn enforce_retention(&self) -> Result<RetentionReport> {
        let policy = self.retention_policy()?;
        let (log_rows, aged_sessions) = self.purge_older_than_days(policy.archive_days)?;
        let (budget_sessions, usage_bytes) = self.evict_to_budget(policy.disk_budget_bytes)?;
        Ok(RetentionReport {
            log_rows,
            aged_sessions,
            budget_sessions,
            usage_bytes,
            budget_bytes: policy.disk_budget_bytes,
        })
    }

    fn evict_to_budget(&self, budget_bytes: u64) -> Result<(u64, u64)> {
        let usage = self.log_disk_usage_bytes()?;
        if usage <= budget_bytes {
            return Ok((0, usage));
        }
        let conn = self.connection();
        let mut sessions: Vec<LogSessionRow> = {
            let mut stmt = conn.prepare(
                "SELECT id, label, started_ms, ended_ms, bench_blob, candump_blob, trace_blob,
                        candump_frame_count, candump_bytes
                 FROM log_sessions ORDER BY started_ms ASC, id ASC",
            )?;
            let rows = stmt.query_map([], map_session_row)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        sessions.pop();

        // Choose victims oldest-first until the accounted usage fits, then
        // remove files before rows in one transaction (same crash ordering
        // as the age purge: re-drivable rows, never orphan blobs). Removal
        // failures are reported by `remove_session_files` and do not abort.
        let mut victims: Vec<&LogSessionRow> = Vec::new();
        let mut accounted = usage;
        for session in &sessions {
            if accounted <= budget_bytes {
                break;
            }
            let freed: u64 = [
                &session.bench_blob,
                &session.candump_blob,
                &session.trace_blob,
            ]
            .into_iter()
            .flatten()
            .filter_map(|path| fs::symlink_metadata(path).ok())
            .filter(fs::Metadata::is_file)
            .map(|meta| meta.len())
            .sum();
            accounted = accounted.saturating_sub(freed);
            victims.push(session);
        }
        for victim in &victims {
            remove_session_files(victim);
        }
        let tx = conn.unchecked_transaction()?;
        for victim in &victims {
            tx.execute(
                "DELETE FROM log_sessions WHERE id = ?1",
                rusqlite::params![victim.id],
            )?;
        }
        tx.commit()?;
        Ok((victims.len() as u64, accounted))
    }

    fn numeric_setting<T: std::str::FromStr>(&self, key: &str) -> Result<T> {
        let raw = self
            .get_setting(key)?
            .ok_or_else(|| StoreError::msg(format!("retention setting {key} is missing")))?;
        raw.trim().parse().map_err(|_| {
            StoreError::msg(format!(
                "retention setting {key} must be a non-negative integer, found {raw:?}"
            ))
        })
    }
}
