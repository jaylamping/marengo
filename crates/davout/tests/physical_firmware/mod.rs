//! Test-only Robstride firmware emulator behind the public `MotorBus` seam.
//!
//! Each configured address owns an encoder, a volatile zero offset, enable and
//! reporting flags, an MCU identifier and fault knobs. A host frame goes on
//! the wire when written (or, while held, when released): its SocketCAN echo
//! is queued first, then the drive's replies, in wire order (a reply with a
//! modeled latency is delivered once that latency has passed, never ahead of
//! an earlier reply from the same drive). Type-24 reports are emitted when the
//! test pumps, at most once per the drive's report period, or after an Enable
//! when a drive models a report built before it acted on that Enable. Like the
//! bench drives, each drive ignores every frame and transmits nothing in its
//! post-SetZero blackout (`Drive::set_zero_blackout` after receiving a
//! SetZero). [`TIMING_MODEL`] bounds every timing knob; it covers the measured
//! bench profile (`tests/firmware_profile.rs`). Inbound status/reply identifiers
//! follow the documented wire layout (`type << 24 | extra << 8 | low`);
//! outbound frames are produced by the supervisor through the robstride encoders.
#![allow(dead_code, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use marengo_config::{MotorEntry, MotorType};
use robstride::motor_type::MitRanges;
use robstride::{
    pack_ext_id, BusError, CanBus, CanFrame, CommunicationType, MotorBus, ParameterId,
    ReceiveAttempt, ReceivedCanFrame, TimedCanFrame, DEFAULT_HOST_ID,
};

const RESET_MODE: u32 = 0;
const RUN_MODE: u32 = 2;
/// Timing ranges a drive's knobs may take. Each covers the range measured on
/// the bench (firmware 0.3.1.42; `docs/commissioning/firmware/
/// robstride-timing-profile.json`, asserted by `tests/firmware_profile.rs`).
#[derive(Debug, Clone, Copy)]
pub struct TimingModel {
    /// Enable received → its Run status queued (measured 1.36-5.23 ms; 10.3 ms
    /// when the Enable's own reply was lost and the next Run frame answered).
    pub enable_to_run: (Duration, Duration),
    /// Type-0 request → MCU UID reply (measured 0.13-0.47 ms).
    pub identity_reply: (Duration, Duration),
    /// Type-24 report period while streaming (measured 7.4-12.6 ms around 10 ms).
    pub report_period: (Duration, Duration),
    /// SetZero received → blackout start (measured last frame before it
    /// 511-614 ms; the true start is up to one report period later).
    pub set_zero_blackout_start: (Duration, Duration),
    /// Blackout length (measured 45.4-60.7 ms).
    pub set_zero_blackout_length: (Duration, Duration),
}

impl TimingModel {
    fn admits(&self, drive: &Drive) -> bool {
        let within =
            |value: Duration, (low, high): (Duration, Duration)| (low..=high).contains(&value);
        within(drive.enable_reply_delay, self.enable_to_run)
            && within(drive.identity_reply_delay, self.identity_reply)
            && within(drive.report_period, self.report_period)
            && within(drive.set_zero_blackout.0, self.set_zero_blackout_start)
            && within(drive.set_zero_blackout.1, self.set_zero_blackout_length)
    }
}

pub const TIMING_MODEL: TimingModel = TimingModel {
    enable_to_run: (Duration::ZERO, Duration::from_millis(11)),
    identity_reply: (Duration::ZERO, Duration::from_millis(1)),
    report_period: (Duration::from_millis(7), Duration::from_millis(13)),
    set_zero_blackout_start: (Duration::from_millis(500), Duration::from_millis(625)),
    set_zero_blackout_length: (Duration::from_millis(40), Duration::from_millis(65)),
};

/// Earliest start and latest end, after receiving a SetZero, of any blackout
/// [`TIMING_MODEL`] admits.
pub const SET_ZERO_BLACKOUT: (Duration, Duration) =
    (Duration::from_millis(500), Duration::from_millis(690));

/// The worst measured blackout (2026-10-03 15:34:08, right_elbow_pitch: last
/// report 613.8 ms after its SetZero, silent for 53.0 ms) at the latest
/// modeled start and length: (start, length).
pub const LATEST_SET_ZERO_BLACKOUT: (Duration, Duration) = (
    TIMING_MODEL.set_zero_blackout_start.1,
    TIMING_MODEL.set_zero_blackout_length.1,
);

