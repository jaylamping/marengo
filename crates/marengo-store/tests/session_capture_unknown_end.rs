//! Existing-public G14 unknown import end and explicit finalization preservation.
//! Newly inferred starts are deliberately outside this end-only baseline oracle.
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use marengo_store::{log_dir, LogSessionRow, Store};
use serde_json::Value;

const IMPORTED: &str = "20200101T000000Z";
const KNOWN: &str = "20000229T123456Z";
const DIRECT: &str = "19700101T000000Z";
const KNOWN_START: u64 = 77;
const KNOWN_END: u64 = 88;
const FINALIZED_END: u64 = 1_577_836_800_500;
const PAYLOADS: [&[u8]; 3] = [
    b"imported first\nimported last\n",
    b"known first\nknown last\n",
    b"direct first\ndirect last\n",
];
const LAST_LINES: [&str; 3] = ["imported last", "known last", "direct last"];
const NEIGHBOR: &[u8] = b"independent end metadata neighbor\n";

#[derive(Debug)]
struct Capture {
    row: LogSessionRow,
    compressed: bool,
    payload: Vec<u8>,
    page: (Vec<String>, u32),
}

#[derive(Debug)]
struct Stage {
    captures: Vec<Capture>,
    neighbor: Value,
    neighbor_page: (Vec<String>, u32),
}

fn capture(store: &Store, id: &str) -> Capture {
    let row = store
        .get_session(id)
        .expect("actual public capture lookup")
        .expect("actual captured row exists");
    let path = row.bench_blob.as_deref().expect("actual bench reference");
    let bytes = fs::read(path).expect("actual referenced artifact bytes");
    let compressed = bytes.starts_with(&[0x1f, 0x8b]);
    let payload = if compressed {
        let mut decoded = Vec::new();
        GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut decoded)
            .expect("independent actual-reference gzip decode");
        decoded
    } else {
        bytes
    };
    let page = store
        .read_bench_page(id, 1, 1)
        .expect("actual public captured bench page");
    Capture {
        row,
        compressed,
        payload,
        page,
    }
}

fn observe(store: &Store, include_direct: bool) -> Stage {
    let mut ids = vec![IMPORTED, KNOWN];
    if include_direct {
        ids.push(DIRECT);
    }
    let neighbor = store
        .get_session("neighbor")
        .expect("public independent neighbor lookup")
        .expect("independent neighbor stays present");
    Stage {
        captures: ids.into_iter().map(|id| capture(store, id)).collect(),
        neighbor: serde_json::to_value(neighbor).expect("public neighbor metadata encoding"),
        neighbor_page: store
            .read_bench_page("neighbor", 0, 10)
            .expect("actual independent neighbor page"),
    }
}

fn row<'a>(stage: &'a Stage, id: &str) -> &'a LogSessionRow {
    &stage
        .captures
        .iter()
        .find(|capture| capture.row.id == id)
        .expect("observed capture row")
        .row
}

fn file_absent(path: &Path) -> bool {
    !path
        .try_exists()
        .expect("actual hot capture removal observation")
}

