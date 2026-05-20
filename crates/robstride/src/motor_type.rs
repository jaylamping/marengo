//! Robstride actuator model (RS00–RS04) with per-model MIT ranges.

use marengo_config::MotorType;

/// MIT field ranges for encode/decode (Seeed table; confirm in vendor PDF).
#[derive(Debug, Clone, Copy)]
pub struct MitRanges {
    pub p_min: f32,
    pub p_max: f32,
    pub v_min: f32,
    pub v_max: f32,
    pub kp_min: f32,
    pub kp_max: f32,
    pub kd_min: f32,
    pub kd_max: f32,
    pub t_min: f32,
    pub t_max: f32,
}

impl MitRanges {
    pub fn for_motor_type(ty: MotorType) -> Self {
        match ty {
            MotorType::Rs00 | MotorType::Rs02 => Self {
                p_min: -12.57,
                p_max: 12.57,
                v_min: -44.0,
                v_max: 44.0,
                kp_min: 0.0,
                kp_max: 500.0,
                kd_min: 0.0,
                kd_max: 5.0,
                t_min: -17.0,
                t_max: 17.0,
            },
            MotorType::Rs03 | MotorType::Rs04 => Self {
                p_min: -12.57,
                p_max: 12.57,
                v_min: -50.0,
                v_max: 50.0,
                kp_min: 0.0,
                kp_max: 5000.0,
                kd_min: 0.0,
                kd_max: 100.0,
                t_min: if matches!(ty, MotorType::Rs04) {
                    -120.0
                } else {
                    -60.0
                },
                t_max: if matches!(ty, MotorType::Rs04) {
                    120.0
                } else {
                    60.0
                },
            },
        }
    }
}
