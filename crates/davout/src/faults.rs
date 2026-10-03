//! Persistent safety authority, independent of replaceable pose caches.

use std::time::Instant;

use robstride::{MalformedReason, MotorAddress, ReceiveCompletion, RxFrameKind};

/// Runtime hazards, separate from rejected operator commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultClass {
    Device,
    Feedback,
    DriveState,
    Communication,
    Transport,
    Controller,
    HardwareEstop,
    StopDelivery,
    WrongSign,
    DangerZone,
}

/// Original vendor domains. Detailed word byte order remains unqualified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceFaultEvidence {
    pub status_flags: u8,
    pub detailed_fault_bytes: [u8; 4],
    pub warning_bytes: [u8; 4],
    /// Received prefixes are retained separately; missing bytes are not a complete word.
    pub partial_fault_bytes: [u8; 4],
    pub partial_fault_available: u8,
    pub partial_warning_bytes: [u8; 4],
    pub partial_warning_available: u8,
    /// Vendor status mode: Reset=0, Calibration=1, Run=2, Reserved=3.
    pub drive_mode: Option<u8>,
    pub received_at: Option<Instant>,
}

impl DeviceFaultEvidence {
    pub(crate) fn merge(&mut self, other: &Self) {
        self.status_flags |= other.status_flags;
        for (stored, observed) in self
            .detailed_fault_bytes
            .iter_mut()
            .zip(other.detailed_fault_bytes)
        {
            *stored |= observed;
        }
        for (stored, observed) in self.warning_bytes.iter_mut().zip(other.warning_bytes) {
            *stored |= observed;
        }
        for (stored, observed) in self
            .partial_fault_bytes
            .iter_mut()
            .zip(other.partial_fault_bytes)
        {
            *stored |= observed;
        }
        for (stored, observed) in self
            .partial_warning_bytes
            .iter_mut()
            .zip(other.partial_warning_bytes)
        {
            *stored |= observed;
        }
        self.partial_fault_available |= other.partial_fault_available;
        self.partial_warning_available |= other.partial_warning_available;
        if other.drive_mode.is_some() {
            self.drive_mode = other.drive_mode;
        }
        if other.received_at > self.received_at {
            self.received_at = other.received_at;
        }
    }
}

/// Original receive envelope. Only raw[..payload_len.min(8)] contains received data;
/// Remote requests carry no payload and their requested length remains in `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveFrameEvidence {
    pub interface: Option<String>,
    pub can_id: u32,
    pub extended: bool,
    pub received_at: Instant,
    pub raw: [u8; 8],
    pub payload_len: u8,
    pub kind: RxFrameKind,
    pub reason: Option<MalformedReason>,
}

impl std::fmt::Display for ReceiveFrameEvidence {
    /// Bounded original-envelope diagnostics, without inventing acquisition time.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let interface = self.interface.as_deref().map(bounded_message);
        write!(
            f,
            "interface={interface:?}, kind={:?}, can_id=0x{:08x}, extended={}, payload_len={}, data=",
            self.kind, self.can_id, self.extended, self.payload_len
        )?;
        if !matches!(self.kind, RxFrameKind::Remote { .. }) {
            for byte in &self.raw[..usize::from(self.payload_len.min(8))] {
                write!(f, "{byte:02x}")?;
            }
        }
        if let Some(reason) = self.reason {
            write!(f, ", reason={reason}")?;
        }
        Ok(())
    }
}

/// An incomplete view retains its declared work and honest completion state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveDrainEvidence {
    pub completion: ReceiveCompletion,
    pub raw_frames: usize,
    pub read_attempts: usize,
}

/// Bounded diagnostics: first and latest envelopes/views plus accumulated event counts.
/// This is not an unbounded packet log or a physical acquisition receipt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiveFaultEvidence {
    pub first_frame: Option<ReceiveFrameEvidence>,
    pub latest_frame: Option<ReceiveFrameEvidence>,
    pub frame_count: u64,
    pub first_incomplete: Option<ReceiveDrainEvidence>,
    pub latest_incomplete: Option<ReceiveDrainEvidence>,
    pub incomplete_count: u64,
}

impl ReceiveFaultEvidence {
    pub(crate) fn frame(frame: ReceiveFrameEvidence) -> Self {
        Self {
            first_frame: Some(frame.clone()),
            latest_frame: Some(frame),
            frame_count: 1,
            ..Self::default()
        }
    }

