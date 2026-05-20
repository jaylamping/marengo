//! Cached motor feedback per joint.

use std::time::{Duration, Instant};

/// Latest feedback for one actuator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotorState {
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
    pub fault: u8,
    pub updated: Option<Instant>,
}

impl Default for MotorState {
    fn default() -> Self {
        Self {
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            torque_nm: 0.0,
            fault: 0,
            updated: None,
        }
    }
}

impl MotorState {
    pub fn is_stale(&self, max_age: Duration) -> bool {
        match self.updated {
            None => true,
            Some(t) => t.elapsed() > max_age,
        }
    }
}
