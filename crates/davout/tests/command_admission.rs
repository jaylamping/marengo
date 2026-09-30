//! Public safety admission contracts; all output is recorded, never sent to hardware.
#![allow(clippy::expect_used)]
use davout::simulation::{SimulationBus, SimulationReceive, TxMatcher, TxOccurrence, TxRule};
use std::time::{Duration, Instant};

use davout::{JointCommand, MitJointCommand, SpeedCommand, Supervisor};
use marengo_config::{MotorEntry, MotorType};
use robstride::{
    CanFrame, DetailedFaultFeedback, DriveMode, FeedbackEvent, FeedbackObservation, FeedbackReport,
    MitFeedback, MotorAddress, MotorBus, MotorState,
};

fn supervisor() -> Supervisor<SimulationBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        davout::simulation::InitialVirtualReference::AllConfigured,
    )
    .expect("valid repo fixture");
    // Successful admission cases should tolerate loaded CI scheduling. Expiry
    // cases explicitly use a short deadline and require rejection.
    supervisor.control.control.comm_watchdog_ms = 1000;
    supervisor
}

fn motor<B: MotorBus>(supervisor: &Supervisor<B>, joint: &str) -> MotorEntry {
    supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == joint)
        .expect("joint")
        .clone()
}

fn activate<B: MotorBus>(supervisor: &mut Supervisor<B>, motors: &[MotorEntry]) {
    let joints: Vec<_> = motors.iter().map(|motor| motor.joint.clone()).collect();
    supervisor
        .enable_targets(&joints)
        .expect("recording enable");
}

