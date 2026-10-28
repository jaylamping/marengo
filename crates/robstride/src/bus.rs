//! CAN bus abstraction for Robstride MIT frames.

use std::collections::{HashMap, VecDeque};
use std::ops::RangeBounds;
use std::time::{Duration, Instant};

use marengo_config::{MotorEntry, MotorType, MotorsConfigFile};
use thiserror::Error;

use crate::comm::{self, CommunicationType};
use crate::command::CommandError;
use crate::feedback::{
    decode_status_header, DetailedFaultFeedback, EchoedCommand, FeedbackEvent, FeedbackObservation,
    FeedbackReport, HostEchoObservation, IdentityObservation, MalformedFeedback, MalformedReason,
    ParameterReadObservation, TransportObservation,
};
use crate::identity;
use crate::lifecycle;
use crate::mit::{self, MitCommand};
use crate::params::{self, ParameterId, ParameterValue, RunMode};
use crate::receive::{RawReceiveReport, ReceiveAttempt, ReceiveCompletion, ReceiveLimits};
use crate::version;

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
/// Extended communication types this host may transmit: identity (0), MIT (1),
/// Enable (3), Disable (4), SetZero (6), parameter read/write (17/18) and
/// active reporting (24). Everything else is refused before the wire, including
/// the persistent or destructive set-CAN-id (7), parameter save (22), baud
/// (23) and protocol switch (25), and drive-direction types (2, 21).
const TX_COMMUNICATION_TYPES: [u8; 8] = [0, 1, 3, 4, 6, 17, 18, 24];

fn validate_can_tx(frame: &CanFrame) -> Result<(), BusError> {
    let comm_type = ((frame.id >> 24) & 0x1f) as u8;
    if frame.extended && !TX_COMMUNICATION_TYPES.contains(&comm_type) {
        return Err(CommandError::ForbiddenCommunicationType { comm_type }.into());
    }
    Ok(())
}

/// Whether an extended identifier is a host command: a host-only type (MIT,
/// Enable, Disable, SetZero, set id, parameter write/save, baud, protocol), or
/// an identity, parameter-read or active-reporting request from this host id.
/// Drive frames (types 2 and 21, and type 0/17/24 replies and reports) carry a
/// drive id where a request carries the host id, so they never match.
#[cfg(any(test, all(feature = "socketcan", target_os = "linux")))]
fn is_host_command(can_id: u32) -> bool {
    let Some(ext) = comm::unpack_ext_id(can_id) else {
        return false;
    };
    match ext.comm_type {
        1 | 3 | 4 | 6 | 7 | 18 | 22 | 23 | 25 => true,
        0 | 17 | 24 => ext.extra_data == u16::from(comm::DEFAULT_HOST_ID),
        _ => false,
    }
}

/// Transmissions still awaiting their echo. Bounded above the interface TX
/// queue (`txqueuelen 1000`); a frame whose echo never comes (dropped at
/// bus-off) is eventually displaced, and an echo that arrives late still
/// matches while it is held.
#[cfg(any(test, all(feature = "socketcan", target_os = "linux")))]
const MAX_PENDING_ECHOES: usize = 1024;

/// What one socket wrote and has not yet read back. Every local CAN socket
/// receives every other local socket's frames (loopback), and the kernel's
/// own-socket flag (`MSG_CONFIRM`) is dropped by the safe `recvmsg` wrapper
/// in use, so this socket attributes a host command to itself only when it
/// matches a frame it transmitted and has not yet seen echoed.
#[cfg(any(test, all(feature = "socketcan", target_os = "linux")))]
#[derive(Debug, Default)]
pub(crate) struct EchoLedger {
    pending: VecDeque<(u32, [u8; 8])>,
}

#[cfg(any(test, all(feature = "socketcan", target_os = "linux")))]
impl EchoLedger {
    /// Record a frame the kernel accepted for transmission on this socket.
    pub(crate) fn transmitted(&mut self, frame: &CanFrame) {
        if !frame.extended {
            return;
        }
        if self.pending.len() == MAX_PENDING_ECHOES {
            self.pending.pop_front();
        }
        self.pending.push_back((frame.id, frame.data));
    }

    /// Consume this socket's matching transmission, if the frame is its echo.
    fn claim(&mut self, received: &ReceivedCanFrame) -> bool {
        if received.kind != RxFrameKind::Data
            || !received.frame.extended
            || received.payload_len != 8
        {
            return false;
        }
        let key = (received.frame.id, received.frame.data);
        match self.pending.iter().position(|pending| *pending == key) {
            Some(index) => {
                self.pending.remove(index);
                true
            }
            None => false,
        }
    }

    /// Admit a received frame, or refuse a host command this socket did not
    /// send: another process (motor-repl, `cansend`, a second marengo-pi) or
    /// another host is writing to drives this socket's owner commands. The
    /// refusal is a receive failure, so the owner latches Transport and stops
    /// instead of taking the frame as its own wire-order evidence.
    pub(crate) fn admit(
        &mut self,
        interface: &str,
        received: &ReceivedCanFrame,
    ) -> Result<(), BusError> {
        if self.claim(received)
            || received.kind == RxFrameKind::Error
            || !received.frame.extended
            || !is_host_command(received.frame.id)
        {
            return Ok(());
        }
        tracing::warn!(
            interface,
            can_id = format_args!("{:#010x}", received.frame.id),
            "host command this socket did not send"
        );
        Err(BusError::Driver(format!(
            "SocketCAN {interface}: host command {:#010x} that this socket did not send; \
             another CAN writer is active",
            received.frame.id
        )))
    }
}

/// Frames the kernel dropped since the last report, from the cumulative
/// `SO_RXQ_OVFL` count attached to a received frame (absent until the first
/// drop). Every new drop is reported once.
#[cfg(any(test, all(feature = "socketcan", target_os = "linux")))]
pub(crate) fn new_kernel_drops(reported: &mut u32, cumulative: Option<u32>) -> Option<u32> {
    let cumulative = cumulative?;
    let new = cumulative.wrapping_sub(*reported);
    if new == 0 {
        return None;
    }
    *reported = cumulative;
    Some(new)
}

