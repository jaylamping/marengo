//! Firmware version query: communication type 4 with `data[1] = 0xC4`.
//!
//! The request is a Disable (type 4) whose Byte[0] stays 0, so it stops the drive
//! and never clears a fault. The drive answers with a type-2 status identifier
//! whose payload starts `00 C4 56` followed by four version bytes. That payload
//! is not an MIT pose: it must never be decoded as one, while its identifier
//! still carries the drive's status flags and mode.

use std::fmt;

use crate::comm::{pack_typed_ext_id, unpack_ext_id, CommunicationType, DEFAULT_HOST_ID};
use crate::DriveMode;

/// Type-4 data byte 1 that selects the version reply.
pub const FIRMWARE_VERSION_QUERY: u8 = 0xC4;

/// First three payload bytes of every type-2 version reply.
const VERSION_REPLY_PREFIX: [u8; 3] = [0x00, FIRMWARE_VERSION_QUERY, 0x56];

/// Four firmware version bytes exactly as received, most significant first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FirmwareVersion(pub [u8; 4]);

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d] = self.0;
        write!(f, "{a}.{b}.{c}.{d}")
    }
}

/// Decoded type-2 version reply. The header hazards are the drive's, as on any
/// status frame; correlation with a request is the caller's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareVersionFeedback {
    pub raw: [u8; 8],
    pub version: FirmwareVersion,
    /// Low identifier byte: the host whose query this answers.
    pub host_id: u8,
    pub status_flags: u8,
    pub drive_mode: DriveMode,
}

pub fn encode_get_firmware_version(host_id: u8, device_id: u8) -> (u32, [u8; 8]) {
    let mut data = [0u8; 8];
    data[1] = FIRMWARE_VERSION_QUERY;
    (
        pack_typed_ext_id(CommunicationType::Disable, u16::from(host_id), device_id),
        data,
    )
}

pub fn encode_default_get_firmware_version(device_id: u8) -> (u32, [u8; 8]) {
    encode_get_firmware_version(DEFAULT_HOST_ID, device_id)
}

/// Decode a complete eight-byte type-2 payload that starts `00 C4 56`. Any other
/// status payload is not a version reply and returns `None`.
pub fn decode_firmware_version_reply(
    can_id: u32,
    payload: &[u8],
) -> Option<FirmwareVersionFeedback> {
    let unpacked = unpack_ext_id(can_id)?;
    if CommunicationType::from_u8(unpacked.comm_type)? != CommunicationType::OperationStatus {
        return None;
    }
    let raw: [u8; 8] = payload.try_into().ok()?;
    if raw[..3] != VERSION_REPLY_PREFIX {
        return None;
    }
    Some(FirmwareVersionFeedback {
        raw,
        version: FirmwareVersion([raw[3], raw[4], raw[5], raw[6]]),
        host_id: (can_id & 0xFF) as u8,
        status_flags: ((can_id >> 16) & 0x3F) as u8,
        drive_mode: DriveMode::from_can_id(can_id),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn query_is_a_byte0_zero_disable_with_c4_selector() {
        let (id, data) = encode_default_get_firmware_version(5);
        assert_eq!(id, 0x0400_FD05);
        assert_eq!(data, [0, 0xC4, 0, 0, 0, 0, 0, 0]);
        assert_ne!(data[0], 1, "Byte[0]=1 would be a fault clear");
    }

    #[test]
    fn version_reply_keeps_header_hazards_and_version_bytes() {
        // Status identifier: mode Run (bit 23), flag bit 18, motor 4, host 0xFD.
        let id = 0x0284_04FD;
        let reply = decode_firmware_version_reply(id, &[0, 0xC4, 0x56, 0, 3, 1, 42, 0])
            .expect("version reply");
        assert_eq!(reply.version, FirmwareVersion([0, 3, 1, 42]));
        assert_eq!(reply.version.to_string(), "0.3.1.42");
        assert_eq!(reply.status_flags, 4);
        assert_eq!(reply.drive_mode, DriveMode::Run);
        assert_eq!(reply.host_id, DEFAULT_HOST_ID);
    }

    #[test]
    fn other_status_payloads_short_frames_and_other_types_are_not_versions() {
        assert!(
            decode_firmware_version_reply(0x0200_04FD, &[0x80, 0, 0x80, 0, 0, 0, 0, 0]).is_none()
        );
        assert!(
            decode_firmware_version_reply(0x0200_04FD, &[0, 0xC4, 0x56, 0, 3, 1, 42]).is_none()
        );
        assert!(
            decode_firmware_version_reply(0x0400_FD04, &[0, 0xC4, 0x56, 0, 3, 1, 42, 0]).is_none()
        );
        assert!(
            decode_firmware_version_reply(0x1800_04FD, &[0, 0xC4, 0x56, 0, 3, 1, 42, 0]).is_none()
        );
    }
}
