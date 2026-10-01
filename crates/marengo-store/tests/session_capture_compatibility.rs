//! Existing-public G14 supported capture conventions and authoritative legacy metadata.
//! Counts, optional new rows and expected paths are collected after all controls/cleanup.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use marengo_store::{blob_dir, log_dir, LegacyImportSummary, LogSessionRow, Store};
use serde_json::Value;

const IDS: [&str; 4] = [
    "20200101T000000Z",
    "profile-20200102T030405Z",
    "旧capture",
    "20230229T000000Z",
];
const STARTS: [u64; 4] = [1_577_836_800_000, 1_577_934_245_000, 123, 1000];
const BUCKETS: [&str; 4] = ["2020-01-01", "2020-01-02", "unknown", "unknown"];
const LEGACY_ENDS: [u64; 2] = [456, 2000];
const LEGACY_LABELS: [&str; 2] = [
    "authoritative unicode capture",
    "authoritative invalid calendar capture",
];
const PREFIXES: [&str; 3] = ["bench", "candump", "position-trace"];
const EXTENSIONS: [&str; 3] = ["log", "log", "csv"];
const PAYLOADS: [&[u8]; 3] = [
    b"first captured event\nlast captured event\n",
    b"(0.000000) can0 123#1122\n(0.005000) can0 124#33\n",
    b"time,q\n1.0,0.25\n",
];
const NEIGHBOR: &[u8] = b"independent compatibility neighbor\n";
const IGNORED: &[u8] = b"ignored producer sidecar or alias remains unchanged\n";
const IGNORED_NAMES: [&str; 10] = [
    "bench-latest.log",
    "candump-latest.log",
    "position-trace-latest.csv",
    "bench-latest.json",
    "bench-20200101T000000Z.json",
    "bench-20200103T000000Z.txt",
    "bench-20200104T000000Z.log.backup",
    "bench-20200105T000000Z.csv",
    "candump-20200106T000000Z.csv",
    "position-trace-20200107T000000Z.log",
];

#[derive(Debug, PartialEq, Eq)]
struct Download {
    compressed: bool,
    payload: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct CandumpPage {
    total: u64,
    frames: Vec<(String, u32, Vec<u8>)>,
}

#[derive(Debug)]
struct Capture {
    row: LogSessionRow,
    downloads: [Option<Result<Download, String>>; 3],
    bench: Option<Result<(Vec<String>, u32), String>>,
    candump: Option<Result<CandumpPage, String>>,
    trace: Option<Result<(Vec<String>, u32), String>>,
}

#[derive(Debug)]
struct Stage {
    captures: Vec<Option<Capture>>,
    all_ids: Vec<String>,
    neighbor: Value,
    neighbor_page: (Vec<String>, u32),
}

fn references(row: &LogSessionRow) -> [Option<&str>; 3] {
    [
        row.bench_blob.as_deref(),
        row.candump_blob.as_deref(),
        row.trace_blob.as_deref(),
    ]
}

fn download(path: &str) -> Result<Download, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let compressed = bytes.starts_with(&[0x1f, 0x8b]);
    let payload = if compressed {
        let mut decoded = Vec::new();
        GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut decoded)
            .map_err(|error| error.to_string())?;
        decoded
    } else {
        bytes
    };
    Ok(Download {
        compressed,
        payload,
    })
}

fn capture(store: &Store, id: &str) -> Option<Capture> {
    let row = store
        .get_session(id)
        .expect("actual public capture lookup")?;
    let downloads = references(&row).map(|path| path.map(download));
    let bench = row.bench_blob.as_ref().map(|_| {
        store
            .read_bench_page(id, 1, 1)
            .map_err(|error| error.to_string())
    });
    let candump = row.candump_blob.as_ref().map(|_| {
        store
            .read_candump_page(id, 1, 1)
            .map(|page| CandumpPage {
                total: page.summary.parsed_frames,
                frames: page
                    .frames
                    .into_iter()
                    .map(|frame| (frame.interface, frame.can_id.get(), frame.data))
                    .collect(),
            })
            .map_err(|error| error.to_string())
    });
    let trace = row.trace_blob.as_ref().map(|_| {
        store
            .read_trace_page(id, 1, 1)
            .map_err(|error| error.to_string())
    });
    Some(Capture {
        row,
        downloads,
        bench,
        candump,
        trace,
    })
}