#[derive(Debug, Error)]
pub enum BusError {
    #[error("invalid motor command: {0}")]
    InvalidCommand(#[from] CommandError),
    #[error("CAN send failed: {message}")]
    Send { message: String },
    #[error("unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("unknown motor address {interface}:{device_id}")]
    UnknownMotorAddress { interface: String, device_id: u8 },
    #[error("duplicate motor address {interface}:{device_id}")]
    DuplicateMotorAddress { interface: String, device_id: u8 },
    #[error("duplicate command for motor {device_id}")]
    DuplicateCommand { device_id: u8 },
    #[error("command motor {command_device_id} does not match routed address {address:?}")]
    CommandAddressMismatch {
        address: MotorAddress,
        command_device_id: u8,
    },
    #[error("driver error: {0}")]
    Driver(String),
    #[error("recv timeout")]
    RecvTimeout,
    #[error("receive ended without observed quiescence: {completion:?}")]
    ReceiveIncomplete { completion: ReceiveCompletion },
    #[error("malformed feedback from {address:?}: {reason}")]
    MalformedFeedback {
        address: MotorAddress,
        reason: MalformedReason,
    },
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
    pub payload_len: u8,
    pub kind: RxFrameKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxFrameKind {
    Data,
    Remote { requested_len: u8 },
    Error,
}

impl ReceivedCanFrame {
    /// Explicitly full-size synthetic data; never use to infer SocketCAN RX length.
    pub fn full_data(interface: Option<String>, frame: CanFrame) -> Self {
        Self {
            interface,
            frame,
            payload_len: 8,
            kind: RxFrameKind::Data,
        }
    }

    pub fn new_data(
        interface: Option<String>,
        id: u32,
        extended: bool,
        payload: &[u8],
    ) -> Result<Self, BusError> {
        if payload.len() > 8 {
            return Err(BusError::Driver(
                "classic CAN payload exceeds eight bytes".into(),
            ));
        }
        let mut data = [0; 8];
        data[..payload.len()].copy_from_slice(payload);
        Ok(Self {
            interface,
            frame: CanFrame { id, data, extended },
            payload_len: payload.len() as u8,
            kind: RxFrameKind::Data,
        })
    }

    pub fn new_remote(
        interface: Option<String>,
        id: u32,
        extended: bool,
        requested_len: u8,
    ) -> Result<Self, BusError> {
        if requested_len > 8 {
            return Err(BusError::Driver(
                "classic remote request exceeds eight bytes".into(),
            ));
        }
        Ok(Self {
            interface,
            frame: CanFrame {
                id,
                data: [0; 8],
                extended,
            },
            payload_len: 0,
            kind: RxFrameKind::Remote { requested_len },
        })
    }

    /// Convert a typed SocketCAN receive without inventing payload bytes or
    /// treating remote/error frames as vendor data. FD is deliberately unsupported.
    #[cfg(all(feature = "socketcan", target_os = "linux"))]
    pub fn from_socketcan(
        interface: Option<String>,
        frame: ::socketcan::CanAnyFrame,
    ) -> Result<Self, BusError> {
        use ::socketcan::{CanAnyFrame, EmbeddedFrame, Frame};
        match frame {
            CanAnyFrame::Normal(data) => {
                if data.dlc() > 8 {
                    return Err(BusError::Driver("invalid classic CAN data length".into()));
                }
                Self::new_data(interface, data.raw_id(), data.is_extended(), data.data())
            }
            CanAnyFrame::Remote(remote) => Self::new_remote(
                interface,
                remote.raw_id(),
                remote.is_extended(),
                remote.dlc() as u8,
            ),
            CanAnyFrame::Error(error) => {
                let length = error.dlc();
                if length > 8 {
                    return Err(BusError::Driver("invalid classic CAN error length".into()));
                }
                // raw_id() masks non-EFF frames to eleven bits; error_bits()
                // retains the full error domain, including unknown classes.
                let mut received = Self::new_data(
                    interface,
                    error.error_bits(),
                    error.is_extended(),
                    &error.data()[..length],
                )?;
                received.kind = RxFrameKind::Error;
                Ok(received)
            }
            CanAnyFrame::Fd(_) => Err(BusError::Driver(
                "received unsupported CAN FD frame on classic CAN backend".into(),
            )),
        }
    }

    /// Only the prefix below payload_len represents actually received bytes.
    pub fn payload(&self) -> Option<&[u8]> {
        if self.payload_len > 8 {
            return None;
        }
        match self.kind {
            RxFrameKind::Data | RxFrameKind::Error => {
                Some(&self.frame.data[..usize::from(self.payload_len)])
            }
            RxFrameKind::Remote { requested_len }
                if requested_len <= 8 && self.payload_len == 0 =>
            {
                Some(&[])
            }
            RxFrameKind::Remote { .. } => None,
        }
    }
}

/// Frame with the host instant it was received. SocketCAN maps the kernel's
/// receive stamp onto `Instant` (never later than the read, see
/// [`crate::rx_time`]); other backends stamp at read or handoff. Clock
/// resolution may give consecutive observations equal times; list order still
/// distinguishes them. These timestamps do not prove physical acquisition
/// times or epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedCanFrame {
    pub received_at: Instant,
    pub received: ReceivedCanFrame,
}

/// Sends encoded Robstride frames.
pub trait CanBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError>;

    /// Required one-source, one-attempt receive. Must not wait, bulk drain, or
    /// retry interruptions. Multi-source backends advance their stable cursor
    /// even on idle, interruption or error.
    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError>;

    /// Stable throughout one poll; an idle pass requires visiting every source.
    fn receive_source_count(&self) -> usize {
        1
    }

    /// Begin a poll in bounded work. Routers rotate their starting source here.
    fn begin_receive(&mut self) {}

    /// The receive stream includes this host's own transmissions at the point
    /// they went on the wire (SocketCAN `IFF_ECHO` with `CAN_RAW_RECV_OWN_MSGS`),
    /// ordered against drive traffic. A successful write only means the frame
    /// was queued; consumers needing wire order wait for its echo instead.
    /// Echoes are raw reads (they share the receive bound), never drive feedback.
    /// SocketCAN attributes host commands to the socket that sent them: one this
    /// socket did not send fails the receive instead of reading as an echo.
    fn echoes_transmissions(&self) -> bool {
        false
    }

    fn recv_raw_report(
        &mut self,
        budget: Duration,
        quiet: Duration,
        limits: ReceiveLimits,
    ) -> RawReceiveReport {
        crate::receive::drain(self, budget, quiet, limits)
    }

    /// Check a route without transmitting. Configured backends override this so
    /// an invalid later destination cannot allow a valid batch prefix to escape.
    fn validate_address(&self, _address: &MotorAddress) -> Result<(), BusError> {
        Ok(())
    }

