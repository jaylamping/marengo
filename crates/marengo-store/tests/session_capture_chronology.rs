//! PROPOSED ONLY: root must review, freeze and execute against a checked base.
//! Public Store import/archive/filter/reopen; no private parser or wall clock.
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use marengo_store::{blob_dir, log_dir, LogSessionRow, Store};

// Independent Gregorian UTC examples, calculated with Python datetime calendar
// subtraction from its 1970-01-01 UTC epoch, not the production Rust parser.
const IMPORTED: [(&str, &str, u64); 3] = [
    ("20200101T000000Z", "2020-01-01", 1_577_836_800_000),
    ("20000229T123456Z", "2000-02-29", 951_827_696_000),
    ("20240229T235959Z", "2024-02-29", 1_709_251_199_000),
];
const DIRECT_ARCHIVE: &str = "19700101T000000Z";
const EXPLICIT: &str = "19991231T235959Z";
const EXPLICIT_START: u64 = 123;
const EXPLICIT_END: u64 = 456;
const LABEL: &str = "operator supplied capture metadata";
const BENCH: &[u8] = b"first captured event\nlast captured event\n";
const NEIGHBOR: &[u8] = b"neighbor remains outside archive\n";

#[derive(Debug)]
struct Stage {
    rows: Vec<LogSessionRow>,
    exact_date_hits: Vec<Vec<String>>,
    pages: Vec<(Vec<String>, u32)>,
}

fn row(store: &Store, id: &str) -> LogSessionRow {
    store
        .get_session(id)
        .expect("public session lookup succeeds")
        .expect("capture row exists")
}

fn observe(store: &Store, include_direct: bool) -> Stage {
    let mut ids: Vec<_> = IMPORTED.iter().map(|(id, _, _)| *id).collect();
    if include_direct {
        ids.push(DIRECT_ARCHIVE);
    }
    ids.push(EXPLICIT);
    let rows = ids.iter().map(|id| row(store, id)).collect();
    let exact_date_hits = IMPORTED
        .iter()
        .map(|(_, _, expected_ms)| {
            store
                .list_sessions(Some(*expected_ms), Some(*expected_ms), None, 50)
                .expect("public inclusive capture date filter")
                .into_iter()
                .map(|row| row.id)
                .collect()
        })
        .collect();
    let pages = ids
        .iter()
        .map(|id| {
            store
                .read_bench_page(id, 1, 1)
                .expect("actual public bench content")
        })
        .collect();
    Stage {
        rows,
        exact_date_hits,
        pages,
    }
}

fn decode_file(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).expect("actual archive bytes exist");
    assert!(bytes.starts_with(&[0x1f, 0x8b]), "gzip magic control");
    let mut decoded = Vec::new();
    GzDecoder::new(bytes.as_slice())
        .read_to_end(&mut decoded)
        .expect("independent gzip content decode");
    decoded
}

fn chronology_failures(name: &str, stage: &Stage, failures: &mut Vec<String>) {
    for (id, _, expected_ms) in IMPORTED {
        let observed = stage
            .rows
            .iter()
            .find(|row| row.id == id)
            .expect("observed capture");
        if observed.started_ms != expected_ms {
            failures.push(format!(
                "{name}: {id} started_ms={} expected={expected_ms}",
                observed.started_ms
            ));
        }
    }
    for (index, (id, _, _)) in IMPORTED.iter().enumerate() {
        if stage.exact_date_hits[index] != vec![id.to_string()] {
            failures.push(format!(
                "{name}: exact UTC filter for {id} returned {:?}",
                stage.exact_date_hits[index]
            ));
        }
    }
    if let Some(direct) = stage.rows.iter().find(|row| row.id == DIRECT_ARCHIVE) {
        if direct.started_ms != 0 {
            failures.push(format!(
                "{name}: direct archive epoch started_ms={} expected=0",
                direct.started_ms
            ));
        }
    }
}

