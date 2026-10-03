//! Motion ownership, reference-queue gating and lag handling through the real
//! drain functions (no hardware: SimulationBus / MemoryBus only).
#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use armee_proto::prost::Message;
use armee_proto::{
    ActionEvent, ControlMode as ProtoControlMode, EnableRequest, Envelope, MitCommandBatch,
    MitJointCommand, SetZeroRequest,
};
use berthier::{ControlLoop, ControlMode};
use chappe::Bus;
use davout::simulation::SimulationBus;

use crate::enable_gate::{EnableGate, GateLine};
use crate::enable_gate_tests::{enabled_without_pose, first_session_status, JOINT};
use crate::motion_owner::{CommandSource, MotionLease, MOTION_REFUSED_ACTION};
use crate::{
    admit_stdin_command, dispatch_stdin_command, drain_chappe_commands, drain_testing_commands,
    PiCommand, PiReferenceQueue,
};

const STDIN_OWNS: MotionLease = MotionLease::new(CommandSource::Stdin);
const CHAPPE_OWNS: MotionLease = MotionLease::new(CommandSource::Chappe);

type Rx = tokio::sync::broadcast::Receiver<Vec<u8>>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Channels {
    enable: Rx,
    homing: Rx,
    set_zero: Rx,
    lease: Rx,
    status_poll: Rx,
    testing: Rx,
    audit: Rx,
}

fn channels(bus: &Bus) -> Channels {
    Channels {
        enable: bus.subscribe("robot/enable"),
        homing: bus.subscribe("robot/homing"),
        set_zero: bus.subscribe("robot/set_zero"),
        lease: bus.subscribe("robot/active_reporting_lease"),
        status_poll: bus.subscribe("robot/motor_status_poll"),
        testing: bus.subscribe("robot/testing/mit_command_batch"),
        audit: bus.subscribe("robot/audit/action"),
    }
}

fn drain_chappe(
    loop_ctrl: &mut ControlLoop<SimulationBus>,
    queue: &mut PiReferenceQueue,
    lease: MotionLease,
    bus: &Bus,
    rx: &mut Channels,
) {
    drain_chappe_commands(
        loop_ctrl,
        queue,
        lease,
        bus,
        &mut rx.enable,
        &mut rx.homing,
        &mut rx.set_zero,
        &mut rx.lease,
        &mut rx.status_poll,
        &AtomicBool::new(false),
    );
}

fn drain_testing(
    loop_ctrl: &mut ControlLoop<SimulationBus>,
    queue: &PiReferenceQueue,
    lease: MotionLease,
    bus: &Bus,
    rx: &mut Channels,
) {
    drain_testing_commands(
        loop_ctrl,
        queue,
        lease,
        bus,
        &mut rx.testing,
        &AtomicBool::new(false),
    );
}

fn audit_events(rx: &mut Rx) -> Vec<ActionEvent> {
    let mut events = Vec::new();
    while let Ok(bytes) = rx.try_recv() {
        let envelope = Envelope::decode(bytes.as_slice()).expect("audit envelope");
        events.push(ActionEvent::decode(envelope.payload.as_slice()).expect("ActionEvent"));
    }
    events
}

fn refusals(events: &[ActionEvent]) -> Vec<&ActionEvent> {
    events
        .iter()
        .filter(|event| event.action == MOTION_REFUSED_ACTION)
        .collect()
}

fn publish_enable(bus: &Bus, enable: bool) {
    bus.publish(
        "robot/enable",
        "test",
        "marengo.v1.EnableRequest",
        &EnableRequest {
            timestamp_ms: 1,
            operator_id: "consul-tab".into(),
            enable,
        },
    )
    .expect("publish enable");
}

fn gain_batch(mode: ProtoControlMode, position: f64, kp: f64) -> MitCommandBatch {
    MitCommandBatch {
        timestamp_ms: 1,
        mode: mode as i32,
        joints: vec![MitJointCommand {
            name: JOINT.into(),
            kp,
            kd: 1.0,
            position,
            ..MitJointCommand::default()
        }],
    }
}

fn publish_batch(bus: &Bus, batch: &MitCommandBatch) {
    bus.publish(
        "robot/testing/mit_command_batch",
        "test",
        "marengo.v1.MitCommandBatch",
        batch,
    )
    .expect("publish testing batch");
}

/// Idle-queue loop in Impedance, where a Testing IMPEDANCE batch with gains
/// has an observable effect (the gain override) and needs no enable.
fn impedance_loop() -> ControlLoop<SimulationBus> {
    let mut loop_ctrl = enabled_without_pose();
    first_session_status(&mut loop_ctrl);
    loop_ctrl.set_control_mode(ControlMode::Impedance);
    loop_ctrl
}

