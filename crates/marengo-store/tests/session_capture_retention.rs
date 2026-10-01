//! Candidate-only public capture retention at a deterministic absolute cutoff.
//! The original has no purge_before API and is never executed with this probe.
#![allow(clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::{ErrorKind, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use marengo_store::{log_dir, LogEventInsert, Store, StructuredLogQuery};
use serde_json::Value;

const CUTOFF: u64 = 1_577_836_800_000;
const IDS: [&str; 4] = [
    "19700101T000000Z",
    "20191231T235959Z",
    "20200101T000000Z",
    "20200101T000001Z",
];
const STARTS: [u64; 4] = [0, 1_577_836_799_000, 1_577_836_800_000, 1_577_836_801_000];
const PAYLOADS: [&[u8]; 4] = [
    b"epoch capture\nepoch last\n",
    b"before capture\nbefore last\n",
    b"exact capture\nexact last\n",
    b"after capture\nafter last\n",
];
const LAST_LINES: [&str; 4] = ["epoch last", "before last", "exact last", "after last"];
const EVENTS: [(u64, &str); 4] = [
    (0, "cutoff epoch"),
    (1_577_836_799_999, "cutoff before"),
    (1_577_836_800_000, "cutoff exact"),
    (1_577_836_800_001, "cutoff after"),
];
const NEIGHBOR: &[u8] = b"newer authoritative neighbor\n";
const UNRELATED: &[u8] = b"unregistered artifact remains outside retention\n";

#[derive(Debug, PartialEq, Eq)]
struct Download {
    compressed: bool,
    payload: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct Capture {
    row: Value,
    download: Result<Download, String>,
    page: Result<(Vec<String>, u32), String>,
}

#[derive(Debug, PartialEq, Eq)]
struct Stage {
    captures: Vec<Option<Capture>>,
    neighbor: Option<Capture>,
    total_sessions: usize,
    events: Vec<(u64, String)>,
    fts_total: u32,
}

fn download(path: Option<&str>) -> Result<Download, String> {
    let path = path.ok_or_else(|| "missing registered bench reference".to_string())?;
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
        .expect("actual public retained capture lookup")?;
    let downloaded = download(row.bench_blob.as_deref());
    let page = store
        .read_bench_page(id, 1, 1)
        .map_err(|error| error.to_string());
    Some(Capture {
        row: serde_json::to_value(row).expect("public retained capture metadata encoding"),
        download: downloaded,
        page,
    })
}

fn observe(store: &Store) -> Stage {
    let query = StructuredLogQuery {
        from_ms: None,
        to_ms: None,
        level: None,
        target: Some("g14-cutoff".to_string()),
        session_id: None,
        q: Some("cutoff".to_string()),
        limit: 50,
        offset: 0,
    };
    let (events, fts_total) = store
        .query_structured_logs(&query)
        .expect("actual public FTS query");
    Stage {
        captures: IDS.iter().map(|id| capture(store, id)).collect(),
        neighbor: capture(store, "neighbor"),
        total_sessions: store
            .list_sessions(None, None, None, 50)
            .expect("complete public retention list")
            .len(),
        events: events
            .into_iter()
            .map(|row| (row.ts_ms, row.message))
            .collect(),
        fts_total,
    }
}

fn optional_bytes(path: &Path) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("actual retained file observation failed for {path:?}: {error}"),
    }
}

