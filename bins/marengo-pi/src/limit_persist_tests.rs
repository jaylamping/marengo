//! Worker regressions through the actual queue, filesystem writes and audit.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{ActionEvent, Envelope, PersistStatus};
use chappe::Bus;
use marengo_config::{load_control_config_from, load_motors_config_from};

use super::{
    carry_pending_limit_patch, ConfigPersistQueue, PersistRequest, PersistTestHooks,
    TOPIC_AUDIT_ACTION,
};

const CLEANUP_BOUND: Duration = Duration::from_secs(2);

struct ReleaseOnDrop(Sender<()>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

pub(super) fn copied_config() -> (tempfile::TempDir, PathBuf) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(expected) = std::env::var_os("BATCH07_EXPECTED_MANIFEST") {
        assert_eq!(
            manifest.canonicalize().expect("compiled manifest"),
            PathBuf::from(expected)
                .canonicalize()
                .expect("expected frozen manifest")
        );
    }
    let root = manifest.join("../..");
    let temp = tempfile::tempdir().expect("exclusive worker resource");
    let config = temp.path().join("config");
    std::fs::create_dir_all(&config).expect("copied config directory");
    for name in ["robot.yaml", "control.yaml", "motors.yaml", "homing.yaml"] {
        std::fs::copy(root.join("config").join(name), config.join(name))
            .expect("copy existing config fixture");
    }
    (temp, config)
}

pub(super) fn request(
    config_dir: &std::path::Path,
    kp: f64,
    timestamp_ms: u64,
    session: &str,
) -> PersistRequest {
    let mut control = load_control_config_from(config_dir).expect("actual source draft");
    control
        .control
        .joints
        .get_mut("right_elbow_pitch")
        .expect("fixture elbow")
        .impedance
        .kp = kp;
    PersistRequest {
        config_dir: config_dir.to_path_buf(),
        motors: None,
        control,
        timestamp_ms,
        session_id: session.into(),
        operator_id: "worker-regression".into(),
        joint: "right_elbow_pitch".into(),
        param: "impedance.kp".into(),
    }
}

pub(super) fn assert_matching_durable(event: &ActionEvent, timestamp_ms: u64, session: &str) {
    assert_eq!(event.timestamp_ms, timestamp_ms);
    assert_eq!(event.session_id, session);
    assert_eq!(event.operator_id, "worker-regression");
    assert_eq!(event.joint, "right_elbow_pitch");
    assert_eq!(event.action, "config_persist");
    assert!(event.accepted);
    assert_eq!(event.persist_status, PersistStatus::Durable as i32);
    assert!(!event.config_revision.is_empty());
}

#[derive(Debug)]
struct RetainedRun {
    final_kp: f64,
    bytes_changed: bool,
    before_write_sessions: Vec<String>,
    actual_written_kp: Vec<(String, Result<Option<f64>, String>)>,
    actions: Vec<ActionEvent>,
    gate_timed_out: bool,
}

