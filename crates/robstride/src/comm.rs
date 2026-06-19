//! Robstride extended CAN identifier helpers.

/// Robstride uses CAN 2.0B extended identifiers with a 5-bit communication type,
/// 16 bits of type-specific extra data, and the 8-bit motor device ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedId {
    pub comm_type: u8,
    pub extra_data: u16,
    pub device_id: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommunicationType {
    OperationControl = 1,
    OperationStatus = 2,
    Enable = 3,
    Disable = 4,
    SetZeroPosition = 6,
    ReadParameter = 17,
    WriteParameter = 18,
    FaultReport = 21,
    ActiveReporting = 24,
}

impl CommunicationType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::OperationControl),
            2 => Some(Self::OperationStatus),
            3 => Some(Self::Enable),
            4 => Some(Self::Disable),
            6 => Some(Self::SetZeroPosition),
            17 => Some(Self::ReadParameter),
            18 => Some(Self::WriteParameter),
            21 => Some(Self::FaultReport),
            24 => Some(Self::ActiveReporting),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Motor id for **inbound** status/fault frames (host id in low byte, motor id at bits 8–15).
/// Outbound command/lifecycle frames use the motor id in the low byte.
pub fn inbound_motor_device_id(can_id: u32, comm_type: CommunicationType) -> u8 {
    match comm_type {
        CommunicationType::OperationStatus | CommunicationType::FaultReport => {
            ((can_id >> 8) & 0xFF) as u8
        }
        _ => (can_id & 0xFF) as u8,
    }
}

pub const EXTENDED_ID_MASK: u32 = 0x1FFF_FFFF;
/// USB-CAN / Motor Studio default host id (lifecycle + parameter frames use this in `extra_data`).
pub const DEFAULT_HOST_ID: u8 = 0xFD;

pub fn pack_ext_id(comm_type: u8, extra_data: u16, device_id: u8) -> u32 {
    ((u32::from(comm_type) & 0x1F) << 24) | (u32::from(extra_data) << 8) | u32::from(device_id)
}

pub fn pack_typed_ext_id(comm_type: CommunicationType, extra_data: u16, device_id: u8) -> u32 {
    pack_ext_id(comm_type.as_u8(), extra_data, device_id)
}

pub fn unpack_ext_id(ext_id: u32) -> Option<ExtendedId> {
    if ext_id > EXTENDED_ID_MASK {
        return None;
    }
    Some(ExtendedId {
        comm_type: ((ext_id >> 24) & 0x1F) as u8,
        extra_data: ((ext_id >> 8) & 0xFFFF) as u16,
        device_id: (ext_id & 0xFF) as u8,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn inbound_motor_device_id_status_uses_bits_8_15() {
        assert_eq!(
            inbound_motor_device_id(0x028002FF, CommunicationType::OperationStatus),
            2
        );
        assert_eq!(
            inbound_motor_device_id(0x02800CFF, CommunicationType::OperationStatus),
            12
        );
        assert_eq!(
            inbound_motor_device_id(0x0300FF0C, CommunicationType::Enable),
            12
        );
    }

    #[test]
    fn pack_unpack_vendor_ext_id() {
        let id = pack_ext_id(18, 0x00FF, 3);
        assert_eq!(id, 0x1200_FF03);
        let unpacked = unpack_ext_id(id).expect("valid extended id");
        assert_eq!(unpacked.comm_type, 18);
        assert_eq!(unpacked.extra_data, 0x00FF);
        assert_eq!(unpacked.device_id, 3);
    }
}