fn idle_queue() -> PiReferenceQueue {
    PiReferenceQueue::new("marengo-pi-test".into())
}

fn busy_queue() -> PiReferenceQueue {
    let mut queue = idle_queue();
    queue
        .admit(&[JOINT.to_string()], true, "bench", |_| true)
        .expect("admit a reference");
    assert!(queue.is_busy());
    queue
}

// ---- non-owner refusal ------------------------------------------------

#[test]
fn testing_batch_from_non_owner_chappe_is_refused_and_published() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = impedance_loop();
    publish_batch(&bus, &gain_batch(ProtoControlMode::Impedance, 0.0, 12.0));
    drain_testing(&mut loop_ctrl, &idle_queue(), STDIN_OWNS, &bus, &mut rx);

    assert!(
        loop_ctrl.gain_override(JOINT).is_none(),
        "a refused batch must not touch the controller"
    );
    let events = audit_events(&mut rx.audit);
    let refused = refusals(&events);
    assert_eq!(refused.len(), 1, "{events:?}");
    assert!(!refused[0].accepted);
    assert!(
        refused[0].reject_reason.contains("owned by stdin"),
        "{}",
        refused[0].reject_reason
    );
}

#[test]
fn testing_batch_from_owning_chappe_is_applied() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = impedance_loop();
    publish_batch(&bus, &gain_batch(ProtoControlMode::Impedance, 0.0, 12.0));
    drain_testing(&mut loop_ctrl, &idle_queue(), CHAPPE_OWNS, &bus, &mut rx);

    let gain = loop_ctrl.gain_override(JOINT).expect("owner batch applied");
    assert_eq!(gain.kp, 12.0);
    assert!(refusals(&audit_events(&mut rx.audit)).is_empty());
}

#[test]
fn chappe_enable_and_set_zero_from_non_owner_are_refused() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = enabled_without_pose();
    let mut queue = idle_queue();
    publish_enable(&bus, true);
    bus.publish(
        "robot/set_zero",
        "test",
        "marengo.v1.SetZeroRequest",
        &SetZeroRequest {
            timestamp_ms: 1,
            operator_id: "consul-tab".into(),
            joint: JOINT.into(),
            confirm: true,
            sign_test_passed: true,
        },
    )
    .expect("publish set_zero");
    drain_chappe(&mut loop_ctrl, &mut queue, STDIN_OWNS, &bus, &mut rx);

    assert!(!queue.is_busy(), "refused set-zero must queue nothing");
    let events = audit_events(&mut rx.audit);
    let reasons: Vec<_> = refusals(&events)
        .iter()
        .map(|event| event.reject_reason.clone())
        .collect();
    assert_eq!(reasons.len(), 2, "{events:?}");
    assert!(reasons.iter().any(|r| r.contains("set-zero refused")));
    assert!(reasons.iter().any(|r| r.contains("enable refused")));
}

#[test]
fn chappe_set_zero_from_owner_is_queued() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = enabled_without_pose();
    let mut queue = idle_queue();
    bus.publish(
        "robot/set_zero",
        "test",
        "marengo.v1.SetZeroRequest",
        &SetZeroRequest {
            timestamp_ms: 1,
            operator_id: "consul-tab".into(),
            joint: JOINT.into(),
            confirm: true,
            sign_test_passed: true,
        },
    )
    .expect("publish set_zero");
    drain_chappe(&mut loop_ctrl, &mut queue, CHAPPE_OWNS, &bus, &mut rx);
    assert!(queue.is_busy(), "owner set-zero is admitted");
}

#[test]
fn stdin_motion_from_non_owner_is_refused_but_status_and_stop_pass() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    for line in ["enable bench", "hold-at right_elbow_pitch 0.1", "hold-on"] {
        let cmd = crate::parse_command(line).expect("parses");
        assert!(
            !admit_stdin_command(CHAPPE_OWNS, &bus, &cmd),
            "{line} must be refused for non-owner stdin"
        );
    }
    assert_eq!(refusals(&audit_events(&mut rx.audit)).len(), 3);
    for cmd in [PiCommand::Disable, PiCommand::Quit, PiCommand::Status] {
        assert!(admit_stdin_command(CHAPPE_OWNS, &bus, &cmd));
    }
    assert!(refusals(&audit_events(&mut rx.audit)).is_empty());
    // The owner is never refused.
    let hold = crate::parse_command("hold-on").expect("parses");
    assert!(admit_stdin_command(STDIN_OWNS, &bus, &hold));
}

// ---- stop is ownerless --------------------------------------------------

