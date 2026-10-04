//! Gravity saturation preflight, swept across control ticks.
//!
//! stdin `enable`, Chappe `robot/enable` and Testing Position auto-enable are
//! refused unless the coupled gravity torque over every joint's live command
//! envelope stays within its `tau_ff_max` (docs/safety.md). The 5^n grid
//! (3125 evaluations for five joints) takes 64–96 ms on the Pi. Run in one
//! call, it read no CAN for that long, so the host's own stall counted as
//! drive silence against `comm_watchdog_ms` (ADR 0036, host-caused silence).
//! [`GravitySweep::advance`] evaluates one bounded slice per control tick
//! instead, and the loop keeps draining feedback between slices.
//!
//! The envelope is captured once, at [`GravitySweep::start`]. Before every
//! slice [`GravitySweep::invalidated`] voids the sweep, and the request that
//! waited on it is refused, if any drive was stopped (Disable, a tick-error or
//! fault stop), a fault latched, reference work started, or a joint's limit
//! policy changed. A stale or partial sweep never grants. Berthier's gravity
//! model is fixed for the `ControlLoop`'s lifetime (there is no reload path),
//! so the model itself cannot change mid-sweep.

use std::fmt;
use std::time::{Duration, Instant};

use armee_kinematics::JointLimitPolicy;
use berthier::ControlLoop;
use davout::MotorBus;
use tracing::{debug, error, warn};

/// Grid points per joint across its live envelope.
const GRID_POINTS: usize = 5;
/// Largest grid the preflight will sweep; a bigger one is refused.
const MAX_GRID_SAMPLES: usize = 1_000_000;

/// How much of the grid one control tick may evaluate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SliceBudget {
    /// Evaluations per slice, at most.
    pub(crate) max_samples: usize,
    /// Wall time per slice, checked after each evaluation.
    pub(crate) max_time: Duration,
}

impl SliceBudget {
    /// The installed loop's share of a 5 ms tick. On the Pi one evaluation
    /// takes about 21 µs, so a slice ends near 2 ms (about 95 evaluations)
    /// and never runs more than 128 (about 2.7 ms).
    pub(crate) const PER_TICK: Self = Self {
        max_samples: 128,
        max_time: Duration::from_millis(2),
    };
}

/// Why a sweep was voided before its verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SweepAbort {
    /// An operator Disable (stdin or Chappe).
    Disabled,
    /// The process is shutting down (`quit`, EOF or a signal).
    Shutdown,
    /// Davout stopped the drives for another reason.
    Stopped,
    FaultLatched,
    ReferenceWork,
    LimitsChanged {
        joint: String,
    },
}

impl fmt::Display for SweepAbort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("disabled"),
            Self::Shutdown => f.write_str("shutting down"),
            Self::Stopped => f.write_str("drives were stopped"),
            Self::FaultLatched => f.write_str("a fault latched"),
            Self::ReferenceWork => f.write_str("reference work started"),
            Self::LimitsChanged { joint } => write!(f, "limits of {joint} changed"),
        }
    }
}

/// Timing of a finished sweep.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SweepStats {
    pub(crate) samples: usize,
    pub(crate) slices: u32,
    /// Longest single synchronous slice.
    pub(crate) max_slice: Duration,
    /// Wall time from [`GravitySweep::start`] to the verdict.
    pub(crate) elapsed: Duration,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SweepStep {
    Pending,
    /// Every sample evaluated: `passed` is false when the preflight refuses.
    Done {
        passed: bool,
        stats: SweepStats,
    },
}

struct JointSweep {
    joint: String,
    lower: f64,
    upper: f64,
    /// The policy the envelope came from; any change voids the sweep.
    policy: JointLimitPolicy,
    max_tau_nm: f64,
}

pub(crate) struct GravitySweep {
    joints: Vec<JointSweep>,
    sample_count: usize,
    next_sample: usize,
    q: Vec<f64>,
    started: Instant,
    slices: u32,
    max_slice: Duration,
    stop_generation: u64,
    fault_latched: bool,
    reference_pending: bool,
}

impl GravitySweep {
    /// Capture each joint's live envelope from its measured position, or
    /// refuse (fail closed) when a motor, policy, position or bound is missing
    /// or invalid, or the grid is too large. Evaluates nothing.
    pub(crate) fn start<B: MotorBus>(loop_ctrl: &ControlLoop<B>) -> Result<Self, ()> {
        let started = Instant::now();
        let supervisor = loop_ctrl.supervisor();
        let mut joints = Vec::with_capacity(loop_ctrl.joint_names().len());
        for joint in loop_ctrl.joint_names() {
            if !supervisor.motors.motors.iter().any(|m| &m.joint == joint) {
                error!(
                    joint,
                    "gravity preflight: no motor configuration; refusing enable"
                );
                return Err(());
            }
            let Some(policy) = supervisor.joint_limit_policy(joint) else {
                error!(
                    joint,
                    "gravity preflight: no live limit policy; refusing enable"
                );
                return Err(());
            };
            let Some(feedback) = supervisor.joint_feedback(joint) else {
                error!(
                    joint,
                    "gravity preflight: no measured position for live envelope"
                );
                return Err(());
            };
            let (q_min, q_max) =
                armee_kinematics::effective_command_bounds(policy, feedback.position_rad, 0.0);
            if !q_min.is_finite() || !q_max.is_finite() || q_min > q_max {
                error!(
                    joint,
                    q_min, q_max, "gravity preflight: invalid live envelope"
                );
                return Err(());
            }
            joints.push(JointSweep {
                joint: joint.clone(),
                lower: q_min,
                upper: q_max,
                policy: *policy,
                max_tau_nm: 0.0,
            });
        }
        let Ok(exponent) = u32::try_from(joints.len()) else {
            error!("gravity preflight: too many joints to sweep");
            return Err(());
        };
        let Some(sample_count) = GRID_POINTS.checked_pow(exponent) else {
            error!("gravity preflight: sweep size overflow");
            return Err(());
        };
        if sample_count > MAX_GRID_SAMPLES {
            error!(
                sample_count,
                "gravity preflight: sweep exceeds bounded work"
            );
            return Err(());
        }
        Ok(Self {
            q: vec![0.0; joints.len()],
            joints,
            sample_count,
            next_sample: 0,
            started,
            slices: 0,
            max_slice: Duration::ZERO,
            stop_generation: supervisor.stop_generation(),
            fault_latched: supervisor.has_latched_fault(),
            reference_pending: supervisor.reference_work_pending(),
        })
    }

