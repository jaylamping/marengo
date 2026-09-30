//! Robstride MIT Mode 0 CAN encoding (vendor 29-bit extended protocol).

use marengo_config::MotorType;

use crate::comm::{pack_typed_ext_id, unpack_ext_id, CommunicationType};
use crate::command::{finite, nonnegative_gain, CommandError, CommandField};
use crate::motor_type::MitRanges;

/// MIT-mode command for one actuator (OpenArm / Robstride semantics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MitCommand {
    pub device_id: u8,
    pub motor_type: MotorType,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub kp: f32,
    pub kd: f32,
    pub torque_ff_nm: f32,
}

impl MitCommand {
    /// Check the motor-space command before encoding or admitting a whole batch.
    /// Finite values outside vendor ranges retain the existing saturating encoding;
    /// joint-space ceilings and enable policy remain Davout's responsibility.
    pub fn validate(&self) -> Result<(), CommandError> {
        finite(self.device_id, CommandField::Position, self.position_rad)?;
        finite(self.device_id, CommandField::Velocity, self.velocity_rad_s)?;
        nonnegative_gain(self.device_id, CommandField::ProportionalGain, self.kp)?;
        nonnegative_gain(self.device_id, CommandField::DampingGain, self.kd)?;
        finite(
            self.device_id,
            CommandField::TorqueFeedforward,
            self.torque_ff_nm,
        )
    }
}

/// Parsed MIT feedback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MitFeedback {
    pub device_id: u8,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
    pub temperature_c: f32,
    /// Compatibility indication of the status flags; not a detailed-fault mask.
    pub fault: u16,
    /// CAN-ID bits 16..21: undervoltage, overcurrent, overtemperature, magnetic
    /// encoder, stall/overload and uncalibrated, respectively.
    pub status_flags: u8,
    pub drive_mode: crate::feedback::DriveMode,
}

/// Extended CAN arbitration ID for a neutral-torque MIT command to `device_id`.
///
/// `encode_mit` includes the real torque field in the ID. This helper remains
/// for callers/tests that only need to recognize the vendor communication type.
pub fn mit_tx_id(device_id: u8) -> u32 {
    pack_typed_ext_id(CommunicationType::OperationControl, 0x7FFF, device_id)
}

/// Extended CAN arbitration ID shape for MIT status from `device_id`.
pub fn mit_rx_id(device_id: u8) -> u32 {
    pack_typed_ext_id(CommunicationType::OperationStatus, 0, device_id)
}

fn signed_to_vendor_u16(value: f32, scale: f32) -> u16 {
    if scale <= 0.0 {
        return 0;
    }
    let clamped = value.clamp(-scale, scale);
    ((clamped / scale + 1.0) * 0x7FFF as f32)
        .round()
        .clamp(0.0, u16::MAX as f32) as u16
}

fn vendor_u16_to_signed(raw: u16, scale: f32) -> f32 {
    ((f32::from(raw) / 0x7FFF as f32) - 1.0) * scale
}

fn unsigned_to_vendor_u16(value: f32, scale: f32) -> u16 {
    if scale <= 0.0 {
        return 0;
    }
    ((value.clamp(0.0, scale) / scale) * u16::MAX as f32)
        .round()
        .clamp(0.0, u16::MAX as f32) as u16
}