/// Chappe disable stops the arm even though stdin owns motion.
#[test]
fn chappe_disable_is_accepted_from_non_owner_chappe() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = impedance_loop();
    let mut queue = idle_queue();
    publish_enable(&bus, false);
    drain_chappe(&mut loop_ctrl, &mut queue, STDIN_OWNS, &bus, &mut rx);

    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    assert_eq!(
        loop_ctrl.supervisor().mode(),
        davout::OperationalMode::Disabled
    );
    assert!(
        refusals(&audit_events(&mut rx.audit)).is_empty(),
        "a stop is never refused"
    );
}

/// Stdin disable stops the arm even though Chappe owns motion.
#[test]
fn stdin_disable_is_accepted_from_non_owner_stdin() {
    let bus = Bus::new(32);
    let mut loop_ctrl = impedance_loop();
    let mut queue = idle_queue();
    let mut gate = EnableGate::default();
    assert!(admit_stdin_command(CHAPPE_OWNS, &bus, &PiCommand::Disable));
    assert!(dispatch_stdin_command(
        &mut loop_ctrl,
        &mut queue,
        &mut gate,
        PiCommand::Disable,
        &repo_root().join("config"),
    ));
    assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
    assert_eq!(
        loop_ctrl.supervisor().mode(),
        davout::OperationalMode::Disabled
    );
}

/// An operator disable from either source is not undone by a later motion
/// command (L-berthier-28).
#[test]
fn operator_disable_blocks_implicit_reenable_from_either_source() {
    let config = repo_root().join("config");
    // stdin disable → stdin hold-at refused
    let mut loop_ctrl = enabled_without_pose();
    first_session_status(&mut loop_ctrl);
    let mut queue = idle_queue();
    let mut gate = EnableGate::default();
    dispatch_stdin_command(
        &mut loop_ctrl,
        &mut queue,
        &mut gate,
        PiCommand::Disable,
        &config,
    );
    assert!(loop_ctrl.implicit_enable_forbidden());
    let lines = gate.submit(
        &mut loop_ctrl,
        PiCommand::HoldAt {
            joint: Some(JOINT.into()),
            position_rad: 0.05,
        },
        Instant::now(),
    );
    assert!(
        matches!(&lines[..], [GateLine::Err(text)] if text.contains("disabled by an operator")),
        "{lines:?}"
    );
    assert_eq!(
        loop_ctrl.supervisor().mode(),
        davout::OperationalMode::Disabled,
        "refusal must not re-enable the drives"
    );

    // Chappe disable → Consul Testing position batch refused, drives stay off
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = enabled_without_pose();
    first_session_status(&mut loop_ctrl);
    let mut queue = idle_queue();
    publish_enable(&bus, false);
    drain_chappe(&mut loop_ctrl, &mut queue, CHAPPE_OWNS, &bus, &mut rx);
    assert!(loop_ctrl.implicit_enable_forbidden());
    publish_batch(&bus, &gain_batch(ProtoControlMode::Position, 0.05, 0.0));
    drain_testing(&mut loop_ctrl, &queue, CHAPPE_OWNS, &bus, &mut rx);
    assert_eq!(
        loop_ctrl.supervisor().mode(),
        davout::OperationalMode::Disabled
    );
    assert_ne!(loop_ctrl.control_mode(), ControlMode::Position);
}

// ---- reference-queue gate ------------------------------------------------

/// L-marengo-pi-02: Testing batches obey the same reference-queue busy gate
/// as Chappe enable; same-drain Set Zero → batch is refused, not executed.
#[test]
fn testing_batch_is_refused_while_the_reference_queue_is_busy() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = impedance_loop();
    let queue = busy_queue();
    publish_batch(&bus, &gain_batch(ProtoControlMode::Impedance, 0.0, 12.0));
    drain_testing(&mut loop_ctrl, &queue, CHAPPE_OWNS, &bus, &mut rx);

    assert!(
        loop_ctrl.gain_override(JOINT).is_none(),
        "batch must have no effect while a reference is queued"
    );
    let events = audit_events(&mut rx.audit);
    let refused = refusals(&events);
    assert_eq!(refused.len(), 1, "{events:?}");
    assert!(refused[0]
        .reject_reason
        .contains("reference acquisition in progress"));

    // Once the queue drains the same batch is accepted.
    publish_batch(&bus, &gain_batch(ProtoControlMode::Impedance, 0.0, 12.0));
    drain_testing(&mut loop_ctrl, &idle_queue(), CHAPPE_OWNS, &bus, &mut rx);
    assert!(loop_ctrl.gain_override(JOINT).is_some());
}

