//! Safety contracts observed at the outgoing CAN boundary, without a physical drive.

#![allow(clippy::expect_used)]

use davout::{MemoryBus, MitJointCommand, Supervisor};
use marengo_config::MotorEntry;
use robstride::{CanFrame, CommunicationType};

fn supervisor() -> Supervisor<MemoryBus> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    Supervisor::from_repo(root, MemoryBus::default()).expect("valid repository fixture")
}

fn activate(supervisor: &mut Supervisor<MemoryBus>, motor: &MotorEntry) {
    supervisor
        .homing_registry_mut()
        .bench_mark_all_verified(std::slice::from_ref(motor))
        .expect("test-only reference verification");
    supervisor
        .enable_targets(std::slice::from_ref(&motor.joint))
        .expect("enable recording bus");
    supervisor.bus_mut().tx.clear();
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

fn pitch_motor(supervisor: &Supervisor<MemoryBus>) -> MotorEntry {
    supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == "right_shoulder_pitch")
        .expect("RS03 shoulder fixture")
        .clone()
}

fn inject_measured_torque(
    supervisor: &mut Supervisor<MemoryBus>,
    motor: &MotorEntry,
    joint_torque_nm: f64,
) {
    inject_status(supervisor, motor, 0.0, 0.0, joint_torque_nm);
}

fn inject_status(
    supervisor: &mut Supervisor<MemoryBus>,
    motor: &MotorEntry,
    joint_position_rad: f64,
    joint_velocity_rad_s: f64,
    joint_torque_nm: f64,
) {
    // RS03 vendor status uses signed ±60 Nm mapped around 0x7fff. Construct the
    // documented bytes directly: an encoder/decoder roundtrip would hide scale errors.
    let scale = f64::from(motor.direction) * motor.gear_ratio;
    let raw_torque = ((joint_torque_nm / scale / 60.0 + 1.0) * 32767.0).round() as u16;
    let mut data = [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0xc8];
    let raw_position = ((joint_position_rad * scale / (4.0 * std::f64::consts::PI) + 1.0) * 32767.0)
        .round() as u16;
    let raw_velocity = ((joint_velocity_rad_s * scale / 50.0 + 1.0) * 32767.0).round() as u16;
    data[0..2].copy_from_slice(&raw_position.to_be_bytes());
    data[2..4].copy_from_slice(&raw_velocity.to_be_bytes());
    data[4..6].copy_from_slice(&raw_torque.to_be_bytes());
    supervisor.bus_mut().rx_queue.push(CanFrame {
        id: (2 << 24) | (u32::from(motor.device_id) << 8) | 0xfd,
        data,
        extended: true,
    });
    assert_eq!(supervisor.drain_feedback().expect("decode status"), 1);
}

#[test]
fn torque_output_contract_danger_zone_cap_overrides_previous_torque() {
    for previous in [-4.0, 4.0] {
        let mut supervisor = supervisor();
        let motor = pitch_motor(&supervisor);
        activate(&mut supervisor, &motor);
        inject_status(&mut supervisor, &motor, 1.0, -0.2, previous);
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

fn last_wire_joint_torque(supervisor: &mut Supervisor<MemoryBus>, motor: &MotorEntry) -> f64 {
    let frame = supervisor
        .bus_mut()
        .tx
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
        for cap in [5.0, 0.2] {
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