fn read_be_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn write_be_u16(data: &mut [u8; 8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

/// Pack MIT command into Robstride's operation-control frame.
///
/// Position/velocity/kp/kd are big-endian u16 fields in the payload. Torque
/// feedforward is the 16-bit `extra_data` field in the extended arbitration ID.
/// Nonfinite fields and negative gains return a typed error before quantization.
pub fn encode_mit(cmd: &MitCommand) -> Result<(u32, [u8; 8]), CommandError> {
    cmd.validate()?;
    let ranges = MitRanges::for_motor_type(cmd.motor_type);
    let p_int = signed_to_vendor_u16(cmd.position_rad, ranges.position_scale);
    let v_int = signed_to_vendor_u16(cmd.velocity_rad_s, ranges.velocity_scale);
    let kp_int = unsigned_to_vendor_u16(cmd.kp, ranges.kp_scale);
    let kd_int = unsigned_to_vendor_u16(cmd.kd, ranges.kd_scale);
    let t_int = signed_to_vendor_u16(cmd.torque_ff_nm, ranges.torque_scale);

    let mut data = [0u8; 8];
    write_be_u16(&mut data, 0, p_int);
    write_be_u16(&mut data, 2, v_int);
    write_be_u16(&mut data, 4, kp_int);
    write_be_u16(&mut data, 6, kd_int);

    Ok((
        pack_typed_ext_id(CommunicationType::OperationControl, t_int, cmd.device_id),
        data,
    ))
}

/// Decode MIT feedback; returns `None` if frame length or ID is invalid.
///
/// Accepts OperationStatus (MIT replies) and ActiveReporting (type-24 free-drive
/// sensing while limp).
pub fn decode_mit_feedback(motor_type: MotorType, can_id: u32, data: &[u8]) -> Option<MitFeedback> {
    if data.len() != 8 {
        return None;
    }
    let unpacked = unpack_ext_id(can_id)?;
    let comm = CommunicationType::from_u8(unpacked.comm_type)?;
    if !matches!(
        comm,
        CommunicationType::OperationStatus | CommunicationType::ActiveReporting
    ) {
        return None;
    }
    let ranges = MitRanges::for_motor_type(motor_type);
    let p = vendor_u16_to_signed(read_be_u16(data, 0), ranges.position_scale);
    let v = vendor_u16_to_signed(read_be_u16(data, 2), ranges.velocity_scale);
    let t = vendor_u16_to_signed(read_be_u16(data, 4), ranges.torque_scale);
    let temp = f32::from(read_be_u16(data, 6)) * 0.1;
    Some(MitFeedback {
        device_id: crate::comm::inbound_motor_device_id(can_id, comm),
        position_rad: p,
        velocity_rad_s: v,
        torque_nm: t,
        temperature_c: temp,
        fault: u16::from(((can_id >> 16) & 0x3f) as u8),
        status_flags: ((can_id >> 16) & 0x3f) as u8,
        drive_mode: crate::feedback::DriveMode::from_can_id(can_id),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::comm::{unpack_ext_id, DEFAULT_HOST_ID};
    use marengo_config::MotorType;

    #[test]
    fn mit_tx_id_uses_vendor_comm_type() {
        let unpacked = unpack_ext_id(mit_tx_id(3)).expect("extended id");
        assert_eq!(
            unpacked.comm_type,
            CommunicationType::OperationControl.as_u8()
        );
        assert_eq!(unpacked.extra_data, 0x7FFF);
        assert_eq!(unpacked.device_id, 3);
    }

    #[test]
    fn encode_produces_vendor_operation_frame() {
        let cmd = MitCommand {
            device_id: 1,
            motor_type: MotorType::Rs03,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 0.0,
        };
        let (id, data) = encode_mit(&cmd).expect("valid command");
        assert_eq!(id, mit_tx_id(1));
        assert_eq!(data.len(), 8);
        assert_eq!(data, [0x7F, 0xFF, 0x7F, 0xFF, 0, 0, 0, 0]);
    }

    #[test]
    fn decode_status_frame_reads_big_endian_fields() {
        let ranges = MitRanges::for_motor_type(MotorType::Rs02);
        let mut data = [0u8; 8];
        write_be_u16(
            &mut data,
            0,
            signed_to_vendor_u16(1.0, ranges.position_scale),
        );
        write_be_u16(
            &mut data,
            2,
            signed_to_vendor_u16(2.0, ranges.velocity_scale),
        );
        write_be_u16(&mut data, 4, signed_to_vendor_u16(3.0, ranges.torque_scale));
        write_be_u16(&mut data, 6, 321);
        let id = pack_typed_ext_id(CommunicationType::OperationStatus, 4, DEFAULT_HOST_ID);
        let fb = decode_mit_feedback(MotorType::Rs02, id, &data).expect("feedback");
        assert_eq!(fb.device_id, 4);
        assert!((fb.position_rad - 1.0).abs() < 0.001);
        assert!((fb.velocity_rad_s - 2.0).abs() < 0.01);
        assert!((fb.torque_nm - 3.0).abs() < 0.01);
        assert!((fb.temperature_c - 32.1).abs() < 0.001);

        let id24 = pack_typed_ext_id(CommunicationType::ActiveReporting, 3, DEFAULT_HOST_ID);
        let fb24 = decode_mit_feedback(MotorType::Rs02, id24, &data).expect("type-24 feedback");
        assert_eq!(fb24.device_id, 3);
        assert!((fb24.position_rad - 1.0).abs() < 0.001);
    }
}