#[test]
fn absolute_capture_cutoff_preserves_exact_boundary_and_refuses_overflow_without_mutation() {
    let fixture = tempfile::Builder::new()
        .prefix("marengo-g14-capture-cutoff-")
        .tempdir()
        .expect("exclusive fixture under root executor's J-backed TMP");
    let root = fixture.path().to_path_buf();
    let db = root.join("captures.sqlite");
    let hot = log_dir(&root);
    fs::create_dir_all(&hot).expect("actual hot capture input directory");
    let store = Store::open(&db, &root).expect("actual writable SQLite Store");
    let neighbor_path = root.join("neighbor.bench");
    fs::write(&neighbor_path, NEIGHBOR).expect("literal newer authoritative neighbor file");
    store
        .register_session(
            "neighbor",
            Some("authoritative newer capture"),
            1_577_836_802_000,
            Some(&neighbor_path),
            None,
            None,
        )
        .expect("public newer authoritative capture registration");
    store
        .finalize_session("neighbor", 1_577_836_803_000)
        .expect("public authoritative neighbor end");
    let unrelated_path = root.join("unregistered.bench");
    fs::write(&unrelated_path, UNRELATED).expect("literal unregistered file outside retention");
    let mut hot_paths = Vec::new();
    for (index, id) in IDS.iter().enumerate() {
        let path = hot.join(format!("bench-{id}.log"));
        fs::write(&path, PAYLOADS[index]).expect("literal dated capture artifact");
        hot_paths.push(path);
    }
    let imported_count = store
        .import_legacy_hot(50)
        .expect("actual capture-date import");
    let archived_count = store
        .archive_hot_sessions(0)
        .expect("actual dated capture archive");
    let hot_removed: Vec<_> = hot_paths
        .iter()
        .map(|path| !path.try_exists().expect("actual hot removal observation"))
        .collect();
    let inserts: Vec<_> = EVENTS
        .iter()
        .map(|(timestamp, message)| LogEventInsert {
            ts_ms: *timestamp,
            level: "info".to_string(),
            target: "g14-cutoff".to_string(),
            message: message.to_string(),
            session_id: None,
            fields_json: None,
        })
        .collect();
    store
        .insert_log_events(&inserts)
        .expect("actual literal structured event insertion");
    let seeded = observe(&store);
    let archived_paths: Vec<_> = seeded
        .captures
        .iter()
        .map(|capture| {
            capture
                .as_ref()
                .expect("actual archived seeded capture")
                .row["bench_blob"]
                .as_str()
                .expect("actual archived seeded reference")
                .to_string()
        })
        .collect();
    let seeded_files: Vec<_> = archived_paths
        .iter()
        .map(|path| optional_bytes(Path::new(path)))
        .collect();

    // This API is intentionally candidate-only. No missing-method original run
    // can count as behavioral regression evidence. Overflow must be checked
    // before either capture or structured-event deletion.
    let overflow_error = store
        .purge_before(u64::MAX)
        .err()
        .map(|error| error.to_string());
    let overflow = observe(&store);
    let overflow_files: Vec<_> = archived_paths
        .iter()
        .map(|path| optional_bytes(Path::new(path)))
        .collect();
    let purged_count = store
        .purge_before(CUTOFF)
        .expect("actual absolute capture cutoff");
    let purged = observe(&store);
    let purged_files: Vec<_> = archived_paths
        .iter()
        .map(|path| optional_bytes(Path::new(path)))
        .collect();
    let repeated_count = store
        .purge_before(CUTOFF)
        .expect("actual repeat absolute cutoff");
    let repeated = observe(&store);
    drop(store);
    let reopened_store = Store::open(&db, &root).expect("actual retention database reopen");
    let reopened = observe(&reopened_store);
    let reopened_files: Vec<_> = archived_paths
        .iter()
        .map(|path| optional_bytes(Path::new(path)))
        .collect();
    let neighbor_bytes = fs::read(&neighbor_path).expect("actual preserved newer neighbor bytes");
    let unrelated_bytes = fs::read(&unrelated_path).expect("actual preserved unregistered bytes");
    drop(reopened_store);
    fixture
        .close()
        .expect("actual retention database/artifact fixture cleanup");
    let cleanup_complete = !root
        .try_exists()
        .expect("actual cutoff fixture cleanup observation");

    // Seed and strictly newer controls precede the final boundary oracle.
    // Exact-cutoff absence/content/count mismatches are never early assertions.
    assert!(cleanup_complete);
    assert_eq!((imported_count, archived_count), (4, 4));
    assert!(hot_removed.into_iter().all(|removed| removed));
    assert_eq!(seeded.total_sessions, 5);
    assert_eq!(seeded.fts_total, 4);
    assert_eq!(
        seeded.events,
        vec![
            (1_577_836_800_001, "cutoff after".to_string()),
            (1_577_836_800_000, "cutoff exact".to_string()),
            (1_577_836_799_999, "cutoff before".to_string()),
            (0, "cutoff epoch".to_string()),
        ]
    );
    for index in 0..4 {
        let capture = seeded.captures[index]
            .as_ref()
            .expect("literal seeded capture present");
        assert_eq!(capture.row["id"], IDS[index]);
        assert_eq!(capture.row["started_ms"], STARTS[index]);
        assert_eq!(
            capture.download,
            Ok(Download {
                compressed: true,
                payload: PAYLOADS[index].to_vec()
            })
        );
        assert_eq!(capture.page, Ok((vec![LAST_LINES[index].to_string()], 2)));
        assert!(seeded_files[index]
            .as_ref()
            .expect("literal seeded gzip bytes")
            .starts_with(&[0x1f, 0x8b]));
    }
    let neighbor = seeded
        .neighbor
        .as_ref()
        .expect("literal newer neighbor present");
    assert_eq!(neighbor.row["id"], "neighbor");
    assert_eq!(neighbor.row["label"], "authoritative newer capture");
    assert_eq!(neighbor.row["started_ms"], 1_577_836_802_000u64);
    assert_eq!(neighbor.row["ended_ms"], 1_577_836_803_000u64);
    assert_eq!(
        neighbor.download,
        Ok(Download {
            compressed: false,
            payload: NEIGHBOR.to_vec()
        })
    );
    assert_eq!(neighbor.page, Ok((Vec::new(), 1)));
    for stage in [&overflow, &purged, &repeated, &reopened] {
        assert_eq!(stage.neighbor, seeded.neighbor);
        assert_eq!(stage.captures[3], seeded.captures[3]);
    }
    assert_eq!(neighbor_bytes, NEIGHBOR);
    assert_eq!(unrelated_bytes, UNRELATED);
    println!("G14_CUTOFF_CONTROLS=complete;cleanup_complete=true;imported_archived=4;literal_events=4;newer_neighbor_preserved=true;unregistered_file_preserved=true");

    let mut failures = Vec::new();
    if overflow_error.is_none() || overflow != seeded || overflow_files != seeded_files {
        failures.push(
            "overflow: refusal must preserve all public rows events and archived bytes".to_string(),
        );
    }
    if purged_count != (2, 2) || repeated_count != (0, 0) {
        failures.push(format!(
            "strict-before deletion/repeat counts differ: {purged_count:?}/{repeated_count:?}"
        ));
    }
    let expected_events = vec![
        (1_577_836_800_001, "cutoff after".to_string()),
        (1_577_836_800_000, "cutoff exact".to_string()),
    ];
    let expected_files = vec![None, None, seeded_files[2].clone(), seeded_files[3].clone()];
    for (name, stage) in [
        ("purge", &purged),
        ("repeat", &repeated),
        ("reopen", &reopened),
    ] {
        if stage.total_sessions != 3 || stage.fts_total != 2 || stage.events != expected_events {
            failures.push(format!(
                "{name}: strict cutoff public list/FTS differs: sessions={} total={} events={:?}",
                stage.total_sessions, stage.fts_total, stage.events
            ));
        }
        if stage.captures[0].is_some()
            || stage.captures[1].is_some()
            || stage.captures[2] != seeded.captures[2]
        {
            failures.push(format!("{name}: old captures must expire and the exact-cutoff capture must remain unchanged"));
        }
    }
    if purged_files != expected_files || reopened_files != expected_files {
        failures
            .push("purge/reopen: only strictly older archived files may be removed".to_string());
    }
    assert!(failures.is_empty(), "G14: absolute retention must refuse overflowing cutoffs without mutation and delete only captures and events strictly before the literal boundary through reopen: {failures:?}");
}