/// One emulated drive at one configured address.
#[derive(Debug, Clone)]
pub struct Drive {
    pub joint: String,
    pub device_id: u8,
    pub motor_type: MotorType,
    /// `direction * gear_ratio`: motor = joint * scale.
    pub scale: f64,
    pub uid: [u8; 8],
    /// Absolute encoder, motor space.
    pub raw_motor_rad: f64,
    /// Volatile firmware zero, motor space; reported position is raw - offset.
    pub zero_offset_motor_rad: f64,
    pub enabled: bool,
    pub reporting: bool,
    /// A rebooting drive receives and answers nothing until this instant.
    pub silent_until: Option<Instant>,
    /// When this drive last received a SetZero (starts its blackout).
    pub set_zero_at: Option<Instant>,
    /// Blackout after receiving a SetZero: (start, length), within [`TIMING_MODEL`].
    pub set_zero_blackout: (Duration, Duration),
    /// Enable received → Run status queued, within [`TIMING_MODEL`].
    pub enable_reply_delay: Duration,
    /// Type-0 request received → UID reply queued, within [`TIMING_MODEL`].
    pub identity_reply_delay: Duration,
    /// Minimum interval between periodic type-24 reports, within [`TIMING_MODEL`].
    pub report_period: Duration,
    last_report_at: Option<Instant>,
    /// Latest instant a reply of this drive is scheduled for (keeps reply order).
    reply_ready_at: Option<Instant>,
    // Fault knobs.
    pub answer_identity: bool,
    pub drop_set_zero_ack: bool,
    /// Hold the Enable status reply and deliver it only after SetZero (stale ack).
    pub hold_enable_reply_until_set_zero: bool,
    held_reply: Option<CanFrame>,
    /// Added to every mechPos readback, joint space.
    pub readback_offset_joint_rad: f64,
    pub readback_status: u8,
    /// Reboot (lose zero, Reset, reporting off) right after queuing the SetZero
    /// ack, then stay silent for the given duration.
    pub reboot_after_ack: Option<Duration>,
    /// Communication types whose transmissions to this address fail.
    pub fail_writes: Vec<u8>,
    /// Communication types that start failing once a mechPos readback was answered.
    pub fail_writes_after_readback: Vec<u8>,
    /// Acknowledge Enable without entering Run (the drive stays in Reset).
    pub ignore_enable: bool,
    /// While reporting, a periodic type-24 report the drive built before it
    /// acted on an Enable follows that Enable on the wire: it still carries the
    /// pre-Enable mode and precedes the Enable reply (Enable-to-Run reply
    /// latency is 1.4-5.2 ms on the bench against a 10 ms report period).
    pub stale_report_after_enable: bool,
}

impl Drive {
    pub fn position_motor_rad(&self) -> f64 {
        self.raw_motor_rad - self.zero_offset_motor_rad
    }

    pub fn position_joint_rad(&self) -> f64 {
        self.position_motor_rad() / self.scale
    }

    fn mode(&self) -> u32 {
        if self.enabled {
            RUN_MODE
        } else {
            RESET_MODE
        }
    }

    fn responsive(&self, now: Instant) -> bool {
        self.silent_until.is_none_or(|until| now >= until) && !self.in_set_zero_blackout(now)
    }

    /// Inside the post-SetZero blackout: no reception, no transmission.
    pub fn in_set_zero_blackout(&self, now: Instant) -> bool {
        let (start, length) = self.set_zero_blackout;
        self.set_zero_at.is_some_and(|at| {
            let since = now.saturating_duration_since(at);
            since >= start && since <= start + length
        })
    }

    /// Type-2 (or type-24) status in the drive's current mode.
    fn status(&self, comm_type: CommunicationType) -> CanFrame {
        let ranges = MitRanges::for_motor_type(self.motor_type);
        let position_scale = f64::from(ranges.position_scale);
        let position = ((self.position_motor_rad() / position_scale + 1.0) * 32767.0)
            .round()
            .clamp(0.0, f64::from(u16::MAX)) as u16;
        let mut data = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x00, 0xc8];
        data[0..2].copy_from_slice(&position.to_be_bytes());
        CanFrame {
            id: (u32::from(comm_type.as_u8()) << 24)
                | (self.mode() << 22)
                | (u32::from(self.device_id) << 8)
                | u32::from(DEFAULT_HOST_ID),
            data,
            extended: true,
        }
    }

    /// Type-0 reply: `(0 << 24) | (device << 8) | 0xFE`, payload = MCU UID.
    fn identity_reply(&self) -> CanFrame {
        CanFrame {
            id: pack_ext_id(
                CommunicationType::GetDeviceId.as_u8(),
                u16::from(self.device_id),
                robstride::identity::DEVICE_ID_REPLY_MARKER,
            ),
            data: self.uid,
            extended: true,
        }
    }

    /// Type-17 reply: `(17 << 24) | (status << 16) | (device << 8) | host`.
    fn read_reply(&self, index: u16, status: u8, value: [u8; 4]) -> CanFrame {
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&index.to_le_bytes());
        data[4..8].copy_from_slice(&value);
        CanFrame {
            id: pack_ext_id(
                CommunicationType::ReadParameter.as_u8(),
                (u16::from(status) << 8) | u16::from(self.device_id),
                DEFAULT_HOST_ID,
            ),
            data,
            extended: true,
        }
    }

    /// Power cycle: the volatile zero and enable/reporting state are lost.
    pub fn reboot(&mut self, silent_until: Option<Instant>) {
        self.zero_offset_motor_rad = 0.0;
        self.enabled = false;
        self.reporting = false;
        self.held_reply = None;
        self.silent_until = silent_until;
        self.set_zero_at = None;
        self.reply_ready_at = None;
    }
}