    fn send_frame_to(&mut self, _address: &MotorAddress, frame: &CanFrame) -> Result<(), BusError> {
        self.send_frame(frame)
    }
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
    /// Validate the complete batch before emitting the first motion frame.
    /// Transport failures can still occur after some valid frames were sent.
    fn mit_control_all_at(&mut self, cmds: &[AddressedMitCommand]) -> Result<(), BusError> {
        for (index, cmd) in cmds.iter().enumerate() {
            cmd.command.validate()?;
            if cmd.command.device_id != cmd.address.device_id {
                return Err(BusError::CommandAddressMismatch {
                    address: cmd.address.clone(),
                    command_device_id: cmd.command.device_id,
                });
            }
            if cmds[..index]
                .iter()
                .any(|prior| prior.address == cmd.address)
            {
                return Err(BusError::DuplicateMotorAddress {
                    interface: cmd.address.interface.clone(),
                    device_id: cmd.address.device_id,
                });
            }
            self.validate_address(&cmd.address)?;
        }
        for cmd in cmds {
            let (id, data) = mit::encode_mit(&cmd.command)?;
            send_encoded_frame_to(self, &cmd.address, id, data)?;
        }
        Ok(())
    }

    fn enable_drive_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_enable(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    fn disable_drive_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_disable(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    fn set_zero_position_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_set_zero_position(address.device_id);
        send_encoded_frame_to(self, address, id, data)
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

    /// Communication type 0: request the 64-bit MCU unique identifier (manual §4.1.1).
    fn get_device_id_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = identity::encode_default_get_device_id(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    /// Communication type 4 with `C4`: a Disable that also requests the firmware
    /// version (see [`crate::version`]). Byte[0] stays 0, so it never clears a fault.
    fn get_firmware_version_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = version::encode_default_get_firmware_version(address.device_id);
        send_encoded_frame_to(self, address, id, data)
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
        )?;
        send_encoded_frame_to(self, address, id, data)
    }

    fn set_run_mode_at(&mut self, address: &MotorAddress, mode: RunMode) -> Result<(), BusError> {
        let (id, data) = params::encode_set_run_mode(address.device_id, mode);
        send_encoded_frame_to(self, address, id, data)
    }

    fn speed_control_at(
        &mut self,
        address: &MotorAddress,
        velocity_rad_s: f32,
    ) -> Result<(), BusError> {
        let (id, data) = params::encode_speed_ref(address.device_id, velocity_rad_s)?;
        send_encoded_frame_to(self, address, id, data)
    }

    fn enable_active_reporting_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_active_reporting(address.device_id, true);
        send_encoded_frame_to(self, address, id, data)
    }

    fn disable_active_reporting_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = lifecycle::encode_default_active_reporting(address.device_id, false);
        send_encoded_frame_to(self, address, id, data)
    }

    /// Lossless bounded receive. Zero budget never waits; positive budget waits
    /// only in the shared engine, which retains source-round completion.
    fn recv_feedback_report(
        &mut self,
        motor_types: &HashMap<MotorAddress, MotorType>,
        budget: Duration,
        quiet: Duration,
    ) -> FeedbackReport {
        self.recv_feedback_report_with_limits(motor_types, budget, quiet, ReceiveLimits::default())
    }

    fn recv_feedback_report_with_limits(
        &mut self,
        motor_types: &HashMap<MotorAddress, MotorType>,
        budget: Duration,
        quiet: Duration,
        limits: ReceiveLimits,
    ) -> FeedbackReport {
        let raw = self.recv_raw_report(budget, quiet, limits);
        feedback_from_raw(motor_types, raw, budget)
    }
}

fn feedback_from_raw(
    motor_types: &HashMap<MotorAddress, MotorType>,
    raw: RawReceiveReport,
    budget: Duration,
) -> FeedbackReport {
    let mut report = FeedbackReport {
        completion: raw.completion,
        raw_frames: raw.frames.len() + raw.ignored_frames,
        ignored_frames: raw.ignored_frames,
        read_attempts: raw.read_attempts,
        terminal_error: raw.terminal_error,
        terminal_error_order: raw.terminal_error_order,
        ..FeedbackReport::default()
    };
    ingest_feedback_frames(motor_types, &mut report, &raw.frames);
    if !budget.is_zero()
        && report.observations.is_empty()
        && report.host_echoes.is_empty()
        && report.transport_frames.is_empty()
        && report.identities.is_empty()
        && report.parameter_reads.is_empty()
        && report.terminal_error.is_none()
    {
        report.terminal_error = Some(BusError::RecvTimeout);
    }
    report
}

fn ingest_feedback_frames(
    motor_types: &HashMap<MotorAddress, MotorType>,
    report: &mut FeedbackReport,
    frames: &[TimedCanFrame],
) {
    for (order, timed) in frames.iter().enumerate() {
        let received = &timed.received;
        let frame = &received.frame;
        if received.kind == RxFrameKind::Error {
            report.transport_frames.push(TransportObservation {
                order,
                frame: timed.clone(),
            });
            continue;
        }
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
            trace_skipped_frame(
                received.interface.as_deref(),
                frame.id,
                "unknown-comm-type",
                Some((frame.id & 0xff) as u8),
                Some(ext.comm_type),
            );
            continue;
        };
        if matches!(
            comm_type,
            CommunicationType::GetDeviceId | CommunicationType::ReadParameter
        ) {
            ingest_reply_frame(motor_types, report, order, timed, comm_type);
            continue;
        }
        let echoed = match comm_type {
            CommunicationType::Enable => Some(EchoedCommand::Enable),
            CommunicationType::SetZeroPosition => Some(EchoedCommand::SetZero),
            _ => None,
        };
        if let Some(command) = echoed {
            ingest_host_echo(motor_types, report, order, timed, command);
            continue;
        }
        let device_id = comm::inbound_motor_device_id(frame.id, comm_type);
        if device_id == comm::DEFAULT_HOST_ID {
            // A type-24 command from a host carries the host id where reports
            // carry the drive. SocketCAN delivers only this socket's own echo
            // here; another writer's host command fails the receive instead.
            if comm_type == CommunicationType::ActiveReporting {
                ingest_host_echo(
                    motor_types,
                    report,
                    order,
                    timed,
                    EchoedCommand::ReportingOff,
                );
            } else {
                trace_skipped_frame(
                    received.interface.as_deref(),
                    frame.id,
                    "host-command",
                    None,
                    Some(ext.comm_type),
                );
            }
            continue;
        }
        let Some((address, motor_type)) =
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
        if !matches!(
            comm_type,
            CommunicationType::OperationStatus
                | CommunicationType::ActiveReporting
                | CommunicationType::FaultReport
        ) {
            trace_skipped_frame(
                received.interface.as_deref(),
                frame.id,
                "unsupported-comm-type",
                Some(device_id),
                Some(ext.comm_type),
            );
            continue;
        }
        let malformed = if received.payload().is_none() {
            Some(MalformedReason::InvalidEnvelope)
        } else if received.kind != RxFrameKind::Data {
            Some(MalformedReason::NonDataFrame)
        } else if received.payload_len != 8 {
            Some(MalformedReason::PayloadLength {
                received: received.payload_len,
            })
        } else {
            None
        };
        let event = if let Some(reason) = malformed {
            let status_header = received.kind == RxFrameKind::Data
                && matches!(
                    comm_type,
                    CommunicationType::OperationStatus | CommunicationType::ActiveReporting
                );
            let (drive_mode, status_flags) = decode_status_header(frame.id);
            FeedbackEvent::Malformed(MalformedFeedback {
                raw: frame.data,
                payload_len: received.payload_len,
                kind: received.kind,
                reason,
                status_flags: status_header.then_some(status_flags),
                drive_mode: status_header.then_some(drive_mode),
            })
        } else {
            match comm_type {
                CommunicationType::OperationStatus | CommunicationType::ActiveReporting => {
                    // Eight-byte Data payload and status comm type were checked above.
                    // A `00 C4 56` type-2 payload answers a version query: never a pose.
                    match version::decode_firmware_version_reply(frame.id, &frame.data) {
                        Some(version) => FeedbackEvent::FirmwareVersion(version),
                        None => FeedbackEvent::Status(mit::decode_status_payload(
                            motor_type,
                            comm_type,
                            frame.id,
                            &frame.data,
                        )),
                    }
                }
                CommunicationType::FaultReport => {
                    FeedbackEvent::DetailedFault(DetailedFaultFeedback { raw: frame.data })
                }
                _ => continue,
            }
        };
        report.observations.push(FeedbackObservation {
            order,
            address: address.clone(),
            received_at: timed.received_at,
            can_id: frame.id,
            event,
        });
    }
}

