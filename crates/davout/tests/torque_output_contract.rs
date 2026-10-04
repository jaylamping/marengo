//! Safety contracts observed at the outgoing CAN boundary, without a physical drive.

#![allow(clippy::expect_used)]
use davout::simulation::SimulationBus;

use davout::{MitJointCommand, Supervisor};
use marengo_config::MotorEntry;
use robstride::{CanFrame, CommunicationType};

fn supervisor() -> Supervisor<SimulationBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut supervisor = Supervisor::from_simulation(
        root,
        SimulationBus::default(),
        davout::simulation::InitialVirtualReference::AllConfigured,
    )
    .expect("valid repository fixture");
    supervisor.control.control.comm_watchdog_ms = 1000;
    supervisor
}

fn activate(supervisor: &mut Supervisor<SimulationBus>, motor: &MotorEntry) {
    activate_at_pose(supervisor, motor, 0.0, 0.0, 0.0);
}

fn activate_at_pose(
    supervisor: &mut Supervisor<SimulationBus>,
    motor: &MotorEntry,
    position: f64,
    velocity: f64,
    torque: f64,
) {
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("enable recording bus");
    // Obtain status in this enable session without seeding the torque limiter.
    inject_status(supervisor, motor, position, velocity, torque);
    supervisor.bus_mut().clear_trace();
}