fn observe(store: &Store) -> Stage {
    let mut all_ids: Vec<_> = store
        .list_sessions(None, None, None, 50)
        .expect("complete public compatibility session list")
        .into_iter()
        .map(|row| row.id)
        .collect();
    all_ids.sort();
    let neighbor = store
        .get_session("neighbor")
        .expect("public independent neighbor lookup")
        .expect("independent neighbor stays present");
    Stage {
        captures: IDS.iter().map(|id| capture(store, id)).collect(),
        all_ids,
        neighbor: serde_json::to_value(neighbor).expect("public independent neighbor encoding"),
        neighbor_page: store
            .read_bench_page("neighbor", 0, 10)
            .expect("actual independent neighbor page"),
    }
}

fn optional_bytes(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("fixture byte observation failed for {path:?}: {error}"),
    }
}

fn expected_page(kind: usize) -> (Vec<String>, u32) {
    match kind {
        0 => (vec!["last captured event".to_string()], 2),
        2 => (vec!["1.0,0.25".to_string()], 2),
        _ => panic!("only literal text pages have this fixture oracle"),
    }
}

fn expected_candump() -> CandumpPage {
    CandumpPage {
        total: 2,
        frames: vec![("can0".to_string(), 0x124, vec![0x33])],
    }
}

fn collect_compatibility_failures(
    name: &str,
    stage: &Stage,
    root: &Path,
    archived: bool,
    failures: &mut Vec<String>,
) {
    let mut expected_ids: Vec<_> = IDS.iter().map(|id| id.to_string()).collect();
    expected_ids.push("neighbor".to_string());
    expected_ids.sort();
    if stage.all_ids != expected_ids {
        failures.push(format!(
            "{name}: unsupported input IDs registered: {:?}",
            stage.all_ids
        ));
    }
    for index in 0..IDS.len() {
        let id = IDS[index];
        let Some(capture) = &stage.captures[index] else {
            failures.push(format!("{name}: supported capture {id} is absent"));
            continue;
        };
        if capture.row.started_ms != STARTS[index] {
            failures.push(format!(
                "{name}/{id}: UTC or authoritative start is {} expected {}",
                capture.row.started_ms, STARTS[index]
            ));
        }
        for kind in 0..3 {
            let expected_path = if index == 1 && kind == 0 {
                None
            } else {
                let filename = format!("{}-{id}.{}", PREFIXES[kind], EXTENSIONS[kind]);
                Some(if archived {
                    blob_dir(root)
                        .join(BUCKETS[index])
                        .join(format!("{filename}.gz"))
                        .display()
                        .to_string()
                } else {
                    log_dir(root).join(filename).display().to_string()
                })
            };
            if references(&capture.row)[kind] != expected_path.as_deref() {
                failures.push(format!(
                    "{name}/{id}: artifact kind {kind} path/bucket differs: {:?}",
                    references(&capture.row)[kind]
                ));
            }
            let expected_download = expected_path.map(|_| {
                Ok(Download {
                    compressed: archived,
                    payload: PAYLOADS[kind].to_vec(),
                })
            });
            if capture.downloads[kind] != expected_download {
                failures.push(format!(
                    "{name}/{id}: artifact kind {kind} actual payload differs: {:?}",
                    capture.downloads[kind]
                ));
            }
        }
        let expected_bench = (index != 1).then(|| Ok(expected_page(0)));
        if capture.bench != expected_bench
            || capture.candump != Some(Ok(expected_candump()))
            || capture.trace != Some(Ok(expected_page(2)))
        {
            failures.push(format!(
                "{name}/{id}: actual supported public page content differs"
            ));
        }
    }
}

