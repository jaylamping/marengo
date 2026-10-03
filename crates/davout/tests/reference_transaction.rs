//! Public R2a virtual-acquisition conformance, never a physical grant or old-binary red.
//! Actual finite raw/scripted writes exercise the shared owner, decoder and stop path.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::Duration;

use davout::simulation::{
    InitialVirtualReference, ReferenceProofMode, ReferenceReplyRule, RuleId, SimulationBus,
    SimulationReceive, SimulationTransmission, TxMatcher, TxOccurrence, TxRule,
};
use davout::{
    FaultClass, JointHomingState, MitJointCommand, OperationalMode, ReferenceCancelReason,
    ReferenceCause, ReferenceCommit, ReferenceError, ReferenceFailureKind, ReferenceHandle,
    ReferencePhase, ReferenceRequest, ReferenceSnapshot, ReferenceTerminal, SafetySnapshot,
    StopAction, StopReport, Supervisor,
};
use robstride::{CanFrame, MotorAddress, ReceiveCompletion, ReceivedCanFrame};

#[path = "../../berthier/tests/support/mod.rs"]
mod support;

const TARGET: &str = "right_elbow_pitch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedWire {
    device_id: u8,
    id: u32,
    data: [u8; 8],
}

// Installed master mapping, three attempts per drive in installed order.
// A successful attempt means transport acceptance, never a physical ACK.
const ALL_STOP: [ExpectedWire; 15] = [
    ExpectedWire {
        device_id: 1,
        id: 0x1200_fd01,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 1,
        id: 0x017f_ff01,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 1,
        id: 0x0400_fd01,
        data: [0; 8],
    },
    ExpectedWire {
        device_id: 2,
        id: 0x1200_fd02,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 2,
        id: 0x017f_ff02,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 2,
        id: 0x0400_fd02,
        data: [0; 8],
    },
    ExpectedWire {
        device_id: 3,
        id: 0x1200_fd03,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 3,
        id: 0x017f_ff03,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 3,
        id: 0x0400_fd03,
        data: [0; 8],
    },
    ExpectedWire {
        device_id: 4,
        id: 0x1200_fd04,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 4,
        id: 0x017f_ff04,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 4,
        id: 0x0400_fd04,
        data: [0; 8],
    },
    ExpectedWire {
        device_id: 5,
        id: 0x1200_fd05,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 5,
        id: 0x017f_ff05,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device_id: 5,
        id: 0x0400_fd05,
        data: [0; 8],
    },
];

const TARGET_ENABLE: ExpectedWire = ExpectedWire {
    device_id: 4,
    id: 0x0300_fd04,
    data: [0; 8],
};
const TARGET_SET_ZERO: ExpectedWire = ExpectedWire {
    device_id: 4,
    id: 0x0600_fd04,
    data: [1, 0, 0, 0, 0, 0, 0, 0],
};

// Explicit maintenance oracle, used only in the distinct applied-reporting case.
const REPORTING_OFF: [ExpectedWire; 5] = [
    ExpectedWire {
        device_id: 1,
        id: 0x1800_fd01,
        data: [1, 2, 3, 4, 5, 6, 0, 0],
    },
    ExpectedWire {
        device_id: 2,
        id: 0x1800_fd02,
        data: [1, 2, 3, 4, 5, 6, 0, 0],
    },
    ExpectedWire {
        device_id: 3,
        id: 0x1800_fd03,
        data: [1, 2, 3, 4, 5, 6, 0, 0],
    },
    ExpectedWire {
        device_id: 4,
        id: 0x1800_fd04,
        data: [1, 2, 3, 4, 5, 6, 0, 0],
    },
    ExpectedWire {
        device_id: 5,
        id: 0x1800_fd05,
        data: [1, 2, 3, 4, 5, 6, 0, 0],
    },
];

/// Literal near-zero Run observation; has NO acquisition token or physical proof.
fn untagged_target_run() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0280_04fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

/// Literal peer Run status with one device fault flag, through the real decoder.
fn peer_fault() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0281_02fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedAttempt {
    interface: Option<String>,
    device_id: Option<u8>,
    id: u32,
    data: [u8; 8],
    extended: bool,
    delivered: bool,
}

fn capture(transmissions: &[SimulationTransmission]) -> Vec<CapturedAttempt> {
    transmissions
        .iter()
        .map(|attempt| CapturedAttempt {
            interface: attempt
                .address
                .as_ref()
                .map(|address| address.interface.clone()),
            device_id: attempt.address.as_ref().map(|address| address.device_id),
            id: attempt.frame.id,
            data: attempt.frame.data,
            extended: attempt.frame.extended,
            delivered: attempt.delivered,
        })
        .collect()
}

fn positive_trace() -> Vec<ExpectedWire> {
    let mut expected = ALL_STOP.to_vec();
    expected.extend([TARGET_ENABLE, TARGET_SET_ZERO]);
    expected.extend(ALL_STOP);
    expected
}

/// Exactly match actual attempts, including routes, payload, format and delivery.
/// Pass at most the deliberately failed global trace indices for a failure case.
fn assert_trace(actual: &[CapturedAttempt], expected: &[ExpectedWire], failed_indices: &[usize]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "reference transaction emitted unexpected writes"
    );
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual.interface.as_deref(),
            Some("can0"),
            "route at write {index}"
        );
        assert_eq!(
            actual.device_id,
            Some(expected.device_id),
            "device at write {index}"
        );
        assert_eq!(actual.id, expected.id, "CAN ID at write {index}");
        assert_eq!(actual.data, expected.data, "payload at write {index}");
        assert!(actual.extended, "format at write {index}");
        assert_eq!(
            actual.delivered,
            !failed_indices.contains(&index),
            "delivery at write {index}"
        );
    }
}

fn assert_all_stop_report(report: &StopReport, failed_indices: &[usize]) {
    let expected = [
        (1, StopAction::ZeroSpeed),
        (1, StopAction::NeutralMit),
        (1, StopAction::Disable),
        (2, StopAction::ZeroSpeed),
        (2, StopAction::NeutralMit),
        (2, StopAction::Disable),
        (3, StopAction::ZeroSpeed),
        (3, StopAction::NeutralMit),
        (3, StopAction::Disable),
        (4, StopAction::ZeroSpeed),
        (4, StopAction::NeutralMit),
        (4, StopAction::Disable),
        (5, StopAction::ZeroSpeed),
        (5, StopAction::NeutralMit),
        (5, StopAction::Disable),
    ];
    assert_eq!(
        report.attempts.len(),
        15,
        "cleanup skipped an installed stop attempt"
    );
    for (index, (actual, (device, action))) in report.attempts.iter().zip(expected).enumerate() {
        assert_eq!(actual.address.interface, "can0");
        assert_eq!(actual.address.device_id, device);
        assert_eq!(actual.action, action);
        assert_eq!(actual.error.is_some(), failed_indices.contains(&index));
    }
}

const HISTORY: &str = "joints:\n  - joint: right_elbow_pitch\n    device_id: 4\n    can_interface: can0\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-01T00:00:00Z'\n    config_revision: earlier-fixture\n    operator: historical-operator\n  - joint: right_shoulder_roll\n    device_id: 2\n    can_interface: can0\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-01T00:00:00Z'\n    config_revision: earlier-fixture\n    operator: historical-peer\n";

struct Fixture {
    tree: support::FixtureTree,
    history_path: PathBuf,
    history: Option<Vec<u8>>,
    master: Vec<(PathBuf, Vec<u8>)>,
}

