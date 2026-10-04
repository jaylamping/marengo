//! Lossless receive evidence. Fault domains remain separate from replaceable pose state.

use std::time::Instant;

use crate::{
    BusError, DeviceUid, FirmwareVersionFeedback, MitFeedback, MotorAddress, ParameterReadReply,
    ReceiveCompletion, RxFrameKind, TimedCanFrame,
};

/// Drive state encoded in status CAN-ID bits 22..23.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveMode {
    Reset,
    Calibration,
    Run,
    Reserved,
}

impl DriveMode {
    /// Drive state from status CAN-ID bits 22..23. Type-2 replies and
    /// type-24 reports share the status header layout.
    pub fn from_can_id(can_id: u32) -> Self {
        match (can_id >> 22) & 3 {
            0 => Self::Reset,
            1 => Self::Calibration,
            2 => Self::Run,
            _ => Self::Reserved,
        }
    }

    /// Stable short name for structured log fields.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reset => "Reset",
            Self::Calibration => "Calibration",
            Self::Run => "Run",
            Self::Reserved => "Reserved",
        }
    }
}

/// Status header shared by type-2 replies and type-24 reports: drive mode
/// from CAN-ID bits 22..23 and the six status flags from bits 16..21.
pub fn decode_status_header(can_id: u32) -> (DriveMode, u8) {
    (
        DriveMode::from_can_id(can_id),
        ((can_id >> 16) & 0x3f) as u8,
    )
}

/// A drive last seen running that now reports Reset likely lost power and
/// rebooted: firmware boots into Reset and stays silent while down.
pub fn is_reboot_transition(previous: Option<DriveMode>, current: DriveMode) -> bool {
    matches!(previous, Some(DriveMode::Run)) && current == DriveMode::Reset
}

/// Complete type-21 payload, without assuming an unqualified firmware byte order.
/// Bytes 0..4 are detailed faults; bytes 4..8 are warnings. These domains have
/// different meanings from the six flags in a status CAN identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailedFaultFeedback {
    pub raw: [u8; 8],
}

impl DetailedFaultFeedback {
    pub fn fault_bytes(self) -> [u8; 4] {
        [self.raw[0], self.raw[1], self.raw[2], self.raw[3]]
    }

    pub fn warning_bytes(self) -> [u8; 4] {
        [self.raw[4], self.raw[5], self.raw[6], self.raw[7]]
    }

    /// Nonzero evidence remains visible even when its bit identity is unknown.
    pub fn has_fault(self) -> bool {
        self.fault_bytes().iter().any(|byte| *byte != 0)
    }

    pub fn has_warning(self) -> bool {
        self.warning_bytes().iter().any(|byte| *byte != 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedbackEvent {
    Status(MitFeedback),
    /// Type-2 reply to a firmware version query: header hazards, never a pose.
    FirmwareVersion(FirmwareVersionFeedback),
    DetailedFault(DetailedFaultFeedback),
    Malformed(MalformedFeedback),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MalformedReason {
    #[error("expected eight data bytes, received {received}")]
    PayloadLength { received: u8 },
    #[error("feedback is not a data frame")]
    NonDataFrame,
    #[error("invalid received frame envelope")]
    InvalidEnvelope,
}

/// Incomplete raw evidence is not a complete pose or detailed-fault word.
/// Header flags/mode are supplied only for Data status frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedFeedback {
    pub raw: [u8; 8],
    pub payload_len: u8,
    pub kind: RxFrameKind,
    pub reason: MalformedReason,
    pub status_flags: Option<u8>,
    pub drive_mode: Option<DriveMode>,
}

/// One configured actuator's observation in transport delivery order.
/// Cross-interface order is host delivery order, not a synchronized physical clock.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackObservation {
    /// Per-poll raw-frame order, independent of timestamp resolution. Ignored
    /// traffic may create gaps in the configured observation sequence.
    pub order: usize,
    pub address: MotorAddress,
    /// Host receive time, independent of the most recent pose timestamp. Distinct
    /// ordered observations may have equal times at the host clock's resolution.
    pub received_at: Instant,
    /// Complete original identifier, including any otherwise uninterpreted bits.
    pub can_id: u32,
    pub event: FeedbackEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportObservation {
    pub order: usize,
    pub frame: TimedCanFrame,
}

/// Type-0 reply from a configured address. Correlating it with a request (pop
/// order after the request) and with a reference binding is the consumer's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityObservation {
    pub order: usize,
    pub address: MotorAddress,
    pub received_at: Instant,
    pub can_id: u32,
    pub uid: DeviceUid,
}

/// Type-17 reply addressed to this host from a configured address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterReadObservation {
    pub order: usize,
    pub address: MotorAddress,
    pub received_at: Instant,
    pub can_id: u32,
    pub reply: ParameterReadReply,
}

/// Which host command an echo reads back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoedCommand {
    /// Type 3 Enable.
    Enable,
    /// Type 24 with `F_CMD` 0: stop the drive's active reporting.
    ReportingOff,
    /// Type 6 SetZero (`data[0] == 1`).
    SetZero,
}