#[test]
fn supported_profile_and_legacy_captures_ignore_sidecars_aliases_and_nonfiles() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g14-capture-compatibility-")
        .tempdir()
        .expect("exclusive fixture under root executor's J-backed TMP");
    let root = fixture.path().to_path_buf();
    let db = root.join("captures.sqlite");
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("actual supported capture directory");
    let store = Store::open(&db, &root).expect("actual writable SQLite Store");
    let neighbor_path = root.join("neighbor.bench");
    fs::write(&neighbor_path, NEIGHBOR).expect("literal independent neighbor file");
    store
        .register_session(
            "neighbor",
            Some("healthy neighbor"),
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
    .expect("public seed neighbor encoding");
    let mut supported_paths = Vec::new();
    for (index, id) in IDS.iter().enumerate() {
        let paths: [PathBuf; 3] = std::array::from_fn(|kind| {
            hot.join(format!("{}-{id}.{}", PREFIXES[kind], EXTENSIONS[kind]))
        });
        for kind in 0..3 {
            if index == 1 && kind == 0 {
                continue;
            }
            fs::write(&paths[kind], PAYLOADS[kind]).expect("literal supported artifact bytes");
            supported_paths.push((paths[kind].clone(), PAYLOADS[kind].to_vec()));
        }
        if index >= 2 {
            store
                .register_session(
                    id,
                    Some(LEGACY_LABELS[index - 2]),
                    STARTS[index],
                    Some(&paths[0]),
                    Some(&paths[1]),
                    Some(&paths[2]),
                )
                .expect("authoritative existing arbitrary-ID capture");
            store
                .finalize_session(id, LEGACY_ENDS[index - 2])
                .expect("authoritative existing capture end");
        }
    }
    let ignored_paths: Vec<_> = IGNORED_NAMES.iter().map(|name| hot.join(name)).collect();
    for path in &ignored_paths {
        fs::write(path, IGNORED).expect("actual regular ignored sidecar or latest alias");
    }
    let directories = [
        hot.join("bench-20200108T000000Z.log"),
        hot.join("position-trace-20200109T000000Z.csv"),
    ];
    for directory in &directories {
        fs::create_dir(directory).expect("prefixed nonfile input fixture");
    }
    let symlink_path = hot.join("bench-20200110T000000Z.log");
    let symlink_target = hot.join("bench-20200101T000000Z.log");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&symlink_target, &symlink_path)
        .expect("actual Unix supported-prefix symlink exclusion fixture");
    let symlink_fixture = cfg!(unix);
    let seeded = observe(&store);

    let imported_count = store
        .import_legacy_hot_report(50)
        .map_err(|error| error.to_string());
    let imported = observe(&store);
    let repeated_count = store
        .import_legacy_hot_report(50)
        .map_err(|error| error.to_string());
    let repeated = observe(&store);
    let kept_supported_bytes: Vec<_> = supported_paths
        .iter()
        .map(|(path, _)| optional_bytes(path))
        .collect();
    let archived_count = store
        .archive_hot_sessions(0)
        .map_err(|error| error.to_string());
    let archived = observe(&store);
    let remaining_supported_bytes: Vec<_> = supported_paths
        .iter()
        .map(|(path, _)| optional_bytes(path))
        .collect();
    let ignored_after: Vec<_> = ignored_paths
        .iter()
        .map(|path| fs::read(path).expect("ignored regular bytes remain available"))
        .collect();
    let directories_after: Vec<_> = directories.iter().map(|path| path.is_dir()).collect();
    let symlink_after = if symlink_fixture {
        Some((
            fs::symlink_metadata(&symlink_path)
                .expect("original symlink metadata")
                .file_type()
                .is_symlink(),
            fs::read_link(&symlink_path).expect("original symlink target"),
        ))
    } else {
        None
    };
    drop(store);
    let reopened_store = Store::open(&db, &root).expect("actual compatibility database reopen");
    let reopened = observe(&reopened_store);
    let ignored_reopened: Vec<_> = ignored_paths
        .iter()
        .map(|path| fs::read(path).expect("ignored regular bytes after reopen"))
        .collect();
    let neighbor_bytes =
        fs::read(&neighbor_path).expect("actual independent neighbor bytes after reopen");
    drop(reopened_store);
    fixture
        .close()
        .expect("actual compatibility database/artifact cleanup");
    let cleanup_complete = !root
        .try_exists()
        .expect("actual compatibility cleanup observation");

    // Existing authoritative rows and independent payload controls must survive
    // even an operation error. New captures, counts and layout remain oracles.
    assert!(cleanup_complete);
    assert_eq!(neighbor_before["id"], "neighbor");
    assert_eq!(neighbor_before["label"], "healthy neighbor");
    assert_eq!(neighbor_before["started_ms"], 9);
    assert_eq!(neighbor_before["ended_ms"], 10);
    assert_eq!(supported_paths.len(), 11);
    assert_eq!(
        kept_supported_bytes,
        supported_paths
            .iter()
            .map(|(_, bytes)| Some(bytes.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(ignored_after, vec![IGNORED.to_vec(); 10]);
    assert_eq!(ignored_reopened, vec![IGNORED.to_vec(); 10]);
    assert!(directories_after.into_iter().all(|exists| exists));
    if symlink_fixture {
        assert_eq!(symlink_after, Some((true, symlink_target)));
    }
    assert_eq!(neighbor_bytes, NEIGHBOR);
    for stage in [&seeded, &imported, &repeated, &archived, &reopened] {
        assert_eq!(stage.neighbor, neighbor_before);
        assert_eq!(
            stage.neighbor_page,
            (vec!["independent compatibility neighbor".to_string()], 1)
        );
        for index in 2..4 {
            let known = stage.captures[index]
                .as_ref()
                .expect("authoritative known capture stays present");
            assert_eq!(known.row.label.as_deref(), Some(LEGACY_LABELS[index - 2]));
            assert_eq!(
                (known.row.started_ms, known.row.ended_ms),
                (STARTS[index], Some(LEGACY_ENDS[index - 2]))
            );
            for (kind, payload) in PAYLOADS.iter().enumerate() {
                assert_eq!(
                    known.downloads[kind]
                        .as_ref()
                        .expect("known sibling reference")
                        .as_ref()
                        .expect("actual known sibling bytes")
                        .payload
                        .as_slice(),
                    *payload
                );
            }
            assert_eq!(known.bench, Some(Ok(expected_page(0))));
            assert_eq!(known.candump, Some(Ok(expected_candump())));
            assert_eq!(known.trace, Some(Ok(expected_page(2))));
        }
    }
    println!("G14_COMPATIBILITY_CONTROLS=complete;cleanup_complete=true;supported_ids=4;supported_artifacts=11;authoritative_legacy=2;ignored_regular=10;ignored_directories=2;symlink_fixture={symlink_fixture}");

    let mut failures = Vec::new();
    for (name, count) in [("import", &imported_count), ("repeat", &repeated_count)] {
        if *count
            != Ok(LegacyImportSummary {
                sessions: 4,
                artifacts: 11,
            })
        {
            failures.push(format!(
                "{name}: processed counts or returned failure: {count:?}"
            ));
        }
    }
    if archived_count != Ok(11) {
        failures.push(format!(
            "archive: supported artifact count or returned failure: {archived_count:?}"
        ));
    }
    if remaining_supported_bytes.iter().any(Option::is_some) {
        failures.push("archive: supported hot files were not removed".to_string());
    }
    for (name, stage, is_archived) in [
        ("import", &imported, false),
        ("repeat", &repeated, false),
        ("archive", &archived, true),
        ("reopen", &reopened, true),
    ] {
        collect_compatibility_failures(name, stage, &root, is_archived, &mut failures);
    }
    assert!(failures.is_empty(), "G14: supported profile and authoritative legacy captures must retain chronology and correct buckets while sidecars aliases nonfiles and symlinks remain excluded: {failures:?}");
}
