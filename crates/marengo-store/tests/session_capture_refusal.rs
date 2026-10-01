//! Existing-public G14 malformed-date refusal before registration or archival.
//! All observations and fixture cleanup precede the collected refusal oracle.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use marengo_store::{blob_dir, log_dir, Store};
use serde_json::Value;

const INVALID_IDS: [&str; 10] = [
    "20230229T000000Z",
    "20201301T000000Z",
    "20200132T000000Z",
    "20200101T240000Z",
    "20200101T006000Z",
    "20200101T000060Z",
    "20200101T000000",
    "20200101T000000Zextra",
    "19691231T235959Z",
    "2020é101T000000Z",
];
const VALID_ID: &str = "20200101T000000Z";
const BENCH: &[u8] = b"valid new capture first\nvalid new capture last\n";
const CANDUMP: &[u8] = b"(0.000000) can0 123#1122\n(0.005000) can0 124#33\n";
const NEIGHBOR: &[u8] = b"independent healthy neighbor\n";
const NEIGHBOR_START: u64 = 123;
const NEIGHBOR_END: u64 = 456;

#[derive(Clone, Copy, Debug)]
enum Operation {
    Import,
    Archive,
}

#[derive(Debug)]
struct State {
    rows: Vec<Value>,
    hot_files: [Option<Vec<u8>>; 2],
    blobs: Vec<(PathBuf, Vec<u8>)>,
    neighbor: Value,
    neighbor_page: (Vec<String>, u32),
    neighbor_bytes: Vec<u8>,
}

#[derive(Debug)]
struct Observation {
    operation: Operation,
    invalid_id: &'static str,
    error: Option<String>,
    before: State,
    after: State,
    reopened: State,
    cleanup_complete: bool,
}

fn optional_bytes(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("fixture file observation failed for {path:?}: {error}"),
    }
}

