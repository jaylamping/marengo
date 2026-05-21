//! Robstride RS-series CAN frame encoding (control subset for arm bring-up).
//!
//! Wire format is a Marengo-internal subset documented here until vendor ICD is linked in
//! `hardware/docs/decisions/0001-can-and-motors.md`.

/// Position-mode motion command for one actuator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionCommand {
    pub device_id: u8,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
}

/// Parsed feedback frame (subset).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionFeedback {
    pub device_id: u8,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
}

const CMD_POSITION: u8 = 0x01;
const CMD_FEEDBACK: u8 = 0x02;
const BASE_TX_ID: u32 = 0x140;
const BASE_RX_ID: u32 = 0x240;

/// Standard CAN arbitration ID for a position command to `device_id`.
pub fn command_can_id(device_id: u8) -> u32 {
    BASE_TX_ID + u32::from(device_id)
}

/// Standard CAN arbitration ID for feedback from `device_id`.
pub fn feedback_can_id(device_id: u8) -> u32 {
    BASE_RX_ID + u32::from(device_id)
}

/// Encode a position-mode command into an 8-byte CAN payload.
pub fn encode_command(cmd: &MotionCommand) -> (u32, [u8; 8]) {
    let mut data = [0u8; 8];
    data[0] = CMD_POSITION;
    data[1..5].copy_from_slice(&cmd.position_rad.to_le_bytes());
    let vel_milli = (cmd.velocity_rad_s * 1000.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    data[5..7].copy_from_slice(&vel_milli.to_le_bytes());
    data[7] = (cmd.torque_nm.clamp(-32.0, 31.0) + 32.0) as u8;
    (command_can_id(cmd.device_id), data)
}

/// Decode feedback payload; returns `None` if the frame is not motion feedback.
pub fn decode_feedback(can_id: u32, data: &[u8]) -> Option<MotionFeedback> {
    if data.len() < 8 || data[0] != CMD_FEEDBACK {
        return None;
    }
    let device_id = can_id.checked_sub(BASE_RX_ID)? as u8;
    let position_rad = f32::from_le_bytes(data[1..5].try_into().ok()?);
    let velocity_rad_s = i16::from_le_bytes(data[5..7].try_into().ok()?) as f32 / 1000.0;
    let torque_nm = f32::from(data[7]) - 32.0;
    Some(MotionFeedback {
        device_id,
        position_rad,
        velocity_rad_s,
        torque_nm,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn command_roundtrip_fields() {
        let cmd = MotionCommand {
            device_id: 3,
            position_rad: 0.25,
            velocity_rad_s: 0.5,
            torque_nm: 1.0,
        };
        let (id, data) = encode_command(&cmd);
        assert_eq!(id, command_can_id(3));
        assert_eq!(data[0], CMD_POSITION);
        let pos = f32::from_le_bytes(data[1..5].try_into().expect("bytes"));
        assert!((pos - 0.25).abs() < 1e-5);
    }

    #[test]
    fn feedback_decode() {
        let mut data = [0u8; 8];
        data[0] = CMD_FEEDBACK;
        data[1..5].copy_from_slice(&0.5f32.to_le_bytes());
        data[5..7].copy_from_slice(&250i16.to_le_bytes()); // 0.25 rad/s
        data[7] = 33; // torque 1.0
        let fb = decode_feedback(feedback_can_id(2), &data).expect("decode");
        assert_eq!(fb.device_id, 2);
        assert!((fb.position_rad - 0.5).abs() < 1e-5);
    }
}
