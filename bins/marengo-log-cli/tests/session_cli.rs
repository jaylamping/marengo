//! Real CLI composition against disposable SQLite and literal hot artifacts.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use marengo_store::{log_dir, Store};

const ID: &str = "20200101T000000Z";
const BYTES: [&[u8]; 3] = [
    b"bench complete\n",
    b"(0.000000) can0 123#1122\n",
    b"time,q\n1.0,0.25\n",
];
const NAMES: [&str; 3] = [
    "bench-20200101T000000Z.log",
    "candump-20200101T000000Z.log",
    "position-trace-20200101T000000Z.csv",
];

fn run(root: &Path, db: &Path, args: &[&str]) -> Output {
    Command::new(assert_cmd::cargo::cargo_bin("marengo-log-cli"))
        .arg("--root")
        .arg(root)
        .arg("--db")
        .arg(db)
        .args(args)
        .output()
        .expect("actual CLI child completed")
}

fn successful(output: &Output) -> String {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("CLI stdout is UTF-8")
}

fn write_hot(root: &Path) {
    let hot = log_dir(root);
    fs::create_dir_all(&hot).expect("hot directory");
    for (name, bytes) in NAMES.into_iter().zip(BYTES) {
        fs::write(hot.join(name), bytes).expect("literal hot capture");
    }
}

#[test]
fn cli_reports_unique_imports_and_clears_only_the_selected_reference() {
    let fixture = tempfile::tempdir().expect("exclusive CLI fixture");
    let root = fixture.path();
    let db = root.join("sessions.sqlite");
    write_hot(root);
    let store = Store::open(&db, root).expect("seeded real Store");
    store
        .register_session(ID, Some("capture label"), 123, None, None, None)
        .expect("authoritative capture metadata");
    store.finalize_session(ID, 456).expect("authoritative end");
    drop(store);

    let first = run(root, &db, &["import-legacy", "--keep", "50"]);
    let repeat = run(root, &db, &["import-legacy", "--keep", "50"]);
    let archive = run(root, &db, &["archive", "--keep", "0"]);
    assert_eq!(
        successful(&first).trim(),
        "imported 1 legacy sessions, 3 artifacts"
    );
    assert_eq!(
        successful(&repeat).trim(),
        "imported 1 legacy sessions, 3 artifacts"
    );
    assert_eq!(successful(&archive).trim(), "archived 3 hot files (keep 0)");
    let store = Store::open(&db, root).expect("reopen after CLI archive");
    let before = store.get_session(ID).expect("row").expect("session");
    assert_eq!(before.label.as_deref(), Some("capture label"));
    assert_eq!((before.started_ms, before.ended_ms), (123, Some(456)));
    assert_eq!(before.candump_frame_count, Some(1));
    assert_eq!(
        store.read_bench_page(ID, 0, 10).expect("bench"),
        (vec!["bench complete".into()], 1)
    );
    assert_eq!(
        store
            .read_candump_page(ID, 0, 10)
            .expect("candump")
            .summary
            .parsed_frames,
        1
    );
    assert_eq!(
        store.read_trace_page(ID, 0, 10).expect("trace"),
        (vec!["time,q".into(), "1.0,0.25".into()], 2)
    );
    let paths = [
        before.bench_blob.clone(),
        before.candump_blob.clone(),
        before.trace_blob.clone(),
    ];
    let archived_bytes: Vec<_> = paths
        .iter()
        .map(|path| {
            fs::read(path.as_ref().expect("registered file")).expect("actual archived bytes")
        })
        .collect();
    drop(store);

    // Each public enum value must reach the right Store operation, including
    // duplicate and absent clearing; files remain even after the last clear.
    for (index, kind) in ["bench", "candump", "trace"].into_iter().enumerate() {
        let cleared = run(
            root,
            &db,
            &["session", "clear-artifact", "--id", ID, "--artifact", kind],
        );
        assert!(successful(&cleared).contains("file preserved"));
        let duplicate = run(
            root,
            &db,
            &["session", "clear-artifact", "--id", ID, "--artifact", kind],
        );
        assert!(successful(&duplicate).starts_with("no "));
        let store = Store::open(&db, root).expect("reopen after selected CLI clear");
        let row = store.get_session(ID).expect("row").expect("session");
        assert_eq!(row.label, before.label);
        assert_eq!((row.started_ms, row.ended_ms), (123, Some(456)));
        let references = [&row.bench_blob, &row.candump_blob, &row.trace_blob];
        for position in 0..3 {
            assert_eq!(
                references[position],
                if position <= index {
                    &None
                } else {
                    &paths[position]
                }
            );
            assert_eq!(
                fs::read(paths[position].as_ref().expect("original file")).expect("preserved file"),
                archived_bytes[position]
            );
        }
        if index >= 1 {
            assert_eq!((row.candump_frame_count, row.candump_bytes), (None, None));
            assert!(store.read_candump_page(ID, 0, 10).is_err());
        }
    }
    let absent = run(
        root,
        &db,
        &[
            "session",
            "clear-artifact",
            "--id",
            "absent",
            "--artifact",
            "bench",
        ],
    );
    assert!(successful(&absent).starts_with("no "));
    let empty_import = run(root, &db, &["import-legacy", "--keep", "50"]);
    assert_eq!(
        successful(&empty_import).trim(),
        "imported 0 legacy sessions, 0 artifacts"
    );
    fixture.close().expect("actual CLI fixture disposal");
}

#[test]
fn cli_argument_and_database_failures_preserve_capture_files_and_rows() {
    let fixture = tempfile::tempdir().expect("exclusive refusal fixture");
    let root = fixture.path();
    let db = root.join("sessions.sqlite");
    write_hot(root);
    let store = Store::open(&db, root).expect("real Store");
    let bench = log_dir(root).join(NAMES[0]);
    store
        .register_session(ID, Some("preserve"), 77, Some(&bench), None, None)
        .expect("existing row");
    store.finalize_session(ID, 88).expect("end");
    let before = serde_json::to_value(store.get_session(ID).expect("row")).expect("row encoding");
    drop(store);
    let invalid = run(
        root,
        &db,
        &["session", "clear-artifact", "--id", ID, "--artifact", "all"],
    );
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid value"));
    let unavailable_db = root.join("database-is-a-directory");
    fs::create_dir(&unavailable_db).expect("real unavailable SQLite path");
    let failed = run(root, &unavailable_db, &["import-legacy", "--keep", "0"]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("sqlite:"));
    let store = Store::open(&db, root).expect("reopen after refused commands");
    assert_eq!(
        serde_json::to_value(store.get_session(ID).expect("row")).expect("row encoding"),
        before
    );
    for (name, bytes) in NAMES.into_iter().zip(BYTES) {
        assert_eq!(
            fs::read(log_dir(root).join(name)).expect("preserved hot file"),
            bytes
        );
    }
    drop(store);
    fixture.close().expect("actual refusal fixture disposal");
}
