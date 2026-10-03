//! Real closed-owner acquisitions and immutable model/stamp continuity.
//! No physical reference grant, history write, or copied safety policy.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use davout::simulation::{
    InitialVirtualReference, ReferenceProofMode, ReferenceReplyRule, RuleId, SimulationBus,
    SimulationReceive, SimulationTransmission, TxMatcher, TxOccurrence, TxRule,
};
use davout::{
    JointHomingState, OperationalMode, ReferenceCancelReason, ReferenceCause, ReferenceCommit,
    ReferenceHandle, ReferencePhase, ReferenceRequest, ReferenceSnapshot, ReferenceTerminal,
    Supervisor,
};
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};

#[path = "../../berthier/tests/support/mod.rs"]
mod support;

const TARGET: &str = "right_elbow_pitch";
const HISTORY: &str = "joints:\n  - joint: right_elbow_pitch\n    device_id: 4\n    can_interface: can0\n    method: manual_reference\n    home_offset_rad: 0.0\n    verified_position_rad: 0.0\n    sign_test_passed: true\n    timestamp_utc: '2026-09-01T00:00:00Z'\n    config_revision: historical-fixture\n    operator: historical-operator\n";

#[derive(Clone, Copy)]
struct ExpectedWire {
    device: u8,
    id: u32,
    data: [u8; 8],
}

const STOP: [ExpectedWire; 15] = [
    ExpectedWire {
        device: 1,
        id: 0x1200_fd01,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 1,
        id: 0x017f_ff01,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 1,
        id: 0x0400_fd01,
        data: [0; 8],
    },
    ExpectedWire {
        device: 2,
        id: 0x1200_fd02,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 2,
        id: 0x017f_ff02,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 2,
        id: 0x0400_fd02,
        data: [0; 8],
    },
    ExpectedWire {
        device: 3,
        id: 0x1200_fd03,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 3,
        id: 0x017f_ff03,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 3,
        id: 0x0400_fd03,
        data: [0; 8],
    },
    ExpectedWire {
        device: 4,
        id: 0x1200_fd04,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 4,
        id: 0x017f_ff04,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 4,
        id: 0x0400_fd04,
        data: [0; 8],
    },
    ExpectedWire {
        device: 5,
        id: 0x1200_fd05,
        data: [0x0a, 0x70, 0, 0, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 5,
        id: 0x017f_ff05,
        data: [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0],
    },
    ExpectedWire {
        device: 5,
        id: 0x0400_fd05,
        data: [0; 8],
    },
];

fn raw_zero() -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".into()),
        CanFrame {
            id: 0x0280_04fd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

fn acquisition_wire() -> Vec<ExpectedWire> {
    let mut expected = STOP.to_vec();
    expected.extend([
        ExpectedWire {
            device: 4,
            id: 0x0300_fd04,
            data: [0; 8],
        },
        ExpectedWire {
            device: 4,
            id: 0x0600_fd04,
            data: [1, 0, 0, 0, 0, 0, 0, 0],
        },
    ]);
    expected.extend(STOP);
    expected
}

fn assert_wire(actual: &[SimulationTransmission], expected: &[ExpectedWire]) {
    assert_eq!(actual.len(), expected.len(), "unexpected addressed writes");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual.address.as_ref(),
            Some(&MotorAddress {
                interface: "can0".into(),
                device_id: expected.device
            }),
            "installed route at write {index}",
        );
        assert_eq!(
            actual.frame.id, expected.id,
            "literal CAN ID at write {index}"
        );
        assert_eq!(
            actual.frame.data, expected.data,
            "literal payload at write {index}"
        );
        assert!(actual.frame.extended, "extended frame at write {index}");
        assert!(actual.delivered, "transport acceptance at write {index}");
    }
}

struct Fixture {
    tree: support::FixtureTree,
    history: PathBuf,
    master: Vec<(PathBuf, Vec<u8>)>,
}

struct Owner {
    fixture: Fixture,
    supervisor: Supervisor<SimulationBus>,
    enable: RuleId,
    proof: RuleId,
}

struct Acquisition {
    request: ReferenceRequest,
    handle: ReferenceHandle,
    steps: Vec<ReferenceSnapshot>,
    snapshot: ReferenceSnapshot,
    terminal: ReferenceTerminal,
    trace: Vec<SimulationTransmission>,
    enable_triggers: usize,
    proof_triggers: usize,
    proof_pops: usize,
    denied: bool,
    homing: JointHomingState,
    mode: OperationalMode,
    busy: bool,
}

