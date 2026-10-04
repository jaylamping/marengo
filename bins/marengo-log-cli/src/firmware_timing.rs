//! `firmware-timing`: Robstride firmware timing measured from passive candump
//! captures (`candump -L` or `candump -t z|a` ASCII lines).
//!
//! Frames are split into host commands (host id [`DEFAULT_HOST_ID`] above the
//! target) and drive frames (device above the host) by
//! [`robstride::classify_frame`]. Times are candump timestamps; a host frame's
//! timestamp is its SocketCAN echo, i.e. when it reached the wire. Every
//! measurement is per drive (`<iface>/<device_id>`) and per capture file: no
//! pairing crosses a file boundary. Definitions (all latencies in ms):
//!
//! - `enable_to_run_ms`: host Enable → first drive status (type 2) or report
//!   (type 24) in Run mode. The Enable window closes on that Run frame, on a
//!   Disable or another Enable to the drive (`enable_never_run` += 1), or
//!   [`ENABLE_RUN_WINDOW_S`] without Run (`enable_never_run` += 1).
//! - `reset_after_enable`: drive status/report frames in Reset mode inside an
//!   open Enable window (after the Enable, before its first Run frame).
//! - `set_zero_silence_start_ms` / `set_zero_silence_ms`: for each host SetZero,
//!   the longest gap between consecutive drive frames within
//!   [`SET_ZERO_WINDOW_S`] after SetZero and at least [`SILENCE_MIN_S`] long.
//!   A gap is observable only if periodic reports were running (and neither
//!   an Off nor another reporting write intervened), or both boundary status
//!   frames answer host requests with requests throughout the gap no more than
//!   [`SILENCE_MIN_S`] apart. A delayed report after an Off cannot re-arm
//!   streaming evidence. Gaps in reply-only traffic with sparse solicits
//!   cannot establish a blackout and are counted as
//!   `set_zero_silence_unobservable`, not as samples. Start = first frame of
//!   the gap − SetZero; silence = gap length (the true silence lies inside it).
//!   A SetZero with no observable qualifying gap is unobservable.
//! - `identity_reply_ms`: host type-0 request → type-0 reply from the drive,
//!   paired with the latest outstanding request. A request superseded by
//!   another, or unanswered after [`REPLY_TIMEOUT_S`], is `identity_unanswered`.
//! - `param_read_reply_ms`: host type-17 read → type-17 reply with the same
//!   index, same pairing rule (no unanswered count).
//! - `report_period_ms`: interval between consecutive type-24 reports from the
//!   drive, excluding intervals spanning a host type-24 write or SetZero to it,
//!   and intervals starting within [`SET_ZERO_WINDOW_S`] after a SetZero.
//! - `report_off_to_last_ms`: host type-24 Off, sent while the drive's stream
//!   was running (a report in the last [`STREAM_RUNNING_S`]) → last report
//!   received after it (0 if none), until the next host type-24 write or
//!   [`OFF_WINDOW_S`]. A report read less than [`ECHO_LAG_S`] before the next
//!   write's echo belongs to that write: the mcp251x echo is raised on
//!   TX-complete and can be read after the drive's answer to the same frame.
//! - `mit_reply_ms` / `mit_unanswered`: host MIT command → its type-2 reply.
//!   Every host frame of types 1, 3, 4, 6, 18 and 24 solicits exactly one type-2
//!   status (bench), so replies are matched FIFO per drive; a pending command
//!   older than [`STATUS_REPLY_WINDOW_S`] when a status arrives (or at capture
//!   end) is dropped, MIT ones as `mit_unanswered`. The window is one 200 Hz
//!   control period: commands lost in a silence would otherwise shift every
//!   later pairing by one period.
//! - `disable_to_reset_ms`: host Disable → first drive status/report in Reset
//!   mode within [`REPLY_TIMEOUT_S`] (an Enable to the drive cancels it).
//! - `non_neutral_mit`: MIT commands whose raw kp or kd is nonzero or whose
//!   torque feedforward is more than one quantization step from zero. `first_s`
//!   / `last_s` are on the concatenated timeline (file k starts at the sum of
//!   the spans of files 0..k), so for one file they are offsets into it.
//! - `bus`: per interface, the most frames in any 10 ms window, and the number
//!   of consecutive-frame gaps over 5 ms (summed over interfaces).
//!
//! Stats are nearest-rank percentiles over all files; `n = 0` → all zeros.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use marengo_candump::{Candump, Frame, TimestampMode};
use robstride::{classify_frame, DriveFrame, DriveMode, HostCommand, WireFrame, DEFAULT_HOST_ID};
use serde::{Deserialize, Serialize};

/// An Enable not answered in Run within this is `enable_never_run`.
pub const ENABLE_RUN_WINDOW_S: f64 = 1.0;
/// SetZero observation window for the post-SetZero silence.
pub const SET_ZERO_WINDOW_S: f64 = 1.5;
/// Two missed 10 ms reports: shorter gaps are not silence.
pub const SILENCE_MIN_S: f64 = 0.020;
/// A reply later than this does not answer the request.
pub const REPLY_TIMEOUT_S: f64 = 0.100;
/// A type-2 status later than this does not answer the command (one 200 Hz
/// control period; the next command is due by then).
pub const STATUS_REPLY_WINDOW_S: f64 = 0.005;
/// A stream counts as running when a report arrived this recently.
pub const STREAM_RUNNING_S: f64 = 0.030;
/// How long reports after a type-24 Off are attributed to it.
pub const OFF_WINDOW_S: f64 = 0.500;
/// The host echo of a frame can be read up to this long after the drive's
/// answer to it (soak 2026-10-03: status 70 µs and report 28 µs before the
/// echo of the type-24 On they answer).
pub const ECHO_LAG_S: f64 = 0.0005;
const BUS_WINDOW_S: f64 = 0.010;
const BUS_GAP_S: f64 = 0.005;
/// Per-frame lines in the human-readable `kernel error frames:` section.
const KERNEL_ERROR_TEXT_CAP: usize = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub n: usize,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

