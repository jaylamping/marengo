//! Existing-public G13 import/archive contract; capture time is explicitly seeded.
//! Blob lookup and byte reads qualify artifact availability, not gateway HTTP.
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use marengo_store::{log_dir, LogSessionRow, Store};

const SESSION: &str = "20200101T000000Z";
const LABEL: &str = "authoritative capture label";
const START: u64 = 1_577_836_800_123;
const END: u64 = 1_577_836_820_123;
const BENCH: &[u8] = b"bench event\nbench completed\n";
const CANDUMP: &[u8] = b"(0.000000) can0 123#1122\n(0.005000) can0 124#33\n";
const TRACE: &[u8] = b"time,q\n1.0,0.25\n";
const NEIGHBOR_BYTES: [&[u8]; 3] = [
    b"neighbor first\nneighbor second\n",
    b"(0.000000) can0 222#55\n(0.005000) can0 223#66\n",
    b"time,q\n2.0,0.5\n",
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

#[derive(Debug, PartialEq, Eq)]
struct Contents {
    bench: Result<(Vec<String>, u32), String>,
    candump: Result<CandumpPage, String>,
    trace: Result<(Vec<String>, u32), String>,
    downloads: [Result<Download, String>; 3],
}

#[derive(Debug)]
struct Observation {
    row: LogSessionRow,
    contents: Contents,
}

fn download(path: Option<&str>) -> Result<Download, String> {
    let path = path.ok_or_else(|| "missing registered artifact".to_string())?;
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

fn observe(store: &Store, id: &str) -> Observation {
    let row = store
        .get_session(id)
        .expect("actual public session lookup")
        .expect("seeded session exists");
    let downloads = [
        download(row.bench_blob.as_deref()),
        download(row.candump_blob.as_deref()),
        download(row.trace_blob.as_deref()),
    ];
    Observation {
        row,
        contents: Contents {
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
        },
    }
}

fn paths(row: &LogSessionRow) -> [Option<&str>; 3] {
    [
        row.bench_blob.as_deref(),
        row.candump_blob.as_deref(),
        row.trace_blob.as_deref(),
    ]
}

fn strings(paths: &[PathBuf; 3]) -> [String; 3] {
    std::array::from_fn(|index| paths[index].display().to_string())
}

fn collect_failures(
    stage: &str,
    observed: &Observation,
    expected_paths: &[String; 3],
    compressed: bool,
    failures: &mut Vec<String>,
) {
    let row = &observed.row;
    if row.id != SESSION
        || row.label.as_deref() != Some(LABEL)
        || row.started_ms != START
        || row.ended_ms != Some(END)
    {
        failures.push(format!("{stage}: authoritative metadata"));
    }
    if paths(row) != std::array::from_fn(|index| Some(expected_paths[index].as_str())) {
        failures.push(format!("{stage}: sibling blob identities"));
    }
    let contents = &observed.contents;
    if contents.bench != Ok((vec!["bench completed".into()], 2))
        || contents.trace != Ok((vec!["1.0,0.25".into()], 2))
        || contents.candump
            != Ok(CandumpPage {
                total: 2,
                frames: vec![("can0".into(), 0x124, vec![0x33])],
            })
    {
        failures.push(format!("{stage}: public artifact pages"));
    }
    for (index, expected) in [BENCH, CANDUMP, TRACE].into_iter().enumerate() {
        if !contents.downloads[index]
            .as_ref()
            .is_ok_and(|download| download.compressed == compressed && download.payload == expected)
        {
            failures.push(format!("{stage}: artifact {index} download contents"));
        }
    }
}

#[test]
fn legacy_import_counts_unique_sessions_and_archive_preserves_authoritative_metadata() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g13-import-")
        .tempdir()
        .expect("exclusive fixture under runner's J-backed child TMP");
    let root = fixture.path().to_path_buf();
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("actual hot log directory");
    let names = [
        "bench-20200101T000000Z.log",
        "candump-20200101T000000Z.log",
        "position-trace-20200101T000000Z.csv",
    ];
    let hot_paths: [PathBuf; 3] = std::array::from_fn(|index| hot.join(names[index]));
    for (path, bytes) in hot_paths.iter().zip([BENCH, CANDUMP, TRACE]) {
        fs::write(path, bytes).expect("literal timestamp sibling hot file");
    }
    let neighbor_dir = root.join("untouched-neighbor");
    fs::create_dir(&neighbor_dir).expect("exclusive neighboring files");
    let neighbor_paths: [PathBuf; 3] =
        std::array::from_fn(|index| neighbor_dir.join(format!("artifact-{index}.log")));
    for (path, bytes) in neighbor_paths.iter().zip(NEIGHBOR_BYTES) {
        fs::write(path, bytes).expect("literal neighboring artifact");
    }
    let database = root.join("sessions.sqlite");
    let store = Store::open(&database, &root).expect("actual SQLite Store");
    store
        .register_session(SESSION, Some(LABEL), START, None, None, None)
        .expect("authoritative metadata before import");
    store.finalize_session(SESSION, END).expect("explicit end");
    let seed = store
        .get_session(SESSION)
        .expect("seed lookup")
        .expect("seed");
    store
        .register_session(
            "neighbor",
            Some("untouched neighboring capture"),
            11,
            Some(&neighbor_paths[0]),
            Some(&neighbor_paths[1]),
            Some(&neighbor_paths[2]),
        )
        .expect("complete neighboring row");
    store
        .finalize_session("neighbor", 22)
        .expect("neighbor end");
    let neighbor_before = observe(&store, "neighbor");

    let imported = store
        .import_legacy_hot_report(50)
        .expect("actual import keeping hot files")
        .sessions;
    let after_import = observe(&store, SESSION);
    let repeated = store
        .import_legacy_hot_report(50)
        .expect("actual repeated import")
        .sessions;
    let after_repeat = observe(&store, SESSION);
    let hot_controls: Vec<_> = hot_paths
        .iter()
        .map(|path| fs::read(path).expect("keep50 retained actual hot bytes"))
        .collect();
    let archived = store.archive_hot_sessions(0).expect("actual gzip archive");
    let after_archive = observe(&store, SESSION);
    let archived_paths: [PathBuf; 3] = std::array::from_fn(|index| {
        log_dir(&root)
            .join("blobs")
            .join("2020-01-01")
            .join(format!("{}.gz", names[index]))
    });
    let archive_controls: Vec<_> = archived_paths
        .iter()
        .map(|path| download(Some(path.to_str().expect("fixture path"))))
        .collect();
    let archived_candump_bytes = fs::metadata(&archived_paths[1])
        .expect("real compressed candump artifact")
        .len();
    let hot_removed: Vec<_> = hot_paths
        .iter()
        .map(|path| {
            !path
                .try_exists()
                .expect("actual archive removal observation")
        })
        .collect();
    let neighbor_after = observe(&store, "neighbor");
    drop(store);
    let reopened_store = Store::open(&database, &root).expect("actual database reopen");
    let reopened = observe(&reopened_store, SESSION);
    let neighbor_reopened = observe(&reopened_store, "neighbor");
    let sessions = reopened_store
        .list_sessions(None, None, None, 100)
        .expect("actual public session list after reopen");
    drop(reopened_store);
    fixture
        .close()
        .expect("actual fixture and database disposal");
    let removed = !root.try_exists().expect("actual cleanup observation");

    // Both imports, archive, all public observations and actual cleanup have
    // completed before healthy controls or the collected G13 classification.
    assert!(removed);
    assert_eq!(seed.label.as_deref(), Some(LABEL));
    assert_eq!(seed.started_ms, START);
    assert_eq!(seed.ended_ms, Some(END));
    assert_eq!(paths(&seed), [None, None, None]);
    assert_eq!(hot_controls, vec![BENCH, CANDUMP, TRACE]);
    assert_eq!(archived, 3, "actual three-artifact archive control");
    assert!(hot_removed.into_iter().all(|removed| removed));
    for (control, expected) in archive_controls.into_iter().zip([BENCH, CANDUMP, TRACE]) {
        assert_eq!(
            control.expect("independent gzip content control"),
            Download {
                compressed: true,
                payload: expected.to_vec()
            }
        );
    }
    assert_eq!(after_archive.row.candump_frame_count, Some(2));
    assert_eq!(
        after_archive.row.candump_bytes,
        Some(archived_candump_bytes)
    );
    assert_eq!(
        neighbor_before.row.label.as_deref(),
        Some("untouched neighboring capture")
    );
    assert_eq!(neighbor_before.row.started_ms, 11);
    assert_eq!(neighbor_before.row.ended_ms, Some(22));
    assert_eq!(
        paths(&neighbor_before.row),
        std::array::from_fn(|index| neighbor_paths[index].to_str())
    );
    assert_eq!(
        neighbor_before.contents.bench,
        Ok((vec!["neighbor second".into()], 2))
    );
    assert_eq!(
        neighbor_before.contents.trace,
        Ok((vec!["2.0,0.5".into()], 2))
    );
    assert_eq!(
        neighbor_before.contents.candump,
        Ok(CandumpPage {
            total: 2,
            frames: vec![("can0".into(), 0x223, vec![0x66])]
        })
    );
    for (index, expected) in NEIGHBOR_BYTES.into_iter().enumerate() {
        assert_eq!(
            neighbor_before.contents.downloads[index],
            Ok(Download {
                compressed: false,
                payload: expected.to_vec()
            })
        );
    }
    assert_eq!(
        serde_json::to_value(&neighbor_before.row).expect("public row encoding"),
        serde_json::to_value(&neighbor_after.row).expect("public row encoding")
    );
    assert_eq!(
        serde_json::to_value(&neighbor_before.row).expect("public row encoding"),
        serde_json::to_value(&neighbor_reopened.row).expect("public row encoding")
    );
    assert_eq!(neighbor_before.contents, neighbor_after.contents);
    assert_eq!(neighbor_before.contents, neighbor_reopened.contents);
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions.iter().filter(|row| row.id == SESSION).count(), 1);
    assert_eq!(
        sessions.iter().filter(|row| row.id == "neighbor").count(),
        1
    );
    println!(
        "G13_IMPORT_CONTROLS=complete;cleanup_complete=true;hot_artifacts=3;unique_sessions=2"
    );

    let mut failures = Vec::new();
    if imported != 1 || repeated != 1 {
        failures.push(format!(
            "unique session count: first={imported}, repeat={repeated}"
        ));
    }
    let expected_hot = strings(&hot_paths);
    let expected_archive = strings(&archived_paths);
    collect_failures("import", &after_import, &expected_hot, false, &mut failures);
    collect_failures("repeat", &after_repeat, &expected_hot, false, &mut failures);
    collect_failures(
        "archive",
        &after_archive,
        &expected_archive,
        true,
        &mut failures,
    );
    collect_failures("reopen", &reopened, &expected_archive, true, &mut failures);
    assert!(failures.is_empty(), "G13: import must count unique sessions and archive must preserve authoritative metadata and sibling contents: {failures:?}");
}
