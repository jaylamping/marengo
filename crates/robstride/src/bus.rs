//! CAN bus abstraction for Robstride MIT frames.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::{MotorEntry, MotorType, MotorsConfigFile};
use thiserror::Error;

use crate::comm::{self, CommunicationType};
use crate::lifecycle;
use crate::mit::{self, MitCommand};
use crate::params::{self, ParameterId, ParameterValue, RunMode};
use crate::protocol::MotionCommand;
use crate::state::MotorState;

fn trace_skipped_frame(
    interface: Option<&str>,
    can_id: u32,
    reason: &'static str,
    device_id: Option<u8>,
    comm_type: Option<u8>,
) {
    tracing::trace!(
        interface = interface.unwrap_or("unknown"),
        can_id = format_args!("{can_id:#010x}"),
        device_id,
        comm_type,
        reason,
        "ignoring CAN frame"
    );
}

#[derive(Debug, Error)]
pub enum BusError {
    #[error("CAN send failed: {message}")]
    Send { message: String },
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("unknown motor address {interface}:{device_id}")]
    UnknownMotorAddress { interface: String, device_id: u8 },
    #[error("duplicate motor address {interface}:{device_id}")]
    DuplicateMotorAddress { interface: String, device_id: u8 },
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

/// Static bus address for one configured motor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MotorAddress {
    pub interface: String,
    pub device_id: u8,
}

impl MotorAddress {
    pub fn new(interface: impl Into<String>, device_id: u8) -> Self {
        Self {
            interface: interface.into(),
            device_id,
        }
    }
}

impl From<&MotorEntry> for MotorAddress {
    fn from(motor: &MotorEntry) -> Self {
        Self::new(motor.can_interface.clone(), motor.device_id)
    }
}

/// MIT command with the routing context needed for multi-bus SocketCAN.
#[derive(Debug, Clone, PartialEq)]
pub struct AddressedMitCommand {
    pub address: MotorAddress,
    pub command: MitCommand,
}

/// Received frame plus the interface it was drained from when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedCanFrame {
    pub interface: Option<String>,
    pub frame: CanFrame,
}

/// Sends encoded Robstride frames.
pub trait CanBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError>;

    fn send_frame_to(&mut self, _address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.send_frame(frame)
    }

    /// Drain received frames into `out` (non-blocking). Default: no RX.
    fn recv_frames(&mut self, _out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        Ok(())
    }

    /// Drain received frames with source interface when the backend can provide it.
    fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
        let mut frames = Vec::new();
        self.recv_frames(&mut frames)?;
        out.extend(frames.into_iter().map(|frame| ReceivedCanFrame {
            interface: None,
            frame,
        }));
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

