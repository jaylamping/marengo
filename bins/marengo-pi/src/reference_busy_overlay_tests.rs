//! Candidate behavior regression for official overlay ownership during R2a.
//! Uses actual dispatch, writer and owner; no old-main or physical protocol claim.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use armee_proto::prost::Message;
use armee_proto::{
    actuator_command::Payload, ActionEvent, ActuatorCommand, Envelope, OperatorCommand,
    PersistStatus, TuningChange, TuningTier,
};
use berthier::{ControlLoop, ControlMode, GainOverride, LoopError};
use chappe::Bus;
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{DavoutError, ReferenceCancelReason, ReferencePhase, ReferenceRequest};
use marengo_config::load_control_config_from;

use crate::limit_persist::{ConfigPersistQueue, PersistDrainReport, PersistDrainStatus};
use crate::overlay::{ActuatorOverlay, OverlayError, OverlayOutcome, TOPIC_AUDIT_ACTION};

#[path = "../../../crates/berthier/tests/support/mod.rs"]
mod support;

const TARGET: &str = "right_elbow_pitch";
const BOUND: Duration = Duration::from_secs(2);

fn command(tier: TuningTier, session: &str) -> OperatorCommand {
    OperatorCommand {
        timestamp_ms: 10421,
        session_id: session.into(),
        operator_id: "reference-busy-overlay-proof".into(),
        seq: 7,
        command: Some(ActuatorCommand {
            joint: TARGET.into(),
            payload: Some(Payload::Tuning(TuningChange {
                tier: tier as i32,
                param: if tier == TuningTier::ConfigOverlay {
                    "impedance.kp"
                } else {
                    "kp"
                }
                .into(),
                value: 41.0,
                persist: tier == TuningTier::ConfigOverlay,
            })),
        }),
    }
}

struct Case {
    busy: bool,
    tier: TuningTier,
    result: Result<Vec<OverlayOutcome>, OverlayError>,
    before_policy: String,
    after_policy: String,
    old_kp: f64,
    live_kp: f64,
    disk_kp: f64,
    before_disk: Vec<u8>,
    after_disk: Vec<u8>,
    after_gain: Option<GainOverride>,
    still_reserved: bool,
    dispatch_writes: usize,
    drain: PersistDrainReport,
    actions: Vec<ActionEvent>,
    history_absent: bool,
    resources_removed: bool,
    master_unchanged: bool,
}