/// Every emulated drive plus the shared wire queues.
#[derive(Debug, Default)]
pub struct Firmware {
    pub drives: Vec<Drive>,
    rx: VecDeque<CanFrame>,
    /// Frames the transport accepted, in order.
    pub tx: Vec<CanFrame>,
    /// When each `tx` frame was accepted.
    pub tx_at: Vec<Instant>,
    /// Frames whose transmission was refused by an injected failure.
    pub failed_tx: Vec<CanFrame>,
    /// Hold the next host Enable and every later write in the kernel/controller
    /// queue: accepted, but not on the wire until [`Self::release_tx`].
    pub hold_tx_from_enable: bool,
    held_tx: Option<VecDeque<CanFrame>>,
    /// Communication types whose echo the host never reads (lost on RX).
    pub lost_echoes: Vec<u8>,
    /// Drive replies with a modeled latency, in scheduling order.
    scheduled: Vec<(Instant, CanFrame)>,
    /// Every receive fails once the host has transmitted an Enable (a bus
    /// read error mid-reference, after the target was armed).
    pub fail_rx_after_enable: bool,
    rx_failed: bool,
}

pub type SharedFirmware = Rc<RefCell<Firmware>>;

impl Firmware {
    /// One drive per configured motor at `initial[joint]` joint radians (0 if absent).
    pub fn from_motors(motors: &[MotorEntry], initial: &[(&str, f64)]) -> SharedFirmware {
        let drives = motors
            .iter()
            .map(|motor| {
                let scale = f64::from(motor.direction) * motor.gear_ratio;
                let joint = initial
                    .iter()
                    .find(|(name, _)| *name == motor.joint)
                    .map_or(0.0, |(_, position)| *position);
                Drive {
                    joint: motor.joint.clone(),
                    device_id: motor.device_id,
                    motor_type: motor.motor_type,
                    scale,
                    uid: [
                        0x45,
                        0x7b,
                        0x30,
                        0x02,
                        0x0c,
                        0x32,
                        0x38,
                        0x10 | motor.device_id,
                    ],
                    raw_motor_rad: joint * scale,
                    zero_offset_motor_rad: 0.0,
                    enabled: false,
                    reporting: false,
                    silent_until: None,
                    set_zero_at: None,
                    set_zero_blackout: (Duration::from_millis(535), Duration::from_millis(55)),
                    enable_reply_delay: Duration::ZERO,
                    identity_reply_delay: Duration::ZERO,
                    report_period: Duration::from_millis(10),
                    last_report_at: None,
                    reply_ready_at: None,
                    answer_identity: true,
                    drop_set_zero_ack: false,
                    hold_enable_reply_until_set_zero: false,
                    held_reply: None,
                    readback_offset_joint_rad: 0.0,
                    readback_status: 0,
                    reboot_after_ack: None,
                    fail_writes: Vec::new(),
                    fail_writes_after_readback: Vec::new(),
                    ignore_enable: false,
                    stale_report_after_enable: false,
                }
            })
            .collect();
        Rc::new(RefCell::new(Self {
            drives,
            ..Self::default()
        }))
    }

    pub fn drive(&self, joint: &str) -> &Drive {
        self.drives
            .iter()
            .find(|drive| drive.joint == joint)
            .expect("emulated joint")
    }

    pub fn drive_mut(&mut self, joint: &str) -> &mut Drive {
        self.drives
            .iter_mut()
            .find(|drive| drive.joint == joint)
            .expect("emulated joint")
    }

