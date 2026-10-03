// External preparation only; root must execute and bind an exact source archive.
// Existing public ControlLoop/closed raw scripts, no copied law or clock pacing.
#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use berthier::{ControlLoop, ControlMode, GainOverride, LoopError};
use davout::simulation::{
    InitialVirtualReference, SimulationBus, SimulationTransmission, TxMatcher, TxOccurrence, TxRule,
};
use davout::{DavoutError, FaultClass, OperationalMode, SafetySnapshot, StopAction};
use robstride::{CanFrame, MotorAddress};

const JOINT: &str = "right_shoulder_pitch";
const NEUTRAL_MIT: [u8; 8] = [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0];
const ZERO_SPEED: [u8; 8] = [0x0a, 0x70, 0, 0, 0, 0, 0, 0];

// Independent literal inputs: can0 motor1 -> hostfd, fault-free Run.
// RS03 direction-1: q about+.01994 joint, first raw dq about+.01038 (±20 rad/s scale).
// Stationary status retains the exact position code and centered raw velocity.
fn status(initial_positive_velocity: bool) -> CanFrame {
    CanFrame {
        id: 0x0280_01fd,
        data: if initial_positive_velocity {
            [0x7f, 0xcb, 0x7f, 0xee, 0x7f, 0xff, 0, 0xc8]
        } else {
            [0x7f, 0xcb, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8]
        },
        extended: true,
    }
}

fn is_non_neutral_mit(frame: &CanFrame) -> bool {
    frame.id >> 24 == 1
        && (((frame.id >> 8) & 0xffff) != 0x7fff
            || frame.data[2..4] != [0x7f, 0xff]
            || frame.data[4..8] != [0, 0, 0, 0])
}

struct Followup {
    before: SafetySnapshot,
    after: SafetySnapshot,
    tick_ok: bool,
    reenable_rejected: bool,
    torque_rejected: bool,
    frames: Vec<SimulationTransmission>,
    disabled: bool,
    intent_cleared: bool,
}