fn command(motor: &MotorEntry) -> MitJointCommand {
    MitJointCommand {
        joint: motor.joint.clone(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.1,
    }
}

fn status(received: Instant) -> MotorState {
    MotorState {
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_nm: 0.0,
        temperature_c: 25.0,
        fault: 0,
        updated: Some(received),
    }
}

// Finite wire-compatible inputs use real raw decoding. Only nonfinite/impossible
// state fields use the separately declared typed consumer-contract seam.
fn inject(supervisor: &mut Supervisor<SimulationBus>, address: MotorAddress, state: MotorState) {
    let finite = [
        state.position_rad,
        state.velocity_rad_s,
        state.torque_nm,
        state.temperature_c,
    ]
    .iter()
    .all(|value| value.is_finite());
    if finite && state.updated.is_some() {
        let motor_type = supervisor
            .motors
            .motors
            .iter()
            .find(|motor| MotorAddress::from(*motor) == address)
            .map_or(MotorType::Rs03, |motor| motor.motor_type);
        let (velocity_scale, torque_scale) = match motor_type {
            MotorType::Rs00 => (50.0, 17.0),
            MotorType::Rs02 => (44.0, 17.0),
            MotorType::Rs03 => (50.0, 60.0),
            MotorType::Rs04 => (15.0, 120.0),
        };
        let raw = |value: f32, scale: f32| {
            ((value / scale + 1.0) * 32767.0)
                .round()
                .clamp(0.0, 65535.0) as u16
        };
        let mut data = [0; 8];
        data[0..2]
            .copy_from_slice(&raw(state.position_rad, 4.0 * std::f32::consts::PI).to_be_bytes());
        data[2..4].copy_from_slice(&raw(state.velocity_rad_s, velocity_scale).to_be_bytes());
        data[4..6].copy_from_slice(&raw(state.torque_nm, torque_scale).to_be_bytes());
        data[6..8].copy_from_slice(&((state.temperature_c * 10.0).round() as u16).to_be_bytes());
        supervisor
            .bus_mut()
            .queue_timed(robstride::TimedCanFrame {
                received_at: state.updated.expect("stamped raw fixture"),
                received: robstride::ReceivedCanFrame::full_data(
                    Some(address.interface),
                    CanFrame {
                        id: 0x0280_00fd
                            | (u32::from(address.device_id) << 8)
                            | (u32::from(state.fault & 0x3f) << 16),
                        data,
                        extended: true,
                    },
                ),
            })
            .expect("finite raw input");
    } else {
        let event = if state.updated.is_some() {
            FeedbackEvent::Status(MitFeedback {
                device_id: address.device_id,
                position_rad: state.position_rad,
                velocity_rad_s: state.velocity_rad_s,
                torque_nm: state.torque_nm,
                temperature_c: state.temperature_c,
                fault: state.fault,
                status_flags: state.fault as u8,
                drive_mode: DriveMode::Run,
            })
        } else {
            FeedbackEvent::DetailedFault(DetailedFaultFeedback {
                raw: [u8::from(state.fault != 0), 0, 0, 0, 0, 0, 0, 0],
            })
        };
        supervisor
            .bus_mut()
            .queue_feedback_report(FeedbackReport {
                observations: vec![FeedbackObservation {
                    order: 0,
                    address,
                    received_at: state.updated.unwrap_or_else(Instant::now),
                    can_id: 0,
                    event,
                }],
                raw_frames: 1,
                read_attempts: 1,
                ..FeedbackReport::default()
            })
            .expect("bounded impossible-wire consumer fixture");
    }
}

fn enable_rule(
    supervisor: &mut Supervisor<SimulationBus>,
    device: u8,
    receive: Vec<SimulationReceive>,
) -> davout::simulation::RuleId {
    supervisor
        .bus_mut()
        .add_tx_rule(TxRule {
            matcher: TxMatcher {
                communication_type: Some(3),
                device_id: Some(device),
                ..TxMatcher::default()
            },
            occurrence: TxOccurrence::Nth(1),
            receive,
            send_error: None,
        })
        .expect("bounded enable-stage script")
}

fn receive(supervisor: &mut Supervisor<SimulationBus>, motors: &[MotorEntry]) {
    let now = Instant::now();
    for motor in motors {
        inject(supervisor, MotorAddress::from(motor), status(now));
    }
    supervisor.drain_feedback().expect("valid status");
}

fn assert_only_stop_traffic(frames: &[CanFrame]) {
    for frame in frames {
        assert!(
            matches!(frame.id >> 24, 1 | 4 | 18 | 24),
            "unexpected stop communication type {}",
            frame.id >> 24
        );
        match frame.id >> 24 {
            1 => {
                assert_eq!((frame.id >> 8) & 0xffff, 0x7fff, "nonzero stop FF");
                assert_eq!(&frame.data[2..4], &[0x7f, 0xff], "nonzero stop velocity");
                assert_eq!(&frame.data[4..8], &[0, 0, 0, 0], "nonzero stop gains");
            }
            4 => assert_eq!(frame.data[0], 0, "ordinary stop cleared firmware fault"),
            18 => assert_eq!(
                &frame.data[4..8],
                &[0, 0, 0, 0],
                "nonzero speed/parameter write"
            ),
            24 => {
                assert_eq!(&frame.data[0..6], &[1, 2, 3, 4, 5, 6]);
                assert!(frame.data[6] <= 1);
                assert_eq!(frame.data[7], 0);
            }
            _ => {}
        }
    }
}

#[test]
fn nonneutral_motion_requires_post_enable_feedback_even_during_bootstrap() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().clear_trace();
    let result = supervisor.send_mit_batch(vec![command(&pitch)]);
    assert!(
        result.is_err(),
        "nonneutral motion was admitted without post-enable pose"
    );
    assert!(supervisor.bus().frames().is_empty());

    let mut neutral = command(&pitch);
    neutral.torque_ff_nm = 0.0;
    supervisor
        .send_mit_batch(vec![neutral])
        .expect("bounded neutral status solicitation");
    assert_eq!(supervisor.bus().frames().len(), 1);
    receive(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("motion with new pose");
}

#[test]
fn empty_drains_and_unknown_traffic_do_not_refresh_expired_pose() {
    for unknown_traffic in [false, true] {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        supervisor.control.control.comm_watchdog_ms = 1;
        activate(&mut supervisor, std::slice::from_ref(&pitch));
        receive(&mut supervisor, std::slice::from_ref(&pitch));
        std::thread::sleep(Duration::from_millis(3));
        for _ in 0..8 {
            if unknown_traffic {
                inject(
                    &mut supervisor,
                    MotorAddress::new("can-other", 99),
                    status(Instant::now()),
                );
            }
            supervisor.drain_feedback().expect("empty or unknown RX");
        }
        supervisor.bus_mut().clear_trace();
        assert!(
            supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
            "empty/unknown drains refreshed expired pose (unknown={unknown_traffic})"
        );
        assert_only_stop_traffic(supervisor.bus().frames());
    }
}

#[test]
fn one_live_peer_cannot_mask_a_silent_active_motor() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let roll = motor(&supervisor, "right_shoulder_roll");
    supervisor.control.control.comm_watchdog_ms = 1;
    activate(&mut supervisor, &[pitch.clone(), roll.clone()]);
    receive(&mut supervisor, &[pitch.clone(), roll]);
    std::thread::sleep(Duration::from_millis(3));
    receive(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "live pitch masked silent roll"
    );
    assert_only_stop_traffic(supervisor.bus().frames());
}

