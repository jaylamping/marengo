//! Owned named pipes exercise refusal without blocking the test runner.
#![allow(clippy::expect_used)]

use super::*;
use crate::reference_journal_tests::support;
use crate::simulation::SimulationBus;
use crate::Supervisor;
use std::os::unix::fs::FileTypeExt;
use std::process::{Child, Command, Stdio};

struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn named_pipe_resources_never_block_identity_preflight_or_legacy_loading() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for suffix in ["", "-journal", "-wal", "-shm", "history"] {
        let tree = support::FixtureTree::new("journal-filetype", &source);
        let target = if suffix == "history" {
            tree.path().join("history.yaml")
        } else {
            fs::write(tree.path().join("history.yaml"), b"joints: []\n")
                .expect("owned regular history");
            tree.path().join(format!("reference.sqlite3{suffix}"))
        };
        assert!(Command::new("mkfifo")
            .arg(&target)
            .status()
            .expect("local named pipe")
            .success());
        let mut child = ChildGuard(Some(
            Command::new(std::env::current_exe().expect("actual test executable"))
                .args([
                    "--exact",
                    "reference_journal::filetype_tests::named_pipe_child_helper",
                    "--nocapture",
                ])
                .env("MARENGO_JOURNAL_FIFO_FIXTURE", tree.path())
                .env("MARENGO_JOURNAL_FIFO_SLOT", suffix)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("bounded child"),
        ));
        let deadline = Instant::now() + Duration::from_secs(3);
        let timed_out = loop {
            if child
                .0
                .as_mut()
                .expect("owned child")
                .try_wait()
                .expect("child status")
                .is_some()
            {
                break false;
            }
            if Instant::now() >= deadline {
                child
                    .0
                    .as_mut()
                    .expect("owned child")
                    .kill()
                    .expect("terminate blocked fixture child");
                break true;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let output = child
            .0
            .take()
            .expect("owned child")
            .wait_with_output()
            .expect("reaped child output");
        assert!(
            !timed_out,
            "construction blocked at owned named-pipe slot {suffix:?}; child reaped"
        );
        assert!(
            output.status.success(),
            "child refusal checks: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(fs::symlink_metadata(&target)
            .expect("preserved unsupported resource")
            .file_type()
            .is_fifo());
    }
}

#[test]
fn named_pipe_child_helper() {
    let Some(root) = std::env::var_os("MARENGO_JOURNAL_FIFO_FIXTURE").map(PathBuf::from) else {
        return; // Inert during ordinary runs; only the parent invokes this child.
    };
    let suffix = std::env::var("MARENGO_JOURNAL_FIFO_SLOT").expect("fixed parent slot");
    let journal = root.join("reference.sqlite3");
    let constructed = Supervisor::from_simulation_with_reference_journal(
        &root,
        SimulationBus::default(),
        root.join("history.yaml"),
        &journal,
    );
    if suffix == "history" {
        assert!(
            constructed.is_err(),
            "nonregular history must refuse before YAML read"
        );
        return;
    }
    let mut owner = constructed.expect("unsupported journal keeps lazy worker refusal");
    assert!(owner
        .drain_reference_journal_until(Instant::now() + Duration::from_secs(5))
        .is_complete());
    drop(owner);
    let identity = JobIdentity {
        nonce: Arc::new(()),
        sequence: 1,
    };
    let mut worker = Journal::spawn(journal, None).expect("actual lazy worker");
    worker
        .submit(
            identity.clone(),
            crate::reference_journal_tests::actual_input(),
        )
        .expect("actual captured input");
    let report = worker.wait_until(Instant::now() + Duration::from_secs(5));
    let completion = worker
        .take_matching(&identity)
        .expect("actual storage outcome");
    assert!(report.worker_terminated);
    assert!(matches!(
        completion.result,
        ReferenceJournalResult::Failed { .. }
    ));
    assert_eq!(
        fs::read(root.join("history.yaml")).expect("preserved history"),
        b"joints: []\n"
    );
}
