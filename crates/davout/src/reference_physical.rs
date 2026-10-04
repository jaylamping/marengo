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
/// from the first request (plus two [`IDENTITY_ADMISSION_SPACING`] per further target). A Robstride drive transmits nothing for 45-61 ms
/// starting 511-614 ms after a SetZero, and never answers a type-0 request it
/// received meanwhile (right-arm drives 1-2 on firmware 0.3.1.42, 3-4 on
/// 0.2.3.34, 5 on 0.0.3.32; 124 measured blackouts; profile
/// `docs/commissioning/firmware/robstride-timing-profile.json`, replies
/// otherwise within 0.5 ms), so the window spans that blackout plus
/// [`IDENTITY_ADMISSION_RETRY`]; `tests/firmware_profile.rs` keeps that
/// margin. Longer silence also exceeds the default `comm_watchdog_ms` liveness
/// bound.
pub const IDENTITY_ADMISSION_TIMEOUT: Duration = Duration::from_millis(100);
/// A target that has not answered yet is asked again this often: a request
/// that reached a drive inside its post-SetZero blackout is never answered.
pub const IDENTITY_ADMISSION_RETRY: Duration = Duration::from_millis(10);
/// Admission requests to different targets leave this far apart, one per
/// interface-group spacing, so their replies cannot overrun the controller's
/// two receive buffers. The deadline grows by two spacings per extra target.
pub const IDENTITY_ADMISSION_SPACING: Duration = crate::burst::BURST_GROUP_SPACING;
/// One identity admission poll; replies normally arrive within a millisecond.
pub(crate) const IDENTITY_ADMISSION_POLL: Duration = Duration::from_millis(5);
/// Longest a type-24 On held back from a drive's possible post-SetZero
/// blackout may stay unwritten once the hold ends (the quiet's end, or the end
/// of owner reference work when later) before the drive loses its grant
/// (ADR 0036, *Owed On*). Until the On is written the drive's silence is the
/// host's; after it, `comm_watchdog_ms` counts from the write. Twice the
/// slowest synchronous host work measured on the Pi (marengo-pi's gravity
/// preflight, 96 ms), rounded up to 50 ms. Only a drive outside an Active
/// session can owe an On, so no command depends on it meanwhile.
pub const OWED_ON_WRITE_BOUND: Duration = Duration::from_millis(200);
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
    /// Silence before this instant is the host's doing, not the drive's: the
    /// host withheld a frame the drive needs to speak (a type-24 On it kept out
    /// of the post-SetZero blackout, until its write) or the drive has not yet
    /// had the chance to answer one (an Enable whose echo was just read; the
    /// stop that ended an Active session). Monotonic.
    silence_counts_from: Option<Instant>,
    /// A type-24 On the reporting sync held back to this instant (the end of
    /// the drive's post-SetZero quiet) and has not written yet. Until it is
    /// written the drive's silence is the host's, for at most
    /// [`OWED_ON_WRITE_BOUND`].
    owed_on_from: Option<Instant>,
}

/// Why a granted address failed the liveness rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lapse {
    /// No feedback within `comm_watchdog_ms` of the instant silence counts
    /// from (or the address was never observed).
    Silent,
    /// A type-24 On held for the post-SetZero quiet was still unwritten
    /// [`OWED_ON_WRITE_BOUND`] after the hold ended.
    OwedOnUnwritten,
}

/// Where the liveness rule starts counting an address's silence (ADR 0036,
/// *Host-caused silence*). Every excuse below ends at an instant the host
/// controls; [`PhysicalDevices::count_silence_from`] and owner work apply in
/// every mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SilenceFrom {
    /// Outside an Active session, and for a granted joint outside the active
    /// set: the address's last frame. A type-24 On the host still owes the
    /// drive excuses it until written, for at most [`OWED_ON_WRITE_BOUND`].
    LastFrame,
    /// Active, the address's traffic withheld from pose until its Enable echo:
    /// its last frame or this instant (activation, or its quiet end), whichever
    /// is later. The Enable-echo bound fails the address closed meanwhile.
    Withheld(Instant),
    /// Active target: the write of the earliest host frame it answers that no
    /// pose has followed. `None`: the host has asked nothing since the drive's
    /// last pose, so no silence counts.
    Solicited(Option<Instant>),
}

