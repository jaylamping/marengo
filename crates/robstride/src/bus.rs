//! CAN bus abstraction for Robstride MIT frames.

use std::collections::{HashMap, VecDeque};
use std::ops::RangeBounds;
use std::time::{Duration, Instant};

use marengo_config::{MotorEntry, MotorType, MotorsConfigFile};
use thiserror::Error;

use crate::comm::{self, CommunicationType};
use crate::command::CommandError;
use crate::feedback::{
    DetailedFaultFeedback, DriveMode, EchoedCommand, FeedbackEvent, FeedbackObservation,
    FeedbackReport, HostEchoObservation, IdentityObservation, MalformedFeedback, MalformedReason,
    ParameterReadObservation, TransportObservation,
};
use crate::identity;
use crate::lifecycle;
use crate::mit::{self, MitCommand};
use crate::params::{self, ParameterId, ParameterValue, RunMode};
use crate::receive::{RawReceiveReport, ReceiveAttempt, ReceiveCompletion, ReceiveLimits};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FaultReport {
    device_id: u8,
    feedback: DetailedFaultFeedback,
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

/// Frame stamped when read by a concrete backend. Compatibility backends that
/// provide only `ReceivedCanFrame` stamp each frame at handoff. Clock resolution
/// may give consecutive observations equal times; list order still distinguishes
/// them. These timestamps do not prove physical acquisition times or epochs.
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

    /// Legacy full-data projection. A short/remote/error envelope cannot be
    /// converted to this type without losing evidence, so conversion fails.
    fn recv_frames(&mut self, out: &mut Vec<CanFrame>) -> Result<(), BusError> {
        let report = self.recv_raw_report(Duration::ZERO, Duration::ZERO, ReceiveLimits::default());
        let mut conversion_error = None;
        for timed in report.frames {
            let received = timed.received;
            if received.kind == RxFrameKind::Data && received.payload_len == 8 {
                out.push(received.frame);
            } else if conversion_error.is_none() {
                conversion_error = Some(BusError::Driver(format!(
                    "legacy raw projection cannot represent CAN {:#x} kind {:?} length {}",
                    received.frame.id, received.kind, received.payload_len,
                )));
            }
        }
        if let Some(error) = report.terminal_error.or(conversion_error) {
            return Err(error);
        }
        if !report.completion.is_complete() {
            return Err(BusError::ReceiveIncomplete {
                completion: report.completion,
            });
        }
        Ok(())
    }

    /// Drain received frames with source interface when the backend can provide it.
    fn recv_frames_from(&mut self, out: &mut Vec<ReceivedCanFrame>) -> Result<(), BusError> {
        let mut frames = Vec::new();
        let result = self.recv_timed_frames_from_nonblocking(&mut frames);
        out.extend(frames.into_iter().map(|timed| timed.received));
        result
    }

    /// Drain whatever is already queued without waiting for the next frame.
    fn recv_frames_from_nonblocking(
        &mut self,
        out: &mut Vec<ReceivedCanFrame>,
    ) -> Result<(), BusError> {
        self.recv_frames_from(out)
    }

    fn recv_timed_frames_from(&mut self, out: &mut Vec<TimedCanFrame>) -> Result<(), BusError> {
        self.recv_raw_report(Duration::ZERO, Duration::ZERO, ReceiveLimits::default())
            .into_result(out)
    }

    fn recv_timed_frames_from_nonblocking(
        &mut self,
        out: &mut Vec<TimedCanFrame>,
    ) -> Result<(), BusError> {
        self.recv_timed_frames_from(out)
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
    /// Validate the complete batch before emitting the first motion frame.
    /// Transport failures can still occur after some valid frames were sent.
    fn mit_control_all(&mut self, cmds: &[MitCommand]) -> Result<(), BusError> {
        for (index, cmd) in cmds.iter().enumerate() {
            cmd.validate()?;
            if cmds[..index]
                .iter()
                .any(|prior| prior.device_id == cmd.device_id)
            {
                return Err(BusError::DuplicateCommand {
                    device_id: cmd.device_id,
                });
            }
        }
        for cmd in cmds {
            let (id, data) = mit::encode_mit(cmd)?;
            send_encoded_frame(self, id, data)?;
        }
        Ok(())
    }

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

    /// Communication type 0: request the 64-bit MCU unique identifier (manual §4.1.1).
    fn get_device_id_at(&mut self, address: &MotorAddress) -> Result<(), BusError> {
        let (id, data) = identity::encode_default_get_device_id(address.device_id);
        send_encoded_frame_to(self, address, id, data)
    }

    fn write_parameter(
        &mut self,
        device_id: u8,
        parameter: ParameterId,
        value: ParameterValue,
    ) -> Result<(), BusError> {
        let (id, data) =
            params::encode_write_parameter(comm::DEFAULT_HOST_ID, device_id, parameter, value)?;
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
        )?;
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
        let (id, data) = params::encode_speed_ref(device_id, velocity_rad_s)?;
        send_encoded_frame(self, id, data)
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

    fn recv_all(
        &mut self,
        motor_types: &HashMap<u8, MotorType>,
        states: &mut HashMap<u8, MotorState>,
        budget: Duration,
        quiet: Duration,
    ) -> Result<usize, BusError> {
        let types = motor_types
            .iter()
            .map(|(id, kind)| (MotorAddress::new("", *id), *kind))
            .collect();
        let mut raw = self.recv_raw_report(budget, quiet, ReceiveLimits::default());
        // This compatibility map intentionally has no interface domain. Retain
        // the checked envelope; only the address projection loses that domain.
        for frame in &mut raw.frames {
            frame.received.interface = None;
        }
        let report = feedback_from_raw(&types, raw, budget);
        for observation in &report.observations {
            observation.update_state(states.entry(observation.address.device_id).or_default());
        }
        finish_feedback_projection(report)
    }

    fn recv_all_addressed(
        &mut self,
        motor_types: &HashMap<MotorAddress, MotorType>,
        states: &mut HashMap<MotorAddress, MotorState>,
        budget: Duration,
        quiet: Duration,
    ) -> Result<usize, BusError> {
        let report = self.recv_feedback_report(motor_types, budget, quiet);
        for observation in &report.observations {
            observation.update_state(states.entry(observation.address.clone()).or_default());
        }
        finish_feedback_projection(report)
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

fn finish_feedback_projection(report: FeedbackReport) -> Result<usize, BusError> {
    if let Some(error) = report.terminal_error {
        return Err(error);
    }
    if let Some(transport) = report.transport_frames.first() {
        return Err(BusError::Driver(format!(
            "CAN error frame on {:?}: id={:#x} length={} bytes={:02x?}",
            transport.frame.received.interface,
            transport.frame.received.frame.id,
            transport.frame.received.payload_len,
            transport.frame.received.frame.data,
        )));
    }
    for observation in &report.observations {
        if let FeedbackEvent::Malformed(feedback) = observation.event {
            return Err(BusError::MalformedFeedback {
                address: observation.address.clone(),
                reason: feedback.reason,
            });
        }
    }
    if !report.completion.is_complete() {
        return Err(BusError::ReceiveIncomplete {
            completion: report.completion,
        });
    }
    Ok(report.observations.len())
}

fn feedback_from_raw(
    motor_types: &HashMap<MotorAddress, MotorType>,
    raw: RawReceiveReport,
    budget: Duration,
) -> FeedbackReport {
    let mut report = FeedbackReport {
        completion: raw.completion,
        raw_frames: raw.frames.len(),
        observations: Vec::with_capacity(raw.frames.len()),
        read_attempts: raw.read_attempts,
        terminal_error: raw.terminal_error,
        terminal_error_order: raw.terminal_error_order,
        ..FeedbackReport::default()
    };
    ingest_feedback_frames(motor_types, &mut report, &raw.frames);
    if !budget.is_zero()
        && report.completion.is_complete()
        && report.observations.is_empty()
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
            continue;
        };
        if matches!(
            comm_type,
            CommunicationType::GetDeviceId | CommunicationType::ReadParameter
        ) {
            ingest_reply_frame(motor_types, report, order, timed, comm_type);
            continue;
        }
        if comm_type == CommunicationType::Enable {
            ingest_host_echo(motor_types, report, order, timed, EchoedCommand::Enable);
            continue;
        }
        let device_id = comm::inbound_motor_device_id(frame.id, comm_type);
        if device_id == comm::DEFAULT_HOST_ID {
            // A type-24 command from a host (this one's echo or another local
            // socket's loopback) carries the host id where reports carry the drive.
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
            FeedbackEvent::Malformed(MalformedFeedback {
                raw: frame.data,
                payload_len: received.payload_len,
                kind: received.kind,
                reason,
                status_flags: status_header.then_some(((frame.id >> 16) & 0x3f) as u8),
                drive_mode: status_header.then_some(DriveMode::from_can_id(frame.id)),
            })
        } else {
            match comm_type {
                CommunicationType::OperationStatus | CommunicationType::ActiveReporting => {
                    // Eight-byte Data payload and status comm type were checked above.
                    FeedbackEvent::Status(mit::decode_status_payload(
                        motor_type,
                        comm_type,
                        frame.id,
                        &frame.data,
                    ))
                }
                CommunicationType::FaultReport => {
                    let Some(fault) = decode_fault_report(
                        frame.id,
                        &frame.data[..usize::from(received.payload_len)],
                    ) else {
                        continue;
                    };
                    FeedbackEvent::DetailedFault(fault.feedback)
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

/// Exactly the frame [`MotorBus::enable_drive_at`] or
/// [`MotorBus::disable_active_reporting_at`] transmits, to a configured
/// address. Any other envelope of that type (an On, another host's command)
/// is not this host's echo.
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

fn decode_fault_report(can_id: u32, data: &[u8]) -> Option<FaultReport> {
    let unpacked = comm::unpack_ext_id(can_id)?;
    if CommunicationType::from_u8(unpacked.comm_type)? != CommunicationType::FaultReport {
        return None;
    }
    let raw: [u8; 8] = data.try_into().ok()?;
    Some(FaultReport {
        device_id: comm::inbound_motor_device_id(can_id, CommunicationType::FaultReport),
        feedback: DetailedFaultFeedback { raw },
    })
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

#[cfg(all(feature = "socketcan", target_os = "linux"))]
mod socketcan {
    use super::*;
    use std::collections::{BTreeSet, HashMap, HashSet};
    use std::io::{ErrorKind, Write};
    use std::os::fd::AsFd;

    use ::socketcan::{
        frame::AsPtr, CanFdSocket, CanFrame as SocketFrame, CanSocket, EmbeddedFrame, ExtendedId,
        Socket, SocketOptions,
    };

    /// Own transmissions come back in wire order (`IFF_ECHO` drivers echo on
    /// TX completion), so consumers can order drive traffic against a frame's
    /// actual transmission rather than its earlier, merely queued write.
    fn configure_own_message_echo(socket: &CanFdSocket) -> Result<(), BusError> {
        socket
            .set_loopback(true)
            .and_then(|_| socket.set_recv_own_msgs(true))
            .map_err(|e| BusError::Driver(format!("configure SocketCAN own-message echo: {e}")))
    }

    #[derive(Debug)]
    pub struct SocketCanBus {
        interface: String,
        socket: CanFdSocket,
    }

    impl SocketCanBus {
        pub fn open(interface: &str) -> Result<Self, BusError> {
            tracing::debug!(interface, "opening SocketCAN interface");
            let classic = CanSocket::open(interface).map_err(|e| BusError::Send {
                message: e.to_string(),
            })?;
            // Classic CanSocket::read_frame uses Read::read_exact, which retries
            // EINTR internally. CanFdSocket::read_frame uses a single read. Wrap
            // a duplicate descriptor of the classic socket without enabling FD:
            // neither CanFdSocket::open nor its FD-enabling TryFrom is used.
            let descriptor = classic
                .as_fd()
                .try_clone_to_owned()
                .map_err(|e| BusError::Driver(format!("clone classic CAN descriptor: {e}")))?;
            let socket = CanFdSocket::from(descriptor);
            drop(classic);
            configure_own_message_echo(&socket)?;
            // Error subscriptions default to drop-all in socketcan. Preserve
            // every kernel error class as transport evidence; failures to
            // configure the subscription prevent this socket from being used.
            socket
                .set_error_filter_accept_all()
                .map_err(|e| BusError::Driver(format!("configure SocketCAN error filter: {e}")))?;
            socket
                .set_nonblocking(true)
                .map_err(|e| BusError::Driver(e.to_string()))?;
            tracing::info!(interface, "opened SocketCAN interface");
            Ok(Self {
                interface: interface.to_string(),
                socket,
            })
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
            match self.socket.read_frame() {
                Ok(frame) => {
                    let received_at = Instant::now();
                    let interface = Some(self.interface.clone());
                    let received = ReceivedCanFrame::from_socketcan(interface, frame)?;
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

        /// Opening fails unless own-message echo was configured.
        fn echoes_transmissions(&self) -> bool {
            true
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
                sockets.insert(interface.clone(), SocketCanBus::open(interface)?);
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
}

#[cfg(all(feature = "socketcan", target_os = "linux"))]
pub use socketcan::{SocketCanBus, SocketCanRouter};
