#![allow(clippy::expect_used)]

use std::fs;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use marengo_store::{now_ms, LogEventInsert, Store, StructuredLogQuery};

/// Exercise the public retention API in a child so a lock regression has a deadline.
#[test]
fn retention_removes_expired_sessions_and_artifacts_without_blocking() {
    const WORKER_ENV: &str = "MARENGO_RETENTION_TEST_WORKER";
    if std::env::var_os(WORKER_ENV).is_some() {
        exercise_retention();
        return;
    }

    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "retention_removes_expired_sessions_and_artifacts_without_blocking",
            "--nocapture",
        ])
        .env(WORKER_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("retention child");
    let mut stdout = child.stdout.take().expect("child output");
    let (finished, completion) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        let result = stdout.read_to_string(&mut output);
        let _ = finished.send((result, output));
    });
    let result = completion.recv_timeout(Duration::from_secs(5));
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        assert!(
            result.is_ok(),
            "Store::purge_older_than_days did not complete within five seconds"
        );
        return;
    }
    let (read_result, output) = result.expect("completion");
    read_result.expect("read child output");
    let status = child.wait().expect("child exit");
    reader.join().expect("reader exit");
    assert!(status.success(), "retention child failed: {output}");
}

fn exercise_retention() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("logs.db");
    let store = Store::open(&db, dir.path()).expect("open store");
    let expired: Vec<_> = ["bench", "candump", "trace"]
        .map(|kind| dir.path().join(format!("expired.{kind}")))
        .into_iter()
        .collect();
    let retained = dir.path().join("retained.bench");
    let unrelated = dir.path().join("unregistered.bench");
    for path in expired.iter().chain([&retained, &unrelated]) {
        fs::write(path, b"recorded artifact").expect("write artifact");
    }
    store
        .register_session(
            "expired",
            Some("expired"),
            1,
            Some(&expired[0]),
            Some(&expired[1]),
            Some(&expired[2]),
        )
        .expect("old session");
    let future_ms = now_ms() + 86_400_000;
    store
        .register_session(
            "retained",
            Some("retained"),
            future_ms,
            Some(&retained),
            None,
            None,
        )
        .expect("new session");
    let event = |ts_ms, message: &str| LogEventInsert {
        ts_ms,
        level: "info".into(),
        target: "retention-test".into(),
        message: message.into(),
        session_id: None,
        fields_json: None,
    };
    store
        .insert_log_events(&[
            event(1, "expired observation"),
            event(future_ms, "retained observation"),
        ])
        .expect("logs");

    assert_eq!(store.purge_older_than_days(30).expect("purge"), (1, 1));
    assert!(store.get_session("expired").expect("old lookup").is_none());
    assert!(store.get_session("retained").expect("new lookup").is_some());
    assert!(expired.iter().all(|path| !path.exists()));
    assert!(retained.exists());
    assert!(unrelated.exists());
    assert_eq!(
        store.purge_older_than_days(30).expect("repeat purge"),
        (0, 0)
    );
    drop(store);

    let reopened = Store::open(&db, dir.path()).expect("reopen store");
    let query = StructuredLogQuery {
        from_ms: None,
        to_ms: None,
        level: None,
        target: None,
        session_id: None,
        q: Some("observation".into()),
        offset: 0,
        limit: 10,
    };
    let (logs, total) = reopened
        .query_structured_logs(&query)
        .expect("search retained logs");
    assert_eq!(total, 1);
    assert_eq!(logs[0].message, "retained observation");
}
