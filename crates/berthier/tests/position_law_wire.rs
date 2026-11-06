//! ADR 0039 wire gate: `control.yaml` `position_law: scaled_pd` reaches the MIT frames.
//!
//! A pitch hold at rest with one-count encoder dither (the bench's at-rest velocity dither) runs
//! through the production ControlLoop → Davout → robstride encode path. The scaled-PD law must
//! send the same kd and v_des in every frame; the legacy law toggles kd.
//! Virtual initial references are initial conditions, not device reference proof.

#![allow(clippy::expect_used)]

#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use berthier::ControlLoop;
use davout::simulation::{InitialVirtualReference, SimulationBus};
use marengo_config::{load_control_config_from, write_control_config_from, PositionLaw};
use robstride::{decode_mit_command_fields, MitCommandFields};

const PITCH: &str = "right_shoulder_pitch";
/// can0 motor 1 (master motors.yaml).
const PITCH_DEVICE: u32 = 1;
const TICKS: usize = 300;

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn select_law(root: &Path, law: PositionLaw) {
    let dir = root.join("config");
    let mut control = load_control_config_from(&dir).expect("fixture control");
    control
        .control
        .joints
        .get_mut(PITCH)
        .expect("pitch entry")
        .position_law = law;
    write_control_config_from(&dir, &control).expect("write fixture control");
}

fn rest_hold_frames(law: PositionLaw) -> Vec<(u16, u16)> {
    let fixture = support::fixture_tree_without_diagnostics("position-law-wire", &source());
    select_law(fixture.path(), law);
    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![PITCH.to_string()]),
        200,
        50,
    )
    .expect("virtual initial reference");
    ctrl.supervisor_mut()
        .enable_targets(&[PITCH.to_string()])
        .expect("scoped enable");
    let queue = |ctrl: &mut ControlLoop<SimulationBus>, q: f64, dq: f64| {
        for joint in ctrl.joint_names().to_vec() {
            let (position, velocity) = if joint == PITCH { (q, dq) } else { (0.0, 0.0) };
            support::queue_joint_status(ctrl.supervisor_mut(), &joint, position, velocity);
        }
    };
    queue(&mut ctrl, 0.0, 0.0);
    ctrl.supervisor_mut().drain_feedback().expect("pose drain");
    ctrl.enter_position_hold_at(Some(PITCH), 0.0)
        .expect("home hold admitted");
    ctrl.supervisor_mut().bus_mut().clear_trace();

    // One RS03 count, toggled on an irregular pattern like a resting encoder.
    let count = f64::from(4.0 * std::f32::consts::PI) / 32767.0;
    let pattern = [0, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 0];
    let mut previous = 0.0;
    for tick in 0..TICKS {
        let q = f64::from(pattern[tick % pattern.len()]) * count;
        queue(&mut ctrl, q, (q - previous) / 0.005);
        previous = q;
        ctrl.tick(None).expect("hold tick");
    }
    ctrl.supervisor()
        .bus()
        .frames()
        .iter()
        .filter(|frame| frame.id & 0xff == PITCH_DEVICE)
        .filter_map(|frame| decode_mit_command_fields(frame.id, &frame.data))
        .map(|fields| (fields.kd, fields.velocity))
        .collect()
}