impl Stats {
    fn from_ms(mut values: Vec<f64>) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        values.sort_by(f64::total_cmp);
        let rank = |p: f64| {
            let index = (p * values.len() as f64).ceil() as usize;
            round_us(values[index.clamp(1, values.len()) - 1])
        };
        Self {
            n: values.len(),
            min: round_us(values[0]),
            p50: rank(0.50),
            p95: rank(0.95),
            max: round_us(values[values.len() - 1]),
        }
    }
}

/// Milliseconds rounded to the microsecond resolution of candump.
fn round_us(ms: f64) -> f64 {
    (ms * 1000.0).round() / 1000.0
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DriveTiming {
    pub enable_to_run_ms: Stats,
    pub enable_never_run: u64,
    pub reset_after_enable: u64,
    pub set_zero_silence_start_ms: Stats,
    pub set_zero_silence_ms: Stats,
    pub set_zero_silence_unobservable: u64,
    pub identity_reply_ms: Stats,
    pub identity_unanswered: u64,
    pub param_read_reply_ms: Stats,
    pub report_period_ms: Stats,
    pub report_off_to_last_ms: Stats,
    pub mit_reply_ms: Stats,
    pub mit_unanswered: u64,
    pub disable_to_reset_ms: Stats,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NonNeutralMit {
    pub count: u64,
    pub first_s: Option<f64>,
    pub last_s: Option<f64>,
}

/// One kernel CAN error frame from the capture (`2000xxxx` id,
/// `CAN_ERR_FLAG` set): bus-state evidence, never a Robstride frame.
/// Error classes live in the id's low 16 bits and controller flags
/// (`CAN_ERR_CRTL_*`) in data[1]; both follow linux/can/error.h.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelErrorFrame {
    /// Seconds on the concatenated timeline (file offsets accumulate).
    pub t_s: f64,
    pub interface: String,
    /// Canonical hex without `0x`, e.g. `20000004`.
    pub can_id: String,
    pub classes: Vec<String>,
    /// Decoded `CAN_ERR_CRTL` data[1] flags; empty unless class `ctrl`.
    pub ctrl: Vec<String>,
    pub bus_off: bool,
    pub restarted: bool,
}

/// `CAN_ERR_*` classes (linux/can/error.h) in snake_case.
const ERROR_CLASSES: &[(u32, &str)] = &[
    (0x01, "tx_timeout"),
    (0x02, "lost_arbitration"),
    (0x04, "ctrl"),
    (0x08, "prot"),
    (0x10, "trx"),
    (0x20, "ack"),
    (0x40, "busoff"),
    (0x80, "bus_error"),
    (0x100, "restarted"),
];

/// `CAN_ERR_CRTL_*` data[1] flags (linux/can/error.h) in snake_case.
const CTRL_FLAGS: &[(u8, &str)] = &[
    (0x01, "rx_overflow"),
    (0x02, "tx_overflow"),
    (0x04, "rx_warning"),
    (0x08, "tx_warning"),
    (0x10, "rx_passive"),
    (0x20, "tx_passive"),
    (0x40, "active"),
];

fn decode_kernel_error(can_id: u32, data: &[u8]) -> KernelErrorDecode {
    let class = can_id & 0xFFFF;
    let mut classes: Vec<String> = ERROR_CLASSES
        .iter()
        .filter(|(bit, _)| class & *bit != 0)
        .map(|(_, name)| (*name).to_string())
        .collect();
    if class & !0x1FF != 0 {
        classes.push(format!("unknown({:#x})", class & !0x1FF));
    }
    let mut ctrl = Vec::new();
    if class & 0x04 != 0 {
        let flags = data.get(1).copied().unwrap_or(0);
        ctrl.extend(
            CTRL_FLAGS
                .iter()
                .filter(|(bit, _)| flags & *bit != 0)
                .map(|(_, name)| (*name).to_string()),
        );
    }
    KernelErrorDecode {
        classes,
        ctrl,
        bus_off: class & 0x40 != 0,
        restarted: class & 0x100 != 0,
    }
}

struct KernelErrorDecode {
    classes: Vec<String>,
    ctrl: Vec<String>,
    bus_off: bool,
    restarted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BusTiming {
    pub max_frames_per_10ms: u64,
    pub gaps_over_5ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FirmwareTiming {
    pub files: Vec<String>,
    pub frames: u64,
    pub span_s: f64,
    pub drives: BTreeMap<String, DriveTiming>,
    pub non_neutral_mit: NonNeutralMit,
    pub bus: BusTiming,
    /// Kernel error frames in capture order; diagnostic only, never drive
    /// traffic and never a PASS input. Defaults empty for older JSON.
    #[serde(default)]
    pub kernel_error_frames: Vec<KernelErrorFrame>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Candump(#[from] marengo_candump::Error),
    #[error("no capture files given")]
    NoFiles,
}

/// Raw samples (ms) per drive, accumulated across files.
#[derive(Debug, Default)]
struct DriveSamples {
    enable_to_run: Vec<f64>,
    enable_never_run: u64,
    reset_after_enable: u64,
    set_zero_silence_start: Vec<f64>,
    set_zero_silence: Vec<f64>,
    set_zero_silence_unobservable: u64,
    identity_reply: Vec<f64>,
    identity_unanswered: u64,
    param_read_reply: Vec<f64>,
    report_period: Vec<f64>,
    report_off_to_last: Vec<f64>,
    mit_reply: Vec<f64>,
    mit_unanswered: u64,
    disable_to_reset: Vec<f64>,
}

impl DriveSamples {
    fn finish(self) -> DriveTiming {
        DriveTiming {
            enable_to_run_ms: Stats::from_ms(self.enable_to_run),
            enable_never_run: self.enable_never_run,
            reset_after_enable: self.reset_after_enable,
            set_zero_silence_start_ms: Stats::from_ms(self.set_zero_silence_start),
            set_zero_silence_ms: Stats::from_ms(self.set_zero_silence),
            set_zero_silence_unobservable: self.set_zero_silence_unobservable,
            identity_reply_ms: Stats::from_ms(self.identity_reply),
            identity_unanswered: self.identity_unanswered,
            param_read_reply_ms: Stats::from_ms(self.param_read_reply),
            report_period_ms: Stats::from_ms(self.report_period),
            report_off_to_last_ms: Stats::from_ms(self.report_off_to_last),
            mit_reply_ms: Stats::from_ms(self.mit_reply),
            mit_unanswered: self.mit_unanswered,
            disable_to_reset_ms: Stats::from_ms(self.disable_to_reset),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EnableWindow {
    at: f64,
}

#[derive(Debug, Clone, Copy)]
struct SetZeroWindow {
    at: f64,
    /// Longest qualifying gap: (first frame, next frame).
    best: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy)]
struct OffWindow {
    at: f64,
    last_report: Option<f64>,
    previous_report: Option<f64>,
}

/// Per-drive pairing state inside one file.
#[derive(Debug, Default)]
struct DriveState {
    enable: Option<EnableWindow>,
    set_zero: Option<SetZeroWindow>,
    /// Latest SetZero, for report-period exclusion.
    last_set_zero: Option<f64>,
    last_frame: Option<f64>,
    /// Stream believed running at `last_frame` (report seen, no Off since).
    stream_on: bool,
    /// An explicit Off remains authoritative even if a delayed report arrives.
    reporting_off: bool,
    /// Whether the last drive frame answered a host status-soliciting frame.
    last_frame_answered: bool,
    /// Latest of `last_frame` and the host requests to the drive since then.
    obligation_at: Option<f64>,
    /// Since `last_frame`: a host request, a host type-24 write, and a stretch
    /// longer than [`SILENCE_MIN_S`] without frame or request.
    gap_requested: bool,
    gap_report_write: bool,
    gap_sparse: bool,
    identity_pending: Option<f64>,
    param_pending: Option<(u16, f64)>,
    /// FIFO of type-2-soliciting host frames: (time, is MIT).
    status_pending: VecDeque<(f64, bool)>,
    last_report: Option<f64>,
    report_chain_valid: bool,
    off: Option<OffWindow>,
    disable_pending: Option<f64>,
}

impl DriveState {
    fn close_enable_without_run(&mut self, samples: &mut DriveSamples) {
        if self.enable.take().is_some() {
            samples.enable_never_run += 1;
        }
    }

    fn close_set_zero(&mut self, samples: &mut DriveSamples) {
        if let Some(window) = self.set_zero.take() {
            if let Some((start, end)) = window.best {
                samples
                    .set_zero_silence_start
                    .push((start - window.at) * 1e3);
                samples.set_zero_silence.push((end - start) * 1e3);
            } else {
                samples.set_zero_silence_unobservable += 1;
            }
        }
    }

    /// `closing_write`: the host type-24 write that ends the window. A report
    /// read less than [`ECHO_LAG_S`] before that write's echo answers the write.
    fn close_off(&mut self, closing_write: Option<f64>, samples: &mut DriveSamples) {
        if let Some(off) = self.off.take() {
            let last = match (off.last_report, closing_write) {
                (Some(last), Some(write)) if write - last < ECHO_LAG_S => off.previous_report,
                (last, _) => last,
            };
            samples
                .report_off_to_last
                .push(last.map_or(0.0, |last| (last - off.at) * 1e3));
        }
    }

    /// Close every window whose deadline passed by `now`.
    fn expire(&mut self, now: f64, samples: &mut DriveSamples) {
        if self
            .enable
            .is_some_and(|window| now - window.at > ENABLE_RUN_WINDOW_S)
        {
            self.close_enable_without_run(samples);
        }
        if self
            .set_zero
            .is_some_and(|window| now - window.at > SET_ZERO_WINDOW_S)
        {
            self.close_set_zero(samples);
        }
        if self.off.is_some_and(|off| now - off.at > OFF_WINDOW_S) {
            self.close_off(None, samples);
        }
        if self
            .identity_pending
            .is_some_and(|at| now - at > REPLY_TIMEOUT_S)
        {
            self.identity_pending = None;
            samples.identity_unanswered += 1;
        }
        if self
            .param_pending
            .is_some_and(|(_, at)| now - at > REPLY_TIMEOUT_S)
        {
            self.param_pending = None;
        }
        if self
            .disable_pending
            .is_some_and(|at| now - at > REPLY_TIMEOUT_S)
        {
            self.disable_pending = None;
        }
        while let Some(&(at, mit)) = self.status_pending.front() {
            if now - at <= STATUS_REPLY_WINDOW_S {
                break;
            }
            self.status_pending.pop_front();
            if mit {
                samples.mit_unanswered += 1;
            }
        }
    }

    /// End of capture: windows that ran their full course close as timed out;
    /// windows the capture cut short record what they observed or nothing.
    fn finish_file(&mut self, end: f64, samples: &mut DriveSamples) {
        self.expire(end, samples);
        self.close_set_zero(samples);
        self.close_off(None, samples);
    }

    fn on_host(&mut self, t: f64, command: HostCommand, samples: &mut DriveSamples) {
        if let Some(previous) = self.obligation_at {
            self.gap_sparse |= t - previous > SILENCE_MIN_S;
        }
        self.obligation_at = Some(t);
        self.gap_requested = true;
        self.gap_report_write |= matches!(command, HostCommand::ActiveReporting { .. });
        let solicits_status = match command {
            HostCommand::GetDeviceId => {
                if self.identity_pending.replace(t).is_some() {
                    samples.identity_unanswered += 1;
                }
                None
            }
            HostCommand::ReadParameter { index } => {
                self.param_pending = Some((index, t));
                None
            }
            HostCommand::Mit(_) => Some(true),
            HostCommand::Enable => {
                self.close_enable_without_run(samples);
                self.enable = Some(EnableWindow { at: t });
                self.disable_pending = None;
                Some(false)
            }
            HostCommand::Disable => {
                self.close_enable_without_run(samples);
                self.disable_pending = Some(t);
                Some(false)
            }
            HostCommand::SetZero => {
                self.close_set_zero(samples);
                self.set_zero = Some(SetZeroWindow { at: t, best: None });
                self.last_set_zero = Some(t);
                self.report_chain_valid = false;
                Some(false)
            }
            HostCommand::WriteParameter { .. } => Some(false),
            HostCommand::ActiveReporting { on } => {
                self.close_off(Some(t), samples);
                self.report_chain_valid = false;
                self.reporting_off = !on;
                if !on {
                    let running = self
                        .last_report
                        .is_some_and(|last| t - last <= STREAM_RUNNING_S);
                    if running {
                        self.off = Some(OffWindow {
                            at: t,
                            last_report: None,
                            previous_report: None,
                        });
                    }
                    self.stream_on = false;
                }
                Some(false)
            }
        };
        if let Some(mit) = solicits_status {
            self.status_pending.push_back((t, mit));
        }
    }

    fn on_drive(&mut self, t: f64, frame: DriveFrame, samples: &mut DriveSamples) {
        let answered = matches!(frame, DriveFrame::Status { .. })
            && self
                .status_pending
                .front()
                .is_some_and(|(at, _)| t - at <= STATUS_REPLY_WINDOW_S);
        // Silence candidate: the gap that this frame ends.
        if let (Some(window), Some(previous)) = (self.set_zero.as_mut(), self.last_frame) {
            let gap = t - previous;
            let stream_owed = self.stream_on
                && !self.reporting_off
                && !self.gap_report_write
                && self
                    .last_report
                    .is_some_and(|last| previous - last <= STREAM_RUNNING_S)
                && matches!(frame, DriveFrame::Report { .. });
            let requests_owed = self.last_frame_answered
                && answered
                && self.gap_requested
                && !self.gap_report_write
                && !self.gap_sparse
                && self
                    .obligation_at
                    .is_some_and(|last| t - last <= SILENCE_MIN_S);
            if previous >= window.at
                && gap >= SILENCE_MIN_S
                && (stream_owed || requests_owed)
                && window.best.is_none_or(|(start, end)| gap > end - start)
            {
                window.best = Some((previous, t));
            }
        }
        self.last_frame = Some(t);
        self.last_frame_answered = answered;
        self.obligation_at = Some(t);
        self.gap_requested = false;
        self.gap_report_write = false;
        self.gap_sparse = false;
        match frame {
            DriveFrame::Status { mode, .. } | DriveFrame::Report { mode, .. } => {
                if let Some(window) = self.enable {
                    match mode {
                        DriveMode::Run => {
                            samples.enable_to_run.push((t - window.at) * 1e3);
                            self.enable = None;
                        }
                        DriveMode::Reset => samples.reset_after_enable += 1,
                        _ => {}
                    }
                }
                if mode == DriveMode::Reset {
                    if let Some(at) = self.disable_pending.take() {
                        samples.disable_to_reset.push((t - at) * 1e3);
                    }
                }
            }
            DriveFrame::Identity => {
                if let Some(at) = self.identity_pending.take() {
                    samples.identity_reply.push((t - at) * 1e3);
                }
            }
            DriveFrame::ParameterRead { index, .. } => {
                if let Some((pending, at)) = self.param_pending {
                    if pending == index {
                        samples.param_read_reply.push((t - at) * 1e3);
                        self.param_pending = None;
                    }
                }
            }
            DriveFrame::FaultReport => {}
        }

        match frame {
            DriveFrame::Status { .. } => {
                if let Some((at, mit)) = self.status_pending.pop_front() {
                    if mit {
                        samples.mit_reply.push((t - at) * 1e3);
                    }
                }
            }
            DriveFrame::Report { .. } => {
                if let Some(previous) = self.last_report {
                    let after_set_zero = self
                        .last_set_zero
                        .is_some_and(|at| previous - at <= SET_ZERO_WINDOW_S);
                    if self.report_chain_valid && !after_set_zero {
                        samples.report_period.push((t - previous) * 1e3);
                    }
                }
                self.last_report = Some(t);
                self.report_chain_valid = true;
                self.stream_on = !self.reporting_off;
                if let Some(off) = self.off.as_mut() {
                    off.previous_report = off.last_report;
                    off.last_report = Some(t);
                }
            }
            _ => {}
        }
    }
}

/// Sliding 10 ms window and gap count for one interface.
#[derive(Debug, Default)]
struct BusState {
    window: VecDeque<f64>,
    last: Option<f64>,
}

/// Streaming analyzer; feed frames file by file.
#[derive(Debug, Default)]
pub struct Analyzer {
    files: Vec<String>,
    frames: u64,
    span_s: f64,
    samples: BTreeMap<(String, u8), DriveSamples>,
    non_neutral: NonNeutralMit,
    bus: BusTiming,
    errors: Vec<KernelErrorFrame>,
}

/// Per-file pairing state; dropped at the file boundary.
#[derive(Default)]
struct FileState {
    drives: HashMap<(String, u8), DriveState>,
    bus: HashMap<String, BusState>,
    last_t: f64,
}

impl Analyzer {
    pub fn add_file(&mut self, path: &Path) -> Result<(), Error> {
        let mut file = FileState::default();
        let summary = Candump::plain().visit_path(path, TimestampMode::Delta, &mut |frame| {
            self.on_frame(&mut file, frame);
        })?;
        self.finish_file(file, path, summary.parsed_frames, summary.duration_s);
        Ok(())
    }

    /// Fixture entry point with the same parser.
    #[cfg(test)]
    pub fn add_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<(), Error> {
        let mut file = FileState::default();
        let summary = Candump::plain().visit_bytes(bytes, TimestampMode::Delta, &mut |frame| {
            self.on_frame(&mut file, frame);
        })?;
        self.finish_file(
            file,
            Path::new(name),
            summary.parsed_frames,
            summary.duration_s,
        );
        Ok(())
    }

    fn finish_file(&mut self, mut file: FileState, path: &Path, frames: u64, span_s: f64) {
        for (key, state) in &mut file.drives {
            let samples = self.samples.entry(key.clone()).or_default();
            state.finish_file(file.last_t, samples);
        }
        self.files.push(path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into(),
        ));
        self.frames += frames;
        self.span_s += span_s;
    }

    fn on_frame(&mut self, file: &mut FileState, frame: &Frame) {
        let t = frame.offset.as_secs_f64();
        file.last_t = t;
        // Kernel error frames are bus-state evidence, never traffic: record
        // them and return before bus, MIT-neutrality and pairing stats, so a
        // mid-gap overflow report neither splits the gap nor joins the load
        // window. (classify_frame already rejects them: unpack_ext_id
        // refuses ids above 0x1FFFFFFF.)
        if frame.can_id.is_error() {
            self.on_kernel_error(frame, t);
            return;
        }
        self.on_bus(file, &frame.interface, t);
        // A candump `CanId` keeps the value, not the frame format, and a type-0
        // reply id such as 0x000001FE is numerically standard-sized. Robstride
        // buses carry only extended frames, so every id is read as extended.
        let Some(wire) = classify_frame(DEFAULT_HOST_ID, frame.can_id.get(), &frame.data) else {
            return;
        };
        let device_id = match wire {
            WireFrame::Host { device_id, .. } | WireFrame::Drive { device_id, .. } => device_id,
        };
        let key = (frame.interface.clone(), device_id);
        let samples = self.samples.entry(key.clone()).or_default();
        let state = file.drives.entry(key).or_default();
        state.expire(t, samples);
        match wire {
            WireFrame::Host { command, .. } => {
                if let HostCommand::Mit(fields) = command {
                    if !fields.is_neutral() {
                        let timeline_s = self.span_s + t;
                        self.non_neutral.count += 1;
                        self.non_neutral.first_s.get_or_insert(timeline_s);
                        self.non_neutral.last_s = Some(timeline_s);
                    }
                }
                state.on_host(t, command, samples);
            }
            WireFrame::Drive { frame, .. } => state.on_drive(t, frame, samples),
        }
    }

    fn on_kernel_error(&mut self, frame: &Frame, t: f64) {
        let decoded = decode_kernel_error(frame.can_id.get(), &frame.data);
        self.errors.push(KernelErrorFrame {
            t_s: ((self.span_s + t) * 1e6).round() / 1e6,
            interface: frame.interface.clone(),
            can_id: frame.can_id.to_canonical_hex(),
            classes: decoded.classes,
            ctrl: decoded.ctrl,
            bus_off: decoded.bus_off,
            restarted: decoded.restarted,
        });
    }

    fn on_bus(&mut self, file: &mut FileState, interface: &str, t: f64) {
        let bus = match file.bus.get_mut(interface) {
            Some(bus) => bus,
            None => file.bus.entry(interface.to_string()).or_default(),
        };
        if bus.last.is_some_and(|last| t - last > BUS_GAP_S) {
            self.bus.gaps_over_5ms += 1;
        }
        bus.last = Some(t);
        while bus
            .window
            .front()
            .is_some_and(|first| t - first >= BUS_WINDOW_S)
        {
            bus.window.pop_front();
        }
        bus.window.push_back(t);
        self.bus.max_frames_per_10ms = self.bus.max_frames_per_10ms.max(bus.window.len() as u64);
    }

    pub fn finish(self) -> FirmwareTiming {
        FirmwareTiming {
            files: self.files,
            frames: self.frames,
            span_s: (self.span_s * 1e6).round() / 1e6,
            drives: self
                .samples
                .into_iter()
                .map(|((interface, device_id), samples)| {
                    (format!("{interface}/{device_id}"), samples.finish())
                })
                .collect(),
            non_neutral_mit: NonNeutralMit {
                first_s: self.non_neutral.first_s.map(|s| (s * 1e6).round() / 1e6),
                last_s: self.non_neutral.last_s.map(|s| (s * 1e6).round() / 1e6),
                ..self.non_neutral
            },
            bus: self.bus,
            kernel_error_frames: self.errors,
        }
    }
}

pub fn analyze(paths: &[PathBuf]) -> Result<FirmwareTiming, Error> {
    if paths.is_empty() {
        return Err(Error::NoFiles);
    }
    let mut analyzer = Analyzer::default();
    for path in paths {
        analyzer.add_file(path)?;
    }
    Ok(analyzer.finish())
}

fn push_stats(out: &mut String, name: &str, stats: &Stats) {
    if stats.n == 0 {
        out.push_str(&format!("  {name:<26} n=0\n"));
    } else {
        out.push_str(&format!(
            "  {name:<26} n={:<5} min={:<9} p50={:<9} p95={:<9} max={}\n",
            stats.n, stats.min, stats.p50, stats.p95, stats.max
        ));
    }
}

/// Human-readable report (the default, non-`--json` output).
pub fn format_text(timing: &FirmwareTiming) -> String {
    let mut out = format!(
        "files={} frames={} span_s={:.3}\n",
        timing.files.join(","),
        timing.frames,
        timing.span_s
    );
    for (drive, d) in &timing.drives {
        out.push_str(&format!("drive {drive}\n"));
        push_stats(&mut out, "enable_to_run_ms", &d.enable_to_run_ms);
        out.push_str(&format!(
            "  enable_never_run={} reset_after_enable={} identity_unanswered={} mit_unanswered={}\n",
            d.enable_never_run, d.reset_after_enable, d.identity_unanswered, d.mit_unanswered
        ));
        push_stats(
            &mut out,
            "set_zero_silence_start_ms",
            &d.set_zero_silence_start_ms,
        );
        push_stats(&mut out, "set_zero_silence_ms", &d.set_zero_silence_ms);
        out.push_str(&format!(
            "  set_zero_silence_unobservable={}\n",
            d.set_zero_silence_unobservable
        ));
        push_stats(&mut out, "identity_reply_ms", &d.identity_reply_ms);
        push_stats(&mut out, "param_read_reply_ms", &d.param_read_reply_ms);
        push_stats(&mut out, "report_period_ms", &d.report_period_ms);
        push_stats(&mut out, "report_off_to_last_ms", &d.report_off_to_last_ms);
        push_stats(&mut out, "mit_reply_ms", &d.mit_reply_ms);
        push_stats(&mut out, "disable_to_reset_ms", &d.disable_to_reset_ms);
    }
    let fmt_s = |s: Option<f64>| s.map_or_else(|| "-".to_string(), |s| format!("{s:.6}"));
    out.push_str(&format!(
        "non_neutral_mit count={} first_s={} last_s={}\n",
        timing.non_neutral_mit.count,
        fmt_s(timing.non_neutral_mit.first_s),
        fmt_s(timing.non_neutral_mit.last_s)
    ));
    out.push_str(&format!(
        "bus max_frames_per_10ms={} gaps_over_5ms={}\n",
        timing.bus.max_frames_per_10ms, timing.bus.gaps_over_5ms
    ));
    out.push_str(&format!(
        "kernel error frames: {}\n",
        timing.kernel_error_frames.len()
    ));
    for err in timing
        .kernel_error_frames
        .iter()
        .take(KERNEL_ERROR_TEXT_CAP)
    {
        let mut what = err.classes.join(",");
        if !err.ctrl.is_empty() {
            what.push_str(&format!("({})", err.ctrl.join(",")));
        }
        out.push_str(&format!(
            "  t={:.6} {} {} {}\n",
            err.t_s, err.interface, err.can_id, what
        ));
    }
    if timing.kernel_error_frames.len() > KERNEL_ERROR_TEXT_CAP {
        out.push_str(&format!(
            "  ... and {} more\n",
            timing.kernel_error_frames.len() - KERNEL_ERROR_TEXT_CAP
        ));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn run(lines: &str) -> FirmwareTiming {
        let mut analyzer = Analyzer::default();
        analyzer.add_bytes("fixture.log", lines.as_bytes()).unwrap();
        analyzer.finish()
    }

    fn drive<'a>(timing: &'a FirmwareTiming, key: &str) -> &'a DriveTiming {
        timing.drives.get(key).expect("drive present")
    }

    // cd-20261003T145133Z.log: Enable, its Run status 1.587 ms later, then a
    // SetZero acked in Run (literal lines).
    const ENABLE_SET_ZERO: &str = "\
(1791039093.499177) can0 0000FD01#0000000000000000
(1791039093.499315) can0 000001FE#457B30020C323817
(1791039093.508996) can0 0300FD01#0000000000000000
(1791039093.510583) can0 028001FD#7FFE7FEC7FA000E6
(1791039093.519172) can0 0600FD01#0100000000000000
(1791039093.519328) can0 028001FD#7FFF80537F9200E6
(1791039093.529268) can0 1100FD01#1970000000000000
(1791039093.529433) can0 110001FD#197000001D070639
(1791039093.534352) can0 1200FD01#0A70000000000000
(1791039093.534524) can0 028001FD#7FFF80587FEB00E6
(1791039093.534659) can0 017FFF01#7FFF7FFF00000000
(1791039093.534824) can0 028001FD#7FFF801A7FD200E6
(1791039093.534970) can0 0400FD01#0000000000000000
(1791039093.535261) can0 1200FD02#0A70000000000000
(1791039093.535415) can0 020002FD#7FFF80487FFF00F0
(1791039093.535545) can0 017FFF02#7FFF7FFF00000000
";

    #[test]
    fn pairs_host_commands_with_drive_replies() {
        let timing = run(ENABLE_SET_ZERO);
        let d1 = drive(&timing, "can0/1");
        assert_eq!(d1.enable_to_run_ms.n, 1);
        assert!((d1.enable_to_run_ms.max - 1.587).abs() < 1e-9);
        assert_eq!(d1.reset_after_enable, 0);
        assert_eq!(d1.enable_never_run, 0);
        assert!((d1.identity_reply_ms.max - 0.138).abs() < 1e-9);
        assert!((d1.param_read_reply_ms.max - 0.165).abs() < 1e-9);
        // FIFO: Enable, SetZero, write and MIT each solicit one type-2 status.
        assert_eq!(d1.mit_reply_ms.n, 1);
        assert!((d1.mit_reply_ms.max - 0.165).abs() < 1e-9);
        assert_eq!(timing.frames, 16);
        assert_eq!(timing.non_neutral_mit.count, 0);
        // Drive 2's MIT has no reply before the capture ends (not timed out yet).
        assert_eq!(drive(&timing, "can0/2").mit_unanswered, 0);
    }

    // cd-20261003T145133Z.log, drive 1: the type-24 stream (10 ms) stops
    // 538.364 ms after SetZero (t=93.519172) and resumes 54.978 ms later.
    const SET_ZERO_SILENCE: &str = "\
(1791039093.519172) can0 0600FD01#0100000000000000
(1791039093.519328) can0 028001FD#7FFF80537F9200E6
(1791039094.027522) can0 180001FD#7FFF7FF57FFF00E6
(1791039094.037529) can0 180001FD#7FFF80017FFF00E6
(1791039094.047530) can0 180001FD#7FFF80267FFF00E6
(1791039094.057536) can0 180001FD#7FFF7FEF7FFF00E6
(1791039094.112514) can0 180001FD#7FFF80167FFF00E6
(1791039094.122512) can0 180001FD#7FFF7FCA7FFF00E6
";

    #[test]
    fn measures_post_set_zero_silence_on_a_running_stream() {
        let timing = run(SET_ZERO_SILENCE);
        let d1 = drive(&timing, "can0/1");
        assert_eq!(d1.set_zero_silence_ms.n, 1);
        assert!((d1.set_zero_silence_start_ms.max - 538.364).abs() < 1e-6);
        assert!((d1.set_zero_silence_ms.max - 54.978).abs() < 1e-6);
        assert_eq!(d1.set_zero_silence_unobservable, 0);
        // Report intervals within the SetZero window are not periods.
        assert_eq!(d1.report_period_ms.n, 0);
    }

    #[test]
    fn reply_only_unanswered_solicit_is_unobservable_not_silence() {
        let timing = run("\
(0.980000) can0 180001FD#7FFF80537F9200E6
(0.990000) can0 180001FD#7FFF80537F9200E6
(0.990100) can0 1800FD01#0000000000000000
(0.990200) can0 180001FD#7FFF80537F9200E6
(1.000000) can0 0600FD01#0100000000000000
(1.000200) can0 020001FD#7FFF80537F9200E6
(1.490000) can0 0400FD01#0000000000000000
(1.490200) can0 020001FD#7FFF80537F9200E6
(1.550000) can0 0400FD01#0000000000000000
(1.610000) can0 0400FD01#0000000000000000
(1.610200) can0 020001FD#7FFF80537F9200E6
(2.510000) can0 0400FD02#0000000000000000
");
        let d1 = drive(&timing, "can0/1");
        assert_eq!(d1.set_zero_silence_ms.n, 0);
        assert_eq!(d1.set_zero_silence_unobservable, 1);
    }

    #[test]
    fn answered_solicits_bound_a_real_blackout_without_streaming() {
        let timing = run("\
(1.000000) can0 0600FD01#0100000000000000
(1.000200) can0 020001FD#7FFF80537F9200E6
(1.510000) can0 0400FD01#0000000000000000
(1.510200) can0 020001FD#7FFF80537F9200E6
(1.520000) can0 0400FD01#0000000000000000
(1.520200) can0 020001FD#7FFF80537F9200E6
(1.530000) can0 0400FD01#0000000000000000
(1.540000) can0 0400FD01#0000000000000000
(1.550000) can0 0400FD01#0000000000000000
(1.560000) can0 0400FD01#0000000000000000
(1.570000) can0 0400FD01#0000000000000000
(1.580000) can0 0400FD01#0000000000000000
(1.590000) can0 0400FD01#0000000000000000
(1.600000) can0 0400FD01#0000000000000000
(1.600200) can0 020001FD#7FFF80537F9200E6
(2.510000) can0 0400FD02#0000000000000000
");
        let d1 = drive(&timing, "can0/1");
        assert_eq!(d1.set_zero_silence_ms.n, 1);
        assert_eq!(d1.set_zero_silence_start_ms.max, 520.2);
        assert_eq!(d1.set_zero_silence_ms.max, 80.0);
        assert_eq!(d1.set_zero_silence_unobservable, 0);
    }

    // candump-20261003T153408Z.log (`candump -t z` ASCII), drive 1: Off while
    // its stream runs (no report follows), identity, Enable, SetZero, readback,
    // run-mode write and a neutral MIT command.
    const ASCII_REFERENCE: &str = " (000.558839)  can0  180001FD   [8]  7F FF 80 74 7F FF 00 F0
 (000.561152)  can0  1800FD01   [8]  01 02 03 04 05 06 00 00
 (000.561443)  can0  020001FD   [8]  7F FE 7F 36 7F FF 00 F0
 (000.566187)  can0  0000FD01   [8]  00 00 00 00 00 00 00 00
 (000.566354)  can0  000001FE   [8]  45 7B 30 02 0C 32 38 17
 (000.581499)  can0  0300FD01   [8]  00 00 00 00 00 00 00 00
 (000.582924)  can0  028001FD   [8]  7F FF 7F DA 80 31 00 F0
 (000.592101)  can0  0600FD01   [8]  01 00 00 00 00 00 00 00
 (000.592283)  can0  028001FD   [8]  7F FF 7F E1 80 44 00 F0
 (000.601539)  can0  1100FD01   [8]  19 70 00 00 00 00 00 00
 (000.601717)  can0  110001FD   [8]  19 70 00 00 AB AA 32 B8
 (000.608190)  can0  1200FD01   [8]  0A 70 00 00 00 00 00 00
 (000.608370)  can0  028001FD   [8]  7F FF 80 6D 80 27 00 F0
 (000.608506)  can0  017FFF01   [8]  7F FF 7F FF 00 00 00 00
 (000.608667)  can0  028001FD   [8]  7F FF 80 45 80 39 00 F0
";

    #[test]
    fn reads_ascii_captures_and_reporting_off() {
        let timing = run(ASCII_REFERENCE);
        let d1 = drive(&timing, "can0/1");
        assert_eq!(d1.report_off_to_last_ms.n, 1);
        assert_eq!(d1.report_off_to_last_ms.max, 0.0);
        assert!((d1.identity_reply_ms.max - 0.167).abs() < 1e-9);
        assert!((d1.enable_to_run_ms.max - 1.425).abs() < 1e-9);
        assert!((d1.param_read_reply_ms.max - 0.178).abs() < 1e-9);
        assert!((d1.mit_reply_ms.max - 0.161).abs() < 1e-9);
        assert_eq!(d1.mit_unanswered, 0);
    }

    // candump-20261003T141027Z.log (ASCII): drive 2 write, MIT, Disable.
    #[test]
    fn disable_is_answered_in_reset() {
        let timing = run(
            " (000.007122)  can0  1200FD02   [8]  0A 70 00 00 00 00 00 00
 (000.007293)  can0  020002FD   [8]  7F CD 7F DA 7F FF 00 F0
 (000.007418)  can0  017FFF02   [8]  7F FF 7F FF 00 00 00 00
 (000.007579)  can0  020002FD   [8]  7F CC 80 21 7F FF 00 F0
 (000.007717)  can0  0400FD02   [8]  00 00 00 00 00 00 00 00
 (000.007893)  can0  020002FD   [8]  7F CC 7F 45 7F FF 00 F0
",
        );
        let d2 = drive(&timing, "can0/2");
        assert!((d2.disable_to_reset_ms.max - 0.176).abs() < 1e-9);
        assert!((d2.mit_reply_ms.max - 0.161).abs() < 1e-9);
    }

    #[test]
    fn enable_answered_in_reset_counts_and_never_runs() {
        let timing = run("\
(1.000000) can0 0300FD03#0000000000000000
(1.000150) can0 180003FD#7FFF80677FFF00DC
(1.000300) can0 020003FD#7FFF80677FFF00DC
(1.010000) can0 0400FD03#0000000000000000
");
        let d3 = drive(&timing, "can0/3");
        assert_eq!(d3.reset_after_enable, 2);
        assert_eq!(d3.enable_never_run, 1);
        assert_eq!(d3.enable_to_run_ms.n, 0);
    }

    // Neutral MIT line from the captures, then the same frame with kp code 1
    // and with torque 0x8000 (one step from neutral 0x7FFF).
    #[test]
    fn non_neutral_mit_is_located_on_the_timeline() {
        let timing = run("\
(10.000000) can0 017FFF01#7FFF7FFF00000000
(10.500000) can0 017FFF01#7FFF7FFF00010000
(11.000000) can0 01800001#7FFF7FFF00000000
");
        assert_eq!(timing.non_neutral_mit.count, 1);
        assert_eq!(timing.non_neutral_mit.first_s, Some(0.5));
        assert_eq!(timing.non_neutral_mit.last_s, Some(0.5));
        // 0x8001 is two steps from neutral 0x7FFF.
        let timing = run("(0.0) can0 01800101#7FFF7FFF00000000\n");
        assert_eq!(timing.non_neutral_mit.count, 1);
    }

    #[test]
    fn identity_superseded_by_a_retry_is_unanswered() {
        let timing = run("\
(0.000000) can0 0000FD02#0000000000000000
(0.010000) can0 0000FD02#0000000000000000
(0.010200) can0 000002FE#785630020C343701
");
        let d2 = drive(&timing, "can0/2");
        assert_eq!(d2.identity_unanswered, 1);
        assert_eq!(d2.identity_reply_ms.n, 1);
        assert!((d2.identity_reply_ms.max - 0.2).abs() < 1e-9);
    }

    #[test]
    fn stats_use_nearest_rank() {
        let stats = Stats::from_ms((1..=20).map(f64::from).collect());
        assert_eq!(stats.n, 20);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.p50, 10.0);
        assert_eq!(stats.p95, 19.0);
        assert_eq!(stats.max, 20.0);
        assert_eq!(Stats::from_ms(Vec::new()), Stats::default());
    }

    #[test]
    fn json_shape_matches_the_contract() {
        let value = serde_json::to_value(run(ENABLE_SET_ZERO)).unwrap();
        for key in [
            "files",
            "frames",
            "span_s",
            "drives",
            "non_neutral_mit",
            "bus",
            "kernel_error_frames",
        ] {
            assert!(value.get(key).is_some(), "{key}");
        }
        let d1 = &value["drives"]["can0/1"];
        for key in [
            "enable_to_run_ms",
            "enable_never_run",
            "reset_after_enable",
            "set_zero_silence_start_ms",
            "set_zero_silence_unobservable",
            "set_zero_silence_ms",
            "identity_reply_ms",
            "identity_unanswered",
            "param_read_reply_ms",
            "report_period_ms",
            "report_off_to_last_ms",
            "mit_reply_ms",
            "mit_unanswered",
            "disable_to_reset_ms",
        ] {
            assert!(d1.get(key).is_some(), "{key}");
        }
        for key in ["n", "min", "p50", "p95", "max"] {
            assert!(d1["enable_to_run_ms"].get(key).is_some(), "{key}");
        }
        assert!(value["non_neutral_mit"]["first_s"].is_null());
        assert!(value["bus"]["max_frames_per_10ms"].is_u64());
        assert!(d1["set_zero_silence_unobservable"].is_u64());
    }
    // `candump -t z` renders a kernel error frame with a trailing
    // `ERRORFRAME` marker (can-utils lib.c): here an mcp251x RX-overflow
    // report (CAN_ERR_CRTL, data[1] RX_OVERFLOW) between two drive reports.
    const RX_OVERFLOW_BETWEEN_REPORTS: &str = "\
(1.000000) can0 180001FD#7FFF7FF57FFF00E6
(1.003000) can0 20000004 [8] 00 01 00 00 00 00 00 00   ERRORFRAME
(1.006000) can0 180001FD#7FFF7FF57FFF00E6
";

    #[test]
    fn kernel_error_frame_is_recorded_not_drive_traffic() {
        let timing = run(RX_OVERFLOW_BETWEEN_REPORTS);
        assert_eq!(timing.kernel_error_frames.len(), 1);
        let err = &timing.kernel_error_frames[0];
        assert_eq!(err.interface, "can0");
        assert_eq!(err.can_id, "20000004");
        assert_eq!(err.classes, ["ctrl"]);
        assert_eq!(err.ctrl, ["rx_overflow"]);
        assert!(!err.bus_off);
        assert!(!err.restarted);
        assert!((err.t_s - 0.003).abs() < 1e-9);
        // The error frame is bus-state evidence, not traffic: the 6 ms
        // drive gap stays one gap and the window holds two drive frames.
        assert_eq!(timing.bus.gaps_over_5ms, 1);
        assert_eq!(timing.bus.max_frames_per_10ms, 2);
        assert_eq!(timing.non_neutral_mit.count, 0);
        assert_eq!(drive(&timing, "can0/1").report_period_ms.n, 1);
    }

    #[test]
    fn error_frame_bytes_never_read_as_mit() {
        // Adversarial payload: every byte set, still a kernel error frame.
        let timing = run("\
(1.000000) can0 20000004 [8] FF FF FF FF FF FF FF FF   ERRORFRAME
");
        assert_eq!(timing.non_neutral_mit.count, 0);
        assert!(timing.drives.is_empty());
        assert_eq!(timing.kernel_error_frames.len(), 1);
        let err = &timing.kernel_error_frames[0];
        assert_eq!(err.classes, ["ctrl"]);
        assert_eq!(
            err.ctrl,
            [
                "rx_overflow",
                "tx_overflow",
                "rx_warning",
                "tx_warning",
                "rx_passive",
                "tx_passive",
                "active"
            ]
        );
        // No drive traffic at all: the error frame leaves bus stats empty.
        assert_eq!(timing.bus.gaps_over_5ms, 0);
        assert_eq!(timing.bus.max_frames_per_10ms, 0);
    }

    #[test]
    fn bus_off_and_restarted_classes_decode() {
        let timing = run("\
(1.000000) can0 20000040 [8] 00 00 00 00 00 00 00 00   ERRORFRAME
(1.001000) can0 20000100 [8] 00 00 00 00 00 00 00 00   ERRORFRAME
");
        assert_eq!(timing.kernel_error_frames.len(), 2);
        let off = &timing.kernel_error_frames[0];
        assert_eq!(off.classes, ["busoff"]);
        assert!(off.bus_off);
        assert!(!off.restarted);
        assert!(off.ctrl.is_empty());
        let restarted = &timing.kernel_error_frames[1];
        assert_eq!(restarted.classes, ["restarted"]);
        assert!(restarted.restarted);
        assert!(!restarted.bus_off);
    }
}