fn fixture(diagnostics: bool, saved_history: bool) -> Fixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master = [
        "config/robot.yaml",
        "config/motors.yaml",
        "config/control.yaml",
        "config/homing.yaml",
        "assets/urdf/marengo.urdf",
    ]
    .into_iter()
    .map(|relative| {
        let path = root.join(relative);
        let bytes = std::fs::read(&path).expect("immutable master fixture input");
        (path, bytes)
    })
    .collect();
    let tree = support::FixtureTree::new("reference-transaction", &root);
    if !diagnostics {
        let path = tree.path().join("config/control.yaml");
        let text = std::fs::read_to_string(&path).expect("copied control");
        assert_eq!(
            text.matches("active_reporting_diagnostics: true").count(),
            1
        );
        std::fs::write(
            path,
            text.replace(
                "active_reporting_diagnostics: true",
                "active_reporting_diagnostics: false",
            ),
        )
        .expect("only copied diagnostics disabled before construction");
    }
    let history_path = tree.path().join("history.yaml");
    let history = saved_history.then(|| HISTORY.as_bytes().to_vec());
    if let Some(bytes) = &history {
        std::fs::write(&history_path, bytes).expect("literal finite historical rows");
    }
    Fixture {
        tree,
        history_path,
        history,
        master,
    }
}

/// Construction writes the first default reporting On; `sync` writes the rest
/// one per interface per control period.
fn settle_reporting(owner: &mut Supervisor<SimulationBus>) {
    for _ in 0..=owner.motors.motors.len() {
        std::thread::sleep(Duration::from_millis(5));
        owner.sync_active_reporting();
    }
}

/// Joints whose type-24 stream the owner turned On, read from literal TX.
/// `settle_reporting` runs before any reference work, so every type-24 frame on
/// the trace is an On written by construction/sync — no Off exists yet.
fn reporting_on_trace(owner: &Supervisor<SimulationBus>) -> [bool; 5] {
    const JOINTS: [&str; 5] = [
        "right_shoulder_pitch",
        "right_shoulder_roll",
        "right_upper_arm_yaw",
        "right_elbow_pitch",
        "right_lower_arm_yaw",
    ];
    JOINTS.map(|joint| {
        let device_id = owner
            .motors
            .motors
            .iter()
            .find(|motor| motor.joint == joint)
            .expect("installed joint")
            .device_id;
        owner.bus().transmissions().iter().any(|tx| {
            tx.frame.id >> 24 == 24
                && tx.frame.id & 0xff == u32::from(device_id)
                && tx.frame.data[6] == 0x01
        })
    })
}

fn owner(fixture: &Fixture) -> Supervisor<SimulationBus> {
    Supervisor::from_simulation(
        fixture.tree.path(),
        SimulationBus::default(),
        InitialVirtualReference::Unreferenced,
    )
    .expect("capable closed owner starts without permission")
}

fn request(owner: &Supervisor<SimulationBus>) -> ReferenceRequest {
    ReferenceRequest {
        stamp: owner
            .reference_snapshot()
            .next_stamp
            .expect("owner-issued stamp"),
        joint: TARGET.into(),
        confirmed: true,
        sign_verified: true,
    }
}

fn matcher(communication_type: u8, device_id: u8) -> TxMatcher {
    TxMatcher {
        communication_type: Some(communication_type),
        device_id: Some(device_id),
        interface: Some("can0".into()),
    }
}

fn enable_reply(owner: &mut Supervisor<SimulationBus>) -> RuleId {
    owner
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: matcher(3, 4),
            occurrence: TxOccurrence::Nth(1),
            receive: vec![SimulationReceive::Received(untagged_target_run())],
            send_error: None,
        })
        .expect("actual selected Enable triggers raw Run observation")
}

fn reference_reply(
    owner: &mut Supervisor<SimulationBus>,
    proof: ReferenceProofMode,
    send_error: Option<&str>,
) -> RuleId {
    owner
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress {
                interface: "can0".into(),
                device_id: 4,
            },
            occurrence: TxOccurrence::Nth(1),
            frame: untagged_target_run(),
            proof,
            send_error: send_error.map(str::to_owned),
        })
        .expect("finite sealed SetZero-triggered raw proof rule")
}

fn peer_run() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0280_02fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

fn steps(
    owner: &mut Supervisor<SimulationBus>,
    handle: &ReferenceHandle,
    count: usize,
) -> Vec<Result<ReferenceSnapshot, ReferenceError>> {
    // A fixed declared number of actual public calls, not a copied lifecycle loop.
    (0..count)
        .map(|_| owner.advance_reference(handle))
        .collect()
}

struct Observation {
    trace: Vec<CapturedAttempt>,
    snapshot: ReferenceSnapshot,
    safety: SafetySnapshot,
    busy: bool,
    mode: OperationalMode,
    states: Vec<(String, JointHomingState)>,
    active: Vec<String>,
    pending: usize,
    history_before_cleanup: Option<Vec<u8>>,
    history_after_cleanup: Option<Vec<u8>>,
    expected_history: Option<Vec<u8>>,
    master_unchanged: bool,
}

#[allow(clippy::panic)]
fn read_history(path: &std::path::Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("fixture history unexpectedly unreadable: {error}"),
    }
}

fn finish(mut owner: Supervisor<SimulationBus>, fixture: Fixture) -> Observation {
    // Automatic outcome/trace are captured BEFORE explicit disposal cleanup.
    let trace = capture(owner.bus().transmissions());
    let snapshot = owner.reference_snapshot();
    let safety = owner.safety_snapshot();
    let busy = owner.reference_busy();
    let mode = owner.mode();
    let states = owner
        .motors
        .motors
        .iter()
        .map(|motor| (motor.joint.clone(), owner.joint_homing_state(&motor.joint)))
        .collect();
    let active = owner.active_joints().iter().cloned().collect();
    let pending = owner.bus().pending_receive_count();
    let history_before_cleanup = read_history(&fixture.history_path);
    let _ = owner.cancel_reference_for_shutdown();
    let history_after_cleanup = read_history(&fixture.history_path);
    let expected_history = fixture.history.clone();
    let master_unchanged = fixture
        .master
        .iter()
        .all(|(path, expected)| std::fs::read(path).expect("master remains readable") == *expected);
    drop(owner);
    drop(fixture);
    Observation {
        trace,
        snapshot,
        safety,
        busy,
        mode,
        states,
        active,
        pending,
        history_before_cleanup,
        history_after_cleanup,
        expected_history,
        master_unchanged,
    }
}

fn assert_resources(observed: &Observation) {
    assert_eq!(observed.history_before_cleanup, observed.expected_history);
    assert_eq!(observed.history_after_cleanup, observed.expected_history);
    assert!(
        observed.master_unchanged,
        "test changed master configuration/model"
    );
}

fn assert_no_grant(observed: &Observation) {
    assert!(!observed.busy);
    assert_eq!(observed.mode, OperationalMode::Disabled);
    assert!(observed.active.is_empty());
    assert!(!observed.snapshot.reference_armed);
    assert!(!observed.snapshot.usable_reference);
    for (joint, state) in &observed.states {
        let expected = if observed.safety.is_latched() {
            JointHomingState::Faulted
        } else {
            JointHomingState::Unhomed
        };
        assert_eq!(*state, expected, "no grant for {joint}");
    }
    assert_resources(observed);
}

fn terminal(observed: &Observation) -> &ReferenceTerminal {
    assert_eq!(observed.snapshot.phase, ReferencePhase::Terminal);
    let terminal = observed
        .snapshot
        .terminal
        .as_ref()
        .expect("actual retained terminal");
    assert_eq!(terminal.commit, ReferenceCommit::Unavailable);
    assert!(!terminal.usable_reference);
    assert_eq!(terminal.joint, TARGET);
    terminal
}

fn assert_phases(
    actual: &[Result<ReferenceSnapshot, ReferenceError>],
    expected: &[ReferencePhase],
) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            actual.as_ref().expect("actual owner advance").phase,
            *expected
        );
    }
}

fn normal_denials(owner: &mut Supervisor<SimulationBus>) -> [bool; 3] {
    [
        owner.set_homing_complete().is_err(),
        owner.enable_targets(&[TARGET.into()]).is_err(),
        owner
            .send_mit_batch(vec![MitJointCommand {
                joint: TARGET.into(),
                kp: 0.0,
                kd: 0.0,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_ff_nm: 0.1,
            }])
            .is_err(),
    ]
}

