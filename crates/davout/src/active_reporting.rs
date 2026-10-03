//! Per-joint Active Reporting (Robstride type-24) desired/applied state and leases.
//!
//! Desired for joint M:
//! `!mode_active && (global_diagnostics || any_unexpired_lease(M))`.
//! Applied tracks last successful F_CMD so failed CAN TX retries on the next sync.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use marengo_config::{MotorEntry, MotorsConfigFile};
use robstride::bus::{BusError, MotorAddress, MotorBus};

/// Default lease lifetime when Consul holds a modal open.
pub const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(30);

/// Re-send type-24 enable at least this often while free-drive sensing is desired.
/// Motors can drop Active Reporting after motion/glitches; applied-only sync would
/// never retry and Set Limits would freeze on a stale near-zero sample.
pub const ACTIVE_REPORTING_HEARTBEAT: Duration = Duration::from_secs(1);

/// Separate healthy-stream refresh attempts by one 200 Hz control period.
/// Initial enables, stale-feedback recovery and reporting Off are not delayed.
const ACTIVE_REPORTING_HEARTBEAT_SPACING: Duration = Duration::from_millis(5);

/// If desired-on and no feedback arrives within this window after enable (or after
/// the last RX), clear the applied bit so the next sync re-asserts type-24.
pub const ACTIVE_REPORTING_FEEDBACK_STALE: Duration = Duration::from_millis(200);

const MAX_LEASE_ID_LEN: usize = 64;
const MAX_CLIENT_ID_LEN: usize = 64;
const MAX_LEASES_PER_JOINT: usize = 8;

#[derive(Debug, Clone)]
struct LeaseEntry {
    client_id: String,
    expires: Instant,
}

/// Lease map + last successfully applied type-24 enable bit per joint.
#[derive(Debug, Default)]
pub struct ActiveReportingState {
    leases: HashMap<String, HashMap<String, LeaseEntry>>,
    applied: HashMap<String, bool>,
    /// Last successful type-24 enable TX time (for heartbeat refresh).
    last_enable_tx: HashMap<String, Instant>,
    /// Rotate refresh attempts even when the selected route's write fails.
    heartbeat_cursor: usize,
    last_heartbeat_attempt: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveReportingLeaseError {
    UnknownJoint { joint: String },
    InvalidLeaseId,
    InvalidClientId,
    TooManyLeases { joint: String },
    MissingLease { joint: String, lease_id: String },
}

/// One actual type-24 Off attempt on an installed route.
#[derive(Debug)]
pub(super) struct ReportingSuspendAttempt {
    pub(super) joint: String,
    pub(super) address: MotorAddress,
    pub(super) error: Option<BusError>,
}

/// At most one Off attempt per installed motor that was applied On.
/// Accepted writes do not acknowledge physical quiescence; the owner must drain.
#[derive(Debug)]
#[must_use]
pub(super) struct ReportingSuspendReport {
    pub(super) attempts: Vec<ReportingSuspendAttempt>,
}

impl ActiveReportingState {
    /// Suspend previously applied streams without changing desired leases.
    /// Every installed On route is attempted once even after another write fails.
    /// Failed routes remain applied On; no retry or receive occurs in this call.
    pub(super) fn suspend_applied<B: MotorBus>(
        &mut self,
        bus: &mut B,
        installed_motors: &[MotorEntry],
    ) -> ReportingSuspendReport {
        let mut attempts = Vec::new();
        for motor in installed_motors {
            if !self.applied_on(&motor.joint) {
                continue;
            }
            let address = MotorAddress::from(motor);
            let error = bus.disable_active_reporting_at(&address).err();
            if error.is_none() {
                self.applied.insert(motor.joint.clone(), false);
                self.last_enable_tx.remove(&motor.joint);
            }
            attempts.push(ReportingSuspendAttempt {
                joint: motor.joint.clone(),
                address,
                error,
            });
        }
        ReportingSuspendReport { attempts }
    }

    pub fn clear_applied(&mut self) {
        self.applied.clear();
        self.last_enable_tx.clear();
        self.heartbeat_cursor = 0;
        self.last_heartbeat_attempt = None;
    }

    pub fn clear_applied_joint(&mut self, joint: &str) {
        self.applied.remove(joint);
        self.last_enable_tx.remove(joint);
    }

    pub fn applied_on(&self, joint: &str) -> bool {
        self.applied.get(joint).copied().unwrap_or(false)
    }

    pub fn has_live_lease(&self, joint: &str, now: Instant) -> bool {
        self.leases
            .get(joint)
            .is_some_and(|m| m.values().any(|e| e.expires > now))
    }

