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
        let mut supervisor = Supervisor::from_simulation(
            tree.path(),
            SimulationBus::default(),
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

use std::time::Duration;

use davout::{DavoutError, ReferenceStageInvalidation, ReferenceStageStatus};

#[derive(Clone, Copy, Debug)]
enum LiveManagement {
    NeighborPatch,
    Restore,
}

impl Owner {
    fn management_attempt(&mut self, kind: LiveManagement) -> Result<(), DavoutError> {
        match kind {
            LiveManagement::NeighborPatch => {
                let patch = marengo_config::limit_patch_from_motor(
                    self.fixture.tree.path().join("config"),
                    "right_shoulder_pitch",
                )
                .expect("valid unchanged neighbor arguments from copied profile");
                self.supervisor.apply_limit_patch(&patch)
            }
            LiveManagement::Restore => self.supervisor.restore_limit_snapshot(
                self.supervisor.motors.clone(),
                self.supervisor.control.clone(),
                self.supervisor.urdf_robot().clone(),
            ),
        }
    }

    fn edit_live_policy(&mut self) {
        self.supervisor
            .control
            .control
            .joints
            .get_mut(TARGET)
            .expect("live selected policy")
            .position_soft_lower_rad = Some(99.0);
    }
}

struct RejectedLiveCase {
    kind: LiveManagement,
    acquired: Acquisition,
    staged_refusal: Result<(), DavoutError>,
    after_restore: ReferenceSnapshot,
    retry: ReferenceSnapshot,
    staged_trace: Vec<SimulationTransmission>,
    fresh: Acquisition,
    staged_resources: Resources,
    initial_states: Vec<JointHomingState>,
    initial_ready: Result<(), DavoutError>,
    initial_refusal: Result<(), DavoutError>,
    restored_states: Vec<JointHomingState>,
    restored_ready: Result<(), DavoutError>,
    initial_trace: Vec<SimulationTransmission>,
    initial_resources: Resources,
}

#[test]
fn rejected_rebuild_observes_live_policy_and_cannot_revive_stage_or_initial_permission() {
    let mut cases = Vec::new();
    for kind in [LiveManagement::NeighborPatch, LiveManagement::Restore] {
        let mut staged = Owner::new("stage-rejected-management");
        let acquired = staged.acquire();
        let original_control = staged.supervisor.control.clone();
        staged.edit_live_policy();
        // No binding getter runs while edited: the management attempt itself
        // must observe the actual live policy before its failed validation.
        let staged_refusal = staged.management_attempt(kind);
        staged.supervisor.control = original_control;
        let after_restore = staged.supervisor.reference_snapshot();
        let retry = staged
            .supervisor
            .advance_reference(&acquired.handle)
            .expect("immutable terminal retry");
        let staged_trace = staged.supervisor.bus().transmissions().to_vec();
        let fresh = staged.acquire();
        let staged_resources = staged.finish();

        let mut initial = Owner::new("initial-rejected-management");
        initial.supervisor = Supervisor::from_simulation(
            initial.fixture.tree.path(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
        )
        .expect("separate declared INITIAL positive fixture");
        let initial_states = initial
            .supervisor
            .motors
            .motors
            .iter()
            .map(|motor| initial.supervisor.joint_homing_state(&motor.joint))
            .collect();
        let initial_ready = initial.supervisor.set_homing_complete();
        let original_control = initial.supervisor.control.clone();
        initial.edit_live_policy();
        let initial_refusal = initial.management_attempt(kind);
        initial.supervisor.control = original_control;
        let restored_states = initial
            .supervisor
            .motors
            .motors
            .iter()
            .map(|motor| initial.supervisor.joint_homing_state(&motor.joint))
            .collect();
        let restored_ready = initial.supervisor.set_homing_complete();
        let initial_trace = initial.supervisor.bus().transmissions().to_vec();
        let initial_resources = initial.finish();
        cases.push(RejectedLiveCase {
            kind,
            acquired,
            staged_refusal,
            after_restore,
            retry,
            staged_trace,
            fresh,
            staged_resources,
            initial_states,
            initial_ready,
            initial_refusal,
            restored_states,
            restored_ready,
            initial_trace,
            initial_resources,
        });
    }
    // All six actual refusal paths, acquisition/INITIAL positive controls and
    // owned-directory cleanup run before the first regression classification.
    for case in &cases {
        assert_acquisition(&case.acquired);
        assert_acquisition(&case.fresh);
        assert_resources(&case.staged_resources);
        assert_resources(&case.initial_resources);
        assert!(
            case.staged_refusal.is_err() && case.initial_refusal.is_err(),
            "{:?} must reject invalid live fields",
            case.kind
        );
        assert!(
            case.initial_ready.is_ok(),
            "actual INITIAL Ready positive control"
        );
        assert_eq!(case.initial_states, vec![JointHomingState::Verified; 5]);
        assert!(case.initial_trace.is_empty());
        assert_wire(&case.staged_trace, &acquisition_wire());
        assert_eq!(
            case.after_restore.terminal.as_ref(),
            Some(&case.acquired.terminal)
        );
        assert_eq!(case.retry.terminal.as_ref(), Some(&case.acquired.terminal));
        assert_eq!(
            case.fresh.snapshot.staged_evidence,
            ReferenceStageStatus::CurrentEvidence
        );
    }
    println!("REBUILD_OBSERVATION_CONTROLS=complete;cleanup_complete=true;cases=6");
    for case in cases {
        assert_eq!(
            case.after_restore.staged_evidence,
            ReferenceStageStatus::Invalidated(ReferenceStageInvalidation::BindingChanged),
            "a rejected management attempt must permanently observe the changed live policy: {:?}",
            case.kind
        );
        assert_eq!(
            case.retry.staged_evidence,
            case.after_restore.staged_evidence
        );
        assert_eq!(case.restored_states, vec![JointHomingState::Unhomed; 5]);
        assert!(
            case.restored_ready.is_err(),
            "INITIAL permission cannot revive after rejected live-policy {:?}",
            case.kind
        );
    }
}

#[test]
fn matched_evidence_is_current_only_after_real_cleanup_and_stays_unusable() {
    let mut owner = Owner::new("stage-matched");
    let before = owner.supervisor.reference_snapshot();
    let acquired = owner.acquire();
    // A later legitimate cache value is not the accepted SetZero proof.
    owner
        .supervisor
        .bus_mut()
        .queue_received(ReceivedCanFrame::full_data(
            Some("can0".into()),
            CanFrame {
                id: 0x0280_04fd,
                data: [0x7e, 0xc6, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
                extended: true,
            },
        ))
        .expect("finite later target diagnostic");
    let drained = owner.supervisor.drain_feedback();
    let later_pose = owner.supervisor.joint_feedback(TARGET);
    let later = owner.supervisor.reference_snapshot();
    let later_trace = owner.supervisor.bus().transmissions().to_vec();
    let resources = owner.finish();

    assert_acquisition(&acquired);
    assert_resources(&resources);
    assert_eq!(before.staged_evidence, ReferenceStageStatus::NoEvidence);
    assert_eq!(
        acquired.steps[5].staged_evidence,
        ReferenceStageStatus::CurrentEvidence
    );
    for step in acquired.steps.iter().take(5) {
        assert_eq!(step.staged_evidence, ReferenceStageStatus::NoEvidence);
    }
    assert_eq!(drained.expect("actual ordinary raw drain"), 1);
    let pose = later_pose.expect("real nonzero target cache");
    assert!((0.119..0.122).contains(&pose.position_rad));
    assert_eq!(later.terminal.as_ref(), Some(&acquired.terminal));
    assert_wire(&later_trace, &acquisition_wire());
    assert!(!later.usable_reference);
    assert_eq!(
        acquired.snapshot.staged_evidence,
        ReferenceStageStatus::CurrentEvidence,
        "only matched evidence after actual cleanup may be current",
    );
    assert_eq!(later.staged_evidence, ReferenceStageStatus::CurrentEvidence);
}

#[derive(Clone, Copy, Debug)]
enum Installation {
    Geometry,
    EquivalentRestore,
    Patch,
}

struct InstalledCase {
    kind: Installation,
    acquired: Acquisition,
    invalid_refused: bool,
    invalid_status: ReferenceStageStatus,
    invalid_model: String,
    invalid_policy: serde_json::Value,
    original_model: String,
    original_policy: serde_json::Value,
    install: Result<(), DavoutError>,
    installed_model: String,
    expected_model: String,
    installed_policy: serde_json::Value,
    after_install: ReferenceSnapshot,
    restored: Result<(), DavoutError>,
    after_restore: ReferenceSnapshot,
    fresh: Option<Acquisition>,
    resources: Resources,
}

#[test]
fn successful_model_installs_invalidate_even_after_restore_and_failed_restore_preserves() {
    let mut cases = Vec::new();
    for kind in [
        Installation::Geometry,
        Installation::EquivalentRestore,
        Installation::Patch,
    ] {
        let mut owner = Owner::new("stage-install");
        let acquired = owner.acquire();
        let original_model = model(&owner.supervisor);
        let original_policy = policy(&owner.supervisor);
        let original = owner.supervisor.urdf_robot().clone();
        let mut replacement = original.clone();
        replacement
            .joints
            .iter_mut()
            .find(|joint| joint.name == TARGET)
            .expect("copied elbow geometry")
            .origin
            .xyz
            .0[0] += 0.01;

        // Invalid typed arguments never install either policy or model.
        let mut invalid_control = owner.supervisor.control.clone();
        invalid_control
            .control
            .joints
            .get_mut(TARGET)
            .expect("control")
            .position_soft_lower_rad = Some(99.0);
        let invalid_refused = owner
            .supervisor
            .restore_limit_snapshot(
                owner.supervisor.motors.clone(),
                invalid_control,
                replacement.clone(),
            )
            .is_err();
        let invalid_status = owner.supervisor.reference_snapshot().staged_evidence;
        let invalid_model = model(&owner.supervisor);
        let invalid_policy = policy(&owner.supervisor);

        let expected_model = if matches!(kind, Installation::Geometry) {
            urdf_rs::write_to_string(&replacement).expect("copied geometry observation")
        } else {
            original_model.clone()
        };
        let install = match kind {
            Installation::Geometry => owner.supervisor.restore_limit_snapshot(
                owner.supervisor.motors.clone(),
                owner.supervisor.control.clone(),
                replacement,
            ),
            Installation::EquivalentRestore => owner.supervisor.restore_limit_snapshot(
                owner.supervisor.motors.clone(),
                owner.supervisor.control.clone(),
                original.clone(),
            ),
            Installation::Patch => {
                let patch = marengo_config::limit_patch_from_motor(
                    owner.fixture.tree.path().join("config"),
                    TARGET,
                )
                .expect("unchanged copied profile bounds/caps");
                owner.supervisor.apply_limit_patch(&patch)
            }
        };
        let installed_model = model(&owner.supervisor);
        let installed_policy = policy(&owner.supervisor);
        let after_install = owner.supervisor.reference_snapshot();
        let restored = owner.supervisor.restore_limit_snapshot(
            owner.supervisor.motors.clone(),
            owner.supervisor.control.clone(),
            original,
        );
        let after_restore = owner.supervisor.reference_snapshot();
        // One real fresh neighbor proves valid geometry and policy remain usable
        // for acquisition; it grants no normal output.
        let fresh = matches!(kind, Installation::Geometry).then(|| owner.acquire());
        let resources = owner.finish();
        cases.push(InstalledCase {
            kind,
            acquired,
            invalid_refused,
            invalid_status,
            invalid_model,
            invalid_policy,
            original_model,
            original_policy,
            install,
            installed_model,
            expected_model,
            installed_policy,
            after_install,
            restored,
            after_restore,
            fresh,
            resources,
        });
    }
    for case in &cases {
        assert_acquisition(&case.acquired);
        assert_resources(&case.resources);
        assert!(
            case.invalid_refused,
            "invalid typed restore accepted for {:?}",
            case.kind
        );
        assert_eq!(case.invalid_model, case.original_model);
        assert_eq!(case.invalid_policy, case.original_policy);
        assert_eq!(case.invalid_status, ReferenceStageStatus::CurrentEvidence);
        case.install
            .as_ref()
            .expect("actual successful installation");
        case.restored.as_ref().expect("actual restoration");
        assert_eq!(case.installed_model, case.expected_model);
        assert_eq!(case.installed_policy, case.original_policy);
        assert_eq!(
            case.after_install.terminal.as_ref(),
            Some(&case.acquired.terminal)
        );
        assert_eq!(
            case.after_restore.terminal.as_ref(),
            Some(&case.acquired.terminal)
        );
        if let Some(fresh) = &case.fresh {
            assert_acquisition(fresh);
            assert_eq!(
                fresh.snapshot.staged_evidence,
                ReferenceStageStatus::CurrentEvidence
            );
        }
    }
    for case in cases {
        assert_eq!(
            case.after_install.staged_evidence,
            ReferenceStageStatus::Invalidated(ReferenceStageInvalidation::BindingChanged),
            "successful {:?} installation must invalidate retained evidence",
            case.kind,
        );
        assert_eq!(
            case.after_restore.staged_evidence,
            ReferenceStageStatus::Invalidated(ReferenceStageInvalidation::BindingChanged),
            "restoring original bytes must not revive an installed stage",
        );
    }
}

#[test]
fn observed_terminal_policy_mismatch_is_sticky_and_old_handle_never_uses_newest_stage() {
    let mut policy_owner = Owner::new("stage-observed-policy");
    let original = policy_owner.acquire();
    policy_owner
        .supervisor
        .control
        .control
        .joints
        .get_mut(TARGET)
        .expect("control")
        .impedance
        .kp = 13.0;
    let edited = policy_owner.supervisor.reference_snapshot();
    policy_owner
        .supervisor
        .control
        .control
        .joints
        .get_mut(TARGET)
        .expect("control")
        .impedance
        .kp = 12.0;
    let restored = policy_owner.supervisor.reference_snapshot();
    let retry = policy_owner
        .supervisor
        .advance_reference(&original.handle)
        .expect("retained retry");
    let fresh = policy_owner.acquire();
    let policy_resources = policy_owner.finish();

    let mut owner = Owner::new("stage-exact-handle");
    let first = owner.acquire();
    owner.supervisor.bus_mut().clear_trace();
    let next_request = owner.request();
    let next = owner
        .supervisor
        .begin_reference(next_request)
        .expect("next real reservation");
    let old_while_busy = owner
        .supervisor
        .advance_reference(&first.handle)
        .expect("old immutable terminal while newer reservation exists");
    let cancelled = owner
        .supervisor
        .cancel_reference(&next, ReferenceCancelReason::Operator)
        .expect("actual newer cleanup");
    let cancelled_trace = owner.supervisor.bus().transmissions().to_vec();
    let newest = owner.acquire();
    let old_again = owner
        .supervisor
        .advance_reference(&first.handle)
        .expect("exact old handle");
    let latest = owner.supervisor.reference_snapshot();
    let latest_trace = owner.supervisor.bus().transmissions().to_vec();
    let resources = owner.finish();

    for acquired in [&original, &fresh, &first, &newest] {
        assert_acquisition(acquired);
    }
    assert_resources(&policy_resources);
    assert_resources(&resources);
    assert_eq!(
        original.snapshot.staged_evidence,
        ReferenceStageStatus::CurrentEvidence
    );
    assert_eq!(
        fresh.snapshot.staged_evidence,
        ReferenceStageStatus::CurrentEvidence
    );
    assert_eq!(
        newest.snapshot.staged_evidence,
        ReferenceStageStatus::CurrentEvidence
    );
    assert_eq!(
        cancelled.cause,
        ReferenceCause::Cancelled(ReferenceCancelReason::Operator)
    );
    assert_wire(&cancelled_trace, &STOP);
    assert_wire(&latest_trace, &acquisition_wire());
    assert_eq!(latest.handle.as_ref(), Some(&newest.handle));
    assert_eq!(
        latest.staged_evidence,
        ReferenceStageStatus::CurrentEvidence
    );
    for observed in [&edited, &restored, &retry] {
        assert_eq!(observed.terminal.as_ref(), Some(&original.terminal));
        assert_eq!(
            observed.staged_evidence,
            ReferenceStageStatus::Invalidated(ReferenceStageInvalidation::BindingChanged),
            "observed terminal policy mismatch must stay invalid after restoration",
        );
    }
    for old in [old_while_busy, old_again] {
        assert_eq!(old.handle.as_ref(), Some(&first.handle));
        assert_eq!(old.terminal.as_ref(), Some(&first.terminal));
        assert_eq!(
            old.staged_evidence,
            ReferenceStageStatus::Invalidated(ReferenceStageInvalidation::Superseded),
            "an old handle must never borrow the newest current stage",
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum EndContinuity {
    Deadline,
    Disable,
    Shutdown,
}

struct EndCase {
    kind: EndContinuity,
    acquired: Acquisition,
    before: ReferenceSnapshot,
    after: ReferenceSnapshot,
    retried: ReferenceSnapshot,
    action_ok: bool,
    trace: Vec<SimulationTransmission>,
    resources: Resources,
}

#[test]
fn original_deadline_disable_and_shutdown_invalidate_without_rewriting_terminal() {
    let mut cases = Vec::new();
    for kind in [
        EndContinuity::Deadline,
        EndContinuity::Disable,
        EndContinuity::Shutdown,
    ] {
        let mut owner = Owner::new("stage-ended");
        let acquired = owner.acquire();
        let (before, action_ok) = match kind {
            EndContinuity::Deadline => {
                owner
                    .supervisor
                    .bus_mut()
                    .elapse_reference_clock(Duration::from_secs(30) - Duration::from_nanos(1))
                    .expect("finite virtual time one nanosecond before original deadline");
                let before = owner
                    .supervisor
                    .advance_reference(&acquired.handle)
                    .expect("retained retry cannot renew the deadline");
                owner
                    .supervisor
                    .bus_mut()
                    .elapse_reference_clock(Duration::from_nanos(1))
                    .expect("exact deadline equality");
                (before, true)
            }
            EndContinuity::Disable => {
                let before = owner.supervisor.reference_snapshot();
                (before, owner.supervisor.disable_all().is_ok())
            }
            EndContinuity::Shutdown => {
                let before = owner.supervisor.reference_snapshot();
                // No live reservation: preserve Option and optional-stop semantics.
                (
                    before,
                    owner.supervisor.cancel_reference_for_shutdown().is_none(),
                )
            }
        };
        let after = owner.supervisor.reference_snapshot();
        let retried = owner
            .supervisor
            .advance_reference(&acquired.handle)
            .expect("same old terminal");
        let trace = owner.supervisor.bus().transmissions().to_vec();
        let resources = owner.finish();
        cases.push(EndCase {
            kind,
            acquired,
            before,
            after,
            retried,
            action_ok,
            trace,
            resources,
        });
    }
    for case in &cases {
        assert_acquisition(&case.acquired);
        assert_resources(&case.resources);
        assert!(case.action_ok, "real {:?} action failed", case.kind);
        assert_eq!(
            case.before.staged_evidence,
            ReferenceStageStatus::CurrentEvidence
        );
        assert_eq!(case.after.terminal.as_ref(), Some(&case.acquired.terminal));
        assert_eq!(
            case.retried.terminal.as_ref(),
            Some(&case.acquired.terminal)
        );
        let mut expected = acquisition_wire();
        if matches!(case.kind, EndContinuity::Disable) {
            expected.extend(STOP);
        }
        assert_wire(&case.trace, &expected);
    }
    for case in cases {
        let reason = match case.kind {
            EndContinuity::Deadline => ReferenceStageInvalidation::DeadlineExpired,
            EndContinuity::Disable => ReferenceStageInvalidation::StopChanged,
            EndContinuity::Shutdown => ReferenceStageInvalidation::Shutdown,
        };
        assert_eq!(
            case.after.staged_evidence,
            ReferenceStageStatus::Invalidated(reason),
            "real {:?} must invalidate staged evidence",
            case.kind,
        );
        assert_eq!(
            case.retried.staged_evidence,
            ReferenceStageStatus::Invalidated(reason)
        );
    }
}