fn blob_inventory(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(base: &Path, directory: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return,
            Err(error) => panic!("fixture blob inventory failed for {directory:?}: {error}"),
        };
        for entry in entries {
            let entry = entry.expect("actual fixture blob directory entry");
            let path = entry.path();
            let kind = entry.file_type().expect("actual fixture blob file type");
            if kind.is_dir() {
                visit(base, &path, files);
            } else {
                files.push((
                    path.strip_prefix(base)
                        .expect("fixture blob stays under its root")
                        .to_path_buf(),
                    fs::read(&path).expect("actual published or temporary blob bytes"),
                ));
            }
        }
    }
    let base = blob_dir(root);
    let mut files = Vec::new();
    visit(&base, &base, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn observe(store: &Store, root: &Path, paths: &[PathBuf; 2], neighbor: &Path) -> State {
    let mut rows = store
        .list_sessions(None, None, None, 50)
        .expect("complete public session list");
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    let rows = rows
        .into_iter()
        .map(|row| serde_json::to_value(row).expect("public row encoding"))
        .collect();
    let neighbor_row = store
        .get_session("neighbor")
        .expect("public healthy neighbor lookup")
        .expect("healthy neighbor remains present");
    State {
        rows,
        hot_files: [optional_bytes(&paths[0]), optional_bytes(&paths[1])],
        blobs: blob_inventory(root),
        neighbor: serde_json::to_value(neighbor_row).expect("public healthy neighbor encoding"),
        neighbor_page: store
            .read_bench_page("neighbor", 0, 10)
            .expect("actual healthy neighbor page"),
        neighbor_bytes: fs::read(neighbor).expect("actual healthy neighbor bytes"),
    }
}

fn exercise(operation: Operation, invalid_id: &'static str) -> Observation {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g14-capture-refusal-")
        .tempdir()
        .expect("exclusive fixture under root executor's J-backed TMP");
    let root = fixture.path().to_path_buf();
    let db = root.join("captures.sqlite");
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("actual hot capture directory");
    let store = Store::open(&db, &root).expect("actual writable SQLite Store");
    let neighbor = root.join("neighbor.bench");
    fs::write(&neighbor, NEIGHBOR).expect("literal healthy neighbor file");
    store
        .register_session(
            "neighbor",
            Some("authoritative healthy neighbor"),
            NEIGHBOR_START,
            Some(&neighbor),
            None,
            None,
        )
        .expect("public healthy neighbor registration");
    store
        .finalize_session("neighbor", NEIGHBOR_END)
        .expect("public authoritative healthy neighbor end");
    let paths = [
        hot.join(format!("bench-{VALID_ID}.log")),
        hot.join(format!("candump-{invalid_id}.log")),
    ];
    fs::write(&paths[0], BENCH).expect("literal valid new bench capture");
    fs::write(&paths[1], CANDUMP).expect("literal malformed-ID parseable candump capture");
    let before = observe(&store, &root, &paths, &neighbor);

    // A valid new bench artifact precedes the malformed candump kind in direct
    // archive order. No expected start is asserted for a partially inserted row.
    // Observe every actual result and state even when the operation returns Ok.
    let error = match operation {
        Operation::Import => store.import_legacy_hot_report(0).err(),
        Operation::Archive => store.archive_hot_sessions(0).err(),
    }
    .map(|error| error.to_string());
    let after = observe(&store, &root, &paths, &neighbor);
    drop(store);
    let reopened_store = Store::open(&db, &root).expect("actual database reopen after operation");
    let reopened = observe(&reopened_store, &root, &paths, &neighbor);
    drop(reopened_store);
    fixture
        .close()
        .expect("actual SQLite and artifact fixture cleanup");
    let cleanup_complete = !root
        .try_exists()
        .expect("actual fixture cleanup observation");
    Observation {
        operation,
        invalid_id,
        error,
        before,
        after,
        reopened,
        cleanup_complete,
    }
}

#[test]
fn invalid_new_capture_dates_are_refused_before_any_registration_or_archive() {
    let mut observations = Vec::new();
    for operation in [Operation::Import, Operation::Archive] {
        for invalid_id in INVALID_IDS {
            observations.push(exercise(operation, invalid_id));
        }
    }

    // All twenty fixtures have returned, reopened and been explicitly disposed.
    // Healthy controls must pass before any malformed-date refusal assertion.
    assert_eq!(observations.len(), 20);
    for observation in &observations {
        assert!(observation.cleanup_complete, "actual fixture cleanup");
        assert_eq!(observation.before.rows.len(), 1);
        assert_eq!(
            observation.before.hot_files,
            [Some(BENCH.to_vec()), Some(CANDUMP.to_vec())]
        );
        assert!(observation.before.blobs.is_empty());
        assert_eq!(observation.before.neighbor["id"], "neighbor");
        assert_eq!(
            observation.before.neighbor["label"],
            "authoritative healthy neighbor"
        );
        assert_eq!(observation.before.neighbor["started_ms"], NEIGHBOR_START);
        assert_eq!(observation.before.neighbor["ended_ms"], NEIGHBOR_END);
        for state in [
            &observation.before,
            &observation.after,
            &observation.reopened,
        ] {
            assert_eq!(state.neighbor, observation.before.neighbor);
            assert_eq!(state.neighbor_bytes, NEIGHBOR);
            assert_eq!(
                state.neighbor_page,
                (vec!["independent healthy neighbor".to_string()], 1)
            );
        }
    }
    println!("G14_REFUSAL_CONTROLS=complete;cleanup_complete=true;fixtures=20;invalid_ids=10;operations=2;healthy_neighbors=20");

    let mut failures = Vec::new();
    for observation in &observations {
        let case = format!("{:?}/{}", observation.operation, observation.invalid_id);
        if observation.error.is_none() {
            failures.push(format!(
                "{case}: operation returned success instead of refusal"
            ));
        }
        for (name, state) in [
            ("after", &observation.after),
            ("reopen", &observation.reopened),
        ] {
            if state.rows != observation.before.rows {
                failures.push(format!(
                    "{case}/{name}: public session rows changed: {:?}",
                    state.rows
                ));
            }
            if state.hot_files != observation.before.hot_files {
                failures.push(format!("{case}/{name}: original hot capture bytes changed"));
            }
            if state.blobs != observation.before.blobs {
                failures.push(format!(
                    "{case}/{name}: published or temporary blob output appeared"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "G14: invalid new capture dates must be refused before any registration or archive output while preserving original files: {failures:?}");
}
