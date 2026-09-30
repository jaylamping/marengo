#![allow(clippy::expect_used)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use robstride::{
    AddressedMitCommand, BusError, CanBus, CanFrame, CommandError, CommandField, CommunicationType,
    MemoryBus, MitCommand, MotorAddress, MotorBus, MotorState, ParameterId, ParameterKind,
    ParameterValue, RunMode,
};

fn command(device_id: u8, motor_type: MotorType) -> MitCommand {
    MitCommand {
        device_id,
        motor_type,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        kp: 0.0,
        kd: 0.0,
        torque_ff_nm: 0.0,
    }
}

#[test]
fn nan_torque_batch_is_rejected_before_any_motor_frame() {
    let mut bus = MemoryBus::default();
    let good = command(1, MotorType::Rs03);
    let mut bad = command(2, MotorType::Rs03);
    bad.torque_ff_nm = f32::NAN;
    let result = bus.mit_control_all(&[good, bad]);
    let last = bus.tx.last().map(|frame| {
        let torque_word = (frame.id >> 8) & 0xffff;
        (torque_word, (f64::from(torque_word) / 32767.0 - 1.0) * 60.0)
    });
    assert!(
        result.is_err(),
        "NaN torque must be rejected: emitted {} frames; final raw torque/Nm={last:?}",
        bus.tx.len()
    );
    assert!(
        bus.tx.is_empty(),
        "valid prefix must not escape an invalid batch"
    );
}

#[test]
fn every_nonfinite_mit_field_and_negative_gain_is_rejected() {
    for model in [
        MotorType::Rs00,
        MotorType::Rs02,
        MotorType::Rs03,
        MotorType::Rs04,
    ] {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for field in 0..5 {
                let mut bad = command(2, model);
                match field {
                    0 => bad.position_rad = invalid,
                    1 => bad.velocity_rad_s = invalid,
                    2 => bad.kp = invalid,
                    3 => bad.kd = invalid,
                    _ => bad.torque_ff_nm = invalid,
                }
                let mut bus = MemoryBus::default();
                assert!(
                    bus.mit_control_all(&[command(1, model), bad]).is_err(),
                    "model={model:?}, field={field}, value={invalid}"
                );
                assert!(bus.tx.is_empty());
                let addressed = [
                    AddressedMitCommand {
                        address: MotorAddress::new("can0", 1),
                        command: command(1, model),
                    },
                    AddressedMitCommand {
                        address: MotorAddress::new("can1", 2),
                        command: bad,
                    },
                ];
                assert!(bus.mit_control_all_at(&addressed).is_err());
                assert!(bus.tx.is_empty());
            }
        }
        for gain in 0..2 {
            let mut bad = command(2, model);
            if gain == 0 {
                bad.kp = -1.0;
            } else {
                bad.kd = -1.0;
            }
            let mut bus = MemoryBus::default();
            assert!(bus.mit_control_all(&[command(1, model), bad]).is_err());
            assert!(bus.tx.is_empty());
        }
    }
}

#[derive(Default)]
struct RecordingBus {
    configured: Vec<MotorAddress>,
    sent: Vec<(MotorAddress, CanFrame)>,
}

impl CanBus for RecordingBus {
    fn validate_address(&self, address: &MotorAddress) -> Result<(), BusError> {
        if !self.configured.contains(address) {
            return Err(BusError::UnknownMotorAddress {
                interface: address.interface.clone(),
                device_id: address.device_id,
            });
        }
        Ok(())
    }

    fn send_frame(&mut self, _frame: &CanFrame) -> Result<(), BusError> {
        Err(BusError::Driver(
            "this test adapter requires routing".into(),
        ))
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.validate_address(address)?;
        self.sent.push((address.clone(), frame.clone()));
        Ok(())
    }
}

impl MotorBus for RecordingBus {}

