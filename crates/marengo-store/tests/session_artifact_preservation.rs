//! Existing-public G13 contract through a real database and artifact files.
//! Blob lookup plus file reads cover download availability, not gateway HTTP.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use marengo_store::{LogSessionRow, Store};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Artifact {
    Bench,
    Candump,
    Trace,
}

impl Artifact {
    fn index(self) -> usize {
        match self {
            Self::Bench => 0,
            Self::Candump => 1,
            Self::Trace => 2,
        }
    }

    fn bytes(self, replaced: bool) -> &'static [u8] {
        match (self, replaced) {
            (Self::Bench, false) => b"bench first\nbench second\n",
            (Self::Bench, true) => b"replacement first\nreplacement second\n",
            (Self::Candump, false) => b"(0.000000) can0 123#1122\n(0.005000) can0 124#33\n",
            (Self::Candump, true) => b"(0.000000) can0 123#1122\n(0.005000) can0 125#44\n",
            (Self::Trace, false) => b"time,q\n0.5,0.25\n",
            (Self::Trace, true) => b"time,q\n0.7,0.75\n",
        }
    }
}

const ARTIFACTS: [Artifact; 3] = [Artifact::Bench, Artifact::Candump, Artifact::Trace];
const ORDERS: [[Artifact; 3]; 6] = [
    [Artifact::Bench, Artifact::Candump, Artifact::Trace],
    [Artifact::Bench, Artifact::Trace, Artifact::Candump],
    [Artifact::Candump, Artifact::Bench, Artifact::Trace],
    [Artifact::Candump, Artifact::Trace, Artifact::Bench],
    [Artifact::Trace, Artifact::Bench, Artifact::Candump],
    [Artifact::Trace, Artifact::Candump, Artifact::Bench],
];

#[derive(Debug, PartialEq, Eq)]
struct Session {
    id: String,
    label: Option<String>,
    start: u64,
    end: Option<u64>,
    paths: [Option<String>; 3],
}

