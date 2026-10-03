//! Robstride parameter IDs and read/write frame encoding.

use crate::comm::{pack_typed_ext_id, unpack_ext_id, CommunicationType, DEFAULT_HOST_ID};
use crate::command::{finite, nonnegative, nonnegative_gain, CommandError, CommandField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RunMode {
    Mit = 0,
    Position = 1,
    Speed = 2,
    Current = 3,
}

impl RunMode {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ParameterId {
    RunMode = 0x7005,
    CurrentTarget = 0x7006,
    SpeedTarget = 0x700A,
    PositionTarget = 0x7016,
    LimitSpeed = 0x7017,
    LimitTorque = 0x700B,
    PositionKp = 0x701E,
    SpeedKp = 0x701F,
    SpeedKi = 0x7020,
    EPScanTime = 0x7026,
    CanTimeout = 0x7028,
    /// Load-side mechanical position (rad). Read-only (manual §4.1.14).
    MechPos = 0x7019,
    /// Power-on position range flag: 0 = 0..2π, 1 = -π..π. Volatile unless saved (type 22).
    ZeroSta = 0x7029,
    /// Offset added to the current zero (rad). Volatile unless saved (type 22).
    AddOffset = 0x702B,
}

impl ParameterId {
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn from_u16(index: u16) -> Option<Self> {
        [
            Self::RunMode,
            Self::CurrentTarget,
            Self::SpeedTarget,
            Self::PositionTarget,
            Self::LimitSpeed,
            Self::LimitTorque,
            Self::PositionKp,
            Self::SpeedKp,
            Self::SpeedKi,
            Self::EPScanTime,
            Self::CanTimeout,
            Self::MechPos,
            Self::ZeroSta,
            Self::AddOffset,
        ]
        .into_iter()
        .find(|parameter| parameter.as_u16() == index)
    }

    /// Static register schema supported by this driver, including unsigned timing values.
    /// Installed model and firmware acceptance still require commissioning verification.
    pub fn value_kind(self) -> ParameterKind {
        match self {
            Self::RunMode | Self::ZeroSta => ParameterKind::U8,
            Self::EPScanTime => ParameterKind::U16,
            Self::CanTimeout => ParameterKind::U32,
            _ => ParameterKind::F32,
        }
    }

    /// Vendor read-only registers reject type-18 encoding before transmission.
    pub fn is_read_only(self) -> bool {
        matches!(self, Self::MechPos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    U8,
    U16,
    U32,
    F32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParameterValue {
    U8(u8),
    U16(u16),
    U32(u32),
    F32(f32),
}

impl ParameterValue {
    pub fn kind(self) -> ParameterKind {
        match self {
            Self::U8(_) => ParameterKind::U8,
            Self::U16(_) => ParameterKind::U16,
            Self::U32(_) => ParameterKind::U32,
            Self::F32(_) => ParameterKind::F32,
        }
    }
}

pub fn encode_read_parameter(host_id: u8, device_id: u8, parameter: ParameterId) -> (u32, [u8; 8]) {
    let mut data = [0u8; 8];
    data[..2].copy_from_slice(&parameter.as_u16().to_le_bytes());
    (
        pack_typed_ext_id(
            CommunicationType::ReadParameter,
            u16::from(host_id),
            device_id,
        ),
        data,
    )
}

pub fn encode_write_parameter(
    host_id: u8,
    device_id: u8,
    parameter: ParameterId,
    value: ParameterValue,
) -> Result<(u32, [u8; 8]), CommandError> {
    if matches!(parameter, ParameterId::ZeroSta | ParameterId::AddOffset) {
        return Err(CommandError::ForbiddenParameterWrite {
            device_id,
            parameter,
        });
    }
    if parameter.is_read_only() {
        return Err(CommandError::ReadOnlyParameter {
            device_id,
            parameter,
        });
    }
    if value.kind() != parameter.value_kind() {
        return Err(CommandError::ParameterType {
            device_id,
            parameter,
            expected: parameter.value_kind(),
            actual: value.kind(),
        });
    }
    if let (ParameterId::RunMode, ParameterValue::U8(value)) = (parameter, value) {
        let supported = [
            RunMode::Mit,
            RunMode::Position,
            RunMode::Speed,
            RunMode::Current,
        ];
        if !supported.into_iter().any(|mode| mode.as_u8() == value) {
            return Err(CommandError::UnsupportedRunMode { device_id, value });
        }
    }
    if let ParameterValue::F32(value) = value {
        let field = CommandField::FirmwareParameter(parameter);
        if matches!(
            parameter,
            ParameterId::PositionKp | ParameterId::SpeedKp | ParameterId::SpeedKi
        ) {
            nonnegative_gain(device_id, field, value)?;
        } else if matches!(
            parameter,
            ParameterId::LimitSpeed | ParameterId::LimitTorque
        ) {
            nonnegative(device_id, field, value)?;
        } else {
            finite(device_id, field, value)?;
        }
    }
    Ok(encode_parameter_bytes(host_id, device_id, parameter, value))
}

fn encode_parameter_bytes(
    host_id: u8,
    device_id: u8,
    parameter: ParameterId,
    value: ParameterValue,
) -> (u32, [u8; 8]) {
    let mut data = [0u8; 8];
    data[..2].copy_from_slice(&parameter.as_u16().to_le_bytes());
    match value {
        ParameterValue::U8(v) => {
            data[4] = v;
        }
        ParameterValue::U16(v) => {
            data[4..6].copy_from_slice(&v.to_le_bytes());
        }
        ParameterValue::U32(v) => {
            data[4..8].copy_from_slice(&v.to_le_bytes());
        }
        ParameterValue::F32(v) => {
            data[4..8].copy_from_slice(&v.to_le_bytes());
        }
    }
    (
        pack_typed_ext_id(
            CommunicationType::WriteParameter,
            u16::from(host_id),
            device_id,
        ),
        data,
    )
}

pub fn encode_set_run_mode(device_id: u8, mode: RunMode) -> (u32, [u8; 8]) {
    encode_parameter_bytes(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::RunMode,
        ParameterValue::U8(mode.as_u8()),
    )
}

pub fn encode_speed_ref(
    device_id: u8,
    velocity_rad_s: f32,
) -> Result<(u32, [u8; 8]), CommandError> {
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::SpeedTarget,
        ParameterValue::F32(velocity_rad_s),
    )
}

pub fn encode_position_ref(
    device_id: u8,
    position_rad: f32,
) -> Result<(u32, [u8; 8]), CommandError> {
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::PositionTarget,
        ParameterValue::F32(position_rad),
    )
}

pub fn encode_current_ref(device_id: u8, current_a: f32) -> Result<(u32, [u8; 8]), CommandError> {
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::CurrentTarget,
        ParameterValue::F32(current_a),
    )
}

/// Decoded type-17 reply (manual §4.1.6): ID bits 15..8 carry the motor id, bits 7..0
/// the host id the request named, and bits 23..16 the read status (0 = success).
/// Data bytes 0..2 echo the index LE, bytes 4..8 hold the value LE. Correlation with
/// a specific request is the caller's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterReadReply {
    pub device_id: u8,
    pub host_id: u8,
    pub status: u8,
    pub index: u16,
    pub value: [u8; 4],
}

impl ParameterReadReply {
    pub fn succeeded(self) -> bool {
        self.status == 0
    }

    pub fn parameter(self) -> Option<ParameterId> {
        ParameterId::from_u16(self.index)
    }

    pub fn as_f32(self) -> f32 {
        f32::from_le_bytes(self.value)
    }

    pub fn as_u8(self) -> u8 {
        self.value[0]
    }
}

/// Decode a complete eight-byte type-17 reply addressed to `host_id`. A request
/// frame (host in bits 15..8, motor in the low byte) is not a reply for that host.
pub fn decode_read_parameter_reply(
    host_id: u8,
    can_id: u32,
    payload: &[u8],
) -> Option<ParameterReadReply> {
    let unpacked = unpack_ext_id(can_id)?;
    if CommunicationType::from_u8(unpacked.comm_type)? != CommunicationType::ReadParameter
        || unpacked.device_id != host_id
    {
        return None;
    }
    let data: [u8; 8] = payload.try_into().ok()?;
    Some(ParameterReadReply {
        device_id: (unpacked.extra_data & 0xFF) as u8,
        host_id: unpacked.device_id,
        status: (unpacked.extra_data >> 8) as u8,
        index: u16::from_le_bytes([data[0], data[1]]),
        value: [data[4], data[5], data[6], data[7]],
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::comm::unpack_ext_id;

    #[test]
    fn run_mode_write_uses_parameter_protocol() {
        let (id, data) = encode_set_run_mode(2, RunMode::Speed);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::WriteParameter.as_u8()
        );
        assert_eq!(unpacked.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(unpacked.device_id, 2);
        assert_eq!(&data[..2], &ParameterId::RunMode.as_u16().to_le_bytes());
        assert_eq!(data[4], 2);
    }

    #[test]
    fn speed_ref_write_uses_little_endian_float_value() {
        let (_id, data) = encode_speed_ref(4, 1.25).expect("finite speed");
        assert_eq!(&data[..2], &ParameterId::SpeedTarget.as_u16().to_le_bytes());
        assert_eq!(&data[4..8], &1.25f32.to_le_bytes());
    }

    #[test]
    fn read_run_mode_uses_parameter_protocol() {
        let host_id = DEFAULT_HOST_ID;
        let device_id = 2;
        let (id, data) = encode_read_parameter(host_id, device_id, ParameterId::RunMode);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(unpacked.comm_type, CommunicationType::ReadParameter.as_u8());
        assert_eq!(unpacked.extra_data, u16::from(host_id));
        assert_eq!(unpacked.device_id, device_id);
        assert_eq!(&data[..2], &ParameterId::RunMode.as_u16().to_le_bytes());
        assert_eq!(data[2..], [0u8; 6]);
    }

    #[test]
    fn read_limit_torque_encodes_parameter_index() {
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 3, ParameterId::LimitTorque);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(unpacked.comm_type, CommunicationType::ReadParameter.as_u8());
        assert_eq!(&data[..2], &0x700Bu16.to_le_bytes());
    }

    #[test]
    fn read_ep_scan_time_encodes_parameter_index() {
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 5, ParameterId::EPScanTime);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(unpacked.comm_type, CommunicationType::ReadParameter.as_u8());
        assert_eq!(&data[..2], &0x7026u16.to_le_bytes());
    }

    #[test]
    fn read_can_timeout_encodes_parameter_index() {
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 7, ParameterId::CanTimeout);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(unpacked.comm_type, CommunicationType::ReadParameter.as_u8());
        assert_eq!(&data[..2], &0x7028u16.to_le_bytes());
    }

    #[test]
    fn mech_pos_and_zero_sta_requests_match_bench_probe_frames() {
        // candump: 1100FD01#1970000000000000 and 1100FD01#2970000000000000
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 1, ParameterId::MechPos);
        assert_eq!(id, 0x1100_FD01);
        assert_eq!(data, [0x19, 0x70, 0, 0, 0, 0, 0, 0]);
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 1, ParameterId::ZeroSta);
        assert_eq!(id, 0x1100_FD01);
        assert_eq!(data, [0x29, 0x70, 0, 0, 0, 0, 0, 0]);
        assert_eq!(ParameterId::AddOffset.as_u16(), 0x702B);
        assert_eq!(ParameterId::from_u16(0x7019), Some(ParameterId::MechPos));
        assert_eq!(ParameterId::from_u16(0x7000), None);
    }

    #[test]
    fn literal_probe_mech_pos_replies_decode_little_endian_float() {
        // Bench probe, firmware 0.3.1.42, arm at mechanical home.
        let reply = decode_read_parameter_reply(
            DEFAULT_HOST_ID,
            0x1100_01FD,
            &[0x19, 0x70, 0x00, 0x00, 0x1D, 0xC7, 0x32, 0xB8],
        )
        .expect("type-17 reply");
        assert_eq!(reply.device_id, 1);
        assert_eq!(reply.host_id, DEFAULT_HOST_ID);
        assert!(reply.succeeded());
        assert_eq!(reply.parameter(), Some(ParameterId::MechPos));
        assert!((reply.as_f32() - (-4.25e-5)).abs() < 1e-6);
        let device2 = decode_read_parameter_reply(
            DEFAULT_HOST_ID,
            0x1100_02FD,
            &[0x19, 0x70, 0, 0, 0x8E, 0x63, 0x5F, 0x39],
        )
        .expect("device 2 reply");
        assert_eq!(device2.device_id, 2);
        assert!((device2.as_f32() - 2.13e-4).abs() < 1e-6);
        let device3 = decode_read_parameter_reply(
            DEFAULT_HOST_ID,
            0x1100_03FD,
            &[0x19, 0x70, 0, 0, 0x63, 0x8C, 0x4F, 0xB9],
        )
        .expect("device 3 reply");
        assert!((device3.as_f32() - (-1.98e-4)).abs() < 1e-6);
        let zero_sta = decode_read_parameter_reply(
            DEFAULT_HOST_ID,
            0x1100_01FD,
            &[0x29, 0x70, 0, 0, 0, 0, 0, 0],
        )
        .expect("zero_sta reply");
        assert_eq!(zero_sta.parameter(), Some(ParameterId::ZeroSta));
        assert_eq!(zero_sta.as_u8(), 0);
    }

    #[test]
    fn read_status_requests_and_foreign_hosts_are_distinguished() {
        let failed = decode_read_parameter_reply(
            DEFAULT_HOST_ID,
            0x1101_01FD,
            &[0x19, 0x70, 0, 0, 0, 0, 0, 0],
        )
        .expect("failed read is still a reply");
        assert!(!failed.succeeded());
        // Outbound request: host in bits 15..8, motor in the low byte.
        assert!(decode_read_parameter_reply(DEFAULT_HOST_ID, 0x1100_FD01, &[0; 8]).is_none());
        assert!(decode_read_parameter_reply(0xAA, 0x1100_01FD, &[0; 8]).is_none());
        assert!(decode_read_parameter_reply(DEFAULT_HOST_ID, 0x1100_01FD, &[0; 4]).is_none());
        assert!(decode_read_parameter_reply(DEFAULT_HOST_ID, 0x0200_01FD, &[0; 8]).is_none());
    }

    #[test]
    fn read_only_mech_pos_write_is_rejected_before_encoding() {
        assert!(matches!(
            encode_write_parameter(
                DEFAULT_HOST_ID,
                1,
                ParameterId::MechPos,
                ParameterValue::F32(0.0)
            ),
            Err(CommandError::ReadOnlyParameter {
                device_id: 1,
                parameter: ParameterId::MechPos
            })
        ));
        for (parameter, value) in [
            (ParameterId::ZeroSta, ParameterValue::U8(1)),
            (ParameterId::AddOffset, ParameterValue::F32(0.25)),
        ] {
            assert!(matches!(
                encode_write_parameter(DEFAULT_HOST_ID, 1, parameter, value),
                Err(CommandError::ForbiddenParameterWrite {
                    device_id: 1,
                    parameter: actual,
                }) if actual == parameter
            ));
        }
    }
}