#[test]
fn virtual_core_stages_only_after_exact_raw_pop_and_never_grants_motion() {
    let fixture = fixture(false, true);
    let mut owner = owner(&fixture);
    owner
        .bus_mut()
        .queue_received(peer_run())
        .expect("finite peer diagnostic");
    let peer_drain = owner.drain_feedback();
    let enable = enable_reply(&mut owner);
    let proof = reference_reply(&mut owner, ReferenceProofMode::CurrentSetZero, None);
    owner.bus_mut().clear_trace();
    let request = request(&owner);
    let handle = owner
        .begin_reference(request.clone())
        .expect("valid reserved reference");
    let initial = owner.reference_snapshot();
    let initial_trace = capture(owner.bus().transmissions());
    let advances = steps(&mut owner, &handle, 4);
    let prezero_target = owner.joint_feedback(TARGET);
    let send = owner.advance_reference(&handle);
    let target_after_attempt = owner.joint_feedback(TARGET);
    let peer_after_attempt = owner.joint_feedback("right_shoulder_roll");
    let proof_pops_before = owner.bus_mut().reference_rule_pop_count(proof);
    let busy_denials = normal_denials(&mut owner);
    let drain_denied = owner.drain_feedback().is_err();
    let proof_pops_after_denied_drain = owner.bus_mut().reference_rule_pop_count(proof);
    let awaited = owner.advance_reference(&handle);
    let after = owner.reference_snapshot().terminal;
    let terminal_denials = normal_denials(&mut owner);
    let retry = owner.begin_reference(request);
    let repeated = owner.advance_reference(&handle);
    let repeated_cancel = owner.cancel_reference(&handle, ReferenceCancelReason::Operator);
    let rule_counts = (
        owner.bus().rule_trigger_count(enable),
        owner.bus_mut().reference_rule_trigger_count(proof),
        owner.bus_mut().reference_rule_pop_count(proof),
    );
    let observed = finish(owner, fixture);

    assert_eq!(peer_drain.expect("real decoded peer"), 1);
    assert_eq!(initial.phase, ReferencePhase::BaselineStop);
    assert_eq!(initial.remaining, Some(Duration::from_secs(2)));
    assert!(!initial.reference_armed);
    assert!(initial_trace.is_empty());
    assert_phases(
        &advances,
        &[
            ReferencePhase::DrainOld,
            ReferencePhase::ArmTarget,
            ReferencePhase::DrainPostArm,
            ReferencePhase::SetZero,
        ],
    );
    assert!(advances[2].as_ref().expect("armed phase").reference_armed);
    let prezero = prezero_target.expect("Enable rule reached real raw decoder");
    assert_eq!(prezero.position_rad, 0.0);
    assert_eq!(prezero.velocity_rad_s, 0.0);
    assert_eq!(
        send.expect("actual SetZero").phase,
        ReferencePhase::AwaitEvidence
    );
    assert!(
        target_after_attempt.is_none(),
        "old coordinate survived attempted SetZero"
    );
    assert!(
        peer_after_attempt.is_some(),
        "SetZero cleared the peer cache"
    );
    assert_eq!(proof_pops_before, 0);
    assert_eq!(busy_denials, [true; 3]);
    assert!(drain_denied, "ordinary drain stole the reserved report");
    assert_eq!(proof_pops_after_denied_drain, 0);
    assert_eq!(
        awaited.expect("matching proof advance").phase,
        ReferencePhase::Terminal
    );
    assert_eq!(
        rule_counts,
        (1, 1, 1),
        "real Enable/SetZero/pop reachability"
    );
    let result = terminal(&observed);
    assert_eq!(result.cause, ReferenceCause::EvidenceStaged);
    assert_eq!(result.stop.failed_writes(), 0);
    let received = result.receive.expect("actual awaited raw report receipt");
    assert_eq!(received.completion, ReceiveCompletion::Idle);
    assert_eq!(received.raw_frames, 1);
    assert_eq!(received.read_attempts, 2);
    assert!(result.reporting.is_empty());
    assert_all_stop_report(&result.stop, &[]);
    assert_eq!(after.as_ref(), Some(result));
    assert_eq!(retry.expect("identical retained retry"), handle);
    assert_eq!(
        repeated.expect("immutable re-advance").terminal.as_ref(),
        Some(result)
    );
    assert_eq!(&repeated_cancel.expect("immutable repeated cancel"), result);
    assert_eq!(terminal_denials, [true; 3]);
    assert_trace(&observed.trace, &positive_trace(), &[]);
    assert!(!observed.safety.is_latched());
    assert_no_grant(&observed);
}

#[test]
fn untagged_or_wrong_correlation_is_diagnostic_only_until_deadline() {
    let mut cases = Vec::new();
    for proof_mode in [
        None,
        Some(ReferenceProofMode::PreviousDeviceEpoch),
        Some(ReferenceProofMode::ForeignRealm),
        Some(ReferenceProofMode::PreviousTransaction),
    ] {
        let fixture = fixture(false, proof_mode.is_some());
        let mut owner = owner(&fixture);
        let cached = if proof_mode.is_none() {
            owner
                .bus_mut()
                .queue_received(untagged_target_run())
                .expect("literal cached zero");
            let drained = owner.drain_feedback();
            Some((drained, owner.joint_feedback(TARGET)))
        } else {
            None
        };
        // The old-transaction selector needs a real earlier reservation AND SetZero.
        let prior = if matches!(proof_mode, Some(ReferenceProofMode::PreviousTransaction)) {
            let earlier = owner
                .begin_reference(request(&owner))
                .expect("real earlier context");
            let prior_steps = steps(&mut owner, &earlier, 5);
            let prior_cancel = owner.cancel_reference(&earlier, ReferenceCancelReason::Operator);
            Some((earlier, prior_steps, prior_cancel))
        } else {
            None
        };
        let enable = enable_reply(&mut owner);
        let rule = match proof_mode {
            Some(proof) => reference_reply(&mut owner, proof, None),
            None => owner
                .bus_mut()
                .add_tx_rule(TxRule {
                    matcher: matcher(6, 4),
                    occurrence: TxOccurrence::Nth(1),
                    receive: vec![SimulationReceive::Received(untagged_target_run())],
                    send_error: None,
                })
                .expect("actual SetZero returns untagged new zero"),
        };
        owner.bus_mut().clear_trace();
        let handle = owner
            .begin_reference(request(&owner))
            .expect("valid capable reservation");
        let advances = steps(&mut owner, &handle, 5);
        let waited = owner.advance_reference(&handle);
        let diagnostic = owner.joint_feedback(TARGET);
        let before_deadline = owner.reference_snapshot();
        let clock = before_deadline
            .remaining
            .map(|remaining| owner.bus_mut().elapse_reference_clock(remaining));
        let expired = owner.advance_reference(&handle);
        let counts = (
            owner.bus().rule_trigger_count(enable),
            owner.bus().rule_trigger_count(rule),
            owner.bus_mut().reference_rule_pop_count(rule),
        );
        let observed = finish(owner, fixture);
        cases.push((
            proof_mode,
            cached,
            prior,
            advances,
            waited,
            diagnostic,
            before_deadline,
            clock,
            expired,
            counts,
            observed,
        ));
    }
    // Every real owner/fixture is disposed before classifying any case.
    for (
        proof,
        cached,
        prior,
        advances,
        waited,
        diagnostic,
        before,
        clock,
        expired,
        counts,
        observed,
    ) in cases
    {
        if let Some((drained, cached)) = cached {
            assert_eq!(drained.expect("actual precommand drain"), 1);
            assert_eq!(cached.expect("visible cached zero").position_rad, 0.0);
        }
        if let Some((earlier, prior_steps, cancelled)) = prior {
            assert_phases(
                &prior_steps,
                &[
                    ReferencePhase::DrainOld,
                    ReferencePhase::ArmTarget,
                    ReferencePhase::DrainPostArm,
                    ReferencePhase::SetZero,
                    ReferencePhase::AwaitEvidence,
                ],
            );
            assert_eq!(
                cancelled.expect("real earlier end").cause,
                ReferenceCause::Cancelled(ReferenceCancelReason::Operator)
            );
            assert_ne!(earlier, terminal(&observed).handle);
        }
        assert_phases(
            &advances,
            &[
                ReferencePhase::DrainOld,
                ReferencePhase::ArmTarget,
                ReferencePhase::DrainPostArm,
                ReferencePhase::SetZero,
                ReferencePhase::AwaitEvidence,
            ],
        );
        assert_eq!(
            waited.expect("bounded diagnostic report").phase,
            ReferencePhase::AwaitEvidence,
            "uncorrelated raw evidence was accepted: {proof:?}"
        );
        assert_eq!(before.phase, ReferencePhase::AwaitEvidence);
        assert!(before.terminal.is_none());
        assert_eq!(
            diagnostic
                .expect("actual decoded diagnostic zero")
                .position_rad,
            0.0
        );
        clock
            .expect("still finite remaining deadline")
            .expect("checked virtual clock");
        assert_eq!(
            expired.expect("deadline advance").phase,
            ReferencePhase::Terminal
        );
        assert_eq!((counts.0, counts.1), (1, 1));
        assert_eq!(counts.2, usize::from(proof.is_some()));
        assert_eq!(terminal(&observed).cause, ReferenceCause::TimedOut);
        assert_all_stop_report(&terminal(&observed).stop, &[]);
        assert_trace(&observed.trace, &positive_trace(), &[]);
        assert!(
            !observed.safety.is_latched(),
            "bad correlation is ignored, not a raw hazard"
        );
        assert_no_grant(&observed);
    }
}