fn toggles(frames: &[(u16, u16)]) -> usize {
    frames.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

#[test]
fn scaled_pd_sends_constant_kd_and_v_des_at_rest_while_legacy_toggles() {
    let scaled = rest_hold_frames(PositionLaw::ScaledPd);
    assert!(scaled.len() >= TICKS - 2, "one MIT frame per tick");
    assert_eq!(
        toggles(&scaled),
        0,
        "scaled-PD wire kd/v_des changed at rest"
    );
    assert_ne!(scaled[0].0, 0, "drive-side damping is on");

    // The legacy law gates drive kd on the dithering velocity.
    let legacy = rest_hold_frames(PositionLaw::Legacy);
    assert!(legacy.len() >= TICKS - 2, "one MIT frame per tick");
    assert!(toggles(&legacy) > 0, "gate must reject legacy");
}

/// Joint-space `(q_des, v_des, kp, kd, τ_ff)` of an RS03 MIT frame (vendor scales: position
/// ±4π, velocity ±20, kp 0..5000, kd 0..100, torque ±60); `scale` = direction × gear ratio.
fn decode_rs03(fields: MitCommandFields, scale: f64) -> (f64, f64, f64, f64, f64) {
    let signed = |raw: u16, range: f64| (f64::from(raw) / 32767.0 - 1.0) * range;
    let unsigned = |raw: u16, range: f64| f64::from(raw) / 65535.0 * range;
    (
        signed(fields.position, 4.0 * std::f64::consts::PI) / scale,
        signed(fields.velocity, 20.0) / scale,
        unsigned(fields.kp, 5000.0) * scale * scale,
        unsigned(fields.kd, 100.0) * scale * scale,
        signed(fields.torque_ff, 60.0) * scale,
    )
}

#[test]
fn scaled_pd_pitch_never_predicts_total_torque_above_the_cap() {
    // ADR 0039 open question 1. Pitch on `scaled_pd` retargets 0 → 0.75 while the joint is
    // first jammed at 0, then dragged down at 2.4 rad/s (just under the 2.5 rad/s cap): the
    // reference leads by up to e1 + e0 and drive damping alone asks kd·2.4 = 7.2 Nm. Every
    // frame's predicted drive total, from the wire fields and the feedback Davout used, stays
    // within the 5 Nm cap.
    let fixture = support::fixture_tree_without_diagnostics("position-law-total", &source());
    select_law(fixture.path(), PositionLaw::ScaledPd);
    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![PITCH.to_string()]),
        200,
        50,
    )
    .expect("virtual initial reference");
    ctrl.supervisor_mut()
        .enable_targets(&[PITCH.to_string()])
        .expect("scoped enable");
    let queue = |ctrl: &mut ControlLoop<SimulationBus>, q: f64, dq: f64| {
        for joint in ctrl.joint_names().to_vec() {
            let (position, velocity) = if joint == PITCH { (q, dq) } else { (0.0, 0.0) };
            support::queue_joint_status(ctrl.supervisor_mut(), &joint, position, velocity);
        }
    };
    queue(&mut ctrl, 0.0, 0.0);
    ctrl.supervisor_mut().drain_feedback().expect("pose drain");
    ctrl.enter_position_hold_at(Some(PITCH), 0.0)
        .expect("home hold admitted");
    ctrl.tick(None).expect("home hold tick");
    ctrl.enter_position_hold_at(Some(PITCH), 0.75)
        .expect("retarget admitted");

    let supervisor = ctrl.supervisor();
    let motor = supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == PITCH)
        .expect("pitch motor")
        .clone();
    let scale = f64::from(motor.direction) * motor.gear_ratio;
    let cap = supervisor.control.control.motor_type_defaults["rs03"]
        .tau_ff_max_nm
        .min(motor.bench.torque_limit_nm);
    let horizon_s = 2.0 / f64::from(supervisor.control.control.loop_hz);
    // One wire code per field at kp 18, kd 3: position, velocity and torque quantization.
    let wire_tolerance = 18.0 * 4.0e-4 + 3.0 * 7.0e-4 + 2.0e-3;

    const JAMMED_TICKS: usize = 60;
    const DRAGGED_TICKS: usize = 40;
    const DRAG_RAD_S: f64 = -2.4;
    let mut drag_start = None;
    let mut worst = f64::NEG_INFINITY;
    for tick in 0..JAMMED_TICKS + DRAGGED_TICKS {
        let (q, dq) = if tick < JAMMED_TICKS {
            (0.0, 0.0)
        } else {
            // Davout derives dq from positions and host receive times, so the drag runs on the
            // wall clock at the control period.
            std::thread::sleep(std::time::Duration::from_millis(5));
            let start = *drag_start.get_or_insert_with(std::time::Instant::now);
            (DRAG_RAD_S * start.elapsed().as_secs_f64(), DRAG_RAD_S)
        };
        queue(&mut ctrl, q, dq);
        ctrl.tick(None).expect("scaled-PD tick");
        let feedback = ctrl.supervisor().joint_feedback(PITCH).expect("pitch pose");
        let frame = ctrl
            .supervisor()
            .bus()
            .frames()
            .iter()
            .rev()
            .find(|frame| frame.id & 0xff == PITCH_DEVICE)
            .and_then(|frame| decode_mit_command_fields(frame.id, &frame.data))
            .expect("pitch MIT frame this tick");
        let (q_des, v_des, kp, kd, tau_ff) = decode_rs03(frame, scale);
        let (q_fb, dq_fb) = (feedback.position_rad, feedback.velocity_rad_s);
        let total = kp * (q_des - q_fb) + kd * (v_des - dq_fb) + tau_ff;
        let predicted = total.abs() + kp * dq_fb.abs().max(v_des.abs()) * horizon_s + kd * 0.1;
        worst = worst.max(predicted);
        assert!(
            predicted <= cap + wire_tolerance,
            "tick {tick}: q {q_fb:.3} dq {dq_fb:.2} q_des {q_des:.3} v_des {v_des:.2} \
             τ_ff {tau_ff:.2}: predicted {predicted:.2} Nm above the {cap} Nm cap"
        );
    }
    let clamps = ctrl.supervisor().total_torque_clamp_count(PITCH);
    assert!(
        clamps >= DRAGGED_TICKS as u64 / 2,
        "the drag must exceed the cap and be clamped ({clamps} clamps, worst {worst:.2} Nm)"
    );
}