    /// Periodic type-24 reports from every live drive whose reporting is on
    /// and whose report period has elapsed since its last periodic report.
    pub fn emit_reports(&mut self) {
        let now = Instant::now();
        let mut frames = Vec::new();
        for drive in &mut self.drives {
            assert!(TIMING_MODEL.admits(drive), "{}: timing knobs", drive.joint);
            let due = drive
                .last_report_at
                .is_none_or(|at| now.saturating_duration_since(at) >= drive.report_period);
            if drive.reporting && drive.responsive(now) && due {
                drive.last_report_at = Some(now);
                frames.push(drive.status(CommunicationType::ActiveReporting));
            }
        }
        self.rx.extend(frames);
    }

    /// Move scheduled replies whose latency has passed to the receive queue.
    fn release_due(&mut self) {
        if self.scheduled.is_empty() {
            return;
        }
        let now = Instant::now();
        let (mut due, pending): (Vec<_>, Vec<_>) =
            self.scheduled.drain(..).partition(|(at, _)| *at <= now);
        self.scheduled = pending;
        // Stable: one drive's replies keep their order at equal instants.
        due.sort_by_key(|(at, _)| *at);
        self.rx.extend(due.into_iter().map(|(_, frame)| frame));
    }

    /// One type-24 report from `joint` in its current drive mode, unless the
    /// drive is in its post-SetZero blackout.
    pub fn emit_report(&mut self, joint: &str) {
        let drive = self.drive(joint);
        if drive.in_set_zero_blackout(Instant::now()) {
            return;
        }
        let frame = drive.status(CommunicationType::ActiveReporting);
        self.rx.push_back(frame);
    }

    /// Deliver `frame` to the host receive queue as is (e.g. a stale echo).
    pub fn inject_rx(&mut self, frame: CanFrame) {
        self.rx.push_back(frame);
    }

    /// Put every held write on the wire, in write order.
    pub fn release_tx(&mut self) {
        for frame in self.held_tx.take().unwrap_or_default() {
            self.on_wire(&frame);
        }
    }

    /// Accepted frames of `comm_type` addressed to `device_id` (outbound low byte).
    pub fn sent(&self, comm_type: CommunicationType, device_id: u8) -> usize {
        self.tx
            .iter()
            .filter(|frame| is_outbound(frame, comm_type, Some(device_id)))
            .count()
    }

    /// Accepted frames of `comm_type` to any address.
    pub fn sent_any(&self, comm_type: CommunicationType) -> usize {
        self.tx
            .iter()
            .filter(|frame| is_outbound(frame, comm_type, None))
            .count()
    }

    pub fn clear_trace(&mut self) {
        self.tx.clear();
        self.tx_at.clear();
        self.failed_tx.clear();
    }

    fn transmit(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        let comm_type = ((frame.id >> 24) & 0x1f) as u8;
        let device_id = (frame.id & 0xff) as u8;
        if self
            .drives
            .iter()
            .any(|drive| drive.device_id == device_id && drive.fail_writes.contains(&comm_type))
        {
            self.failed_tx.push(frame.clone());
            return Err(BusError::Send {
                message: format!("injected write failure to device {device_id}"),
            });
        }
        self.tx.push(frame.clone());
        self.tx_at.push(Instant::now());
        if self.fail_rx_after_enable && comm_type == CommunicationType::Enable.as_u8() {
            self.rx_failed = true;
        }
        if self.hold_tx_from_enable && comm_type == CommunicationType::Enable.as_u8() {
            self.hold_tx_from_enable = false;
            self.held_tx = Some(VecDeque::new());
        }
        match &mut self.held_tx {
            Some(held) => held.push_back(frame.clone()),
            None => self.on_wire(frame),
        }
        Ok(())
    }