#[test]
fn cancellation_at_reserved_armed_and_sent_phases_is_final() {
    let mut cases = Vec::new();
    for count in [0, 3, 5] {
        let fixture = fixture(false, true);
        let mut owner = owner(&fixture);
        let enable = enable_reply(&mut owner);
        let proof = reference_reply(&mut owner, ReferenceProofMode::CurrentSetZero, None);
        owner.bus_mut().clear_trace();
        let request = request(&owner);
        let handle = owner
            .begin_reference(request.clone())
            .expect("valid reference");
        let advances = steps(&mut owner, &handle, count);
        let before = owner.reference_snapshot();
        let reason = match count {
            0 => ReferenceCancelReason::Operator,
            3 => ReferenceCancelReason::Disable,
            5 => ReferenceCancelReason::Shutdown,
            _ => unreachable!("finite stage table"),
        };
        let (cancel_ok, cancelled) = match count {
            0 => match owner.cancel_reference(&handle, reason) {
                Ok(result) => (true, Some(result)),
                Err(_) => (false, None),
            },
            3 => (
                owner.disable_all().is_ok(),
                owner.reference_snapshot().terminal,
            ),
            5 => {
                let result = owner.cancel_reference_for_shutdown();
                (result.is_some(), result)
            }
            _ => unreachable!("finite stage table"),
        };
        let cancellation_trace = capture(owner.bus().transmissions());
        let pending_after_cancel = owner.bus().pending_receive_count();
        let late = owner.advance_reference(&handle);
        let again = owner.cancel_reference(&handle, ReferenceCancelReason::Shutdown);
        let retry = owner.begin_reference(request);
        let counts = (
            owner.bus().rule_trigger_count(enable),
            owner.bus_mut().reference_rule_trigger_count(proof),
            owner.bus_mut().reference_rule_pop_count(proof),
        );
        let observed = finish(owner, fixture);
        cases.push((
            count,
            advances,
            before,
            reason,
            cancel_ok,
            cancelled,
            cancellation_trace,
            pending_after_cancel,
            late,
            again,
            retry,
            counts,
            handle,
            observed,
        ));
    }
    for (
        count,
        advances,
        before,
        reason,
        cancel_ok,
        cancelled,
        cancellation_trace,
        pending,
        late,
        again,
        retry,
        counts,
        handle,
        observed,
    ) in cases
    {
        let expected_phases = [
            ReferencePhase::DrainOld,
            ReferencePhase::ArmTarget,
            ReferencePhase::DrainPostArm,
            ReferencePhase::SetZero,
            ReferencePhase::AwaitEvidence,
        ];
        assert_phases(&advances, &expected_phases[..count]);
        assert_eq!(
            before.phase,
            match count {
                0 => ReferencePhase::BaselineStop,
                3 => ReferencePhase::DrainPostArm,
                5 => ReferencePhase::AwaitEvidence,
                _ => unreachable!("finite stage table"),
            }
        );
        assert_eq!(before.reference_armed, count > 0);
        let result = terminal(&observed);
        assert!(cancel_ok, "real cancellation entry point failed");
        assert_eq!(result.cause, ReferenceCause::Cancelled(reason));
        assert_eq!(&cancelled.expect("same-call cancellation"), result);
        assert_eq!(
            late.expect("late advance cannot restart").terminal.as_ref(),
            Some(result)
        );
        assert_eq!(
            &again.expect("repeated cancel stays original cause"),
            result
        );
        assert_eq!(retry.expect("retained identical request"), handle);
        let mut expected = Vec::new();
        if count > 0 {
            expected.extend(ALL_STOP);
            expected.push(TARGET_ENABLE);
        }
        if count == 5 {
            expected.push(TARGET_SET_ZERO);
        }
        expected.extend(ALL_STOP);
        assert_trace(&observed.trace, &expected, &[]);
        assert_eq!(
            observed.trace, cancellation_trace,
            "terminal action emitted new writes"
        );
        assert_all_stop_report(&result.stop, &[]);
        assert_eq!(counts, (usize::from(count > 0), usize::from(count == 5), 0));
        assert_eq!(pending, usize::from(count > 0));
        assert!(!observed.safety.is_latched());
        assert_no_grant(&observed);
    }
}

#[test]
fn exact_deadline_wins_a_tied_reply_but_one_nanosecond_before_can_stage() {
    let mut cases = Vec::new();
    for before_deadline in [true, false] {
        let fixture = fixture(false, true);
        let mut owner = owner(&fixture);
        let enable = enable_reply(&mut owner);
        let proof = reference_reply(&mut owner, ReferenceProofMode::CurrentSetZero, None);
        owner.bus_mut().clear_trace();
        let handle = owner
            .begin_reference(request(&owner))
            .expect("valid reference");
        let advances = steps(&mut owner, &handle, 5);
        let waiting = owner.reference_snapshot();
        let remaining = waiting.remaining.expect("effective owner deadline");
        let elapsed = if before_deadline {
            remaining
                .checked_sub(Duration::from_nanos(1))
                .expect("positive configured deadline")
        } else {
            remaining
        };
        let clock = owner.bus_mut().elapse_reference_clock(elapsed);
        let outcome = owner.advance_reference(&handle);
        let repeat = owner.advance_reference(&handle);
        let counts = (
            owner.bus().rule_trigger_count(enable),
            owner.bus_mut().reference_rule_trigger_count(proof),
            owner.bus_mut().reference_rule_pop_count(proof),
        );
        let observed = finish(owner, fixture);
        cases.push((
            before_deadline,
            advances,
            waiting,
            clock,
            outcome,
            repeat,
            counts,
            observed,
        ));
    }
    for (before, advances, waiting, clock, outcome, repeat, counts, observed) in cases {
        assert_phases(
            &advances,
            &[
                ReferencePhase::DrainOld,
                ReferencePhase::ArmTarget,
                ReferencePhase::DrainPostArm,
                ReferencePhase::SetZero,
                ReferencePhase::AwaitEvidence,
            ],
        );
        assert_eq!(waiting.remaining, Some(Duration::from_secs(30)));
        assert!(waiting.reference_armed);
        clock.expect("finite monotonic virtual time");
        assert_eq!(
            outcome.expect("deadline/public proof decision").phase,
            ReferencePhase::Terminal
        );
        let result = terminal(&observed);
        assert_eq!(
            result.cause,
            if before {
                ReferenceCause::EvidenceStaged
            } else {
                ReferenceCause::TimedOut
            },
            "tied reply must lose to the deadline"
        );
        assert_eq!(
            repeat.expect("terminal remains final").terminal.as_ref(),
            Some(result)
        );
        assert_eq!(counts, (1, 1, usize::from(before)));
        assert_eq!(
            observed.pending,
            usize::from(!before),
            "expiry consumed the tied reply"
        );
        assert_trace(&observed.trace, &positive_trace(), &[]);
        assert_all_stop_report(&result.stop, &[]);
        assert_no_grant(&observed);
    }
}