// ---- ADR 0040 item 3: Position entry and retargets keep the sent τ_ff -------------------------

/// Pitch `position_ff_step_max_nm` in master control.yaml.
const FF_STEP_MAX_NM: f64 = 0.048;
/// One RS03 torque code (±60 Nm over 16 bits) of wire quantization.
const WIRE_TORQUE_CODE_NM: f64 = 120.0 / 65535.0;

fn pitch_scaled_loop(label: &str) -> (support::FixtureTree, ControlLoop<SimulationBus>) {
    let fixture = support::fixture_tree_without_diagnostics(label, &source());
    select_law(fixture.path(), PositionLaw::ScaledPd);
    let mut ctrl = ControlLoop::from_simulation(
        fixture.path(),
        SimulationBus::default(),
        InitialVirtualReference::Joints(vec![PITCH.to_string()]),
        200,
        50,
    )
    .expect("virtual initial reference");
    ctrl.supervisor_mut()
        .enable_targets(&[PITCH.to_string()])
        .expect("scoped enable");
    (fixture, ctrl)
}

/// One status per joint; pitch reports `(q, dq)` and a drive torque of `torque` Nm.
fn queue_pitch(ctrl: &mut ControlLoop<SimulationBus>, q: f64, dq: f64, torque: f64) {
    let motors = ctrl.supervisor().motors.motors.clone();
    for motor in motors {
        let frame = if motor.joint == PITCH {
            support::status_with_torque(&motor, q, dq, torque)
        } else {
            support::status(&motor, 0.0, 0.0)
        };
        ctrl.supervisor_mut()
            .bus_mut()
            .queue_frame(frame)
            .expect("finite raw status queue");
    }
}

/// Joint-space `(q_des, τ_ff)` of the latest pitch MIT frame on the wire.
fn last_pitch_wire(ctrl: &ControlLoop<SimulationBus>) -> (f64, f64) {
    let motor = ctrl
        .supervisor()
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == PITCH)
        .expect("pitch motor");
    let scale = f64::from(motor.direction) * motor.gear_ratio;
    let fields = ctrl
        .supervisor()
        .bus()
        .frames()
        .iter()
        .rev()
        .filter(|frame| frame.id & 0xff == PITCH_DEVICE)
        .find_map(|frame| decode_mit_command_fields(frame.id, &frame.data))
        .expect("pitch MIT frame");
    let (q_des, _, _, _, tau_ff) = decode_rs03(fields, scale);
    (q_des, tau_ff)
}

/// GravityComp at `q` until Davout's τ_ff limiter has reached τ_g, the drive reporting the
/// τ_ff it was sent. Returns that τ_ff (Nm).
fn settle_gravity_comp(ctrl: &mut ControlLoop<SimulationBus>, q: f64) -> f64 {
    queue_pitch(ctrl, q, 0.0, 0.0);
    ctrl.supervisor_mut().drain_feedback().expect("pose drain");
    ctrl.try_set_control_mode(berthier::ControlMode::GravityComp)
        .expect("gravity-on");
    let mut sent = 0.0;
    for _ in 0..40 {
        // Davout's τ_ff rate limiter runs on the wall clock.
        std::thread::sleep(std::time::Duration::from_millis(5));
        queue_pitch(ctrl, q, 0.0, sent);
        ctrl.tick(None).expect("gravity-comp tick");
        sent = ctrl
            .supervisor()
            .last_tau_ff_nm(PITCH)
            .expect("GravityComp sent a τ_ff");
    }
    sent
}