impl From<LogSessionRow> for Session {
    fn from(row: LogSessionRow) -> Self {
        Self {
            id: row.id,
            label: row.label,
            start: row.started_ms,
            end: row.ended_ms,
            paths: [row.bench_blob, row.candump_blob, row.trace_blob],
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CandumpPage {
    total: u64,
    frames: Vec<(String, u32, Vec<u8>)>,
}

#[derive(Debug, PartialEq, Eq)]
enum Download {
    Missing,
    Bytes(Vec<u8>),
    Error(String),
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    session: Session,
    bench: Result<(Vec<String>, u32), String>,
    candump: Result<CandumpPage, String>,
    trace: Result<(Vec<String>, u32), String>,
    downloads: [Download; 3],
}

fn observe(store: &Store, id: &str) -> Observation {
    let session = Session::from(
        store
            .get_session(id)
            .expect("actual session lookup")
            .expect("registered session exists"),
    );
    let downloads = std::array::from_fn(|index| match &session.paths[index] {
        Some(path) => match fs::read(path) {
            Ok(bytes) => Download::Bytes(bytes),
            Err(error) => Download::Error(error.to_string()),
        },
        None => Download::Missing,
    });
    Observation {
        session,
        bench: store
            .read_bench_page(id, 1, 1)
            .map_err(|error| error.to_string()),
        candump: store
            .read_candump_page(id, 1, 1)
            .map(|page| CandumpPage {
                total: page.summary.parsed_frames,
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
        downloads,
    }
}

fn artifact_path(dir: &Path, artifact: Artifact, replaced: bool) -> PathBuf {
    dir.join(format!("{artifact:?}-{replaced}.log"))
}

fn write_artifact(dir: &Path, artifact: Artifact, replaced: bool) -> PathBuf {
    fs::create_dir_all(dir).expect("exclusive fixture subdirectory");
    let path = artifact_path(dir, artifact, replaced);
    fs::write(&path, artifact.bytes(replaced)).expect("literal artifact bytes");
    path
}

fn register_one(store: &Store, id: &str, artifact: Artifact, path: &Path) {
    store
        .register_session(
            id,
            None,
            42, // A later import must not replace the authoritative capture start.
            (artifact == Artifact::Bench).then_some(path),
            (artifact == Artifact::Candump).then_some(path),
            (artifact == Artifact::Trace).then_some(path),
        )
        .expect("actual sparse registration");
}

fn path_strings(paths: &[PathBuf; 3]) -> [Option<String>; 3] {
    std::array::from_fn(|index| Some(paths[index].display().to_string()))
}

struct Case {
    id: String,
    label: String,
    order: [Artifact; 3],
    seed: Session,
    initial_paths: [Option<String>; 3],
    final_paths: [Option<String>; 3],
    missing_file_absent: bool,
    missing: Observation,
    complete: Observation,
    duplicate: Observation,
    replacement: Observation,
    reopened: Option<Observation>,
}

fn collect_failures(
    case: &Case,
    stage: &str,
    observed: &Observation,
    expected_paths: &[Option<String>; 3],
    replaced: Option<Artifact>,
    failures: &mut Vec<String>,
) {
    if observed.session.id != case.id
        || observed.session.label.as_deref() != Some(case.label.as_str())
        || observed.session.start != 1_570_000_000_123
        || observed.session.end != Some(1_570_000_010_123)
    {
        failures.push(format!("{:?}/{stage}: authoritative metadata", case.order));
    }
    if &observed.session.paths != expected_paths {
        failures.push(format!("{:?}/{stage}: sibling paths", case.order));
    }
    for artifact in ARTIFACTS {
        let index = artifact.index();
        let present = expected_paths[index].is_some();
        let changed = replaced == Some(artifact);
        let page_matches = match artifact {
            Artifact::Bench if present => observed.bench.as_ref().is_ok_and(|(lines, total)| {
                *total == 2
                    && lines
                        == &[if changed {
                            "replacement second".to_string()
                        } else {
                            "bench second".to_string()
                        }]
            }),
            Artifact::Trace if present => observed.trace.as_ref().is_ok_and(|(lines, total)| {
                *total == 2
                    && lines
                        == &[if changed {
                            "0.7,0.75".to_string()
                        } else {
                            "0.5,0.25".to_string()
                        }]
            }),
            Artifact::Candump if present => observed.candump.as_ref().is_ok_and(|page| {
                page.total == 2
                    && page.frames
                        == vec![(
                            "can0".to_string(),
                            if changed { 0x125 } else { 0x124 },
                            if changed { vec![0x44] } else { vec![0x33] },
                        )]
            }),
            Artifact::Bench => observed.bench.is_err(),
            Artifact::Candump => observed.candump.is_err(),
            Artifact::Trace => observed.trace.is_err(),
        };
        let download_matches = match &observed.downloads[index] {
            Download::Missing => !present,
            Download::Bytes(bytes) => present && bytes == artifact.bytes(changed),
            Download::Error(_) => false,
        };
        if !page_matches || !download_matches {
            failures.push(format!(
                "{:?}/{stage}: {artifact:?} page/download availability",
                case.order
            ));
        }
    }
}

#[test]
fn sparse_registration_preserves_session_metadata_and_all_sibling_artifacts() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g13-")
        .tempdir()
        .expect("unique fixture under runner's J-backed child TMP");
    let root = fixture.path().to_path_buf();
    let database = root.join("sessions.sqlite");
    let store = Store::open(&database, &root).expect("actual SQLite Store");
    let neighbor_paths = std::array::from_fn(|index| {
        write_artifact(&root.join("neighbor"), ARTIFACTS[index], false)
    });
    store
        .register_session(
            "neighbor",
            Some("untouched neighbor"),
            101,
            Some(&neighbor_paths[0]),
            Some(&neighbor_paths[1]),
            Some(&neighbor_paths[2]),
        )
        .expect("complete neighboring registration");
    store
        .finalize_session("neighbor", 303)
        .expect("neighbor end");
    let neighbor_before = observe(&store, "neighbor");
    let mut cases = Vec::new();
    let mut files = Vec::new();
    for (number, order) in ORDERS.into_iter().enumerate() {
        let id = format!("registration-order-{number}");
        let label = format!("manual label {number}");
        let dir = root.join(&id);
        let paths: [PathBuf; 3] =
            std::array::from_fn(|index| artifact_path(&dir, ARTIFACTS[index], false));
        for artifact in order.iter().take(2) {
            write_artifact(&dir, *artifact, false);
        }
        store
            .register_session(&id, Some(&label), 1_570_000_000_123, None, None, None)
            .expect("authoritative metadata seed");
        store
            .finalize_session(&id, 1_570_000_010_123)
            .expect("authoritative finalized end");
        let seed = Session::from(store.get_session(&id).expect("seed lookup").expect("seed"));
        register_one(&store, &id, order[0], &paths[order[0].index()]);
        register_one(&store, &id, order[1], &paths[order[1].index()]);
        let missing = observe(&store, &id);
        let missing_file_absent = !paths[order[2].index()]
            .try_exists()
            .expect("third sibling has not arrived on disk");
        write_artifact(&dir, order[2], false);
        for artifact in ARTIFACTS {
            files.push((paths[artifact.index()].clone(), artifact.bytes(false)));
        }
        register_one(&store, &id, order[2], &paths[order[2].index()]);
        let complete = observe(&store, &id);
        register_one(&store, &id, order[2], &paths[order[2].index()]);
        let duplicate = observe(&store, &id);
        let replacement_path = write_artifact(&dir, order[1], true);
        files.push((replacement_path.clone(), order[1].bytes(true)));
        register_one(&store, &id, order[1], &replacement_path);
        let replacement = observe(&store, &id);
        let initial_paths = path_strings(&paths);
        let mut final_paths = initial_paths.clone();
        final_paths[order[1].index()] = Some(replacement_path.display().to_string());
        cases.push(Case {
            id,
            label,
            order,
            seed,
            initial_paths,
            final_paths,
            missing_file_absent,
            missing,
            complete,
            duplicate,
            replacement,
            reopened: None,
        });
    }
    let neighbor_after = observe(&store, "neighbor");
    drop(store);
    let reopened_store = Store::open(&database, &root).expect("real database reopen");
    for case in &mut cases {
        case.reopened = Some(observe(&reopened_store, &case.id));
    }
    let neighbor_reopened = observe(&reopened_store, "neighbor");
    let sessions = reopened_store
        .list_sessions(None, None, None, 100)
        .expect("public session list");
    let file_controls: Vec<_> = files
        .into_iter()
        .map(|(path, expected)| {
            (
                expected,
                fs::read(path).expect("independent fixture content control"),
            )
        })
        .collect();
    drop(reopened_store);
    fixture
        .close()
        .expect("actual database and fixture disposal");
    let removed = !root.try_exists().expect("cleanup observation");

    // All six orders, missing-sibling arrival, duplicates, replacements and
    // reopened pages/download lookups have run before any contract assertion.
    assert!(removed, "exclusive fixture must be removed");
    for (expected, actual) in file_controls {
        assert_eq!(
            actual, expected,
            "registration must not alter artifact bytes"
        );
    }
    assert_eq!(
        neighbor_before.session.label.as_deref(),
        Some("untouched neighbor")
    );
    assert_eq!(neighbor_before.session.start, 101);
    assert_eq!(neighbor_before.session.end, Some(303));
    assert_eq!(neighbor_before.session.paths, path_strings(&neighbor_paths));
    for artifact in ARTIFACTS {
        assert_eq!(
            neighbor_before.downloads[artifact.index()],
            Download::Bytes(artifact.bytes(false).to_vec())
        );
    }
    assert_eq!(neighbor_before.bench, Ok((vec!["bench second".into()], 2)));
    assert_eq!(neighbor_before.trace, Ok((vec!["0.5,0.25".into()], 2)));
    assert_eq!(
        neighbor_before.candump,
        Ok(CandumpPage {
            total: 2,
            frames: vec![("can0".into(), 0x124, vec![0x33])]
        })
    );
    assert_eq!(neighbor_before, neighbor_after);
    assert_eq!(neighbor_before, neighbor_reopened);
    assert_eq!(
        sessions.len(),
        7,
        "one row per session after duplicate registration"
    );
    for case in &cases {
        assert_eq!(sessions.iter().filter(|row| row.id == case.id).count(), 1);
        assert_eq!(case.seed.id, case.id);
        assert_eq!(case.seed.label.as_deref(), Some(case.label.as_str()));
        assert_eq!(case.seed.start, 1_570_000_000_123);
        assert_eq!(case.seed.end, Some(1_570_000_010_123));
        assert_eq!(case.seed.paths, [None, None, None]);
        assert!(case.missing_file_absent, "late sibling was absent on disk");
    }
    println!("G13_ARTIFACT_CONTROLS=complete;cleanup_complete=true;orders=6;unique_sessions=7");
    let mut failures = Vec::new();
    for case in &cases {
        let mut missing_paths = case.initial_paths.clone();
        missing_paths[case.order[2].index()] = None;
        collect_failures(
            case,
            "missing-before-arrival",
            &case.missing,
            &missing_paths,
            None,
            &mut failures,
        );
        collect_failures(
            case,
            "all-arrived",
            &case.complete,
            &case.initial_paths,
            None,
            &mut failures,
        );
        collect_failures(
            case,
            "duplicate",
            &case.duplicate,
            &case.initial_paths,
            None,
            &mut failures,
        );
        collect_failures(
            case,
            "same-kind-replacement",
            &case.replacement,
            &case.final_paths,
            Some(case.order[1]),
            &mut failures,
        );
        collect_failures(
            case,
            "reopened",
            case.reopened.as_ref().expect("actual reopened observation"),
            &case.final_paths,
            Some(case.order[1]),
            &mut failures,
        );
    }
    assert!(failures.is_empty(), "G13: sparse artifact registration must preserve omitted siblings and authoritative metadata: {failures:?}");
}