#[test]
fn addressed_batch_preflights_all_routes_and_preserves_distinct_interfaces() {
    let can0 = MotorAddress::new("can0", 1);
    let can1 = MotorAddress::new("can1", 1);
    let first = AddressedMitCommand {
        address: can0.clone(),
        command: command(1, MotorType::Rs03),
    };
    let second = AddressedMitCommand {
        address: can1.clone(),
        command: command(1, MotorType::Rs03),
    };
    let mut bus = RecordingBus {
        configured: vec![can0.clone()],
        ..RecordingBus::default()
    };
    assert!(matches!(
        bus.mit_control_all_at(&[first.clone(), second.clone()]),
        Err(BusError::UnknownMotorAddress { device_id: 1, .. })
    ));
    assert!(
        bus.sent.is_empty(),
        "bad later route must not emit the valid prefix"
    );
    bus.configured.push(can1.clone());
    bus.mit_control_all_at(&[first, second])
        .expect("distinct physical motors");
    assert_eq!(bus.sent.len(), 2);
    assert_eq!(bus.sent[0].0, can0);
    assert_eq!(bus.sent[1].0, can1);
    assert_eq!(bus.sent[0].1.id & 0xff, 1);
    assert_eq!(bus.sent[1].1.id & 0xff, 1);
}

#[test]
fn public_encoders_report_invalid_field_and_register_kind() {
    let mut invalid = command(2, MotorType::Rs03);
    invalid.torque_ff_nm = f32::NAN;
    assert_eq!(
        robstride::encode_mit(&invalid),
        Err(CommandError::NonFinite {
            device_id: 2,
            field: CommandField::TorqueFeedforward
        })
    );
    invalid.torque_ff_nm = 0.0;
    invalid.kp = -1.0;
    assert_eq!(
        invalid.validate(),
        Err(CommandError::NegativeGain {
            device_id: 2,
            field: CommandField::ProportionalGain
        })
    );
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(robstride::encode_speed_ref(2, value).is_err());
        assert!(robstride::encode_position_ref(2, value).is_err());
        assert!(robstride::encode_current_ref(2, value).is_err());
    }
    assert_eq!(
        robstride::encode_write_parameter(
            0xfd,
            2,
            ParameterId::CanTimeout,
            ParameterValue::F32(1.0)
        ),
        Err(CommandError::ParameterType {
            device_id: 2,
            parameter: ParameterId::CanTimeout,
            expected: ParameterKind::U32,
            actual: ParameterKind::F32,
        })
    );
}

#[test]
fn unsupported_raw_run_modes_never_reach_the_bus() {
    let address = MotorAddress::new("can0", 2);
    // PP mode 5 exists in a newer vendor manual, but this driver implements only 0..=3.
    for mode in 4..=u8::MAX {
        let mut bus = MemoryBus::default();
        let unaddressed = bus.write_parameter(2, ParameterId::RunMode, ParameterValue::U8(mode));
        let addressed =
            bus.write_parameter_at(&address, ParameterId::RunMode, ParameterValue::U8(mode));
        assert!(bus.tx.is_empty(), "unsupported mode {mode} emitted a frame");
        assert!(matches!(
            unaddressed,
            Err(BusError::InvalidCommand(CommandError::UnsupportedRunMode {
                device_id: 2,
                value
            })) if value == mode
        ));
        assert!(matches!(
            addressed,
            Err(BusError::InvalidCommand(CommandError::UnsupportedRunMode {
                device_id: 2,
                value
            })) if value == mode
        ));
        assert_eq!(
            robstride::encode_write_parameter(
                0xfd,
                2,
                ParameterId::RunMode,
                ParameterValue::U8(mode)
            ),
            Err(CommandError::UnsupportedRunMode {
                device_id: 2,
                value: mode,
            })
        );
    }
}

