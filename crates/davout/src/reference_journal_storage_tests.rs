//! Concrete SQL boundary tests using an input from a real closed acquisition.
#![allow(clippy::expect_used)]
use super::*;
use crate::reference_journal_tests::support;
fn resource() -> support::FixtureTree {
    support::FixtureTree::new(
        "journal-sql",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
}
fn state() -> Shared {
    Shared {
        state: Mutex::new(State::default()),
        changed: Condvar::new(),
    }
}

#[test]
fn actual_sql_unsigned_key_equal_retry_and_conflicting_body_are_exact() {
    let tree = resource();
    let path = tree.path().join("journal.sqlite3");
    let mut database = Database::open(&path, true, true).expect("concrete pinned connection");
    let mut input = crate::reference_journal_tests::actual_input();
    let original = input
        .encode(database.session, u64::MAX)
        .expect("full-width unsigned key body");
    let shared = state();
    let write = database
        .append(u64::MAX, &original, &shared, None)
        .expect("actual SQL transaction/readback");
    assert!(matches!(
        write,
        ReferenceJournalResult::DurableHistory {
            job_sequence: u64::MAX,
            ..
        }
    ));
    assert_eq!(
        database
            .append(u64::MAX, &original, &shared, None)
            .expect("equal SQL retry"),
        write
    );
    input.capture.audit.session.push('2');
    let changed = input
        .encode(database.session, u64::MAX)
        .expect("different valid body");
    assert!(matches!(
        database.append(u64::MAX, &changed, &shared, None),
        Err(ReferenceJournalResult::Failed { .. })
    ));
    let count: i64 = database
        .connection
        .query_row("SELECT count(*) FROM reference_events", [], |row| {
            row.get(0)
        })
        .expect("real row count");
    assert_eq!(count, 1);
    drop(database);
    let records = inspect(&path, 8).expect("actual reopened unsigned history");
    assert_eq!(records[0].job_sequence(), u64::MAX);
    assert_eq!(
        records[0].audit().expect("typed audit").session,
        "fixture-session"
    );
}

#[test]
fn actual_sql_full_error_keeps_preexisting_history_and_refuses_success() {
    let tree = resource();
    let path = tree.path().join("journal.sqlite3");
    let mut database = Database::open(&path, true, true).expect("real bounded SQLite connection");
    let configured: i64 = database
        .connection
        .pragma_query_value(None, "max_page_count", |row| row.get(0))
        .expect("actual configured cap");
    assert_eq!(configured, 16_384);
    let pages: i64 = database
        .connection
        .pragma_query_value(None, "page_count", |row| row.get(0))
        .expect("existing pages");
    // Lower the actual pager limit only in this owned fixture to reach SQLITE_FULL quickly.
    database
        .connection
        .pragma_update(None, "max_page_count", pages)
        .expect("actual smaller fixture storage capacity");
    let input = crate::reference_journal_tests::actual_input();
    let body = input
        .encode(database.session, 1)
        .expect("real captured body");
    let result = database
        .append(1, &body, &state(), None)
        .expect_err("actual pager must refuse additional pages");
    assert!(matches!(
        result,
        ReferenceJournalResult::Failed { .. } | ReferenceJournalResult::Uncertain { .. }
    ));
    assert!(format!("{result:?}").contains("full"));
    assert_eq!(
        database
            .connection
            .query_row::<i64, _, _>("SELECT count(*) FROM reference_events", [], |row| row
                .get(0))
            .expect("post-failure readback"),
        0
    );
    drop(database);
    assert!(inspect(&path, 8)
        .expect("intact actual database")
        .is_empty());
}

/// REF-01: a writer opening a new session drops whole oldest sessions, so a
/// full journal can never refuse the row a grant needs.
#[test]
fn opening_a_session_drops_the_oldest_sessions_past_retention() {
    let tree = resource();
    let path = tree.path().join("journal.sqlite3");
    let input = crate::reference_journal_tests::actual_input();
    let mut sessions = Vec::new();
    for _ in 0..3 {
        let mut database = Database::open(&path, true, true).expect("session");
        for job in 1..=2 {
            let body = input.encode(database.session, job).expect("body");
            database
                .append(job, &body, &state(), None)
                .expect("durable row");
        }
        sessions.push(database.session);
    }
    let mut database = Database::open(&path, true, true).expect("fourth session");
    let dropped = database
        .prune_oldest_sessions(4, PAGE_CAPACITY)
        .expect("prune");
    assert_eq!(dropped, 1);
    let kept: Vec<u64> = {
        let mut query = database
            .connection
            .prepare("SELECT DISTINCT session FROM reference_events ORDER BY session")
            .expect("query");
        query
            .query_map([], |row| row.get::<_, i64>(0))
            .expect("rows")
            .map(|session| session.expect("session") as u64)
            .collect()
    };
    assert_eq!(kept, sessions[1..].to_vec(), "only the oldest session goes");
    let body = input.encode(database.session, 1).expect("body");
    database
        .append(1, &body, &state(), None)
        .expect("the new session still writes");
    assert_eq!(
        database
            .prune_oldest_sessions(0, PAGE_CAPACITY)
            .expect("prune"),
        2,
        "every older session may go, never the current one"
    );
    let left: i64 = database
        .connection
        .query_row("SELECT count(*) FROM reference_events", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(left, 1);
}
