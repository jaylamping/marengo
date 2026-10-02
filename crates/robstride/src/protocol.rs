//! Vendor diagnostic queries. Replies are observations, never reference permission.

use std::time::Instant;

use crate::comm::{pack_typed_ext_id, unpack_ext_id, CommunicationType};
use crate::{CanFrame, DriveMode, MotorAddress, ReceivedCanFrame, RxFrameKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolReply {
    /// Wire-order bytes of the immutable 64-bit MCU identifier, not a boot ID.
    DeviceIdentity([u8; 8]),
    FirmwareVersion([u8; 4]),
    Parameter {
        index: u16,
        error: u8,
        value: [u8; 4],
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolObservation {
    pub order: usize,
    pub address: MotorAddress,
    pub received_at: Instant,
    pub host_id: u8,
    pub can_id: u32,
    pub raw: [u8; 8],
    pub reply: ProtocolReply,
}

/// Firmware-version bytes must never be interpreted as an MIT pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareVersionFeedback {
    pub raw: [u8; 8],
    pub version: [u8; 4],
    pub status_flags: u8,
    pub drive_mode: DriveMode,
}

pub fn encode_device_identity_query(host_id: u8, device_id: u8) -> CanFrame {
    CanFrame {
        id: pack_typed_ext_id(
            CommunicationType::DeviceIdentity,
            u16::from(host_id),
            device_id,
        ),
        data: [0; 8],
        extended: true,
    }
}

/// Type 4 with C4 reads the version and also stops the drive; byte 0 never clears faults.
pub fn encode_firmware_version_query(host_id: u8, device_id: u8) -> CanFrame {
    CanFrame {
        id: pack_typed_ext_id(CommunicationType::Disable, u16::from(host_id), device_id),
        data: [0, 0xC4, 0, 0, 0, 0, 0, 0],
        extended: true,
    }
}

pub fn decode_firmware_version(received: &ReceivedCanFrame) -> Option<FirmwareVersionFeedback> {
    let id = unpack_ext_id(received.frame.id)?;
    if !received.frame.extended
        || received.kind != RxFrameKind::Data
        || received.payload_len != 8
        || id.comm_type != CommunicationType::OperationStatus.as_u8()
        || received.frame.data[..3] != [0, 0xC4, 0x56]
    {
        return None;
    }
    let raw = received.frame.data;
    Some(FirmwareVersionFeedback {
        raw,
        version: [raw[3], raw[4], raw[5], raw[6]],
        status_flags: ((received.frame.id >> 16) & 0x3F) as u8,
        drive_mode: DriveMode::from_can_id(received.frame.id),
    })
}

/// Return source motor, destination host and exact reply. Routing/config admission is the caller's.
pub fn decode_protocol_reply(received: &ReceivedCanFrame) -> Option<(u8, u8, ProtocolReply)> {
    if !received.frame.extended || received.kind != RxFrameKind::Data || received.payload_len != 8 {
        return None;
    }
    let id = unpack_ext_id(received.frame.id)?;
    let source = id.extra_data as u8;
    let raw = received.frame.data;
    let reply = match CommunicationType::from_u8(id.comm_type)? {
        CommunicationType::DeviceIdentity if id.device_id == 0xFE && id.extra_data >> 8 == 0 => {
            ProtocolReply::DeviceIdentity(raw)
        }
        CommunicationType::ReadParameter if raw[2..4] == [0, 0] => ProtocolReply::Parameter {
            index: u16::from_le_bytes([raw[0], raw[1]]),
            error: (id.extra_data >> 8) as u8,
            value: [raw[4], raw[5], raw[6], raw[7]],
        },
        CommunicationType::OperationStatus => {
            ProtocolReply::FirmwareVersion(decode_firmware_version(received)?.version)
        }
        _ => return None,
    };
    Some((source, id.device_id, reply))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn inspection_queries_never_enable_or_clear_faults() {
        let identity = encode_device_identity_query(0xA1, 5);
        assert_eq!(identity.id, 0x0000_A105);
        assert_eq!(identity.data, [0; 8]);
        let version = encode_firmware_version_query(0xA2, 5);
        assert_eq!(version.id, 0x0400_A205);
        assert_eq!(version.data, [0, 0xC4, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn version_is_non_pose_and_keeps_header_hazards() {
        let frame = ReceivedCanFrame::full_data(
            None,
            CanFrame {
                id: 0x0284_05A2,
                data: [0, 0xC4, 0x56, 0, 3, 1, 42, 0],
                extended: true,
            },
        );
        let version = decode_firmware_version(&frame).expect("version reply");
        assert_eq!(version.version, [0, 3, 1, 42]);
        assert_eq!(version.status_flags, 4);
        assert_eq!(version.drive_mode, DriveMode::Run);
        let mut short = frame.clone();
        short.payload_len = 7;
        assert!(decode_protocol_reply(&short).is_none());
        let mut remote = frame;
        remote.kind = RxFrameKind::Remote { requested_len: 8 };
        assert!(decode_protocol_reply(&remote).is_none());
    }

    #[test]
    fn parameter_reply_preserves_source_host_index_and_error() {
        let frame = ReceivedCanFrame::full_data(
            None,
            CanFrame {
                id: 0x1101_03A4,
                data: [0x19, 0x70, 0, 0, 0, 0, 0x80, 0x3F],
                extended: true,
            },
        );
        assert_eq!(
            decode_protocol_reply(&frame),
            Some((
                3,
                0xA4,
                ProtocolReply::Parameter {
                    index: 0x7019,
                    error: 1,
                    value: [0, 0, 0x80, 0x3F],
                }
            ))
        );
    }
}
