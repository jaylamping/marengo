//! Phase B / WP-D: `ControlLoop`-level evidence for the fuse, wave and admission fixes.
#![allow(clippy::expect_used, clippy::panic)]

use super::*;
use crate::position_hold::POSITION_ASCENT_STALL_FAULT_MS;
use crate::position_trace::PositionTrace;
use crate::test_support::queue_all_status;
use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{FaultClass, OperationalMode};

const JOINT: &str = "right_shoulder_pitch";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn test_loop() -> ControlLoop<SimulationBus> {
    ControlLoop::from_simulation(
        repo_root(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        200,
        50,
    )
    .expect("declared virtual initial reference")
}

fn ready_active_at(ctrl: &mut ControlLoop<SimulationBus>, q: f64) {
    ctrl.supervisor_mut().set_homing_complete().expect("ready");
    ctrl.supervisor_mut().request_enable(true).expect("enable");
    queue_all_status(ctrl.supervisor_mut(), Some((JOINT, q, 0.0)));
    ctrl.supervisor_mut()
        .drain_feedback()
        .expect("raw initial observations");
}

fn replay_feedback(ctrl: &mut ControlLoop<SimulationBus>, q: f64) {
    queue_all_status(ctrl.supervisor_mut(), Some((JOINT, q, 0.0)));
    ctrl.supervisor_mut()
        .drain_feedback()
        .expect("finite raw observations");
}

fn intent_discarded(ctrl: &ControlLoop<SimulationBus>) -> bool {
    ctrl.control_mode() == ControlMode::Disabled
        && ctrl.supervisor().mode() == OperationalMode::Disabled
        && ctrl.position_setpoints().is_none()
        && ctrl.position_hold_commands().is_none()
        && !ctrl.position_wave_active()
        && ctrl.gain_override(JOINT).is_none()
        && ctrl.torque_cmd(JOINT) == 0.0
}

/// After a latched controller fault: the reason is recorded, re-enable is refused, and a later
/// tick sends no MIT or Enable.
fn assert_latched_controller_fault(
    ctrl: &mut ControlLoop<SimulationBus>,
    joint: Option<&str>,
    reason: &str,
) {
    let snapshot = ctrl.supervisor().safety_snapshot();
    let first = snapshot.first_fault().expect("controller fault latched");
    assert_eq!(first.class, FaultClass::Controller);
    assert_eq!(first.joint.as_deref(), joint);
    assert_eq!(first.message, reason);
    assert!(intent_discarded(ctrl), "motion intent must be discarded");
    assert!(matches!(
        ctrl.supervisor_mut().enable_targets(&[JOINT.to_string()]),
        Err(DavoutError::FaultLatched {
            class: FaultClass::Controller,
            ..
        })
    ));
    ctrl.supervisor_mut().bus_mut().clear_trace();
    replay_feedback(ctrl, 0.0);
    ctrl.tick(None).expect("tick after the latch is quiet");
    assert!(
        !ctrl
            .supervisor()
            .bus()
            .frames()
            .iter()
            .any(|frame| matches!(frame.id >> 24, 1 | 3)),
        "no MIT or Enable after the latch"
    );
}

// ---- G1 / safety-invariants gap 7: HoldTracking where it latches --------------------------

#[test]
fn hold_tracking_trip_latches_controller_fault_and_discards_intent() {
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.0);
    ctrl.enter_position_hold()
        .expect("hold-on latches measured q");
    // Soft P so the model's holding torque dominates the net command at the sagged pose.
    ctrl.apply_gain_override(
        JOINT,
        GainOverride {
            kp: 0.5,
            kd: 0.0,
            ki: 0.0,
            fc: 0.0,
        },
    )
    .expect("gain override");
    let mut trip = None;
    for step in 1..=600_u32 {
        replay_feedback(&mut ctrl, -0.3);
        if let Err(err) = ctrl.tick(None) {
            trip = Some((err, step));
            break;
        }
    }
    let (err, step) = trip.expect("a sagged hold with opposing net torque must trip");
    let LoopError::HoldTracking {
        joint, ms, trip, ..
    } = &err
    else {
        panic!("expected HoldTracking, got {err:?}");
    };
    assert_eq!(joint, JOINT);
    assert!(
        (POSITION_ASCENT_STALL_FAULT_MS..POSITION_ASCENT_STALL_FAULT_MS + 10).contains(ms),
        "{ms}"
    );
    assert!(step <= 410, "trip too late at step {step}");
    assert_eq!(trip.target, 0.0);
    assert!(
        trip.tau_p + trip.tau_ff < 0.0 && trip.target > trip.q,
        "net command pushes away from the target: {trip:?}"
    );
    let text = err.to_string();
    assert!(
        text.contains("hold tracking failure on right_shoulder_pitch"),
        "{text}"
    );
    assert_latched_controller_fault(
        &mut ctrl,
        Some(JOINT),
        "position hold off target with opposing commanded torque and no progress",
    );
}