/// Exactly the frame [`MotorBus::enable_drive_at`],
/// [`MotorBus::disable_active_reporting_at`] or
/// [`MotorBus::set_zero_position_at`] transmits, to a configured address. Any
/// other envelope of that type (an On, another host's command) is not this
/// host's echo.
fn ingest_host_echo(
    motor_types: &HashMap<MotorAddress, MotorType>,
    report: &mut FeedbackReport,
    order: usize,
    timed: &TimedCanFrame,
    command: EchoedCommand,
) {
    let received = &timed.received;
    let frame = &received.frame;
    let device_id = (frame.id & 0xff) as u8;
    let (id, data) = match command {
        EchoedCommand::Enable => lifecycle::encode_default_enable(device_id),
        EchoedCommand::ReportingOff => lifecycle::encode_default_active_reporting(device_id, false),
        EchoedCommand::SetZero => lifecycle::encode_default_set_zero_position(device_id),
    };
    let exact = received.kind == RxFrameKind::Data
        && frame.id == id
        && received.payload() == Some(data.as_slice());
    let address = exact
        .then(|| address_for_frame(motor_types, received.interface.as_deref(), device_id))
        .flatten();
    let Some((address, _)) = address else {
        trace_skipped_frame(
            received.interface.as_deref(),
            frame.id,
            "not-own-echo",
            Some(device_id),
            Some((frame.id >> 24) as u8 & 0x1f),
        );
        return;
    };
    report.host_echoes.push(HostEchoObservation {
        order,
        address: address.clone(),
        received_at: timed.received_at,
        can_id: frame.id,
        command,
    });
}

/// Complete eight-byte Data replies from configured addresses only. Requests,
/// foreign-host replies and short/remote/error envelopes are not reply evidence.
fn ingest_reply_frame(
    motor_types: &HashMap<MotorAddress, MotorType>,
    report: &mut FeedbackReport,
    order: usize,
    timed: &TimedCanFrame,
    comm_type: CommunicationType,
) {
    let received = &timed.received;
    let frame = &received.frame;
    let payload = match received.payload() {
        Some(payload) if received.kind == RxFrameKind::Data && payload.len() == 8 => payload,
        _ => {
            trace_skipped_frame(
                received.interface.as_deref(),
                frame.id,
                "incomplete-reply",
                None,
                Some(comm_type.as_u8()),
            );
            return;
        }
    };
    enum Decoded {
        Identity(identity::DeviceUid),
        Read(params::ParameterReadReply),
    }
    let decoded = match comm_type {
        CommunicationType::GetDeviceId => identity::decode_device_id_reply(frame.id, payload)
            .map(|reply| (reply.device_id, Decoded::Identity(reply.uid))),
        _ => params::decode_read_parameter_reply(comm::DEFAULT_HOST_ID, frame.id, payload)
            .map(|reply| (reply.device_id, Decoded::Read(reply))),
    };
    let Some((device_id, decoded)) = decoded else {
        // Outbound requests and replies to another host share these types.
        return;
    };
    let Some((address, _)) =
        address_for_frame(motor_types, received.interface.as_deref(), device_id)
    else {
        trace_skipped_frame(
            received.interface.as_deref(),
            frame.id,
            "unconfigured-motor",
            Some(device_id),
            Some(comm_type.as_u8()),
        );
        return;
    };
    match decoded {
        Decoded::Identity(uid) => report.identities.push(IdentityObservation {
            order,
            address: address.clone(),
            received_at: timed.received_at,
            can_id: frame.id,
            uid,
        }),
        Decoded::Read(reply) => report.parameter_reads.push(ParameterReadObservation {
            order,
            address: address.clone(),
            received_at: timed.received_at,
            can_id: frame.id,
            reply,
        }),
    }
}

/// Configured key and type for a received frame. A scan of the small configured
/// set avoids building an owned probe key per frame; keys are unique, so the
/// interface-qualified match is the same entry a hash lookup would return.
fn address_for_frame<'a>(
    motor_types: &'a HashMap<MotorAddress, MotorType>,
    interface: Option<&str>,
    device_id: u8,
) -> Option<(&'a MotorAddress, MotorType)> {
    let mut matches = motor_types
        .iter()
        .filter(|(address, _)| address.device_id == device_id);
    if let Some(interface) = interface {
        return matches
            .find(|(address, _)| address.interface == interface)
            .map(|(address, kind)| (address, *kind));
    }
    let (address, kind) = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some((address, *kind))
    }
}

/// FIFO storage for synthetic full-eight-byte frames. The compatibility push
/// method keeps fixture injection simple without shifting an unread backlog.
#[derive(Debug, Default)]
pub struct MemoryRxQueue(VecDeque<CanFrame>);

impl MemoryRxQueue {
    pub fn push(&mut self, frame: CanFrame) {
        self.0.push_back(frame);
    }
    pub fn pop_front(&mut self) -> Option<CanFrame> {
        self.0.pop_front()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn drain<R: RangeBounds<usize>>(
        &mut self,
        range: R,
    ) -> std::collections::vec_deque::Drain<'_, CanFrame> {
        self.0.drain(range)
    }
}