#[test]
fn matching_proof_cannot_hide_a_later_peer_fault_in_the_same_report() {
    let mut cases = Vec::new();
    for saturated in [false, true] {
        let fixture = fixture(false, true);
        let mut owner = owner(&fixture);
        let enable = enable_reply(&mut owner);
        let proof = reference_reply(&mut owner, ReferenceProofMode::CurrentSetZero, None);
        let mut receive = vec![SimulationReceive::Received(peer_fault())];
        if saturated {
            // Proof + Device fault + 63 ignored frames = 65 raw frames. The
            // actual first 64 are consumed; one suffix stays unread, not decoded.
            receive.extend((0..63).map(|_| {
                SimulationReceive::Received(ReceivedCanFrame::full_data(
                    Some("can0".into()),
                    CanFrame {
                        id: 0x1f00_04fd,
                        data: [0; 8],
                        extended: true,
                    },
                ))
            }));
        }
        let fault = owner
            .bus_mut()
            .add_tx_rule(TxRule {
                matcher: matcher(6, 4),
                occurrence: TxOccurrence::Nth(1),
                receive,
                send_error: None,
            })
            .expect("later literal peer fault and optional bounded noise suffix");
        owner.bus_mut().clear_trace();
        let handle = owner
            .begin_reference(request(&owner))
            .expect("valid reference");
        let advances = steps(&mut owner, &handle, 6);
        let counts = (
            owner.bus().rule_trigger_count(enable),
            owner.bus_mut().reference_rule_trigger_count(proof),
            owner.bus_mut().reference_rule_pop_count(proof),
            owner.bus().rule_trigger_count(fault),
        );
        let observed = finish(owner, fixture);
        cases.push((saturated, advances, counts, observed));
    }
    for (saturated, advances, counts, observed) in cases {
        assert_phases(
            &advances,
            &[
                ReferencePhase::DrainOld,
                ReferencePhase::ArmTarget,
                ReferencePhase::DrainPostArm,
                ReferencePhase::SetZero,
                ReferencePhase::AwaitEvidence,
                ReferencePhase::Terminal,
            ],
        );
        assert_eq!(counts, (1, 1, 1, 1));
        let result = terminal(&observed);
        let receive = result.receive.expect("actual final report summary");
        assert_eq!(
            (
                receive.completion,
                receive.raw_frames,
                receive.read_attempts
            ),
            if saturated {
                (ReceiveCompletion::WorkLimit, 64, 64)
            } else {
                (ReceiveCompletion::Idle, 2, 3)
            }
        );
        assert_eq!(observed.pending, usize::from(saturated));
        let first = observed
            .safety
            .first_fault()
            .expect("retained actual peer fault");
        assert_eq!(first.class, FaultClass::Device);
        assert_eq!(first.joint.as_deref(), Some("right_shoulder_roll"));
        assert_eq!(
            first.address.as_ref(),
            Some(&MotorAddress {
                interface: "can0".into(),
                device_id: 2
            })
        );
        assert_eq!(first.device.status_flags, 1);
        if saturated {
            let transport = observed
                .safety
                .faults
                .iter()
                .find(|fault| fault.class == FaultClass::Transport)
                .expect("later incomplete transport evidence is retained separately");
            let incomplete = transport
                .receive
                .first_incomplete
                .as_ref()
                .expect("typed bounded receive evidence");
            assert_eq!(incomplete.completion, ReceiveCompletion::WorkLimit);
            assert_eq!((incomplete.raw_frames, incomplete.read_attempts), (64, 64));
        }
        assert_trace(&observed.trace, &positive_trace(), &[]);
        assert_all_stop_report(&result.stop, &[]);
        assert_no_grant(&observed);
        assert!(
            matches!(
                result.cause,
                ReferenceCause::Failed {
                    kind: ReferenceFailureKind::Hazard,
                    ..
                }
            ),
            "earlier Device fault must remain the terminal cause before later WorkLimit"
        );
    }
}

