//! Candidate-only public controller/virtual reference owner conformance.
//! INITIAL coverage is setup, never acquisition or physical commissioning proof.
#![allow(clippy::expect_used)]

mod support;

use std::path::PathBuf;

use berthier::{ControlLoop, ControlMode, GainOverride};
use davout::simulation::{
    InitialVirtualReference, ReferenceProofMode, ReferenceReplyRule, SimulationBus,
    SimulationTransmission, TxOccurrence,
};
use davout::{
    JointHomingState, OperationalMode, ReferenceCause, ReferenceCommit, ReferenceFailureKind,
    ReferencePhase, ReferenceRequest, ReferenceSnapshot, SafetySnapshot, StopAction,
};
use robstride::{CanFrame, MotorAddress, ReceiveCompletion, ReceivedCanFrame};
use support::FixtureTree;

const TARGET: &str = "right_elbow_pitch";
const ZERO_SPEED: [u8; 8] = [0x0a, 0x70, 0, 0, 0, 0, 0, 0];
const NEUTRAL_MIT: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0];

// Literal master routes/payloads, independent of production frame helpers.
const ALL_STOP: [(u32, [u8; 8]); 15] = [
    (0x1200_fd01, ZERO_SPEED),
    (0x017f_ff01, NEUTRAL_MIT),
    (0x0400_fd01, [0; 8]),
    (0x1200_fd02, ZERO_SPEED),
    (0x017f_ff02, NEUTRAL_MIT),
    (0x0400_fd02, [0; 8]),
    (0x1200_fd03, ZERO_SPEED),
    (0x017f_ff03, NEUTRAL_MIT),
    (0x0400_fd03, [0; 8]),
    (0x1200_fd04, ZERO_SPEED),
    (0x017f_ff04, NEUTRAL_MIT),
    (0x0400_fd04, [0; 8]),
    (0x1200_fd05, ZERO_SPEED),
    (0x017f_ff05, NEUTRAL_MIT),
    (0x0400_fd05, [0; 8]),
];

fn status(id: u32) -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

struct Observed {
    phases: Vec<ReferencePhase>,
    frame_counts: Vec<usize>,
    proof_counts: Vec<(usize, usize)>,
    tick_results: Vec<Result<(), String>>,
    initial_reference: bool,
    old_intent_reached: bool,
    first_tick_inhibited: bool,
    trace: Vec<SimulationTransmission>,
    terminal: ReferenceSnapshot,
    safety: SafetySnapshot,
    final_disabled: bool,
    final_unreferenced: bool,
    reenable_refused: bool,
    later_quiet: bool,
    record_untouched: bool,
}