#[test]
fn original_timestamp_and_enable_generation_are_required() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    inject(
        &mut supervisor,
        MotorAddress::from(&pitch),
        status(Instant::now() - Duration::from_secs(1)),
    );
    supervisor
        .drain_feedback()
        .expect("old sample can remain diagnostic");
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "draining an old sample fabricated a new acquisition time"
    );
    assert!(supervisor.bus().frames().is_empty());
    receive(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("fresh motion");
    supervisor.disable_all().expect("recording stop");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "previous enable generation reused pose"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn preenable_queued_status_cannot_authorize_new_motion() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    supervisor
        .bus_mut()
        .queue_frame(CanFrame {
            id: (2 << 24) | (u32::from(pitch.device_id) << 8) | 0xfd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        })
        .expect("finite closed script");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor.drain_feedback().expect("queued RX");
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "queued pre-enable status became current readiness"
    );
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn status_queued_during_enable_cannot_authorize_new_motion() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let rule = enable_rule(
        &mut supervisor,
        pitch.device_id,
        vec![CanFrame {
            id: 0x0280_00fd | (u32::from(pitch.device_id) << 8),
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        }
        .into()],
    );
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    assert_eq!(supervisor.bus().rule_trigger_count(rule), 1);
    supervisor
        .drain_feedback()
        .expect("post-enable empty drain");
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.send_mit_batch(vec![command(&pitch)]).is_err(),
        "status queued during enable writes authorized nonneutral motion"
    );
    assert!(supervisor.bus().frames().is_empty());

    supervisor
        .bus_mut()
        .queue_frame(CanFrame {
            id: (2 << 24) | (2 << 22) | (u32::from(pitch.device_id) << 8) | 0xfd,
            data: [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        })
        .expect("finite closed script");
    supervisor.drain_feedback().expect("new session status");
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("new status authorizes motion");
    assert_eq!(supervisor.bus().frames().len(), 1);
}

#[test]
fn enable_final_drain_failure_rolls_back_to_disabled() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let rule = enable_rule(
        &mut supervisor,
        pitch.device_id,
        vec![SimulationReceive::Error(
            "injected final-drain failure".into(),
        )],
    );
    assert!(supervisor
        .enable_targets(std::slice::from_ref(&pitch.joint))
        .is_err());
    assert_eq!(supervisor.mode(), davout::OperationalMode::Disabled);
    assert!(supervisor.active_joints().is_empty());
    assert!(supervisor.enable_session_started_at().is_none());
    assert_eq!(supervisor.bus().rule_trigger_count(rule), 1);
    assert!(supervisor
        .bus()
        .frames()
        .iter()
        .any(|frame| frame.id >> 24 == 4 && frame.id & 0xff == u32::from(pitch.device_id)));
}

