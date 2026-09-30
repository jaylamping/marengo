//! Lossless receive evidence. Fault domains remain separate from replaceable pose state.

use std::time::Instant;

use crate::{BusError, MitFeedback, MotorAddress, MotorState};

/// Drive state encoded in status CAN-ID bits 22..23.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveMode {
    Reset,
    Calibration,
    Run,
    Reserved,
}

impl DriveMode {
    pub(crate) fn from_can_id(can_id: u32) -> Self {
        match (can_id >> 22) & 3 {
            0 => Self::Reset,
            1 => Self::Calibration,
            2 => Self::Run,
            _ => Self::Reserved,
        }
    }
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
    DetailedFault(DetailedFaultFeedback),
}

/// One configured actuator's observation in transport delivery order.
/// Cross-interface order is acquisition order, not a synchronized physical clock.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackObservation {
    pub address: MotorAddress,
    /// Host receive time, independent of the most recent pose timestamp. Distinct
    /// ordered observations may have equal times at the host clock's resolution.
    pub received_at: Instant,
    /// Complete original identifier, including any otherwise uninterpreted bits.
    pub can_id: u32,
    pub event: FeedbackEvent,
}

impl FeedbackObservation {
    /// Latest-state compatibility only. Safety consumers must inspect every event
    /// in the report; a healthy status can replace this diagnostic projection.
    pub fn update_state(&self, state: &mut MotorState) {
        match self.event {
            FeedbackEvent::Status(feedback) => {
                *state = MotorState {
                    position_rad: feedback.position_rad,
                    velocity_rad_s: feedback.velocity_rad_s,
                    torque_nm: feedback.torque_nm,
                    temperature_c: feedback.temperature_c,
                    fault: feedback.fault,
                    updated: Some(self.received_at),
                };
            }
            FeedbackEvent::DetailedFault(feedback) => {
                // This is an indication, not a decoded detailed-fault identity.
                state.fault = u16::from(feedback.has_fault());
            }
        }
    }
}

/// A delivered prefix remains available even if the drain ends in a transport
/// error. An empty nonblocking drain has no terminal error; a blocking drain with
/// no recognized observations may report `RecvTimeout`.
#[derive(Debug, Default)]
pub struct FeedbackReport {
    pub observations: Vec<FeedbackObservation>,
    pub terminal_error: Option<BusError>,
}