#[test]
fn uncertain_setzero_and_failed_terminal_stop_keep_the_original_outcome() {
    let mut cases = Vec::new();
    for fail_setzero in [true, false] {
        let fixture = fixture(false, true);
        let mut owner = owner(&fixture);
        let enable = enable_reply(&mut owner);
        let proof = reference_reply(
            &mut owner,
            ReferenceProofMode::CurrentSetZero,
            fail_setzero.then_some("selected SetZero delivery uncertain"),
        );
        let stop_failure = if fail_setzero {
            None
        } else {
            Some(
                owner
                    .bus_mut()
                    .add_tx_rule(TxRule {
                        matcher: matcher(18, 3),
                        occurrence: TxOccurrence::Nth(2),
                        receive: vec![],
                        send_error: Some("terminal motor3 zero-speed failure".into()),
                    })
                    .expect("second zero-speed is terminal, first is baseline"),
            )
        };
        owner.bus_mut().clear_trace();
        let handle = owner
            .begin_reference(request(&owner))
            .expect("valid reference");
        let advances = steps(&mut owner, &handle, 4);
        let decoded_before_zero = owner.joint_feedback(TARGET);
        let send = owner.advance_reference(&handle);
        let target_after_attempt = owner.joint_feedback(TARGET);
        let awaited = owner.advance_reference(&handle);
        let automatic = capture(owner.bus().transmissions());
        let original = owner.reference_snapshot().terminal;
        let first_failed = owner.safety_snapshot().first_failed_stop;
        let later_disable = owner.disable_all();
        let later_terminal = owner.advance_reference(&handle);
        let counts = (
            owner.bus().rule_trigger_count(enable),
            owner.bus_mut().reference_rule_trigger_count(proof),
            owner.bus_mut().reference_rule_pop_count(proof),
            stop_failure.map(|rule| owner.bus().rule_trigger_count(rule)),
        );
        let observed = finish(owner, fixture);
        cases.push((
            fail_setzero,
            advances,
            decoded_before_zero,
            send,
            target_after_attempt,
            awaited,
            automatic,
            original,
            first_failed,
            later_disable,
            later_terminal,
            counts,
            observed,
        ));
    }
    // A distinct uncertain baseline must retain its completed first fifteen
    // attempts. Applied reporting makes a skipped Off phase observable too.
    let baseline_fixture = fixture(true, true);
    let mut baseline_owner = owner(&baseline_fixture);
    settle_reporting(&mut baseline_owner);
    let applied_before_baseline = reporting_on_trace(&baseline_owner);
    let baseline_enable = enable_reply(&mut baseline_owner);
    let baseline_proof = reference_reply(
        &mut baseline_owner,
        ReferenceProofMode::CurrentSetZero,
        None,
    );
    let baseline_failure = baseline_owner
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: matcher(18, 3),
            occurrence: TxOccurrence::Nth(1),
            receive: vec![],
            send_error: Some("baseline motor3 zero-speed failure".into()),
        })
        .expect("first zero-speed is the initial all-address stop");
    baseline_owner.bus_mut().clear_trace();
    let baseline_handle = baseline_owner
        .begin_reference(request(&baseline_owner))
        .expect("valid baseline reservation");
    let baseline_advance = baseline_owner.advance_reference(&baseline_handle);
    let baseline_automatic = capture(baseline_owner.bus().transmissions());
    let baseline_terminal = baseline_owner.reference_snapshot().terminal;
    let baseline_repeat = baseline_owner.advance_reference(&baseline_handle);
    let baseline_cancel =
        baseline_owner.cancel_reference(&baseline_handle, ReferenceCancelReason::Shutdown);
    let baseline_counts = (
        baseline_owner.bus().rule_trigger_count(baseline_failure),
        baseline_owner.bus().rule_trigger_count(baseline_enable),
        baseline_owner
            .bus_mut()
            .reference_rule_trigger_count(baseline_proof),
        baseline_owner
            .bus_mut()
            .reference_rule_pop_count(baseline_proof),
    );
    let baseline_observed = finish(baseline_owner, baseline_fixture);
    for (
        fail_setzero,
        advances,
        decoded,
        send,
        after_attempt,
        awaited,
        automatic,
        original,
        first_failed,
        later_disable,
        later_terminal,
        counts,
        observed,
    ) in cases
    {
        assert_phases(
            &advances,
            &[
                ReferencePhase::DrainOld,
                ReferencePhase::ArmTarget,
                ReferencePhase::DrainPostArm,
                ReferencePhase::SetZero,
            ],
        );
        assert!(
            decoded.is_some(),
            "must invalidate an actually admitted prior coordinate"
        );
        assert!(
            after_attempt.is_none(),
            "attempted SetZero kept old selected scratch"
        );
        assert_eq!(
            send.expect("SetZero boundary").phase,
            if fail_setzero {
                ReferencePhase::Terminal
            } else {
                ReferencePhase::AwaitEvidence
            }
        );
        assert_eq!(
            awaited.expect("failure or evidence terminal").phase,
            ReferencePhase::Terminal
        );
        assert_eq!(counts.0, 1);
        assert_eq!(counts.1, 1);
        assert_eq!(counts.2, usize::from(!fail_setzero));
        let result = terminal(&observed);
        assert_eq!(original.as_ref(), Some(result));
        assert_eq!(
            later_terminal
                .expect("retained terminal after healthy Disable")
                .terminal
                .as_ref(),
            Some(result)
        );
        later_disable.expect("later ordinary stop accepted");
        assert_eq!(
            observed
                .safety
                .last_stop
                .as_ref()
                .expect("later real stop")
                .failed_writes(),
            0
        );
        assert!(
            observed
                .safety
                .last_stop
                .as_ref()
                .expect("later stop")
                .generation
                > result.stop.generation
        );
        if fail_setzero {
            assert!(matches!(
                result.cause,
                ReferenceCause::Failed {
                    kind: ReferenceFailureKind::Delivery,
                    ..
                }
            ));
            assert_trace(&automatic, &positive_trace(), &[16]);
            assert_all_stop_report(&result.stop, &[]);
            assert!(first_failed.is_none());
        } else {
            assert_eq!(result.cause, ReferenceCause::EvidenceStaged);
            assert_eq!(counts.3, Some(1));
            assert_trace(&automatic, &positive_trace(), &[23]);
            assert_all_stop_report(&result.stop, &[6]);
            assert_eq!(
                result.stop.attempts[6].error.as_deref(),
                Some("CAN send failed: terminal motor3 zero-speed failure")
            );
            assert_eq!(first_failed.as_ref(), Some(&result.stop));
            assert_eq!(
                observed.safety.first_failed_stop.as_ref(),
                Some(&result.stop)
            );
        }
        let mut expected = positive_trace();
        expected.extend(ALL_STOP);
        assert_trace(
            &observed.trace,
            &expected,
            if fail_setzero { &[16] } else { &[23] },
        );
        assert_no_grant(&observed);
    }
    assert_eq!(applied_before_baseline, [true; 5]);
    assert_eq!(
        baseline_advance
            .expect("uncertain baseline ends in this call")
            .phase,
        ReferencePhase::Terminal
    );
    assert_eq!(baseline_counts, (1, 0, 0, 0));
    assert_trace(&baseline_automatic, &ALL_STOP, &[6]);
    assert_eq!(baseline_automatic, baseline_observed.trace);
    let result = terminal(&baseline_observed);
    assert!(matches!(
        result.cause,
        ReferenceCause::Failed {
            kind: ReferenceFailureKind::Delivery,
            ..
        }
    ));
    assert!(
        result.reporting.is_empty(),
        "uncertain baseline cannot send Off"
    );
    assert!(
        result.receive.is_none(),
        "uncertain baseline cannot drain or arm"
    );
    assert_all_stop_report(&result.stop, &[6]);
    assert_eq!(
        result.stop.attempts[6].error.as_deref(),
        Some("CAN send failed: baseline motor3 zero-speed failure")
    );
    assert_eq!(baseline_terminal.as_ref(), Some(result));
    assert_eq!(
        baseline_repeat
            .expect("retained baseline outcome")
            .terminal
            .as_ref(),
        Some(result)
    );
    assert_eq!(baseline_cancel.expect("retained cancel outcome"), *result);
    assert_eq!(
        baseline_observed.safety.last_stop.as_ref(),
        Some(&result.stop)
    );
    assert_eq!(
        baseline_observed.safety.first_failed_stop.as_ref(),
        Some(&result.stop)
    );
    assert_no_grant(&baseline_observed);
}

#[test]
fn incomplete_old_receive_terminates_without_a_new_budget_or_target_enable() {
    let fixture = fixture(false, false);
    let mut owner = owner(&fixture);
    owner.bus_mut().clear_trace();
    let handle = owner
        .begin_reference(request(&owner))
        .expect("valid reference");
    let baseline = owner.advance_reference(&handle);
    for _ in 0..65 {
        owner
            .bus_mut()
            .queue_received(ReceivedCanFrame::full_data(
                Some("can0".into()),
                CanFrame {
                    id: 0x1f00_04fd,
                    data: [0; 8],
                    extended: true,
                },
            ))
            .expect("finite ignored raw traffic still consumes global work");
    }
    let incomplete = owner.advance_reference(&handle);
    let automatic = capture(owner.bus().transmissions());
    let pending = owner.bus().pending_receive_count();
    let suffix = owner.bus_mut().drain_raw();
    let observed = finish(owner, fixture);

    assert_eq!(
        baseline.expect("baseline completed").phase,
        ReferencePhase::DrainOld
    );
    assert_eq!(
        incomplete.expect("same-call termination").phase,
        ReferencePhase::Terminal
    );
    let result = terminal(&observed);
    assert!(matches!(
        result.cause,
        ReferenceCause::Failed {
            kind: ReferenceFailureKind::IncompleteReceive,
            ..
        }
    ));
    let receive = result.receive.expect("actual bounded report receipt");
    assert_eq!(receive.completion, ReceiveCompletion::WorkLimit);
    assert_eq!(receive.raw_frames, 64);
    assert_eq!(receive.read_attempts, 64);
    assert_eq!(
        pending, 1,
        "a second fresh drain consumed the unread suffix"
    );
    assert_eq!(suffix.frames.len(), 1);
    assert_eq!(suffix.frames[0].received.frame.id, 0x1f00_04fd);
    assert!(suffix.completion.is_complete());
    let mut expected = ALL_STOP.to_vec();
    expected.extend(ALL_STOP);
    assert_trace(&automatic, &expected, &[]);
    assert_eq!(
        automatic, observed.trace,
        "explicit suffix inspection changed TX"
    );
    assert_all_stop_report(&result.stop, &[]);
    assert_no_grant(&observed);
}

