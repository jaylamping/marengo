//! Robstride lifecycle frame encoding.

use crate::comm::{pack_typed_ext_id, CommunicationType, DEFAULT_HOST_ID};

/// Set-zero payload byte 0 observed on RS03 via Motor Studio (`0600FDxx` frames).
const SET_ZERO_DATA: [u8; 8] = [0x01, 0, 0, 0, 0, 0, 0, 0];

fn empty_frame(comm_type: CommunicationType, host_id: u8, device_id: u8) -> (u32, [u8; 8]) {
    (
        pack_typed_ext_id(comm_type, u16::from(host_id), device_id),
        [0u8; 8],
    )
}

pub fn encode_enable(host_id: u8, device_id: u8) -> (u32, [u8; 8]) {
    empty_frame(CommunicationType::Enable, host_id, device_id)
}

pub fn encode_disable(host_id: u8, device_id: u8) -> (u32, [u8; 8]) {
    empty_frame(CommunicationType::Disable, host_id, device_id)
}

pub fn encode_set_zero_position(host_id: u8, device_id: u8) -> (u32, [u8; 8]) {
    (
        pack_typed_ext_id(
            CommunicationType::SetZeroPosition,
            u16::from(host_id),
            device_id,
        ),
        SET_ZERO_DATA,
    )
}

pub fn encode_default_enable(device_id: u8) -> (u32, [u8; 8]) {
    encode_enable(DEFAULT_HOST_ID, device_id)
}

pub fn encode_default_disable(device_id: u8) -> (u32, [u8; 8]) {
    encode_disable(DEFAULT_HOST_ID, device_id)
}

pub fn encode_default_set_zero_position(device_id: u8) -> (u32, [u8; 8]) {
    encode_set_zero_position(DEFAULT_HOST_ID, device_id)
}

/// Active reporting (comm type 24) — manual §4.1.11 payload `01..06 F_CMD`; F_CMD 00=off, 01=on.
pub fn encode_active_reporting(host_id: u8, device_id: u8, enable: bool) -> (u32, [u8; 8]) {
    let f_cmd = if enable { 0x01 } else { 0x00 };
    (
        pack_typed_ext_id(
            CommunicationType::ActiveReporting,
            u16::from(host_id),
            device_id,
        ),
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, f_cmd, 0x00],
    )
}

pub fn encode_default_active_reporting(device_id: u8, enable: bool) -> (u32, [u8; 8]) {
    encode_active_reporting(DEFAULT_HOST_ID, device_id, enable)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::comm::unpack_ext_id;

    #[test]
    fn enable_uses_comm_type_three() {
        let (id, data) = encode_default_enable(1);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(unpacked.comm_type, CommunicationType::Enable.as_u8());
        assert_eq!(unpacked.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(unpacked.device_id, 1);
        assert_eq!(data, [0; 8]);
    }

    #[test]
    fn set_zero_matches_motor_studio_capture_for_device_12() {
        let (id, data) = encode_default_set_zero_position(12);
        assert_eq!(id, 0x0600_FD0C);
        assert_eq!(data, SET_ZERO_DATA);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::SetZeroPosition.as_u8()
        );
        assert_eq!(unpacked.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(unpacked.device_id, 12);
    }

    #[test]
    fn active_reporting_enable_matches_manual_section_4_1_11() {
        let (id, data) = encode_active_reporting(DEFAULT_HOST_ID, 12, true);
        assert_eq!(id, 0x1800_FD0C);
        assert_eq!(data, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x01, 0x00]);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::ActiveReporting.as_u8()
        );
        assert_eq!(unpacked.extra_data, u16::from(DEFAULT_HOST_ID));
        assert_eq!(unpacked.device_id, 12);
    }

    #[test]
    fn active_reporting_disable_uses_f_cmd_zero() {
        let (id, data) = encode_active_reporting(DEFAULT_HOST_ID, 3, false);
        assert_eq!(id, 0x1800_FD03);
        assert_eq!(data, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x00, 0x00]);
        let unpacked = unpack_ext_id(id).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::ActiveReporting.as_u8()
        );
        assert_eq!(unpacked.device_id, 3);
    }
}
