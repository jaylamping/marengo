//! Robstride MIT Mode 0 CAN encoding (29-bit extended, TX id 0x200 + motor_id).

use marengo_config::MotorType;

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

/// Parsed MIT feedback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MitFeedback {
    pub device_id: u8,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
    pub temperature_c: u8,
    pub fault: u8,
}

pub const MIT_TX_BASE: u32 = 0x200;
pub const MIT_RX_BASE: u32 = 0x200;

/// Extended CAN arbitration ID for MIT command to `device_id`.
pub fn mit_tx_id(device_id: u8) -> u32 {
    MIT_TX_BASE + u32::from(device_id)
}

/// Extended CAN arbitration ID for MIT feedback from `device_id`.
pub fn mit_rx_id(device_id: u8) -> u32 {
    MIT_RX_BASE + u32::from(device_id)
}

fn float_to_uint(x: f32, x_min: f32, x_max: f32, bits: u32) -> u32 {
    let span = x_max - x_min;
    if span <= 0.0 {
        return 0;
    }
    let clamped = x.clamp(x_min, x_max);
    let max_int = (1u32 << bits) - 1;
    let norm = (clamped - x_min) / span;
    (norm * max_int as f32).round() as u32
}

fn uint_to_float(x: u32, x_min: f32, x_max: f32, bits: u32) -> f32 {
    let max_int = (1u32 << bits) - 1;
    let span = x_max - x_min;
    x_min + (x as f32 / max_int as f32) * span
}

/// Pack MIT command into 8 data bytes (Cheetah-style layout used by Robstride family).
pub fn encode_mit(cmd: &MitCommand) -> (u32, [u8; 8]) {
    let ranges = MitRanges::for_motor_type(cmd.motor_type);
    let p_int = float_to_uint(cmd.position_rad, ranges.p_min, ranges.p_max, 16);
    let v_int = float_to_uint(cmd.velocity_rad_s, ranges.v_min, ranges.v_max, 12);
    let kp_int = float_to_uint(cmd.kp, ranges.kp_min, ranges.kp_max, 12);
    let kd_int = float_to_uint(cmd.kd, ranges.kd_min, ranges.kd_max, 12);
    let t_int = float_to_uint(cmd.torque_ff_nm, ranges.t_min, ranges.t_max, 12);

    let mut data = [0u8; 8];
    data[0] = (p_int >> 8) as u8;
    data[1] = (p_int & 0xFF) as u8;
    data[2] = (v_int >> 4) as u8;
    data[3] = (((v_int & 0xF) << 4) | (kp_int >> 8)) as u8;
    data[4] = (kp_int & 0xFF) as u8;
    data[5] = (kd_int >> 4) as u8;
    data[6] = (((kd_int & 0xF) << 4) | (t_int >> 8)) as u8;
    data[7] = (t_int & 0xFF) as u8;

    (mit_tx_id(cmd.device_id), data)
}

/// Decode MIT feedback; returns `None` if frame length or ID is invalid.
pub fn decode_mit_feedback(
    motor_type: MotorType,
    can_id: u32,
    data: &[u8],
) -> Option<MitFeedback> {
    if data.len() < 8 {
        return None;
    }
    let device_id = can_id.checked_sub(MIT_RX_BASE)? as u8;
    let ranges = MitRanges::for_motor_type(motor_type);
    let p_int = (u32::from(data[0]) << 8) | u32::from(data[1]);
    let v_int = (u32::from(data[2]) << 4) | (u32::from(data[3]) >> 4);
    let t_int = ((u32::from(data[3]) & 0xF) << 8) | u32::from(data[4]);
    let p = uint_to_float(p_int, ranges.p_min, ranges.p_max, 16);
    let v = uint_to_float(v_int, ranges.v_min, ranges.v_max, 12);
    let t = uint_to_float(t_int, ranges.t_min, ranges.t_max, 12);
    Some(MitFeedback {
        device_id,
        position_rad: p,
        velocity_rad_s: v,
        torque_nm: t,
        temperature_c: data[5],
        fault: data[6],
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use marengo_config::MotorType;

    #[test]
    fn mit_tx_id_matches_adr() {
        assert_eq!(mit_tx_id(3), 0x203);
    }

    #[test]
    fn encode_produces_eight_bytes() {
        let cmd = MitCommand {
            device_id: 1,
            motor_type: MotorType::Rs03,
            position_rad: 0.1,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 2.0,
        };
        let (id, data) = encode_mit(&cmd);
        assert_eq!(id, 0x201);
        assert_eq!(data.len(), 8);
    }

    #[test]
    fn rs02_rs03_different_torque_ranges() {
        let t = 50.0f32;
        let rs02 = MitRanges::for_motor_type(MotorType::Rs02);
        let rs03 = MitRanges::for_motor_type(MotorType::Rs03);
        assert!(rs03.t_max > rs02.t_max);
        let _ = float_to_uint(t, rs02.t_min, rs02.t_max, 12);
        let hi = float_to_uint(t, rs03.t_min, rs03.t_max, 12);
        assert!(hi > 0);
    }
}
