//! Existing calibration history must not occupy any SQLite sidecar slot.
#![allow(clippy::expect_used)]

use super::*;
use crate::reference_journal_tests::support;
use crate::simulation::SimulationBus;
use crate::Supervisor;

#[test]
fn preexisting_calibration_history_cannot_be_admitted_as_a_sqlite_sidecar() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for suffix in ["-journal", "-wal", "-shm"] {
        let tree = support::FixtureTree::new("journal-namespace", &source);
        let journal = tree.path().join("reference.sqlite3");
        let mut filename = journal.as_os_str().to_os_string();
        filename.push(suffix);
        let history = PathBuf::from(filename);
        let bytes = b"joints: []\n";
        fs::write(&history, bytes).expect("owned valid calibration history");
        // The fixture is well-formed YAML; the refusal below is about the
        // sidecar slot, not about parse failure.
        let _: serde_yaml::Value = serde_yaml::from_slice(bytes).expect("positive YAML fixture");
        let refused = match Supervisor::from_simulation_with_reference_journal(
            tree.path(),
            SimulationBus::default(),
            &history,
            &journal,
        ) {
            Err(_) => true,
            Ok(mut owner) => {
                // Fully release the admitted lazy owner before probing the same
                // concrete backend with a real captured acquisition input.
                let closed =
                    owner.drain_reference_journal_until(Instant::now() + Duration::from_secs(5));
                assert!(closed.is_complete());
                drop(owner);
                let identity = JobIdentity {
                    nonce: Arc::new(()),
                    sequence: 1,
                };
                let mut worker = Journal::spawn(journal.clone(), None).expect("real worker");
                worker
                    .submit(
                        identity.clone(),
                        crate::reference_journal_tests::actual_input(),
                    )
                    .expect("actual captured input accepted");
                let report = worker.wait_until(Instant::now() + Duration::from_secs(5));
                let _completion = worker
                    .take_matching(&identity)
                    .expect("actual storage outcome");
                assert!(report.worker_terminated);
                false
            }
        };
        let preserved = fs::read(&history).is_ok_and(|actual| actual == bytes);
        assert!(
            refused && preserved,
            "{suffix} slot: factory refused={refused}, existing history preserved={preserved}"
        );
        assert!(!journal.exists(), "refusal must precede database creation");
    }
}

#[cfg(unix)]
#[test]
fn canonical_parent_alias_cannot_hide_a_missing_history_sidecar_slot() {
    let tree = support::FixtureTree::new(
        "journal-sidecar-alias",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    );
    let actual = tree.path().join("actual");
    fs::create_dir(&actual).expect("owned parent");
    let alias = tree.path().join("alias");
    std::os::unix::fs::symlink(&actual, &alias).expect("owned parent alias");
    for suffix in ["-journal", "-wal", "-shm"] {
        let journal = actual.join("reference.sqlite3");
        let history = alias.join(format!("reference.sqlite3{suffix}"));
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
        assert!(refused, "canonical sidecar alias {suffix} must refuse");
        assert!(!journal.exists());
        assert!(!history.exists());
    }
}