    pub(crate) fn incomplete(drain: ReceiveDrainEvidence) -> Self {
        Self {
            first_incomplete: Some(drain.clone()),
            latest_incomplete: Some(drain),
            incomplete_count: 1,
            ..Self::default()
        }
    }

    fn merge(&mut self, other: Self) {
        if self.first_frame.is_none() {
            self.first_frame = other.first_frame;
        }
        if other.latest_frame.is_some() {
            self.latest_frame = other.latest_frame;
        }
        self.frame_count = self.frame_count.saturating_add(other.frame_count);
        if self.first_incomplete.is_none() {
            self.first_incomplete = other.first_incomplete;
        }
        if other.latest_incomplete.is_some() {
            self.latest_incomplete = other.latest_incomplete;
        }
        self.incomplete_count = self.incomplete_count.saturating_add(other.incomplete_count);
    }
}

/// Stable first occurrence and accumulated evidence for one hazard class/address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaultRecord {
    pub id: u64,
    pub class: FaultClass,
    pub joint: Option<String>,
    pub address: Option<MotorAddress>,
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub message: String,
    pub device: DeviceFaultEvidence,
    pub receive: ReceiveFaultEvidence,
}

/// Warning observations retained for diagnostics; they do not grant or revoke motion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceWarning {
    pub joint: String,
    pub address: MotorAddress,
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub warning_bytes: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopAction {
    ZeroSpeed,
    NeutralMit,
    Disable,
}

/// A successful write means transport acceptance, not a physical drive acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopAttempt {
    pub address: MotorAddress,
    pub action: StopAction,
    pub error: Option<String>,
}

/// All configured addresses are attempted even when earlier writes fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopReport {
    pub generation: u64,
    pub attempts: Vec<StopAttempt>,
}

impl StopReport {
    pub fn failed_writes(&self) -> usize {
        self.attempts
            .iter()
            .filter(|attempt| attempt.error.is_some())
            .count()
    }
}

/// Owned read-only projection. No reset or qualified physical recovery is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetySnapshot {
    pub faults: Vec<FaultRecord>,
    pub observed_warnings: Vec<DeviceWarning>,
    pub stop_generation: u64,
    pub hardware_estop_asserted: bool,
    pub last_stop: Option<StopReport>,
    /// First failed stop remains evidence even if later writes are accepted.
    pub first_failed_stop: Option<StopReport>,
    pub recovery_available: bool,
}

impl SafetySnapshot {
    pub fn first_fault(&self) -> Option<&FaultRecord> {
        self.faults.first()
    }

    pub fn is_latched(&self) -> bool {
        !self.faults.is_empty()
    }
}

#[derive(Debug, Default)]
pub(crate) struct FaultAuthority {
    faults: Vec<FaultRecord>,
    warnings: Vec<DeviceWarning>,
    stop_generation: u64,
    last_stop: Option<StopReport>,
    first_failed_stop: Option<StopReport>,
}

/// Cap for an accumulated fault record message: dedupe keeps one record
/// per (class, address), so distinct causes append here instead of
/// spawning unbounded records.
const MAX_FAULT_MESSAGE_LEN: usize = 1024;

impl FaultAuthority {
    pub(crate) fn is_latched(&self) -> bool {
        !self.faults.is_empty()
    }
    pub(crate) fn first_fault(&self) -> Option<&FaultRecord> {
        self.faults.first()
    }

    pub(crate) fn joint_is_faulted(&self, joint: &str) -> bool {
        self.faults
            .iter()
            .any(|fault| fault.joint.as_deref().is_none_or(|name| name == joint))
    }

    pub(crate) fn record_warning(&mut self, joint: &str, address: &MotorAddress, bytes: [u8; 4]) {
        if bytes == [0; 4] {
            return;
        }
        let now = Instant::now();
        if let Some(warning) = self
            .warnings
            .iter_mut()
            .find(|warning| warning.address == *address)
        {
            warning.last_seen = now;
            for (stored, observed) in warning.warning_bytes.iter_mut().zip(bytes) {
                *stored |= observed;
            }
        } else {
            self.warnings.push(DeviceWarning {
                joint: joint.to_owned(),
                address: address.clone(),
                first_seen: now,
                last_seen: now,
                warning_bytes: bytes,
            });
        }
    }