#[test]
fn position_entry_from_gravity_comp_starts_from_the_last_sent_tau_ff() {
    // GravityComp → Position (`hold-at`) while the drive reports a total torque 0.5 Nm below the
    // τ_ff Davout sent. The first Position τ_ff on the wire starts from the sent τ_ff, not from
    // the measured torque (ADR 0040 item 3).
    let (_fixture, mut ctrl) = pitch_scaled_loop("position-entry-ff");
    let q = 1.2;
    let sent = settle_gravity_comp(&mut ctrl, q);
    assert!(
        sent.abs() > 1.0,
        "elevated pose carries gravity: τ_ff {sent}"
    );
    let measured = sent - 0.5;
    queue_pitch(&mut ctrl, q, 0.0, measured);
    ctrl.enter_position_hold_at(Some(PITCH), q)
        .expect("hold-at admitted");
    let reported = ctrl
        .supervisor()
        .joint_torque_rad(PITCH)
        .expect("pitch torque");
    assert!(
        (reported - measured).abs() < 0.01,
        "the drive reports {reported}, fixture wants {measured}"
    );
    ctrl.supervisor_mut().bus_mut().clear_trace();
    std::thread::sleep(std::time::Duration::from_millis(5));
    queue_pitch(&mut ctrl, q, 0.0, measured);
    ctrl.tick(None).expect("first Position tick");
    let (_, first) = last_pitch_wire(&ctrl);
    assert!(
        (first - sent).abs() <= FF_STEP_MAX_NM + WIRE_TORQUE_CODE_NM,
        "first Position τ_ff {first:.4} Nm must start from the sent {sent:.4} Nm \
         (measured {measured:.4} Nm)"
    );
}

#[test]
fn mid_move_retarget_never_steps_the_tau_ff_past_the_guard() {
    // A `hold-at` retarget while pitch is moving, with the drive reporting 1 Nm more than the
    // τ_ff Davout last sent (PD effort in the measured total). No sent τ_ff changes by more than
    // the whole-τ_ff step guard from one tick to the next.
    let (_fixture, mut ctrl) = pitch_scaled_loop("retarget-ff");
    let q0 = 0.6;
    settle_gravity_comp(&mut ctrl, q0);
    let sent = ctrl.supervisor().last_tau_ff_nm(PITCH).unwrap_or(0.0);
    queue_pitch(&mut ctrl, q0, 0.0, sent);
    ctrl.enter_position_hold_at(Some(PITCH), q0 + 0.4)
        .expect("move admitted");
    let mut q = q0;
    let mut wire_ff = Vec::new();
    let mut retarget_at = None;
    const RETARGET_TICK: usize = 60;
    for tick in 0..RETARGET_TICK + 30 {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let sent = ctrl.supervisor().last_tau_ff_nm(PITCH).unwrap_or(0.0);
        if tick == RETARGET_TICK {
            let q_ref = ctrl.position_hold_commands().expect("planner")[0];
            assert!(
                q_ref > q0 + 0.02 && q_ref < q0 + 0.38,
                "retarget lands mid-move (q_ref {q_ref})"
            );
            queue_pitch(&mut ctrl, q, 0.0, sent + 1.0);
            ctrl.enter_position_hold_at(Some(PITCH), q0 + 0.6)
                .expect("mid-move retarget admitted");
            retarget_at = Some(wire_ff.len());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        queue_pitch(&mut ctrl, q, 0.0, sent);
        ctrl.tick(None).expect("Position tick");
        let (q_des, tau_ff) = last_pitch_wire(&ctrl);
        wire_ff.push(tau_ff);
        // The drive tracks the commanded setpoint.
        q = q_des;
    }
    let retarget_at = retarget_at.expect("retarget ran");
    for (k, pair) in wire_ff.windows(2).enumerate() {
        let step = (pair[1] - pair[0]).abs();
        assert!(
            step <= FF_STEP_MAX_NM + WIRE_TORQUE_CODE_NM,
            "tick {} (retarget before tick {retarget_at}): τ_ff stepped {step:.4} Nm \
             ({:.4} → {:.4})",
            k + 1,
            pair[0],
            pair[1]
        );
    }
}