impl Extend<CanFrame> for MemoryRxQueue {
    fn extend<T: IntoIterator<Item = CanFrame>>(&mut self, frames: T) {
        self.0.extend(frames);
    }
}

impl From<Vec<CanFrame>> for MemoryRxQueue {
    fn from(frames: Vec<CanFrame>) -> Self {
        Self(frames.into())
    }
}

/// In-memory bus for unit tests; receive pops one frame in bounded work.
#[derive(Debug, Default)]
pub struct MemoryBus {
    pub tx: Vec<CanFrame>,
    pub rx_queue: MemoryRxQueue,
}

impl CanBus for MemoryBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        validate_can_tx(frame)?;
        self.tx.push(frame.clone());
        Ok(())
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        Ok(match self.rx_queue.pop_front() {
            Some(frame) => ReceiveAttempt::Frame(TimedCanFrame {
                received_at: Instant::now(),
                received: ReceivedCanFrame::full_data(None, frame),
            }),
            None => ReceiveAttempt::Idle,
        })
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
    /// One unowned interface socket: for stops and observers, which must
    /// work while a motor owner holds the interface.
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

    /// One unowned transmit-only interface socket for a stop (see
    /// [`SocketCanBus::open_transmit_only`]).
    pub fn socketcan_transmit_only(interface: &str) -> Result<Self, BusError> {
        #[cfg(all(feature = "socketcan", target_os = "linux"))]
        {
            SocketCanBus::open_transmit_only(interface).map(Self::Socket)
        }
        #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
        {
            let _ = interface;
            Err(socketcan_unavailable())
        }
    }

    /// One interface for a motor owner (see [`SocketCanBus::open_owner`]).
    pub fn socketcan_owner(interface: &str) -> Result<Self, BusError> {
        #[cfg(all(feature = "socketcan", target_os = "linux"))]
        {
            SocketCanBus::open_owner(interface).map(Self::Socket)
        }
        #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
        {
            let _ = interface;
            Err(socketcan_unavailable())
        }
    }

    /// Every configured interface, each claimed for this motor owner.
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
    fn validate_address(&self, address: &MotorAddress) -> Result<(), BusError> {
        let _ = address;
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.validate_address(address),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.validate_address(address),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => Err(socketcan_unavailable()),
        }
    }

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

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.recv_one_nonblocking(),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.recv_one_nonblocking(),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => Err(socketcan_unavailable()),
        }
    }

    fn receive_source_count(&self) -> usize {
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.receive_source_count(),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.receive_source_count(),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => 1,
        }
    }

    fn begin_receive(&mut self) {
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.begin_receive(),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.begin_receive(),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => {}
        }
    }

    fn echoes_transmissions(&self) -> bool {
        match self {
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Socket(bus) => bus.echoes_transmissions(),
            #[cfg(all(feature = "socketcan", target_os = "linux"))]
            Self::Router(bus) => bus.echoes_transmissions(),
            #[cfg(not(all(feature = "socketcan", target_os = "linux")))]
            _ => false,
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

#[cfg(all(feature = "socketcan", target_os = "linux"))]
mod socketcan {
    use super::*;
    use std::collections::{BTreeSet, HashMap, HashSet};
    use std::io::{ErrorKind, IoSliceMut, Write};
    use std::os::fd::{AsFd, AsRawFd};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    use std::time::SystemTime;

    use ::socketcan::{
        frame::AsPtr, CanAnyFrame, CanFdSocket, CanFrame as SocketFrame, CanSocket, EmbeddedFrame,
        ExtendedId, Socket, SocketOptions,
    };
    use nix::libc::{CANFD_MTU, CAN_MTU};
    use nix::sys::socket::{recvmsg, setsockopt, sockopt, ControlMessageOwned, MsgFlags, RecvMsg};
    use nix::sys::time::TimeSpec;

    use crate::rx_time::{wall_time_from_timespec, RxClock, RxTimestampCounts};

    /// Own transmissions come back in wire order (`IFF_ECHO` drivers echo on
    /// TX completion), so consumers can order drive traffic against a frame's
    /// actual transmission rather than its earlier, merely queued write.
    fn configure_own_message_echo(socket: &CanFdSocket) -> Result<(), BusError> {
        socket
            .set_loopback(true)
            .and_then(|_| socket.set_recv_own_msgs(true))
            .map_err(|e| BusError::Driver(format!("configure SocketCAN own-message echo: {e}")))
    }

    /// Software RX timestamps (`SO_TIMESTAMPNS`): the kernel stamps each skb,
    /// drive frames and own-TX echoes alike, when the driver hands it to the
    /// network stack, so a delayed read does not make old traffic look fresh.
    fn configure_receive_timestamps(socket: &CanFdSocket) -> Result<(), BusError> {
        setsockopt(socket.as_raw_socket(), sockopt::ReceiveTimestampns, &true)
            .map_err(|e| BusError::Driver(format!("configure SocketCAN receive timestamps: {e}")))
    }

    /// Count the kernel's own receive-queue drops (`SO_RXQ_OVFL`): without it,
    /// a stalled reader loses status frames without a trace.
    fn configure_drop_counter(socket: &CanFdSocket) -> Result<(), BusError> {
        setsockopt(socket.as_raw_socket(), sockopt::RxqOvfl, &1)
            .map_err(|e| BusError::Driver(format!("configure SocketCAN drop counter: {e}")))
    }

    /// The cumulative `SO_RXQ_OVFL` count, attached once any frame was dropped.
    fn kernel_drop_count(message: &RecvMsg<'_, '_, ()>) -> Option<u32> {
        message.cmsgs().ok()?.find_map(|control| match control {
            ControlMessageOwned::RxqOvfl(count) => Some(count),
            _ => None,
        })
    }

    /// The `SCM_TIMESTAMPNS` realtime stamp, if delivered intact.
    fn kernel_rx_time(message: &RecvMsg<'_, '_, ()>) -> Option<SystemTime> {
        message.cmsgs().ok()?.find_map(|control| match control {
            ControlMessageOwned::ScmTimestampns(stamp) => wall_time_from_timespec(
                u64::try_from(stamp.tv_sec()).ok()?,
                u32::try_from(stamp.tv_nsec()).ok()?,
            ),
            _ => None,
        })
    }

    /// Classic `struct can_frame` bytes as the dependency's typed frame, or the
    /// reason the envelope is foreign. The socket never enables CAN FD, so the
    /// kernel delivers exactly `CAN_MTU` per classic frame.
    fn classic_frame(datagram: &[u8; CANFD_MTU]) -> Result<CanAnyFrame, &'static str> {
        let (head, _) = datagram
            .split_first_chunk::<CAN_MTU>()
            .ok_or("truncated classic CAN datagram")?;
        let [i0, i1, i2, i3, len, _pad, _res0, len8_dlc, data @ ..] = *head;
        let mut raw = ::socketcan::frame::can_frame_default();
        raw.can_id = u32::from_ne_bytes([i0, i1, i2, i3]);
        raw.can_dlc = len;
        raw.len8_dlc = len8_dlc;
        raw.data = data;
        match CanAnyFrame::from(raw) {
            CanAnyFrame::Remote(remote) if remote.dlc() > 8 => {
                Err("oversized classic CAN remote frame")
            }
            frame => Ok(frame),
        }
    }

    /// Exclusive motor ownership of one CAN interface on this host: a bound
    /// abstract Unix socket named after the interface. The kernel releases it
    /// when the process exits, however it exits; it needs no file, directory
    /// or permission, and is not inherited across exec. Abstract names are
    /// per network namespace, like the CAN interfaces themselves.
    #[derive(Debug)]
    struct InterfaceClaim {
        _listener: UnixListener,
    }

    fn claim_interface(interface: &str) -> Result<InterfaceClaim, BusError> {
        let name = format!("marengo/can-owner/{interface}");
        let address = SocketAddr::from_abstract_name(name.as_bytes())
            .map_err(|e| BusError::Driver(format!("claim SocketCAN {interface}: {e}")))?;
        match UnixListener::bind_addr(&address) {
            Ok(listener) => Ok(InterfaceClaim {
                _listener: listener,
            }),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {
                tracing::warn!(interface, "SocketCAN interface already has a motor owner");
                Err(BusError::Driver(format!(
                    "SocketCAN {interface} already has a motor owner on this host \
                     (marengo-pi or motor-repl); stop it first (motor-repl disable needs no ownership)"
                )))
            }
            Err(error) => Err(BusError::Driver(format!(
                "claim SocketCAN {interface}: {error}"
            ))),
        }
    }

    #[derive(Debug)]
    pub struct SocketCanBus {
        interface: String,
        socket: CanFdSocket,
        /// Reused `SCM_TIMESTAMPNS` control buffer: no per-read allocation.
        control: Vec<u8>,
        rx_clock: RxClock,
        echoes: EchoLedger,
        /// Held for the socket's lifetime by a motor owner; `None` for stops
        /// and observers.
        _claim: Option<InterfaceClaim>,
        /// False for a transmit-only stop socket: nothing receive-side was
        /// configured and it must never be read.
        receives: bool,
        /// Kernel receive-queue drops already reported.
        drops_reported: u32,
    }

    impl SocketCanBus {
        /// Open as a motor owner: claim the interface first, so a second
        /// owner on this host (a `motor-repl set-zero` under a live
        /// marengo-pi) is refused before it opens a socket or sends a frame.
        pub fn open_owner(interface: &str) -> Result<Self, BusError> {
            let claim = claim_interface(interface)?;
            let mut bus = Self::open(interface)?;
            bus._claim = Some(claim);
            Ok(bus)
        }

        /// Open without ownership, for observers. An owner can never block it.
        pub fn open(interface: &str) -> Result<Self, BusError> {
            Self::open_socket(interface, true)
        }

        /// Open for a stop: bind and set nonblocking, nothing else. No
        /// receive-side option (own-message echo, timestamps, error filter)
        /// is configured, so none of them can fail the stop. Loopback stays
        /// at its kernel default (on), so a running owner still sees the stop.
        /// Unowned: an owner can never block a stop.
        pub fn open_transmit_only(interface: &str) -> Result<Self, BusError> {
            Self::open_socket(interface, false)
        }

        fn open_socket(interface: &str, receives: bool) -> Result<Self, BusError> {
            tracing::debug!(interface, receives, "opening SocketCAN interface");
            // Before bind: no frame can be queued on this socket earlier.
            let opened = Instant::now();
            let classic = CanSocket::open(interface)
                .map_err(|error| BusError::Driver(format!("SocketCAN open failed: {error}")))?;
            // Reads are one recvmsg each (see recv_one_nonblocking). Wrap a
            // duplicate descriptor of the classic socket without enabling FD:
            // neither CanFdSocket::open nor its FD-enabling TryFrom is used.
            let descriptor = classic
                .as_fd()
                .try_clone_to_owned()
                .map_err(|e| BusError::Driver(format!("clone classic CAN descriptor: {e}")))?;
            let socket = CanFdSocket::from(descriptor);
            drop(classic);
            if receives {
                configure_own_message_echo(&socket)?;
                configure_receive_timestamps(&socket)?;
                configure_drop_counter(&socket)?;
                // Error subscriptions default to drop-all in socketcan. Preserve
                // every kernel error class as transport evidence; failures to
                // configure the subscription prevent this socket from being used.
                socket.set_error_filter_accept_all().map_err(|e| {
                    BusError::Driver(format!("configure SocketCAN error filter: {e}"))
                })?;
            }
            socket
                .set_nonblocking(true)
                .map_err(|e| BusError::Driver(e.to_string()))?;
            tracing::info!(interface, "opened SocketCAN interface");
            Ok(Self {
                interface: interface.to_string(),
                socket,
                control: nix::cmsg_space!(TimeSpec, u32),
                rx_clock: RxClock::new(opened),
                echoes: EchoLedger::default(),
                _claim: None,
                receives,
                drops_reported: 0,
            })
        }

        /// How this socket's received frames were stamped since it opened.
        pub fn rx_timestamp_counts(&self) -> RxTimestampCounts {
            self.rx_clock.counts()
        }
    }

    impl CanBus for SocketCanBus {
        fn validate_address(&self, address: &MotorAddress) -> Result<(), BusError> {
            if address.interface != self.interface {
                return Err(BusError::UnknownMotorAddress {
                    interface: address.interface.clone(),
                    device_id: address.device_id,
                });
            }
            Ok(())
        }

        fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
            validate_can_tx(frame)?;
            let sent = frame;
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
            // A CAN datagram must be written atomically. write_all silently
            // retries EINTR/short writes; one nonblocking write instead reports
            // either condition without retrying or claiming physical delivery.
            let bytes = frame.as_bytes();
            let written = self
                .socket
                .as_raw_socket()
                .write(bytes)
                .map_err(|e| BusError::Send {
                    message: e.to_string(),
                })?;
            if written != bytes.len() {
                return Err(BusError::Send {
                    message: format!("short CAN datagram write: {written}/{} bytes", bytes.len()),
                });
            }
            self.echoes.transmitted(sent);
            Ok(())
        }

        fn send_frame_to(
            &mut self,
            address: &MotorAddress,
            frame: &CanFrame,
        ) -> Result<(), BusError> {
            self.validate_address(address)?;
            self.send_frame(frame)
        }

        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            if !self.receives {
                return Err(BusError::Driver(format!(
                    "SocketCAN {} is transmit-only",
                    self.interface
                )));
            }
            let before_read = Instant::now();
            let mut datagram = [0_u8; CANFD_MTU];
            let read = {
                let mut iov = [IoSliceMut::new(&mut datagram)];
                recvmsg::<()>(
                    self.socket.as_raw_fd(),
                    &mut iov,
                    Some(&mut self.control),
                    MsgFlags::empty(),
                )
                .map(|message| {
                    (
                        message.bytes,
                        kernel_rx_time(&message),
                        kernel_drop_count(&message),
                    )
                })
                .map_err(std::io::Error::from)
            };
            match read {
                Ok((bytes, kernel, drops)) => {
                    if let Some(dropped) = new_kernel_drops(&mut self.drops_reported, drops) {
                        tracing::warn!(interface = %self.interface, dropped, "SocketCAN receive queue overflowed");
                        return Err(BusError::Driver(format!(
                            "SocketCAN {}: kernel dropped {dropped} received frame(s) from a full receive queue",
                            self.interface
                        )));
                    }
                    // Wall clock before the monotonic read instant: the gap can
                    // only move the mapped time toward the read.
                    let wall_at_read = SystemTime::now();
                    let read_instant = Instant::now();
                    let frame = match bytes {
                        CAN_MTU => classic_frame(&datagram),
                        CANFD_MTU => Err("unsupported CAN FD frame"),
                        _ => {
                            return Err(BusError::Driver(format!(
                                "SocketCAN {} receive: unexpected {bytes}-byte datagram",
                                self.interface
                            )))
                        }
                    };
                    let frame = match frame {
                        Ok(frame) => frame,
                        Err(reason) => {
                            tracing::warn!(interface = %self.interface, reason, "skipping foreign CAN frame");
                            return Ok(ReceiveAttempt::Ignored);
                        }
                    };
                    let received_at =
                        self.rx_clock
                            .stamp(&self.interface, kernel, wall_at_read, read_instant);
                    let interface = Some(self.interface.clone());
                    let received = ReceivedCanFrame::from_socketcan(interface, frame)?;
                    self.echoes.admit(&self.interface, &received)?;
                    tracing::trace!(
                        interface = %self.interface,
                        can_id = format_args!("{:#010x}", received.frame.id),
                        extended = received.frame.extended,
                        payload_len = received.payload_len,
                        kind = ?received.kind,
                        "SocketCAN rx"
                    );
                    Ok(ReceiveAttempt::Frame(TimedCanFrame {
                        received_at,
                        received,
                    }))
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    self.rx_clock.observe_empty(before_read);
                    Ok(ReceiveAttempt::Idle)
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {
                    Ok(ReceiveAttempt::Interrupted)
                }
                Err(error) => Err(BusError::Driver(format!(
                    "SocketCAN {} receive: {error}",
                    self.interface
                ))),
            }
        }

        /// Opening a receiving socket fails unless own-message echo was
        /// configured. A host command this socket did not send is a receive
        /// failure, never an echo.
        fn echoes_transmissions(&self) -> bool {
            self.receives
        }
    }

    impl MotorBus for SocketCanBus {}

    #[derive(Debug)]
    pub struct SocketCanRouter {
        sockets: HashMap<String, SocketCanBus>,
        addresses: HashSet<MotorAddress>,
        interfaces: Vec<String>,
        cursor: usize,
        next_start: usize,
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
            let interfaces: Vec<_> = interfaces.into_iter().collect();
            let mut sockets = HashMap::new();
            for interface in &interfaces {
                sockets.insert(interface.clone(), SocketCanBus::open_owner(interface)?);
            }

            Ok(Self {
                sockets,
                addresses,
                interfaces,
                cursor: 0,
                next_start: 0,
            })
        }
    }

    impl CanBus for SocketCanRouter {
        fn validate_address(&self, address: &MotorAddress) -> Result<(), BusError> {
            if !self.addresses.contains(address) || !self.sockets.contains_key(&address.interface) {
                return Err(BusError::UnknownMotorAddress {
                    interface: address.interface.clone(),
                    device_id: address.device_id,
                });
            }
            Ok(())
        }

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

        fn receive_source_count(&self) -> usize {
            self.interfaces.len()
        }

        fn begin_receive(&mut self) {
            if !self.interfaces.is_empty() {
                self.cursor = self.next_start;
                self.next_start = (self.next_start + 1) % self.interfaces.len();
            }
        }

        fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
            let Some(interface) = self.interfaces.get(self.cursor) else {
                return Err(BusError::Driver(
                    "routed SocketCAN has no receive source".into(),
                ));
            };
            self.cursor = (self.cursor + 1) % self.interfaces.len();
            let socket = self.sockets.get_mut(interface).ok_or_else(|| {
                BusError::Driver(format!("missing routed SocketCAN interface {interface}"))
            })?;
            socket.recv_one_nonblocking()
        }

        /// Every routed socket is a [`SocketCanBus`], which always echoes.
        fn echoes_transmissions(&self) -> bool {
            self.sockets
                .values()
                .all(SocketCanBus::echoes_transmissions)
        }
    }

    impl MotorBus for SocketCanRouter {}

    #[cfg(test)]
    mod ownership_tests {
        #![allow(clippy::expect_used)]

        use super::*;

        fn message(error: BusError) -> String {
            match error {
                BusError::Driver(message) => message,
                other => format!("{other:?}"),
            }
        }

        /// P2A-03: a second motor owner of an interface is refused before
        /// it opens a socket (the interface here does not even exist).
        #[test]
        fn a_second_owner_is_refused_before_opening_the_interface() {
            let interface = "mrgtest-own0";
            let held = claim_interface(interface).expect("first owner");
            let refused = message(SocketCanBus::open_owner(interface).expect_err("second owner"));
            assert!(refused.contains("already has a motor owner"), "{refused}");
            drop(held);
            let reopened =
                message(SocketCanBus::open_owner(interface).expect_err("no such interface"));
            assert!(reopened.contains("SocketCAN open failed"), "{reopened}");
        }

        /// A stop never needs ownership, so a live owner cannot block it.
        #[test]
        fn an_unowned_open_ignores_a_held_claim() {
            let interface = "mrgtest-own1";
            let _held = claim_interface(interface).expect("owner");
            let error = message(SocketCanBus::open(interface).expect_err("no such interface"));
            assert!(error.contains("SocketCAN open failed"), "{error}");
        }

        #[test]
        fn claims_are_per_interface() {
            let _first = claim_interface("mrgtest-own2").expect("first interface");
            let _second = claim_interface("mrgtest-own3").expect("second interface");
        }
    }
}

