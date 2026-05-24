//! CAN bus abstraction for Robstride MIT frames.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::MotorType;
use thiserror::Error;

use crate::comm::{self, CommunicationType};
use crate::lifecycle;
use crate::mit::{self, MitCommand};
use crate::params::{self, ParameterId, ParameterValue, RunMode};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FaultReport {
    device_id: u8,
    fault: u16,
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

fn send_encoded_frame<B: CanBus + ?Sized>(
    bus: &mut B,
    id: u32,
    data: [u8; 8],
) -> Result<(), BusError> {
    bus.send_frame(&CanFrame {
        id,
        data,
        extended: true,
    })
}

/// Motor bus: MIT commands + feedback cache.
pub trait MotorBus: CanBus {
    fn mit_control_all(&mut self, cmds: &[MitCommand]) -> Result<(), BusError> {
        for cmd in cmds {
            let (id, data) = mit::encode_mit(cmd);
            send_encoded_frame(self, id, data)?;
        }
        Ok(())
    }

    fn enable_drive(&mut self, device_id: u8) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_enable(device_id);
        send_encoded_frame(self, id, data)
    }

    fn disable_drive(&mut self, device_id: u8) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_disable(device_id);
        send_encoded_frame(self, id, data)
    }

    fn set_zero_position(&mut self, device_id: u8) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_set_zero_position(device_id);
        send_encoded_frame(self, id, data)
    }

    fn read_parameter(&mut self, device_id: u8, parameter: ParameterId) -> Result<(), BusError> {
        let (id, data) = params::encode_read_parameter(comm::DEFAULT_HOST_ID, device_id, parameter);
        send_encoded_frame(self, id, data)
    }

    fn write_parameter(
        &mut self,
        device_id: u8,
        parameter: ParameterId,
        value: ParameterValue,
    ) -> Result<(), BusError> {
        let (id, data) =
            params::encode_write_parameter(comm::DEFAULT_HOST_ID, device_id, parameter, value);
        send_encoded_frame(self, id, data)
    }

    fn set_run_mode(&mut self, device_id: u8, mode: RunMode) -> Result<(), BusError> {
        let (id, data) = params::encode_set_run_mode(device_id, mode);
        send_encoded_frame(self, id, data)
    }

    fn speed_control(&mut self, device_id: u8, velocity_rad_s: f32) -> Result<(), BusError> {
        let (id, data) = params::encode_speed_ref(device_id, velocity_rad_s);
        send_encoded_frame(self, id, data)
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
        loop {
            frames.clear();
            self.recv_frames(&mut frames)?;
            if frames.is_empty() {
                if count > 0 {
                    return Ok(count);
                }
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_micros(200));
                continue;
            }
            for frame in &frames {
                if !frame.extended {
                    continue;
                }
                let Some(ext) = comm::unpack_ext_id(frame.id) else {
                    continue;
                };
                match CommunicationType::from_u8(ext.comm_type) {
                    Some(CommunicationType::OperationStatus) => {
                        let Some(motor_type) = motor_types.get(&ext.device_id).copied() else {
                            continue;
                        };
                        if let Some(fb) =
                            mit::decode_mit_feedback(motor_type, frame.id, frame.data.as_slice())
                        {
                            states.insert(
                                ext.device_id,
                                MotorState {
                                    position_rad: fb.position_rad,
                                    velocity_rad_s: fb.velocity_rad_s,
                                    torque_nm: fb.torque_nm,
                                    temperature_c: fb.temperature_c,
                                    fault: fb.fault,
                                    updated: Some(Instant::now()),
                                },
                            );
                            count += 1;
                        }
                    }
                    Some(CommunicationType::FaultReport) => {
                        if !motor_types.contains_key(&ext.device_id) {
                            continue;
                        }
                        if let Some(report) = decode_fault_report(frame.id, frame.data.as_slice()) {
                            let state = states.entry(report.device_id).or_default();
                            state.fault = report.fault;
                            state.updated = Some(Instant::now());
                            count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(BusError::RecvTimeout)
    }
}

fn decode_fault_report(can_id: u32, data: &[u8]) -> Option<FaultReport> {
    let unpacked = comm::unpack_ext_id(can_id)?;
    if CommunicationType::from_u8(unpacked.comm_type)? != CommunicationType::FaultReport {
        return None;
    }
    let fault = if data.len() >= 2 {
        u16::from_le_bytes([data[0], data[1]])
    } else {
        unpacked.extra_data
    };
    Some(FaultReport {
        device_id: unpacked.device_id,
        fault,
    })
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

/// Runtime-selected bus for binaries that need config/CLI bus selection.
#[derive(Debug)]
pub enum RuntimeBus {
    Memory(MemoryBus),
    #[cfg(all(feature = "vcan", target_os = "linux"))]
    Socket(SocketCanBus),
}

impl RuntimeBus {
    pub fn memory() -> Self {
        Self::Memory(MemoryBus::default())
    }

    pub fn socketcan(interface: &str) -> Result<Self, BusError> {
        #[cfg(all(feature = "vcan", target_os = "linux"))]
        {
            SocketCanBus::open(interface).map(Self::Socket)
        }
        #[cfg(not(all(feature = "vcan", target_os = "linux")))]
        {
            let _ = interface;
            Err(BusError::Driver(
                "SocketCAN support requires building robstride with the vcan feature on Linux"
                    .to_string(),
            ))
        }
    }
}

impl CanBus for RuntimeBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        match self {
            Self::Memory(bus) => bus.send_frame(frame),
            #[cfg(all(feature = "vcan", target_os = "linux"))]
            Self::Socket(bus) => bus.send_frame(frame),
        }
    }

    fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        match self {
            Self::Memory(bus) => bus.recv_frames(out),
            #[cfg(all(feature = "vcan", target_os = "linux"))]
            Self::Socket(bus) => bus.recv_frames(out),
        }
    }
}

impl MotorBus for RuntimeBus {}

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
    use std::io::ErrorKind;

    use ::socketcan::{CanFrame as SocketFrame, CanSocket, EmbeddedFrame, ExtendedId, Frame, Socket};

    #[derive(Debug)]
    pub struct SocketCanBus {
        socket: CanSocket,
    }

    impl SocketCanBus {
        pub fn open(interface: &str) -> Result<Self, BusError> {
            let socket = CanSocket::open(interface).map_err(|e| BusError::Send {
                message: e.to_string(),
            })?;
            socket
                .set_read_timeout(Duration::from_millis(1))
                .map_err(|e| BusError::Driver(e.to_string()))?;
            Ok(Self { socket })
        }
    }

    impl CanBus for SocketCanBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            let frame = if frame.extended {
                let id = ExtendedId::new(frame.id).ok_or_else(|| BusError::Send {
                    message: format!("invalid extended id {}", frame.id),
                })?;
                SocketFrame::new(id, &frame.data).ok_or_else(|| BusError::Send {
                    message: "invalid extended frame payload".to_string(),
                })?
            } else {
                use ::socketcan::StandardId;
                let id = StandardId::new(frame.id as u16).ok_or_else(|| BusError::Send {
                    message: format!("invalid standard id {}", frame.id),
                })?;
                SocketFrame::new(id, &frame.data).ok_or_else(|| BusError::Send {
                    message: "invalid standard frame payload".to_string(),
                })?
            };
            self.socket.write_frame(&frame).map_err(|e| BusError::Send {
                message: e.to_string(),
            })
        }

        fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
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
                    Err(e)
                        if matches!(
                            e.kind(),
                            ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                        ) =>
                    {
                        break;
                    }
                    Err(e) => return Err(BusError::Driver(e.to_string())),
                }
            }
            Ok(())
        }
    }

    impl MotorBus for SocketCanBus {}
}

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub use socketcan::SocketCanBus;
