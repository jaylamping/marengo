//! Physical Robstride reference backend state (ADR 0036).
//!
//! Owner-local device identity, coordinate continuity and request/reply
//! correlation for the qualified physical acquisition. Nothing here is a permit:
//! the private authority binds an epoch from this state and every admission
//! re-reads it. No token, callback or transport is exported by this module.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use robstride::{
    DeviceUid, IdentityObservation, MotorAddress, ParameterId, ParameterReadObservation,
};
use rustc_hash::FxHashMap;

/// Upper bound for the type-0 identity round trip at each Enable admission,
/// from the first request. A Robstride drive transmits nothing for 48-57 ms
/// starting about 535 ms after a SetZero, and never answers a type-0 request
/// it received meanwhile (bench candumps 2026-10-03 14:51:33 and 15:34:09,
/// all five drives), so the window spans that blackout plus
/// [`IDENTITY_ADMISSION_RETRY`]. Longer silence also exceeds the default
/// `comm_watchdog_ms` liveness bound.
pub(crate) const IDENTITY_ADMISSION_TIMEOUT: Duration = Duration::from_millis(100);
/// A target that has not answered yet is asked again this often: a request
/// that reached a drive inside its post-SetZero blackout is never answered.
pub(crate) const IDENTITY_ADMISSION_RETRY: Duration = Duration::from_millis(10);
/// One identity admission poll; replies normally arrive within a millisecond.
pub(crate) const IDENTITY_ADMISSION_POLL: Duration = Duration::from_millis(5);
/// Retained replies per kind. A request only accepts replies popped after it.
const INBOX_CAPACITY: usize = 32;

/// Fixed owner realm and monotonic transaction clock. Cloning shares the realm.
#[derive(Clone)]
pub(crate) struct PhysicalBackend {
    pub(crate) realm: Arc<()>,
    origin: Instant,
}

impl PhysicalBackend {
    pub(crate) fn new() -> Self {
        Self {
            realm: Arc::new(()),
            origin: Instant::now(),
        }
    }

    /// Host monotonic time since owner construction; governs transaction deadlines.
    pub(crate) fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// Coordinate continuity bounds, resolved per installed address at construction.
#[derive(Clone, Copy)]
pub(crate) struct ContinuityBounds {
    /// Largest joint-space speed the drive can report (vendor MIT velocity range).
    pub(crate) max_rate_rad_s: f64,
    /// Two feedback quantization steps in joint space.
    pub(crate) quantization_rad: f64,
}

struct Device {
    bounds: ContinuityBounds,
    /// Advances on every reason the drive's coordinate may have changed.
    epoch: u64,
    uid: Option<DeviceUid>,
    last_seen: Option<Instant>,
    last_position_rad: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct InboxIdentity {
    serial: u64,
    pub(crate) address: MotorAddress,
    pub(crate) uid: DeviceUid,
}

#[derive(Debug, Clone)]
pub(crate) struct InboxRead {
    serial: u64,
    pub(crate) observation: ParameterReadObservation,
}

/// Reason a device epoch advanced; inspection only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContinuityBreak {
    IdentityChanged,
    Discontinuity,
    CalibrationMode,
}

/// Every installed address of one physical owner. Never serialized or shared.
#[derive(Default)]
pub(crate) struct PhysicalDevices {
    devices: FxHashMap<MotorAddress, Device>,
    serial: u64,
    identities: VecDeque<InboxIdentity>,
    reads: VecDeque<InboxRead>,
    last_owner_work: Option<Instant>,
}

impl PhysicalDevices {
    pub(crate) fn new(
        installed: impl IntoIterator<Item = (MotorAddress, ContinuityBounds)>,
    ) -> Self {
        Self {
            devices: installed
                .into_iter()
                .map(|(address, bounds)| {
                    (
                        address,
                        Device {
                            bounds,
                            epoch: 1,
                            uid: None,
                            last_seen: None,
                            last_position_rad: None,
                        },
                    )
                })
                .collect(),
            ..Self::default()
        }
    }

    /// Replies popped after this watermark were received after the caller's request.
    pub(crate) fn watermark(&self) -> u64 {
        self.serial
    }

    /// Owner reference work suspends reporting; liveness restarts from its end.
    pub(crate) fn mark_owner_work(&mut self, now: Instant) {
        self.last_owner_work = Some(self.last_owner_work.map_or(now, |last| last.max(now)));
    }

    pub(crate) fn record_identity(
        &mut self,
        observation: IdentityObservation,
    ) -> Option<ContinuityBreak> {
        self.serial = self.serial.saturating_add(1);
        let mut changed = None;
        if let Some(device) = self.devices.get_mut(&observation.address) {
            if device.uid.is_some_and(|uid| uid != observation.uid) {
                device.epoch = device.epoch.saturating_add(1);
                changed = Some(ContinuityBreak::IdentityChanged);
            }
            device.uid = Some(observation.uid);
        }
        if self.identities.len() == INBOX_CAPACITY {
            self.identities.pop_front();
        }
        self.identities.push_back(InboxIdentity {
            serial: self.serial,
            address: observation.address,
            uid: observation.uid,
        });
        changed
    }