#[cfg(all(feature = "socketcan", target_os = "linux"))]
pub use socketcan::{SocketCanBus, SocketCanRouter};

#[cfg(test)]
mod kernel_drop_tests {
    use super::new_kernel_drops;

    /// DRV-06: a receive-queue overflow is reported once per new drop.
    #[test]
    fn each_new_kernel_drop_is_reported_once() {
        let mut reported = 0;
        assert_eq!(new_kernel_drops(&mut reported, None), None);
        assert_eq!(new_kernel_drops(&mut reported, Some(3)), Some(3));
        assert_eq!(new_kernel_drops(&mut reported, Some(3)), None);
        assert_eq!(new_kernel_drops(&mut reported, Some(5)), Some(2));
        let mut reported = u32::MAX;
        assert_eq!(
            new_kernel_drops(&mut reported, Some(1)),
            Some(2),
            "counter wraps"
        );
    }
}

#[cfg(test)]
mod echo_ledger_tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn received(id: u32, data: [u8; 8]) -> ReceivedCanFrame {
        ReceivedCanFrame::full_data(
            Some("can0".into()),
            CanFrame {
                id,
                data,
                extended: true,
            },
        )
    }

    fn sent(id: u32, data: [u8; 8]) -> CanFrame {
        CanFrame {
            id,
            data,
            extended: true,
        }
    }

    fn host_commands() -> Vec<(u32, [u8; 8])> {
        let mit = mit::encode_mit(&MitCommand {
            device_id: 1,
            motor_type: MotorType::Rs03,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            kp: 0.0,
            kd: 0.0,
            torque_ff_nm: 0.0,
        })
        .expect("neutral MIT");
        vec![
            mit,
            lifecycle::encode_default_enable(1),
            lifecycle::encode_default_disable(1),
            lifecycle::encode_default_set_zero_position(1),
            lifecycle::encode_default_active_reporting(1, false),
            identity::encode_default_get_device_id(1),
            version::encode_default_get_firmware_version(1),
            params::encode_read_parameter(comm::DEFAULT_HOST_ID, 1, ParameterId::MechPos),
            params::encode_set_run_mode(1, RunMode::Mit),
        ]
    }

    /// DRV-05 / P2A-03: an exact Enable, SetZero or reporting Off another
    /// socket wrote is not this socket's echo; it fails the receive.
    #[test]
    fn host_commands_this_socket_did_not_send_are_refused() {
        for (id, data) in host_commands() {
            let mut ledger = EchoLedger::default();
            let error = ledger
                .admit("can0", &received(id, data))
                .expect_err("foreign host command");
            assert!(
                matches!(&error, BusError::Driver(message) if message.contains("another CAN writer")),
                "{id:#010x}: {error}"
            );
        }
    }

    #[test]
    fn own_echo_is_admitted_once_and_a_second_copy_is_foreign() {
        for (id, data) in host_commands() {
            let mut ledger = EchoLedger::default();
            ledger.transmitted(&sent(id, data));
            ledger.admit("can0", &received(id, data)).expect("own echo");
            assert!(
                ledger.admit("can0", &received(id, data)).is_err(),
                "{id:#010x}: one write echoes once"
            );
        }
    }

    #[test]
    fn echoes_match_out_of_order_and_by_payload() {
        let mut ledger = EchoLedger::default();
        let (enable, enable_data) = lifecycle::encode_default_enable(1);
        let (off, off_data) = lifecycle::encode_default_active_reporting(2, false);
        ledger.transmitted(&sent(enable, enable_data));
        ledger.transmitted(&sent(off, off_data));
        let (on, on_data) = lifecycle::encode_default_active_reporting(2, true);
        assert_eq!(on, off, "same identifier, different F_CMD byte");
        assert!(ledger.admit("can0", &received(on, on_data)).is_err());
        ledger
            .admit("can0", &received(off, off_data))
            .expect("second write echoed first");
        ledger
            .admit("can0", &received(enable, enable_data))
            .expect("first write echoed second");
    }

    #[test]
    fn drive_frames_and_kernel_errors_are_never_host_commands() {
        let mut ledger = EchoLedger::default();
        for id in [
            0x0280_01fd_u32, // type 2 status, Run
            0x0200_01fd,     // type 2 status, Reset, no flags
            0x1880_01fd,     // type 24 report
            0x1800_01fd,     // type 24 report, Reset, no flags
            0x1500_01fd,     // type 21 detailed fault
            0x1100_01fd,     // type 17 reply
            0x0000_01fe,     // type 0 reply
        ] {
            ledger
                .admit("can0", &received(id, [0; 8]))
                .expect("drive frame");
        }
        let mut error = received(0x2000_0004, [0; 8]);
        error.kind = RxFrameKind::Error;
        ledger.admit("can0", &error).expect("kernel error frame");
        let standard = ReceivedCanFrame::new_data(Some("can0".into()), 0x123, false, &[0; 8])
            .expect("standard frame");
        ledger
            .admit("can0", &standard)
            .expect("not a Robstride frame");
    }

    #[test]
    fn short_or_remote_host_commands_are_still_foreign() {
        let mut ledger = EchoLedger::default();
        let (id, data) = lifecycle::encode_default_enable(1);
        ledger.transmitted(&sent(id, data));
        let short = ReceivedCanFrame::new_data(Some("can0".into()), id, true, &data[..4])
            .expect("short frame");
        assert!(ledger.admit("can0", &short).is_err());
        let remote =
            ReceivedCanFrame::new_remote(Some("can0".into()), id, true, 8).expect("remote frame");
        assert!(ledger.admit("can0", &remote).is_err());
    }

    #[test]
    fn pending_echoes_are_bounded_and_the_oldest_is_displaced() {
        let mut ledger = EchoLedger::default();
        let (id, data) = lifecycle::encode_default_enable(1);
        let first = [9; 8];
        ledger.transmitted(&sent(id, first));
        for _ in 0..MAX_PENDING_ECHOES {
            ledger.transmitted(&sent(id, data));
        }
        assert_eq!(ledger.pending.len(), MAX_PENDING_ECHOES);
        assert!(ledger.admit("can0", &received(id, first)).is_err());
        ledger
            .admit("can0", &received(id, data))
            .expect("held transmission");
    }
}