    /// Whether the sweep no longer describes the drives it would admit.
    /// `reference_queued`: the operator reference queue has admitted joints.
    pub(crate) fn invalidated<B: MotorBus>(
        &self,
        loop_ctrl: &ControlLoop<B>,
        reference_queued: bool,
    ) -> Option<SweepAbort> {
        let supervisor = loop_ctrl.supervisor();
        if supervisor.stop_generation() != self.stop_generation {
            return Some(SweepAbort::Stopped);
        }
        if supervisor.has_latched_fault() && !self.fault_latched {
            return Some(SweepAbort::FaultLatched);
        }
        if reference_queued || (supervisor.reference_work_pending() && !self.reference_pending) {
            return Some(SweepAbort::ReferenceWork);
        }
        self.joints
            .iter()
            .find(|joint| supervisor.joint_limit_policy(&joint.joint) != Some(&joint.policy))
            .map(|joint| SweepAbort::LimitsChanged {
                joint: joint.joint.clone(),
            })
    }

    /// Evaluate one slice of the grid within `budget`. The last slice also
    /// applies the saturation rule: every joint's `max |τ_g|` must be within a
    /// finite, positive `tau_ff_max`; a model error or non-finite torque
    /// refuses at once.
    pub(crate) fn advance<B: MotorBus>(
        &mut self,
        loop_ctrl: &ControlLoop<B>,
        budget: SliceBudget,
    ) -> SweepStep {
        let slice_start = Instant::now();
        let slice_end = self
            .next_sample
            .saturating_add(budget.max_samples.max(1))
            .min(self.sample_count);
        let mut evaluable = true;
        while self.next_sample < slice_end {
            if !self.evaluate(loop_ctrl, self.next_sample) {
                evaluable = false;
                break;
            }
            self.next_sample += 1;
            if slice_start.elapsed() >= budget.max_time {
                break;
            }
        }
        self.slices = self.slices.saturating_add(1);
        self.max_slice = self.max_slice.max(slice_start.elapsed());
        if evaluable && self.next_sample < self.sample_count {
            return SweepStep::Pending;
        }
        let stats = SweepStats {
            samples: self.next_sample,
            slices: self.slices,
            max_slice: self.max_slice,
            elapsed: self.started.elapsed(),
        };
        debug!(
            samples = stats.samples,
            elapsed_us = micros(stats.elapsed),
            max_slice_us = micros(stats.max_slice),
            slices = stats.slices,
            "gravity preflight sweep"
        );
        SweepStep::Done {
            passed: evaluable && self.within_limits(),
            stats,
        }
    }

    /// One grid point; false (logged) when the model cannot vouch for it.
    fn evaluate<B: MotorBus>(&mut self, loop_ctrl: &ControlLoop<B>, sample: usize) -> bool {
        let mut digits = sample;
        for (q, joint) in self.q.iter_mut().zip(&self.joints) {
            let point = digits % GRID_POINTS;
            digits /= GRID_POINTS;
            *q =
                joint.lower + (joint.upper - joint.lower) * point as f64 / (GRID_POINTS - 1) as f64;
        }
        let tau = match loop_ctrl.preview_gravity_torques(&self.q) {
            Ok(tau) => tau,
            Err(error) => {
                error!(%error, "gravity preflight: model could not be evaluated; refusing enable");
                return false;
            }
        };
        if tau.len() != self.joints.len() {
            error!(
                torques = tau.len(),
                joints = self.joints.len(),
                "gravity preflight: torque vector does not match the joints; refusing enable"
            );
            return false;
        }
        for (joint, value) in self.joints.iter_mut().zip(tau) {
            if !value.is_finite() {
                error!(joint = %joint.joint, "gravity preflight: non-finite torque");
                return false;
            }
            joint.max_tau_nm = joint.max_tau_nm.max(value.abs());
        }
        true
    }

    fn within_limits(&self) -> bool {
        let mut saturated = false;
        for joint in &self.joints {
            let limit = joint.policy.tau_ff_max;
            let tau_max_nm = joint.max_tau_nm;
            if !limit.is_finite() || limit <= 0.0 || tau_max_nm > limit {
                error!(
                    joint = %joint.joint,
                    tau_max_nm, limit, "gravity saturation: refusing enable"
                );
                saturated = true;
            } else if tau_max_nm > 0.8 * limit {
                warn!(
                    joint = %joint.joint,
                    tau_max_nm, limit, "gravity torque >80% of motor limit"
                );
            }
        }
        !saturated
    }
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
