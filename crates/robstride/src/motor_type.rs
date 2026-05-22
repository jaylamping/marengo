//! Robstride actuator model (RS00–RS04) with per-model vendor MIT scales.

use marengo_config::MotorType;

/// Vendor MIT field scales for encode/decode.
///
/// Position, velocity, and torque are represented as signed quantities via
/// `(value / scale + 1.0) * 0x7fff`; gains use unsigned `value / scale * 0xffff`.
#[derive(Debug, Clone, Copy)]
pub struct MitRanges {
    pub position_scale: f32,
    pub velocity_scale: f32,
    pub kp_scale: f32,
    pub kd_scale: f32,
    pub torque_scale: f32,
}

impl MitRanges {
    pub fn for_motor_type(ty: MotorType) -> Self {
        match ty {
            MotorType::Rs00 => Self {
                position_scale: 4.0 * std::f32::consts::PI,
                velocity_scale: 50.0,
                kp_scale: 500.0,
                kd_scale: 5.0,
                torque_scale: 17.0,
            },
            MotorType::Rs02 => Self {
                position_scale: 4.0 * std::f32::consts::PI,
                velocity_scale: 44.0,
                kp_scale: 500.0,
                kd_scale: 5.0,
                torque_scale: 17.0,
            },
            MotorType::Rs03 => Self {
                position_scale: 4.0 * std::f32::consts::PI,
                velocity_scale: 50.0,
                kp_scale: 5000.0,
                kd_scale: 100.0,
                torque_scale: 60.0,
            },
            MotorType::Rs04 => Self {
                position_scale: 4.0 * std::f32::consts::PI,
                velocity_scale: 15.0,
                kp_scale: 5000.0,
                kd_scale: 100.0,
                torque_scale: 120.0,
            },
        }
    }

    pub fn p_min(self) -> f32 {
        -self.position_scale
    }

    pub fn p_max(self) -> f32 {
        self.position_scale
    }

    pub fn v_min(self) -> f32 {
        -self.velocity_scale
    }

    pub fn v_max(self) -> f32 {
        self.velocity_scale
    }

    pub fn t_min(self) -> f32 {
        -self.torque_scale
    }

    pub fn t_max(self) -> f32 {
        self.torque_scale
    }
}
