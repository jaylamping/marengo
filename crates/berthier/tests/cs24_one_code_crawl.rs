// Archive-only, unexecuted preparation against the existing exported ControlLoop.
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::collections::HashSet;
use std::path::Path;

use berthier::{ControlLoop, ControlMode, GainOverride, LoopError};
use davout::simulation::{InitialVirtualReference, SimulationBus, SimulationTransmission};
use davout::OperationalMode;
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};

const SELECTED: &str = "right_shoulder_pitch";
const PEER: &str = "right_shoulder_roll";

// Literal fault-free Run observations: can0 motor1 -> hostfd, pitch direction -1.
// Raw q decreases one decoder count toward the independently requested +.30 target.
// Raw velocity/torque stay centered; these are inputs, never planner echoes.
const STATUS_BYTES: [[u8; 8]; 7] = [
    [0x7f, 0xcb, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
    [0x7f, 0xca, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
    [0x7f, 0xc9, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
    [0x7f, 0xc8, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
    [0x7f, 0xc7, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
    [0x7f, 0xc6, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
    [0x7f, 0xc5, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
];

fn selected_status(stage: usize) -> ReceivedCanFrame {
    ReceivedCanFrame::full_data(
        Some("can0".to_string()),
        CanFrame {
            id: 0x0280_01fd,
            data: STATUS_BYTES[stage],
            extended: true,
        },
    )
}
fn queue_peer_feedback(ctrl: &mut ControlLoop<SimulationBus>) {
    for joint in ctrl.joint_names().to_vec() {
        if joint != SELECTED {
            support::queue_joint_status(ctrl.supervisor_mut(), &joint, 0.0, 0.0);
        }
    }
}

#[derive(Debug)]
struct PoseObservation {
    step: u32,
    stage: usize,
    position: Option<f64>,
    derived_velocity: Option<f64>,
    fault: Option<u16>,
    peer_measured: bool,
    pending_receive: usize,
}

#[derive(Debug)]
struct CaseObservation {
    completed: u32,
    stall: Option<(String, u64, u32)>,
    other_error: Option<String>,
    initial_position: Option<f64>,
    target_before: Option<f64>,
    target_after: Option<f64>,
    mode_before: (OperationalMode, ControlMode),
    mode_after: (OperationalMode, ControlMode),
    active_before: HashSet<String>,
    active_after: HashSet<String>,
    stop_before: u64,
    stop_after: u64,
    latched: bool,
    input: Vec<PoseObservation>,
    trace: Vec<SimulationTransmission>,
    cleanup_error: Option<String>,
    fixture_removed: bool,
}

fn run_case(source: &Path, label: &str, ticks: u32, crawl: bool) -> CaseObservation {
    let fixture = support::fixture_tree_without_diagnostics(label, source);
    let fixture_path = fixture.path().to_path_buf();

    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![SELECTED.to_string()]),
        200,
        50,
    )
    .expect("closed selected-only INITIAL virtual reference");
    ctrl.supervisor_mut()
        .enable_targets(&[SELECTED.to_string()])
        .expect("actual scoped Enable selects only pitch");
    ctrl.supervisor_mut()
        .bus_mut()
        .queue_received(selected_status(0))
        .expect("finite literal pose queued after Enable returns");
    queue_peer_feedback(&mut ctrl);
    ctrl.supervisor_mut()
        .drain_feedback()
        .expect("actual current-session pose drain");
    let initial_position = ctrl
        .supervisor()
        .joint_feedback(SELECTED)
        .map(|pose| pose.position_rad);
    ctrl.enter_position_hold_at(Some(SELECTED), 0.30)
        .expect("actual finite unresolved target admission");
    ctrl.apply_gain_override(
        SELECTED,
        GainOverride {
            kp: 8.0,
            kd: 1.25,
            ki: 0.0,
            fc: 0.0,
        },
    )
    .expect("finite valid gains below the installed caps");
    let selected_index = ctrl
        .joint_names()
        .iter()
        .position(|name| name == SELECTED)
        .expect("configured selected joint");
    let target_before = ctrl
        .position_setpoints()
        .and_then(|targets| targets.get(selected_index).copied());
    let mode_before = (ctrl.supervisor().mode(), ctrl.control_mode());
    let active_before = ctrl.supervisor().active_joints().clone();
    let stop_before = ctrl.supervisor().stop_generation();
    ctrl.supervisor_mut().bus_mut().clear_trace();

    let mut completed = 0;
    let mut stall = None;
    let mut other_error = None;
    let mut input = Vec::new();
    for step in 1..=ticks {
        // A literal one-code advance at ticks300,600,...,1800. No wall-clock
        // pacing or injected timestamps; the controller's fixed period is5ms.
        let stage = if crawl { (step / 300) as usize } else { 0 };
        queue_peer_feedback(&mut ctrl);
        if let Err(error) = ctrl
            .supervisor_mut()
            .bus_mut()
            .queue_received(selected_status(stage))
        {
            other_error = Some(format!("literal receive admission failed: {error}"));
            break;
        }
        let result = ctrl.tick(None);
        let pose = ctrl.supervisor().joint_feedback(SELECTED);
        input.push(PoseObservation {
            step,
            stage,
            position: pose.as_ref().map(|pose| pose.position_rad),
            // Davout derives this from q and adjacent REAL receive Instants.
            // Centered raw dq is not a claim of coherent physical crawl speed.
            derived_velocity: pose.as_ref().map(|pose| pose.velocity_rad_s),
            fault: pose.as_ref().map(|pose| pose.fault),
            peer_measured: ctrl.supervisor().joint_feedback(PEER).is_some(),
            pending_receive: ctrl.supervisor().bus().pending_receive_count(),
        });
        match result {
            Ok(()) => completed = step,
            Err(LoopError::AscentStall { joint, ms, .. }) => {
                stall = Some((joint, ms, step));
                break;
            }
            Err(error) => {
                other_error = Some(error.to_string());
                break;
            }
        }
    }

    // Every output/state oracle is captured before the unconditional cleanup.
    let trace = ctrl.supervisor().bus().transmissions().to_vec();
    let snapshot = ctrl.supervisor().safety_snapshot();
    let target_after = ctrl
        .position_setpoints()
        .and_then(|targets| targets.get(selected_index).copied());
    let mode_after = (ctrl.supervisor().mode(), ctrl.control_mode());
    let active_after = ctrl.supervisor().active_joints().clone();
    let cleanup_error = if ctrl.supervisor().mode() == OperationalMode::Active {
        ctrl.supervisor_mut()
            .disable_all()
            .err()
            .map(|error| error.to_string())
    } else {
        None
    };
    drop(ctrl);
    drop(fixture);
    let fixture_removed = !fixture_path
        .try_exists()
        .expect("observe actual exclusive-fixture cleanup");

    CaseObservation {
        completed,
        stall,
        other_error,
        initial_position,
        target_before,
        target_after,
        mode_before,
        mode_after,
        active_before,
        active_after,
        stop_before,
        stop_after: snapshot.stop_generation,
        latched: snapshot.is_latched(),
        input,
        trace,
        cleanup_error,
        fixture_removed,
    }
}

fn selected_gain_mit(tx: &SimulationTransmission) -> bool {
    tx.delivered
        && tx.address == Some(MotorAddress::new("can0", 1))
        && tx.frame.id >> 24 == 1
        && tx.frame.id & 0xff == 1
        && tx.frame.data[4..6] != [0, 0]
}

fn assert_input_prefix(case: &CaseObservation) {
    let initial = case.initial_position.expect("actual post-enable raw pose");
    assert!((0.0199..0.0200).contains(&initial));
    let mut previous_stage = 0;
    let mut previous_position = initial;
    for observed in &case.input {
        let position = observed.position.expect("actual selected input visibility");
        let velocity = observed
            .derived_velocity
            .expect("actual position-derived velocity visibility");
        assert_eq!(observed.fault, Some(0));
        assert!(observed.peer_measured);
        assert_eq!(observed.pending_receive, 0);
        assert!(position.is_finite() && velocity.is_finite());
        if observed.stage == previous_stage {
            assert_eq!(position, previous_position);
            assert_eq!(velocity, 0.0, "held raw code at step{}", observed.step);
        } else {
            assert_eq!(observed.stage, previous_stage + 1);
            assert_eq!(observed.step % 300, 0);
            assert!(
                (0.00038..0.00039).contains(&(position - previous_position)),
                "one literal pitch decoder count must be measured toward target"
            );
            // A one-count edge may produce a large velocity under fast host
            // execution. Record it; do not replace it with nominal q-step/dt.
            assert!(velocity >= 0.0);
            previous_stage = observed.stage;
            previous_position = position;
        }
    }
}

#[test]
fn one_encoder_code_each_300_ticks_keeps_selected_position_owner_active() {
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original_control =
        std::fs::read(source.join("config/control.yaml")).expect("immutable master control input");

    // Genuine short unresolved output control runs BEFORE the nine-second case.
    // Both fixtures, including a failing crawl, are fully captured/dropped before
    // final outcome assertions, so a blanket early-fault repair cannot pass.
    let positive = run_case(&source, "cs24-crawl-positive", 100, false);
    let crawl = run_case(&source, "cs24-one-code-crawl", 1800, true);

    let boundary_feedback: Vec<_> = crawl
        .input
        .iter()
        .filter(|pose| pose.step % 300 == 0)
        .map(|pose| (pose.step, pose.position, pose.derived_velocity))
        .collect();
    eprintln!(
        "CS24 raw crawl: short_completed={}, crawl_completed={}, stall={:?}, other={:?}, mode={:?}, stop_generation={}->{}; 9000 nominal ms, literal boundary feedback={boundary_feedback:?}",
        positive.completed, crawl.completed, crawl.stall, crawl.other_error,
        crawl.mode_after, crawl.stop_before, crawl.stop_after,
    );

    for case in [&positive, &crawl] {
        assert!(
            case.fixture_removed,
            "actual exclusive fixture must be gone"
        );
        assert!(
            case.cleanup_error.is_none(),
            "virtual cleanup: {:?}",
            case.cleanup_error
        );
        assert_eq!(case.target_before, Some(0.30));
        assert_eq!(
            case.mode_before,
            (OperationalMode::Active, ControlMode::Position)
        );
        assert_eq!(case.active_before.len(), 1);
        assert!(case.active_before.contains(SELECTED));
        assert!(!case.active_before.contains(PEER));
        assert_input_prefix(case);
    }
    assert_eq!(
        std::fs::read(source.join("config/control.yaml")).expect("master control unchanged"),
        original_control
    );
    assert!(
        positive.other_error.is_none() && positive.stall.is_none(),
        "short unresolved gain-MIT reachability must pass: completed={}, stall={:?}, other={:?}",
        positive.completed,
        positive.stall,
        positive.other_error
    );
    assert_eq!(positive.completed, 100);
    assert_eq!(
        positive
            .trace
            .iter()
            .filter(|tx| selected_gain_mit(tx))
            .count(),
        100,
        "100 real unresolved selected gain-bearing MIT writes precede crawl classification"
    );
    assert_eq!(positive.mode_after, positive.mode_before);
    assert_eq!(positive.stop_after, positive.stop_before);
    assert!(!positive.latched);

    assert!(
        crawl.other_error.is_none(),
        "a different runtime failure is not one-code stall evidence: {:?}",
        crawl.other_error
    );
    assert_eq!(
        crawl.completed, 1800,
        "CS24 crawl: one measured pitch encoder count every300 nominal ticks must preserve the selected position owner for1800 ticks; stall={:?}",
        crawl.stall
    );
    assert!(crawl.stall.is_none());
    assert_eq!(crawl.input.len(), 1800);
    assert_eq!(crawl.input.last().expect("full literal sequence").stage, 6);
    assert_eq!(crawl.target_after, crawl.target_before);
    assert_eq!(crawl.mode_after, crawl.mode_before);
    assert_eq!(crawl.active_after, crawl.active_before);
    assert_eq!(crawl.stop_after, crawl.stop_before);
    assert!(!crawl.latched);
    assert_eq!(
        crawl
            .trace
            .iter()
            .filter(|tx| selected_gain_mit(tx))
            .count(),
        1800,
        "each actual crawl tick must reach a selected gain-bearing MIT write"
    );
    assert!(crawl.trace.iter().all(|tx| {
        tx.delivered
            && tx.frame.id >> 24 == 1
            && tx.address == Some(MotorAddress::new("can0", 1))
            && tx.frame.id & 0xff == 1
            && tx.frame.data[4..6] != [0, 0]
    }), "the captured crawl must contain only selected gain-MIT output, no peer Enable/MIT or stop attempts");
}
