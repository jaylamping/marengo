//! Existing-public G13 reference/statistics integrity through actual SQLite.
//! Unknown statistics after replacement are distinct from freshly inspected data.
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use marengo_store::{blob_dir, log_dir, LogSessionRow, Store};

const ID: &str = "20200101T000000Z";
const LABEL: &str = "authoritative capture";
const START: u64 = 1_577_836_800_123;
const END: u64 = 1_577_836_820_123;
const BYTES: [&[u8]; 3] = [
    b"bench first\nbench second\n",
    b"(0.000000) can0 123#1122\n(0.005000) can0 124#33\n",
    b"time,q\n1.0,0.25\n",
];
const REPLACEMENT: &[u8] = b"(0.010000) can0 125#4455\n";
const NAMES: [&str; 3] = [
    "bench-20200101T000000Z.log",
    "candump-20200101T000000Z.log",
    "position-trace-20200101T000000Z.csv",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Session {
    id: String,
    label: Option<String>,
    start: u64,
    end: Option<u64>,
    paths: [Option<String>; 3],
    count: Option<u64>,
    bytes: Option<u64>,
}

impl From<LogSessionRow> for Session {
    fn from(row: LogSessionRow) -> Self {
        Self {
            id: row.id,
            label: row.label,
            start: row.started_ms,
            end: row.ended_ms,
            paths: [row.bench_blob, row.candump_blob, row.trace_blob],
            count: row.candump_frame_count,
            bytes: row.candump_bytes,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CandumpPage {
    count: u64,
    frames: Vec<(String, u32, Vec<u8>)>,
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    session: Session,
    bench: Result<(Vec<String>, u32), String>,
    candump: Result<CandumpPage, String>,
    trace: Result<(Vec<String>, u32), String>,
}

fn observe(store: &Store, id: &str) -> Observation {
    Observation {
        session: store
            .get_session(id)
            .expect("actual public row lookup")
            .expect("registered row exists")
            .into(),
        bench: store
            .read_bench_page(id, 0, 10)
            .map_err(|error| error.to_string()),
        candump: store
            .read_candump_page(id, 0, 10)
            .map(|page| CandumpPage {
                count: page.summary.parsed_frames,
                frames: page
                    .frames
                    .into_iter()
                    .map(|frame| (frame.interface, frame.can_id.get(), frame.data))
                    .collect(),
            })
            .map_err(|error| error.to_string()),
        trace: store
            .read_trace_page(id, 0, 10)
            .map_err(|error| error.to_string()),
    }
}

fn register_full(store: &Store, paths: &[PathBuf; 3]) {
    store
        .register_session(
            ID,
            Some(LABEL),
            START,
            Some(&paths[0]),
            Some(&paths[1]),
            Some(&paths[2]),
        )
        .expect("actual full registration with the old reference");
}

fn files(paths: &[PathBuf; 3]) -> Vec<Vec<u8>> {
    paths
        .iter()
        .map(|path| fs::read(path).expect("actual retained artifact"))
        .collect()
}

fn decoded(files: &[Vec<u8>]) -> Vec<Vec<u8>> {
    files
        .iter()
        .map(|bytes| {
            let mut output = Vec::new();
            GzDecoder::new(bytes.as_slice())
                .read_to_end(&mut output)
                .expect("independent gzip decoder");
            output
        })
        .collect()
}

fn old_pages_match(observed: &Observation) -> bool {
    observed.bench == Ok((vec!["bench first".into(), "bench second".into()], 2))
        && observed.trace == Ok((vec!["time,q".into(), "1.0,0.25".into()], 2))
        && observed.candump
            == Ok(CandumpPage {
                count: 2,
                frames: vec![
                    ("can0".into(), 0x123, vec![0x11, 0x22]),
                    ("can0".into(), 0x124, vec![0x33]),
                ],
            })
}

fn paths(paths: &[PathBuf; 3]) -> [Option<String>; 3] {
    std::array::from_fn(|index| Some(paths[index].display().to_string()))
}

fn replacement_matches(observed: &Observation, expected: &Session) -> bool {
    observed.session == *expected
        && observed.bench == Ok((vec!["bench first".into(), "bench second".into()], 2))
        && observed.trace == Ok((vec!["time,q".into(), "1.0,0.25".into()], 2))
        && observed.candump
            == Ok(CandumpPage {
                count: 1,
                frames: vec![("can0".into(), 0x125, vec![0x44, 0x55])],
            })
}

#[test]
fn replacing_candump_invalidates_old_statistics_without_losing_siblings_or_capture_metadata() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g13-candump-replacement-")
        .tempdir()
        .expect("exclusive fixture under runner's J-backed child TMP");
    let root = fixture.path().to_path_buf();
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("actual hot directory");
    let hot_paths: [PathBuf; 3] = std::array::from_fn(|index| hot.join(NAMES[index]));
    for (path, bytes) in hot_paths.iter().zip(BYTES) {
        fs::write(path, bytes).expect("literal sibling capture bytes");
    }
    let database = root.join("sessions.sqlite");
    let store = Store::open(&database, &root).expect("actual SQLite Store");
    register_full(&store, &hot_paths);
    let archived = store
        .archive_hot_sessions(0)
        .expect("actual three-file archive");
    // Establish the explicit authoritative end after archive so the already
    // known original archive-time overwrite cannot mask this integrity probe.
    store
        .finalize_session(ID, END)
        .expect("explicit authoritative capture end");
    let archived_paths: [PathBuf; 3] = std::array::from_fn(|index| {
        blob_dir(&root)
            .join("2020-01-01")
            .join(format!("{}.gz", NAMES[index]))
    });
    let original_files = files(&archived_paths);
    let original_decoded = decoded(&original_files);
    let before = observe(&store, ID);
    store
        .register_session(
            "neighbor",
            Some("shared original artifacts"),
            7,
            Some(&archived_paths[0]),
            Some(&archived_paths[1]),
            Some(&archived_paths[2]),
        )
        .expect("actual shared-file neighboring session");
    store
        .finalize_session("neighbor", 9)
        .expect("explicit neighboring end");
    let neighbor_before = observe(&store, "neighbor");

    store
        .register_session(
            ID,
            None,
            999,
            Some(&archived_paths[0]),
            None,
            Some(&archived_paths[2]),
        )
        .expect("actual omitted-candump registration");
    let omitted = observe(&store, ID);
    register_full(&store, &archived_paths);
    let duplicate = observe(&store, ID);
    // Re-establish the same valid original-compatible fixture after the
    // omission/duplicate observations, without changing derived statistics.
    store
        .finalize_session(ID, END)
        .expect("explicit end before replacement");
    let before_replacement = observe(&store, ID);

    let new_path = root.join("replacement-candump.log");
    fs::write(&new_path, REPLACEMENT).expect("independent one-frame replacement");
    store
        .register_session(ID, None, 999, None, Some(&new_path), None)
        .expect("actual sparse supplied different-candump registration");
    let replaced = observe(&store, ID);
    let neighbor_after = observe(&store, "neighbor");
    let replacement_bytes = fs::read(&new_path).expect("actual replacement content");
    drop(store);
    let reopened_store = Store::open(&database, &root).expect("actual database reopen");
    let reopened = observe(&reopened_store, ID);
    let neighbor_reopened = observe(&reopened_store, "neighbor");
    let final_files = files(&archived_paths);
    let reopened_replacement_bytes = fs::read(&new_path).expect("replacement remains available");
    let rows = reopened_store
        .list_sessions(None, None, None, 100)
        .expect("actual public session list");
    drop(reopened_store);
    fixture.close().expect("actual fixture and SQLite disposal");
    let removed = !root.try_exists().expect("actual cleanup observation");

    // All observations and actual cleanup precede controls and classification.
    assert!(removed);
    assert_eq!(archived, 3);
    assert_eq!(original_decoded, BYTES.map(|bytes| bytes.to_vec()));
    assert_eq!(before.session.id, ID);
    assert_eq!(before.session.label.as_deref(), Some(LABEL));
    assert_eq!(
        (before.session.start, before.session.end),
        (START, Some(END))
    );
    assert_eq!(before.session.paths, paths(&archived_paths));
    assert_eq!(before.session.count, Some(2));
    assert_eq!(before.session.bytes, Some(original_files[1].len() as u64));
    assert!(old_pages_match(&before));
    assert_eq!(omitted.session.count, before.session.count);
    assert_eq!(omitted.session.bytes, before.session.bytes);
    assert_eq!(duplicate.session.count, before.session.count);
    assert_eq!(duplicate.session.bytes, before.session.bytes);
    assert!(old_pages_match(&duplicate));
    assert_eq!(before_replacement, before);
    assert_eq!(
        replaced.candump,
        Ok(CandumpPage {
            count: 1,
            frames: vec![("can0".into(), 0x125, vec![0x44, 0x55])],
        })
    );
    assert_eq!(reopened.candump, replaced.candump);
    assert_eq!(replacement_bytes, REPLACEMENT);
    assert_eq!(reopened_replacement_bytes, REPLACEMENT);
    assert_eq!(original_files, final_files);
    assert!(old_pages_match(&neighbor_before));
    assert_eq!(neighbor_before.session.id, "neighbor");
    assert_eq!(
        neighbor_before.session.label.as_deref(),
        Some("shared original artifacts")
    );
    assert_eq!(
        (neighbor_before.session.start, neighbor_before.session.end),
        (7, Some(9))
    );
    assert_eq!(neighbor_before, neighbor_after);
    assert_eq!(neighbor_before, neighbor_reopened);
    assert_eq!(rows.len(), 2);
    println!("G13_CANDUMP_CONTROLS=complete;cleanup_complete=true;archived_frames=2;replacement_frames=1;unique_sessions=2");

    let mut expected = before.session.clone();
    expected.paths[1] = Some(new_path.display().to_string());
    expected.count = None;
    expected.bytes = None;
    let mut failures = Vec::new();
    if omitted != before {
        failures.push(format!("omitted registration: {:?}", omitted.session));
    }
    if duplicate != before {
        failures.push(format!("duplicate old reference: {:?}", duplicate.session));
    }
    if !replacement_matches(&replaced, &expected) {
        failures.push(format!(
            "replacement statistics/siblings/metadata: {:?}",
            replaced.session
        ));
    }
    if !replacement_matches(&reopened, &expected) {
        failures.push(format!(
            "reopened statistics/siblings/metadata: {:?}",
            reopened.session
        ));
    }
    assert!(failures.is_empty(), "G13: a different candump reference must invalidate old statistics and preserve sibling artifacts and authoritative metadata: {failures:?}");
}