    pub fn lease_count(&self, joint: &str, now: Instant) -> usize {
        self.leases
            .get(joint)
            .map(|m| m.values().filter(|e| e.expires > now).count())
            .unwrap_or(0)
    }

    pub fn desired(
        &self,
        joint: &str,
        mode_active: bool,
        global_diagnostics: bool,
        now: Instant,
    ) -> bool {
        !mode_active && (global_diagnostics || self.has_live_lease(joint, now))
    }

    /// Drop expired lease entries. Returns true if any entry was removed.
    pub fn expire_stale(&mut self, now: Instant) -> bool {
        let mut changed = false;
        self.leases.retain(|_, by_id| {
            let before = by_id.len();
            by_id.retain(|_, entry| entry.expires > now);
            if by_id.len() != before {
                changed = true;
            }
            !by_id.is_empty()
        });
        changed
    }

    pub fn acquire(
        &mut self,
        joint: &str,
        client_id: &str,
        lease_id: &str,
        ttl: Duration,
        now: Instant,
        known_joints: &MotorsConfigFile,
    ) -> Result<(), ActiveReportingLeaseError> {
        validate_ids(client_id, lease_id)?;
        ensure_known_joint(joint, known_joints)?;
        let entry = LeaseEntry {
            client_id: client_id.to_string(),
            expires: now + ttl,
        };
        let by_id = self.leases.entry(joint.to_string()).or_default();
        if !by_id.contains_key(lease_id) && by_id.len() >= MAX_LEASES_PER_JOINT {
            return Err(ActiveReportingLeaseError::TooManyLeases {
                joint: joint.to_string(),
            });
        }
        by_id.insert(lease_id.to_string(), entry);
        Ok(())
    }

    pub fn renew(
        &mut self,
        joint: &str,
        client_id: &str,
        lease_id: &str,
        ttl: Duration,
        now: Instant,
        known_joints: &MotorsConfigFile,
    ) -> Result<(), ActiveReportingLeaseError> {
        validate_ids(client_id, lease_id)?;
        ensure_known_joint(joint, known_joints)?;
        let Some(by_id) = self.leases.get_mut(joint) else {
            return Err(ActiveReportingLeaseError::MissingLease {
                joint: joint.to_string(),
                lease_id: lease_id.to_string(),
            });
        };
        let Some(entry) = by_id.get_mut(lease_id) else {
            return Err(ActiveReportingLeaseError::MissingLease {
                joint: joint.to_string(),
                lease_id: lease_id.to_string(),
            });
        };
        // Client may rotate labels; lease_id is authoritative.
        entry.client_id = client_id.to_string();
        entry.expires = now + ttl;
        Ok(())
    }

    pub fn release(
        &mut self,
        joint: &str,
        lease_id: &str,
        known_joints: &MotorsConfigFile,
    ) -> Result<(), ActiveReportingLeaseError> {
        if lease_id.trim().is_empty() || lease_id.len() > MAX_LEASE_ID_LEN {
            return Err(ActiveReportingLeaseError::InvalidLeaseId);
        }
        ensure_known_joint(joint, known_joints)?;
        if let Some(by_id) = self.leases.get_mut(joint) {
            by_id.remove(lease_id);
            if by_id.is_empty() {
                self.leases.remove(joint);
            }
        }
        Ok(())
    }

