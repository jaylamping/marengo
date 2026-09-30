//! An observed live-policy mismatch stays invalid after public-field restoration.
//! Candidate conformance through actual owner APIs and raw virtual traffic only.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use davout::simulation::{
    InitialVirtualReference, SimulationBus, SimulationReceive, SimulationTransmission, TxMatcher,
    TxOccurrence, TxRule,
};
use davout::{
    DavoutError, JointHomingState, OperationalMode, ReferenceCancelReason, ReferenceCause,
    ReferenceCommit, ReferenceError, ReferenceFailureKind, ReferenceHandle, ReferencePhase,
    ReferenceRequest, ReferenceSnapshot, ReferenceTerminal, StopAction, Supervisor,
};
use marengo_config::{load_robot_config_from, validate_safety_config};
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};

#[path = "../../berthier/tests/support/mod.rs"]
mod support;

const TARGET: &str = "right_elbow_pitch";
const HISTORY: &[u8] = b"joints:\n  - joint: right_elbow_pitch\n    device_id: 4\n    can_interface: can0\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-01T00:00:00Z'\n    config_revision: historical-fixture\n    operator: historical-operator\n";

#[derive(Clone, Copy, Debug)]
enum Stage {
    Reserved,
    Armed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Wire {
    address: Option<MotorAddress>,
    frame: CanFrame,
    delivered: bool,
}

fn capture(trace: &[SimulationTransmission]) -> Vec<Wire> {
    trace
        .iter()
        .map(|tx| Wire {
            address: tx.address.clone(),
            frame: tx.frame.clone(),
            delivered: tx.delivered,
        })
        .collect()
}

fn wire(device: u8, id: u32, data: [u8; 8]) -> Wire {
    Wire {
        address: Some(MotorAddress::new("can0", device)),
        frame: CanFrame {
            id,
            data,
            extended: true,
        },
        delivered: true,
    }
}

// Literal original master routes and inert payloads; no production encoder oracle.
fn all_stop() -> Vec<Wire> {
    [
        (1, [0x1200fd01, 0x017fff01, 0x0400fd01]),
        (2, [0x1200fd02, 0x017fff02, 0x0400fd02]),
        (3, [0x1200fd03, 0x017fff03, 0x0400fd03]),
        (4, [0x1200fd04, 0x017fff04, 0x0400fd04]),
        (5, [0x1200fd05, 0x017fff05, 0x0400fd05]),
    ]
    .into_iter()
    .flat_map(|(device, ids)| {
        ids.into_iter()
            .zip([
                [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
                [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
                [0; 8],
            ])
            .map(move |(id, data)| wire(device, id, data))
    })
    .collect()
}

struct Case {
    stage: Stage,
    handle: ReferenceHandle,
    initial_states: Vec<JointHomingState>,
    begin_trace: Vec<Wire>,
    phases: Vec<ReferencePhase>,
    before_edit: ReferenceSnapshot,
    before_trace: Vec<Wire>,
    before_pending: usize,
    enable_triggers: usize,
    edited_policy_valid: bool,
    edited: ReferenceSnapshot,
    edited_trace: Vec<Wire>,
    advanced: Result<ReferenceSnapshot, ReferenceError>,
    automatic: ReferenceSnapshot,
    automatic_trace: Vec<Wire>,
    automatic_pending: usize,
    automatic_states: Vec<JointHomingState>,
    ready: Result<(), DavoutError>,
    enable: Result<(), DavoutError>,
    mode: OperationalMode,
    active_empty: bool,
    history_before_cleanup: Vec<u8>,
    cleanup: Result<ReferenceTerminal, ReferenceError>,
    after_cleanup_trace: Vec<Wire>,
    history_after_cleanup: Vec<u8>,
    removed: bool,
    master_unchanged: bool,
}

#[test]
fn observed_reserved_or_armed_mapping_mismatch_cannot_resume_after_restoration() {
    let mut cases = Vec::new();
    for stage in [Stage::Reserved, Stage::Armed] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let master: Vec<_> = [
            "config/robot.yaml",
            "config/motors.yaml",
            "config/control.yaml",
            "config/homing.yaml",
            "assets/urdf/marengo.urdf",
        ]
        .into_iter()
        .map(|relative| {
            let path = root.join(relative);
            let bytes = std::fs::read(&path).expect("immutable master input");
            (path, bytes)
        })
        .collect();
        let fixture = support::FixtureTree::new("reference-busy-observed-mismatch", &root);
        let fixture_path = fixture.path().to_path_buf();
        let config = fixture.path().join("config");
        let control_path = config.join("control.yaml");
        let text = std::fs::read_to_string(&control_path).expect("copied control");
        assert_eq!(
            text.matches("active_reporting_diagnostics: true").count(),
            1
        );
        std::fs::write(
            control_path,
            text.replace(
                "active_reporting_diagnostics: true",
                "active_reporting_diagnostics: false",
            ),
        )
        .expect("only copied diagnostics disabled before construction");
        let history = fixture.path().join("history.yaml");
        std::fs::write(&history, HISTORY).expect("inspection-only historical neighbor");
        let robot = load_robot_config_from(&config).expect("actual copied robot policy");
        let mut owner = Supervisor::from_simulation_with_calibration_record_path(
            fixture.path(),
            SimulationBus::default(),
            &history,
            InitialVirtualReference::Unreferenced,
        )
        .expect("closed reference owner without an initial grant");
        let names: Vec<_> = owner
            .motors
            .motors
            .iter()
            .map(|motor| motor.joint.clone())
            .collect();
        let initial_states: Vec<_> = names
            .iter()
            .map(|joint| owner.joint_homing_state(joint))
            .collect();
        let enable_rule = owner
            .bus_mut()
            .add_tx_rule(TxRule {
                matcher: TxMatcher {
                    communication_type: Some(3),
                    device_id: Some(4),
                    interface: Some("can0".into()),
                },
                occurrence: TxOccurrence::Nth(1),
                receive: vec![SimulationReceive::Received(ReceivedCanFrame::full_data(
                    Some("can0".into()),
                    CanFrame {
                        id: 0x028004fd,
                        data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                        extended: true,
                    },
                ))],
                send_error: None,
            })
            .expect("actual target Enable queues a literal raw Run response");
        owner.bus_mut().clear_trace();
        let handle = owner
            .begin_reference(ReferenceRequest {
                stamp: owner.reference_snapshot().next_stamp.expect("issued stamp"),
                joint: TARGET.into(),
                confirmed: true,
                sign_verified: true,
            })
            .expect("actual finite reservation");
        let begin_trace = capture(owner.bus().transmissions());
        let mut phases = Vec::new();
        if matches!(stage, Stage::Armed) {
            for _ in 0..3 {
                phases.push(
                    owner
                        .advance_reference(&handle)
                        .expect("real pre-edit phase")
                        .phase,
                );
            }
        }
        let before_edit = owner.reference_snapshot();
        let before_trace = capture(owner.bus().transmissions());
        let before_pending = owner.bus().pending_receive_count();
        let enable_triggers = owner.bus().rule_trigger_count(enable_rule);
        let motor = owner
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == TARGET)
            .expect("installed target");
        assert_eq!(motor.device_id, 4);
        motor.device_id = 6;
        let edited_policy_valid =
            validate_safety_config(&robot, &owner.motors, &owner.control, &owner.homing_config)
                .is_ok();
        // This is the only binding observation while edited. It must remember
        // the mismatch even though begin already revoked ordinary permission.
        let edited = owner.reference_snapshot();
        let edited_trace = capture(owner.bus().transmissions());
        owner
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == TARGET)
            .expect("same public target")
            .device_id = 4;
        let advanced = owner.advance_reference(&handle);
        let automatic = owner.reference_snapshot();
        let automatic_trace = capture(owner.bus().transmissions());
        let automatic_pending = owner.bus().pending_receive_count();
        let automatic_states: Vec<_> = names
            .iter()
            .map(|joint| owner.joint_homing_state(joint))
            .collect();
        let ready = owner.set_homing_complete();
        let enable = owner.enable_targets(&[TARGET.into()]);
        let mode = owner.mode();
        let active_empty = owner.active_joints().is_empty();
        let history_before_cleanup = std::fs::read(&history).expect("history before fallback");
        // Against a defective candidate this is real fallback cleanup. Against
        // the repair it retries an already retained terminal, adding no writes.
        let cleanup = owner.cancel_reference(&handle, ReferenceCancelReason::Operator);
        let after_cleanup_trace = capture(owner.bus().transmissions());
        let history_after_cleanup = std::fs::read(&history).expect("history after actual cleanup");
        drop(owner);
        drop(fixture);
        let removed = !fixture_path
            .try_exists()
            .expect("exclusive fixture removal");
        let master_unchanged = master
            .iter()
            .all(|(path, bytes)| std::fs::read(path).expect("master after cleanup") == *bytes);
        cases.push(Case {
            stage,
            handle,
            initial_states,
            begin_trace,
            phases,
            before_edit,
            before_trace,
            before_pending,
            enable_triggers,
            edited_policy_valid,
            edited,
            edited_trace,
            advanced,
            automatic,
            automatic_trace,
            automatic_pending,
            automatic_states,
            ready,
            enable,
            mode,
            active_empty,
            history_before_cleanup,
            cleanup,
            after_cleanup_trace,
            history_after_cleanup,
            removed,
            master_unchanged,
        });
    }
    // Both real stages and disposal finish before decisive outcome assertions.
    for case in cases {
        let armed = matches!(case.stage, Stage::Armed);
        assert!(case.removed && case.master_unchanged);
        assert_eq!(case.initial_states, vec![JointHomingState::Unhomed; 5]);
        assert!(case.begin_trace.is_empty());
        assert_eq!(
            case.phases,
            if armed {
                vec![
                    ReferencePhase::DrainOld,
                    ReferencePhase::ArmTarget,
                    ReferencePhase::DrainPostArm,
                ]
            } else {
                vec![]
            }
        );
        let expected_phase = if armed {
            ReferencePhase::DrainPostArm
        } else {
            ReferencePhase::BaselineStop
        };
        assert_eq!(case.before_edit.phase, expected_phase);
        assert_eq!(case.before_edit.reference_armed, armed);
        assert_eq!(case.enable_triggers, usize::from(armed));
        assert_eq!(case.before_pending, usize::from(armed));
        let mut expected_before = if armed { all_stop() } else { Vec::new() };
        if armed {
            expected_before.push(wire(4, 0x0300fd04, [0; 8]));
        }
        assert_eq!(case.before_trace, expected_before);
        assert!(
            case.edited_policy_valid,
            "unused6 is valid scalar/unique mapping, not malformed input"
        );
        assert!(case.edited.next_stamp.is_none());
        assert_eq!(case.edited.phase, expected_phase);
        assert_eq!(case.edited.handle.as_ref(), Some(&case.handle));
        assert_eq!(
            case.edited_trace, case.before_trace,
            "inspection must not transmit"
        );
        let cleanup_report = case.cleanup.expect("actual cleanup before classification");
        assert_eq!(cleanup_report.stop.attempts.len(), 15);
        assert_eq!(cleanup_report.stop.failed_writes(), 0);
        assert_eq!(case.history_before_cleanup, HISTORY);
        assert_eq!(case.history_after_cleanup, HISTORY);
        assert_eq!(
            case.advanced
                .expect("observed mismatch terminates in this advance")
                .phase,
            ReferencePhase::Terminal,
            "observed {:?} mismatch must remain invalid after mapping restoration",
            case.stage
        );
        let terminal = case
            .automatic
            .terminal
            .expect("automatic terminal before fallback");
        assert!(matches!(
            terminal.cause,
            ReferenceCause::Failed {
                kind: ReferenceFailureKind::BindingChanged,
                ..
            }
        ));
        assert_eq!(terminal.handle, case.handle);
        assert_eq!(terminal.commit, ReferenceCommit::Unavailable);
        assert!(!terminal.usable_reference);
        assert_eq!(terminal.stop.generation, if armed { 2 } else { 1 });
        assert_eq!(terminal.stop.attempts.len(), 15);
        assert_eq!(terminal.stop.failed_writes(), 0);
        for ((attempt, expected_wire), expected_action) in
            terminal.stop.attempts.iter().zip(all_stop()).zip(
                [
                    StopAction::ZeroSpeed,
                    StopAction::NeutralMit,
                    StopAction::Disable,
                ]
                .into_iter()
                .cycle(),
            )
        {
            assert_eq!(
                attempt.address,
                expected_wire.address.expect("literal addressed cleanup")
            );
            assert_eq!(attempt.action, expected_action);
            assert!(attempt.error.is_none());
        }
        assert!(terminal
            .receive
            .is_none_or(|receive| receive.raw_frames == 0));
        assert_eq!(
            case.automatic_pending, case.before_pending,
            "mismatch cannot consume the queued post-arm pose"
        );
        let mut expected_automatic = expected_before;
        expected_automatic.extend(all_stop());
        assert_eq!(
            case.automatic_trace, expected_automatic,
            "same-call cleanup must use the original five addresses; no SetZero/new route"
        );
        assert_eq!(
            case.after_cleanup_trace, case.automatic_trace,
            "retained terminal retry must not add another stop"
        );
        assert_eq!(cleanup_report, terminal);
        assert_eq!(case.automatic_states, vec![JointHomingState::Unhomed; 5]);
        assert!(case.ready.is_err() && case.enable.is_err());
        assert_eq!(case.mode, OperationalMode::Disabled);
        assert!(case.active_empty);
    }
}
