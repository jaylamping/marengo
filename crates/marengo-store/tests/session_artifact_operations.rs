//! Candidate API conformance through real SQLite, hot files and gzip archives.
//! Explicit clear removes references, not files; this is not a crash-durability test.
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use marengo_store::{blob_dir, log_dir, LogSessionRow, SessionArtifact, Store, StoreError};

const SESSION: &str = "20200101T000000Z";
const LABEL: &str = "authoritative capture";
const START: u64 = 1_577_836_800_123;
const END: u64 = 1_577_836_820_123;
const BYTES: [&[u8]; 3] = [
    b"bench first\nbench second\n",
    b"(0.000000) can0 123#1122\n(0.005000) can0 124#33\n",
    b"time,q\n1.0,0.25\n",
];
const ARTIFACTS: [SessionArtifact; 3] = [
    SessionArtifact::Bench,
    SessionArtifact::Candump,
    SessionArtifact::Trace,
];
const NAMES: [&str; 3] = [
    "bench-20200101T000000Z.log",
    "candump-20200101T000000Z.log",
    "position-trace-20200101T000000Z.csv",
];

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    database: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let directory = tempfile::Builder::new()
            .prefix(label)
            .tempdir()
            .expect("exclusive fixture under runner's J-backed child TMP");
        let root = directory.path().to_path_buf();
        let database = root.join("sessions.sqlite");
        Self {
            directory,
            root,
            database,
        }
    }

    fn open(&self) -> Store {
        Store::open(&self.database, &self.root).expect("real SQLite Store")
    }

    fn close(self) -> bool {
        self.directory.close().expect("actual fixture disposal");
        !self.root.try_exists().expect("actual cleanup observation")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    id: String,
    label: Option<String>,
    start: u64,
    end: Option<u64>,
    paths: [Option<String>; 3],
    candump_count: Option<u64>,
    candump_bytes: Option<u64>,
}

