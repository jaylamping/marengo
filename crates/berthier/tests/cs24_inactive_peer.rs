// External, unexecuted preparation. Parent binds/formats/runs this exact public-API case.
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use berthier::{ControlLoop, ControlMode, GainOverride, LoopError};
use davout::simulation::InitialVirtualReference;
use davout::simulation::SimulationBus;
use davout::OperationalMode;
use robstride::{CanFrame, MotorAddress, ReceivedCanFrame};

const SELECTED: &str = "right_shoulder_pitch";
const PEER: &str = "right_shoulder_roll";

fn selected_status() -> ReceivedCanFrame {
    // Independent literal fault-free Run status, interfacecan0 motor1 -> hostfd.
    // Direction-1 RS03 gives joint q about+.01994; centered velocity gives0.
    ReceivedCanFrame::full_data(
        Some("can0".to_string()),
        CanFrame {
            id: 0x0280_01fd,
            data: [0x7f, 0xcb, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

#[test]
fn unresolved_inactive_peer_cannot_trip_selected_position_owner() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original_control =
        std::fs::read(source.join("config/control.yaml")).expect("immutable master control input");
    let fixture = support::FixtureTree::new("cs24-inactive-peer", &source);
    let fixture_path = fixture.path().to_path_buf();
    let copied_control = fixture.path().join("config/control.yaml");
    let text = std::fs::read_to_string(&copied_control).expect("copied control input");
    assert_eq!(
        text.matches("active_reporting_diagnostics: true").count(),
        1
    );
    std::fs::write(
        &copied_control,
        text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("only copied diagnostic policy changes before construction");

    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![SELECTED.to_string()]),
        200,
        50,
    )
    .expect("closed selected-only INITIAL virtual reference");
    assert_eq!(ctrl.configured_loop_hz(), 200);
    assert_eq!(ctrl.supervisor().motors.motors.len(), 5);
    ctrl.supervisor_mut()
        .enable_targets(&[SELECTED.to_string()])
        .expect("actual scoped Enable selects only pitch");
    ctrl.supervisor_mut()
        .bus_mut()
        .queue_received(selected_status())
        .expect("finite addressed pose queued after Enable returns");
    ctrl.supervisor_mut()
        .drain_feedback()
        .expect("actual current-session pose drain");
    let measured = ctrl
        .supervisor()
        .joint_feedback(SELECTED)
        .expect("actual fresh selected feedback");
    assert!((0.019..0.021).contains(&measured.position_rad));
    assert_eq!(measured.velocity_rad_s, 0.0);
    assert_eq!(measured.fault, 0);
    assert!(ctrl.supervisor().joint_feedback(PEER).is_none());
    // Settled target is measured encoder q, never a planner output or echo.
    ctrl.enter_position_hold_at(Some(SELECTED), measured.position_rad)
        .expect("actual settled selected position entry");
    ctrl.apply_gain_override(
        SELECTED,
        GainOverride {
            kp: 8.0,
            kd: 1.25,
            ki: 0.0,
            fc: 0.0,
        },
    )
    .expect("finite valid output gains below installed caps");
    ctrl.supervisor_mut()
        .bus_mut()
        .queue_received(selected_status())
        .expect("fresh selected positive-neighbor observation");
    ctrl.tick(None)
        .expect("selected positive neighbor reaches real control output");
    let positive_trace = ctrl.supervisor().bus().transmissions().to_vec();
    assert!(
        positive_trace.iter().any(|tx| {
            tx.delivered
                && tx.address == Some(MotorAddress::new("can0", 1))
                && tx.frame.id >> 24 == 1
                && tx.frame.data[4..6] != [0, 0]
        }),
        "actual selected gain-bearing MIT must precede the unresolved peer case"
    );

    // This is an actual existing public method and a finite envelope-valid target.
    // It does not reference/enable the peer or install any feedback/cache state.
    let admission = ctrl.set_joint_position_setpoint(PEER, 0.30);
    let admitted = admission.is_ok();
    let admission_error = admission.err().map(|error| error.to_string());
    let selected_index = ctrl
        .joint_names()
        .iter()
        .position(|name| name == SELECTED)
        .expect("configured selected joint");
    let peer_index = ctrl
        .joint_names()
        .iter()
        .position(|name| name == PEER)
        .expect("configured inactive peer");
    let target_before = ctrl.position_setpoints().map(<[f64]>::to_vec);
    let mode_before = (ctrl.supervisor().mode(), ctrl.control_mode());
    let active_before = ctrl.supervisor().active_joints().clone();
    let stop_generation_before = ctrl.supervisor().stop_generation();
    let mut completed = 0_u32;
    let mut observed_stall = None;
    let mut unrelated_error = None;
    if admitted {
        for step in 1_u32..=600 {
            ctrl.supervisor_mut()
                .bus_mut()
                .queue_received(selected_status())
                .expect("finite fresh settled selected status");
            match ctrl.tick(None) {
                Ok(()) => {
                    completed = step;
                    let q = ctrl
                        .supervisor()
                        .joint_feedback(SELECTED)
                        .expect("selected-only fresh raw input remains available");
                    assert_eq!(q.position_rad, measured.position_rad);
                    assert_eq!(q.velocity_rad_s, 0.0);
                    assert!(ctrl.supervisor().joint_feedback(PEER).is_none());
                }
                Err(LoopError::AscentStall { joint, ms, .. }) => {
                    observed_stall = Some((joint, ms, step));
                    break;
                }
                Err(error) => {
                    unrelated_error = Some(error.to_string());
                    break;
                }
            }
        }
    }
    // Capture actual production output and persistent state before any cleanup.
    let trace_before_cleanup = ctrl.supervisor().bus().transmissions().to_vec();
    let safety_before_cleanup = ctrl.supervisor().safety_snapshot();
    let mode_after = (ctrl.supervisor().mode(), ctrl.control_mode());
    let active_after = ctrl.supervisor().active_joints().clone();
    let targets_after = ctrl.position_setpoints().map(<[f64]>::to_vec);
    if ctrl.supervisor().mode() == OperationalMode::Active {
        ctrl.supervisor_mut()
            .disable_all()
            .expect("virtual cleanup only; excluded from output/state oracles");
    }
    drop(ctrl);
    drop(fixture);
    assert!(!fixture_path
        .try_exists()
        .expect("actual exclusive-fixture cleanup"));
    assert_eq!(
        std::fs::read(source.join("config/control.yaml")).expect("master unchanged"),
        original_control
    );
    assert!(admitted, "actual peer target admission was refused; this cannot count as watchdog reachability: {admission_error:?}");
    let targets_before = target_before.expect("real position intent after peer admission");
    assert!((targets_before[selected_index] - measured.position_rad).abs() < 1e-12);
    assert!(
        (targets_before[peer_index] - 0.30).abs() < 1e-12,
        "actual public method must admit the unresolved peer target without clamp masking"
    );
    assert_eq!(
        mode_before,
        (OperationalMode::Active, ControlMode::Position)
    );
    assert_eq!(active_before.len(), 1);
    assert!(active_before.contains(SELECTED) && !active_before.contains(PEER));
    assert!(
        unrelated_error.is_none(),
        "other errors are not inactive-peer evidence: {unrelated_error:?}"
    );
    assert_eq!(completed, 600,
        "CS24: an unresolved inactive peer must not trip a settled selected ControlLoop owner; completed={completed}, observed_stall={observed_stall:?}");
    assert!(observed_stall.is_none());
    assert!(!safety_before_cleanup.is_latched());
    assert_eq!(
        safety_before_cleanup.stop_generation,
        stop_generation_before
    );
    assert_eq!(mode_after, mode_before);
    assert_eq!(active_after, active_before);
    assert_eq!(targets_after, Some(targets_before));
    let selected_mit = trace_before_cleanup
        .iter()
        .filter(|tx| {
            tx.frame.id >> 24 == 1
                && tx.address == Some(MotorAddress::new("can0", 1))
                && tx.frame.data[4..6] != [0, 0]
                && tx.delivered
        })
        .count();
    assert!(
        selected_mit >= 601,
        "each actual settled tick must retain selected MIT reachability"
    );
    assert!(
        trace_before_cleanup
            .iter()
            .filter(|tx| matches!(tx.frame.id >> 24, 1 | 3))
            .all(|tx| tx.address == Some(MotorAddress::new("can0", 1)) && tx.frame.id & 0xff == 1),
        "configured peers must never receive Enable or MIT from the scoped owner"
    );
}