#[test]
fn fault_only_wire_traffic_never_creates_pose() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    supervisor
        .bus_mut()
        .queue_frame(CanFrame {
            id: (21 << 24) | (u32::from(pitch.device_id) << 8) | 0xfd,
            data: [0; 8],
            extended: true,
        })
        .expect("finite closed script");
    supervisor.drain_feedback().expect("zero fault report");
    supervisor.bus_mut().clear_trace();
    assert!(
        supervisor.joint_feedback(&pitch.joint).is_none(),
        "fault-only report fabricated zero pose"
    );
    assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn all_nonfinite_mit_fields_and_negative_gains_are_rejected_before_clamping() {
    let mut accepted = Vec::new();
    for (field, value) in (0..5)
        .flat_map(|field| [f64::NAN, f64::INFINITY, f64::NEG_INFINITY].map(|value| (field, value)))
    {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        let mut request = command(&pitch);
        match field {
            0 => request.position_rad = value,
            1 => request.velocity_rad_s = value,
            2 => request.kp = value,
            3 => request.kd = value,
            _ => request.torque_ff_nm = value,
        }
        if supervisor.filter_mit_command(request, &pitch).is_ok() {
            accepted.push(format!("field={field} value={value}"));
        }
    }
    for field in [2, 3] {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        let mut request = command(&pitch);
        if field == 2 {
            request.kp = -1.0;
        } else {
            request.kd = -1.0;
        }
        if supervisor.filter_mit_command(request, &pitch).is_ok() {
            accepted.push(format!("negative gain field={field}"));
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted invalid MIT data: {accepted:?}"
    );
}

#[test]
fn nonfinite_legacy_and_speed_requests_are_rejected() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        assert!(
            supervisor
                .filter_speed_command(
                    SpeedCommand {
                        joint: pitch.joint.clone(),
                        velocity_rad_s: value
                    },
                    &pitch
                )
                .is_err(),
            "speed {value} accepted"
        );
        for field in 0..3 {
            let mut request = JointCommand {
                joint: pitch.joint.clone(),
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_nm: 0.1,
            };
            match field {
                0 => request.position_rad = value,
                1 => request.velocity_rad_s = value,
                _ => request.torque_nm = value,
            }
            assert!(
                supervisor.filter_command(request).is_err(),
                "legacy field{field}={value} accepted"
            );
        }
    }
}

#[test]
fn rejected_batch_emits_nothing_and_does_not_advance_output_history() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    let roll = motor(&supervisor, "right_shoulder_roll");
    activate(&mut supervisor, &[pitch.clone(), roll.clone()]);
    receive(&mut supervisor, &[pitch.clone(), roll.clone()]);
    let mut first = command(&pitch);
    first.torque_ff_nm = 5.0;
    let mut invalid = command(&roll);
    invalid.velocity_rad_s = 100.0;
    supervisor.bus_mut().clear_trace();
    assert!(supervisor
        .send_mit_batch(vec![first.clone(), invalid])
        .is_err());
    assert!(
        supervisor.bus().frames().is_empty(),
        "partial batch motion escaped admission"
    );
    supervisor.send_mit_batch(vec![first]).expect("valid retry");
    let frame = supervisor.bus().frames().last().expect("MIT retry");
    let torque =
        (f64::from((frame.id >> 8) & 0xffff) / 32767.0 - 1.0) * 60.0 * f64::from(pitch.direction);
    assert!(
        torque.abs() <= 0.602,
        "rejected batch advanced slew history: {torque}"
    );
}

#[test]
fn invalid_motor_transform_is_rejected_before_any_batch_frame() {
    for gear_ratio in [1e40, 1e-200, f64::NAN] {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        let roll = motor(&supervisor, "right_shoulder_roll");
        activate(&mut supervisor, &[pitch.clone(), roll.clone()]);
        receive(&mut supervisor, &[pitch.clone(), roll.clone()]);
        supervisor
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == pitch.joint)
            .expect("pitch")
            .gear_ratio = gear_ratio;
        let mut invalid = command(&pitch);
        invalid.position_rad = 1.0;
        invalid.kp = 1.0;
        supervisor.bus_mut().clear_trace();
        assert!(
            supervisor
                .send_mit_batch(vec![command(&roll), invalid])
                .is_err(),
            "gear={gear_ratio} emitted a nonfinite wire transform"
        );
        assert_only_stop_traffic(supervisor.bus().frames());
        assert_eq!(supervisor.mode(), davout::OperationalMode::Disabled);
    }
}

#[test]
fn nonfinite_feedback_is_rejected_before_cache_or_freshness_update() {
    for field in 0..4 {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut supervisor = supervisor();
            let pitch = motor(&supervisor, "right_shoulder_pitch");
            activate(&mut supervisor, std::slice::from_ref(&pitch));
            let mut sample = status(Instant::now());
            match field {
                0 => sample.position_rad = value,
                1 => sample.velocity_rad_s = value,
                2 => sample.torque_nm = value,
                _ => sample.temperature_c = value,
            }
            inject(&mut supervisor, MotorAddress::from(&pitch), sample);
            assert!(
                supervisor.drain_feedback().is_err(),
                "invalid feedback field{field}={value} accepted"
            );
            assert!(supervisor.joint_feedback(&pitch.joint).is_none());
            supervisor.bus_mut().clear_trace();
            assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
            assert!(supervisor.bus().frames().is_empty());
        }
    }
}