impl From<LogSessionRow> for Row {
    fn from(row: LogSessionRow) -> Self {
        Self {
            id: row.id,
            label: row.label,
            start: row.started_ms,
            end: row.ended_ms,
            paths: [row.bench_blob, row.candump_blob, row.trace_blob],
            candump_count: row.candump_frame_count,
            candump_bytes: row.candump_bytes,
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
    row: Option<Row>,
    bench: Result<(Vec<String>, u32), String>,
    candump: Result<CandumpPage, String>,
    trace: Result<(Vec<String>, u32), String>,
}

fn observe(store: &Store, id: &str) -> Observation {
    Observation {
        row: store
            .get_session(id)
            .expect("public row lookup")
            .map(Row::from),
        bench: store
            .read_bench_page(id, 1, 1)
            .map_err(|error| error.to_string()),
        candump: store
            .read_candump_page(id, 1, 1)
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
            .read_trace_page(id, 1, 1)
            .map_err(|error| error.to_string()),
    }
}

fn pages_match(observed: &Observation, present: [bool; 3]) -> bool {
    (if present[0] {
        observed.bench == Ok((vec!["bench second".into()], 2))
    } else {
        observed.bench.is_err()
    }) && (if present[1] {
        observed.candump
            == Ok(CandumpPage {
                count: 2,
                frames: vec![("can0".into(), 0x124, vec![0x33])],
            })
    } else {
        observed.candump.is_err()
    }) && (if present[2] {
        observed.trace == Ok((vec!["1.0,0.25".into()], 2))
    } else {
        observed.trace.is_err()
    })
}

fn write_hot(root: &Path) -> [PathBuf; 3] {
    let hot = log_dir(root);
    fs::create_dir_all(&hot).expect("actual hot directory");
    let paths = std::array::from_fn(|index| hot.join(NAMES[index]));
    for (path, bytes) in paths.iter().zip(BYTES) {
        fs::write(path, bytes).expect("literal hot artifact");
    }
    paths
}

fn register_full(store: &Store, paths: &[PathBuf; 3]) {
    store
        .register_session(
            SESSION,
            Some(LABEL),
            START,
            Some(&paths[0]),
            Some(&paths[1]),
            Some(&paths[2]),
        )
        .expect("actual complete session registration");
    store
        .finalize_session(SESSION, END)
        .expect("explicit capture end");
}

fn archive_fixture(fixture: &Fixture, store: &Store) -> ([PathBuf; 3], u32) {
    let hot = write_hot(&fixture.root);
    register_full(store, &hot);
    let count = store
        .archive_hot_sessions(0)
        .expect("actual archive populates candump stats");
    let paths = std::array::from_fn(|index| {
        blob_dir(&fixture.root)
            .join("2020-01-01")
            .join(format!("{}.gz", NAMES[index]))
    });
    (paths, count)
}

fn files(paths: &[PathBuf; 3]) -> Vec<Vec<u8>> {
    paths
        .iter()
        .map(|path| fs::read(path).expect("actual artifact bytes"))
        .collect()
}

fn decoded(files: &[Vec<u8>]) -> Vec<Vec<u8>> {
    files
        .iter()
        .map(|bytes| {
            let mut output = Vec::new();
            GzDecoder::new(bytes.as_slice())
                .read_to_end(&mut output)
                .expect("independent standard gzip decoder");
            output
        })
        .collect()
}

fn metadata_matches(row: &Row) -> bool {
    row.id == SESSION
        && row.label.as_deref() == Some(LABEL)
        && row.start == START
        && row.end == Some(END)
}

fn readonly(error: &StoreError) -> bool {
    matches!(error, StoreError::Sqlite(rusqlite::Error::SqliteFailure(error, _))
        if error.code == rusqlite::ErrorCode::ReadOnly)
}

fn query_only(store: &Store, enabled: bool) -> i64 {
    let connection = store.connection();
    connection
        .pragma_update(None, "query_only", enabled)
        .expect("actual SQLite query-only guard");
    connection
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .expect("actual SQLite guard readback")
}

#[test]
fn explicit_clear_preserves_sibling_references_metadata_and_shared_files_across_reopen() {
    let fixture = Fixture::new("marengo-g13-clear-");
    let mut store = fixture.open();
    let (paths, archived) = archive_fixture(&fixture, &store);
    let before = observe(&store, SESSION);
    let original_files = files(&paths);
    let original_decoded = decoded(&original_files);
    store
        .register_session(
            "neighbor",
            Some("shared artifacts"),
            7,
            Some(&paths[0]),
            Some(&paths[1]),
            Some(&paths[2]),
        )
        .expect("actual shared-file neighbor");
    store
        .finalize_session("neighbor", 9)
        .expect("neighbor capture end");
    let neighbor_before = observe(&store, "neighbor");
    let mut cases = Vec::new();
    for artifact in ARTIFACTS {
        let cleared = store
            .clear_session_artifact(SESSION, artifact)
            .map_err(|error| error.to_string());
        let repeated = store
            .clear_session_artifact(SESSION, artifact)
            .map_err(|error| error.to_string());
        let absent = store
            .clear_session_artifact("absent", artifact)
            .map_err(|error| error.to_string());
        let after = observe(&store, SESSION);
        drop(store);
        store = fixture.open();
        let reopened = observe(&store, SESSION);
        let neighbor = observe(&store, "neighbor");
        cases.push((cleared, repeated, absent, after, reopened, neighbor));
    }
    let absent = store.get_session("absent").expect("absent row lookup");
    let count = store
        .list_sessions(None, None, None, 100)
        .expect("actual row list")
        .len();
    let final_files = files(&paths);
    drop(store);
    let removed = fixture.close();

    assert!(removed);
    assert_eq!(archived, 3, "real three-artifact archive control");
    let mut expected = before.row.clone().expect("healthy archived session");
    assert!(metadata_matches(&expected));
    assert_eq!(expected.candump_count, Some(2));
    assert_eq!(expected.candump_bytes, Some(original_files[1].len() as u64));
    assert_eq!(original_decoded, BYTES.map(|bytes| bytes.to_vec()));
    assert!(pages_match(&before, [true; 3]));
    assert!(pages_match(&neighbor_before, [true; 3]));
    assert_eq!(count, 2);
    assert!(absent.is_none());
    assert_eq!(
        original_files, final_files,
        "clear must not delete shared artifact files"
    );
    for (index, (cleared, repeated, absent, after, reopened, neighbor)) in
        cases.into_iter().enumerate()
    {
        expected.paths[index] = None;
        if index == 1 {
            expected.candump_count = None;
            expected.candump_bytes = None;
        }
        assert_eq!(cleared, Ok(true));
        assert_eq!(repeated, Ok(false));
        assert_eq!(absent, Ok(false));
        assert_eq!(after.row.as_ref(), Some(&expected));
        assert_eq!(reopened.row.as_ref(), Some(&expected));
        assert!(pages_match(
            &after,
            std::array::from_fn(|kind| kind > index)
        ));
        assert!(pages_match(
            &reopened,
            std::array::from_fn(|kind| kind > index)
        ));
        assert_eq!(neighbor, neighbor_before);
    }
}

#[test]
fn readonly_clear_preserves_populated_rows_and_files_and_writable_retry_succeeds() {
    let fixture = Fixture::new("marengo-g13-clear-readonly-");
    let store = fixture.open();
    let (paths, archived) = archive_fixture(&fixture, &store);
    let before = observe(&store, SESSION);
    let original_files = files(&paths);
    let guarded = query_only(&store, true);
    let mut failures = Vec::new();
    for artifact in ARTIFACTS {
        let result = store.clear_session_artifact(SESSION, artifact);
        failures.push((
            result.as_ref().err().is_some_and(readonly),
            observe(&store, SESSION),
        ));
    }
    let failed_files = files(&paths);
    let writable = query_only(&store, false);
    let retries: Vec<_> = ARTIFACTS
        .into_iter()
        .map(|artifact| {
            store
                .clear_session_artifact(SESSION, artifact)
                .map_err(|error| error.to_string())
        })
        .collect();
    drop(store);
    let reopened_store = fixture.open();
    let reopened = observe(&reopened_store, SESSION);
    let final_files = files(&paths);
    drop(reopened_store);
    let removed = fixture.close();

    assert!(removed);
    assert_eq!(archived, 3, "real three-artifact archive control");
    assert_eq!(guarded, 1);
    assert_eq!(writable, 0);
    assert!(pages_match(&before, [true; 3]));
    let mut expected = before
        .row
        .clone()
        .expect("healthy complete archived session");
    assert!(metadata_matches(&expected));
    assert_eq!(expected.candump_count, Some(2));
    for (typed_refusal, after) in failures {
        assert!(typed_refusal, "actual SQLite ReadOnly must be propagated");
        assert_eq!(
            after, before,
            "failed clear cannot publish altered reference state"
        );
    }
    assert_eq!(retries, vec![Ok(true), Ok(true), Ok(true)]);
    expected.paths = [None, None, None];
    expected.candump_count = None;
    expected.candump_bytes = None;
    assert_eq!(reopened.row, Some(expected));
    assert!(pages_match(&reopened, [false; 3]));
    assert_eq!(original_files, failed_files);
    assert_eq!(original_files, final_files);
}

#[test]
fn import_report_counts_sessions_separately_and_refuses_readonly_storage_without_losing_files() {
    let fixture = Fixture::new("marengo-g13-report-");
    let store = fixture.open();
    let paths = write_hot(&fixture.root);
    store
        .register_session(SESSION, Some(LABEL), START, None, None, None)
        .expect("authoritative seed before import");
    store
        .finalize_session(SESSION, END)
        .expect("explicit end before import");
    let before = observe(&store, SESSION);
    let original_files = files(&paths);
    let guarded = query_only(&store, true);
    let refused = store.import_legacy_hot_report(0);
    let typed_refusal = refused.as_ref().err().is_some_and(readonly);
    let after_failure = observe(&store, SESSION);
    let failed_files = files(&paths);
    let writable = query_only(&store, false);

    // A real second session with only a bench artifact distinguishes both counters.
    let peer_path = log_dir(&fixture.root).join("bench-20200102T000000Z.log");
    fs::write(&peer_path, BYTES[0]).expect("literal second-session artifact");
    store
        .register_session(
            "20200102T000000Z",
            Some("second capture"),
            11,
            None,
            None,
            None,
        )
        .expect("authoritative peer seed");
    store
        .finalize_session("20200102T000000Z", 22)
        .expect("peer finalized end");
    let first = store
        .import_legacy_hot_report(50)
        .map(|summary| (summary.sessions, summary.artifacts))
        .map_err(|error| error.to_string());
    let repeated = store
        .import_legacy_hot_report(50)
        .map(|summary| (summary.sessions, summary.artifacts))
        .map_err(|error| error.to_string());
    let legacy = store
        .import_legacy_hot(50)
        .map_err(|error| error.to_string());
    let after = observe(&store, SESSION);
    let peer = observe(&store, "20200102T000000Z");
    let retained_files = files(&paths);
    let peer_bytes = fs::read(&peer_path).expect("actual peer hot bytes retained");
    drop(store);
    let reopened_store = fixture.open();
    let reopened = observe(&reopened_store, SESSION);
    let reopened_peer = observe(&reopened_store, "20200102T000000Z");
    let rows = reopened_store
        .list_sessions(None, None, None, 100)
        .expect("actual reopened list");
    drop(reopened_store);
    let removed = fixture.close();

    assert!(removed);
    assert_eq!(guarded, 1);
    assert_eq!(writable, 0);
    assert!(
        typed_refusal,
        "import must propagate the actual SQLite ReadOnly failure"
    );
    assert_eq!(after_failure, before);
    assert_eq!(original_files, failed_files);
    assert_eq!(original_files, retained_files);
    assert_eq!(peer_bytes, BYTES[0]);
    assert_eq!(first, Ok((2, 4)));
    assert_eq!(repeated, Ok((2, 4)));
    assert_eq!(legacy, Ok(2));
    let row = after.row.as_ref().expect("healthy imported session");
    assert!(metadata_matches(row));
    assert_eq!(
        row.paths,
        paths.map(|path| Some(path.display().to_string()))
    );
    assert!(pages_match(&after, [true; 3]));
    let peer_row = peer.row.as_ref().expect("healthy imported peer");
    assert_eq!(peer_row.id, "20200102T000000Z");
    assert_eq!(peer_row.label.as_deref(), Some("second capture"));
    assert_eq!(peer_row.start, 11);
    assert_eq!(peer_row.end, Some(22));
    assert_eq!(
        peer_row.paths,
        [Some(peer_path.display().to_string()), None, None]
    );
    assert!(pages_match(&peer, [true, false, false]));
    assert_eq!(after, reopened);
    assert_eq!(peer, reopened_peer);
    assert_eq!(rows.len(), 2);
}
