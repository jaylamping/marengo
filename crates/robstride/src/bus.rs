//! CAN bus abstraction for Robstride MIT frames.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use thiserror::Error;

use crate::mit::{self, MitCommand};
use crate::protocol::MotionCommand;
use crate::state::MotorState;

#[derive(Debug, Error)]
pub enum BusError {
    #[error("CAN send failed: {message}")]
    Send { message: String },
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("driver error: {0}")]
    Driver(String),
    #[error("recv timeout")]
    RecvTimeout,
}

/// One recorded frame (extended 29-bit id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanFrame {
    pub id: u32,
    pub data: [u8; 8],
    pub extended: bool,
}

/// Sends encoded Robstride frames.
pub trait CanBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError>;

    /// Drain received frames into `out` (non-blocking). Default: no RX.
    fn recv_frames(&mut self, _out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        Ok(())
    }
}

/// Motor bus: MIT commands + feedback cache.
pub trait MotorBus: CanBus {
    fn mit_control_all(&mut self, cmds: &[MitCommand]) -> Result<(), BusError> {
        for cmd in cmds {
            let (id, data) = mit::encode_mit(cmd);
            self.send_frame(&CanFrame {
                id,
                data,
                extended: true,
            })?;
        }
        Ok(())
    }

    fn recv_all(
        &mut self,
        motor_types: &HashMap<u8, MotorType>,
        states: &mut HashMap<u8, MotorState>,
        timeout: Duration,
    ) -> Result<usize, BusError> {
        let deadline = Instant::now() + timeout;
        let mut frames = Vec::new();
        let mut count = 0;
        while Instant::now() < deadline {
            frames.clear();
            self.recv_frames(&mut frames)?;
            if frames.is_empty() {
                std::thread::sleep(Duration::from_micros(200));
                continue;
            }
            for frame in &frames {
                if let Some(device_id) = frame.id.checked_sub(mit::MIT_RX_BASE) {
                    let device_id = device_id as u8;
                    let motor_type = motor_types.get(&device_id).copied().unwrap_or(MotorType::Rs02);
                    if let Some(fb) = mit::decode_mit_feedback(motor_type, frame.id, frame.data.as_slice()) {
                        states.insert(
                            device_id,
                            MotorState {
                                position_rad: fb.position_rad,
                                velocity_rad_s: fb.velocity_rad_s,
                                torque_nm: fb.torque_nm,
                                fault: fb.fault,
                                updated: Some(Instant::now()),
                            },
                        );
                        count += 1;
                    }
                }
            }
            if count > 0 {
                return Ok(count);
            }
        }
        Err(BusError::RecvTimeout)
    }
}

/// In-memory bus for unit tests.
#[derive(Debug, Default)]
pub struct MemoryBus {
    pub tx: Vec<CanFrame>,
    pub rx_queue: Vec<CanFrame>,
}

impl CanBus for MemoryBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.tx.push(frame.clone());
        Ok(())
    }

    fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        out.append(&mut self.rx_queue);
        Ok(())
    }
}

impl MotorBus for MemoryBus {}

/// Joint name + motion setpoint (legacy position path).
#[derive(Debug, Clone, PartialEq)]
pub struct JointMotion {
    pub joint: String,
    pub device_id: u8,
    pub motor_type: MotorType,
    pub position_rad: f32,
    pub velocity_rad_s: f32,
    pub torque_nm: f32,
}

/// Encode MIT and send one joint command (after Davout approval).
pub fn send_mit<B: MotorBus>(bus: &mut B, cmd: &MitCommand) -> Result<(), BusError> {
    bus.mit_control_all(std::slice::from_ref(cmd))
}

/// Legacy position-mode send (maps to MIT with kp=0, kd=0).
pub fn send_motion<B: MotorBus>(bus: &mut B, motion: &JointMotion) -> Result<(), BusError> {
    let cmd = MitCommand {
        device_id: motion.device_id,
        motor_type: motion.motor_type,
        position_rad: motion.position_rad,
        velocity_rad_s: motion.velocity_rad_s,
        kp: 0.0,
        kd: 0.0,
        torque_ff_nm: motion.torque_nm,
    };
    send_mit(bus, &cmd)
}

/// Encode legacy motion as standard (non-extended) stub for compatibility tests.
pub fn send_motion_legacy<B: CanBus>(bus: &mut B, motion: &JointMotion) -> Result<(), BusError> {
    let cmd = MotionCommand {
        device_id: motion.device_id,
        position_rad: motion.position_rad,
        velocity_rad_s: motion.velocity_rad_s,
        torque_nm: motion.torque_nm,
    };
    let (id, data) = crate::protocol::encode_command(&cmd);
    bus.send_frame(&CanFrame {
        id,
        data,
        extended: false,
    })
}

#[cfg(all(feature = "vcan", target_os = "linux"))]
mod socketcan {
    use super::*;
    use socketcan::{CanFrame as SocketFrame, CanSocket, EmbeddedFrame, ExtendedId, Frame, Socket};

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
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            let frame = if frame.extended {
                let id = ExtendedId::new(frame.id).ok_or_else(|| BusError::Send {
                    message: format!("invalid extended id {}", frame.id),
                })?;
                SocketFrame::new(id, &frame.data).map_err(|e| BusError::Send {
                    message: e.to_string(),
                })?
            } else {
                use socketcan::StandardId;
                let id = StandardId::new(frame.id as u16).ok_or_else(|| BusError::Send {
                    message: format!("invalid standard id {}", frame.id),
                })?;
                SocketFrame::new(id, &frame.data).map_err(|e| BusError::Send {
                    message: e.to_string(),
                })?
            };
            self.socket.write_frame(&frame).map_err(|e| BusError::Send {
                message: e.to_string(),
            })
        }

        fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
            use socketcan::SocketOptions;
            self.socket
                .set_read_timeout(Duration::from_millis(1))
                .map_err(|e| BusError::Driver(e.to_string()))?;
            loop {
                match self.socket.read_frame() {
                    Ok(frame) => {
                        let id = frame.raw_id();
                        let mut data = [0u8; 8];
                        let payload = frame.data();
                        let len = payload.len().min(8);
                        data[..len].copy_from_slice(&payload[..len]);
                        out.push(CanFrame {
                            id,
                            data,
                            extended: frame.is_extended(),
                        });
                    }
                    Err(_) => break,
                }
            }
            Ok(())
        }
    }

    impl MotorBus for SocketCanBus {}
}

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub use socketcan::SocketCanBus;
