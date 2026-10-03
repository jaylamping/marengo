//! Communication type 0 (manual §4.1.1): device ID and 64-bit MCU unique identifier.
//!
//! Request: `(0 << 24) | (host_id << 8) | device_id`, eight zero data bytes.
//! Reply: `(0 << 24) | (device_id << 8) | 0xFE`, data = the MCU unique identifier.
//! The identifier is compared as raw bytes; no byte order is assumed.

use std::fmt;

use crate::comm::{pack_typed_ext_id, unpack_ext_id, CommunicationType, DEFAULT_HOST_ID};

/// Low identifier byte of every type-0 reply (manual §4.1.1).
pub const DEVICE_ID_REPLY_MARKER: u8 = 0xFE;

/// Opaque 64-bit MCU unique identifier exactly as received on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceUid(pub [u8; 8]);

impl DeviceUid {
    /// Stable numeric spelling of the raw wire bytes for logs and history.
    pub fn as_u64(self) -> u64 {
        u64::from_le_bytes(self.0)
    }
}

impl fmt::Display for DeviceUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Decoded type-0 reply. Correlation with a request is the caller's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceIdReply {
    pub device_id: u8,
    pub uid: DeviceUid,
}

pub fn encode_get_device_id(host_id: u8, device_id: u8) -> (u32, [u8; 8]) {
    (
        pack_typed_ext_id(
            CommunicationType::GetDeviceId,
            u16::from(host_id),
            device_id,
        ),
        [0u8; 8],
    )
}

pub fn encode_default_get_device_id(device_id: u8) -> (u32, [u8; 8]) {
    encode_get_device_id(DEFAULT_HOST_ID, device_id)
}

/// Decode a complete eight-byte type-0 reply. Requests (whose low byte is the
/// target device) and short payloads are not replies and return `None`.
pub fn decode_device_id_reply(can_id: u32, payload: &[u8]) -> Option<DeviceIdReply> {
    let unpacked = unpack_ext_id(can_id)?;
    if CommunicationType::from_u8(unpacked.comm_type)? != CommunicationType::GetDeviceId
        || unpacked.device_id != DEVICE_ID_REPLY_MARKER
        || unpacked.extra_data > 0xFF
    {
        return None;
    }
    let raw: [u8; 8] = payload.try_into().ok()?;
    Some(DeviceIdReply {
        device_id: (unpacked.extra_data & 0xFF) as u8,
        uid: DeviceUid(raw),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn request_matches_bench_probe_frame() {
        // candump: 0000FD01#0000000000000000
        let (id, data) = encode_default_get_device_id(1);
        assert_eq!(id, 0x0000_FD01);
        assert_eq!(data, [0; 8]);
    }

    #[test]
    fn literal_probe_replies_decode_device_and_uid() {
        // candump on the bench Pi, firmware 0.3.1.42.
        for (id, data, device) in [
            (
                0x0000_01FE,
                [0x45, 0x7B, 0x30, 0x02, 0x0C, 0x32, 0x38, 0x17],
                1,
            ),
            (
                0x0000_02FE,
                [0x78, 0x56, 0x30, 0x02, 0x0C, 0x34, 0x37, 0x01],
                2,
            ),
            (
                0x0000_03FE,
                [0x9D, 0x6A, 0x3B, 0x85, 0x9C, 0x02, 0x30, 0x19],
                3,
            ),
        ] {
            let reply = decode_device_id_reply(id, &data).expect("type-0 reply");
            assert_eq!(reply.device_id, device);
            assert_eq!(reply.uid, DeviceUid(data));
        }
        let first = decode_device_id_reply(
            0x0000_01FE,
            &[0x45, 0x7B, 0x30, 0x02, 0x0C, 0x32, 0x38, 0x17],
        )
        .expect("reply");
        assert_eq!(first.uid.to_string(), "457b30020c323817");
    }

    #[test]
    fn requests_short_payloads_and_other_types_are_not_replies() {
        assert!(decode_device_id_reply(0x0000_FD01, &[0; 8]).is_none());
        assert!(decode_device_id_reply(0x0000_01FE, &[0; 7]).is_none());
        assert!(decode_device_id_reply(0x1100_01FE, &[0; 8]).is_none());
        assert!(decode_device_id_reply(0x2000_0000, &[0; 8]).is_none());
    }
}