    pub(crate) fn record_read(&mut self, observation: ParameterReadObservation) {
        self.serial = self.serial.saturating_add(1);
        if self.reads.len() == INBOX_CAPACITY {
            self.reads.pop_front();
        }
        self.reads.push_back(InboxRead {
            serial: self.serial,
            observation,
        });
    }

    /// First identity reply from `address` popped after `watermark`.
    pub(crate) fn identity_after(
        &self,
        address: &MotorAddress,
        watermark: u64,
    ) -> Option<DeviceUid> {
        self.identities
            .iter()
            .find(|reply| reply.serial > watermark && reply.address == *address)
            .map(|reply| reply.uid)
    }

    /// First reply for `parameter` from `address` popped after `watermark`.
    pub(crate) fn read_after(
        &self,
        address: &MotorAddress,
        parameter: ParameterId,
        watermark: u64,
    ) -> Option<&ParameterReadObservation> {
        self.reads
            .iter()
            .find(|read| {
                read.serial > watermark
                    && read.observation.address == *address
                    && read.observation.reply.index == parameter.as_u16()
            })
            .map(|read| &read.observation)
    }

    /// Another installed address already answered with this identifier.
    pub(crate) fn uid_claimed_elsewhere(&self, address: &MotorAddress, uid: DeviceUid) -> bool {
        self.devices
            .iter()
            .any(|(other, device)| other != address && device.uid == Some(uid))
    }

    /// An addressed SetZero attempt starts a new coordinate, even if delivery is
    /// uncertain. Returns the new epoch, or `None` when the counter is exhausted.
    pub(crate) fn begin_new_coordinate(&mut self, address: &MotorAddress) -> Option<u64> {
        let device = self.devices.get_mut(address)?;
        device.epoch = device.epoch.checked_add(1)?;
        device.last_seen = None;
        device.last_position_rad = None;
        Some(device.epoch)
    }

    /// Admitted joint-space pose or readback for an installed address. Within
    /// continuous observation a jump beyond what the drive can report breaks
    /// continuity. Across owner reference work (all joints stopped and the arm
    /// supported) the bound is the at-rest reference tolerance.
    pub(crate) fn observe_position(
        &mut self,
        address: &MotorAddress,
        position_rad: f64,
        received_at: Instant,
        at_rest_tolerance_rad: f64,
    ) -> Option<ContinuityBreak> {
        let last_owner_work = self.last_owner_work;
        let device = self.devices.get_mut(address)?;
        let mut broken = None;
        if let (Some(previous), Some(seen)) = (device.last_position_rad, device.last_seen) {
            let spans_owner_work = last_owner_work.is_some_and(|work| work > seen);
            let bound = if spans_owner_work {
                at_rest_tolerance_rad
            } else {
                let dt = received_at.saturating_duration_since(seen).as_secs_f64();
                device.bounds.max_rate_rad_s * dt + device.bounds.quantization_rad
            };
            if !position_rad.is_finite() || (position_rad - previous).abs() > bound {
                broken = Some(ContinuityBreak::Discontinuity);
            }
        }
        if broken.is_some() {
            device.epoch = device.epoch.saturating_add(1);
        }
        device.last_position_rad = Some(position_rad);
        device.last_seen = Some(
            device
                .last_seen
                .map_or(received_at, |seen| seen.max(received_at)),
        );
        broken
    }

    /// A drive reporting Calibration mode is recalibrating its encoder.
    pub(crate) fn record_calibration_mode(
        &mut self,
        address: &MotorAddress,
    ) -> Option<ContinuityBreak> {
        let device = self.devices.get_mut(address)?;
        device.epoch = device.epoch.saturating_add(1);
        Some(ContinuityBreak::CalibrationMode)
    }

    /// Coordinate epoch without the liveness rule (pre-selection stage checks).
    pub(crate) fn epoch(&self, address: &MotorAddress) -> Option<u64> {
        self.devices.get(address).map(|device| device.epoch)
    }

    /// Epoch of a granted address, or `None` once it went unobserved for longer
    /// than `window` outside owner reference work (comm loss or a possible reboot).
    /// While an Active session withholds the address's traffic from pose (its
    /// Enable echo is pending, since `withheld_since`), silence counts from that
    /// start instead: the owner is not listening, and the Enable-echo bound
    /// (the same window from activation) fails the address closed.
    pub(crate) fn live_epoch(
        &self,
        address: &MotorAddress,
        now: Instant,
        window: Duration,
        owner_busy: bool,
        withheld_since: Option<Instant>,
    ) -> Option<u64> {
        let device = self.devices.get(address)?;
        if owner_busy {
            return Some(device.epoch);
        }
        let reference = [device.last_seen, self.last_owner_work, withheld_since]
            .into_iter()
            .flatten()
            .max()?;
        (now.saturating_duration_since(reference) <= window).then_some(device.epoch)
    }

    pub(crate) fn uid(&self, address: &MotorAddress) -> Option<DeviceUid> {
        self.devices.get(address).and_then(|device| device.uid)
    }
}