struct Resources {
    history_before: Vec<u8>,
    history_after: Vec<u8>,
    master_unchanged: bool,
    removed: bool,
}

impl Owner {
    fn new(label: &str) -> Self {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let master = [
            "config/robot.yaml",
            "config/motors.yaml",
            "config/control.yaml",
            "config/homing.yaml",
            "assets/urdf/marengo.urdf",
        ]
        .into_iter()
        .map(|relative| {
            let path = source.join(relative);
            let bytes = std::fs::read(&path).expect("immutable master fixture input");
            (path, bytes)
        })
        .collect();
        let tree = support::fixture_tree_without_diagnostics(label, &source);
        let history = tree.path().join("history.yaml");
        std::fs::write(&history, HISTORY)
            .expect("literal historical row, never current permission");
        let mut supervisor = Supervisor::from_simulation_with_calibration_record_path(
            tree.path(),
            SimulationBus::default(),
            &history,
            InitialVirtualReference::Unreferenced,
        )
        .expect("closed unreferenced owner");
        let enable = supervisor
            .bus_mut()
            .add_tx_rule(TxRule {
                matcher: TxMatcher {
                    communication_type: Some(3),
                    device_id: Some(4),
                    interface: Some("can0".into()),
                },
                occurrence: TxOccurrence::Every,
                receive: vec![SimulationReceive::Received(raw_zero())],
                send_error: None,
            })
            .expect("finite Enable diagnostic effect");
        let proof = supervisor
            .bus_mut()
            .add_reference_reply_rule(ReferenceReplyRule {
                address: MotorAddress {
                    interface: "can0".into(),
                    device_id: 4,
                },
                occurrence: TxOccurrence::Every,
                frame: raw_zero(),
                proof: ReferenceProofMode::CurrentSetZero,
                send_error: None,
            })
            .expect("actual addressed SetZero and exact received pop");
        Self {
            fixture: Fixture {
                tree,
                history,
                master,
            },
            supervisor,
            enable,
            proof,
        }
    }

    fn request(&self) -> ReferenceRequest {
        ReferenceRequest {
            stamp: self
                .supervisor
                .reference_snapshot()
                .next_stamp
                .expect("owner-issued stamp"),
            joint: TARGET.into(),
            confirmed: true,
            sign_verified: true,
        }
    }

    fn acquire(&mut self) -> Acquisition {
        let enable_before = self.supervisor.bus().rule_trigger_count(self.enable);
        let proof_before = self
            .supervisor
            .bus()
            .reference_rule_trigger_count(self.proof);
        let pops_before = self.supervisor.bus().reference_rule_pop_count(self.proof);
        self.supervisor.bus_mut().clear_trace();
        let request = self.request();
        let handle = self
            .supervisor
            .begin_reference(request.clone())
            .expect("actual reservation");
        let steps = (0..6)
            .map(|_| {
                self.supervisor
                    .advance_reference(&handle)
                    .expect("actual public phase advance")
            })
            .collect::<Vec<_>>();
        let terminal = steps
            .last()
            .expect("six actual calls")
            .terminal
            .clone()
            .expect("actual immutable terminal");
        // The denied ordinary Enable must add neither an arm nor a permit.
        let denied = self.supervisor.enable_targets(&[TARGET.into()]).is_err();
        Acquisition {
            request,
            handle,
            steps,
            snapshot: self.supervisor.reference_snapshot(),
            terminal,
            trace: self.supervisor.bus().transmissions().to_vec(),
            enable_triggers: self.supervisor.bus().rule_trigger_count(self.enable) - enable_before,
            proof_triggers: self
                .supervisor
                .bus()
                .reference_rule_trigger_count(self.proof)
                - proof_before,
            proof_pops: self.supervisor.bus().reference_rule_pop_count(self.proof) - pops_before,
            denied,
            homing: self.supervisor.joint_homing_state(TARGET),
            mode: self.supervisor.mode(),
            busy: self.supervisor.reference_busy(),
        }
    }

