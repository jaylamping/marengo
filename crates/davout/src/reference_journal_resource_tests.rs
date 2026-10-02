//! Real resource aliases and incompatible persistent SQLite formats.
#![allow(clippy::expect_used)]

use super::*;
use crate::reference_journal_tests::support;
use crate::simulation::SimulationBus;
use crate::Supervisor;

fn resource() -> support::FixtureTree {
    support::FixtureTree::new(
        "journal-resource",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
}

fn factory_refuses(tree: &support::FixtureTree, history: &Path, journal: &Path) -> bool {
    match Supervisor::from_simulation_with_reference_journal(
        tree.path(),
        SimulationBus::default(),
        history,
        journal,
    ) {
        Err(_) => true,
        Ok(mut owner) => {
            // A failing alias oracle must still close and join its actual worker.
            let drained =
                owner.drain_reference_journal_until(Instant::now() + Duration::from_secs(5));
            assert!(drained.is_complete());
            false
        }
    }
}

#[test]
fn aliased_history_spellings_refuse_before_lazy_journal_installation() {
    let tree = resource();
    let name = format!(
        "{}-relative-slot.sqlite3",
        tree.path()
            .file_name()
            .expect("fixture name")
            .to_string_lossy()
    );
    let relative = PathBuf::from(name);
    let absolute = std::env::current_dir()
        .expect("process cwd")
        .join(&relative);
    let relative_refused = factory_refuses(&tree, &relative, &absolute);
    assert!(!absolute.exists());

    let sub = tree.path().join("sub");
    fs::create_dir(&sub).expect("owned alias traversal directory");
    let journal = tree.path().join("shared.sqlite3");
    let parent_spelling = sub.join("../shared.sqlite3");
    let parent_refused = factory_refuses(&tree, &parent_spelling, &journal);
    assert!(!journal.exists());

    #[cfg(unix)]
    let symlink_refused = {
        let link = tree.path().join("alias");
        std::os::unix::fs::symlink(&sub, &link).expect("owned symlink alias");
        let actual = sub.join("shared.sqlite3");
        let refused = factory_refuses(&tree, &link.join("shared.sqlite3"), &actual);
        assert!(!actual.exists());
        refused
    };

    assert!(
        relative_refused,
        "relative and absolute paths share one slot"
    );
    assert!(parent_refused, "parent traversal shares one slot");
    #[cfg(unix)]
    assert!(symlink_refused, "symlink parent shares one slot");
}

#[test]
fn distinct_explicit_resources_keep_database_opening_lazy() {
    let tree = resource();
    let history = tree.path().join("history.yaml");
    let journal = tree.path().join("journal.sqlite3");
    let mut owner = Supervisor::from_simulation_with_reference_journal(
        tree.path(),
        SimulationBus::default(),
        &history,
        &journal,
    )
    .expect("distinct explicit resources");
    let drained = owner.drain_reference_journal_until(Instant::now() + Duration::from_secs(5));
    assert!(drained.is_complete());
    assert!(!history.exists());
    assert!(!journal.exists());
}

fn assert_no_sidecars(path: &Path) {
    for suffix in ["-journal", "-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        assert!(!PathBuf::from(sidecar).exists(), "unexpected {suffix}");
    }
}

fn wal_format_is_preserved(worker: bool) {
    let tree = resource();
    let original = tree.path().join("wal.sqlite3");
    let input = crate::reference_journal_tests::actual_input();
    let mut db = Database::open(&original, true, true).expect("actual journal creation");
    let body = input.encode(db.session, 1).expect("actual acquired input");
    let shared = Shared {
        state: Mutex::new(State::default()),
        changed: Condvar::new(),
    };
    db.append(1, &body, &shared, None)
        .expect("real durable row");
    drop(db);
    let connection = Connection::open(&original).expect("owned corruption fixture");
    connection
        .execute("UPDATE reference_events SET checksum=zeroblob(32)", [])
        .expect("actual corrupt checksum");
    let mode: String = connection
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .expect("real persistent WAL format");
    assert_eq!(mode, "wal");
    drop(connection);
    let bytes = fs::read(&original).expect("closed incompatible bytes");
    assert_eq!(&bytes[18..20], &[2, 2]);
    assert_no_sidecars(&original);

    let path = tree.path().join(if worker {
        "worker.sqlite3"
    } else {
        "reader.sqlite3"
    });
    fs::write(&path, &bytes).expect("independent incompatible fixture");
    if worker {
        let identity = JobIdentity {
            nonce: Arc::new(()),
            sequence: 1,
        };
        let mut journal = Journal::spawn(path.clone(), None).expect("actual lazy worker");
        journal
            .submit(
                identity.clone(),
                crate::reference_journal_tests::actual_input(),
            )
            .expect("accepted actual input");
        let report = journal.wait_until(Instant::now() + Duration::from_secs(5));
        let completion = journal.take_matching(&identity).expect("actual completion");
        assert!(report.worker_terminated);
        assert!(matches!(
            completion.result,
            ReferenceJournalResult::Failed { .. }
        ));
    } else {
        assert!(
            inspect(&path, 8).is_err(),
            "inspection must refuse incompatible history"
        );
    }
    assert!(
        fs::read(&path).expect("preserved file") == bytes,
        "incompatible database bytes changed"
    );
    assert_no_sidecars(&path);
}

#[test]
fn wal_format_with_corrupt_history_is_preserved_by_actual_worker() {
    wal_format_is_preserved(true);
}

#[test]
fn wal_format_with_corrupt_history_is_preserved_by_inspection() {
    wal_format_is_preserved(false);
}