    /// Diff desired vs applied and send type-24 enables/disables. Updates applied only on Ok.
    ///
    /// While sensing is desired, also re-asserts enable on heartbeat and when
    /// `last_feedback_rx` shows the joint has gone silent (free-drive dropout).
    /// Healthy heartbeat attempts rotate with spacing across repeated callers;
    /// initial enables, stale retries and reporting Off retain immediate handling.
    pub fn sync<B: MotorBus, S: std::hash::BuildHasher>(
        &mut self,
        bus: &mut B,
        motors: &MotorsConfigFile,
        mode_active: bool,
        global_diagnostics: bool,
        now: Instant,
        last_feedback_rx: &HashMap<String, Instant, S>,
    ) {
        self.expire_stale(now);
        let spacing_elapsed = self
            .last_heartbeat_attempt
            .map(|last| now.saturating_duration_since(last) >= ACTIVE_REPORTING_HEARTBEAT_SPACING)
            .unwrap_or(true);
        let heartbeat_motor = if spacing_elapsed && !motors.motors.is_empty() {
            let count = motors.motors.len();
            let start = self.heartbeat_cursor % count;
            (0..count)
                .map(|offset| (start + offset) % count)
                .find(|index| {
                    let joint = &motors.motors[*index].joint;
                    self.desired(joint, mode_active, global_diagnostics, now)
                        && self.applied_on(joint)
                        && last_feedback_rx.get(joint).is_some_and(|last| {
                            now.saturating_duration_since(*last) < ACTIVE_REPORTING_FEEDBACK_STALE
                        })
                        && self
                            .last_enable_tx
                            .get(joint)
                            .map(|last| {
                                now.saturating_duration_since(*last) >= ACTIVE_REPORTING_HEARTBEAT
                            })
                            .unwrap_or(true)
                })
        } else {
            None
        };
        for (index, motor) in motors.motors.iter().enumerate() {
            let joint = motor.joint.as_str();
            let want = self.desired(joint, mode_active, global_diagnostics, now);
            let have = self.applied_on(joint);
            let feedback_stale = if !have {
                false
            } else {
                match last_feedback_rx.get(joint) {
                    Some(t) => now.saturating_duration_since(*t) >= ACTIVE_REPORTING_FEEDBACK_STALE,
                    // Enabled on paper but never saw RX — retry after stale window.
                    None => self
                        .last_enable_tx
                        .get(joint)
                        .map(|t| {
                            now.saturating_duration_since(*t) >= ACTIVE_REPORTING_FEEDBACK_STALE
                        })
                        .unwrap_or(false),
                }
            };
            let need_enable = want && (!have || heartbeat_motor == Some(index) || feedback_stale);
            let need_disable = !want && have;
            if !need_enable && !need_disable {
                continue;
            }
            if heartbeat_motor == Some(index) {
                self.last_heartbeat_attempt = Some(now);
                self.heartbeat_cursor = (index + 1) % motors.motors.len();
            }
            let address = MotorAddress::from(motor);
            let result = if want {
                bus.enable_active_reporting_at(&address)
            } else {
                bus.disable_active_reporting_at(&address)
            };
            if result.is_ok() {
                self.applied.insert(joint.to_string(), want);
                if want {
                    self.last_enable_tx.insert(joint.to_string(), now);
                } else {
                    self.last_enable_tx.remove(joint);
                }
            }
        }
    }
}

fn validate_ids(client_id: &str, lease_id: &str) -> Result<(), ActiveReportingLeaseError> {
    let client = client_id.trim();
    let lease = lease_id.trim();
    if client.is_empty() || client.len() > MAX_CLIENT_ID_LEN {
        return Err(ActiveReportingLeaseError::InvalidClientId);
    }
    if lease.is_empty() || lease.len() > MAX_LEASE_ID_LEN {
        return Err(ActiveReportingLeaseError::InvalidLeaseId);
    }
    Ok(())
}

fn ensure_known_joint(
    joint: &str,
    known_joints: &MotorsConfigFile,
) -> Result<(), ActiveReportingLeaseError> {
    if known_joints.motors.iter().any(|m| m.joint == joint) {
        Ok(())
    } else {
        Err(ActiveReportingLeaseError::UnknownJoint {
            joint: joint.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use marengo_config::{MotorBenchLimits, MotorEntry, MotorType, MotorsConfigFile};
    use robstride::bus::MemoryBus;
    use robstride::{unpack_ext_id, CommunicationType};

    fn sample_motor(joint: &str, device_id: u8) -> MotorEntry {
        MotorEntry {
            joint: joint.into(),
            driver: "robstride".into(),
            motor_type: MotorType::Rs02,
            can_interface: "can0".into(),
            device_id,
            direction: 1,
            gear_ratio: 1.0,
            recv_can_id: 0,
            firmware_version: "test".into(),
            bench: MotorBenchLimits {
                position_lower_rad: -1.0,
                position_upper_rad: 1.0,
                velocity_limit_rad_s: 1.0,
                torque_limit_nm: 1.0,
            },
        }
    }

    fn motors_two() -> MotorsConfigFile {
        MotorsConfigFile {
            motors: vec![sample_motor("j1", 1), sample_motor("j2", 2)],
        }
    }

    fn type24_cmds(bus: &MemoryBus) -> Vec<(u8, u8)> {
        bus.tx
            .iter()
            .filter_map(|f| {
                let u = unpack_ext_id(f.id)?;
                if u.comm_type != CommunicationType::ActiveReporting.as_u8() {
                    return None;
                }
                Some((u.device_id, f.data[6]))
            })
            .collect()
    }

    #[test]
    fn lease_only_enables_that_joint() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let mut bus = MemoryBus::default();
        let now = Instant::now();
        state
            .acquire("j1", "c1", "lease-a", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire");
        let rx = HashMap::new();
        state.sync(&mut bus, &motors, false, false, now, &rx);
        let cmds = type24_cmds(&bus);
        assert_eq!(cmds, vec![(1, 0x01)]);
        assert!(state.applied_on("j1"));
        assert!(!state.applied_on("j2"));
    }

    #[test]
    fn stale_release_does_not_kill_newer_lease() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let now = Instant::now();
        state
            .acquire("j1", "c1", "lease-old", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire old");
        state
            .acquire("j1", "c1", "lease-new", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire new");
        state
            .release("j1", "lease-old", &motors)
            .expect("release old");
        assert_eq!(state.lease_count("j1", now), 1);
        assert!(state.has_live_lease("j1", now));
    }

    #[test]
    fn expire_drops_desired_without_mode_change() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let mut bus = MemoryBus::default();
        let now = Instant::now();
        state
            .acquire(
                "j1",
                "c1",
                "lease-a",
                Duration::from_millis(1),
                now,
                &motors,
            )
            .expect("acquire");
        let rx = HashMap::new();
        state.sync(&mut bus, &motors, false, false, now, &rx);
        assert!(state.applied_on("j1"));
        bus.tx.clear();
        let later = now + Duration::from_millis(50);
        state.sync(&mut bus, &motors, false, false, later, &rx);
        assert_eq!(type24_cmds(&bus), vec![(1, 0x00)]);
        assert!(!state.applied_on("j1"));
    }

    #[test]
    fn active_mode_forces_off_despite_lease() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let mut bus = MemoryBus::default();
        let now = Instant::now();
        state
            .acquire("j1", "c1", "lease-a", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire");
        let rx = HashMap::new();
        state.sync(&mut bus, &motors, false, false, now, &rx);
        bus.tx.clear();
        state.sync(&mut bus, &motors, true, false, now, &rx);
        assert_eq!(type24_cmds(&bus), vec![(1, 0x00)]);
    }

    #[test]
    fn heartbeat_reasserts_enable_while_desired() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let mut bus = MemoryBus::default();
        let now = Instant::now();
        state
            .acquire("j1", "c1", "lease-a", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire");
        let mut rx = HashMap::new();
        rx.insert("j1".into(), now);
        state.sync(&mut bus, &motors, false, false, now, &rx);
        assert_eq!(type24_cmds(&bus), vec![(1, 0x01)]);
        bus.tx.clear();
        // Fresh feedback — no re-TX until heartbeat.
        rx.insert("j1".into(), now + Duration::from_millis(100));
        state.sync(
            &mut bus,
            &motors,
            false,
            false,
            now + Duration::from_millis(100),
            &rx,
        );
        assert!(type24_cmds(&bus).is_empty());
        // Heartbeat due with healthy RX — re-assert enable.
        rx.insert("j1".into(), now + ACTIVE_REPORTING_HEARTBEAT);
        state.sync(
            &mut bus,
            &motors,
            false,
            false,
            now + ACTIVE_REPORTING_HEARTBEAT,
            &rx,
        );
        assert_eq!(type24_cmds(&bus), vec![(1, 0x01)]);
        assert!(state.applied_on("j1"));
    }

    #[test]
    fn stale_feedback_reasserts_enable() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let mut bus = MemoryBus::default();
        let now = Instant::now();
        state
            .acquire("j1", "c1", "lease-a", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire");
        let rx_empty = HashMap::new();
        state.sync(&mut bus, &motors, false, false, now, &rx_empty);
        assert_eq!(type24_cmds(&bus), vec![(1, 0x01)]);
        bus.tx.clear();
        // No RX since enable — after stale window, re-assert.
        let later = now + ACTIVE_REPORTING_FEEDBACK_STALE;
        state.sync(&mut bus, &motors, false, false, later, &rx_empty);
        assert_eq!(type24_cmds(&bus), vec![(1, 0x01)]);
        assert!(state.applied_on("j1"));
    }

    #[test]
    fn fresh_feedback_suppresses_stale_retry_before_heartbeat() {
        let motors = motors_two();
        let mut state = ActiveReportingState::default();
        let mut bus = MemoryBus::default();
        let now = Instant::now();
        state
            .acquire("j1", "c1", "lease-a", DEFAULT_LEASE_TTL, now, &motors)
            .expect("acquire");
        let mut rx = HashMap::new();
        state.sync(&mut bus, &motors, false, false, now, &rx);
        bus.tx.clear();
        let later = now + ACTIVE_REPORTING_FEEDBACK_STALE;
        rx.insert("j1".into(), later);
        state.sync(&mut bus, &motors, false, false, later, &rx);
        assert!(
            type24_cmds(&bus).is_empty(),
            "healthy feedback must not force re-enable before heartbeat"
        );
    }
}