#[test]
fn new_valid_diagnostics_cannot_clear_an_invalid_feedback_fault() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    let original = Instant::now();
    inject(
        &mut supervisor,
        MotorAddress::from(&pitch),
        status(original),
    );
    supervisor.drain_feedback().expect("initial valid pose");
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("initial motion");

    let mut invalid = status(Instant::now());
    invalid.torque_nm = f32::NAN;
    inject(&mut supervisor, MotorAddress::from(&pitch), invalid);
    assert!(supervisor.drain_feedback().is_err());
    assert!(supervisor.joint_feedback(&pitch.joint).is_none());
    supervisor.bus_mut().clear_trace();
    assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
    assert!(supervisor.bus().frames().is_empty());

    inject(
        &mut supervisor,
        MotorAddress::from(&pitch),
        status(original),
    );
    supervisor
        .drain_feedback()
        .expect("old valid replay ignored");
    assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
    assert!(supervisor.bus().frames().is_empty());

    receive(&mut supervisor, std::slice::from_ref(&pitch));
    assert!(
        supervisor.joint_feedback(&pitch.joint).is_some(),
        "new valid diagnostic pose"
    );
    assert!(supervisor.send_mit_batch(vec![command(&pitch)]).is_err());
    assert!(supervisor.has_latched_fault());
    assert!(supervisor.bus().frames().is_empty());
}

#[test]
fn replayed_sample_cannot_poison_current_pose_or_velocity_policy() {
    let mut supervisor = supervisor();
    let pitch = motor(&supervisor, "right_shoulder_pitch");
    activate(&mut supervisor, std::slice::from_ref(&pitch));
    let original = Instant::now();
    inject(
        &mut supervisor,
        MotorAddress::from(&pitch),
        status(original),
    );
    supervisor.drain_feedback().expect("initial valid pose");
    receive(&mut supervisor, std::slice::from_ref(&pitch));

    let mut replay = status(original);
    replay.position_rad = 0.01; // Safe old pose must not replace current pose or mutate derivative trips.
    replay.velocity_rad_s = 50.0;
    inject(&mut supervisor, MotorAddress::from(&pitch), replay);
    supervisor.drain_feedback().expect("older pose is ignored");
    let pose = supervisor
        .joint_feedback(&pitch.joint)
        .expect("current pose");
    assert_eq!(pose.position_rad, 0.0);
    assert_eq!(pose.velocity_rad_s, 0.0);
    supervisor.bus_mut().clear_trace();
    supervisor
        .send_mit_batch(vec![command(&pitch)])
        .expect("replay did not trip current position/velocity policy");
    assert_eq!(supervisor.bus().frames().len(), 1);
}

#[test]
fn invalid_speed_admission_emits_no_run_mode_or_parameter_writes() {
    for (velocity, gear_ratio) in [
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (f64::NEG_INFINITY, 1.0),
        (0.1, 1e40),
    ] {
        let mut supervisor = supervisor();
        let pitch = motor(&supervisor, "right_shoulder_pitch");
        activate(&mut supervisor, std::slice::from_ref(&pitch));
        receive(&mut supervisor, std::slice::from_ref(&pitch));
        supervisor.control.control.bench.allow_firmware_speed_mode = true;
        supervisor
            .motors
            .motors
            .iter_mut()
            .find(|motor| motor.joint == pitch.joint)
            .expect("pitch")
            .gear_ratio = gear_ratio;
        supervisor.bus_mut().clear_trace();
        assert!(supervisor
            .send_speed_command(SpeedCommand {
                joint: pitch.joint,
                velocity_rad_s: velocity,
            })
            .is_err());
        if gear_ratio == 1.0 {
            assert!(supervisor.bus().frames().is_empty());
        } else {
            assert_only_stop_traffic(supervisor.bus().frames());
            assert_eq!(supervisor.mode(), davout::OperationalMode::Disabled);
        }
    }
}