#[test]
fn imported_capture_end_stays_unknown_until_explicit_finalize_and_survives_reimport() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g14-unknown-capture-end-")
        .tempdir()
        .expect("exclusive fixture under root executor's J-backed TMP");
    let root = fixture.path().to_path_buf();
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("actual hot capture directory");
    let db = root.join("captures.sqlite");
    let store = Store::open(&db, &root).expect("actual writable SQLite Store");
    let known_path = hot.join(format!("bench-{KNOWN}.log"));
    fs::write(&known_path, PAYLOADS[1]).expect("literal known capture artifact");
    store
        .register_session(
            KNOWN,
            Some("authoritative known capture"),
            KNOWN_START,
            Some(&known_path),
            None,
            None,
        )
        .expect("explicit known capture start and label");
    store
        .finalize_session(KNOWN, KNOWN_END)
        .expect("explicit known capture end");
    let neighbor_path = root.join("neighbor.bench");
    fs::write(&neighbor_path, NEIGHBOR).expect("literal independent neighbor file");
    store
        .register_session(
            "neighbor",
            Some("neighbor"),
            9,
            Some(&neighbor_path),
            None,
            None,
        )
        .expect("public independent neighbor registration");
    store
        .finalize_session("neighbor", 10)
        .expect("public independent neighbor end");
    let neighbor_before = serde_json::to_value(
        store
            .get_session("neighbor")
            .expect("seed neighbor lookup")
            .expect("seed neighbor exists"),
    )
    .expect("public seeded neighbor encoding");
    let imported_path = hot.join(format!("bench-{IMPORTED}.log"));
    fs::write(&imported_path, PAYLOADS[0]).expect("literal new imported capture artifact");
    let imported_count = store
        .import_legacy_hot(50)
        .expect("actual new and known capture import");
    let imported = observe(&store, false);
    let repeated_count = store
        .import_legacy_hot(50)
        .expect("actual repeat capture import");
    let repeated = observe(&store, false);
    let kept_bytes = [
        fs::read(&imported_path).expect("kept new imported bytes"),
        fs::read(&known_path).expect("kept known capture bytes"),
    ];
    let direct_was_absent = store
        .get_session(DIRECT)
        .expect("public unregistered capture lookup")
        .is_none();
    let direct_path = hot.join(format!("bench-{DIRECT}.log"));
    fs::write(&direct_path, PAYLOADS[2])
        .expect("literal independent unregistered archive artifact");
    let archived_count = store
        .archive_hot_sessions(0)
        .expect("actual known and unregistered archival");
    let archived = observe(&store, true);
    let hot_removed = [
        file_absent(&imported_path),
        file_absent(&known_path),
        file_absent(&direct_path),
    ];
    drop(store);

    let reopened_store =
        Store::open(&db, &root).expect("actual database reopen before finalization");
    let reopened = observe(&reopened_store, true);
    // This is an explicit caller fact. A new inferred start is not used as a
    // control, including on the original whose UTC parser still falls back.
    reopened_store
        .finalize_session(IMPORTED, FINALIZED_END)
        .expect("explicit imported capture finalization");
    let finalized = observe(&reopened_store, true);
    fs::write(&imported_path, PAYLOADS[0]).expect("re-created real hot sibling after finalization");
    let reimported_count = reopened_store
        .import_legacy_hot(50)
        .expect("actual hot sibling reimport after finalization");
    let reimported = observe(&reopened_store, true);
    let reimported_hot_bytes =
        fs::read(&imported_path).expect("actual reimported hot sibling bytes");
    drop(reopened_store);
    let final_store =
        Store::open(&db, &root).expect("actual database reopen after finalized reimport");
    let final_reopened = observe(&final_store, true);
    let final_count = final_store
        .list_sessions(None, None, None, 50)
        .expect("complete public final capture list")
        .len();
    let neighbor_bytes =
        fs::read(&neighbor_path).expect("actual independent neighbor bytes after reopen");
    drop(final_store);
    fixture
        .close()
        .expect("actual capture database and artifact fixture cleanup");
    let cleanup_complete = !root
        .try_exists()
        .expect("actual capture end fixture cleanup observation");

    // All imports, archive, explicit finalization, second reimport/reopen and
    // actual fixture disposal are complete before any unknown-end assertion.
    assert!(cleanup_complete);
    assert_eq!(
        (
            imported_count,
            repeated_count,
            archived_count,
            reimported_count
        ),
        (2, 2, 3, 1)
    );
    assert_eq!(kept_bytes, [PAYLOADS[0].to_vec(), PAYLOADS[1].to_vec()]);
    assert!(direct_was_absent);
    assert!(hot_removed.into_iter().all(|removed| removed));
    assert_eq!(reimported_hot_bytes, PAYLOADS[0]);
    assert_eq!(final_count, 4);
    assert_eq!(neighbor_before["id"], "neighbor");
    assert_eq!(neighbor_before["label"], "neighbor");
    assert_eq!(neighbor_before["started_ms"], 9);
    assert_eq!(neighbor_before["ended_ms"], 10);
    assert_eq!(neighbor_bytes, NEIGHBOR);
    for stage in [
        &imported,
        &repeated,
        &archived,
        &reopened,
        &finalized,
        &reimported,
        &final_reopened,
    ] {
        assert_eq!(stage.neighbor, neighbor_before);
        assert_eq!(
            stage.neighbor_page,
            (vec!["independent end metadata neighbor".to_string()], 1)
        );
        let known = row(stage, KNOWN);
        assert_eq!(known.label.as_deref(), Some("authoritative known capture"));
        assert_eq!(
            (known.started_ms, known.ended_ms),
            (KNOWN_START, Some(KNOWN_END))
        );
        for (index, capture) in stage.captures.iter().enumerate() {
            assert_eq!(capture.payload.as_slice(), PAYLOADS[index]);
            assert_eq!(capture.page, (vec![LAST_LINES[index].to_string()], 2));
        }
    }
    for stage in [&imported, &repeated] {
        assert!(stage.captures.iter().all(|capture| !capture.compressed));
    }
    for stage in [&archived, &reopened, &finalized] {
        assert!(stage.captures.iter().all(|capture| capture.compressed));
        assert_eq!(row(stage, DIRECT).ended_ms, None);
    }
    for stage in [&finalized, &reimported, &final_reopened] {
        assert_eq!(row(stage, IMPORTED).ended_ms, Some(FINALIZED_END));
        assert_eq!(row(stage, DIRECT).ended_ms, None);
    }
    for stage in [&reimported, &final_reopened] {
        assert!(!stage.captures[0].compressed);
        assert!(stage.captures[1..].iter().all(|capture| capture.compressed));
    }
    println!("G14_END_CONTROLS=complete;cleanup_complete=true;unknown_end_observations=4;known_metadata_rows=2;direct_archive_unknown_end=true;explicit_finalize_reimport_preserved=true");

    let mut failures = Vec::new();
    for (name, stage) in [
        ("import", &imported),
        ("repeat", &repeated),
        ("archive", &archived),
        ("reopen", &reopened),
    ] {
        if let Some(end) = row(stage, IMPORTED).ended_ms {
            failures.push(format!(
                "{name}: newly imported capture fabricated ended_ms={end}"
            ));
        }
    }
    assert!(failures.is_empty(), "G14: newly imported capture end must remain unknown until explicit finalization through repeat archive and reopen: {failures:?}");
}