fn command(motor: &MotorEntry, torque_ff_nm: f64) -> MitJointCommand {
    MitJointCommand {
        joint: motor.joint.clone(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm,
    }
}

fn pitch_motor(supervisor: &Supervisor<SimulationBus>) -> MotorEntry {
    supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_shoulder_pitch")
        .expect("RS03 shoulder fixture")
        .clone()
}

fn inject_measured_torque(
    supervisor: &mut Supervisor<SimulationBus>,
    motor: &MotorEntry,
    joint_torque_nm: f64,
) {
    inject_status(supervisor, motor, 0.0, 0.0, joint_torque_nm);
}

fn inject_status(
    supervisor: &mut Supervisor<SimulationBus>,
    motor: &MotorEntry,
    joint_position_rad: f64,
    joint_velocity_rad_s: f64,
    joint_torque_nm: f64,
) {
    // RS03 vendor status uses signed ±60 Nm and ±20 rad/s mapped around 0x7fff. Construct the
    // documented bytes directly: an encoder/decoder roundtrip would hide scale errors.
    let scale = f64::from(motor.direction) * motor.gear_ratio;
    let raw_torque = ((joint_torque_nm / scale / 60.0 + 1.0) * 32767.0).round() as u16;
    let mut data = [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0xc8];
    let raw_position = ((joint_position_rad * scale / (4.0 * std::f64::consts::PI) + 1.0) * 32767.0)
        .round() as u16;
    let raw_velocity = ((joint_velocity_rad_s * scale / 20.0 + 1.0) * 32767.0).round() as u16;
    data[0..2].copy_from_slice(&raw_position.to_be_bytes());
    data[2..4].copy_from_slice(&raw_velocity.to_be_bytes());
    data[4..6].copy_from_slice(&raw_torque.to_be_bytes());
    supervisor
        .bus_mut()
        .queue_frame(CanFrame {
            id: (2 << 24) | (2 << 22) | (u32::from(motor.device_id) << 8) | 0xfd,
            data,
            extended: true,
        })
        .expect("finite closed script");
    assert_eq!(supervisor.drain_feedback().expect("decode status"), 1);
}

#[test]
fn torque_output_contract_danger_zone_cap_overrides_previous_torque() {
    for previous in [-4.0, 4.0] {
        let mut supervisor = supervisor();
        let motor = pitch_motor(&supervisor);
        activate_at_pose(&mut supervisor, &motor, 1.0, -0.2, previous);
        supervisor.seed_tau_ff_rate_limiter();
        supervisor.control.control.danger_zones = vec![marengo_config::DangerZoneRule {
            name: "test-measured-descent".into(),
            joint: motor.joint.clone(),
            position_above_rad: 0.5,
            velocity_below_rad_s: -0.1,
            action: "clamp_torque".into(),
            max_velocity_rad_s: 0.1,
            max_torque_nm: Some(0.2),
        }];
        let mut request = command(&motor, previous);
        request.position_rad = 1.0;
        supervisor.send_mit_batch(vec![request]).expect("send");
        let output = last_wire_joint_torque(&mut supervisor, &motor);
        assert!(
            output.abs() <= 0.202,
            "danger-zone cap allowed {output} Nm from previous {previous} Nm"
        );
    }
}

#[test]
fn torque_output_contract_delayed_tick_does_not_accumulate_slew_credit() {
    let mut supervisor = supervisor();
    // This test isolates output slew from the independently tested RX watchdog.
    // Allow scheduler delay without making a future per-drive watchdog repair flaky.
    supervisor.control.control.comm_watchdog_ms = 1000;
    let motor = pitch_motor(&supervisor);
    activate(&mut supervisor, &motor);
    inject_measured_torque(&mut supervisor, &motor, 0.0);
    supervisor
        .send_mit_batch(vec![command(&motor, 0.0)])
        .expect("establish zero output");
    std::thread::sleep(std::time::Duration::from_millis(30));
    supervisor
        .send_mit_batch(vec![command(&motor, 5.0)])
        .expect("command before watchdog deadline");
    let output = last_wire_joint_torque(&mut supervisor, &motor);
    let max_step = supervisor.control.control.tau_ff_rate_limit_nm_per_s * 0.01;
    assert!(
        output <= max_step + 0.002,
        "delayed tick accumulated a {output} Nm step above the {max_step} Nm tick bound"
    );
}

fn last_wire_joint_torque(supervisor: &mut Supervisor<SimulationBus>, motor: &MotorEntry) -> f64 {
    let frame = supervisor
        .bus()
        .frames()
        .iter()
        .rev()
        .find(|frame| {
            frame.id >> 24 == u32::from(CommunicationType::OperationControl.as_u8())
                && frame.id & 0xff == u32::from(motor.device_id)
        })
        .expect("outgoing MIT frame");
    // Independent wire oracle: FF lives in ID bits 8..23, not the payload.
    let raw_torque = (frame.id >> 8) & 0xffff;
    let motor_torque = (f64::from(raw_torque) / 32767.0 - 1.0) * 60.0;
    motor_torque * f64::from(motor.direction) * motor.gear_ratio
}

#[test]
fn torque_output_contract_measured_seed_and_reduced_cap_never_exceed_bound() {
    for measured in [-8.0, 8.0] {
        // The reduced cap stays above master pitch static friction (fs 0.65 Nm): config
        // validation refuses a τ_ff cap below a joint's static friction.
        for cap in [5.0, 0.7] {
            let mut supervisor = supervisor();
            let motor = pitch_motor(&supervisor);
            activate(&mut supervisor, &motor);
            inject_measured_torque(&mut supervisor, &motor, measured);
            supervisor.seed_tau_ff_rate_limiter();
            // A mode/config transition may reduce a cap after the seed was taken.
            supervisor
                .control
                .control
                .motor_type_defaults
                .get_mut("rs03")
                .expect("RS03 defaults")
                .tau_ff_max_nm = cap;
            supervisor
                .send_mit_batch(vec![command(&motor, 0.0)])
                .expect("valid MIT command");
            let output = last_wire_joint_torque(&mut supervisor, &motor);
            assert!(
                output.abs() <= cap + 0.002,
                "measured seed {measured} Nm produced {output} Nm above {cap} Nm cap"
            );
        }
    }
}

#[test]
fn torque_output_contract_first_activation_slews_from_zero() {
    for requested in [-5.0, 5.0] {
        let mut supervisor = supervisor();
        let motor = pitch_motor(&supervisor);
        activate(&mut supervisor, &motor);
        supervisor
            .send_mit_batch(vec![command(&motor, requested)])
            .expect("valid MIT command");
        let output = last_wire_joint_torque(&mut supervisor, &motor);
        let max_first_step = supervisor.control.control.tau_ff_rate_limit_nm_per_s * 0.01;
        assert!(
            output.abs() <= max_first_step + 0.002,
            "first activation passed {output} Nm instead of slewing from zero by {max_first_step} Nm"
        );
        assert!(
            output * requested > 0.0,
            "torque must start toward the request"
        );
    }
}

#[test]
fn torque_output_contract_disable_reenable_discards_old_torque() {
    for previous in [-4.0, 4.0] {
        let mut supervisor = supervisor();
        let motor = pitch_motor(&supervisor);
        activate(&mut supervisor, &motor);
        inject_measured_torque(&mut supervisor, &motor, previous);
        supervisor.seed_tau_ff_rate_limiter();
        supervisor
            .send_mit_batch(vec![command(&motor, previous)])
            .expect("establish previous output");
        assert!((last_wire_joint_torque(&mut supervisor, &motor) - previous).abs() < 0.002);

        supervisor.disable_all().expect("disable recording bus");
        activate(&mut supervisor, &motor);
        supervisor
            .send_mit_batch(vec![command(&motor, 0.0)])
            .expect("neutral command after re-enable");
        let output = last_wire_joint_torque(&mut supervisor, &motor);
        assert!(
            output.abs() < 0.002,
            "disable/re-enable replayed {output} Nm from the old {previous} Nm session"
        );
    }
}

#[test]
fn torque_output_contract_drive_damping_is_bounded_and_tau_ff_stays_capped() {
    // ADR 0039 scaled PD sends constant drive kd with v_des = the reference velocity; the
    // damping torque kd·(v_des − dq) is computed in the drive, outside Davout's τ_ff cap.
    for measured in [-8.0, 8.0] {
        let mut supervisor = supervisor();
        let motor = pitch_motor(&supervisor);
        activate(&mut supervisor, &motor);
        inject_measured_torque(&mut supervisor, &motor, measured);
        supervisor.seed_tau_ff_rate_limiter();
        let defaults = supervisor.control.control.motor_type_defaults["rs03"].clone();
        let cap = defaults.tau_ff_max_nm.min(motor.bench.torque_limit_nm);
        let mut request = command(&motor, 10.0 * measured);
        request.kp = 18.0;
        request.kd = 3.0;
        request.velocity_rad_s = 1.0;
        supervisor
            .send_mit_batch(vec![request])
            .expect("drive damping within kd_max");
        let output = last_wire_joint_torque(&mut supervisor, &motor);
        assert!(
            output.abs() <= cap + 0.002,
            "kd > 0 let τ_ff {output} Nm past the {cap} Nm cap"
        );
        // Independent wire oracle: RS03 kd is 0..100 over the 16-bit payload field.
        let frame = supervisor
            .bus()
            .frames()
            .iter()
            .rev()
            .find(|frame| frame.id >> 24 == u32::from(CommunicationType::OperationControl.as_u8()))
            .expect("outgoing MIT frame")
            .clone();
        let wire_kd =
            f64::from(u16::from_be_bytes([frame.data[6], frame.data[7]])) / 65535.0 * 100.0;
        let motor_kd = 3.0 / motor.gear_ratio.powi(2);
        assert!((wire_kd - motor_kd).abs() < 0.01, "wire kd {wire_kd}");
    }

    // Drive kd above the motor-type maximum is refused, never clamped.
    let mut supervisor = supervisor();
    let motor = pitch_motor(&supervisor);
    activate(&mut supervisor, &motor);
    let kd_max = supervisor.control.control.motor_type_defaults["rs03"].kd_max;
    let mut request = command(&motor, 0.0);
    request.kd = 1.01 * kd_max * motor.gear_ratio.powi(2);
    assert!(supervisor.send_mit_batch(vec![request]).is_err());
    assert!(
        supervisor
            .bus()
            .frames()
            .iter()
            .all(|frame| frame.id >> 24 != u32::from(CommunicationType::OperationControl.as_u8())),
        "refused kd reached the wire"
    );
}

/// Joint-space `(q_des, v_des, kp, kd, τ_ff)` of the last pitch MIT frame, decoded with the RS03
/// vendor scales (position ±4π, velocity ±20, kp 0..5000, kd 0..100, torque ±60).
fn last_wire_pitch_command(
    supervisor: &Supervisor<SimulationBus>,
    motor: &MotorEntry,
) -> (f64, f64, f64, f64, f64) {
    let frame = supervisor
        .bus()
        .frames()
        .iter()
        .rev()
        .find(|frame| {
            frame.id >> 24 == u32::from(CommunicationType::OperationControl.as_u8())
                && frame.id & 0xff == u32::from(motor.device_id)
        })
        .expect("outgoing MIT frame");
    let signed = |raw: u16, scale: f64| (f64::from(raw) / 32767.0 - 1.0) * scale;
    let unsigned = |raw: u16, scale: f64| f64::from(raw) / 65535.0 * scale;
    let field = |offset: usize| u16::from_be_bytes([frame.data[offset], frame.data[offset + 1]]);
    let scale = f64::from(motor.direction) * motor.gear_ratio;
    (
        signed(field(0), 4.0 * std::f64::consts::PI) / scale,
        signed(field(2), 20.0) / scale,
        unsigned(field(4), 5000.0) * scale * scale,
        unsigned(field(6), 100.0) * scale * scale,
        signed(((frame.id >> 8) & 0xffff) as u16, 60.0) * scale,
    )
}

#[test]
fn torque_output_contract_predicted_total_torque_is_clamped_continuously() {
    // ADR 0039 open question 1: the drive computes kp·(q_des − q) + kd·(v_des − dq) + τ_ff
    // with no documented clamp. Sweep the position lead through the cap: the predicted total
    // (feedback motion over two 5 ms ticks and a 0.1 rad/s velocity margin included) never
    // exceeds the cap, the clamped torque never steps, and requests under the cap pass as sent.
    let (q, dq, tau_ff) = (0.3, -0.5, 2.0);
    let (kp, kd, v_des) = (18.0, 3.0, 1.0);
    let mut supervisor = supervisor();
    let motor = pitch_motor(&supervisor);
    activate_at_pose(&mut supervisor, &motor, q, dq, tau_ff);
    supervisor.seed_tau_ff_rate_limiter();
    let defaults = supervisor.control.control.motor_type_defaults["rs03"].clone();
    let cap = defaults.tau_ff_max_nm.min(motor.bench.torque_limit_nm);
    let margin = |kp: f64, kd: f64| kp * dq.abs().max(v_des) * 0.01 + kd * 0.1;
    // One wire code of each field: position kp·4π/32767, velocity kd·20/32767, torque 60/32767.
    let wire_tolerance = kp * 4.0e-4 + kd * 7.0e-4 + 2.0e-3;

    let mut previous_total: Option<f64> = None;
    let mut over_cap_requests: u64 = 0;
    for step in 0..=400 {
        let lead = -0.3 + f64::from(step) * 1e-3;
        let request = MitJointCommand {
            joint: motor.joint.clone(),
            kp,
            kd,
            position_rad: q + lead,
            velocity_rad_s: v_des,
            torque_ff_nm: tau_ff,
        };
        let requested_total = kp * lead + kd * (v_des - dq) + tau_ff;
        if requested_total.abs() + margin(kp, kd) > cap {
            over_cap_requests += 1;
        }
        supervisor
            .send_mit_batch(vec![request])
            .expect("a total-torque clamp is not a refusal");
        let (q_w, v_w, kp_w, kd_w, tau_w) = last_wire_pitch_command(&supervisor, &motor);
        let total = kp_w * (q_w - q) + kd_w * (v_w - dq) + tau_w;
        assert!(
            total.abs() + margin(kp_w, kd_w) <= cap + wire_tolerance,
            "lead {lead}: predicted total {} Nm above the {cap} Nm cap",
            total.abs() + margin(kp_w, kd_w)
        );
        assert!(
            (tau_w - tau_ff).abs() < 2.0e-3,
            "τ_ff is never the clamped term"
        );
        if requested_total.abs() + margin(kp, kd) <= cap - wire_tolerance {
            assert!(
                (q_w - (q + lead)).abs() < 4.0e-4 && (v_w - v_des).abs() < 7.0e-4,
                "lead {lead}: a request under the cap was modified"
            );
        }
        if let Some(previous) = previous_total {
            assert!(
                (total - previous).abs() <= kp * 1e-3 + wire_tolerance,
                "lead {lead}: total torque stepped {previous} → {total} Nm"
            );
        }
        previous_total = Some(total);
    }
    assert!(over_cap_requests > 100, "the sweep must cross the cap");
    // Feedback q and dq arrive quantized, so one request at the boundary may fall either way.
    let clamps = supervisor.total_torque_clamp_count(&motor.joint);
    assert!(
        clamps.abs_diff(over_cap_requests) <= 1,
        "{clamps} clamps counted for {over_cap_requests} over-cap requests"
    );
}
