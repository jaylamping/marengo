#![allow(clippy::expect_used)]

//! `log_archive_days` and `log_disk_budget_bytes` are enforced, not just stored.

use std::fs;
use std::path::{Path, PathBuf};

use marengo_store::{now_ms, RetentionPolicy, Store};
use time::OffsetDateTime;

const DAY_MS: u64 = 86_400_000;

fn open(dir: &Path) -> Store {
    Store::open(dir.join("var/marengo.db"), dir).expect("open store")
}

fn session_id(age_ms: u64) -> String {
    let at = OffsetDateTime::from_unix_timestamp(
        i64::try_from((now_ms() - age_ms) / 1000).expect("timestamp"),
    )
    .expect("capture time");
    let format = time::format_description::parse("[year][month][day]T[hour][minute][second]Z")
        .expect("format");
    at.format(&format).expect("session id")
}

fn artifact(dir: &Path, name: &str, len: usize) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, vec![b'x'; len]).expect("artifact");
    path
}

#[test]
fn policy_reads_the_seeded_settings() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = open(dir.path());
    store.set_setting("log_archive_days", "7", 1).expect("days");
    store
        .set_setting("log_disk_budget_bytes", "4096", 1)
        .expect("budget");
    assert_eq!(
        store.retention_policy().expect("policy"),
        RetentionPolicy {
            archive_days: 7,
            disk_budget_bytes: 4096
        }
    );
    store
        .set_setting("log_archive_days", "\"soon\"", 2)
        .expect("bad days");
    assert!(store.retention_policy().is_err());
}

#[test]
fn hot_capture_older_than_log_archive_days_is_archived_then_purged() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = open(dir.path());
    store.set_setting("log_archive_days", "7", 1).expect("days");

    let hot = marengo_store::log_dir(dir.path());
    fs::create_dir_all(&hot).expect("hot dir");
    let old_id = session_id(10 * DAY_MS);
    let fresh_id = session_id(DAY_MS);
    let old_hot = artifact(&hot, &format!("bench-{old_id}.log"), 64);
    let fresh_hot = artifact(&hot, &format!("bench-{fresh_id}.log"), 64);

    // The maintenance unit archives hot files, then purges.
    assert_eq!(store.archive_hot_sessions(0).expect("archive"), 2);
    let old_blob = PathBuf::from(
        store
            .get_session(&old_id)
            .expect("lookup")
            .expect("archived old session")
            .bench_blob
            .expect("old blob"),
    );
    let fresh_blob = PathBuf::from(
        store
            .get_session(&fresh_id)
            .expect("lookup")
            .expect("archived fresh session")
            .bench_blob
            .expect("fresh blob"),
    );
    assert!(old_blob.exists() && fresh_blob.exists());
    assert!(!old_hot.exists() && !fresh_hot.exists());

    let report = store.enforce_retention().expect("enforce");
    assert_eq!(report.aged_sessions, 1);
    assert_eq!(report.budget_sessions, 0, "default budget is not exceeded");
    assert!(store.get_session(&old_id).expect("lookup").is_none());
    assert!(!old_blob.exists());
    assert!(store.get_session(&fresh_id).expect("lookup").is_some());
    assert!(fresh_blob.exists());
}

#[test]
fn exceeding_log_disk_budget_evicts_oldest_sessions_but_keeps_the_newest() {
    const ARTIFACT_BYTES: usize = 100_000;
    let dir = tempfile::tempdir().expect("temp dir");
    let store = open(dir.path());
    store
        .set_setting("log_archive_days", "3650", 1)
        .expect("days");
    let hot = marengo_store::log_dir(dir.path());
    fs::create_dir_all(&hot).expect("hot dir");
    let files: Vec<PathBuf> = (0..3)
        .map(|i| artifact(&hot, &format!("capture-{i}.log"), ARTIFACT_BYTES))
        .collect();
    for (i, file) in files.iter().enumerate() {
        let age = (3 - i as u64) * DAY_MS;
        store
            .register_session(
                &format!("session-{i}"),
                None,
                now_ms() - age,
                Some(file),
                None,
                None,
            )
            .expect("register");
    }

    // Under budget: nothing is touched. The purge checkpoints the WAL, so this
    // first pass also settles the database size before the next measurement.
    store
        .set_setting("log_disk_budget_bytes", "1000000000", 2)
        .expect("budget");
    let report = store.enforce_retention().expect("under budget");
    assert_eq!((report.aged_sessions, report.budget_sessions), (0, 0));
    assert!(files.iter().all(|file| file.exists()));

    // Needing 150 kB freed removes the two oldest 100 kB sessions only. The
    // 50 kB margin on each side absorbs database-size jitter.
    let usage = store.log_disk_usage_bytes().expect("usage");
    let budget = usage - 150_000;
    store
        .set_setting("log_disk_budget_bytes", &budget.to_string(), 3)
        .expect("budget");
    let report = store.enforce_retention().expect("over budget");
    assert_eq!(report.budget_sessions, 2);
    assert_eq!(report.aged_sessions, 0);
    assert!(report.usage_bytes <= budget);
    assert!(!files[0].exists() && !files[1].exists());
    assert!(files[2].exists());
    assert!(store.get_session("session-0").expect("lookup").is_none());
    assert!(store.get_session("session-1").expect("lookup").is_none());
    assert!(store.get_session("session-2").expect("lookup").is_some());

    // An impossible budget still never evicts the newest session.
    store
        .set_setting("log_disk_budget_bytes", "0", 4)
        .expect("budget");
    let report = store.enforce_retention().expect("zero budget");
    assert_eq!(report.budget_sessions, 0);
    assert!(files[2].exists());
    assert!(store.get_session("session-2").expect("lookup").is_some());
}