fn exercise_overlay(busy: bool, tier: TuningTier) -> Case {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master: Vec<_> = [
        "config/robot.yaml",
        "config/motors.yaml",
        "config/control.yaml",
        "config/homing.yaml",
        "assets/urdf/marengo.urdf",
    ]
    .into_iter()
    .map(|relative| {
        let path = source.join(relative);
        let bytes = std::fs::read(&path).expect("immutable master input");
        (path, bytes)
    })
    .collect();
    let fixture = support::FixtureTree::new("reference-busy-overlay", &source);
    let fixture_path = fixture.path().to_path_buf();
    let config = fixture.path().join("config");
    let control_path = config.join("control.yaml");
    let text = std::fs::read_to_string(&control_path).expect("copied control input");
    assert_eq!(
        text.matches("active_reporting_diagnostics: true").count(),
        1
    );
    std::fs::write(
        &control_path,
        text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("only copied diagnostics disabled before construction");
    let before_disk = std::fs::read(&control_path).expect("actual pre-dispatch disk bytes");
    let history = fixture.path().join("history.yaml");
    let mut ctrl = ControlLoop::from_simulation_with_calibration_record_path(
        fixture.path(),
        SimulationBus::default(),
        &history,
        InitialVirtualReference::Unreferenced,
        200,
        25,
    )
    .expect("closed capable owner without a motion permit");
    // Real inactive intent makes RuntimeMit admission reachable; no motion tick
    // or initial grant is used to populate this existing public gain state.
    ctrl.set_control_mode(ControlMode::Impedance);
    ctrl.apply_gain_override(
        TARGET,
        GainOverride {
            kp: 17.0,
            kd: 1.0,
            ki: 0.0,
            fc: 0.0,
        },
    )
    .expect("finite pre-reservation gain control");
    let owner = ctrl.supervisor_mut();
    owner.bus_mut().clear_trace();
    let handle = busy.then(|| {
        owner
            .begin_reference(ReferenceRequest {
                stamp: owner.reference_snapshot().next_stamp.expect("issued stamp"),
                joint: TARGET.into(),
                confirmed: true,
                sign_verified: true,
            })
            .expect("real reservation performs no writes")
    });
    let before_policy = format!("{:?}", ctrl.supervisor().control);
    let old_kp = ctrl.supervisor().control.control.joints[TARGET]
        .impedance
        .kp;
    let chappe = Arc::new(Bus::new(16));
    let mut actions_rx = chappe.subscribe(TOPIC_AUDIT_ACTION);
    let queue = ConfigPersistQueue::spawn(Arc::clone(&chappe), fixture.path().to_path_buf());
    let mut overlay = ActuatorOverlay::from_config_dir(&config, queue).expect("real allowlist");
    let op = command(tier, if busy { "busy" } else { "healthy" });
    let result = overlay.apply_operator_command(&mut ctrl, &config, &op);
    // Capture official dispatch before cleanup: an automatic or fallback stop
    // cannot manufacture these no-mutation/no-enqueue observations.
    let after_policy = format!("{:?}", ctrl.supervisor().control);
    let live_kp = ctrl.supervisor().control.control.joints[TARGET]
        .impedance
        .kp;
    let after_gain = ctrl.gain_override(TARGET).cloned();
    let snapshot = ctrl.supervisor().reference_snapshot();
    let still_reserved = handle.as_ref().is_some_and(|handle| {
        ctrl.supervisor().reference_busy()
            && snapshot.handle.as_ref() == Some(handle)
            && snapshot.phase == ReferencePhase::BaselineStop
            && !snapshot.reference_armed
            && !snapshot.usable_reference
    });
    let dispatch_writes = ctrl.supervisor().bus().transmissions().len();
    if let Some(handle) = handle {
        ctrl.supervisor_mut()
            .cancel_reference(&handle, ReferenceCancelReason::Operator)
            .expect("bounded real cleanup before writer drain");
    }
    let drain = overlay.close_persist_and_drain(BOUND);
    let after_disk = std::fs::read(&control_path).expect("actual bytes after joined drain");
    let disk_kp = load_control_config_from(&config)
        .expect("actual installed disk reload")
        .control
        .joints[TARGET]
        .impedance
        .kp;
    let mut actions = Vec::new();
    while let Ok(bytes) = actions_rx.try_recv() {
        let envelope = Envelope::decode(bytes.as_slice()).expect("real worker audit envelope");
        actions.push(ActionEvent::decode(envelope.payload.as_slice()).expect("real worker action"));
    }
    let history_absent = !history.try_exists().expect("history resource observation");
    drop(overlay);
    drop(ctrl);
    drop(fixture);
    let resources_removed = !fixture_path
        .try_exists()
        .expect("exclusive fixture cleanup");
    let master_unchanged = master
        .iter()
        .all(|(path, bytes)| std::fs::read(path).expect("master input after cleanup") == *bytes);
    Case {
        busy,
        tier,
        result,
        before_policy,
        after_policy,
        old_kp,
        live_kp,
        disk_kp,
        before_disk,
        after_disk,
        after_gain,
        still_reserved,
        dispatch_writes,
        drain,
        actions,
        history_absent,
        resources_removed,
        master_unchanged,
    }
}

#[test]
fn reserved_reference_refuses_actual_persist_and_runtime_overlay_before_mutation() {
    // The same valid configuration write must reach actual disk + Durable when
    // no reservation exists. Clean every fixture before decisive classification.
    let healthy = exercise_overlay(false, TuningTier::ConfigOverlay);
    let cases = [TuningTier::ConfigOverlay, TuningTier::RuntimeMit]
        .map(|tier| exercise_overlay(true, tier));
    assert!(healthy.resources_removed && healthy.master_unchanged && healthy.history_absent);
    assert_eq!(healthy.dispatch_writes, 0);
    assert_eq!(healthy.drain.status, PersistDrainStatus::Complete);
    assert!(healthy.drain.worker_terminated && !healthy.drain.in_flight);
    assert_eq!(healthy.drain.pending_requests, 0);
    assert_eq!(
        (
            healthy.drain.successful_writes,
            healthy.drain.failed_writes,
            healthy.drain.publication_failures
        ),
        (1, 0, 0)
    );
    assert_ne!(
        healthy.old_kp, 41.0,
        "valid neighbor must actually change the profile"
    );
    assert_eq!((healthy.live_kp, healthy.disk_kp), (41.0, 41.0));
    assert_ne!(healthy.before_disk, healthy.after_disk);
    let healthy_outcomes = healthy
        .result
        .as_ref()
        .expect("outside-busy valid dispatch");
    assert!(healthy_outcomes.iter().any(|outcome| matches!(outcome,
        OverlayOutcome::Action(action) if action.accepted && action.action == "config_persist"
            && action.persist_status == PersistStatus::Pending as i32)));
    assert_eq!(healthy.actions.len(), 1);
    let action = &healthy.actions[0];
    assert_eq!(action.timestamp_ms, 10421);
    assert_eq!(action.session_id, "healthy");
    assert_eq!(action.operator_id, "reference-busy-overlay-proof");
    assert_eq!(action.joint, TARGET);
    assert_eq!(action.action, "config_persist");
    assert!(action.accepted && !action.config_revision.is_empty());
    assert_eq!(action.persist_status, PersistStatus::Durable as i32);
    for case in cases {
        assert!(case.busy);
        assert!(case.resources_removed && case.master_unchanged && case.history_absent);
        assert!(
            case.still_reserved,
            "official refusal must preserve the live reservation"
        );
        assert_eq!(case.dispatch_writes, 0);
        assert_eq!(case.drain.status, PersistDrainStatus::Complete);
        assert!(case.drain.worker_terminated && !case.drain.in_flight);
        assert_eq!(case.drain.pending_requests, 0);
        // Decisive admission assertion precedes preservation checks, so the
        // defective candidate is classified as real acceptance, not setup IO.
        assert!(
            matches!(
                &case.result,
                Err(OverlayError::Controller(LoopError::Safety(
                    DavoutError::ReferenceBusy { .. }
                )))
            ),
            "reserved reference must reject {:?} before official overlay mutation: {:?}",
            case.tier,
            case.result
        );
        assert_eq!(case.before_policy, case.after_policy);
        assert_eq!(case.live_kp, case.old_kp);
        assert_eq!(case.disk_kp, case.old_kp);
        assert_eq!(case.before_disk, case.after_disk);
        assert_eq!(
            case.after_gain
                .as_ref()
                .expect("existing gain is preserved")
                .kp,
            17.0
        );
        assert_eq!(
            (
                case.drain.successful_writes,
                case.drain.failed_writes,
                case.drain.publication_failures,
                case.drain.coalesced_requests
            ),
            (0, 0, 0, 0)
        );
        assert!(
            case.actions.is_empty(),
            "refused overlay enqueued a real durable publication"
        );
    }
}
