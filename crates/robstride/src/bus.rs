//! CAN bus abstraction for Robstride frames.

use crate::protocol::{self, MotionCommand};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BusError {
    #[error("CAN send failed: {message}")]
    Send { message: String },
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("driver error: {0}")]
    Driver(String),
}

/// Sends encoded Robstride frames. Production impl uses SocketCAN; tests use memory bus.
pub trait CanBus {
    fn send_raw(&mut self, can_id: u32, data: &[u8]) -> Result<(), BusError>;
}

/// In-memory bus for unit tests (records last frame per device).
#[derive(Debug, Default)]
pub struct MemoryBus {
    pub frames: Vec<(u32, [u8; 8])>,
}

impl CanBus for MemoryBus {
    fn send_raw(&mut self, can_id: u32, data: &[u8]) -> Result<(), BusError> {
        let mut buf = [0u8; 8];
        let len = data.len().min(8);
        buf[..len].copy_from_slice(&data[..len]);
        self.frames.push((can_id, buf));
        Ok(())
    }
}

/// Joint name + motion setpoint (after Davout approval).
#[derive(Debug, Clone, PartialEq)]
pub struct JointMotion {
    pub joint: String,
    pub device_id: u8,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
}

/// Encode and send one joint motion command.
pub fn send_motion<B: CanBus>(bus: &mut B, motion: &JointMotion) -> Result<(), BusError> {
    let cmd = MotionCommand {
        device_id: motion.device_id,
        position_rad: motion.position_rad,
        velocity_rad_s: motion.velocity_rad_s,
        torque_nm: motion.torque_nm,
    };
    let (id, data) = protocol::encode_command(&cmd);
    bus.send_raw(id, &data)
}

#[cfg(all(feature = "vcan", target_os = "linux"))]
mod socketcan {
    use super::*;
    use socketcan::{CanFrame, CanSocket, EmbeddedFrame, Frame, Socket, StandardId};

    pub struct SocketCanBus {
        socket: CanSocket,
    }

    impl SocketCanBus {
        pub fn open(interface: &str) -> Result<Self, BusError> {
            let socket = CanSocket::open(interface).map_err(|e| BusError::Send {
                message: e.to_string(),
            })?;
            Ok(Self { socket })
        }
    }

    impl CanBus for SocketCanBus {
        fn send_raw(&mut self, can_id: u32, data: &[u8]) -> Result<(), BusError> {
            let id = StandardId::new(can_id as u16).ok_or_else(|| BusError::Send {
                message: format!("invalid CAN id {can_id}"),
            })?;
            let frame = CanFrame::new(id, data).map_err(|e| BusError::Send {
                message: e.to_string(),
            })?;
            self.socket.write_frame(&frame).map_err(|e| BusError::Send {
                message: e.to_string(),
            })
        }
    }
}

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub use socketcan::SocketCanBus;