    fn finish(mut self) -> Resources {
        // All tested snapshots/traces were captured before this disposal call.
        let history_before = std::fs::read(&self.fixture.history).expect("retained history");
        let _ = self.supervisor.cancel_reference_for_shutdown();
        let history_after =
            std::fs::read(&self.fixture.history).expect("retained history after cleanup");
        let master_unchanged =
            self.fixture.master.iter().all(|(path, bytes)| {
                std::fs::read(path).expect("unchanged master input") == *bytes
            });
        let path = self.fixture.tree.path().to_path_buf();
        drop(self.supervisor);
        drop(self.fixture);
        Resources {
            history_before,
            history_after,
            master_unchanged,
            removed: !path.try_exists().expect("fixture cleanup check"),
        }
    }
}

fn assert_acquisition(actual: &Acquisition) {
    assert_eq!(actual.request.joint, TARGET);
    assert!(actual.request.confirmed && actual.request.sign_verified);
    let expected = [
        ReferencePhase::DrainOld,
        ReferencePhase::ArmTarget,
        ReferencePhase::DrainPostArm,
        ReferencePhase::SetZero,
        ReferencePhase::AwaitEvidence,
        ReferencePhase::Terminal,
    ];
    assert_eq!(actual.steps.len(), expected.len());
    for (snapshot, phase) in actual.steps.iter().zip(expected) {
        assert_eq!(snapshot.phase, phase);
        assert_eq!(snapshot.handle.as_ref(), Some(&actual.handle));
        assert!(!snapshot.usable_reference);
    }
    assert_eq!(actual.enable_triggers, 1, "actual Enable effect");
    assert_eq!(actual.proof_triggers, 1, "actual SetZero effect");
    assert_eq!(actual.proof_pops, 1, "actual correlated raw pop");
    assert_eq!(actual.terminal.cause, ReferenceCause::EvidenceStaged);
    assert_eq!(actual.terminal.commit, ReferenceCommit::Unavailable);
    assert!(!actual.terminal.usable_reference);
    assert_eq!(actual.snapshot.terminal.as_ref(), Some(&actual.terminal));
    assert_eq!(actual.terminal.stop.attempts.len(), 15);
    assert_eq!(actual.terminal.stop.failed_writes(), 0);

    let stop_actions = [
        (1, davout::StopAction::ZeroSpeed),
        (1, davout::StopAction::NeutralMit),
        (1, davout::StopAction::Disable),
        (2, davout::StopAction::ZeroSpeed),
        (2, davout::StopAction::NeutralMit),
        (2, davout::StopAction::Disable),
        (3, davout::StopAction::ZeroSpeed),
        (3, davout::StopAction::NeutralMit),
        (3, davout::StopAction::Disable),
        (4, davout::StopAction::ZeroSpeed),
        (4, davout::StopAction::NeutralMit),
        (4, davout::StopAction::Disable),
        (5, davout::StopAction::ZeroSpeed),
        (5, davout::StopAction::NeutralMit),
        (5, davout::StopAction::Disable),
    ];
    for (attempt, (device, action)) in actual.terminal.stop.attempts.iter().zip(stop_actions) {
        assert_eq!(attempt.address.interface, "can0");
        assert_eq!(attempt.address.device_id, device);
        assert_eq!(attempt.action, action);
        assert!(attempt.error.is_none());
    }

    assert_wire(&actual.trace, &acquisition_wire());
    assert_eq!(actual.homing, JointHomingState::Unhomed);
    assert_eq!(actual.mode, OperationalMode::Disabled);
    assert!(actual.denied);
    assert!(!actual.busy);
    assert!(!actual.snapshot.reference_armed);
    assert!(!actual.snapshot.usable_reference);
}

fn assert_resources(actual: &Resources) {
    assert_eq!(actual.history_before, HISTORY.as_bytes());
    assert_eq!(actual.history_after, HISTORY.as_bytes());
    assert!(actual.master_unchanged, "master config/model changed");
    assert!(actual.removed, "exclusive fixture not cleaned up");
}

fn model(supervisor: &Supervisor<SimulationBus>) -> String {
    urdf_rs::write_to_string(supervisor.urdf_robot()).expect("test observation of actual model")
}

fn policy(supervisor: &Supervisor<SimulationBus>) -> serde_json::Value {
    serde_json::to_value((
        &supervisor.motors,
        &supervisor.control,
        &supervisor.homing_config,
    ))
    .expect("test observation of actual installed policy")
}

use davout::{DavoutError, ReferenceError};