#[test]
fn import_and_unregistered_archive_preserve_utc_capture_dates_through_reopen() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g14-capture-chronology-")
        .tempdir()
        .expect("exclusive fixture under root executor's J-backed TMP");
    let root = fixture.path().to_path_buf();
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("real hot directory");
    let db = root.join("captures.sqlite");
    let store = Store::open(&db, &root).expect("actual SQLite Store");
    store
        .register_session(EXPLICIT, Some(LABEL), EXPLICIT_START, None, None, None)
        .expect("explicit capture metadata");
    store
        .finalize_session(EXPLICIT, EXPLICIT_END)
        .expect("explicit capture end");
    let neighbor_path = root.join("neighbor.bench");
    fs::write(&neighbor_path, NEIGHBOR).expect("independent neighboring file");
    store
        .register_session(
            "neighbor",
            Some("neighbor"),
            9,
            Some(&neighbor_path),
            None,
            None,
        )
        .expect("neighbor row");
    store
        .finalize_session("neighbor", 10)
        .expect("neighbor end");
    let neighbor_before =
        serde_json::to_value(row(&store, "neighbor")).expect("public row encoding");
    let mut hot_paths = Vec::new();
    for id in IMPORTED.iter().map(|(id, _, _)| *id).chain([EXPLICIT]) {
        let path = hot.join(format!("bench-{id}.log"));
        fs::write(&path, BENCH).expect("literal capture artifact");
        hot_paths.push(path);
    }

    // No capture start is seeded for these three import rows: their IDs are the
    // independent capture evidence. The fourth row's explicit start must win.
    let imported_count = store.import_legacy_hot(50).expect("actual import");
    let imported = observe(&store, false);
    let repeated_count = store.import_legacy_hot(50).expect("actual repeat import");
    let repeated = observe(&store, false);
    let hot_bytes: Vec<_> = hot_paths
        .iter()
        .map(|path| fs::read(path).expect("kept hot bytes"))
        .collect();
    let direct_was_absent = store
        .get_session(DIRECT_ARCHIVE)
        .expect("public absent row lookup")
        .is_none();
    let direct_path = hot.join("bench-19700101T000000Z.log");
    fs::write(&direct_path, BENCH).expect("new unregistered epoch capture file");
    hot_paths.push(direct_path);
    let archived_count = store.archive_hot_sessions(0).expect("actual archive");
    let archived = observe(&store, true);
    let expected_archive_paths: Vec<_> = IMPORTED
        .iter()
        .map(|(id, day, _)| blob_dir(&root).join(day).join(format!("bench-{id}.log.gz")))
        .chain([
            blob_dir(&root).join("1970-01-01/bench-19700101T000000Z.log.gz"),
            blob_dir(&root).join("1999-12-31/bench-19991231T235959Z.log.gz"),
        ])
        .collect();
    let archive_payloads: Vec<_> = expected_archive_paths
        .iter()
        .map(|path| decode_file(path))
        .collect();
    let hot_removed: Vec<_> = hot_paths
        .iter()
        .map(|path| !path.try_exists().expect("hot removal observation"))
        .collect();
    let neighbor_after =
        serde_json::to_value(row(&store, "neighbor")).expect("public neighbor row encoding");
    drop(store);

    let reopened_store = Store::open(&db, &root).expect("real database reopen");
    let reopened = observe(&reopened_store, true);
    let neighbor_reopened = serde_json::to_value(row(&reopened_store, "neighbor"))
        .expect("reopened public neighbor row encoding");
    let neighbor_bytes = fs::read(&neighbor_path).expect("neighbor bytes after archive/reopen");
    let epoch_hits: Vec<_> = reopened_store
        .list_sessions(Some(0), Some(0), None, 50)
        .expect("public epoch-zero filter")
        .into_iter()
        .map(|row| row.id)
        .collect();
    let final_count = reopened_store
        .list_sessions(None, None, None, 50)
        .expect("complete public list")
        .len();
    drop(reopened_store);
    fixture
        .close()
        .expect("actual database/artifact fixture cleanup");
    let cleanup_complete = !root.try_exists().expect("actual cleanup observation");

    // Classify chronology only after actual import, repeat, archive, reopen,
    // independent byte/row controls and complete fixture disposal.
    assert!(cleanup_complete);
    assert_eq!((imported_count, repeated_count, archived_count), (4, 4, 5));
    assert_eq!(hot_bytes, vec![BENCH.to_vec(); 4]);
    assert!(direct_was_absent);
    assert!(hot_removed.into_iter().all(|removed| removed));
    assert_eq!(archive_payloads, vec![BENCH.to_vec(); 5]);
    assert_eq!(final_count, 6);
    assert_eq!(neighbor_before, neighbor_after);
    assert_eq!(neighbor_before, neighbor_reopened);
    assert_eq!(neighbor_bytes, NEIGHBOR);
    for stage in [&imported, &repeated, &archived, &reopened] {
        assert!(stage
            .pages
            .iter()
            .all(|page| *page == (vec!["last captured event".into()], 2)));
        let explicit = stage
            .rows
            .iter()
            .find(|row| row.id == EXPLICIT)
            .expect("explicit capture observed");
        assert_eq!(explicit.label.as_deref(), Some(LABEL));
        assert_eq!(
            (explicit.started_ms, explicit.ended_ms),
            (EXPLICIT_START, Some(EXPLICIT_END))
        );
    }
    println!("G14_CAPTURE_CONTROLS=complete;cleanup_complete=true;import_rows=4;archive_artifacts=5;neighbor_rows=1");

    let mut failures = Vec::new();
    for (name, stage) in [
        ("import", &imported),
        ("repeat", &repeated),
        ("archive", &archived),
        ("reopen", &reopened),
    ] {
        chronology_failures(name, stage, &mut failures);
    }
    if epoch_hits != vec![DIRECT_ARCHIVE.to_string()] {
        failures.push(format!(
            "reopen: exact epoch-zero filter returned {epoch_hits:?}"
        ));
    }
    assert!(failures.is_empty(), "G14: literal UTC capture dates must survive import/archive/reopen and drive exact date filters: {failures:?}");
}
