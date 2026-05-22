//! Robstride lifecycle frame encoding.

use crate::comm::{pack_typed_ext_id, CommunicationType, DEFAULT_HOST_ID};

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
    empty_frame(CommunicationType::SetZeroPosition, host_id, device_id)
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

#[cfg(test)]
mod tests {
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
}