#[test]
fn ascent_stall_trip_records_its_reason_and_discards_intent() {
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.02);
    ctrl.enter_position_hold_at(Some(JOINT), 0.15)
        .expect("hold-at");
    let mut err = None;
    for _ in 0..600 {
        replay_feedback(&mut ctrl, 0.02);
        if let Err(e) = ctrl.tick(None) {
            err = Some(e);
            break;
        }
    }
    let err = err.expect("stuck outbound ascent must trip");
    assert!(matches!(err, LoopError::AscentStall { .. }), "{err:?}");
    assert!(err
        .to_string()
        .contains("outbound ascent stall on right_shoulder_pitch"));
    assert_latched_controller_fault(
        &mut ctrl,
        Some(JOINT),
        "position ascent stalled without measured progress",
    );
}

#[test]
fn wave_stall_trip_latches_controller_fault_and_discards_intent() {
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.3);
    ctrl.start_position_wave(JOINT, 0.2, 0.6, 3, 2.0)
        .expect("valid wave");
    let mut err = None;
    for step in 1..=700_u32 {
        replay_feedback(&mut ctrl, 0.3);
        if let Err(e) = ctrl.tick(None) {
            err = Some((e, step));
            break;
        }
    }
    let (err, step) = err.expect("a jammed wave joint must trip");
    assert!(matches!(err, LoopError::WaveStall { .. }), "{err:?}");
    assert!(step <= 520, "trip too late at step {step}");
    assert_latched_controller_fault(
        &mut ctrl,
        Some(JOINT),
        "position wave stalled without measured motion",
    );
}

// ---- L-berthier-27 / -15: internal invariant breaks fail closed ---------------------------

#[test]
fn position_mode_without_a_latched_setpoint_latches_and_discards() {
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.0);
    ctrl.set_control_mode(ControlMode::Position);
    let err = ctrl.tick(None).expect_err("no setpoint");
    assert!(matches!(err, LoopError::MissingSetpoint { .. }), "{err:?}");
    assert_latched_controller_fault(
        &mut ctrl,
        None,
        "position hold lost its setpoint latch (controller invariant)",
    );
}

#[test]
fn hold_length_mismatch_is_not_reported_as_an_operator_setpoint_error() {
    assert!(matches!(
        LoopError::from(HoldError::LenMismatch),
        LoopError::HoldLenMismatch
    ));
}

// ---- L-berthier-04 (intent L4): non-finite hold targets are refused -----------------------

#[test]
fn non_finite_hold_target_is_refused_without_half_applying() {
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.02);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = ctrl
            .enter_position_hold_at(Some(JOINT), value)
            .expect_err("enter_position_hold_at must refuse");
        assert!(matches!(err, LoopError::NonFiniteTarget { .. }), "{err:?}");
        let err = ctrl
            .set_joint_position_setpoint(JOINT, value)
            .expect_err("set_joint_position_setpoint must refuse");
        assert!(matches!(err, LoopError::NonFiniteTarget { .. }), "{err:?}");
    }
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    assert!(
        ctrl.position_setpoints().is_none(),
        "nothing armed or latched"
    );

    ctrl.enter_position_hold_at(Some(JOINT), 0.1)
        .expect("finite");
    let before = ctrl.position_setpoints().expect("armed").to_vec();
    let err = ctrl
        .set_joint_position_setpoint(JOINT, f64::NAN)
        .expect_err("NaN retarget refused while armed");
    assert!(matches!(err, LoopError::NonFiniteTarget { .. }), "{err:?}");
    let after = ctrl.position_setpoints().expect("still armed");
    assert_eq!(after, before.as_slice());
    assert!(after.iter().all(|target| target.is_finite()));
    replay_feedback(&mut ctrl, 0.02);
    ctrl.tick(None).expect("the finite hold keeps running");
}

// ---- L-berthier-03: wave admission --------------------------------------------------------