    /// The frame is transmitted: the host's echo precedes any drive reaction.
    fn on_wire(&mut self, frame: &CanFrame) {
        let comm_type = ((frame.id >> 24) & 0x1f) as u8;
        let device_id = (frame.id & 0xff) as u8;
        let now = Instant::now();
        if !self.lost_echoes.contains(&comm_type) {
            self.rx.push_back(frame.clone());
        }
        let Some(drive) = self
            .drives
            .iter_mut()
            .find(|drive| drive.device_id == device_id)
        else {
            return;
        };
        if !drive.responsive(now) {
            return;
        }
        assert!(TIMING_MODEL.admits(drive), "{}: timing knobs", drive.joint);
        drive.silent_until = None;
        // (latency, frame); a zero latency reply follows the echo directly.
        let mut replies = Vec::new();
        match CommunicationType::from_u8(comm_type) {
            Some(CommunicationType::GetDeviceId) => {
                if drive.answer_identity {
                    replies.push((drive.identity_reply_delay, drive.identity_reply()));
                }
            }
            Some(CommunicationType::OperationControl) => {
                replies.push((
                    Duration::ZERO,
                    drive.status(CommunicationType::OperationStatus),
                ));
            }
            Some(CommunicationType::Enable) => {
                if drive.stale_report_after_enable && drive.reporting {
                    replies.push((
                        Duration::ZERO,
                        drive.status(CommunicationType::ActiveReporting),
                    ));
                }
                drive.enabled = !drive.ignore_enable;
                let reply = drive.status(CommunicationType::OperationStatus);
                if drive.hold_enable_reply_until_set_zero {
                    drive.held_reply = Some(reply);
                } else {
                    replies.push((drive.enable_reply_delay, reply));
                }
            }
            Some(CommunicationType::Disable) => {
                drive.enabled = false;
                replies.push((
                    Duration::ZERO,
                    drive.status(CommunicationType::OperationStatus),
                ));
            }
            Some(CommunicationType::SetZeroPosition) => {
                drive.zero_offset_motor_rad = drive.raw_motor_rad;
                drive.set_zero_at = Some(now);
                if let Some(stale) = drive.held_reply.take() {
                    replies.push((Duration::ZERO, stale));
                }
                if !drive.drop_set_zero_ack {
                    replies.push((
                        Duration::ZERO,
                        drive.status(CommunicationType::OperationStatus),
                    ));
                }
                if let Some(silence) = drive.reboot_after_ack {
                    drive.reboot(Some(now + silence));
                }
            }
            Some(CommunicationType::ReadParameter) => {
                let index = u16::from_le_bytes([frame.data[0], frame.data[1]]);
                if index == ParameterId::MechPos.as_u16() {
                    let joint = drive.position_joint_rad() + drive.readback_offset_joint_rad;
                    let value = ((joint * drive.scale) as f32).to_le_bytes();
                    let reply = drive.read_reply(index, drive.readback_status, value);
                    replies.push((Duration::ZERO, reply));
                    let armed = std::mem::take(&mut drive.fail_writes_after_readback);
                    drive.fail_writes.extend(armed);
                } else if index == ParameterId::RunMode.as_u16() {
                    replies.push((Duration::ZERO, drive.read_reply(index, 0, [0; 4])));
                } else {
                    replies.push((Duration::ZERO, drive.read_reply(index, 1, [0; 4])));
                }
            }
            Some(CommunicationType::ActiveReporting) => {
                // Bench candump: every type-24 write is answered by a type-2 status.
                drive.reporting = frame.data[6] == 0x01;
                replies.push((
                    Duration::ZERO,
                    drive.status(CommunicationType::OperationStatus),
                ));
            }
            _ => {}
        }
        // A drive answers in receive order: no reply overtakes an earlier one.
        let mut ready = drive.reply_ready_at.filter(|at| *at > now);
        let mut immediate = Vec::new();
        let mut later = Vec::new();
        for (latency, frame) in replies {
            match ready {
                None if latency.is_zero() => immediate.push(frame),
                Some(at) if at >= now + latency => later.push((at, frame)),
                _ => {
                    ready = Some(now + latency);
                    later.push((now + latency, frame));
                }
            }
        }
        drive.reply_ready_at = ready;
        self.rx.extend(immediate);
        self.scheduled.extend(later);
    }
}

fn is_outbound(frame: &CanFrame, comm_type: CommunicationType, device_id: Option<u8>) -> bool {
    (frame.id >> 24) & 0x1f == u32::from(comm_type.as_u8())
        && ((frame.id >> 8) & 0xff) as u8 == DEFAULT_HOST_ID
        && device_id.is_none_or(|device| (frame.id & 0xff) as u8 == device)
}

/// Supervisor-owned transport handle; the test keeps the other `Rc`.
pub struct FirmwareBus(pub SharedFirmware);

impl CanBus for FirmwareBus {
    fn send_frame(&mut self, frame: &CanFrame) -> Result<(), BusError> {
        self.0.borrow_mut().transmit(frame)
    }

    fn recv_one_nonblocking(&mut self) -> Result<ReceiveAttempt, BusError> {
        let mut firmware = self.0.borrow_mut();
        if firmware.rx_failed {
            return Err(BusError::Driver("injected receive failure".into()));
        }
        firmware.release_due();
        Ok(match firmware.rx.pop_front() {
            Some(frame) => ReceiveAttempt::Frame(TimedCanFrame {
                received_at: Instant::now(),
                received: ReceivedCanFrame::full_data(None, frame),
            }),
            None => ReceiveAttempt::Idle,
        })
    }

    /// Like the SocketCAN backend: own writes are read back in wire order.
    fn echoes_transmissions(&self) -> bool {
        true
    }
}

impl MotorBus for FirmwareBus {}
