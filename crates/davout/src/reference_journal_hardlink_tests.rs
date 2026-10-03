//! Hard-link identity remains shared even when canonical filenames differ.
#![allow(clippy::expect_used)]

use super::*;
use crate::reference_journal_tests::support;
use crate::simulation::SimulationBus;
use crate::Supervisor;

fn resource() -> support::FixtureTree {
    support::FixtureTree::new(
        "journal-hardlink",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
}

#[test]
fn existing_database_refuses_history_linked_to_its_rollback_resource() {
    let tree = resource();
    let journal = tree.path().join("reference.sqlite3");
    let mut database = Database::open(&journal, true, true).expect("real existing database");
    let input = crate::reference_journal_tests::actual_input();
    let body = input
        .encode(database.session, 1)
        .expect("real retained body");
    let shared = Shared {
        state: Mutex::new(State::default()),
        changed: Condvar::new(),
    };
    database
        .append(1, &body, &shared, None)
        .expect("existing durable history");
    drop(database);
    let database_before = fs::read(&journal).expect("closed database bytes");
    let history = tree.path().join("history.yaml");
    let rollback = tree.path().join("reference.sqlite3-journal");
    let bytes = b"joints: []\n";
    fs::write(&history, bytes).expect("owned valid YAML");
    fs::hard_link(&history, &rollback).expect("actual shared file identity");
    // The fixture is well-formed YAML; the refusal below is about file
    // identity with the journal sidecar, not about parse failure.
    let _: serde_yaml::Value = serde_yaml::from_slice(bytes).expect("positive YAML fixture");
    let mut actual_storage_result = None;
    let refused = match Supervisor::from_simulation_with_reference_journal(
        tree.path(),
        SimulationBus::default(),
        &history,
        &journal,
    ) {
        Err(_) => true,
        Ok(mut owner) => {
            assert!(owner
                .drain_reference_journal_until(Instant::now() + Duration::from_secs(5))
                .is_complete());
            drop(owner);
            let identity = JobIdentity {
                nonce: Arc::new(()),
                sequence: 2,
            };
            let mut worker = Journal::spawn(journal.clone(), None).expect("actual worker");
            worker
                .submit(
                    identity.clone(),
                    crate::reference_journal_tests::actual_input(),
                )
                .expect("actual captured input");
            let report = worker.wait_until(Instant::now() + Duration::from_secs(5));
            actual_storage_result = Some(
                worker
                    .take_matching(&identity)
                    .expect("actual outcome")
                    .result,
            );
            assert!(report.worker_terminated);
            false
        }
    };
    let preserved = fs::read(&history).is_ok_and(|actual| actual == bytes);
    assert!(refused,
        "shared inode admitted; real legacy bytes preserved={preserved}; actual write={actual_storage_result:?}");
    assert!(preserved, "existing history must remain unchanged");
    assert!(
        fs::read(&journal).expect("preserved database") == database_before,
        "refusal must precede SQLite changes"
    );
    assert!(fs::read(&rollback).expect("preserved link") == bytes);
}

#[test]
fn other_sidecar_hardlinks_are_refused_before_worker_installation() {
    for suffix in ["-wal", "-shm"] {
        let tree = resource();
        let history = tree.path().join("history.yaml");
        let journal = tree.path().join("reference.sqlite3");
        let link = tree.path().join(format!("reference.sqlite3{suffix}"));
        fs::write(&history, b"joints: []\n").expect("owned valid history");
        fs::hard_link(&history, &link).expect("actual hard link");
        let refused = match Supervisor::from_simulation_with_reference_journal(
            tree.path(),
            SimulationBus::default(),
            &history,
            &journal,
        ) {
            Err(_) => true,
            Ok(mut owner) => {
                assert!(owner
                    .drain_reference_journal_until(Instant::now() + Duration::from_secs(5))
                    .is_complete());
                false
            }
        };
        assert!(
            refused,
            "hard-linked {suffix} must refuse before worker installation"
        );
        assert_eq!(
            fs::read(&history).expect("preserved history"),
            b"joints: []\n"
        );
        assert!(!journal.exists());
    }
}
