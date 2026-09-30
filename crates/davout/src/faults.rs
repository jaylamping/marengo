//! Persistent safety authority, independent of replaceable pose caches.

use std::time::Instant;

use robstride::MotorAddress;

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
        if other.drive_mode.is_some() {
            self.drive_mode = other.drive_mode;
        }
        if other.received_at > self.received_at {
            self.received_at = other.received_at;
        }
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
        });
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