#[test]
fn wave_admission_refuses_non_finite_out_of_range_and_too_aggressive_waves() {
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.02);
    for (min, max, half) in [
        (f64::NAN, 0.5, 1.0),
        (0.0, f64::NAN, 1.0),
        (0.0, 0.5, f64::NAN),
        (0.0, f64::INFINITY, 1.0),
        (0.0, 0.5, f64::INFINITY),
    ] {
        let err = ctrl
            .start_position_wave(JOINT, min, max, 1, half)
            .expect_err("non-finite wave");
        assert!(matches!(err, LoopError::NonFiniteWave { .. }), "{err:?}");
    }
    // Pitch soft range is -0.873 .. 2.898 rad.
    let err = ctrl
        .start_position_wave(JOINT, 0.0, 3.0, 1, 8.0)
        .expect_err("beyond the soft upper bound");
    assert!(
        matches!(err, LoopError::WaveOutsideLimits { .. }),
        "{err:?}"
    );
    let err = ctrl
        .start_position_wave(JOINT, -1.0, 0.5, 1, 8.0)
        .expect_err("beyond the soft lower bound");
    assert!(
        matches!(err, LoopError::WaveOutsideLimits { .. }),
        "{err:?}"
    );
    // 1 rad in 0.1 s: ~15.7 rad/s peak, far above the cap.
    let err = ctrl
        .start_position_wave(JOINT, 0.0, 1.0, 1, 0.1)
        .expect_err("too fast");
    assert!(
        matches!(&err, LoopError::WaveExceedsMotionLimit { quantity, .. } if quantity.starts_with("speed")),
        "{err:?}"
    );
    // A sub-tick half period is quantized to one tick and judged at its true speed.
    let err = ctrl
        .start_position_wave(JOINT, 0.0, 0.01, 1, 1e-9)
        .expect_err("sub-tick half period");
    assert!(
        matches!(err, LoopError::WaveExceedsMotionLimit { .. }),
        "{err:?}"
    );
    // Speed within the cap but acceleration beyond position_trajectory_accel_rad_s2 (4.5).
    let err = ctrl
        .start_position_wave(JOINT, 0.0, 0.2, 1, 0.4)
        .expect_err("too aggressive");
    assert!(
        matches!(&err, LoopError::WaveExceedsMotionLimit { quantity, .. } if quantity.starts_with("acceleration")),
        "{err:?}"
    );
    assert!(!ctrl.position_wave_active());
    assert_eq!(ctrl.control_mode(), ControlMode::Disabled);
    assert!(
        ctrl.position_setpoints().is_none(),
        "refused waves arm nothing"
    );

    ctrl.start_position_wave(JOINT, 0.0, 0.3, 1, 2.0)
        .expect("a gentle in-range wave is admitted");
    assert!(ctrl.position_wave_active());
}

// ---- L-berthier-14 -------------------------------------------------------------------------

#[test]
fn zero_loop_rate_is_rejected_not_coerced_to_one_hertz() {
    let result = ControlLoop::from_simulation(
        repo_root(),
        SimulationBus::default(),
        InitialVirtualReference::AllConfigured,
        0,
        50,
    );
    assert!(matches!(
        result.err(),
        Some(LoopError::InvalidLoopPeriod { .. })
    ));
}

// ---- L-berthier-01: the law judges the wire kp --------------------------------------------

fn trace_kp_column(csv: &str) -> Vec<f64> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().expect("trace header").split(',').collect();
    let joint_col = header
        .iter()
        .position(|c| *c == "joint")
        .expect("joint col");
    let kp_col = header.iter().position(|c| *c == "kp").expect("kp col");
    lines
        .filter_map(|line| {
            let cols: Vec<&str> = line.split(',').collect();
            (cols.get(joint_col) == Some(&JOINT))
                .then(|| cols[kp_col].parse::<f64>().expect("numeric kp"))
        })
        .collect()
}

#[test]
fn position_law_kp_is_the_wire_kp_during_a_gain_ramp() {
    let path = std::env::temp_dir().join(format!(
        "berthier-wp-d-kp-{}-{}.csv",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let mut ctrl = test_loop();
    ready_active_at(&mut ctrl, 0.0);
    ctrl.set_control_mode(ControlMode::GravityComp);
    ctrl.position_trace = Some(PositionTrace::open_for_test(&path, 200).expect("trace file"));
    let yaml_kp = ctrl.supervisor.control.control.joints[JOINT].impedance.kp;
    assert!(yaml_kp > 1.0, "fixture needs a stiff impedance kp");
    // GravityComp -> Position arms a 100 ms kp ramp that starts at the GravityComp gains.
    ctrl.enter_position_hold().expect("hold-on");
    ctrl.supervisor_mut().bus_mut().clear_trace();
    let mut wire_kp_raw = Vec::new();
    for _ in 0..3 {
        ctrl.supervisor_mut().bus_mut().clear_trace();
        replay_feedback(&mut ctrl, 0.0);
        ctrl.tick(None).expect("ramp tick");
        let raw = ctrl
            .supervisor()
            .bus()
            .frames()
            .iter()
            .find(|frame| frame.id >> 24 == 1 && frame.id & 0xff == 1)
            .map(|frame| u16::from_be_bytes([frame.data[4], frame.data[5]]))
            .expect("pitch MIT frame");
        wire_kp_raw.push(raw);
    }
    ctrl.position_trace
        .as_mut()
        .expect("trace")
        .flush()
        .expect("flush trace");
    let csv = std::fs::read_to_string(&path).expect("trace csv");
    let _ = std::fs::remove_file(&path);
    let kp = trace_kp_column(&csv);
    assert!(kp.len() >= 3, "trace rows: {kp:?}");
    // 20-tick linear ramp from 0: tick k carries yaml_kp * k / 20.
    for (k, value) in kp.iter().take(3).enumerate() {
        let expected = yaml_kp * k as f64 / 20.0;
        assert!(
            (value - expected).abs() < 1e-3,
            "tick {k}: law kp {value} must equal the wire kp {expected}, not the YAML kp {yaml_kp}"
        );
    }
    assert_eq!(wire_kp_raw[0], 0, "ramp starts at zero on the wire");
    assert!(wire_kp_raw[1] > 0 && wire_kp_raw[2] > wire_kp_raw[1]);
}
