//! Robstride parameter IDs and read/write frame encoding.

use crate::comm::{pack_typed_ext_id, CommunicationType, DEFAULT_HOST_ID};

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
}

impl ParameterId {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParameterValue {
    U8(u8),
    F32(f32),
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
) -> (u32, [u8; 8]) {
    let mut data = [0u8; 8];
    data[..2].copy_from_slice(&parameter.as_u16().to_le_bytes());
    match value {
        ParameterValue::U8(v) => {
            data[4] = v;
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
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::RunMode,
        ParameterValue::U8(mode.as_u8()),
    )
}

pub fn encode_speed_ref(device_id: u8, velocity_rad_s: f32) -> (u32, [u8; 8]) {
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::SpeedTarget,
        ParameterValue::F32(velocity_rad_s),
    )
}

pub fn encode_position_ref(device_id: u8, position_rad: f32) -> (u32, [u8; 8]) {
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::PositionTarget,
        ParameterValue::F32(position_rad),
    )
}

pub fn encode_current_ref(device_id: u8, current_a: f32) -> (u32, [u8; 8]) {
    encode_write_parameter(
        DEFAULT_HOST_ID,
        device_id,
        ParameterId::CurrentTarget,
        ParameterValue::F32(current_a),
    )
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
        let (_id, data) = encode_speed_ref(4, 1.25);
        assert_eq!(&data[..2], &ParameterId::SpeedTarget.as_u16().to_le_bytes());
        assert_eq!(&data[4..8], &1.25f32.to_le_bytes());
    }

    #[test]
    fn read_run_mode_uses_parameter_protocol() {
        let host_id = DEFAULT_HOST_ID;
        let device_id = 2;
        let (id, data) = encode_read_parameter(host_id, device_id, ParameterId::RunMode);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::ReadParameter.as_u8()
        );
        assert_eq!(unpacked.extra_data, u16::from(host_id));
        assert_eq!(unpacked.device_id, device_id);
        assert_eq!(&data[..2], &ParameterId::RunMode.as_u16().to_le_bytes());
        assert_eq!(data[2..], [0u8; 6]);
    }

    #[test]
    fn read_limit_torque_encodes_parameter_index() {
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 3, ParameterId::LimitTorque);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::ReadParameter.as_u8()
        );
        assert_eq!(&data[..2], &0x700Bu16.to_le_bytes());
    }

    #[test]
    fn read_ep_scan_time_encodes_parameter_index() {
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 5, ParameterId::EPScanTime);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::ReadParameter.as_u8()
        );
        assert_eq!(&data[..2], &0x7026u16.to_le_bytes());
    }

    #[test]
    fn read_can_timeout_encodes_parameter_index() {
        let (id, data) = encode_read_parameter(DEFAULT_HOST_ID, 7, ParameterId::CanTimeout);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::ReadParameter.as_u8()
        );
        assert_eq!(&data[..2], &0x7028u16.to_le_bytes());
    }
}