#[test]
fn raw_velocity_then_stiction_faults_controller_and_preserves_stop_latch() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let master_control_before =
        std::fs::read(source.join("config/control.yaml")).expect("immutable master control input");
    let fixture = support::FixtureTree::new("cs24-controller-stiction", &source);
    let fixture_path = fixture.path().to_path_buf();
    let control_path = fixture.path().join("config/control.yaml");
    let control_text = std::fs::read_to_string(&control_path).expect("copied fixture control");
    assert_eq!(
        control_text
            .matches("active_reporting_diagnostics: true")
            .count(),
        1
    );
    std::fs::write(
        &control_path,
        control_text.replace(
            "active_reporting_diagnostics: true",
            "active_reporting_diagnostics: false",
        ),
    )
    .expect("only copied reporting policy changed before construction");

    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![JOINT.to_string()]),
        200,
        50,
    )
    .expect("closed declared initial virtual reference, not physical acquisition");
    assert_eq!(ctrl.configured_loop_hz(), 200);
    assert_eq!(ctrl.supervisor().motors.motors.len(), 5);
    ctrl.supervisor_mut()
        .enable_targets(&[JOINT.to_string()])
        .expect("scoped virtual Enable reaches Active");
    // Queue only AFTER Enable's two real preactivation drains and session marker.
    ctrl.supervisor_mut()
        .bus_mut()
        .queue_frame(status(true))
        .expect("finite post-enable literal Run status");
    for joint in ctrl.joint_names().to_vec() {
        if joint != JOINT {
            support::queue_joint_status(ctrl.supervisor_mut(), &joint, 0.0, 0.0);
        }
    }
    ctrl.enter_position_hold_at(Some(JOINT), 0.30)
        .expect("actual public entry consumes raw pose and seeds its real filter");
    let entry = ctrl
        .supervisor()
        .joint_feedback(JOINT)
        .expect("current-enable feedback");
    assert!(
        (0.019..0.021).contains(&entry.position_rad),
        "literal raw position reaches joint space"
    );
    assert!(
        (0.005..0.015).contains(&entry.velocity_rad_s),
        "first literal positive velocity reaches entry"
    );
    assert_eq!(entry.fault, 0);
    assert_eq!(ctrl.supervisor().mode(), OperationalMode::Active);
    assert_eq!(ctrl.control_mode(), ControlMode::Position);
    assert_eq!(ctrl.supervisor().active_joints().len(), 1);
    assert!(ctrl.supervisor().active_joints().contains(JOINT));
    assert!(!ctrl.supervisor().has_latched_fault());
    let index = ctrl
        .joint_names()
        .iter()
        .position(|name| name == JOINT)
        .expect("configured target");
    assert!(
        (ctrl.position_setpoints().expect("actual latched target")[index] - 0.30).abs() < 1e-12
    );
    ctrl.apply_gain_override(
        JOINT,
        GainOverride {
            kp: 8.0,
            kd: 1.25,
            ki: 0.0,
            fc: 0.0,
        },
    )
    .expect("valid output-only gains below installed caps");

    let transition = ctrl
        .supervisor_mut()
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(1),
                device_id: Some(1),
                interface: Some("can0".to_string()),
            },
            occurrence: TxOccurrence::Nth(1),
            receive: vec![status(false).into()],
            send_error: None,
        })
        .expect("one finite first-MIT stationary observation");
    ctrl.supervisor_mut().bus_mut().clear_trace();
    ctrl.tick(None)
        .expect("positive neighbor reaches actual nonneutral MIT before stiction");
    let first_trace = ctrl.supervisor().bus().transmissions().to_vec();
    let transition_triggers = ctrl.supervisor().bus().rule_trigger_count(transition);
    let first_stationary = ctrl
        .supervisor()
        .joint_feedback(JOINT)
        .expect("fresh post-send stationary pose");
    assert_eq!(
        transition_triggers, 1,
        "real addressed MIT must trigger the finite stationary script"
    );
    assert_eq!(first_stationary.position_rad, entry.position_rad);
    assert_eq!(
        first_stationary.velocity_rad_s, 0.0,
        "actual successive-position derivation sees stiction"
    );
    assert!(!ctrl.supervisor().has_latched_fault());
    assert!(
        first_trace.iter().any(|tx| {
            tx.delivered
                && tx.address == Some(MotorAddress::new("can0", 1))
                && is_non_neutral_mit(&tx.frame)
                && tx.frame.data[4..6] != [0, 0]
        }),
        "positive neighbor must exercise actual gain-bearing MIT output"
    );

    let generation_before_fault = ctrl.supervisor().stop_generation();
    let mut fault = None;
    let mut unexpected_error = None;
    let mut automatic_stop = Vec::new();
    // Actual public tick uses its installed 5ms dt. Each call receives a freshly
    // delivered same-q frame through Davout; no future stamps, sleep, empty-read
    // failure, encoder roundtrip or private planner measurement is involved.
    for step in 1_u32..=600 {
        for joint in ctrl.joint_names().to_vec() {
            if joint != JOINT {
                support::queue_joint_status(ctrl.supervisor_mut(), &joint, 0.0, 0.0);
            }
        }
        ctrl.supervisor_mut()
            .bus_mut()
            .queue_frame(status(false))
            .expect("finite fresh stationary status");
        let trace_start = ctrl.supervisor().bus().transmissions().len();
        match ctrl.tick(None) {
            Ok(()) => {
                let observed = ctrl
                    .supervisor()
                    .joint_feedback(JOINT)
                    .expect("fresh actual stationary feedback");
                assert_eq!(observed.position_rad, entry.position_rad);
                assert_eq!(observed.velocity_rad_s, 0.0);
            }
            Err(LoopError::AscentStall { joint, ms, .. }) => {
                fault = Some((joint, ms, step));
                // Capture only actual automatic fault writes, BEFORE any cleanup.
                automatic_stop = ctrl.supervisor().bus().transmissions()[trace_start..].to_vec();
                break;
            }
            Err(error) => {
                unexpected_error = Some(error.to_string());
                break;
            }
        }
    }

    let followup = if fault.is_some() {
        let before = ctrl.supervisor().safety_snapshot();
        ctrl.supervisor_mut().bus_mut().clear_trace();
        ctrl.supervisor_mut()
            .bus_mut()
            .queue_frame(status(false))
            .expect("fresh healthy status after fault");
        let tick_ok = ctrl.tick(None).is_ok();
        let reenable_rejected = matches!(
            ctrl.supervisor_mut().enable_targets(&[JOINT.to_string()]),
            Err(DavoutError::FaultLatched {
                class: FaultClass::Controller,
                ..
            })
        );
        let torque_rejected = matches!(
            ctrl.set_torque_cmd(JOINT, 0.05),
            Err(LoopError::Safety(DavoutError::FaultLatched {
                class: FaultClass::Controller,
                ..
            }))
        );
        Some(Followup {
            before,
            after: ctrl.supervisor().safety_snapshot(),
            tick_ok,
            reenable_rejected,
            torque_rejected,
            frames: ctrl.supervisor().bus().transmissions().to_vec(),
            disabled: ctrl.supervisor().mode() == OperationalMode::Disabled
                && ctrl.control_mode() == ControlMode::Disabled,
            intent_cleared: ctrl.position_setpoints().is_none()
                && ctrl.position_hold_commands().is_none()
                && !ctrl.position_wave_active()
                && ctrl.gain_override(JOINT).is_none()
                && ctrl.torque_cmd(JOINT) == 0.0,
        })
    } else {
        None
    };
    let final_transition_triggers = ctrl.supervisor().bus().rule_trigger_count(transition);
    // Baseline no-fault cleanup cannot manufacture automatic-stop evidence.
    if ctrl.supervisor().mode() == OperationalMode::Active {
        ctrl.supervisor_mut()
            .disable_all()
            .expect("virtual cleanup only, excluded from automatic trace");
    }
    drop(ctrl);
    drop(fixture);
    assert!(!fixture_path
        .try_exists()
        .expect("actual fixture cleanup query"));
    assert_eq!(
        std::fs::read(source.join("config/control.yaml")).expect("unchanged master input"),
        master_control_before
    );
    assert!(
        unexpected_error.is_none(),
        "setup/other failures are not CS24 evidence: {unexpected_error:?}"
    );
    assert!(matches!(&fault, Some((joint, ms, step)) if joint == JOINT && *ms >= 2000 && *ms <= 2500 && *step <= 600),
        "CS24: actual ControlLoop must fault on fresh stationary encoder input after raw positive velocity evidence; got {fault:?}");

    let followup = followup.expect("typed actual stall reached followup capture");
    assert_eq!(
        final_transition_triggers, 1,
        "one-shot stationary script cannot refire on neutral stop"
    );
    assert_eq!(
        automatic_stop.len(),
        15,
        "all automatic stop attempts captured before cleanup"
    );
    let ids = [
        0x1200_fd01,
        0x017f_ff01,
        0x0400_fd01,
        0x1200_fd02,
        0x017f_ff02,
        0x0400_fd02,
        0x1200_fd03,
        0x017f_ff03,
        0x0400_fd03,
        0x1200_fd04,
        0x017f_ff04,
        0x0400_fd04,
        0x1200_fd05,
        0x017f_ff05,
        0x0400_fd05,
    ];
    let report = followup
        .before
        .last_stop
        .as_ref()
        .expect("actual automatic StopReport");
    assert_eq!(report.generation, generation_before_fault + 1);
    assert_eq!(report.attempts.len(), 15);
    assert_eq!(report.failed_writes(), 0);
    for (index, (tx, id)) in automatic_stop.iter().zip(ids).enumerate() {
        let device = (index / 3 + 1) as u8;
        let action = [
            StopAction::ZeroSpeed,
            StopAction::NeutralMit,
            StopAction::Disable,
        ][index % 3];
        let bytes = [ZERO_SPEED, NEUTRAL_MIT, [0; 8]][index % 3];
        assert_eq!(tx.address, Some(MotorAddress::new("can0", device)));
        assert_eq!(tx.frame.id, id);
        assert!(tx.frame.extended && tx.delivered);
        assert_eq!(tx.frame.data, bytes, "independent literal wire payload");
        assert_eq!(
            report.attempts[index].address,
            MotorAddress::new("can0", device)
        );
        assert_eq!(report.attempts[index].action, action);
        assert!(report.attempts[index].error.is_none());
    }
    let initiating = followup
        .before
        .first_fault()
        .expect("persistent controller fault");
    assert_eq!(initiating.class, FaultClass::Controller);
    assert_eq!(initiating.joint.as_deref(), Some(JOINT));
    assert_eq!(initiating.address, Some(MotorAddress::new("can0", 1)));
    assert!(followup.after.is_latched());
    assert_eq!(followup.after.first_fault(), Some(initiating));
    assert_eq!(followup.after.last_stop, followup.before.last_stop);
    assert_eq!(
        followup.after.stop_generation,
        followup.before.stop_generation
    );
    assert!(followup.tick_ok && followup.reenable_rejected && followup.torque_rejected);
    assert!(followup.disabled && followup.intent_cleared);
    assert!(
        !followup
            .frames
            .iter()
            .any(|tx| matches!(tx.frame.id >> 24, 1 | 3)),
        "healthy diagnostics and rejected later commands cannot send MIT or Enable after the latch"
    );
}