/// What became of an owed type-24 On at a reporting sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwedOn {
    /// Still due and unwritten.
    Owed,
    /// Written at this instant: silence counts from it.
    Written(Instant),
    /// No longer wanted (an Active session, an expired lease): silence counts
    /// from the quiet's end, as for a held On that is no longer owed.
    Released,
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
                            silence_counts_from: None,
                            owed_on_from: None,
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

    /// The host caused `address`'s silence up to `at`: liveness counts it from
    /// `at` at the earliest (see `Device::silence_counts_from`). The latest
    /// instant wins. A drive that stays silent for `comm_watchdog_ms` after
    /// `at` still loses its grant.
    pub(crate) fn count_silence_from(&mut self, address: &MotorAddress, at: Instant) {
        if let Some(device) = self.devices.get_mut(address) {
            device.silence_counts_from = Some(device.silence_counts_from.map_or(at, |s| s.max(at)));
        }
    }

    /// The reporting sync held `address`'s due type-24 On until `quiet_end`.
    /// Until [`Self::settle_owed_reporting_ons`] sees it written, the drive's
    /// silence is excused, for at most [`OWED_ON_WRITE_BOUND`] past the later
    /// of `quiet_end` and the end of owner work.
    pub(crate) fn owe_reporting_on(&mut self, address: &MotorAddress, quiet_end: Instant) {
        if let Some(device) = self.devices.get_mut(address) {
            device.owed_on_from = Some(device.owed_on_from.map_or(quiet_end, |s| s.max(quiet_end)));
        }
    }

    /// Resolve every owed On with `fate`, called after each reporting sync.
    /// A written On restarts liveness from its write; a released one from its
    /// quiet's end.
    pub(crate) fn settle_owed_reporting_ons(
        &mut self,
        mut fate: impl FnMut(&MotorAddress) -> OwedOn,
    ) {
        for (address, device) in &mut self.devices {
            let Some(owed_from) = device.owed_on_from else {
                continue;
            };
            let counts_from = match fate(address) {
                OwedOn::Owed => continue,
                OwedOn::Written(at) => at,
                OwedOn::Released => owed_from,
            };
            device.owed_on_from = None;
            device.silence_counts_from = Some(
                device
                    .silence_counts_from
                    .map_or(counts_from, |s| s.max(counts_from)),
            );
        }
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

    /// Epoch of a granted address, or why it lapsed: silent for longer than
    /// `window` outside owner reference work (comm loss or a possible reboot),
    /// counted from `from`, or an owed type-24 On left unwritten past
    /// [`OWED_ON_WRITE_BOUND`]. Silence the host itself caused
    /// ([`Self::count_silence_from`], owner work) is not counted.
    pub(crate) fn live_epoch(
        &self,
        address: &MotorAddress,
        now: Instant,
        window: Duration,
        owner_busy: bool,
        from: SilenceFrom,
    ) -> Result<u64, Lapse> {
        let device = self.devices.get(address).ok_or(Lapse::Silent)?;
        if owner_busy {
            return Ok(device.epoch);
        }
        if let (SilenceFrom::LastFrame, Some(owed_from)) = (from, device.owed_on_from) {
            // The host has not written the On the drive needs to speak. The
            // excuse ends a fixed bound after the host could first write it.
            let writable = self
                .last_owner_work
                .map_or(owed_from, |work| work.max(owed_from));
            return if now.saturating_duration_since(writable) <= OWED_ON_WRITE_BOUND {
                Ok(device.epoch)
            } else {
                Err(Lapse::OwedOnUnwritten)
            };
        }
        match self.counted_silence(address, now, from) {
            Some(silence) if silence <= window => Ok(device.epoch),
            _ => Err(Lapse::Silent),
        }
    }

    /// Silence the liveness rule counts for `address` at `now` (see
    /// [`Self::live_epoch`] and [`SilenceFrom`]), or `None` when it was never
    /// observed.
    pub(crate) fn counted_silence(
        &self,
        address: &MotorAddress,
        now: Instant,
        from: SilenceFrom,
    ) -> Option<Duration> {
        let device = self.devices.get(address)?;
        let counted_from = match from {
            SilenceFrom::Solicited(None) => return Some(Duration::ZERO),
            SilenceFrom::Solicited(Some(solicited)) => [Some(solicited), None],
            SilenceFrom::Withheld(since) => [device.last_seen, Some(since)],
            SilenceFrom::LastFrame => [device.last_seen, None],
        };
        let reference = counted_from
            .into_iter()
            .chain([self.last_owner_work, device.silence_counts_from])
            .flatten()
            .max()?;
        Some(now.saturating_duration_since(reference))
    }

    pub(crate) fn uid(&self, address: &MotorAddress) -> Option<DeviceUid> {
        self.devices.get(address).and_then(|device| device.uid)
    }

    /// Latest admitted pose time for `address`, if it was ever observed.
    pub(crate) fn last_seen(&self, address: &MotorAddress) -> Option<Instant> {
        self.devices
            .get(address)
            .and_then(|device| device.last_seen)
    }
}