#[test]
fn actual_reporting_off_precedes_flush_and_cannot_interfere_with_reference() {
    let mut cases = Vec::new();
    for off_failure in [false, true] {
        let fixture = fixture(true, true);
        let mut owner = owner(&fixture);
        settle_reporting(&mut owner);
        let applied = reporting_on_trace(&owner);
        let lease = owner.acquire_active_reporting_lease(
            TARGET,
            "reference-fixture",
            "actual-target-lease",
            Duration::from_secs(30),
        );
        let enable = enable_reply(&mut owner);
        let proof = reference_reply(&mut owner, ReferenceProofMode::CurrentSetZero, None);
        let off_rule = if off_failure {
            Some(
                owner
                    .bus_mut()
                    .add_tx_rule(TxRule {
                        matcher: matcher(24, 2),
                        occurrence: TxOccurrence::Nth(1),
                        receive: vec![],
                        send_error: Some("reporting Off delivery uncertain".into()),
                    })
                    .expect("first reporting write after rule installation is Off"),
            )
        } else {
            None
        };
        owner.bus_mut().clear_trace();
        let handle = owner
            .begin_reference(request(&owner))
            .expect("valid reference");
        let initial = owner.reference_snapshot();
        let baseline = owner.advance_reference(&handle);
        let after_baseline = capture(owner.bus().transmissions());
        let mut remaining = Vec::new();
        let mut solicit_results = Vec::new();
        for _ in 0..5 {
            if !off_failure {
                owner.sync_active_reporting();
                solicit_results.push(owner.solicit_status_feedback());
            }
            remaining.push(owner.advance_reference(&handle));
        }
        let counts = (
            owner.bus().rule_trigger_count(enable),
            owner.bus_mut().reference_rule_trigger_count(proof),
            owner.bus_mut().reference_rule_pop_count(proof),
            off_rule.map(|rule| owner.bus().rule_trigger_count(rule)),
        );
        let observed = finish(owner, fixture);
        cases.push((
            off_failure,
            applied,
            lease,
            initial,
            baseline,
            after_baseline,
            remaining,
            solicit_results,
            counts,
            observed,
        ));
    }
    for (
        failed,
        applied,
        lease,
        initial,
        baseline,
        after_baseline,
        remaining,
        solicit_results,
        counts,
        observed,
    ) in cases
    {
        assert_eq!(
            applied, [true; 5],
            "real default reporting must first be applied"
        );
        lease.expect("real public lease admitted");
        assert_eq!(initial.phase, ReferencePhase::BaselineStop);
        let result = terminal(&observed);
        assert_eq!(result.reporting.len(), 5);
        let joints = [
            "right_shoulder_pitch",
            "right_shoulder_roll",
            "right_upper_arm_yaw",
            "right_elbow_pitch",
            "right_lower_arm_yaw",
        ];
        for (index, (attempt, expected)) in result.reporting.iter().zip(REPORTING_OFF).enumerate() {
            assert_eq!(attempt.address.interface, "can0");
            assert_eq!(attempt.address.device_id, expected.device_id);
            assert_eq!(attempt.joint, joints[index]);
            assert_eq!(attempt.error.is_some(), failed && index == 1);
        }
        let mut expected = ALL_STOP.to_vec();
        expected.extend(REPORTING_OFF);
        if failed {
            assert_eq!(
                baseline.expect("Off failure terminal").phase,
                ReferencePhase::Terminal
            );
            assert!(matches!(
                result.cause,
                ReferenceCause::Failed {
                    kind: ReferenceFailureKind::Reporting,
                    ..
                }
            ));
            assert_eq!(counts, (0, 0, 0, Some(1)));
            assert_phases(&remaining, &[ReferencePhase::Terminal; 5]);
            expected.extend(ALL_STOP);
            assert_trace(&after_baseline, &expected, &[16]);
        } else {
            assert_eq!(
                baseline.expect("all reporting Off accepted").phase,
                ReferencePhase::DrainOld
            );
            assert_trace(&after_baseline, &expected, &[]);
            assert_phases(
                &remaining,
                &[
                    ReferencePhase::ArmTarget,
                    ReferencePhase::DrainPostArm,
                    ReferencePhase::SetZero,
                    ReferencePhase::AwaitEvidence,
                    ReferencePhase::Terminal,
                ],
            );
            assert_eq!(result.cause, ReferenceCause::EvidenceStaged);
            assert_eq!(counts, (1, 1, 1, None));
            for result in solicit_results {
                result.expect("busy solicitation is inert");
            }
            expected.extend([TARGET_ENABLE, TARGET_SET_ZERO]);
            expected.extend(ALL_STOP);
        }
        // No filtered type24 allowance: every maintenance and stop attempt is literal.
        assert_trace(&observed.trace, &expected, if failed { &[16] } else { &[] });
        assert_all_stop_report(&result.stop, &[]);
        assert_no_grant(&observed);
    }
}

#[test]
fn ordinary_capability_and_rejected_preflight_cannot_arm_or_consume_a_stamp() {
    let ordinary_fixture = fixture(false, true);
    let mut ordinary =
        Supervisor::from_repo(ordinary_fixture.tree.path(), SimulationBus::default())
            .expect("ordinary constructor even with concrete virtual transport");
    // Ordinary construction honors ambient/runtime configuration. Qualify its
    // startup diagnostics separately, then record every preflight write.
    assert!(ordinary
        .bus()
        .transmissions()
        .iter()
        .all(|tx| tx.frame.id >> 24 == 24));
    ordinary.bus_mut().clear_trace();
    let unsupported = ordinary.begin_reference(request(&ordinary));
    let ordinary_observed = finish(ordinary, ordinary_fixture);

    let fixture = fixture(false, true);
    let mut owner = owner(&fixture);
    owner.bus_mut().clear_trace();
    let valid = request(&owner);
    let mut results = Vec::new();
    for variant in 0..3 {
        let mut invalid = valid.clone();
        match variant {
            0 => invalid.joint = "missing_joint".into(),
            1 => invalid.confirmed = false,
            2 => invalid.sign_verified = false,
            _ => unreachable!("finite preflight table"),
        }
        results.push((
            owner.begin_reference(invalid),
            owner.reference_snapshot(),
            capture(owner.bus().transmissions()),
            owner.has_latched_fault(),
        ));
    }
    let accepted = owner.begin_reference(valid);
    let before_cancel = capture(owner.bus().transmissions());
    let cancelled = accepted
        .as_ref()
        .ok()
        .map(|handle| owner.cancel_reference(handle, ReferenceCancelReason::Operator));
    let observed = finish(owner, fixture);

    assert!(matches!(unsupported, Err(ReferenceError::Unsupported)));
    assert!(ordinary_observed.trace.is_empty());
    assert_eq!(ordinary_observed.snapshot.phase, ReferencePhase::Idle);
    assert!(!ordinary_observed.safety.is_latched());
    assert_no_grant(&ordinary_observed);
    for (result, snapshot, trace, latched) in results {
        assert!(
            matches!(
                result,
                Err(ReferenceError::InvalidRequest { .. }) | Err(ReferenceError::Admission(_))
            ),
            "specific request admission must reject"
        );
        assert_eq!(snapshot.phase, ReferencePhase::Idle);
        assert!(trace.is_empty(), "preflight attempted an actuator write");
        assert!(!latched);
    }
    accepted.expect("same opaque stamp still valid after all rejected requests");
    assert!(before_cancel.is_empty());
    cancelled
        .expect("valid neighbor reserved")
        .expect("valid neighbor cancelled");
    assert_trace(&observed.trace, &ALL_STOP, &[]);
    assert_no_grant(&observed);
}