    /// Returns true only for the first transition into latched state.
    pub(crate) fn record(
        &mut self,
        class: FaultClass,
        joint: Option<String>,
        address: Option<MotorAddress>,
        message: &str,
        device: DeviceFaultEvidence,
    ) -> bool {
        let now = Instant::now();
        if let Some(existing) = self
            .faults
            .iter_mut()
            .find(|fault| fault.class == class && fault.address == address)
        {
            existing.last_seen = now;
            existing.device.merge(&device);
            // Dedupe keeps one record per (class, address), but later
            // distinct causes must stay distinguishable: accumulate bounded
            // message evidence instead of dropping it.
            let incoming = bounded_message(message);
            if !incoming.is_empty()
                && !existing.message.contains(incoming.as_str())
                && existing.message.len() < MAX_FAULT_MESSAGE_LEN
            {
                existing.message.push_str("; ");
                existing.message.push_str(&incoming);
                existing.message = bounded_message(&existing.message);
            }
            return false;
        }
        let first = self.faults.is_empty();
        // At most one record per class/configured address (plus global class).
        // Repeated faults accumulate masks rather than growing an event log.
        self.faults.push(FaultRecord {
            id: self.faults.len() as u64 + 1,
            class,
            joint,
            address,
            first_seen: now,
            last_seen: now,
            message: bounded_message(message),
            device,
            receive: ReceiveFaultEvidence::default(),
        });
        first
    }

    pub(crate) fn record_receive(
        &mut self,
        class: FaultClass,
        joint: Option<String>,
        address: Option<MotorAddress>,
        message: &str,
        device: DeviceFaultEvidence,
        receive: ReceiveFaultEvidence,
    ) -> bool {
        let first = self.record(class, joint, address.clone(), message, device);
        if let Some(record) = self
            .faults
            .iter_mut()
            .find(|record| record.class == class && record.address == address)
        {
            record.receive.merge(receive);
        }
        first
    }

    pub(crate) fn begin_stop(&mut self) -> u64 {
        self.stop_generation = self.stop_generation.saturating_add(1);
        self.stop_generation
    }

    pub(crate) fn stop_generation(&self) -> u64 {
        self.stop_generation
    }

    pub(crate) fn set_stop_report(&mut self, report: StopReport) {
        if report.failed_writes() > 0 && self.first_failed_stop.is_none() {
            self.first_failed_stop = Some(report.clone());
        }
        self.last_stop = Some(report);
    }

    pub(crate) fn snapshot(&self, hardware_estop_asserted: bool) -> SafetySnapshot {
        SafetySnapshot {
            faults: self.faults.clone(),
            observed_warnings: self.warnings.clone(),
            stop_generation: self.stop_generation,
            hardware_estop_asserted,
            last_stop: self.last_stop.clone(),
            first_failed_stop: self.first_failed_stop.clone(),
            recovery_available: false,
        }
    }
}

pub(crate) fn bounded_message(message: &str) -> String {
    message.chars().take(512).collect()
}

#[cfg(test)]
mod record_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use robstride::MotorAddress;

    fn address() -> MotorAddress {
        MotorAddress::new("can0", 1)
    }

    #[test]
    fn distinct_drive_state_causes_on_one_address_stay_distinguishable() {
        let mut authority = FaultAuthority::default();
        assert!(authority.record(
            FaultClass::DriveState,
            Some("joint".to_string()),
            Some(address()),
            "missing Enable echo",
            DeviceFaultEvidence::default(),
        ));
        assert!(!authority.record(
            FaultClass::DriveState,
            Some("joint".to_string()),
            Some(address()),
            "missing Off echo",
            DeviceFaultEvidence::default(),
        ));
        let snapshot = authority.snapshot(false);
        assert_eq!(snapshot.faults.len(), 1);
        let message = &snapshot.faults[0].message;
        assert!(
            message.contains("missing Enable echo") && message.contains("missing Off echo"),
            "both causes must survive dedupe: {message}"
        );
    }

    #[test]
    fn repeated_same_cause_does_not_grow_the_message() {
        let mut authority = FaultAuthority::default();
        for _ in 0..10 {
            authority.record(
                FaultClass::DriveState,
                Some("joint".to_string()),
                Some(address()),
                "missing Enable echo",
                DeviceFaultEvidence::default(),
            );
        }
        let snapshot = authority.snapshot(false);
        assert_eq!(snapshot.faults.len(), 1);
        assert_eq!(snapshot.faults[0].message, "missing Enable echo");
    }
}