fn run_retained_pair(owner_stopping: bool) -> RetainedRun {
    let (temp, config_dir) = copied_config();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("original config bytes");
    let owner_shutdown = Arc::new(AtomicBool::new(false));
    let chappe = Arc::new(Bus::new(16));
    let mut actions_rx = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleaseOnDrop(release_tx.clone());
    let gate_rx = Mutex::new(release_rx);
    let gate_timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_in_hook = Arc::clone(&gate_timed_out);
    let before_write_sessions = Arc::new(Mutex::new(Vec::new()));
    let before_write_in_hook = Arc::clone(&before_write_sessions);
    let actual_written_kp = Arc::new(Mutex::new(Vec::new()));
    let written_kp_in_hook = Arc::clone(&actual_written_kp);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_write: Some(Arc::new(move |request| {
                before_write_in_hook
                    .lock()
                    .expect("bounded observer")
                    .push(request.session_id.clone());
                if request.session_id == "worker-first" {
                    let _ = entered_tx.send(());
                    if gate_rx
                        .lock()
                        .expect("single in-flight write gate")
                        .recv_timeout(CLEANUP_BOUND)
                        .is_err()
                    {
                        timed_out_in_hook.store(true, Ordering::SeqCst);
                    }
                }
            })),
            before_publish: Some(Arc::new(move |request| {
                // Independently read the real installed bytes at this positive
                // seam; a mocked success/publication cannot satisfy the oracle.
                let installed = load_control_config_from(&request.config_dir)
                    .map(|control| {
                        control
                            .control
                            .joints
                            .get("right_elbow_pitch")
                            .map(|joint| joint.impedance.kp)
                    })
                    .map_err(|error| error.to_string());
                written_kp_in_hook
                    .lock()
                    .expect("bounded disk observer")
                    .push((request.session_id.clone(), installed));
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
        },
    );
    queue
        .enqueue(request(&config_dir, 41.0, 7031, "worker-first"))
        .expect("accept real first write");
    entered_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual first request taken and writing at gated I/O");
    queue
        .enqueue(request(&config_dir, 43.0, 7032, "worker-second"))
        .expect("accept second while first is in flight");
    owner_shutdown.store(owner_stopping, Ordering::SeqCst);
    // Close the caller's sender after both acceptances. This permits bounded
    // termination in both the old and repaired real worker implementations.
    // No assertion can strand the gate, including a demonstrated abandonment.
    let _ = release_tx.send(());
    drop(queue);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual worker return after closing queue admission");

    let mut actions = Vec::new();
    while let Ok(bytes) = actions_rx.try_recv() {
        let envelope = Envelope::decode(bytes.as_slice()).expect("actual worker envelope");
        assert_eq!(envelope.source_node, "marengo-pi");
        assert_eq!(envelope.message_type, "marengo.v1.ActionEvent");
        actions
            .push(ActionEvent::decode(envelope.payload.as_slice()).expect("actual worker action"));
    }
    let final_kp = load_control_config_from(&config_dir)
        .expect("actual final disk config")
        .control
        .joints["right_elbow_pitch"]
        .impedance
        .kp;
    let result = RetainedRun {
        final_kp,
        bytes_changed: std::fs::read(config_dir.join("control.yaml")).expect("actual final bytes")
            != original,
        before_write_sessions: before_write_sessions
            .lock()
            .expect("completed write observer")
            .clone(),
        actual_written_kp: actual_written_kp
            .lock()
            .expect("completed disk observer")
            .clone(),
        actions,
        gate_timed_out: gate_timed_out.load(Ordering::SeqCst),
    };
    println!("owner_stopping={owner_stopping}, actual retained-write run={result:?}");
    result
}

fn assert_first_write_reached_real_disk_and_outcome(run: &RetainedRun) {
    assert!(!run.gate_timed_out, "real gate was explicitly released");
    assert!(run.bytes_changed);
    assert_eq!(
        run.before_write_sessions.first().map(String::as_str),
        Some("worker-first")
    );
    assert_eq!(
        run.actual_written_kp.first(),
        Some(&("worker-first".into(), Ok(Some(41.0))))
    );
    assert_matching_durable(
        run.actions.first().expect("actual first terminal action"),
        7031,
        "worker-first",
    );
}

fn assert_both_accepted_writes_completed(run: &RetainedRun) {
    assert_eq!(run.before_write_sessions, ["worker-first", "worker-second"]);
    assert_eq!(
        run.actual_written_kp,
        [
            ("worker-first".into(), Ok(Some(41.0))),
            ("worker-second".into(), Ok(Some(43.0))),
        ]
    );
    assert_eq!(
        run.actions.len(),
        2,
        "each retained request has its real outcome"
    );
    assert_matching_durable(&run.actions[0], 7031, "worker-first");
    assert_matching_durable(&run.actions[1], 7032, "worker-second");
}

#[test]
fn owner_shutdown_preserves_the_second_accepted_real_write() {
    // Neighboring reachability control: the same actual queue/Drop/gate fixture
    // reaches both filesystem writes and matching actions without owner stop.
    let control = run_retained_pair(false);
    assert_first_write_reached_real_disk_and_outcome(&control);
    assert_eq!(control.final_kp, 43.0);
    assert_both_accepted_writes_completed(&control);

    let stopping = run_retained_pair(true);
    assert_first_write_reached_real_disk_and_outcome(&stopping);
    // Decisive old-behavior regression after real worker return and all cleanup:
    // the owner flag must not discard a previously accepted retained draft.
    assert_eq!(
        stopping.final_kp, 43.0,
        "owner shutdown must finish the second accepted filesystem write"
    );
    assert_both_accepted_writes_completed(&stopping);
}

#[test]
fn queue_stays_busy_until_the_real_terminal_action_is_published() {
    let (temp, config_dir) = copied_config();
    let original = std::fs::read(config_dir.join("control.yaml")).expect("original config bytes");
    let owner_shutdown = Arc::new(AtomicBool::new(false));
    let chappe = Arc::new(Bus::new(16));
    let mut actions = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (exited_tx, exited_rx) = mpsc::channel();
    let _release_on_failure = ReleaseOnDrop(release_tx.clone());
    let gate_rx = Mutex::new(release_rx);
    let gate_timed_out = Arc::new(AtomicBool::new(false));
    let timeout_in_hook = Arc::clone(&gate_timed_out);
    let queue = ConfigPersistQueue::spawn_with_test_hooks(
        Arc::clone(&chappe),
        temp.path().to_path_buf(),
        PersistTestHooks {
            before_publish: Some(Arc::new(move |request| {
                let _ = entered_tx.send(request.session_id.clone());
                if gate_rx
                    .lock()
                    .expect("one publication gate")
                    .recv_timeout(CLEANUP_BOUND)
                    .is_err()
                {
                    timeout_in_hook.store(true, Ordering::SeqCst);
                }
            })),
            on_worker_exit: Some(Arc::new(move || {
                let _ = exited_tx.send(());
            })),
            ..PersistTestHooks::default()
        },
    );
    queue
        .enqueue(request(
            &config_dir,
            47.0,
            7047,
            "worker-publication-boundary",
        ))
        .expect("accept real write before publication gate");
    let entered_session = entered_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("actual completed write reached publication boundary");
    let at_publication_kp = load_control_config_from(&config_dir)
        .expect("actual written YAML while terminal publication is blocked")
        .control
        .joints["right_elbow_pitch"]
        .impedance
        .kp;
    let written_bytes = std::fs::read(config_dir.join("control.yaml"))
        .expect("actual installed bytes before publication");
    // Observe existing public queue state and actual topic at a positive gate.
    // No timer/sleep or fabricated completion result supplies this oracle.
    let busy_before_publication = queue.is_busy();
    let action_before_publication = actions.try_recv();
    owner_shutdown.store(true, Ordering::SeqCst);
    let _ = release_tx.send(());
    drop(queue);
    exited_rx
        .recv_timeout(CLEANUP_BOUND)
        .expect("real worker returned after publication and queue closure");

    // Real I/O/publication and cleanup controls precede the busy assertion.
    assert!(!gate_timed_out.load(Ordering::SeqCst));
    assert_eq!(entered_session, "worker-publication-boundary");
    assert_eq!(at_publication_kp, 47.0);
    assert_ne!(written_bytes, original);
    assert!(
        matches!(
            action_before_publication,
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ),
        "actual matching terminal publication was pending at the gate"
    );
    let bytes = actions
        .try_recv()
        .expect("actual completed terminal publication after releasing gate");
    let envelope = Envelope::decode(bytes.as_slice()).expect("actual worker envelope");
    let event = ActionEvent::decode(envelope.payload.as_slice()).expect("actual worker action");
    assert_matching_durable(&event, 7047, "worker-publication-boundary");
    assert_eq!(
        load_control_config_from(&config_dir)
            .expect("actual final config")
            .control
            .joints["right_elbow_pitch"]
            .impedance
            .kp,
        47.0
    );
    println!(
        "actual publication boundary: busy={busy_before_publication}, disk_kp={at_publication_kp}, terminal={event:?}"
    );
    assert!(
        busy_before_publication,
        "worker must remain busy until the matching terminal action is published"
    );
}
#[test]
fn later_control_save_preserves_pending_limit_patch() {
    let (_temp, config_dir) = copied_config();
    let mut pending = request(&config_dir, 41.0, 8011, "pending-limit");
    let mut motors = load_motors_config_from(&config_dir).expect("motors fixture");
    let motor = motors
        .motors
        .iter_mut()
        .find(|motor| motor.joint == pending.joint)
        .expect("fixture joint");
    motor.bench.position_lower_rad = -1.23;
    motor.bench.position_upper_rad = 2.34;
    pending.motors = Some(motors);
    pending.param = "limit_patch".into();
    let entry = pending
        .control
        .control
        .joints
        .get_mut(&pending.joint)
        .expect("fixture control");
    entry.position_soft_lower_rad = Some(-1.1);
    entry.position_soft_upper_rad = Some(2.2);

    let mut later = request(&config_dir, 47.0, 8012, "later-control-save");
    carry_pending_limit_patch(&pending, &mut later).expect("carry pending limits");

    let motor = later
        .motors
        .as_ref()
        .expect("limits require motors write")
        .motors
        .iter()
        .find(|motor| motor.joint == pending.joint)
        .expect("fixture motor");
    assert_eq!(motor.bench.position_lower_rad, -1.23);
    assert_eq!(motor.bench.position_upper_rad, 2.34);
    let control = later
        .control
        .control
        .joints
        .get(&pending.joint)
        .expect("fixture control");
    assert_eq!(control.position_soft_lower_rad, Some(-1.1));
    assert_eq!(control.position_soft_upper_rad, Some(2.2));
    assert_eq!(control.impedance.kp, 47.0);
}