#[test]
fn supported_raw_and_typed_run_modes_preserve_wire_values() {
    let address = MotorAddress::new("can0", 2);
    for (typed, byte) in [
        (RunMode::Mit, 0),
        (RunMode::Position, 1),
        (RunMode::Speed, 2),
        (RunMode::Current, 3),
    ] {
        let expected = [0x05, 0x70, 0, 0, byte, 0, 0, 0];
        let mut bus = MemoryBus::default();
        bus.write_parameter(2, ParameterId::RunMode, ParameterValue::U8(byte))
            .expect("supported raw mode");
        bus.write_parameter_at(&address, ParameterId::RunMode, ParameterValue::U8(byte))
            .expect("supported addressed raw mode");
        bus.set_run_mode(2, typed).expect("supported typed mode");
        assert_eq!(bus.tx.len(), 3);
        for frame in bus.tx {
            assert_eq!(frame.id, 0x1200fd02);
            assert_eq!(frame.data, expected);
        }
    }
}

#[test]
fn firmware_register_writes_validate_kind_sign_and_exact_payload() {
    let address = MotorAddress::new("can0", 1);
    for parameter in [
        ParameterId::PositionKp,
        ParameterId::SpeedKp,
        ParameterId::SpeedKi,
        ParameterId::LimitSpeed,
        ParameterId::LimitTorque,
        ParameterId::EPScanTime,
        ParameterId::CanTimeout,
    ] {
        let mut bus = MemoryBus::default();
        assert!(bus
            .write_parameter(1, parameter, ParameterValue::F32(-1.0))
            .is_err());
        assert!(bus
            .write_parameter_at(&address, parameter, ParameterValue::F32(-1.0))
            .is_err());
        assert!(bus.tx.is_empty());
    }
    for (parameter, value) in [
        (ParameterId::RunMode, ParameterValue::F32(1.0)),
        (ParameterId::PositionTarget, ParameterValue::U8(1)),
        (ParameterId::EPScanTime, ParameterValue::U32(1)),
        (ParameterId::CanTimeout, ParameterValue::U16(1)),
    ] {
        let mut bus = MemoryBus::default();
        assert!(matches!(
            bus.write_parameter(1, parameter, value),
            Err(BusError::InvalidCommand(CommandError::ParameterType { .. }))
        ));
        assert!(bus.tx.is_empty());
    }
    let mut bus = MemoryBus::default();
    bus.write_parameter(1, ParameterId::EPScanTime, ParameterValue::U16(0x1234))
        .expect("report interval");
    bus.write_parameter(1, ParameterId::CanTimeout, ParameterValue::U32(20000))
        .expect("timeout threshold");
    assert_eq!(bus.tx[0].id, 0x1200fd01);
    assert_eq!(bus.tx[0].data, [0x26, 0x70, 0, 0, 0x34, 0x12, 0, 0]);
    assert_eq!(bus.tx[1].data, [0x28, 0x70, 0, 0, 0x20, 0x4e, 0, 0]);
    for (parameter, encode) in [
        (
            ParameterId::SpeedTarget,
            robstride::encode_speed_ref as fn(u8, f32) -> Result<(u32, [u8; 8]), CommandError>,
        ),
        (ParameterId::PositionTarget, robstride::encode_position_ref),
        (ParameterId::CurrentTarget, robstride::encode_current_ref),
    ] {
        let (id, data) = encode(1, -1.25).expect("signed finite target");
        assert_eq!(id, 0x1200fd01);
        assert_eq!(&data[..2], &parameter.as_u16().to_le_bytes());
        assert_eq!(&data[4..8], &[0, 0, 0xa0, 0xbf]);
    }
}

#[test]
fn addressed_mit_batch_rejects_mismatched_and_duplicate_identity() {
    let good = AddressedMitCommand {
        address: MotorAddress::new("can0", 1),
        command: command(1, MotorType::Rs03),
    };
    let mismatched = AddressedMitCommand {
        address: MotorAddress::new("can1", 2),
        command: command(3, MotorType::Rs03),
    };
    let mut bus = MemoryBus::default();
    assert!(bus.mit_control_all_at(&[good.clone(), mismatched]).is_err());
    assert!(bus.tx.is_empty());
    assert!(bus.mit_control_all_at(&[good.clone(), good]).is_err());
    assert!(bus.tx.is_empty());
}

