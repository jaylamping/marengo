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
            // RS00 is disputed: every RS00 manual revision (251112, 260112,
            // 260713) gives V ±33 rad/s and T ±14 Nm, while these values match
            // the Seeed SDK (V ±50, T ±17). No RS00 bench motion has been
            // captured, so they stay unchanged until one settles it. See
            // docs/commissioning/firmware/robstride-mit-ranges.md.
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
            // RS03 velocity is ±20 rad/s per RS03 User Manual §4.1.2
            // (communication types 1 and 2: "-20rad/s~20rad/s") and the §4.4
            // program sample (`V_MIN -20.0f` / `V_MAX 20.0f`), in revisions
            // 251112, 260112 and 260713. The Seeed SDK's 50 is wrong for the
            // deployed drives: bench capture cd-20261003T145133Z (right
            // shoulder pitch, id1) fits a full scale of 20.05 rad/s against
            // d(position)/dt (r = 0.999, n = 227).
            MotorType::Rs03 => Self {
                position_scale: 4.0 * std::f32::consts::PI,
                velocity_scale: 20.0,
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

    /// Nominal adjacent position-feedback code spacing, before joint conversion.
    /// This describes the software wire mapping, not physical encoder accuracy.
    pub fn feedback_position_step(self) -> f64 {
        f64::from(self.position_scale) / f64::from(crate::mit::SIGNED_FIELD_CENTER)
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