fn exercise_controller(peer_fault: bool) -> Observed {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = FixtureTree::new("reference-owner-tick", &source);
    let control_path = fixture.path().join("config/control.yaml");
    let control = std::fs::read_to_string(&control_path).expect("copied control policy");
    assert_eq!(
        control
            .matches("active_reporting_diagnostics: true")
            .count(),
        1
    );
    std::fs::write(
        control_path,
        control.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("disable diagnostics only in exclusive fixture before construction");
    let record_path = fixture.path().join("reference-history.yaml");
    let mut ctrl = ControlLoop::from_simulation_with_calibration_record_path(
        fixture.path(),
        SimulationBus::default(),
        &record_path,
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("real controller with an INITIAL virtual condition and isolated history");
    let initial_reference =
        ctrl.supervisor().joint_homing_state(TARGET) == JointHomingState::Verified;
    if peer_fault {
        ctrl.set_control_mode(ControlMode::Impedance);
        ctrl.apply_gain_override(
            TARGET,
            GainOverride {
                kp: 8.0,
                kd: 1.25,
                ki: 0.0,
                fc: 0.0,
            },
        )
        .expect("retain actual public gain intent while operationally Disabled");
    } else {
        ctrl.set_torque_cmd(TARGET, 0.25)
            .expect("retain actual public nonzero torque intent");
    }
    let old_intent_reached = if peer_fault {
        ctrl.control_mode() == ControlMode::Impedance && ctrl.gain_override(TARGET).is_some()
    } else {
        ctrl.control_mode() == ControlMode::TorqueOnly && ctrl.torque_cmd(TARGET) == 0.25
    };
    let rule = ctrl
        .supervisor_mut()
        .bus_mut()
        .add_reference_reply_rule(ReferenceReplyRule {
            address: MotorAddress::new("can0", 4),
            occurrence: TxOccurrence::Nth(1),
            frame: status(0x0280_04fd),
            proof: ReferenceProofMode::CurrentSetZero,
            send_error: None,
        })
        .expect("finite actual SetZero-triggered raw reply");
    let stamp = ctrl
        .supervisor()
        .reference_snapshot()
        .next_stamp
        .expect("owner-issued request stamp");
    ctrl.supervisor_mut()
        .begin_reference(ReferenceRequest {
            stamp,
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        })
        .expect("reserve without TX; revoke INITIAL permission");

    let mut phases = vec![ctrl.supervisor().reference_snapshot().phase];
    let mut frame_counts = Vec::new();
    let mut proof_counts = Vec::new();
    let mut tick_results = Vec::new();
    let mut first_tick_inhibited = false;
    for index in 0..6 {
        // The real SetZero has already queued its tagged reply. A later peer
        // fault in that same report must win before the owner can stage it.
        if index == 5 && peer_fault {
            ctrl.supervisor_mut()
                .bus_mut()
                .queue_received(status(0x0281_02fd))
                .expect("literal ordered peer fault after actual target reply");
        }
        let result = ctrl.tick(None).map_err(|error| error.to_string());
        tick_results.push(result);
        phases.push(ctrl.supervisor().reference_snapshot().phase);
        frame_counts.push(ctrl.supervisor().bus().transmissions().len());
        proof_counts.push((
            ctrl.supervisor().bus().reference_rule_trigger_count(rule),
            ctrl.supervisor().bus().reference_rule_pop_count(rule),
        ));
        if index == 0 {
            first_tick_inhibited = ctrl.control_mode() == ControlMode::Disabled
                && ctrl.torque_cmd(TARGET) == 0.0
                && ctrl.gain_override(TARGET).is_none();
        }
    }
    let terminal = ctrl.supervisor().reference_snapshot();
    let safety = ctrl.supervisor().safety_snapshot();
    let trace = ctrl.supervisor().bus().transmissions().to_vec();
    let final_disabled = ctrl.control_mode() == ControlMode::Disabled
        && ctrl.supervisor().mode() == OperationalMode::Disabled;
    let final_unreferenced = ctrl
        .joint_names()
        .iter()
        .all(|joint| ctrl.supervisor().joint_homing_state(joint) != JointHomingState::Verified);
    let reenable_refused = ctrl
        .supervisor_mut()
        .enable_targets(&[TARGET.into()])
        .is_err();
    let before_later_ticks = ctrl.supervisor().bus().transmissions().len();
    let mut later_quiet = true;
    for _ in 0..3 {
        later_quiet &= ctrl.tick(None).is_ok();
    }
    later_quiet &= ctrl.supervisor().bus().transmissions().len() == before_later_ticks
        && ctrl.control_mode() == ControlMode::Disabled
        && ctrl.torque_cmd(TARGET) == 0.0
        && ctrl.gain_override(TARGET).is_none();
    let record_untouched = !record_path.exists();
    // Bound cleanup even if a decisive assertion will reveal a failed phase.
    // Captured trace/outcome precede this fallback and cannot be masked by it.
    let _ = ctrl.supervisor_mut().cancel_reference_for_shutdown();
    drop(ctrl);
    drop(fixture);
    Observed {
        phases,
        frame_counts,
        proof_counts,
        tick_results,
        initial_reference,
        old_intent_reached,
        first_tick_inhibited,
        trace,
        terminal,
        safety,
        final_disabled,
        final_unreferenced,
        reenable_refused,
        later_quiet,
        record_untouched,
    }
}

fn assert_complete_controller_contract(observed: &Observed) {
    assert!(
        observed.initial_reference && observed.old_intent_reached,
        "intent setup was never reached"
    );
    assert!(
        observed.first_tick_inhibited,
        "busy tick retained controller intent"
    );
    assert!(
        observed.tick_results.iter().all(Result::is_ok),
        "busy ticks: {:?}",
        observed.tick_results
    );
    assert_eq!(
        observed.phases,
        [
            ReferencePhase::BaselineStop,
            ReferencePhase::DrainOld,
            ReferencePhase::ArmTarget,
            ReferencePhase::DrainPostArm,
            ReferencePhase::SetZero,
            ReferencePhase::AwaitEvidence,
            ReferencePhase::Terminal
        ]
    );
    assert_eq!(observed.frame_counts, [15, 15, 16, 16, 17, 32]);
    assert_eq!(
        observed.proof_counts,
        [(0, 0), (0, 0), (0, 0), (0, 0), (1, 0), (1, 1)]
    );
    let mut expected = ALL_STOP.to_vec();
    expected.extend([
        (0x0300_fd04, [0; 8]),
        (0x0600_fd04, [1, 0, 0, 0, 0, 0, 0, 0]),
    ]);
    expected.extend(ALL_STOP);
    assert_eq!(observed.trace.len(), 32);
    for (actual, (id, data)) in observed.trace.iter().zip(expected) {
        let address = actual
            .address
            .as_ref()
            .expect("every write is actually addressed");
        assert_eq!(address.interface, "can0");
        assert_eq!(u32::from(address.device_id), id & 0xff);
        assert_eq!((actual.frame.id, actual.frame.data), (id, data));
        assert!(actual.frame.extended && actual.delivered);
    }
    let terminal = observed
        .terminal
        .terminal
        .as_ref()
        .expect("immutable actual terminal");
    assert_eq!(terminal.commit, ReferenceCommit::Unavailable);
    assert!(!terminal.usable_reference && !observed.terminal.usable_reference);
    assert!(!observed.terminal.reference_armed);
    assert_eq!(terminal.stop.attempts.len(), 15);
    assert_eq!(terminal.stop.failed_writes(), 0);
    for (index, attempt) in terminal.stop.attempts.iter().enumerate() {
        assert_eq!(
            attempt.address,
            MotorAddress::new("can0", (index / 3 + 1) as u8)
        );
        assert_eq!(
            attempt.action,
            [
                StopAction::ZeroSpeed,
                StopAction::NeutralMit,
                StopAction::Disable
            ][index % 3]
        );
        assert!(attempt.error.is_none());
    }
    assert_eq!(observed.safety.last_stop.as_ref(), Some(&terminal.stop));
    assert!(observed.final_disabled && observed.final_unreferenced && observed.reenable_refused);
    assert!(
        observed.later_quiet,
        "terminal tick resumed intent or repeated stop writes"
    );
    assert!(
        observed.record_untouched,
        "acquisition published historical permission"
    );
}

#[test]
fn busy_ticks_own_each_reference_phase_and_inhibit_retained_torque() {
    let observed = exercise_controller(false);
    assert_complete_controller_contract(&observed);
    assert_eq!(
        observed.terminal.terminal.as_ref().expect("terminal").cause,
        ReferenceCause::EvidenceStaged
    );
    assert!(!observed.safety.is_latched());
    let received = observed
        .terminal
        .receive
        .expect("actual final report accounting");
    assert_eq!(received.completion, ReceiveCompletion::Idle);
    assert_eq!((received.raw_frames, received.read_attempts), (1, 2));
}

#[test]
fn busy_tick_returns_ok_after_peer_fault_and_same_call_cleanup() {
    let observed = exercise_controller(true);
    assert_complete_controller_contract(&observed);
    assert!(matches!(
        &observed.terminal.terminal.as_ref().expect("terminal").cause,
        ReferenceCause::Failed {
            kind: ReferenceFailureKind::Hazard,
            ..
        }
    ));
    let fault = observed
        .safety
        .first_fault()
        .expect("actual peer fault retained");
    assert_eq!(fault.joint.as_deref(), Some("right_shoulder_roll"));
    assert_eq!(fault.address.as_ref(), Some(&MotorAddress::new("can0", 2)));
    let received = observed
        .terminal
        .receive
        .expect("whole fault-containing report accounting");
    assert_eq!(received.completion, ReceiveCompletion::Idle);
    assert_eq!((received.raw_frames, received.read_attempts), (2, 3));
}