#[test]
fn nonfinite_firmware_float_writes_and_speed_emit_nothing() {
    let address = MotorAddress::new("can0", 1);
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for parameter in [
            ParameterId::CurrentTarget,
            ParameterId::SpeedTarget,
            ParameterId::PositionTarget,
            ParameterId::PositionKp,
            ParameterId::LimitTorque,
        ] {
            let mut bus = MemoryBus::default();
            assert!(bus
                .write_parameter(1, parameter, ParameterValue::F32(value))
                .is_err());
            assert!(bus
                .write_parameter_at(&address, parameter, ParameterValue::F32(value))
                .is_err());
            assert!(bus.tx.is_empty());
        }
        let mut bus = MemoryBus::default();
        assert!(bus.speed_control(1, value).is_err());
        assert!(bus.speed_control_at(&address, value).is_err());
        assert!(bus.tx.is_empty());
    }
}

#[test]
fn finite_rs03_command_matches_independent_wire_fixture() {
    let mut bus = MemoryBus::default();
    let mut request = command(1, MotorType::Rs03);
    request.kp = 2500.0;
    request.kd = 50.0;
    request.torque_ff_nm = 30.0;
    bus.mit_control_all(&[request]).expect("finite command");
    assert_eq!(bus.tx.len(), 1);
    assert_eq!(bus.tx[0].id, 0x01bfff01);
    assert_eq!(bus.tx[0].data, [0x7f, 0xff, 0x7f, 0xff, 0x80, 0, 0x80, 0]);
    assert!(bus.tx[0].extended);
    for (model, kp, kd, torque, speed) in [
        (MotorType::Rs00, 250.0, 2.5, 8.5, 25.0),
        (MotorType::Rs02, 250.0, 2.5, 8.5, 22.0),
        (MotorType::Rs03, 2500.0, 50.0, 30.0, 25.0),
        (MotorType::Rs04, 2500.0, 50.0, 60.0, 7.5),
    ] {
        let request = MitCommand {
            device_id: 1,
            motor_type: model,
            position_rad: 2.0 * std::f32::consts::PI,
            velocity_rad_s: speed,
            kp,
            kd,
            torque_ff_nm: torque,
        };
        let (id, data) = robstride::encode_mit(&request).expect("half-scale command");
        assert_eq!(id, 0x01bfff01, "model={model:?}");
        assert_eq!(
            data,
            [0xbf, 0xff, 0xbf, 0xff, 0x80, 0, 0x80, 0],
            "model={model:?}"
        );
    }
}

#[test]
fn fault_only_rx_neither_refreshes_nor_creates_pose_evidence() {
    let address = MotorAddress::new("can0", 1);
    let motor_types = HashMap::from([(address.clone(), MotorType::Rs03)]);
    let stamp = Instant::now() - Duration::from_millis(100);
    let pose = MotorState {
        position_rad: 0.5,
        updated: Some(stamp),
        ..MotorState::default()
    };
    let fault_frame = robstride::CanFrame {
        id: robstride::pack_ext_id(CommunicationType::FaultReport.as_u8(), 1, 0xfd),
        data: [1, 0, 0, 0, 0, 0, 0, 0],
        extended: true,
    };
    let mut bus = MemoryBus::default();
    let mut states = HashMap::from([(address.clone(), pose)]);
    bus.rx_queue.push(fault_frame.clone());
    assert_eq!(
        bus.recv_all_addressed(&motor_types, &mut states, Duration::ZERO, Duration::ZERO)
            .expect("fault receive"),
        1
    );
    assert_eq!(states[&address].position_rad, 0.5);
    assert_eq!(states[&address].fault, 1);
    assert_eq!(
        states[&address].updated,
        Some(stamp),
        "fault is not a new pose sample"
    );
    states.clear();
    bus.rx_queue.push(fault_frame);
    bus.recv_all_addressed(&motor_types, &mut states, Duration::ZERO, Duration::ZERO)
        .expect("fault without pose");
    assert_eq!(states[&address].updated, None);
}
