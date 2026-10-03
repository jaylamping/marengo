//! New-interface close/drain conformance through the actual filesystem worker.
//! This is not an original-binary regression or a substituted I/O outcome.
#![allow(clippy::expect_used)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{ActionEvent, Envelope};
use chappe::Bus;
use marengo_config::load_control_config_from;

use super::tests::{assert_matching_durable, copied_config, request};
use super::{ConfigPersistQueue, PersistDrainStatus, PersistTestHooks, TOPIC_AUDIT_ACTION};

const CLEANUP_BOUND: Duration = Duration::from_secs(2);

/// Release and close even if a fixture or observation fails before explicit cleanup.
struct GatedWorkerCleanup<'a> {
    queue: &'a ConfigPersistQueue,
    release: Sender<()>,
}

impl Drop for GatedWorkerCleanup<'_> {
    fn drop(&mut self) {
        let _ = self.release.send(());
        let _ = self.queue.close_and_drain(CLEANUP_BOUND);
    }
}

#[test]
fn zero_budget_close_reports_live_work_and_rejects_later_valid_admission() {
    let (temp, config_dir) = copied_config();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("original copied bytes");
    let retained = request(&config_dir, 51.0, 7051, "worker-zero-budget-retained");
    let later = request(&config_dir, 53.0, 7053, "worker-closed-later");
    let chappe = Arc::new(Bus::new(16));
    let mut actions = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (returned_tx, returned_rx) = mpsc::channel();
    let gate_rx = Mutex::new(release_rx);
    let gate_timed_out = Arc::new(AtomicBool::new(false));
    let timeout_in_hook = Arc::clone(&gate_timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |draft| {
                let _ = entered_tx.send(draft.session_id.clone());
                if gate_rx
                    .lock()
                    .expect("one real write gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    timeout_in_hook.store(true, Ordering::SeqCst);
                }
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = returned_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let cleanup = GatedWorkerCleanup {
        queue: &queue,
        release: release_tx.clone(),
    };

    // Capture observations; do not assert a timeout/admission result while the
    // worker is held. The positive BeforeWrite handshake identifies actual work.
    let retained_admission = queue.enqueue(retained);
    let entered_session = entered_rx.recv_timeout(CLEANUP_BOUND);
    let before_write_bytes = std::fs::read(config_dir.join("control.yaml"));
    let before_write_action = actions.try_recv();
    let timed_out = queue.close_and_drain(Duration::ZERO);
    let later_admission = queue.enqueue(later);
    let returned_while_gated = returned_rx.try_recv();

    // Unconditionally release, drain through actual JoinHandle termination and
    // observe the real worker-return hook before any decisive assertions.
    let released = release_tx.send(());
    let completed = queue.close_and_drain(CLEANUP_BOUND);
    let returned = returned_rx.recv_timeout(CLEANUP_BOUND);
    drop(cleanup);
    drop(queue);
    let installed = load_control_config_from(&config_dir).expect("actual final disk reload");
    let installed_bytes = std::fs::read(config_dir.join("control.yaml")).expect("final disk bytes");
    let bytes = actions
        .try_recv()
        .expect("actual matching local completion");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual worker envelope");
    let event = ActionEvent::decode(envelope.payload.as_slice()).expect("actual worker action");
    let extra_action = actions.try_recv();

    // Reachability and cleanup controls precede the timeout/closed-admission oracle.
    assert!(retained_admission.is_ok());
    assert_eq!(
        entered_session.expect("actual retained request entered BeforeWrite"),
        "worker-zero-budget-retained"
    );
    assert!(released.is_ok());
    assert!(returned.is_ok(), "actual worker-return hook was observed");
    assert!(!gate_timed_out.load(Ordering::SeqCst));
    assert_eq!(completed.status, PersistDrainStatus::Complete);
    assert!(completed.worker_terminated, "actual worker joined");
    assert!(completed.is_idle());
    assert_eq!(completed.successful_writes, 1);
    assert_eq!(completed.failed_writes, 0);
    assert_eq!(completed.publication_failures, 0);
    assert_eq!(completed.coalesced_requests, 0);
    assert!(completed.worker_error.is_none());
    assert_eq!(
        installed.control.joints["right_elbow_pitch"].impedance.kp,
        51.0
    );
    assert_ne!(installed_bytes, original);
    assert_eq!(envelope.source_node, "marengo-pi");
    assert_eq!(envelope.message_type, "marengo.v1.ActionEvent");
    assert_matching_durable(&event, 7051, "worker-zero-budget-retained");

    assert_eq!(before_write_bytes.expect("gated original bytes"), original);
    assert!(matches!(
        before_write_action,
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert!(
        matches!(returned_while_gated, Err(mpsc::TryRecvError::Empty)),
        "worker was still held at the real write boundary"
    );
    assert_eq!(timed_out.status, PersistDrainStatus::TimedOut);
    assert!(timed_out.in_flight);
    assert_eq!(timed_out.pending_requests, 0);
    assert!(!timed_out.worker_terminated);
    assert_eq!(timed_out.successful_writes, 0);
    assert_eq!(timed_out.failed_writes, 0);
    assert_eq!(timed_out.publication_failures, 0);
    assert!(timed_out.worker_error.is_none());
    assert!(
        later_admission.is_err(),
        "closed admission accepted a valid later draft"
    );
    assert!(
        matches!(
            extra_action,
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ),
        "rejected later draft must not produce a completion action"
    );
}

#[test]
#[allow(clippy::panic)] // Deliberate unwind only in the isolated actual worker hook.
fn publication_unwind_retains_written_disk_and_reports_unfinished_local_completion() {
    let (temp, config_dir) = copied_config();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("original copied bytes");
    let retained = request(&config_dir, 57.0, 7057, "worker-publication-unwind");
    let chappe = Arc::new(Bus::new(16));
    let mut actions = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (returned_tx, returned_rx) = mpsc::channel();
    let gate_rx = Mutex::new(release_rx);
    let gate_timed_out = Arc::new(AtomicBool::new(false));
    let timeout_in_hook = Arc::clone(&gate_timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_publish: Some(Arc::new(move |draft| {
                let _ = entered_tx.send(draft.session_id.clone());
                if gate_rx
                    .lock()
                    .expect("one real publication gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    timeout_in_hook.store(true, Ordering::SeqCst);
                }
                // The actual write has returned; publication has not begun.
                // Do not substitute an Err for either real I/O operation.
                panic!("qualification: isolated worker unwinds before local publication");
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = returned_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    let cleanup = GatedWorkerCleanup {
        queue: &queue,
        release: release_tx.clone(),
    };

    let admission = queue.enqueue(retained);
    let entered_session = entered_rx.recv_timeout(CLEANUP_BOUND);
    // Independent filesystem reads while the actual worker is BeforePublish.
    let written_at_gate = std::fs::read(config_dir.join("control.yaml"));
    let installed_at_gate = load_control_config_from(&config_dir);
    let action_at_gate = actions.try_recv();
    let returned_while_gated = returned_rx.try_recv();

    // Release the deliberate unwind, close/drain and observe actual termination
    // before checking the written state or the failure/publication assertions.
    let released = release_tx.send(());
    let failed = queue.close_and_drain(CLEANUP_BOUND);
    let returned = returned_rx.recv_timeout(CLEANUP_BOUND);
    drop(cleanup);
    drop(queue);
    let installed = load_control_config_from(&config_dir).expect("actual disk after worker unwind");
    let installed_bytes =
        std::fs::read(config_dir.join("control.yaml")).expect("written disk bytes");
    let final_action = actions.try_recv();

    assert!(admission.is_ok());
    assert_eq!(
        entered_session.expect("actual retained request entered BeforePublish"),
        "worker-publication-unwind"
    );
    assert!(released.is_ok());
    assert!(
        returned.is_ok(),
        "actual unwind return observer was reached"
    );
    assert!(!gate_timed_out.load(Ordering::SeqCst));
    assert!(
        failed.worker_terminated,
        "actual failed JoinHandle was joined"
    );
    assert_eq!(
        installed_at_gate
            .expect("actual disk reload at BeforePublish")
            .control
            .joints["right_elbow_pitch"]
            .impedance
            .kp,
        57.0
    );
    assert_eq!(
        installed.control.joints["right_elbow_pitch"].impedance.kp,
        57.0
    );
    assert_ne!(installed_bytes, original);
    assert_eq!(
        written_at_gate.expect("actual bytes at BeforePublish"),
        installed_bytes
    );
    assert!(matches!(
        returned_while_gated,
        Err(mpsc::TryRecvError::Empty)
    ));

    assert_eq!(failed.status, PersistDrainStatus::WorkerFailed);
    assert!(failed.in_flight, "completion publication never finished");
    assert!(!failed.is_idle());
    assert_eq!(failed.pending_requests, 0);
    assert!(failed.worker_error.as_deref().is_some_and(|message|
        message.contains("unexpectedly") || message.contains("panicked")),
        "actual worker unwind must retain a truthful error");
    // Counters finalize only after local publication returns. The independently
    // proven written disk is unfinished completion, not an invented failed write
    // or a substituted publication error result.
    assert_eq!(failed.successful_writes, 0);
    assert_eq!(failed.failed_writes, 0);
    assert_eq!(failed.publication_failures, 0);
    assert_eq!(failed.coalesced_requests, 0);
    assert!(matches!(
        action_at_gate,
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert!(
        matches!(
            final_action,
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ),
        "a completed write without actual publication must not invent Durable"
    );
}