/// This host's Enable, reporting Off or SetZero ([`DEFAULT_HOST_ID`](crate::DEFAULT_HOST_ID))
/// to a configured address, read back from the receive stream. On a bus that
/// [echoes transmissions](crate::CanBus::echoes_transmissions) its `order` is
/// where the command actually went on the wire, after any drive traffic that
/// preceded it there. It is never drive feedback, liveness or a device reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEchoObservation {
    pub order: usize,
    pub address: MotorAddress,
    pub received_at: Instant,
    pub can_id: u32,
    pub command: EchoedCommand,
}

/// A delivered prefix remains available even if the drain ends in a transport
/// error. Incomplete work is explicit and independent of errors. An empty
/// nonblocking drain has no terminal error; a completed positive-budget drain
/// with no recognized observations may report benign `RecvTimeout`. Merge vendor
/// and transport events by `order`; a terminal read error precedes a later frame
/// at the same `terminal_error_order`. Host order does not prove physical cause.
#[derive(Debug, Default)]
pub struct FeedbackReport {
    pub observations: Vec<FeedbackObservation>,
    /// Kernel Error frames are transport evidence, never vendor-addressed status.
    pub transport_frames: Vec<TransportObservation>,
    pub completion: ReceiveCompletion,
    /// Unsupported foreign frames skipped by the SocketCAN backend, still charged to the poll quota.
    pub ignored_frames: usize,
    pub raw_frames: usize,
    pub read_attempts: usize,
    pub terminal_error: Option<BusError>,
    pub terminal_error_order: Option<usize>,
    /// Type-0 device identity replies, in raw delivery order. Never pose or fault evidence.
    pub identities: Vec<IdentityObservation>,
    /// Type-17 parameter replies to this host, in raw delivery order.
    pub parameter_reads: Vec<ParameterReadObservation>,
    /// Echoed host Enable and reporting Off frames, in raw delivery order.
    /// Ordering evidence only.
    pub host_echoes: Vec<HostEchoObservation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_header_decode_matches_incident_ids() {
        // 2026-10-04 gravity-calibration capture: id5 running, then Reset
        // after its 738 ms silence.
        assert_eq!(decode_status_header(0x0280_05FD), (DriveMode::Run, 0x00));
        assert_eq!(decode_status_header(0x0200_05FD), (DriveMode::Reset, 0x00));
    }

    #[test]
    fn reboot_transition_only_flags_run_to_reset() {
        assert!(is_reboot_transition(Some(DriveMode::Run), DriveMode::Reset));
        for previous in [
            None,
            Some(DriveMode::Reset),
            Some(DriveMode::Calibration),
            Some(DriveMode::Reserved),
        ] {
            assert!(
                !is_reboot_transition(previous, DriveMode::Reset),
                "no reboot from {previous:?}"
            );
        }
        for current in [DriveMode::Run, DriveMode::Calibration, DriveMode::Reserved] {
            assert!(
                !is_reboot_transition(Some(DriveMode::Run), current),
                "no reboot into {current:?}"
            );
        }
    }
}