#[derive(Debug, Clone, Copy)]
enum Replacement {
    GeometryOnly,
    ByteEquivalent,
}

struct OldPublicCase {
    kind: Replacement,
    initial: Acquisition,
    install: Result<(), DavoutError>,
    expected_model: String,
    installed_model: String,
    policy_before: serde_json::Value,
    policy_after: serde_json::Value,
    stamped_attempt: Result<ReferenceHandle, ReferenceError>,
    rejected_trace: Vec<SimulationTransmission>,
    terminal_before_attempt: ReferenceTerminal,
    state_before_cleanup: JointHomingState,
    fresh: Acquisition,
    resources: Resources,
}

#[test]
fn a_stamp_issued_after_acquisition_must_not_attest_a_later_model_install() {
    let mut cases = Vec::new();
    for kind in [Replacement::GeometryOnly, Replacement::ByteEquivalent] {
        let mut owner = Owner::new("old-public-model-stamp");
        let initial = owner.acquire();
        // This is the critical old-public seam: already revoked authority,
        // then an unused stamp, then a successful typed model installation.
        let old_stamp_request = owner.request();
        let policy_before = policy(&owner.supervisor);
        let mut replacement = owner.supervisor.urdf_robot().clone();
        if matches!(kind, Replacement::GeometryOnly) {
            replacement
                .joints
                .iter_mut()
                .find(|joint| joint.name == TARGET)
                .expect("copied elbow")
                .origin
                .xyz
                .0[0] += 0.01;
        }
        let expected_model =
            urdf_rs::write_to_string(&replacement).expect("copied model observation");
        let install = owner.supervisor.restore_limit_snapshot(
            owner.supervisor.motors.clone(),
            owner.supervisor.control.clone(),
            replacement,
        );
        let installed_model = model(&owner.supervisor);
        let policy_after = policy(&owner.supervisor);
        let terminal_before_attempt = owner
            .supervisor
            .reference_snapshot()
            .terminal
            .expect("original retained terminal");
        owner.supervisor.bus_mut().clear_trace();
        let stamped_attempt = owner.supervisor.begin_reference(old_stamp_request);
        let rejected_trace = owner.supervisor.bus().transmissions().to_vec();
        let state_before_cleanup = owner.supervisor.joint_homing_state(TARGET);
        // The unchanged baseline can wrongly admit the old stamp. Clean that
        // actual reservation before exercising the independently fresh neighbor.
        if let Ok(handle) = &stamped_attempt {
            owner
                .supervisor
                .cancel_reference(handle, ReferenceCancelReason::Operator)
                .expect("cleanup of wrongly admitted baseline reservation");
        }
        let fresh = owner.acquire();
        let resources = owner.finish();
        cases.push(OldPublicCase {
            kind,
            initial,
            install,
            expected_model,
            installed_model,
            policy_before,
            policy_after,
            stamped_attempt,
            rejected_trace,
            terminal_before_attempt,
            state_before_cleanup,
            fresh,
            resources,
        });
    }

    // Every real positive and disposal control completes before the single
    // strengthened existing-public model/stamp contract assertion.
    for case in &cases {
        assert_acquisition(&case.initial);
        assert_acquisition(&case.fresh);
        assert_resources(&case.resources);
        case.install
            .as_ref()
            .expect("actual valid model replacement");
        assert_eq!(case.installed_model, case.expected_model);
        assert_eq!(case.policy_after, case.policy_before);
        assert_eq!(case.terminal_before_attempt, case.initial.terminal);
        assert_eq!(case.state_before_cleanup, JointHomingState::Unhomed);
        assert!(
            case.rejected_trace.is_empty(),
            "begin itself must not transmit"
        );
        println!(
            "OLD_PUBLIC_MODEL_STAMP_CONTROLS kind={:?} initial=EvidenceStaged fresh=EvidenceStaged old_begin={:?} history_unchanged=true resources_removed=true",
            case.kind, case.stamped_attempt,
        );
    }
    println!("STAMP_MODEL_CONTROLS=complete;cleanup_complete=true;cases=2");
    assert!(
        cases
            .iter()
            .all(|case| matches!(case.stamped_attempt, Err(ReferenceError::StaleStamp))),
        "a pre-install stamp must not attest a replaced installed model: {:?}",
        cases
            .iter()
            .map(|case| (&case.kind, &case.stamped_attempt))
            .collect::<Vec<_>>(),
    );
}
