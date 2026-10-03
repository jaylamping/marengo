//! Public owner regression for a finite phase deadline near Duration::MAX.
//! Captures the candidate behavior before fallback cleanup; no physical proof.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::Duration;

use davout::simulation::{
    InitialVirtualReference, SimulationBus, SimulationReceive, SimulationTransmission, TxMatcher,
    TxOccurrence, TxRule,
};
use davout::{
    JointHomingState, OperationalMode, ReferenceCancelReason, ReferenceCause, ReferenceCommit,
    ReferencePhase, ReferenceRequest, Supervisor,
};
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};

#[path = "../../berthier/tests/support/mod.rs"]
mod support;

const TARGET: &str = "right_elbow_pitch";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Wire {
    address: Option<MotorAddress>,
    frame: CanFrame,
    delivered: bool,
}

fn captured(trace: &[SimulationTransmission]) -> Vec<Wire> {
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

/// Independently literal installed cleanup routes and payloads.
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

#[test]
fn finite_phase_deadline_is_capped_before_overflow_after_target_arming() {
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
        let bytes = std::fs::read(&path).expect("immutable master fixture input");
        (path, bytes)
    })
    .collect();
    let fixture = support::fixture_tree_without_diagnostics("reference-clock-boundary", &root);
    let fixture_path = fixture.path().to_path_buf();
    let history = fixture.path().join("history.yaml");
    let homing_path = fixture.path().join("config/homing.yaml");
    let mut homing: serde_yaml::Value =
        serde_yaml::from_slice(&std::fs::read(&homing_path).expect("copied homing bytes"))
            .expect("actual copied homing document");
    homing["homing"]["joints"][TARGET]
        .as_mapping_mut()
        .expect("selected manual-reference mapping")
        .insert(
            serde_yaml::Value::String("search_timeout_s".into()),
            serde_yaml::to_value(9.0_f64).expect("finite selected timeout"),
        );
    std::fs::write(
        &homing_path,
        serde_yaml::to_string(&homing).expect("only selected copied timeout changed"),
    )
    .expect("copied selected policy");
    let mut owner = Supervisor::from_simulation_with_calibration_record_path(
        fixture.path(),
        SimulationBus::default(),
        &history,
        InitialVirtualReference::Unreferenced,
    )
    .expect("actual closed owner with isolated history");
    let enable = owner
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
        .expect("actual selected Enable triggers a literal raw Run response");
    owner
        .bus_mut()
        .elapse_reference_clock(Duration::MAX - Duration::from_secs(9))
        .expect("finite monotonic clock with room for the actual nine-second reservation");
    owner.bus_mut().clear_trace();
    let request = ReferenceRequest {
        stamp: owner
            .reference_snapshot()
            .next_stamp
            .expect("actual owner stamp"),
        joint: TARGET.into(),
        confirmed: true,
        sign_verified: true,
    };
    let handle = owner
        .begin_reference(request)
        .expect("finite preflight fits at clock boundary");
    let initial = owner.reference_snapshot();
    let initial_trace = captured(owner.bus().transmissions());
    // Four literal 1.9-second intervals, including BEFORE the first advance:
    // MAX−7.1s stop; MAX−5.2s old drain; MAX−3.3s Enable; MAX−1.4s post-arm drain.
    // Each admitted old phase deadline is still 100ms in the future. The final
    // two-second candidate addition overflows despite the finite overall cap.
    let mut advances = Vec::new();
    for _ in 0..4 {
        owner
            .bus_mut()
            .elapse_reference_clock(Duration::from_millis(1900))
            .expect("finite monotonic interval below each live deadline");
        advances.push(owner.advance_reference(&handle));
    }
    // Preserve the actual result/state/wire output before any fallback action.
    let before_cleanup = owner.reference_snapshot();
    let before_trace = captured(owner.bus().transmissions());
    let before_busy = owner.reference_busy();
    let before_pose = owner.joint_feedback(TARGET);
    let enable_triggers = owner.bus().rule_trigger_count(enable);
    let terminal = owner
        .cancel_reference(&handle, ReferenceCancelReason::Operator)
        .expect("actual mandatory fallback cleanup after capturing the boundary");
    let motion_refused = owner.enable_targets(&[TARGET.into()]).is_err();
    let final_trace = captured(owner.bus().transmissions());
    let final_snapshot = owner.reference_snapshot();
    let final_mode = owner.mode();
    let final_states: Vec<_> = owner
        .motors
        .motors
        .iter()
        .map(|motor| owner.joint_homing_state(&motor.joint))
        .collect();
    let history_absent = !history
        .try_exists()
        .expect("actual history path observation");
    drop(owner);
    drop(fixture);
    let removed = !fixture_path
        .try_exists()
        .expect("exclusive fixture cleanup observation");
    let master_unchanged = master
        .iter()
        .all(|(path, bytes)| std::fs::read(path).expect("master after cleanup") == *bytes);

    // Real admission, raw decoding, cleanup and permission controls precede the
    // decisive finite-deadline assertion, even against the defective candidate.
    assert!(removed);
    assert!(master_unchanged);
    assert!(history_absent);
    assert_eq!(initial.phase, ReferencePhase::BaselineStop);
    assert_eq!(initial.remaining, Some(Duration::from_secs(2)));
    assert!(
        initial_trace.is_empty(),
        "reservation performs no transmission"
    );
    let first_three: Vec<_> = advances[..3]
        .iter()
        .map(|step| {
            step.as_ref()
                .expect("unexpired reachable pre-overflow phase")
                .phase
        })
        .collect();
    assert_eq!(
        first_three,
        [
            ReferencePhase::DrainOld,
            ReferencePhase::ArmTarget,
            ReferencePhase::DrainPostArm
        ]
    );
    assert_eq!(enable_triggers, 1);
    assert_eq!(
        before_pose
            .expect("post-arm response reached the actual decoder")
            .position_rad,
        0.0
    );
    let mut expected_before = all_stop();
    expected_before.push(wire(4, 0x0300fd04, [0; 8]));
    assert_eq!(before_trace, expected_before);
    assert!(before_busy);
    assert!(before_cleanup.reference_armed);
    assert!(!before_cleanup.usable_reference);
    assert_eq!(
        terminal.cause,
        ReferenceCause::Cancelled(ReferenceCancelReason::Operator)
    );
    assert_eq!(terminal.handle, handle);
    assert_eq!(terminal.commit, ReferenceCommit::Unavailable);
    assert!(!terminal.usable_reference);
    assert!(terminal.reporting.is_empty());
    assert_eq!(terminal.stop.generation, 2);
    assert_eq!(terminal.stop.attempts.len(), 15);
    assert_eq!(terminal.stop.failed_writes(), 0);
    let mut expected_final = expected_before;
    expected_final.extend(all_stop());
    assert_eq!(
        final_trace, expected_final,
        "real fallback completed all fifteen cleanup attempts"
    );
    assert!(motion_refused);
    assert_eq!(final_mode, OperationalMode::Disabled);
    assert!(final_states
        .iter()
        .all(|state| *state == JointHomingState::Unhomed));
    assert_eq!(final_snapshot.terminal.as_ref(), Some(&terminal));
    assert!(!final_snapshot.reference_armed);
    println!("actual MAX boundary results={advances:?}, pre-cleanup={before_cleanup:?}, cleanup={terminal:?}");
    assert!(advances[3].is_ok(),
        "finite overall deadline must cap the next phase before overflowing after the actual target Enable: {:?}", advances[3]);
    assert_eq!(before_cleanup.phase, ReferencePhase::SetZero);
    assert_eq!(
        before_cleanup.remaining,
        Some(Duration::from_millis(1400)),
        "finite remaining overall time must bound the next phase at the maximum virtual clock"
    );
}
