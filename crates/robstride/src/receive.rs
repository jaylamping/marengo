//! One bounded, nonblocking receive engine shared by raw and decoded projections.

use std::time::{Duration, Instant};

use crate::bus::{BusError, CanBus, TimedCanFrame};

pub const MAX_RX_FRAMES_PER_POLL: usize = 64;
pub const MAX_RX_ATTEMPTS_PER_POLL: usize = 256;

/// Limits can narrow the declared per-poll capacity, never increase it.
#[derive(Debug, Clone, Copy)]
pub struct ReceiveLimits {
    pub max_frames: usize,
    pub max_attempts: usize,
    pub deadline: Option<Instant>,
}

impl Default for ReceiveLimits {
    fn default() -> Self {
        Self {
            max_frames: MAX_RX_FRAMES_PER_POLL,
            max_attempts: MAX_RX_ATTEMPTS_PER_POLL,
            deadline: None,
        }
    }
}

/// Completion describes host observations, not physical acquisition or epochs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveCompletion {
    #[default]
    Idle,
    Quiet,
    WorkLimit,
    Deadline,
    Failed,
}

impl ReceiveCompletion {
    pub fn is_complete(self) -> bool {
        matches!(self, Self::Idle | Self::Quiet)
    }
}

/// Exactly one backend read attempt; Interrupted is not proof of an empty source.
#[derive(Debug)]
pub enum ReceiveAttempt {
    Frame(TimedCanFrame),
    Idle,
    Interrupted,
}

#[derive(Debug, Default)]
pub struct RawReceiveReport {
    pub frames: Vec<TimedCanFrame>,
    pub completion: ReceiveCompletion,
    pub read_attempts: usize,
    pub terminal_error: Option<BusError>,
    /// Number of frames delivered before the first backend error. That error
    /// precedes any later peer frame with the same frame index.
    pub terminal_error_order: Option<usize>,
}

impl RawReceiveReport {
    pub(crate) fn into_result(self, out: &mut Vec<TimedCanFrame>) -> Result<(), BusError> {
        out.extend(self.frames);
        if let Some(error) = self.terminal_error {
            return Err(error);
        }
        if !self.completion.is_complete() {
            return Err(BusError::ReceiveIncomplete {
                completion: self.completion,
            });
        }
        Ok(())
    }
}

/// Each backend attempt is bounded; source rounds establish observed idle.
/// At a cap no lookahead read is performed, so pending work is unknown.
pub(crate) fn drain<B: CanBus + ?Sized>(
    bus: &mut B,
    budget: Duration,
    quiet: Duration,
    limits: ReceiveLimits,
) -> RawReceiveReport {
    let started = Instant::now();
    let budget_deadline = if budget.is_zero() {
        None
    } else if let Some(end) = started.checked_add(budget) {
        Some(end)
    } else {
        return RawReceiveReport {
            completion: ReceiveCompletion::Failed,
            terminal_error: Some(BusError::Driver(
                "receive budget overflows host clock".into(),
            )),
            ..RawReceiveReport::default()
        };
    };
    let deadline = match (budget_deadline, limits.deadline) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (first, second) => first.or(second),
    };
    let max_frames = limits.max_frames.min(MAX_RX_FRAMES_PER_POLL);
    let max_attempts = limits.max_attempts.min(MAX_RX_ATTEMPTS_PER_POLL);
    let sources = bus.receive_source_count();
    let mut report = RawReceiveReport::default();
    if sources == 0 {
        report.completion = ReceiveCompletion::Failed;
        report.terminal_error = Some(BusError::Driver("receive backend has no sources".into()));
        return report;
    }
    bus.begin_receive();
    let mut round_attempts = 0;
    let mut round_idle = true;
    let mut last_round_idle = false;
    let mut last_frame_at = None;
    loop {
        if report.frames.len() >= max_frames || report.read_attempts >= max_attempts {
            report.completion = ReceiveCompletion::WorkLimit;
            return report;
        }
        if deadline.is_some_and(|end| Instant::now() >= end) {
            // Expiry at a boundary after a complete idle pass is benign. A
            // partial or busy pass cannot establish idle, even after older ones.
            report.completion = if round_attempts == 0 && last_round_idle {
                ReceiveCompletion::Idle
            } else {
                ReceiveCompletion::Deadline
            };
            return report;
        }
        report.read_attempts += 1;
        round_attempts += 1;
        match bus.recv_one_nonblocking() {
            Ok(ReceiveAttempt::Frame(frame)) => {
                last_frame_at = Some(Instant::now());
                report.frames.push(frame);
                round_idle = false;
            }
            Ok(ReceiveAttempt::Idle) => {}
            Ok(ReceiveAttempt::Interrupted) => round_idle = false,
            Err(error) => {
                round_idle = false;
                if report.terminal_error.is_none() {
                    report.terminal_error_order = Some(report.frames.len());
                    report.terminal_error = Some(error);
                }
            }
        }
        if round_attempts != sources {
            continue;
        }
        last_round_idle = round_idle;
        // A terminal failure does not prevent the remaining sources in this
        // bounded round from contributing their queued evidence.
        if report.terminal_error.is_some() {
            report.completion = ReceiveCompletion::Failed;
            return report;
        }
        if round_idle {
            if budget.is_zero() {
                report.completion = ReceiveCompletion::Idle;
                return report;
            }
            let now = Instant::now();
            if last_frame_at.is_some_and(|last| now.duration_since(last) >= quiet) {
                report.completion = ReceiveCompletion::Quiet;
                return report;
            }
            if deadline.is_some_and(|end| now >= end) {
                report.completion = ReceiveCompletion::Idle;
                return report;
            }
            let remaining_rounds = (max_attempts - report.read_attempts) / sources;
            if remaining_rounds == 0 {
                // This complete pass established idle independently of the
                // quota. Do not start a partial pass just to burn unused tokens.
                report.completion = ReceiveCompletion::Idle;
                return report;
            }
            // Wait only after an entirely idle round, within the overall deadline.
            if let Some(end) = deadline {
                let remaining = end.saturating_duration_since(now);
                let paced = remaining / (remaining_rounds as u32 + 1);
                let wait = Duration::from_micros(200).max(paced).min(remaining);
                if !wait.is_zero() {
                    std::thread::sleep(wait);
                }
                // The latest complete pass was idle; expiry while waiting does
                // not imply overload or silently promote a partial pass.
                if Instant::now() >= end {
                    report.completion = ReceiveCompletion::Idle;
                    return report;
                }
            }
        }
        round_attempts = 0;
        round_idle = true;
    }
}