#[test]
fn an_actually_active_owner_refuses_reference_before_revoking_its_intact_permission() {
    let fixture = fixture(false, false);
    let mut owner = Supervisor::from_simulation(
        fixture.tree.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![TARGET.into()]),
    )
    .expect("INITIAL fixture only qualifies existing Active admission");
    let enable = enable_reply(&mut owner);
    let enabled = owner.enable_targets(&[TARGET.into()]);
    owner
        .bus_mut()
        .queue_received(untagged_target_run())
        .expect("post-enable current-session Run");
    let drained = owner.drain_feedback();
    let output = owner.send_mit_batch(vec![MitJointCommand {
        joint: TARGET.into(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.1,
    }]);
    let positive_trace = capture(owner.bus().transmissions());
    let before_generation = owner.reference_generation();
    let reference = owner.begin_reference(request(&owner));
    let after_mode = owner.mode();
    let after_state = owner.joint_homing_state(TARGET);
    let after_generation = owner.reference_generation();
    let after_trace = capture(owner.bus().transmissions());
    let after_snapshot = owner.reference_snapshot();
    let after_fault = owner.has_latched_fault();
    let enable_count = owner.bus().rule_trigger_count(enable);
    let cleanup = owner.disable_all();
    let observed = finish(owner, fixture);

    enabled.expect("actual scoped Active admission");
    drained.expect("actual current-session raw pose");
    output.expect("real nonneutral output before rejected reference request");
    assert_eq!(enable_count, 1);
    assert!(
        positive_trace.iter().any(|attempt| {
            attempt.interface.as_deref() == Some("can0")
                && attempt.device_id == Some(4)
                && attempt.id >> 24 == 1
                && attempt.data[6..8] != [0x7f, 0xff]
                && attempt.delivered
        }),
        "Active refusal control never reached nonneutral addressed MIT"
    );
    assert!(reference.is_err());
    assert_eq!(after_mode, OperationalMode::Active);
    assert_eq!(after_state, JointHomingState::Verified);
    assert_eq!(after_generation, before_generation);
    assert_eq!(after_snapshot.phase, ReferencePhase::Idle);
    assert_eq!(
        after_trace, positive_trace,
        "rejected reference mutated output"
    );
    assert!(!after_fault);
    cleanup.expect("real normal all-address cleanup");
    assert_resources(&observed);
}

#[test]
fn owner_identity_retry_conflict_staleness_and_eight_outcomes_are_bounded() {
    let fixture_a = fixture(false, false);
    let fixture_b = fixture(false, false);
    let mut owner_a = owner(&fixture_a);
    let mut owner_b = owner(&fixture_b);
    owner_a.bus_mut().clear_trace();
    owner_b.bus_mut().clear_trace();
    let first_request = request(&owner_a);
    let first = owner_a
        .begin_reference(first_request.clone())
        .expect("owner A reservation");
    let request_b = request(&owner_b);
    let foreign_stamp = owner_a.begin_reference(request_b.clone());
    let handle_b = owner_b
        .begin_reference(request_b)
        .expect("owner B real identity");
    let foreign_cancel = owner_a.cancel_reference(&handle_b, ReferenceCancelReason::Operator);
    let foreign_advance = owner_a.advance_reference(&handle_b);
    let still_reserved = owner_a.reference_snapshot();
    let identical_busy = owner_a.begin_reference(first_request.clone());
    let mut conflict = first_request.clone();
    conflict.joint = "right_shoulder_roll".into();
    let conflicting_busy = owner_a.begin_reference(conflict.clone());
    let next_busy_request = request(&owner_a);
    let second_busy = owner_a.begin_reference(next_busy_request.clone());
    let before_cancel = capture(owner_a.bus().transmissions());
    let first_terminal = owner_a.cancel_reference(&first, ReferenceCancelReason::Operator);
    let identical_terminal = owner_a.begin_reference(first_request.clone());
    let conflicting_terminal = owner_a.begin_reference(conflict);
    let retained = owner_a.advance_reference(&first);
    let first_retry_trace = capture(owner_a.bus().transmissions());
    // Busy did not consume the request identity, but actual cancellation changed
    // stop continuity. A stamp issued before that stop cannot authorize a new begin.
    let stale_busy_request = owner_a.begin_reference(next_busy_request);
    let after_stale_busy = capture(owner_a.bus().transmissions());
    let second = owner_a
        .begin_reference(request(&owner_a))
        .expect("fresh stamp after actual cancellation");
    let second_terminal = owner_a.cancel_reference(&second, ReferenceCancelReason::Operator);
    let mut later = Vec::new();
    for _ in 0..7 {
        let request = request(&owner_a);
        let handle = owner_a
            .begin_reference(request)
            .expect("next finite cache entry");
        later.push(owner_a.cancel_reference(&handle, ReferenceCancelReason::Operator));
    }
    let expired_advance = owner_a.advance_reference(&first);
    let expired_cancel = owner_a.cancel_reference(&first, ReferenceCancelReason::Operator);
    let expired_retry = owner_a.begin_reference(first_request);
    let second_retained = owner_a.advance_reference(&second);
    let current = owner_a
        .begin_reference(request(&owner_a))
        .expect("new live reservation");
    let old_cancel = owner_a.cancel_reference(&second, ReferenceCancelReason::Shutdown);
    let still_current = owner_a.reference_snapshot();
    let current_cancel = owner_a.cancel_reference(&current, ReferenceCancelReason::Operator);
    let stale_request = request(&owner_a);
    let ordinary_stop = owner_a.disable_all();
    let stale = owner_a.begin_reference(stale_request);
    let observed_a = finish(owner_a, fixture_a);
    let observed_b = finish(owner_b, fixture_b);

    assert!(matches!(
        foreign_stamp,
        Err(ReferenceError::ForeignIdentity)
    ));
    assert!(matches!(
        foreign_cancel,
        Err(ReferenceError::ForeignIdentity)
    ));
    assert!(matches!(
        foreign_advance,
        Err(ReferenceError::ForeignIdentity)
    ));
    assert_ne!(
        first, handle_b,
        "owner equality must use pointer identity, not Arc<()> value"
    );
    assert_eq!(still_reserved.handle.as_ref(), Some(&first));
    assert_eq!(still_reserved.phase, ReferencePhase::BaselineStop);
    assert_eq!(
        identical_busy.expect("same request remains the same reservation"),
        first
    );
    assert!(matches!(
        conflicting_busy,
        Err(ReferenceError::ConflictingRetry)
    ));
    assert!(matches!(second_busy, Err(ReferenceError::Busy)));
    assert!(before_cancel.is_empty());
    let first_terminal = first_terminal.expect("first actual cancellation");
    assert_eq!(
        identical_terminal.expect("retained retry before stale continuity"),
        first
    );
    assert!(matches!(
        conflicting_terminal,
        Err(ReferenceError::ConflictingRetry)
    ));
    assert_eq!(
        retained.expect("first retained result").terminal.as_ref(),
        Some(&first_terminal)
    );
    assert_trace(&first_retry_trace, &ALL_STOP, &[]);
    assert!(matches!(
        stale_busy_request,
        Err(ReferenceError::StaleStamp)
    ));
    assert_eq!(after_stale_busy, first_retry_trace);
    let second_terminal = second_terminal.expect("second actual cancellation");
    assert_ne!(first, second);
    for result in later {
        result.expect("real bounded terminal entry");
    }
    assert!(matches!(
        expired_advance,
        Err(ReferenceError::OutcomeExpired)
    ));
    assert!(matches!(
        expired_cancel,
        Err(ReferenceError::OutcomeExpired)
    ));
    assert!(matches!(expired_retry, Err(ReferenceError::OutcomeExpired)));
    assert_eq!(
        second_retained
            .expect("newer entry remains retained")
            .terminal
            .as_ref(),
        Some(&second_terminal)
    );
    assert_eq!(
        old_cancel.expect("old retained handle cannot cancel current"),
        second_terminal
    );
    assert_eq!(still_current.handle.as_ref(), Some(&current));
    assert_eq!(still_current.phase, ReferencePhase::BaselineStop);
    current_cancel.expect("actual current cancellation");
    ordinary_stop.expect("successful ordinary Disable changes motion continuity");
    assert!(matches!(stale, Err(ReferenceError::StaleStamp)));
    let mut expected_a = Vec::new();
    for _ in 0..11 {
        expected_a.extend(ALL_STOP);
    }
    assert_trace(&observed_a.trace, &expected_a, &[]);
    // B was still reserved at capture; disposal cleanup cannot manufacture A's oracles.
    assert!(observed_b.trace.is_empty());
    assert!(observed_b.busy);
    assert_resources(&observed_b);
    assert_no_grant(&observed_a);
}