#[test]
fn same_tick_set_zero_then_testing_batch_does_not_enable_or_arm() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = enabled_without_pose();
    first_session_status(&mut loop_ctrl);
    let mut queue = idle_queue();
    bus.publish(
        "robot/set_zero",
        "test",
        "marengo.v1.SetZeroRequest",
        &SetZeroRequest {
            timestamp_ms: 1,
            operator_id: "consul".into(),
            joint: JOINT.into(),
            confirm: true,
            sign_test_passed: true,
        },
    )
    .expect("publish set_zero");
    publish_batch(&bus, &gain_batch(ProtoControlMode::Position, 0.05, 5.0));
    // Same tick order as run_control_loop: Chappe commands, then Testing.
    drain_chappe(&mut loop_ctrl, &mut queue, CHAPPE_OWNS, &bus, &mut rx);
    assert!(queue.is_busy());
    drain_testing(&mut loop_ctrl, &queue, CHAPPE_OWNS, &bus, &mut rx);

    assert_ne!(loop_ctrl.control_mode(), ControlMode::Position);
    assert!(loop_ctrl.gain_override(JOINT).is_none());
    assert!(loop_ctrl.position_setpoints().is_none());
    assert_eq!(refusals(&audit_events(&mut rx.audit)).len(), 1);
}

// ---- lagged channels -------------------------------------------------------

/// L-marengo-pi-06: a lagged robot/enable channel may have dropped a Disable.
/// The drain must stop the drives (fail closed), not treat Lagged as empty.
#[test]
fn lagged_enable_channel_never_silently_drops_a_disable() {
    for lease in [CHAPPE_OWNS, STDIN_OWNS] {
        // Capacity 4: the first message (the operator's Disable) is evicted.
        let bus = Bus::new(4);
        let mut rx = channels(&bus);
        let mut probe = bus.subscribe("robot/enable");
        let mut loop_ctrl = impedance_loop();
        let mut queue = idle_queue();
        publish_enable(&bus, false);
        for _ in 0..8 {
            publish_enable(&bus, true);
        }
        assert!(
            matches!(
                probe.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
            ),
            "fixture must lag the receiver and evict the disable"
        );

        drain_chappe(&mut loop_ctrl, &mut queue, lease, &bus, &mut rx);

        assert_eq!(
            loop_ctrl.control_mode(),
            ControlMode::Disabled,
            "lag must fail closed, lease {lease:?}"
        );
        assert_eq!(
            loop_ctrl.supervisor().mode(),
            davout::OperationalMode::Disabled
        );
        assert!(
            loop_ctrl.implicit_enable_forbidden(),
            "no implicit re-enable after a lag stop"
        );
        assert!(
            matches!(
                rx.enable.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            ),
            "survivors that cannot be ordered against the lost disable are discarded"
        );
        let events = audit_events(&mut rx.audit);
        assert!(
            events.iter().any(|event| event.action == "stop_on_lag"),
            "the stop must be visible: {events:?}"
        );
    }
}

// ---- CS19: gains survive the mode switch ------------------------------------

/// A Testing POSITION batch with gains arriving in GravityComp: the gains used
/// to be applied before the mode switch and silently dropped.
#[test]
fn position_batch_gains_survive_entering_position_from_gravity_comp() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = enabled_without_pose();
    first_session_status(&mut loop_ctrl);
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    publish_batch(&bus, &gain_batch(ProtoControlMode::Position, 0.02, 20.0));
    drain_testing(&mut loop_ctrl, &idle_queue(), CHAPPE_OWNS, &bus, &mut rx);

    assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    let gain = loop_ctrl
        .gain_override(JOINT)
        .expect("requested gains survive the GravityComp → Position switch");
    assert_eq!(gain.kp, 20.0);
}

/// An IMPEDANCE batch while the loop is in GravityComp cannot hold gains; the
/// refusal is logged and nothing is stashed.
#[test]
fn impedance_batch_in_gravity_comp_stashes_nothing() {
    let bus = Bus::new(32);
    let mut rx = channels(&bus);
    let mut loop_ctrl = enabled_without_pose();
    first_session_status(&mut loop_ctrl);
    loop_ctrl.set_control_mode(ControlMode::GravityComp);
    publish_batch(&bus, &gain_batch(ProtoControlMode::Impedance, 0.0, 12.0));
    drain_testing(&mut loop_ctrl, &idle_queue(), CHAPPE_OWNS, &bus, &mut rx);
    assert!(loop_ctrl.gain_override(JOINT).is_none());
    assert_eq!(loop_ctrl.control_mode(), ControlMode::GravityComp);
}
