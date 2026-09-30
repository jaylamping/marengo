//! Byte-identical old-public regression replay across closure of mutable bus access.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "../../marengo-homing/tests/support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use davout::{DavoutError, JointHomingState, OperationalMode, Supervisor};
use robstride::{BusError, CanBus, CanFrame, MemoryBus, MotorBus, ReceiveAttempt};
use support::TestDirectory;

#[derive(Default, Debug)]
struct Recording {
    tx: Vec<CanFrame>,
    zero_triggers: usize,
    delivered_frames: usize,
}

struct RecordingBus {
    inner: MemoryBus,
    witness: Arc<Mutex<Recording>>,
}

impl CanBus for RecordingBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.inner.send_frame(frame)?;
        let mut recording = self.witness.lock().expect("finite local recording");
        recording.tx.push(frame.clone());
        if frame.id == 0x0600fd01 {
            recording.zero_triggers += 1;
            // Literal Run status from configured pitch drive. This finite script
            // responds only to a real recorded SetZero; it is not a grant API.
            self.inner.rx_queue.push(CanFrame {
                id: 0x028001fd,
                data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8],
                extended: true,
            });
        }
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let attempt = self.inner.recv_one_nonblocking()?;
        if matches!(attempt, ReceiveAttempt::Frame(_)) {
            self.witness
                .lock()
                .expect("finite local recording")
                .delivered_frames += 1;
        }
        Ok(attempt)
    }
}

impl MotorBus for RecordingBus {}

fn fixture(
    label: &str,
) -> (
    TestDirectory,
    PathBuf,
    Supervisor<RecordingBus>,
    Arc<Mutex<Recording>>,
) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // The baseline replay sets this to bind compilation to its exact archive.
    // Ordinary collection still executes every behavioral assertion below.
    if let Some(expected) = std::env::var_os("BATCH06_EXPECTED_MANIFEST") {
        assert_eq!(
            manifest.canonicalize().expect("compiled manifest"),
            PathBuf::from(expected)
                .canonicalize()
                .expect("expected manifest")
        );
    }
    let owner = TestDirectory::new(label);
    let history = owner.path().join("history.yaml");
    let witness = Arc::new(Mutex::new(Recording::default()));
    let bus = RecordingBus {
        inner: MemoryBus::default(),
        witness: Arc::clone(&witness),
    };
    let supervisor =
        Supervisor::from_repo_with_calibration_record_path(manifest.join("../.."), bus, &history)
            .expect("existing public explicit-path owner");
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(supervisor.motors.motors.len(), 5);
    assert!(supervisor.homing_registry().calibration().joints.is_empty());
    assert!(!history.exists());
    *witness.lock().expect("recording reset") = Recording::default();
    (owner, history, supervisor, witness)
}

fn enables(recording: &Recording) -> Vec<u32> {
    recording
        .tx
        .iter()
        .filter(|frame| frame.id >> 24 == 3)
        .map(|frame| frame.id)
        .collect()
}

fn zeros(recording: &Recording) -> Vec<u32> {
    recording
        .tx
        .iter()
        .filter(|frame| frame.id >> 24 == 6)
        .map(|frame| frame.id)
        .collect()
}

#[test]
fn unhomed_direct_enable_cannot_arm_without_current_reference() {
    let (_owner, history, mut supervisor, witness) = fixture("unchanged-direct-enable");
    let target = supervisor.motors.motors[0].joint.clone();
    assert_eq!(
        supervisor.joint_homing_state(&target),
        JointHomingState::Unhomed
    );
    let result = supervisor.enable_targets(std::slice::from_ref(&target));
    let recording = witness.lock().expect("external witness");
    println!(
        "direct Unhomed Enable result={result:?}, mode={:?}, witness={recording:?}",
        supervisor.mode()
    );
    assert!(!history.exists());
    assert!(
        result.is_err()
            && enables(&recording).is_empty()
            && supervisor.mode() == OperationalMode::Disabled,
        "unqualified direct caller armed: result={result:?}, witness={recording:?}"
    );
}

#[test]
fn false_sign_calibration_refuses_before_any_arming_or_zero() {
    let (_owner, history, mut supervisor, witness) = fixture("unchanged-false-sign");
    let target = supervisor.motors.motors[0].joint.clone();
    assert!(
        supervisor
            .homing_config
            .homing
            .effective_joint(&target)
            .expect("configured target")
            .sign_test_required
    );
    let result = supervisor.calibrate_joint_zero(&target, "unchanged-false-sign", false);
    let recording = witness.lock().expect("external witness");
    println!(
        "false-sign calibration result={result:?}, mode={:?}, history={}, witness={recording:?}",
        supervisor.mode(),
        history.exists()
    );
    assert!(
        result.is_err()
            && enables(&recording).is_empty()
            && zeros(&recording).is_empty()
            && !history.exists()
            && supervisor.mode() == OperationalMode::Disabled,
        "false sign armed before refusal: result={result:?}, witness={recording:?}"
    );
}

#[test]
fn arbitrary_bus_one_target_calibration_cannot_arm_peers_or_persist_reference() {
    let (_owner, history, mut supervisor, witness) = fixture("unchanged-one-target");
    let target = supervisor.motors.motors[0].joint.clone();
    let result = supervisor.calibrate_joint_zero(&target, "unchanged-one-target", true);
    let recording = witness.lock().expect("external witness");
    println!(
        "one-target calibration result={result:?}, mode={:?}, history={}, witness={recording:?}",
        supervisor.mode(),
        history.exists()
    );
    assert!(result.is_err() && enables(&recording).is_empty() && zeros(&recording).is_empty()
        && !history.exists() && supervisor.mode() == OperationalMode::Disabled,
        "ordinary arbitrary-bus owner acquired reference or armed peers: result={result:?}, witness={recording:?}");
}

#[test]
fn unknown_target_preflight_refuses_without_output_control() {
    let (_owner, history, mut supervisor, witness) = fixture("unchanged-unknown-target");
    let result = supervisor.calibrate_joint_zero("missing_joint", "control", true);
    let recording = witness.lock().expect("external witness");
    assert!(
        matches!(result, Err(DavoutError::UnknownJoint { .. })),
        "target validation remains specific: {result:?}"
    );
    assert!(
        recording.tx.is_empty() && recording.zero_triggers == 0 && recording.delivered_frames == 0
    );
    assert!(!history.exists());
}

#[test]
fn unhomed_normal_enable_refuses_and_disable_remains_available_control() {
    let (_owner, history, mut supervisor, witness) = fixture("unchanged-stop-control");
    assert!(supervisor.set_homing_complete().is_err());
    assert!(supervisor.request_enable(true).is_err());
    assert!(enables(&witness.lock().expect("external witness")).is_empty());
    supervisor
        .request_enable(false)
        .expect("Disable requires no reference");
    let recording = witness.lock().expect("external witness");
    let disables = recording
        .tx
        .iter()
        .filter(|frame| frame.id >> 24 == 4)
        .map(|frame| frame.id)
        .collect::<Vec<_>>();
    assert_eq!(
        disables,
        [0x0400fd01, 0x0400fd02, 0x0400fd03, 0x0400fd04, 0x0400fd05]
    );
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert!(!history.exists());
}