fn send_encoded_frame_to<B: CanBus + ?Sized>(
    bus: &mut B,
    address: &MotorAddress,
    id: u32,
    data: [u8; 8],
) -> Result<(), BusError> {
    tracing::trace!(
        interface = %address.interface,
        device_id = address.device_id,
        can_id = format_args!("{id:#010x}"),
        "robstride addressed tx"
    );
    bus.send_frame_to(
        address,
        &CanFrame {
            id,
            data,
            extended: true,
        },
    )
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

    fn mit_control_all_at(&mut self, cmds: &[AddressedMitCommand]) -> Result<(), BusError> {
        for cmd in cmds {
            let (id, data) = mit::encode_mit(&cmd.command);
            send_encoded_frame_to(self, &cmd.address, id, data)?;
        }
        Ok(())
    }

    fn enable_drive(&mut self, device_id: u8) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_enable(device_id);
        send_encoded_frame(self, id, data)
    }

    fn enable_drive_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_enable(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    fn disable_drive(&mut self, device_id: u8) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_disable(device_id);
        send_encoded_frame(self, id, data)
    }

    fn disable_drive_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_disable(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    fn set_zero_position(&mut self, device_id: u8) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_set_zero_position(device_id);
        send_encoded_frame(self, id, data)
    }

    fn set_zero_position_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_set_zero_position(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    fn read_parameter(&mut self, device_id: u8, parameter: ParameterId) -> Result<(), BusError> {
        let (id, data) = params::encode_read_parameter(comm::DEFAULT_HOST_ID, device_id, parameter);
        send_encoded_frame(self, id, data)
    }

    fn read_parameter_at(
        &mut self,
        address: &MotorAddress,
        parameter: ParameterId,
    ) -> Result<(), BusError> {
        let (id, data) =
            params::encode_read_parameter(comm::DEFAULT_HOST_ID, address.device_id, parameter);
        send_encoded_frame_to(self, address, id, data)
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

    fn write_parameter_at(
        &mut self,
        address: &MotorAddress,
        parameter: ParameterId,
        value: ParameterValue,
    ) -> Result<(), BusError> {
        let (id, data) = params::encode_write_parameter(
            comm::DEFAULT_HOST_ID,
            address.device_id,
            parameter,
            value,
        );
        send_encoded_frame_to(self, address, id, data)
    }

    fn set_run_mode(&mut self, device_id: u8, mode: RunMode) -> Result<(), BusError> {
        let (id, data) = params::encode_set_run_mode(device_id, mode);
        send_encoded_frame(self, id, data)
    }

    fn set_run_mode_at(&mut self, address: &MotorAddress, mode: RunMode) -> Result<(), BusError> {
        let (id, data) = params::encode_set_run_mode(address.device_id, mode);
        send_encoded_frame_to(self, address, id, data)
    }

    fn speed_control(&mut self, device_id: u8, velocity_rad_s: f32) -> Result<(), BusError> {
        let (id, data) = params::encode_speed_ref(device_id, velocity_rad_s);
        send_encoded_frame(self, id, data)
    }

    fn speed_control_at(
        &mut self,
        address: &MotorAddress,
        velocity_rad_s: f32,
    ) -> Result<(), BusError> {
        let (id, data) = params::encode_speed_ref(address.device_id, velocity_rad_s);
        send_encoded_frame_to(self, address, id, data)
    }

    fn recv_all(
        &mut self,
        motor_types: &HashMap<u8, MotorType>,
        states: &mut HashMap<u8, MotorState>,
        budget: Duration,
        quiet: Duration,
    ) -> Result<usize, BusError> {
        let deadline = Instant::now() + budget;
        let mut frames = Vec::new();
        let mut count = 0;
        let mut last_frame_at: Option<Instant> = None;
        loop {
            frames.clear();
            self.recv_frames(&mut frames)?;
            if frames.is_empty() {
                if let Some(last) = last_frame_at {
                    if count > 0 && last.elapsed() >= quiet {
                        return Ok(count);
                    }
                }
                if Instant::now() >= deadline {
                    if count > 0 {
                        return Ok(count);
                    }
                    break;
                }
                std::thread::sleep(Duration::from_micros(200));
                continue;
            }
            last_frame_at = Some(Instant::now());
            for frame in &frames {
                if !frame.extended {
                    continue;
                }
                let Some(ext) = comm::unpack_ext_id(frame.id) else {
                    continue;
                };
                match CommunicationType::from_u8(ext.comm_type) {
                    Some(CommunicationType::OperationStatus) => {
                        let device_id = comm::inbound_motor_device_id(
                            frame.id,
                            CommunicationType::OperationStatus,
                        );
                        let Some(motor_type) = motor_types.get(&device_id).copied() else {
                            continue;
                        };
                        if let Some(fb) =
                            mit::decode_mit_feedback(motor_type, frame.id, frame.data.as_slice())
                        {
                            states.insert(
                                device_id,
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
                        let device_id =
                            comm::inbound_motor_device_id(frame.id, CommunicationType::FaultReport);
                        if !motor_types.contains_key(&device_id) {
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

    fn recv_all_addressed(
        &mut self,
        motor_types: &HashMap<MotorAddress, MotorType>,
        states: &mut HashMap<MotorAddress, MotorState>,
        budget: Duration,
        quiet: Duration,
    ) -> Result<usize, BusError> {
        let deadline = Instant::now() + budget;
        let mut frames = Vec::new();
        let mut count = 0;
        let mut last_frame_at: Option<Instant> = None;
        loop {
            frames.clear();
            self.recv_frames_from(&mut frames)?;
            if frames.is_empty() {
                if let Some(last) = last_frame_at {
                    if count > 0 && last.elapsed() >= quiet {
                        return Ok(count);
                    }
                }
                if Instant::now() >= deadline {
                    if count > 0 {
                        return Ok(count);
                    }
                    break;
                }
                std::thread::sleep(Duration::from_micros(200));
                continue;
            }
            last_frame_at = Some(Instant::now());
            for received in &frames {
                let frame = &received.frame;
                if !frame.extended {
                    trace_skipped_frame(
                        received.interface.as_deref(),
                        frame.id,
                        "non-extended",
                        None,
                        None,
                    );
                    continue;
                }
                let Some(ext) = comm::unpack_ext_id(frame.id) else {
                    trace_skipped_frame(
                        received.interface.as_deref(),
                        frame.id,
                        "invalid-extended-id",
                        None,
                        None,
                    );
                    continue;
                };
                let Some(comm_type) = CommunicationType::from_u8(ext.comm_type) else {
                    continue;
                };
                let device_id = comm::inbound_motor_device_id(frame.id, comm_type);
                let Some(address) =
                    address_for_frame(motor_types, received.interface.as_deref(), device_id)
                else {
                    trace_skipped_frame(
                        received.interface.as_deref(),
                        frame.id,
                        "unconfigured-motor",
                        Some(device_id),
                        Some(ext.comm_type),
                    );
                    continue;
                };
                match comm_type {
                    CommunicationType::OperationStatus => {
                        let Some(motor_type) = motor_types.get(&address).copied() else {
                            continue;
                        };
                        if let Some(fb) =
                            mit::decode_mit_feedback(motor_type, frame.id, frame.data.as_slice())
                        {
                            tracing::trace!(
                                interface = %address.interface,
                                device_id = address.device_id,
                                position_rad = fb.position_rad,
                                velocity_rad_s = fb.velocity_rad_s,
                                torque_nm = fb.torque_nm,
                                temperature_c = fb.temperature_c,
                                fault = fb.fault,
                                "decoded Robstride status"
                            );
                            states.insert(
                                address,
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
                        } else {
                            tracing::trace!(
                                interface = %address.interface,
                                device_id = address.device_id,
                                can_id = format_args!("{:#010x}", frame.id),
                                "failed to decode Robstride status frame"
                            );
                        }
                    }
                    CommunicationType::FaultReport => {
                        if let Some(report) = decode_fault_report(frame.id, frame.data.as_slice()) {
                            tracing::warn!(
                                interface = %address.interface,
                                device_id = report.device_id,
                                fault = format_args!("{:#06x}", report.fault),
                                "Robstride fault report"
                            );
                            let state = states.entry(address).or_default();
                            state.fault = report.fault;
                            state.updated = Some(Instant::now());
                            count += 1;
                        }
                    }
                    _ => {
                        trace_skipped_frame(
                            received.interface.as_deref(),
                            frame.id,
                            "unsupported-comm-type",
                            Some(address.device_id),
                            Some(ext.comm_type),
                        );
                    }
                }
            }
        }
        Err(BusError::RecvTimeout)
    }
}

fn address_for_frame(
    motor_types: &HashMap<MotorAddress, MotorType>,
    interface: Option<&str>,
    device_id: u8,
) -> Option<MotorAddress> {
    if let Some(interface) = interface {
        let address = MotorAddress::new(interface.to_string(), device_id);
        return motor_types.contains_key(&address).then_some(address);
    }

    let mut matches = motor_types
        .keys()
        .filter(|address| address.device_id == device_id)
        .cloned();
    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
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
        device_id: comm::inbound_motor_device_id(can_id, CommunicationType::FaultReport),
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
    #[cfg(all(feature = "socketcan", target_os = "linux"))]
    Socket(SocketCanBus),
    #[cfg(all(feature = "socketcan", target_os = "linux"))]
    Router(SocketCanRouter),
}

impl RuntimeBus {
    pub fn socketcan(interface: &str) -> Result<Self, BusError> {
        #[cfg(all(feature = "socketcan", target_os = "linux"))]
        {
            SocketCanBus::open(interface).map(Self::Socket)
        }
        #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
        {
            let _ = interface;
            Err(BusError::Driver(
                "SocketCAN support requires building robstride with the socketcan feature on Linux"
                    .to_string(),
            ))
        }
    }

    pub fn socketcan_from_motors(motors: &MotorsConfigFile) -> Result<Self, BusError> {
        #[cfg(all(feature = "socketcan", target_os = "linux"))]
        {
            SocketCanRouter::open(motors).map(Self::Router)
        }
        #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
        {
            let _ = motors;
            Err(BusError::Driver(
                "SocketCAN support requires building robstride with the socketcan feature on Linux"
                    .to_string(),
            ))
        }
    }
}

impl CanBus for RuntimeBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        let _ = frame;
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.send_frame(frame),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.send_frame(frame),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => Err(socketcan_unavailable()),
        }
    }

    fn send_frame_to(&mut self, address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        let _ = (address, frame);
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.send_frame_to(address, frame),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.send_frame_to(address, frame),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => Err(socketcan_unavailable()),
        }
    }

    fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        let _ = out;
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.recv_frames(out),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.recv_frames(out),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => Err(socketcan_unavailable()),
        }
    }

    fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
        let _ = out;
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.recv_frames_from(out),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.recv_frames_from(out),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => Err(socketcan_unavailable()),
        }
    }
}

impl MotorBus for RuntimeBus {}

#[cfg(not(all(feature = "socketcan", target_os = "linux")))]
fn socketcan_unavailable() -> BusError {
    BusError::Driver(
        "SocketCAN support requires building robstride with the socketcan feature on Linux"
            .to_string(),
    )
}

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

#[cfg(all(feature = "socketcan", target_os = "linux"))]
mod socketcan {
    use super::*;
    use std::collections::{BTreeSet, HashMap, HashSet};
    use std::io::ErrorKind;

    use ::socketcan::{
        CanFrame as SocketFrame, CanSocket, EmbeddedFrame, ExtendedId, Frame, Socket, SocketOptions,
    };

    fn configure_vcan_loopback(socket: &CanSocket, interface: &str) -> Result<(), BusError> {
        if !interface.starts_with("vcan") {
            return Ok(());
        }
        socket
            .set_loopback(true)
            .and_then(|_| socket.set_recv_own_msgs(true))
            .map_err(|e| BusError::Driver(e.to_string()))
    }

    #[derive(Debug)]
    pub struct SocketCanBus {
        interface: String,
        socket: CanSocket,
    }

    impl SocketCanBus {
        pub fn open(interface: &str) -> Result<Self, BusError> {
            tracing::debug!(interface, "opening SocketCAN interface");
            let socket = CanSocket::open(interface).map_err(|e| BusError::Send {
                message: e.to_string(),
            })?;
            configure_vcan_loopback(&socket, interface)?;
            socket
                .set_read_timeout(Duration::from_millis(1))
                .map_err(|e| BusError::Driver(e.to_string()))?;
            tracing::info!(interface, "opened SocketCAN interface");
            Ok(Self {
                interface: interface.to_string(),
                socket,
            })
        }
    }

    impl CanBus for SocketCanBus {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            tracing::trace!(
                interface = %self.interface,
                can_id = format_args!("{:#010x}", frame.id),
                extended = frame.extended,
                dlc = frame.data.len(),
                "SocketCAN tx"
            );
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

        fn send_frame_to(
            &mut self,
            _address: &MotorAddress,
            frame: &CanFrame,
        ) -> Result<(), BusError> {
            self.send_frame(frame)
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
                        tracing::trace!(
                            interface = %self.interface,
                            can_id = format_args!("{id:#010x}"),
                            extended = frame.is_extended(),
                            dlc = payload.len(),
                            "SocketCAN rx"
                        );
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
                    Err(e) => {
                        tracing::warn!(
                            interface = %self.interface,
                            error = %e,
                            "SocketCAN receive error"
                        );
                        return Err(BusError::Driver(e.to_string()));
                    }
                }
            }
            Ok(())
        }

        fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
            let mut frames = Vec::new();
            self.recv_frames(&mut frames)?;
            out.extend(frames.into_iter().map(|frame| ReceivedCanFrame {
                interface: Some(self.interface.clone()),
                frame,
            }));
            Ok(())
        }
    }

    impl MotorBus for SocketCanBus {}

    #[derive(Debug)]
    pub struct SocketCanRouter {
        sockets: HashMap<String, SocketCanBus>,
        addresses: HashSet<MotorAddress>,
    }

    impl SocketCanRouter {
        pub fn open(motors: &MotorsConfigFile) -> Result<Self, BusError> {
            let mut addresses = HashSet::new();
            let mut interfaces = BTreeSet::new();
            for motor in &motors.motors {
                let address = MotorAddress::from(motor);
                if !addresses.insert(address.clone()) {
                    tracing::warn!(
                        interface = %address.interface,
                        device_id = address.device_id,
                        joint = %motor.joint,
                        "duplicate configured motor address"
                    );
                    return Err(BusError::DuplicateMotorAddress {
                        interface: address.interface,
                        device_id: address.device_id,
                    });
                }
                interfaces.insert(motor.can_interface.clone());
            }

            tracing::info!(
                motor_count = motors.motors.len(),
                interfaces = ?interfaces,
                "opening routed SocketCAN bus"
            );
            let mut sockets = HashMap::new();
            for interface in interfaces {
                sockets.insert(interface.clone(), SocketCanBus::open(&interface)?);
            }

            Ok(Self { sockets, addresses })
        }
    }

    impl CanBus for SocketCanRouter {
        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            if self.sockets.len() == 1 {
                let Some(socket) = self.sockets.values_mut().next() else {
                    tracing::warn!("routed SocketCAN has no configured interfaces");
                    return Err(BusError::Driver(
                        "no SocketCAN interfaces configured".to_string(),
                    ));
                };
                return socket.send_frame(frame);
            }
            tracing::warn!(
                can_id = format_args!("{:#010x}", frame.id),
                "routed SocketCAN send missing motor address"
            );
            Err(BusError::Driver(
                "routed SocketCAN requires a configured motor address".to_string(),
            ))
        }

        fn send_frame_to(
            &mut self,
            address: &MotorAddress,
            frame: &CanFrame,
        ) -> Result<(), BusError> {
            if !self.addresses.contains(address) {
                tracing::warn!(
                    interface = %address.interface,
                    device_id = address.device_id,
                    can_id = format_args!("{:#010x}", frame.id),
                    "send to unconfigured SocketCAN motor address"
                );
                return Err(BusError::UnknownMotorAddress {
                    interface: address.interface.clone(),
                    device_id: address.device_id,
                });
            }
            let socket = self.sockets.get_mut(&address.interface).ok_or_else(|| {
                tracing::warn!(
                    interface = %address.interface,
                    device_id = address.device_id,
                    "configured SocketCAN interface missing from router"
                );
                BusError::UnknownMotorAddress {
                    interface: address.interface.clone(),
                    device_id: address.device_id,
                }
            })?;
            socket.send_frame(frame)
        }

        fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
            for socket in self.sockets.values_mut() {
                socket.recv_frames(out)?;
            }
            Ok(())
        }

        fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
            for socket in self.sockets.values_mut() {
                socket.recv_frames_from(out)?;
            }
            Ok(())
        }
    }

    impl MotorBus for SocketCanRouter {}
}

#[cfg(all(feature = "socketcan", target_os = "linux"))]
pub use socketcan::{SocketCanBus, SocketCanRouter};
