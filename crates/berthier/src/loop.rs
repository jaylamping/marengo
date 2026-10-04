//! Periodic control loop (OpenArm-style refresh → compute → MIT send).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use armee_dynamics::{DynamicsModel, UrdfGravityModel};
use armee_kinematics::clamp_hold_target;
use armee_proto::{ControlMode as ProtoControlMode, JointState, RobotState};
use chappe::Bus;
use davout::{
    ControlMode, DavoutError, MitJointCommand as DavoutMit, MotorBus, OperationalMode, Supervisor,
};
use marengo_config::{
    load_robot_config_from, motor_type_key, resolve_config_dir, resolve_urdf_path, ModeGains,
    MotorTypeDefaults, PositionLaw,
};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::degraded::{
    DegradedControl, DegradedEvent, DegradedLowerCause, DegradedPhase, DriveLossAdmission,
    DEGRADED_REST_TOLERANCE_RAD,
};
use crate::friction::POSITION_HOLD_ERROR_DEADBAND_RAD;
use crate::gain_runtime::{
    mode_allows_gain_override, target_gains_from_yaml, GainClampLimits, GainOverride, GainRuntime,
    GainShapeError, JointModeGains,
};
use crate::mit_feedforward::{MitFeedforward, MitFfJointIn};
use crate::position_hold::{
    DescentCap, HoldError, HoldFuseTrip, HoldJointDiag, HoldJointParams, HoldLaw, HoldRetarget,
    HoldWorld, PositionHold, ADVANCE_MAX_LEAD_DEFAULT,
};
use crate::position_law::{ReferenceFriction, ScaledPdGains};
use crate::position_profile::position_profile_v_max;
use crate::position_setpoint::{downward_return_seed_velocity, envelope_dq_cmd_for_hold_clamp};
use crate::position_trace::{PositionTrace, PositionTraceRow};
use crate::position_wave::PositionWave;
use crate::torque_cmd::TorqueCmdLatch;

/// Bound for an operator Enable or Position-mode arm waiting on
/// [`ControlLoop::enable_completion`]. Covers a target held for its drive's
/// post-SetZero quiet ([`davout::POST_SET_ZERO_QUIET`], 800 ms) right after a
/// home, the one-per-interface Enable stagger and the first session status,
/// with margin. Exceeding it refuses the waiting command.
pub const ENABLE_COMPLETION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum LoopError {
    #[error("safety: {0}")]
    Safety(#[from] DavoutError),
    #[error("dynamics: {0}")]
    Dynamics(#[from] armee_dynamics::DynamicsError),
    #[error("config: {0}")]
    Config(#[from] marengo_config::ConfigError),
    #[error("chappe: {0}")]
    Chappe(#[from] chappe::BusError),
    #[error("position hold: no setpoint latched for joint {joint}")]
    MissingSetpoint { joint: String },
    #[error("position hold: unknown joint {joint}")]
    UnknownJoint { joint: String },
    #[error("position hold: joint name required for hold-at on multi-joint configs")]
    JointNameRequired,
    #[error("position wave: min_rad must be less than max_rad")]
    InvalidWaveRange,
    #[error("position wave: cycles must be at least 1")]
    InvalidWaveCycles,
    #[error("position wave: half_period_sec must be positive")]
    InvalidWavePeriod,
    #[error("position wave: {field} must be finite (got {value})")]
    NonFiniteWave { field: &'static str, value: f64 },
    #[error(
        "position wave on {joint}: {min_rad:.4}..{max_rad:.4} rad leaves the commandable range {lower:.4}..{upper:.4} rad"
    )]
    WaveOutsideLimits {
        joint: String,
        min_rad: f64,
        max_rad: f64,
        lower: f64,
        upper: f64,
    },
    #[error(
        "position wave on {joint}: peak {quantity} {requested:.4} exceeds the joint limit {limit:.4}"
    )]
    WaveExceedsMotionLimit {
        joint: String,
        quantity: &'static str,
        requested: f64,
        limit: f64,
    },
    #[error(
        "position wave on {joint}: descends at up to {requested:.4} rad/s above {above_rad:.4} rad, where danger zone(s) {zones} clamp descent to {limit:.4} rad/s; use a half period of at least {min_half_period_s:.2} s"
    )]
    WaveExceedsDangerZone {
        joint: String,
        zones: String,
        above_rad: f64,
        requested: f64,
        limit: f64,
        min_half_period_s: f64,
    },
    #[error("position hold: target for {joint} must be finite (got {value})")]
    NonFiniteTarget { joint: String, value: f64 },
    #[error("missing motor feedback for joint {joint}")]
    MissingFeedback { joint: String },
    /// A Position-mode arm latches measured `q`; it waits until every Active
    /// target's staggered Enable is written and its session feedback is fresh.
    #[error("waiting for enable to complete: {detail}")]
    EnableIncomplete { detail: String },
    #[error(
        "position hold: outbound ascent stall on {joint}: no measured progress toward target for {ms} ms ({trip})"
    )]
    AscentStall {
        joint: String,
        ms: u64,
        trip: HoldFuseTrip,
    },
    #[error(
        "position hold: hold tracking failure on {joint}: off target with net commanded torque opposing it and no measured progress for {ms} ms ({trip})"
    )]
    HoldTracking {
        joint: String,
        ms: u64,
        trip: HoldFuseTrip,
    },
    #[error(
        "position wave: wave stall on {joint}: no measured motion while the wave commanded motion for {ms} ms ({trip})"
    )]
    WaveStall {
        joint: String,
        ms: u64,
        trip: HoldFuseTrip,
    },
    /// Internal invariant break: the hold law saw joint vectors of different lengths.
    #[error("position hold: joint vector length mismatch")]
    HoldLenMismatch,
    #[error("torque cmd: non-finite τ_cmd for joint {joint}")]
    NonFiniteTorqueCmd { joint: String },
    #[error("invalid gain override for {joint}: {field} must be finite and nonnegative")]
    InvalidGainOverride { joint: String, field: &'static str },
    /// An operator disabled the drives; a motion command may not re-enable them.
    #[error("drives were disabled by an operator; enable explicitly before commanding motion")]
    ExplicitEnableRequired,
    /// A Testing gain override only exists in Impedance/Position; elsewhere it
    /// would be silently dropped, so the caller is told instead of getting Ok.
    #[error("gain override for {joint} not applied: control mode {mode:?} has no runtime gains")]
    GainOverrideNotApplicable { joint: String, mode: ControlMode },
    #[error("invalid nominal controller period {seconds} seconds")]
    InvalidLoopPeriod { seconds: f64 },
    /// A degraded episode owns the drives: only disable or lower is accepted.
    #[error(
        "degraded hold after losing {joint}: motion refused; only disable or lower is accepted"
    )]
    DegradedHold { joint: String },
    /// `on_drive_loss: shed_subtree` failed admission (ADR 0038).
    #[error("drive-loss admission refused for {joint}: {message}")]
    DriveLossAdmission { joint: String, message: String },
    /// `lower` outside a degraded episode.
    #[error("lower refused: no degraded episode is in progress")]
    NoDegradedEpisode,
    /// Internal invariant break: per-joint gain inputs were not parallel to the joint list.
    #[error(transparent)]
    GainShape(#[from] GainShapeError),
}

impl From<HoldError> for LoopError {
    fn from(err: HoldError) -> Self {
        match err {
            HoldError::AscentStall { joint, ms, trip } => Self::AscentStall { joint, ms, trip },
            HoldError::HoldTracking { joint, ms, trip } => Self::HoldTracking { joint, ms, trip },
            HoldError::MissingSetpoint { joint } => Self::MissingSetpoint { joint },
            HoldError::WaveStall { joint, ms, trip } => Self::WaveStall { joint, ms, trip },
            HoldError::LenMismatch => Self::HoldLenMismatch,
            HoldError::InvalidPeriod { seconds } => Self::InvalidLoopPeriod { seconds },
        }
    }
}

/// Realtime control loop facade.
pub struct ControlLoop<B: MotorBus> {
    supervisor: Supervisor<B>,
    dynamics: UrdfGravityModel,
    /// Per joint, the URDF inertia about its axis at the zero pose (kg·m²): the scaled-PD
    /// `J·a` feed-forward constant (ADR 0039). Rotor inertia is not in the URDF.
    reference_inertia: Vec<f64>,
    /// Repo root for commissioning-scope / robot.yaml resolution on re-arm.
    repo_root: PathBuf,
    joint_names: Vec<String>,
    control_mode: ControlMode,
    /// Position-hold lifecycle + control law ([`PositionHold`]).
    position_hold: PositionHold,
    loop_period: Duration,
    chappe_publish_period: Duration,
    last_chappe: Option<Instant>,
    last_position_diag: Option<Instant>,
    tick_count: u64,
    loop_hz: u32,
    position_trace: Option<PositionTrace>,
    /// Cumulative per-tick phase times for 1 Hz diagnostics (`take_tick_phase_averages`).
    tick_phase: TickPhaseAccumulator,
    /// Previous enable-session marker, including re-enable between controller ticks.
    last_enable_session: Option<Instant>,
    /// Stop invalidates retained planners even if Disable/Enable occurs between ticks.
    last_stop_generation: u64,
    /// Ticks allowed without joint feedback immediately after enable (post-homing tick_count > 0).
    active_feedback_grace_ticks: u8,
    /// Testing overrides + mode-transition kp/kd ramp + per-tick resolve.
    gains: GainRuntime,
    /// Per-joint latched open-loop `τ_cmd` (Nm) for [`ControlMode::TorqueOnly`].
    torque_cmds: TorqueCmdLatch,
    /// In-loop triangle wave on one joint while others hold fixed setpoints.
    position_wave: Option<PositionWave>,
    /// Set by an operator disable: motion commands then refuse instead of
    /// re-enabling drives (see [`ControlLoop::ensure_active_for_motion`]).
    implicit_enable_forbidden: bool,
    /// Chappe telemetry publish failures: counted and logged, never
    /// propagated as tick errors (telemetry must not stop motion).
    telemetry_failures: u64,
    /// Ticks whose wall time exceeded the loop period (M06 loop-budget evidence).
    tick_overruns: u64,
    /// Berthier's side of a degraded episode Davout started (ADR 0038).
    degraded: Option<DegradedControl>,
    /// Episode transitions not yet taken by the owner.
    degraded_events: Vec<DegradedEvent>,
}

/// Per-tick CPU time inside [`ControlLoop::tick`] (microseconds, averaged over a window).
#[derive(Debug, Clone, Copy, Default)]
pub struct TickPhaseAverages {
    pub ticks: u32,
    pub feedback_us: u64,
    pub gravity_us: u64,
    pub planner_us: u64,
    pub compose_us: u64,
    pub trace_us: u64,
    pub send_us: u64,
    pub chappe_us: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct TickPhaseSample {
    feedback_us: u64,
    gravity_us: u64,
    planner_us: u64,
    compose_us: u64,
    trace_us: u64,
    send_us: u64,
    chappe_us: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct TickPhaseAccumulator {
    ticks: u32,
    feedback_us: u64,
    gravity_us: u64,
    planner_us: u64,
    compose_us: u64,
    trace_us: u64,
    send_us: u64,
    chappe_us: u64,
}

impl TickPhaseAccumulator {
    fn record(&mut self, sample: TickPhaseSample) {
        self.ticks = self.ticks.saturating_add(1);
        self.feedback_us = self.feedback_us.saturating_add(sample.feedback_us);
        self.gravity_us = self.gravity_us.saturating_add(sample.gravity_us);
        self.planner_us = self.planner_us.saturating_add(sample.planner_us);
        self.compose_us = self.compose_us.saturating_add(sample.compose_us);
        self.trace_us = self.trace_us.saturating_add(sample.trace_us);
        self.send_us = self.send_us.saturating_add(sample.send_us);
        self.chappe_us = self.chappe_us.saturating_add(sample.chappe_us);
    }

    fn into_averages(self) -> Option<TickPhaseAverages> {
        if self.ticks == 0 {
            return None;
        }
        let n = u64::from(self.ticks);
        let avg = TickPhaseAverages {
            ticks: self.ticks,
            feedback_us: self.feedback_us / n,
            gravity_us: self.gravity_us / n,
            planner_us: self.planner_us / n,
            compose_us: self.compose_us / n,
            trace_us: self.trace_us / n,
            send_us: self.send_us / n,
            chappe_us: self.chappe_us / n,
        };
        Some(avg)
    }
}

fn phase_elapsed_us(since: Instant) -> (u64, Instant) {
    let now = Instant::now();
    let us = u64::try_from(now.duration_since(since).as_micros()).unwrap_or(u64::MAX);
    (us, now)
}

impl ControlLoop<davout::simulation::SimulationBus> {
    #[cfg(any(test, feature = "reference-journal-test-support"))]
    pub fn from_simulation_with_paused_reference_journal(
        repo_root: impl AsRef<Path>,
        bus: davout::simulation::SimulationBus,
        record_path: impl AsRef<Path>,
        journal_path: impl AsRef<Path>,
        pause: &davout::simulation::JournalTestPause,
        loop_hz: u32,
        chappe_hz: u32,
    ) -> Result<Self, LoopError> {
        let root = repo_root.as_ref();
        Self::from_repo_inner(
            root,
            &root.join("config"),
            bus,
            loop_hz,
            chappe_hz,
            |root, bus| {
                Supervisor::from_simulation_with_paused_reference_journal(
                    root,
                    bus,
                    record_path,
                    journal_path,
                    pause,
                )
            },
        )
    }

    #[cfg(any(test, feature = "reference-journal-test-support"))]
    pub fn from_simulation_with_paused_current_reference_journal(
        repo_root: impl AsRef<Path>,
        bus: davout::simulation::SimulationBus,
        record_path: impl AsRef<Path>,
        journal_path: impl AsRef<Path>,
        pause: &davout::simulation::JournalTestPause,
        loop_hz: u32,
        chappe_hz: u32,
    ) -> Result<Self, LoopError> {
        let root = repo_root.as_ref();
        Self::from_repo_inner(
            root,
            &root.join("config"),
            bus,
            loop_hz,
            chappe_hz,
            |root, bus| {
                Supervisor::from_simulation_with_paused_current_reference_journal(
                    root,
                    bus,
                    record_path,
                    journal_path,
                    pause,
                )
            },
        )
    }

    /// Construct the real controller over Davout's closed in-memory transport.
    ///
    /// The declared virtual reference is an initial simulation condition. It
    /// does not establish SetZero causality or qualify a physical reference.
    /// Dynamics, session initialization, admission and tick behavior use the
    /// same implementation as ordinary construction.
    /// Configuration comes from the supplied root's `config/`, independent of
    /// the installed Pi configuration and environment configuration override.
    pub fn from_simulation(
        repo_root: impl AsRef<Path>,
        bus: davout::simulation::SimulationBus,
        initial_reference: davout::simulation::InitialVirtualReference,
        loop_hz: u32,
        chappe_hz: u32,
    ) -> Result<Self, LoopError> {
        let root = repo_root.as_ref();
        Self::from_repo_inner(
            root,
            &root.join("config"),
            bus,
            loop_hz,
            chappe_hz,
            |root, bus| Supervisor::from_simulation(root, bus, initial_reference),
        )
    }
}

impl<B: MotorBus> ControlLoop<B> {
    pub fn from_repo(
        repo_root: impl AsRef<Path>,
        bus: B,
        loop_hz: u32,
        chappe_hz: u32,
    ) -> Result<Self, LoopError> {
        let root = repo_root.as_ref();
        Self::from_repo_inner(
            root,
            &resolve_config_dir(root),
            bus,
            loop_hz,
            chappe_hz,
            |root, bus| Supervisor::from_repo(root, bus),
        )
    }

    /// [`Self::from_repo`] with the qualified physical Robstride reference owner
    /// (ADR 0036). `journal_path` must be absolute and distinct from the
    /// reserved history location; see `marengo_config::resolve_reference_journal_path`.
    pub fn from_repo_with_physical_reference(
        repo_root: impl AsRef<Path>,
        bus: B,
        journal_path: impl AsRef<Path>,
        loop_hz: u32,
        chappe_hz: u32,
    ) -> Result<Self, LoopError> {
        let root = repo_root.as_ref();
        let journal = journal_path.as_ref();
        Self::from_repo_inner(
            root,
            &resolve_config_dir(root),
            bus,
            loop_hz,
            chappe_hz,
            |root, bus| Supervisor::from_repo_with_physical_reference(root, bus, journal),
        )
    }

    fn from_repo_inner(
        root: &Path,
        config_dir: &Path,
        bus: B,
        loop_hz: u32,
        chappe_hz: u32,
        build_supervisor: impl FnOnce(&Path, B) -> Result<Supervisor<B>, DavoutError>,
    ) -> Result<Self, LoopError> {
        if loop_hz == 0 {
            // A zero rate has no period; never reinterpret it as 1 Hz.
            return Err(LoopError::InvalidLoopPeriod {
                seconds: f64::INFINITY,
            });
        }
        let seconds = 1.0 / f64::from(loop_hz);
        let loop_period = Duration::try_from_secs_f64(seconds)
            .map_err(|_| LoopError::InvalidLoopPeriod { seconds })?;
        if loop_period.is_zero() {
            return Err(LoopError::InvalidLoopPeriod { seconds });
        }
        let robot = load_robot_config_from(config_dir)?;
        let joint_names = robot.robot.joints.clone();
        let urdf = resolve_urdf_path(root, &robot)?;
        let dynamics = UrdfGravityModel::from_urdf(&urdf, &joint_names)?;
        let zero_pose = vec![0.0; joint_names.len()];
        let reference_inertia = joint_names
            .iter()
            .map(|joint| dynamics.joint_inertia(joint, &zero_pose))
            .collect::<Result<Vec<_>, _>>()?;
        let supervisor = build_supervisor(root, bus)?;
        let progress_thresholds = joint_names
            .iter()
            .map(|joint| supervisor.joint_position_progress_threshold(joint))
            .collect::<Result<Vec<_>, _>>()?;
        let position_trace = PositionTrace::from_env(loop_hz, &joint_names);
        Ok(Self {
            supervisor,
            dynamics,
            reference_inertia,
            repo_root: root.to_path_buf(),
            joint_names,
            control_mode: ControlMode::Disabled,
            position_hold: PositionHold::with_progress_thresholds(progress_thresholds),
            loop_period,
            chappe_publish_period: Duration::from_secs_f64(1.0 / f64::from(chappe_hz.max(1))),
            last_chappe: None,
            last_position_diag: None,
            tick_count: 0,
            loop_hz,
            position_trace,
            tick_phase: TickPhaseAccumulator::default(),
            last_enable_session: None,
            last_stop_generation: 0,
            active_feedback_grace_ticks: 0,
            gains: GainRuntime::for_loop_hz(loop_hz),
            torque_cmds: TorqueCmdLatch::new(),
            position_wave: None,
            implicit_enable_forbidden: false,
            telemetry_failures: 0,
            tick_overruns: 0,
            degraded: None,
            degraded_events: Vec::new(),
        })
    }

    /// Mean per-tick phase times since the last call; resets the accumulator.
    pub fn take_tick_phase_averages(&mut self) -> Option<TickPhaseAverages> {
        std::mem::take(&mut self.tick_phase).into_averages()
    }

    /// Chappe telemetry publish failures since construction. These are
    /// counted and logged at the publish site, never propagated: a failed
    /// `robot/state` send must not stop motion (L-berthier-24).
    pub fn telemetry_failures(&self) -> u64 {
        self.telemetry_failures
    }

    /// Ticks whose wall time exceeded the loop period (M06 evidence).
    pub fn tick_overruns(&self) -> u64 {
        self.tick_overruns
    }

    /// Flush the position-trace CSV. Call on owner shutdown, never in the tick.
    pub fn flush_position_trace(&mut self) {
        if let Some(trace) = self.position_trace.as_mut() {
            if let Err(error) = trace.flush() {
                warn!(error = %error, "position trace flush failed");
            }
        }
    }

    /// Enable is complete for motion: the supervisor is Active, no staggered
    /// Enable is still unwritten (e.g. one held for its drive's post-SetZero
    /// quiet), and every Active joint has fresh pose from this enable session.
    /// Every Position-mode arm or retarget requires this before it uses
    /// measured `q`, which reads 0.0 for a joint without pose.
    pub fn enable_completion(&self) -> Result<(), LoopError> {
        let mode = self.supervisor.mode();
        if mode != OperationalMode::Active {
            return Err(LoopError::EnableIncomplete {
                detail: format!("supervisor {mode:?}, not Active"),
            });
        }
        if self.supervisor.enable_writes_pending() {
            return Err(LoopError::EnableIncomplete {
                detail: "staggered Enable writes pending".into(),
            });
        }
        let active = self.supervisor.active_joints();
        if let Some(joint) = self
            .joint_names
            .iter()
            .find(|name| active.contains(*name) && !self.has_joint_feedback(name))
        {
            return Err(LoopError::EnableIncomplete {
                detail: format!("no session feedback from {joint}"),
            });
        }
        Ok(())
    }

    fn latch_position_from_q(&mut self, q: &[f64]) {
        self.position_hold.arm(q, q, self.tick_count);
        for (i, name) in self.joint_names.iter().enumerate() {
            let dq = self.joint_velocity(name);
            self.position_hold.seed_dq_filter(i, dq);
        }
    }

    pub fn position_setpoints(&self) -> Option<&[f64]> {
        self.position_hold.targets()
    }

    /// Planner trajectory reference (for status / tests), not the clamped MIT setpoint.
    pub fn position_hold_commands(&self) -> Option<Vec<f64>> {
        self.position_hold.q_traj()
    }

    pub fn set_joint_position_setpoint(
        &mut self,
        joint: &str,
        position_rad: f64,
    ) -> Result<(), LoopError> {
        self.refuse_degraded()?;
        let Some(i) = self.joint_names.iter().position(|n| n == joint) else {
            return Err(LoopError::UnknownJoint {
                joint: joint.to_string(),
            });
        };
        let trimmed = self.hold_target_trim(joint, position_rad);
        if !trimmed.is_finite() {
            return Err(LoopError::NonFiniteTarget {
                joint: joint.to_string(),
                value: position_rad,
            });
        }
        let q_now = self.refresh_joint_positions()?;
        self.enable_completion()?;
        if !self.position_hold.is_armed() {
            self.latch_position_from_q(&q_now);
        }
        self.apply_joint_retarget(i, q_now[i], trimmed);
        Ok(())
    }

    /// Latch joint `i`'s target `requested` (trim applied) from measured `q`:
    /// envelope clamp, downward-return seed and planner retarget, the one law
    /// every hold-at uses.
    fn apply_joint_retarget(&mut self, i: usize, q: f64, requested: f64) {
        let joint = self.joint_names[i].clone();
        let joint = joint.as_str();
        let dq_cmd = self.estimated_retarget_dq_cmd(joint, q, requested);
        let slew = self
            .supervisor
            .control
            .control
            .joints
            .get(joint)
            .map(|c| c.position_slew_rad_s)
            .unwrap_or(0.15);
        let dq_envelope = envelope_dq_cmd_for_hold_clamp(
            self.supervisor.joint_limit_policy(joint),
            q,
            requested,
            dq_cmd,
            slew,
        );
        let target = self.clamp_hold_target(joint, q, requested, dq_envelope);
        let old_target = self
            .position_hold
            .targets()
            .and_then(|sp| sp.get(i).copied())
            .unwrap_or(q);
        if (requested - target).abs() > 1e-6 {
            info!(
                joint = %joint,
                requested,
                clamped = target,
                q,
                "hold-at target clamped to limit envelope"
            );
        }
        let downward_seed_rate = if (old_target - target).abs() > 1e-6 {
            self.supervisor
                .control
                .control
                .joints
                .get(joint)
                .map(|cfg| {
                    let move_dist = (target - q).abs();
                    let threshold = cfg.position_trajectory_threshold_rad;
                    let v_max = self.clamp_v_max(
                        joint,
                        position_profile_v_max(
                            move_dist,
                            cfg.position_slew_rad_s,
                            cfg.position_trajectory_velocity_rad_s,
                            threshold,
                        ),
                    );
                    downward_return_seed_velocity(cfg.position_slew_rad_s, v_max, q, target)
                })
        } else {
            None
        };
        // Retargets follow the joint's configured law (scaled PD replans from its reference).
        self.position_hold
            .set_joint_law(i, self.configured_position_law(joint));
        let target_changed = self.position_hold.apply_retarget(HoldRetarget {
            joint_idx: i,
            clamped: target,
            requested,
            q,
            tick: self.tick_count,
            dq_seed: Some(self.joint_velocity(joint)),
            downward_seed: downward_seed_rate,
        });
        // Same-target hold-at must not cancel an in-loop wave (idempotent operator path).
        if target_changed {
            self.position_wave = None;
            info!(
                joint = %joint,
                q,
                old_target,
                new_target = target,
                delta = target - q,
                tick = self.tick_count,
                "position hold retarget"
            );
        }
    }

    /// Trace rows for this tick: every full-rate joint, the rest on the decimation period
    /// (no-op unless `MARENGO_POSITION_TRACE` is set).
    fn record_position_trace(&mut self, diag: &[HoldJointDiag]) {
        let Some(trace) = self.position_trace.as_mut() else {
            return;
        };
        let t_ms = self.tick_count.saturating_mul(1000) / u64::from(self.loop_hz);
        for (index, (name, d)) in self.joint_names.iter().zip(diag).enumerate() {
            if !trace.records(self.tick_count, index) {
                continue;
            }
            let row = PositionTraceRow {
                joint: name,
                q: d.q,
                dq: d.dq_raw,
                q_traj: d.q_traj,
                dq_traj: d.dq_traj,
                q_des: d.q_des,
                target: d.target,
                target_raw: d.target_raw,
                q_env_lo: d.q_env_lo,
                q_env_hi: d.q_env_hi,
                lead: d.lead,
                lead_sat: d.lead_sat,
                settle_error: d.settle_error,
                phase: d.phase,
                friction_mode: d.friction_mode,
                tau_p: d.tau_p,
                tau_g: d.tau_g,
                tau_f: d.tau_f,
                tau_d: d.tau_d,
                tau_ff_cmd: d.tau_ff_cmd,
                tau_meas: d.tau_meas,
                dq_mit: d.mit_velocity,
                kp: d.kp,
                kd: d.kd,
                joint_stuck: d.joint_stuck,
                planner_frozen: d.planner_frozen,
                retarget_age_ms: d.retarget_age_ms,
                planner_event: d.planner_event.as_str(),
                law: d.law.as_str(),
                q_ref: d.q_ref,
                dq_ref: d.dq_ref,
                time_scale: d.time_scale,
                tau_i: d.tau_i,
                kd_mit: d.mit_kd,
                // Davout's last sent τ_ff (post-cap, post-rate-limit); NaN before any send.
                tau_ff_wire: self.supervisor.last_tau_ff_nm(name).unwrap_or(f64::NAN),
            };
            trace.maybe_record(self.tick_count, t_ms, index, &row);
        }
    }

    fn configured_position_law(&self, joint: &str) -> PositionLaw {
        self.supervisor
            .control
            .control
            .joints
            .get(joint)
            .map(|c| c.position_law)
            .unwrap_or_default()
    }

    fn hold_target_trim(&self, joint: &str, position_rad: f64) -> f64 {
        let trim = self
            .supervisor
            .control
            .control
            .joints
            .get(joint)
            .map(|c| c.position_hold_trim_rad)
            .unwrap_or(0.0);
        position_rad + trim
    }

    fn clamp_hold_target(&self, joint: &str, q: f64, requested_rad: f64, dq_cmd: f64) -> f64 {
        let Some(policy) = self.supervisor.joint_limit_policy(joint) else {
            return requested_rad;
        };
        clamp_hold_target(policy, q, dq_cmd, requested_rad)
    }

    fn clamp_v_max(&self, joint: &str, v_requested: f64) -> f64 {
        let v = self
            .supervisor
            .joint_velocity_cap(joint)
            .map_or(v_requested, |cap| v_requested.min(cap));
        self.degraded_lower_velocity()
            .map_or(v, |lower| v.min(lower))
    }

    fn estimated_retarget_dq_cmd(&self, joint: &str, q: f64, requested_rad: f64) -> f64 {
        let delta = requested_rad - q;
        if delta.abs() <= 1e-9 {
            return 0.0;
        }
        let cfg = self.supervisor.control.control.joints.get(joint);
        let slew = cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25);
        let trajectory_v = cfg
            .map(|c| c.position_trajectory_velocity_rad_s)
            .unwrap_or(0.30);
        let threshold = cfg
            .map(|c| c.position_trajectory_threshold_rad)
            .unwrap_or(0.15);
        let v_max = position_profile_v_max(delta.abs(), slew, trajectory_v, threshold);
        delta.signum() * self.clamp_v_max(joint, v_max)
    }

    pub fn clear_position_hold(&mut self) {
        self.position_hold.clear();
        self.position_wave = None;
    }

    /// Start a continuous triangle wave on one joint (Position mode).
    ///
    /// Other joints keep their current latched setpoints. The wave runs in-loop for
    /// `cycles` full min→max→min periods without stdin pacing.
    pub fn start_position_wave(
        &mut self,
        joint: &str,
        min_rad: f64,
        max_rad: f64,
        cycles: u32,
        half_period_sec: f64,
    ) -> Result<f64, LoopError> {
        self.refuse_degraded()?;
        for (field, value) in [
            ("min_rad", min_rad),
            ("max_rad", max_rad),
            ("half_period_sec", half_period_sec),
        ] {
            if !value.is_finite() {
                return Err(LoopError::NonFiniteWave { field, value });
            }
        }
        if min_rad >= max_rad {
            return Err(LoopError::InvalidWaveRange);
        }
        if cycles == 0 {
            return Err(LoopError::InvalidWaveCycles);
        }
        if half_period_sec <= 0.0 {
            return Err(LoopError::InvalidWavePeriod);
        }
        let Some(i) = self.joint_names.iter().position(|n| n == joint) else {
            return Err(LoopError::UnknownJoint {
                joint: joint.to_string(),
            });
        };
        let half_period_ticks = (half_period_sec * f64::from(self.loop_hz)).round().max(1.0) as u64;
        self.validate_wave_motion(joint, min_rad, max_rad, half_period_ticks)?;
        let q = self.motion_positions()?;
        let was_armed = self.position_hold.is_armed();
        self.position_hold.ensure_armed_from_q(&q, self.tick_count);
        if !was_armed {
            for (i, name) in self.joint_names.iter().enumerate() {
                self.position_hold
                    .seed_dq_filter(i, self.joint_velocity(name));
            }
        }
        self.set_control_mode(ControlMode::Position);
        let wave = PositionWave::new(
            i,
            min_rad,
            max_rad,
            self.tick_count,
            half_period_ticks,
            cycles,
        );
        let duration_sec = wave.duration_sec(self.loop_hz);
        self.position_wave = Some(wave);
        info!(
            joint = %joint,
            min_rad,
            max_rad,
            cycles,
            half_period_sec,
            duration_sec,
            "position wave started"
        );
        Ok(duration_sec)
    }

    /// Admit a wave only if its whole raised-cosine reference stays inside what Berthier would
    /// itself plan: the wave bypasses the trapezoid planner (its target is the reference), so
    /// the planner's limits are enforced here, once, instead of per tick.
    ///
    /// - Range inside the joint's soft limit envelope (hold-at clamps; a wave is refused rather
    ///   than silently reshaped).
    /// - Peak speed `A·ω` within the Davout velocity cap and the joint's configured
    ///   `position_trajectory_velocity_rad_s`.
    /// - Peak acceleration `A·ω²` within `position_trajectory_accel_rad_s2`.
    /// - Descent above a `clamp_velocity` danger zone's threshold within its speed (see
    ///   [`DescentCap::wave_descent_speed`]): Davout would clamp the MIT velocity there, and
    ///   with drive damping the clamp brakes against the wave and steps torque on release.
    ///
    /// `A = (max − min)/2` and `ω = π / T` use the tick-quantized half period `T`, so a tiny
    /// `half_period_sec` that rounds to one tick is judged at its true speed.
    fn validate_wave_motion(
        &self,
        joint: &str,
        min_rad: f64,
        max_rad: f64,
        half_period_ticks: u64,
    ) -> Result<(), LoopError> {
        if let Some(policy) = self.supervisor.joint_limit_policy(joint) {
            let (lower, upper) = (policy.soft_lower(), policy.soft_upper());
            if min_rad < lower || max_rad > upper {
                return Err(LoopError::WaveOutsideLimits {
                    joint: joint.to_string(),
                    min_rad,
                    max_rad,
                    lower,
                    upper,
                });
            }
        }
        let half_period = half_period_ticks as f64 / f64::from(self.loop_hz);
        let amplitude = 0.5 * (max_rad - min_rad);
        let omega = std::f64::consts::PI / half_period;
        let cfg = self.supervisor.control.control.joints.get(joint);
        let speed_limit = [
            self.supervisor.joint_velocity_cap(joint),
            cfg.map(|c| c.position_trajectory_velocity_rad_s),
        ]
        .into_iter()
        .flatten()
        .fold(f64::INFINITY, f64::min);
        let peak_speed = amplitude * omega;
        if peak_speed > speed_limit {
            return Err(LoopError::WaveExceedsMotionLimit {
                joint: joint.to_string(),
                quantity: "speed (rad/s)",
                requested: peak_speed,
                limit: speed_limit,
            });
        }
        if let Some(accel_limit) = cfg.map(|c| c.position_trajectory_accel_rad_s2) {
            let peak_accel = amplitude * omega * omega;
            if peak_accel > accel_limit {
                return Err(LoopError::WaveExceedsMotionLimit {
                    joint: joint.to_string(),
                    quantity: "acceleration (rad/s²)",
                    requested: peak_accel,
                    limit: accel_limit,
                });
            }
        }
        let zones = &self.supervisor.control.control.danger_zones;
        if let Some(cap) = DescentCap::from_zones(zones, joint) {
            let factor = cap.wave_descent_speed(min_rad, max_rad, 1.0);
            let descent = peak_speed * factor;
            if descent > cap.max_velocity_rad_s {
                let names = zones
                    .iter()
                    .filter(|z| z.joint == joint && z.action == "clamp_velocity")
                    .map(|z| z.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(LoopError::WaveExceedsDangerZone {
                    joint: joint.to_string(),
                    zones: names,
                    above_rad: cap.above_rad,
                    requested: descent,
                    limit: cap.max_velocity_rad_s,
                    min_half_period_s: std::f64::consts::PI * amplitude * factor
                        / cap.max_velocity_rad_s,
                });
            }
        }
        Ok(())
    }

    pub fn position_wave_active(&self) -> bool {
        self.position_wave
            .as_ref()
            .is_some_and(|w| !w.is_finished())
    }

    /// Latch current `q` and enter [`ControlMode::Position`] (gravity FF + impedance gains).
    /// Re-arms drives when needed; refuses with [`LoopError::EnableIncomplete`]
    /// until [`Self::enable_completion`] holds.
    pub fn enter_position_hold(&mut self) -> Result<(), LoopError> {
        self.refuse_degraded()?;
        let q = self.motion_positions()?;
        self.latch_position_from_q(&q);
        self.set_control_mode(ControlMode::Position);
        Ok(())
    }

    /// Measured `q` for a Position-mode arm: drain feedback (a receive failure
    /// refuses before any Enable), re-arm drives when needed, then require
    /// [`Self::enable_completion`]. A re-enable done here always waits: its
    /// session has no pose yet, so the `q` drained before it is never used.
    fn motion_positions(&mut self) -> Result<Vec<f64>, LoopError> {
        let q = self.refresh_joint_positions()?;
        self.ensure_active_for_motion()?;
        self.enable_completion()?;
        Ok(q)
    }

    /// Re-arm drives after a safety disable via scoped commissioning Enable.
    ///
    /// Never calls [`Supervisor::set_homing_complete`] — Verified is Set Zero only.
    /// Targets come from [`Supervisor::resolve_enable_targets`] (persisted scope or
    /// full-master Robot Ready).
    ///
    /// After an operator disable ([`Self::forbid_implicit_enable`]) a motion
    /// command no longer implies Enable: it is refused with
    /// [`LoopError::ExplicitEnableRequired`] until the operator enables
    /// explicitly (L-berthier-28).
    pub fn ensure_active_for_motion(&mut self) -> Result<(), LoopError> {
        self.refuse_degraded()?;
        self.synchronize_stop_generation();
        let targets = if self.supervisor.mode() == OperationalMode::Active {
            self.supervisor.active_joints().iter().cloned().collect()
        } else if self.implicit_enable_forbidden {
            return Err(LoopError::ExplicitEnableRequired);
        } else {
            self.supervisor.resolve_enable_targets(&self.repo_root)?
        };
        self.supervisor.enable_targets(&targets)?;
        Ok(())
    }

    /// The operator disabled the drives: a later motion command must not
    /// silently re-enable them. Cleared by [`Self::allow_implicit_enable`].
    pub fn forbid_implicit_enable(&mut self) {
        self.implicit_enable_forbidden = true;
    }

    /// The operator enabled the drives explicitly; motion commands may again
    /// re-arm drives that a safety (non-operator) disable dropped.
    pub fn allow_implicit_enable(&mut self) {
        self.implicit_enable_forbidden = false;
    }

    pub fn implicit_enable_forbidden(&self) -> bool {
        self.implicit_enable_forbidden
    }

    /// MIT / MissingFeedback apply only to Davout `active_joints` while Active.
    fn filter_mit_to_active(&self, batch: Vec<DavoutMit>) -> Vec<DavoutMit> {
        let active = self.supervisor.active_joints();
        batch
            .into_iter()
            .filter(|cmd| active.contains(&cmd.joint))
            .collect()
    }

    /// Enter position hold with an explicit setpoint (single-joint bench: one angle; multi-joint: joint name required).
    /// Re-arms drives when needed; refuses with [`LoopError::EnableIncomplete`]
    /// until [`Self::enable_completion`] holds.
    pub fn enter_position_hold_at(
        &mut self,
        joint: Option<&str>,
        position_rad: f64,
    ) -> Result<(), LoopError> {
        self.refuse_degraded()?;
        if !position_rad.is_finite() {
            return Err(LoopError::NonFiniteTarget {
                joint: joint
                    .map(str::to_string)
                    .or_else(|| self.joint_names.first().cloned())
                    .unwrap_or_default(),
                value: position_rad,
            });
        }
        let q = self.motion_positions()?;
        if !self.position_hold.is_armed() {
            self.position_hold.arm(&q, &q, self.tick_count);
            for (i, name) in self.joint_names.iter().enumerate() {
                self.position_hold
                    .seed_dq_filter(i, self.joint_velocity(name));
            }
        }
        if self.joint_names.len() == 1 {
            let name = self.joint_names[0].clone();
            self.set_joint_position_setpoint(&name, position_rad)?;
        } else {
            let joint = joint.ok_or(LoopError::JointNameRequired)?;
            self.set_joint_position_setpoint(joint, position_rad)?;
        }
        self.set_control_mode(ControlMode::Position);
        Ok(())
    }

    pub fn supervisor_mut(&mut self) -> &mut Supervisor<B> {
        &mut self.supervisor
    }

    pub fn supervisor(&self) -> &Supervisor<B> {
        &self.supervisor
    }

    /// Current Testing/actuator runtime gain override for `joint`, if any.
    pub fn gain_override(&self, joint_name: &str) -> Option<&GainOverride> {
        self.gains.get(joint_name)
    }

    /// During a degraded episode only `Disabled` is honoured: it stops every
    /// drive (ending the episode); any other mode is ignored (ADR 0038).
    pub fn set_control_mode(&mut self, mode: ControlMode) {
        if self.supervisor.reference_busy() {
            return;
        }
        if let Some(joint) = self.degraded_lost_joint() {
            if mode != ControlMode::Disabled {
                warn!(lost_joint = %joint, ?mode, "control mode change refused during degraded hold");
                return;
            }
            if self.supervisor.degraded_episode().is_some() {
                if let Err(error) = self.supervisor.disable_all() {
                    warn!(%error, "stop during degraded hold not delivered to every drive");
                }
            }
        }
        self.synchronize_stop_generation();
        self.set_control_mode_inner(mode);
    }

    fn refuse_reference_intent(&self, operation: &'static str) -> Result<(), LoopError> {
        if self.supervisor.reference_busy() {
            Err(davout::DavoutError::ReferenceBusy { operation }.into())
        } else {
            Ok(())
        }
    }

    /// Run the offline drive-loss admission (ADR 0038) for every
    /// `on_drive_loss: shed_subtree` joint and install the admitted plans in
    /// Davout. Any refused joint refuses the whole configuration. Without this
    /// call every drive loss stops every drive.
    pub fn admit_drive_loss(&mut self) -> Result<Vec<DriveLossAdmission>, LoopError> {
        let admitted =
            crate::degraded::admit_drive_loss(&self.dynamics, &self.joint_names, &self.supervisor)?;
        self.supervisor
            .install_drive_loss_plans(admitted.iter().map(|a| a.plan.clone()).collect())?;
        Ok(admitted)
    }

    /// The lost joint of a degraded episode in progress (Davout's or Berthier's view).
    pub fn degraded_lost_joint(&self) -> Option<String> {
        self.degraded
            .as_ref()
            .map(|control| control.lost_joint.clone())
            .or_else(|| {
                self.supervisor
                    .degraded_episode()
                    .map(|episode| episode.lost_joint.clone())
            })
    }

    /// The holding joints are lowering to rest.
    pub fn degraded_lowering(&self) -> bool {
        self.degraded
            .as_ref()
            .is_some_and(|control| control.phase == DegradedPhase::Lowering)
    }

    /// Episode transitions since the last call, oldest first.
    pub fn take_degraded_events(&mut self) -> Vec<DegradedEvent> {
        self.observe_degraded_episode();
        std::mem::take(&mut self.degraded_events)
    }

    /// Operator `lower`: start lowering the holding joints to rest at the next tick.
    pub fn request_degraded_lower(&mut self) -> Result<(), LoopError> {
        self.observe_degraded_episode();
        let control = self.degraded.as_mut().ok_or(LoopError::NoDegradedEpisode)?;
        if control.phase == DegradedPhase::Hold {
            control.phase = DegradedPhase::LowerRequested(DegradedLowerCause::Operator);
        }
        Ok(())
    }

    fn refuse_degraded(&self) -> Result<(), LoopError> {
        match self.degraded_lost_joint() {
            Some(joint) => Err(LoopError::DegradedHold { joint }),
            None => Ok(()),
        }
    }

    /// Lower speed cap while an episode runs.
    fn degraded_lower_velocity(&self) -> Option<f64> {
        self.degraded.as_ref()?;
        self.supervisor
            .control
            .control
            .drive_loss
            .as_ref()
            .map(|drive_loss| drive_loss.lower_velocity_rad_s)
    }

    /// Follow Davout: enter the degraded hold when it shed a subtree, report
    /// the end when it stopped every drive.
    fn observe_degraded_episode(&mut self) {
        match (self.supervisor.degraded_episode().is_some(), &self.degraded) {
            (true, None) => self.enter_degraded_hold(),
            (false, Some(_)) => {
                if let Some(control) = self.degraded.take() {
                    let end = self
                        .supervisor
                        .degraded_outcome()
                        .map_or(davout::DegradedEnd::Stopped, |outcome| outcome.end);
                    warn!(lost_joint = %control.lost_joint, end = end.as_str(), "degraded episode ended by Davout");
                    self.degraded_events.push(DegradedEvent::Ended {
                        lost_joint: control.lost_joint,
                        end,
                    });
                }
            }
            _ => {}
        }
    }

    /// Freeze the holding joints in Position hold at their current pose,
    /// aborting any in-flight motion, wave or torque command.
    fn enter_degraded_hold(&mut self) {
        let Some(episode) = self.supervisor.degraded_episode().cloned() else {
            return;
        };
        let index = |name: &String| self.joint_names.iter().position(|n| n == name);
        let shed = episode.shed_joints.iter().filter_map(index).collect();
        let holding = episode.holding_joints.iter().filter_map(index).collect();
        let frozen = episode
            .frozen_positions
            .iter()
            .filter_map(|(joint, q)| index(joint).map(|i| (i, *q)))
            .collect();
        let hold_window = self
            .supervisor
            .control
            .control
            .drive_loss
            .as_ref()
            .and_then(|drive_loss| Duration::try_from_secs_f64(drive_loss.hold_window_s).ok())
            .unwrap_or(Duration::ZERO);
        self.degraded = Some(DegradedControl {
            lost_joint: episode.lost_joint.clone(),
            shed,
            holding,
            frozen,
            auto_lower_at: episode
                .since
                .checked_add(hold_window)
                .unwrap_or(episode.deadline),
            phase: DegradedPhase::Hold,
        });
        self.position_wave = None;
        self.torque_cmds.clear_all();
        let q = self.read_positions();
        self.set_control_mode_inner(ControlMode::Position);
        self.latch_position_from_q(&q);
        warn!(
            lost_joint = %episode.lost_joint,
            shed = ?episode.shed_joints,
            holding = ?episode.holding_joints,
            auto_lower_in = ?hold_window,
            "drive lost: holding the remaining joints in place"
        );
        self.degraded_events.push(DegradedEvent::DriveLost {
            joint: episode.lost_joint,
            shed: episode.shed_joints,
            holding: episode.holding_joints,
            auto_lower_in: hold_window,
        });
    }

    /// Start the lower to rest once the operator asked or the hold window ran out.
    fn advance_degraded_phase(&mut self, q: &[f64]) {
        let Some(control) = self.degraded.as_ref() else {
            return;
        };
        let cause = match control.phase {
            DegradedPhase::Hold if Instant::now() >= control.auto_lower_at => {
                DegradedLowerCause::Timeout
            }
            DegradedPhase::LowerRequested(cause) => cause,
            _ => return,
        };
        if self.control_mode != ControlMode::Position {
            return;
        }
        let holding = control.holding.clone();
        if !self.position_hold.is_armed() {
            self.latch_position_from_q(q);
        }
        for i in holding {
            let rest = self.hold_target_trim(&self.joint_names[i], 0.0);
            self.apply_joint_retarget(i, q[i], rest);
        }
        if let Some(control) = self.degraded.as_mut() {
            control.phase = DegradedPhase::Lowering;
        }
        warn!(cause = cause.as_str(), "degraded lower to rest started");
        self.degraded_events
            .push(DegradedEvent::LowerStarted { cause });
    }

    /// Every holding joint's planner reached its rest target and measured `q`
    /// is within [`DEGRADED_REST_TOLERANCE_RAD`]: Davout stops every drive and
    /// latches the episode's fault.
    fn complete_degraded_lower_at_rest(&mut self, q: &[f64]) -> Result<(), LoopError> {
        let Some(control) = self.degraded.as_ref() else {
            return Ok(());
        };
        if control.phase != DegradedPhase::Lowering {
            return Ok(());
        }
        let (Some(targets), Some(q_traj)) =
            (self.position_hold.targets(), self.position_hold.q_traj())
        else {
            return Ok(());
        };
        let at_rest =
            control
                .holding
                .iter()
                .all(|&i| match (targets.get(i), q_traj.get(i), q.get(i)) {
                    (Some(target), Some(traj), Some(q)) => {
                        (traj - target).abs() <= 1e-6
                            && (q - target).abs() <= DEGRADED_REST_TOLERANCE_RAD
                    }
                    _ => false,
                });
        if !at_rest {
            return Ok(());
        }
        self.degraded = None;
        let stop = self.supervisor.complete_degraded_lower();
        self.discard_motion_intent();
        self.last_stop_generation = self.supervisor.stop_generation();
        warn!("degraded lower complete; every drive stopped");
        self.degraded_events.push(DegradedEvent::LowerComplete);
        stop.map_err(LoopError::from)
    }

    /// Discard retained controller intent before the owner's graceful exit.
    ///
    /// Clears Position/Wave, runtime gains, latched torque and feedback grace
    /// through the existing intent lifecycle. This performs no drive stop writes
    /// and confirms no physical stop; the owner must apply its configured Davout
    /// exit-stop policy before waiting for persistence.
    pub fn inhibit_motion_for_shutdown(&mut self) {
        self.discard_motion_intent();
    }

    fn set_control_mode_inner(&mut self, mode: ControlMode) {
        let previous = self.control_mode;
        // Capture ramp endpoints before mutating mode or clearing overrides.
        let from = {
            let prev_targets = self.target_gains_for_mode(previous);
            self.gains
                .wire_gains_now(previous, &self.joint_names, &prev_targets)
                .unwrap_or_else(|error| {
                    // Unreachable: targets are built from `joint_names`. Without ramp
                    // startpoints the transition uses the YAML targets directly.
                    tracing::error!(%error, "gain ramp startpoints unavailable; no ramp");
                    Vec::new()
                })
        };
        let to = self.target_gains_for_mode(mode);
        if mode != ControlMode::Position {
            self.position_hold.clear();
            self.last_position_diag = None;
            self.position_wave = None;
        }
        if previous == ControlMode::TorqueOnly && mode != ControlMode::TorqueOnly {
            self.torque_cmds.on_leave_torque_only();
        }
        self.control_mode = mode;
        self.supervisor.set_control_mode(mode);
        if previous != mode {
            info!(?previous, ?mode, "control mode transition");
        }
        let arm_ramp = mode != ControlMode::Disabled && previous != ControlMode::Disabled;
        self.gains.on_mode_enter(previous, mode, &from, &to);
        if arm_ramp {
            // Mode changes slew from measured torque within the current hard cap.
            // Enable/disable resets the limiter; an unseeded output starts at zero.
            self.supervisor.seed_tau_ff_rate_limiter();
        }
    }

    /// Enter [`ControlMode::TorqueOnly`] with `τ_cmd ≡ 0` (operator `gravity-off`).
    ///
    /// Clears any prior latch before the mode transition so a same-mode
    /// `gravity-off` after `torque-cmd` still yields true no-FF.
    pub fn enter_torque_only_zero(&mut self) {
        self.torque_cmds.clear_all();
        self.set_control_mode(ControlMode::TorqueOnly);
    }

    /// Latch a per-joint open-loop torque command for [`ControlMode::TorqueOnly`].
    ///
    /// Enters TorqueOnly when not already there. Values persist until cleared or
    /// until leaving TorqueOnly. Default when unset is 0. Rejects unknown joints
    /// and non-finite values.
    pub fn set_torque_cmd(&mut self, joint_name: &str, tau_nm: f64) -> Result<(), LoopError> {
        self.refuse_degraded()?;
        self.refuse_reference_intent("torque intent")?;
        self.synchronize_stop_generation();
        if !self.joint_names.iter().any(|n| n == joint_name) {
            return Err(LoopError::UnknownJoint {
                joint: joint_name.to_string(),
            });
        }
        if !tau_nm.is_finite() {
            return Err(LoopError::NonFiniteTorqueCmd {
                joint: joint_name.to_string(),
            });
        }
        self.supervisor.check_fault_authority()?;
        if self.control_mode != ControlMode::TorqueOnly {
            self.set_control_mode(ControlMode::TorqueOnly);
        }
        self.torque_cmds.set(joint_name, tau_nm);
        Ok(())
    }

    /// Latched `τ_cmd` for a joint, or `0.0` when unset.
    pub fn torque_cmd(&self, joint_name: &str) -> f64 {
        self.torque_cmds.get(joint_name)
    }

    /// Target (kp, kd) per joint from YAML (`gravity_comp` / `impedance`).
    fn target_gains_for_mode(&self, mode: ControlMode) -> Vec<(f64, f64)> {
        const ZERO: ModeGains = ModeGains {
            kp: 0.0,
            kd: 0.0,
            ki: 0.0,
        };
        self.joint_names
            .iter()
            .map(|name| {
                let cfg = self.supervisor.control.control.joints.get(name);
                target_gains_from_yaml(
                    mode,
                    cfg.map(|c| &c.gravity_comp).unwrap_or(&ZERO),
                    cfg.map(|c| &c.impedance).unwrap_or(&ZERO),
                )
            })
            .collect()
    }

    pub fn control_mode(&self) -> ControlMode {
        self.control_mode
    }

    pub fn joint_names(&self) -> &[String] {
        &self.joint_names
    }

    pub fn loop_period(&self) -> Duration {
        self.loop_period
    }

    pub fn configured_loop_hz(&self) -> u32 {
        self.loop_hz
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    // -- Gain override API -------------------------------------------

    /// Apply a per-joint gain override, clamped to motor-type safety limits.
    ///
    /// Refused ([`LoopError::GainOverrideNotApplicable`]) under GravityComp /
    /// TorqueOnly / Disabled: Testing cannot stash stiffness that would snap
    /// back on Impedance/Position enter, and the caller must not be told Ok for
    /// a gain that was dropped (L-berthier-10).
    pub fn apply_gain_override(
        &mut self,
        joint_name: &str,
        gain_override: GainOverride,
    ) -> Result<(), LoopError> {
        self.refuse_degraded()?;
        self.refuse_reference_intent("gain override")?;
        self.validate_gain_override(joint_name, &gain_override)?;
        self.require_gain_mode(joint_name)?;
        let limits = self.clamp_limits_for(joint_name);
        self.gains
            .apply(self.control_mode, joint_name, gain_override, limits);
        Ok(())
    }

    fn require_gain_mode(&self, joint: &str) -> Result<(), LoopError> {
        if mode_allows_gain_override(self.control_mode) {
            Ok(())
        } else {
            Err(LoopError::GainOverrideNotApplicable {
                joint: joint.to_owned(),
                mode: self.control_mode,
            })
        }
    }

    fn validate_gain_override(&self, joint: &str, gains: &GainOverride) -> Result<(), LoopError> {
        if !self.joint_names.iter().any(|name| name == joint) {
            return Err(LoopError::UnknownJoint {
                joint: joint.to_owned(),
            });
        }
        for (field, value) in [
            ("kp", gains.kp),
            ("kd", gains.kd),
            ("ki", gains.ki),
            ("fc", gains.fc),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(LoopError::InvalidGainOverride {
                    joint: joint.to_owned(),
                    field,
                });
            }
        }
        Ok(())
    }

    /// Remove the gain override for a single joint, reverting to config gains.
    pub fn clear_gain_override(&mut self, joint_name: &str) {
        self.gains.clear(joint_name);
    }

    /// Resolve [MotorTypeDefaults] for a joint from the control config.
    fn resolve_motor_type_defaults(&self, joint_name: &str) -> Option<&MotorTypeDefaults> {
        let joint_cfg = self.supervisor.control.control.joints.get(joint_name)?;
        let key = motor_type_key(joint_cfg.motor_type);
        self.supervisor.control.control.motor_type_defaults.get(key)
    }

    fn clamp_limits_for(&self, joint_name: &str) -> GainClampLimits {
        let defaults = self.resolve_motor_type_defaults(joint_name);
        GainClampLimits {
            kp_max: defaults.map(|d| d.kp_max).unwrap_or(f64::MAX),
            kd_max: defaults.map(|d| d.kd_max).unwrap_or(f64::MAX),
            tau_ff_max: defaults.map(|d| d.tau_ff_max_nm).unwrap_or(f64::MAX),
        }
    }

    fn joint_mode_gains_yaml(
        &self,
        missing_impedance: &'static ModeGains,
    ) -> Vec<JointModeGains<'_>> {
        static ZERO_STATIC: ModeGains = ModeGains {
            kp: 0.0,
            kd: 0.0,
            ki: 0.0,
        };
        self.joint_names
            .iter()
            .map(|name| {
                let cfg = self.supervisor.control.control.joints.get(name);
                JointModeGains {
                    gravity_comp: cfg.map(|c| &c.gravity_comp).unwrap_or(&ZERO_STATIC),
                    impedance: cfg.map(|c| &c.impedance).unwrap_or(missing_impedance),
                }
            })
            .collect()
    }

    /// One control cycle: recv → compute → send → optional Chappe publish.
    pub fn tick(&mut self, chappe: Option<&Bus>) -> Result<(), LoopError> {
        let tick_start = Instant::now();
        self.synchronize_stop_generation();
        let result = self.tick_inner(chappe);
        // One timestamp read and compare per tick; no allocation (M06).
        if tick_start.elapsed() > self.loop_period {
            self.tick_overruns = self.tick_overruns.saturating_add(1);
        }
        if let Err(error) = &result {
            match error {
                LoopError::MissingFeedback { joint } => self.supervisor.latch_control_fault(
                    "current-enable feedback missing after neutral bootstrap",
                    Some(joint),
                ),
                LoopError::AscentStall { joint, .. } => self.supervisor.latch_control_fault(
                    "position ascent stalled without measured progress",
                    Some(joint),
                ),
                LoopError::HoldTracking { joint, .. } => self.supervisor.latch_control_fault(
                    "position hold off target with opposing commanded torque and no progress",
                    Some(joint),
                ),
                LoopError::WaveStall { joint, .. } => self.supervisor.latch_control_fault(
                    "position wave stalled without measured motion",
                    Some(joint),
                ),
                // Internal invariant breaks: the loop can no longer compute a command it can
                // vouch for, and every later tick would fail the same way before any MIT or
                // keepalive is sent. Latch and discard instead of waiting for a watchdog.
                LoopError::MissingSetpoint { .. } | LoopError::HoldLenMismatch => {
                    self.supervisor.latch_control_fault(
                        "position hold lost its setpoint latch (controller invariant)",
                        None,
                    )
                }
                LoopError::Dynamics(_) => self.supervisor.latch_control_fault(
                    "gravity model failed on the control tick (controller invariant)",
                    None,
                ),
                LoopError::GainShape(_) => self.supervisor.latch_control_fault(
                    "gain resolution inputs not parallel to joints (controller invariant)",
                    None,
                ),
                _ => {}
            }
            if matches!(
                error,
                LoopError::Safety(_)
                    | LoopError::GainShape(_)
                    | LoopError::Dynamics(_)
                    | LoopError::MissingSetpoint { .. }
                    | LoopError::HoldLenMismatch
                    | LoopError::MissingFeedback { .. }
                    | LoopError::AscentStall { .. }
                    | LoopError::HoldTracking { .. }
                    | LoopError::WaveStall { .. }
            ) {
                self.discard_motion_intent();
                self.last_stop_generation = self.supervisor.stop_generation();
            }
        }
        // A stop (deadline, fault, second loss) may have ended Davout's episode.
        self.observe_degraded_episode();
        result
    }

    fn tick_inner(&mut self, chappe: Option<&Bus>) -> Result<(), LoopError> {
        let mut phase = TickPhaseSample::default();
        let mut t = Instant::now();

        if self.supervisor.reference_work_pending() {
            self.discard_motion_intent();
            self.supervisor
                .advance_reference_work()
                .map_err(|error| DavoutError::Homing {
                    message: error.to_string(),
                })?;
            self.last_stop_generation = self.supervisor.stop_generation();
            self.last_enable_session = None;
            (phase.feedback_us, t) = phase_elapsed_us(t);
            // The transaction owns this tick's sole receive acquisition and
            // cleanup. Its terminal is retained by Davout, including failures;
            // returning Ok prevents a runtime fallback from duplicating stops.
            let q = self.read_positions();
            if let Some(bus) = chappe {
                let now = Instant::now();
                if self
                    .last_chappe
                    .map(|last| now.duration_since(last) >= self.chappe_publish_period)
                    .unwrap_or(true)
                {
                    self.publish_robot_state_counted(bus, &q);
                    self.last_chappe = Some(now);
                    phase.chappe_us = phase_elapsed_us(t).0;
                }
            }
            self.tick_phase.record(phase);
            self.tick_count += 1;
            return Ok(());
        }

        self.supervisor.begin_tick_feedback();
        self.supervisor.drain_feedback()?;
        (phase.feedback_us, t) = phase_elapsed_us(t);
        // Davout may have shed a lost drive's subtree in that drain or in the
        // last batch's admission (ADR 0038).
        self.observe_degraded_episode();

        let mut q = self.read_positions();

        let operational_mode = self.supervisor.mode();
        let enable_session = self.supervisor.enable_session_started_at();
        if enable_session.is_some() && enable_session != self.last_enable_session {
            // A disable/re-enable can occur without any tick observing Disabled.
            self.active_feedback_grace_ticks = 2;
        }
        self.last_enable_session = enable_session;

        let needs_joint_feedback = operational_mode == OperationalMode::Active
            && self.control_mode != ControlMode::Disabled;
        // No per-tick allocation: iterate the live set twice instead of
        // collecting it (L-berthier-08).
        let mut all_have_feedback = self
            .supervisor
            .active_joints()
            .iter()
            .all(|name| self.has_joint_feedback(name));
        if needs_joint_feedback && !all_have_feedback && self.active_feedback_grace_ticks == 0 {
            // A pose read stale here and a grant judged live in the drain count
            // the same unanswered solicit a moment apart: judge the grants now,
            // so a qualifying drive loss sheds its subtree (ADR 0038) instead of
            // failing the tick on its missing pose.
            self.supervisor.check_active_references()?;
            self.observe_degraded_episode();
            q = self.read_positions();
            all_have_feedback = self
                .supervisor
                .active_joints()
                .iter()
                .all(|name| self.has_joint_feedback(name));
        }
        // First tick (or first ticks after enable) may run before CAN status arrives.
        let feedback_bootstrap =
            needs_joint_feedback && !all_have_feedback && self.active_feedback_grace_ticks > 0;
        if needs_joint_feedback && !feedback_bootstrap {
            for name in self.supervisor.active_joints() {
                if !self.has_joint_feedback(name) {
                    return Err(LoopError::MissingFeedback {
                        joint: name.clone(),
                    });
                }
            }
        }
        if needs_joint_feedback && !feedback_bootstrap {
            // The gravity model couples every URDF joint: a silent peer is not q = 0.
            // Fail closed rather than biasing τ_g for the scoped Active joints.
            for (index, joint) in self.joint_names.iter().enumerate() {
                // A shed joint's angle is frozen in `q` (ADR 0038).
                let frozen = self
                    .degraded
                    .as_ref()
                    .is_some_and(|control| control.is_shed(index));
                if !frozen && !self.has_joint_feedback(joint) {
                    return Err(LoopError::MissingFeedback {
                        joint: joint.clone(),
                    });
                }
            }
        }
        if self.active_feedback_grace_ticks > 0 {
            if all_have_feedback {
                self.active_feedback_grace_ticks = 0;
            } else if !self.supervisor.enable_writes_pending() {
                // A target whose staggered Enable is not yet written cannot
                // have session pose; the grace counts from the last write.
                self.active_feedback_grace_ticks -= 1;
            }
        }
        self.advance_degraded_phase(&q);

        if self.supervisor.mode() == OperationalMode::Active {
            if self.control_mode != ControlMode::Disabled && !feedback_bootstrap {
                let tau_g = self.dynamics.gravity_torques(&q)?;
                (phase.gravity_us, t) = phase_elapsed_us(t);

                let log_position_diag = self.control_mode == ControlMode::Position
                    && self
                        .last_position_diag
                        .map(|t| t.elapsed() >= Duration::from_secs(1))
                        .unwrap_or(true);
                let mut batch = Vec::new();
                let mut position_diag: Option<Vec<HoldJointDiag>> = None;

                if self.control_mode == ControlMode::Position {
                    static POSITION_DEFAULT_IMPEDANCE: ModeGains = ModeGains {
                        kp: 20.0,
                        kd: 1.0,
                        ki: 0.0,
                    };
                    let yaml = self.joint_mode_gains_yaml(&POSITION_DEFAULT_IMPEDANCE);
                    let resolved =
                        self.gains
                            .resolve_all(self.control_mode, &self.joint_names, &yaml)?;
                    let dq_meas: Vec<f64> = self
                        .joint_names
                        .iter()
                        .map(|name| self.joint_velocity(name))
                        .collect();
                    // During a degraded episode every joint moves at most at the
                    // configured lower speed (ADR 0038).
                    let degraded_velocity = self.degraded_lower_velocity();
                    let joint_params: Vec<HoldJointParams> = self
                        .joint_names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let r = &resolved[i];
                            let mut friction = cfg.map(|c| c.friction.clone());
                            if let Some(fc) = r.law_fc {
                                if let Some(ref mut f) = friction {
                                    f.fc = fc;
                                }
                            }
                            let law = match cfg {
                                Some(c) if c.position_law == PositionLaw::ScaledPd => {
                                    HoldLaw::ScaledPd(ScaledPdGains {
                                        kd: r.wire_kd,
                                        e0: c.time_scale_e0_rad(),
                                        e1: c.time_scale_e1_rad(),
                                        integral_band: c.integral_band_rad(),
                                        integral_leak_s: c.integral_leak_s(),
                                        inertia: self.reference_inertia[i],
                                        friction: Some(ReferenceFriction::from_gains(
                                            &c.friction,
                                            r.law_fc,
                                        )),
                                    })
                                }
                                _ => HoldLaw::Legacy,
                            };
                            HoldJointParams {
                                // The torque the law judges (fuses, evidence, diag) is the
                                // torque on the wire: override > ramp > YAML.
                                kp: r.wire_kp,
                                kd: r.law_kd,
                                ki: r.law_ki,
                                max_lead: cfg.map(|c| c.position_slew_max_lead_rad).unwrap_or(0.15),
                                vel_deadband: cfg
                                    .map(|c| c.position_trajectory_velocity_deadband_rad)
                                    .unwrap_or(0.02),
                                advance_max_lead: cfg
                                    .map(|c| c.position_slew_max_lead_rad)
                                    .unwrap_or(ADVANCE_MAX_LEAD_DEFAULT),
                                advance_vel_deadband: cfg
                                    .map(|c| c.position_trajectory_velocity_deadband_rad)
                                    .unwrap_or(POSITION_HOLD_ERROR_DEADBAND_RAD),
                                slew_rad_s: cfg.map(|c| c.position_slew_rad_s).unwrap_or(0.25),
                                trajectory_v_max: cfg
                                    .map(|c| c.position_trajectory_velocity_rad_s)
                                    .unwrap_or(0.30),
                                trajectory_threshold_rad: cfg
                                    .map(|c| c.position_trajectory_threshold_rad)
                                    .unwrap_or(0.15),
                                a_max: cfg
                                    .map(|c| c.position_trajectory_accel_rad_s2)
                                    .unwrap_or(0.20),
                                velocity_cap: match (
                                    self.supervisor.joint_velocity_cap(name),
                                    degraded_velocity,
                                ) {
                                    (Some(cap), Some(lower)) => Some(cap.min(lower)),
                                    (cap, None) => cap,
                                    (None, lower) => lower,
                                },
                                friction,
                                limit_policy: self.supervisor.joint_limit_policy(name).cloned(),
                                tau_meas: self.joint_torque(name),
                                law,
                                descent_cap: DescentCap::from_zones(
                                    &self.supervisor.control.control.danger_zones,
                                    name,
                                ),
                            }
                        })
                        .collect();
                    let hold_out = {
                        for (i, joint) in self.joint_names.iter().enumerate() {
                            self.position_hold
                                .set_commanded_joint(i, self.supervisor.joint_drive_active(joint));
                        }
                        let world = HoldWorld {
                            q: &q,
                            dq_meas: &dq_meas,
                            tau_g: tau_g.as_slice(),
                            joints: &joint_params,
                            joint_names: &self.joint_names,
                            dt: self.loop_period.as_secs_f64(),
                            hz: self.loop_hz,
                            tick_count: self.tick_count,
                            wave: &mut self.position_wave,
                        };
                        self.position_hold.tick(world)?
                    };
                    (phase.planner_us, t) = phase_elapsed_us(t);
                    for cmd in hold_out.mit {
                        // Compose already carries the wire `kp` (see `HoldJointParams.kp`)
                        // and owns `kd` (`kd_mit`, usually 0).
                        batch.push(cmd);
                    }
                    for (i, d) in hold_out.diag.iter().enumerate() {
                        let name = &self.joint_names[i];
                        if log_position_diag {
                            info!(
                                joint = %name,
                                q = d.q,
                                dq = d.dq_raw,
                                dq_filt = d.dq_filt,
                                q_traj = d.q_traj,
                                dq_traj = d.dq_traj,
                                q_des = d.q_des,
                                target = d.target,
                                lead = d.lead,
                                lead_sat = d.lead_sat,
                                settle_error = d.settle_error,
                                settling = d.settling,
                                friction_mode = d.friction_mode,
                                retarget_age_ms = d.retarget_age_ms,
                                joint_stuck = d.joint_stuck,
                                planner_frozen = d.planner_frozen,
                                ascent_stall_ms = d.ascent_stall_ms,
                                hold_tracking_ms = d.hold_tracking_ms,
                                planner_event = d.planner_event.as_str(),
                                phase = %d.phase,
                                kp = d.kp,
                                kd = d.kd,
                                tau_g = d.tau_g,
                                tau_f = d.tau_f,
                                tau_d = d.tau_d,
                                tau_ff_cmd = d.tau_ff_cmd,
                                tau_meas = d.tau_meas,
                                tau_err = d.tau_meas - d.tau_ff_cmd,
                                tau_p = d.tau_p,
                                dq_mit = d.mit_velocity,
                                kp_mit = d.kp,
                                kd_mit = d.mit_kd,
                                law = d.law.as_str(),
                                time_scale = d.time_scale,
                                tau_i = d.tau_i,
                                move_dist = d.move_dist,
                                v_max_eff = d.v_max_eff,
                                "position hold command"
                            );
                        }
                        if should_log_position_onset(d.retarget_tick, self.tick_count, self.loop_hz)
                        {
                            debug!(
                                joint = %name,
                                retarget_age_ms = d.retarget_age_ms,
                                q = d.q,
                                dq = d.dq_raw,
                                dq_filt = d.dq_filt,
                                q_traj = d.q_traj,
                                dq_traj = d.dq_traj,
                                lead = d.lead,
                                settle_error = d.settle_error,
                                settling = d.settling,
                                friction_mode = d.friction_mode,
                                tau_f = d.tau_f,
                                tau_d = d.tau_d,
                                tau_ff_cmd = d.tau_ff_cmd,
                                phase = %d.phase,
                                joint_stuck = d.joint_stuck,
                                planner_frozen = d.planner_frozen,
                                move_dist = d.move_dist,
                                v_max_eff = d.v_max_eff,
                                "position hold onset"
                            );
                        }
                    }
                    if log_position_diag {
                        self.last_position_diag = Some(Instant::now());
                    }
                    position_diag = Some(hold_out.diag);
                } else {
                    (phase.planner_us, t) = phase_elapsed_us(t);
                    static ZERO_IMPEDANCE: ModeGains = ModeGains {
                        kp: 0.0,
                        kd: 0.0,
                        ki: 0.0,
                    };
                    let yaml = self.joint_mode_gains_yaml(&ZERO_IMPEDANCE);
                    let resolved =
                        self.gains
                            .resolve_all(self.control_mode, &self.joint_names, &yaml)?;
                    let ff_joints: Vec<MitFfJointIn> = self
                        .joint_names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| {
                            let cfg = self.supervisor.control.control.joints.get(name);
                            let r = &resolved[i];
                            MitFfJointIn {
                                name: name.clone(),
                                q: q[i],
                                dq: self.joint_velocity(name),
                                tau_g: tau_g[i],
                                tau_cmd: self.torque_cmd(name),
                                friction: cfg.map(|c| c.friction.clone()),
                                wire_kp: r.wire_kp,
                                wire_kd: r.wire_kd,
                                fc: r.law_fc,
                            }
                        })
                        .collect();
                    batch = MitFeedforward::compose(self.control_mode, &ff_joints);
                }
                (phase.compose_us, t) = phase_elapsed_us(t);

                let batch = self.filter_mit_to_active(batch);
                self.supervisor.send_mit_batch(batch)?;
                (phase.send_us, t) = phase_elapsed_us(t);
                // After the send, so the row carries the τ_ff Davout actually sent.
                if let Some(diag) = position_diag.as_deref() {
                    self.record_position_trace(diag);
                    (phase.trace_us, t) = phase_elapsed_us(t);
                }
                self.supervisor.drain_feedback()?;
                self.gains.advance_tick();
                self.complete_degraded_lower_at_rest(&q)?;
            } else {
                // Robstride only streams status after MIT frames; hold current q with zero
                // gains/torque between enable and the first fresh pose. Bootstrap must
                // never calculate gravity or PD output from missing or prior-session q.
                // Scoped Enable: keepalive only for Davout active_joints.
                let active = self.supervisor.active_joints();
                let batch: Vec<DavoutMit> = self
                    .joint_names
                    .iter()
                    .zip(q.iter())
                    .filter(|(name, _)| active.contains(*name))
                    .map(|(name, &position_rad)| {
                        let policy = self.supervisor.joint_limit_policy(name).ok_or_else(|| {
                            LoopError::UnknownJoint {
                                joint: name.clone(),
                            }
                        })?;
                        // Missing current-session q falls back to zero, which
                        // need not lie in a valid taught range. With zero gains,
                        // this bounded target remains an inert status solicit.
                        Ok(DavoutMit {
                            joint: name.clone(),
                            kp: 0.0,
                            kd: 0.0,
                            position_rad: position_rad
                                .clamp(policy.hard_lower(), policy.hard_upper()),
                            velocity_rad_s: 0.0,
                            torque_ff_nm: 0.0,
                        })
                    })
                    .collect::<Result<_, LoopError>>()?;
                (phase.compose_us, t) = phase_elapsed_us(t);
                self.supervisor.send_mit_batch(batch)?;
                (phase.send_us, t) = phase_elapsed_us(t);
                self.supervisor.drain_feedback()?;
            }
        }

        if let Some(bus) = chappe {
            let now = Instant::now();
            if self
                .last_chappe
                .map(|t| now.duration_since(t) >= self.chappe_publish_period)
                .unwrap_or(true)
            {
                self.publish_robot_state_counted(bus, &q);
                self.last_chappe = Some(now);
                phase.chappe_us = phase_elapsed_us(t).0;
            }
        }

        self.tick_phase.record(phase);
        self.tick_count += 1;
        Ok(())
    }

    /// Poll bus feedback, then read cached joint positions for planner init.
    fn refresh_joint_positions(&mut self) -> Result<Vec<f64>, LoopError> {
        self.synchronize_stop_generation();
        if let Err(error) = self
            .supervisor
            .drain_feedback()
            .and_then(|_| self.supervisor.check_fault_authority())
        {
            self.discard_motion_intent();
            self.last_stop_generation = self.supervisor.stop_generation();
            return Err(error.into());
        }
        Ok(self.read_positions())
    }

    fn discard_motion_intent(&mut self) {
        self.set_control_mode_inner(ControlMode::Disabled);
        self.torque_cmds.clear_all();
        self.active_feedback_grace_ticks = 0;
    }

    fn synchronize_stop_generation(&mut self) {
        let generation = self.supervisor.stop_generation();
        if generation != self.last_stop_generation {
            self.discard_motion_intent();
            self.last_stop_generation = generation;
        }
    }

    fn has_joint_feedback(&self, joint: &str) -> bool {
        self.supervisor.joint_feedback(joint).is_some()
    }

    /// Measured `q`; a shed joint reads its frozen angle during an episode.
    fn read_positions(&self) -> Vec<f64> {
        let mut q: Vec<f64> = self
            .joint_names
            .iter()
            .map(|name| {
                self.supervisor
                    .joint_feedback(name)
                    .map(|s| s.position_rad)
                    .unwrap_or(0.0)
            })
            .collect();
        if let Some(control) = &self.degraded {
            for &(index, frozen) in &control.frozen {
                if let Some(slot) = q.get_mut(index) {
                    *slot = frozen;
                }
            }
        }
        q
    }

    fn joint_velocity(&self, joint: &str) -> f64 {
        self.supervisor
            .joint_feedback(joint)
            .map(|s| s.velocity_rad_s)
            .unwrap_or(0.0)
    }

    fn joint_torque(&self, joint: &str) -> f64 {
        self.supervisor
            .joint_feedback(joint)
            .map(|s| s.torque_nm)
            .unwrap_or(0.0)
    }

    fn publish_robot_state(&self, chappe: &Bus, q: &[f64]) -> Result<(), LoopError> {
        // Presence = fresh CAN feedback. Davout omits stale free-drive samples
        // (FREE_DRIVE_FEEDBACK_TTL) so Consul Online tracks recent RX, not sticky cache.
        let joints: Vec<JointState> = self
            .joint_names
            .iter()
            .zip(q.iter())
            .filter_map(|(name, &position)| {
                let state = self.supervisor.joint_feedback(name)?;
                let (homing_state, drive_active, out_of_limits) =
                    self.supervisor.joint_commissioning_wire(name);
                Some(JointState {
                    name: name.clone(),
                    position,
                    velocity: state.velocity_rad_s,
                    effort: state.torque_nm,
                    temperature_c: state.temperature_c,
                    fault: u32::from(state.fault),
                    homing_state,
                    drive_active,
                    out_of_limits,
                    sample_age_ms: state.sample_age.as_millis().min(u128::from(u64::MAX)) as u64,
                })
            })
            .collect();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let state = RobotState {
            timestamp_ms,
            joints,
        };
        chappe.publish("robot/state", "berthier", "marengo.v1.RobotState", &state)?;
        Ok(())
    }

    /// Tick publish that never fails the tick (L-berthier-24): a telemetry
    /// error is counted and logged, and motion continues.
    fn publish_robot_state_counted(&mut self, chappe: &Bus, q: &[f64]) {
        let outcome = self.publish_robot_state(chappe, q);
        self.note_telemetry_outcome(outcome);
    }

    fn note_telemetry_outcome(&mut self, outcome: Result<(), LoopError>) {
        if let Err(error) = outcome {
            self.telemetry_failures = self.telemetry_failures.saturating_add(1);
            warn!(error = %error, "chappe telemetry publish failed; motion continues");
        }
    }

    /// Preview gravity torques without sending (for motor-repl status).
    pub fn preview_gravity_torques(&self, q: &[f64]) -> Result<Vec<f64>, LoopError> {
        Ok(self.dynamics.gravity_torques(q)?.into_inner())
    }

    /// Read-only access to the dynamics model (for pre-flight saturation checks).
    pub fn dynamics_model(&self) -> &dyn DynamicsModel {
        &self.dynamics
    }

    /// Test-only: planner `(q_traj, dq_traj)` for replay assertions.
    #[cfg(test)]
    pub fn test_planner_state(&self, joint: &str) -> Option<(f64, f64)> {
        let i = self.joint_names.iter().position(|n| n == joint)?;
        self.position_hold.planner_state(i)
    }

    /// Test-only: force Hold at `q_traj` (residual lead-follow / AscentStall scenarios).
    #[cfg(test)]
    pub fn test_force_planner_hold_at(
        &mut self,
        joint: &str,
        q_traj: f64,
    ) -> Result<(), LoopError> {
        let i = self
            .joint_names
            .iter()
            .position(|n| n == joint)
            .ok_or_else(|| LoopError::UnknownJoint {
                joint: joint.to_string(),
            })?;
        self.position_hold.force_planner_hold_at(i, q_traj)?;
        Ok(())
    }
}

/// High-rate onset logs for the first window after retarget (`MARENGO_POSITION_ONSET_LOG_MS`, default 250).
fn should_log_position_onset(retarget_tick: Option<u64>, tick: u64, loop_hz: u32) -> bool {
    static ONSET_MS: OnceLock<u64> = OnceLock::new();
    let onset_ms = *ONSET_MS.get_or_init(|| {
        std::env::var("MARENGO_POSITION_ONSET_LOG_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(250)
    });
    let Some(retarget_tick) = retarget_tick else {
        return false;
    };
    let age_ticks = tick.saturating_sub(retarget_tick);
    let age_ms = age_ticks.saturating_mul(1000) / u64::from(loop_hz.max(1));
    if age_ms > onset_ms {
        return false;
    }
    let decimate = u64::from(loop_hz.max(1) / 20).max(1);
    age_ticks % decimate == 0
}

/// Map Davout mode to protobuf (for logging / Consul).
pub fn proto_control_mode(mode: ControlMode) -> ProtoControlMode {
    match mode {
        ControlMode::Disabled => ProtoControlMode::Disabled,
        ControlMode::GravityComp => ProtoControlMode::GravityComp,
        ControlMode::Impedance => ProtoControlMode::Impedance,
        ControlMode::Position => ProtoControlMode::Position,
        ControlMode::TorqueOnly => ProtoControlMode::TorqueOnly,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::approx_constant, clippy::expect_used)]

    use super::*;
    use crate::position_hold::POSITION_ASCENT_STALL_FAULT_MS;
    use crate::position_setpoint::{
        apply_lead_follow_hold_short, clamp_trajectory_setpoint, descent_breakaway_confirmed,
        descent_stuck_mit_pull, lead_follow_stuck_residual, planner_drifted_from_measurement,
        planner_overshoot_hold_while_moving, planner_premature_hold,
        planner_should_freeze_on_descent, planner_should_latch_on_overshoot_hold,
        planner_should_lead_follow_hold_short, planner_should_recover_ascent_stall,
        planner_should_reopen_premature_hold, planner_should_resync_stuck_lead,
        position_hold_effective_max_lead, position_hold_mit_kd, position_hold_mit_velocity,
        reopen_planner_from_premature_hold, POSITION_SETTLE_TOLERANCE_RAD,
    };
    use crate::position_trajectory::{JointPositionPlanner, TrapezoidPhase};
    use crate::test_support::{queue_all_status, queue_joint_status};
    use armee_kinematics::JointLimitPolicy;
    use davout::simulation::{InitialVirtualReference, SimulationBus};
    use davout::{MemoryBus, OperationalMode};

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn test_loop() -> ControlLoop<SimulationBus> {
        ControlLoop::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
            200,
            50,
        )
        .expect("declared virtual initial reference")
    }

    fn virtual_ready_active(loop_ctrl: &mut ControlLoop<SimulationBus>) {
        virtual_ready_active_at(loop_ctrl, None);
    }

    fn virtual_ready_active_at(
        loop_ctrl: &mut ControlLoop<SimulationBus>,
        initial: Option<(&str, f64, f64)>,
    ) {
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("ready");
        let joints: Vec<String> = loop_ctrl
            .supervisor()
            .motors
            .motors
            .iter()
            .map(|motor| motor.joint.clone())
            .collect();
        loop_ctrl
            .supervisor_mut()
            .enable_targets(&joints)
            .expect("enable");
        queue_all_status(loop_ctrl.supervisor_mut(), initial);
        loop_ctrl
            .supervisor_mut()
            .drain_feedback()
            .expect("raw initial observations");
    }

    fn replay_feedback(
        loop_ctrl: &mut ControlLoop<SimulationBus>,
        joint: &str,
        position: f32,
        velocity: f32,
    ) {
        // Every enabled joint receives a fresh raw observation through the real
        // receive/validation path. This never grants or writes a pose cache.
        queue_all_status(
            loop_ctrl.supervisor_mut(),
            Some((joint, f64::from(position), f64::from(velocity))),
        );
        loop_ctrl
            .supervisor_mut()
            .drain_feedback()
            .expect("finite raw observations");
    }

    #[test]
    fn enable_requires_verified_homing() {
        let mut loop_ctrl = ControlLoop::from_repo(repo_root(), MemoryBus::default(), 200, 50)
            .expect("ordinary unreferenced loop");
        let joints: Vec<String> = loop_ctrl
            .supervisor()
            .motors
            .motors
            .iter()
            .map(|motor| motor.joint.clone())
            .collect();
        let err = loop_ctrl
            .supervisor_mut()
            .enable_targets(&joints)
            .expect_err("enable without homing");
        assert!(matches!(err, DavoutError::Homing { .. }));
    }

    #[test]
    fn tick_sends_no_mit_when_supervisor_not_active() {
        let mut loop_ctrl = test_loop();
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("ready");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Ready);
        // Stay Ready (do not call enter_position_hold*, which re-arms Active).
        loop_ctrl.supervisor_mut().bus_mut().clear_trace();
        loop_ctrl.tick(None).expect("tick");
        assert!(loop_ctrl
            .supervisor()
            .bus()
            .frames()
            .iter()
            .all(|frame| (frame.id >> 24) & 0x1f != 1));
    }

    #[test]
    fn telemetry_failure_is_counted_not_propagated() {
        // L-berthier-24: a Chappe publish error must not become a tick error.
        let mut loop_ctrl = test_loop();
        assert_eq!(loop_ctrl.telemetry_failures(), 0);
        loop_ctrl.note_telemetry_outcome(Err(LoopError::Chappe(chappe::BusError::Publish(
            "receiver race".to_string(),
        ))));
        assert_eq!(loop_ctrl.telemetry_failures(), 1);
        loop_ctrl.note_telemetry_outcome(Ok(()));
        assert_eq!(loop_ctrl.telemetry_failures(), 1);
    }

    #[test]
    fn chappe_telemetry_never_fails_the_tick() {
        // Publish errors are unreachable through `Bus` today (no-receiver
        // send succeeds); the tick must stay `Ok` with subscriber churn, and
        // the counter stays put while state is actually published.
        let mut loop_ctrl = test_loop();
        let bus = Bus::default();
        let mut state_rx = bus.subscribe("robot/state");
        for _ in 0..3 {
            loop_ctrl.tick(Some(&bus)).expect("tick with telemetry");
            // Churn subscribers between ticks: dropping the last receiver
            // must not fail a later publish.
            let churn = bus.subscribe("robot/state");
            drop(churn);
        }
        assert_eq!(loop_ctrl.telemetry_failures(), 0);
        assert!(
            state_rx.try_recv().is_ok(),
            "robot/state must be published from the tick"
        );
    }

    #[test]
    fn tick_overruns_count_ticks_slower_than_period() {
        // M06: one timestamp read per tick, no allocation; a 1 ns period is
        // always exceeded, so every tick overruns deterministically.
        let mut loop_ctrl = ControlLoop::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
            1_000_000_000,
            50,
        )
        .expect("tiny-period loop");
        assert_eq!(loop_ctrl.tick_overruns(), 0);
        loop_ctrl.tick(None).expect("overrun tick still runs");
        loop_ctrl.tick(None).expect("overrun tick still runs");
        assert_eq!(loop_ctrl.tick_overruns(), 2);
    }

    #[test]
    fn nominal_ticks_do_not_count_overruns() {
        // A 1 Hz period: one simulated tick is nominal even on an emulated,
        // contended test host, so this asserts the counter, not host speed.
        let mut loop_ctrl = ControlLoop::from_simulation(
            repo_root(),
            SimulationBus::default(),
            InitialVirtualReference::AllConfigured,
            1,
            50,
        )
        .expect("slow-period loop");
        loop_ctrl.tick(None).expect("tick");
        assert_eq!(loop_ctrl.tick_overruns(), 0);
    }

    #[test]
    fn tick_sends_comm_watchdog_keepalive_when_active_control_disabled() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
        loop_ctrl.supervisor_mut().bus_mut().clear_trace();
        loop_ctrl.tick(None).expect("tick");
        let n = loop_ctrl.joint_names.len();
        assert_eq!(
            loop_ctrl.supervisor().bus().frames().len(),
            n,
            "Active + Disabled control mode must send zero-gain MIT keepalive per joint"
        );
    }

    #[test]
    fn active_gravity_refuses_when_a_coupled_joint_lacks_feedback() {
        let mut loop_ctrl = test_loop();
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("virtual reference reaches Ready");
        loop_ctrl
            .supervisor_mut()
            .enable_targets(&["right_elbow_pitch".to_string()])
            .expect("scoped enable");
        queue_joint_status(loop_ctrl.supervisor_mut(), "right_elbow_pitch", 0.4, 0.0);
        loop_ctrl
            .supervisor_mut()
            .drain_feedback()
            .expect("active elbow feedback");
        loop_ctrl.supervisor_mut().bus_mut().clear_trace();
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::GravityComp);

        let error = loop_ctrl
            .tick(None)
            .expect_err("gravity torque needs every coupled joint position");
        assert!(
            matches!(
                error,
                LoopError::MissingFeedback { ref joint } if joint == "right_shoulder_pitch"
            ),
            "{error}"
        );
        assert!(
            loop_ctrl
                .supervisor()
                .bus()
                .frames()
                .iter()
                .all(|frame| (frame.id >> 24) & 0x1f != 1 || frame.data[4..8] == [0; 4]),
            "fault cleanup may send neutral MIT, but missing coupled feedback must not send gravity torque"
        );
    }

    #[test]
    fn hold_at_reenables_after_safety_disable() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        loop_ctrl.supervisor_mut().disable_all().expect("disable");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Disabled);
        let refused = loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.0)
            .expect_err("re-enabled, but no session feedback yet");
        assert!(matches!(refused, LoopError::EnableIncomplete { .. }));
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Active);
        assert_ne!(loop_ctrl.control_mode(), ControlMode::Position);
        queue_all_status(loop_ctrl.supervisor_mut(), None);
        loop_ctrl
            .supervisor_mut()
            .drain_feedback()
            .expect("first session status");
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.0)
            .expect("hold-at home once enable completed");
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    }

    /// L-berthier-28: after an operator disable a motion command refuses
    /// instead of re-enabling; an explicit enable lifts the requirement.
    #[test]
    fn hold_at_refuses_to_reenable_after_operator_disable() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        loop_ctrl.supervisor_mut().disable_all().expect("disable");
        loop_ctrl.set_control_mode(ControlMode::Disabled);
        loop_ctrl.forbid_implicit_enable();
        let refused = loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.0)
            .expect_err("operator disable must not be undone by a motion command");
        assert!(matches!(refused, LoopError::ExplicitEnableRequired));
        assert_eq!(
            loop_ctrl.supervisor_mut().mode(),
            OperationalMode::Disabled,
            "refusal must not touch the drives"
        );
        loop_ctrl.allow_implicit_enable();
        let after_enable = loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.0)
            .expect_err("explicit enable restores the re-arm path");
        assert!(matches!(after_enable, LoopError::EnableIncomplete { .. }));
    }

    /// Active with no session pose: every Position-mode arm refuses and latches
    /// nothing, rather than holding the 0.0 placeholder of a missing pose.
    #[test]
    fn position_arms_wait_for_session_feedback() {
        let mut loop_ctrl = test_loop();
        loop_ctrl
            .supervisor_mut()
            .set_homing_complete()
            .expect("ready");
        let joints: Vec<String> = loop_ctrl
            .supervisor()
            .motors
            .motors
            .iter()
            .map(|motor| motor.joint.clone())
            .collect();
        loop_ctrl
            .supervisor_mut()
            .enable_targets(&joints)
            .expect("active without pose");
        let refusals = [
            loop_ctrl.enter_position_hold().map(|()| 0.0),
            loop_ctrl
                .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
                .map(|()| 0.0),
            loop_ctrl.start_position_wave("right_shoulder_pitch", 0.1, 0.2, 1, 1.0),
            loop_ctrl
                .set_joint_position_setpoint("right_shoulder_pitch", 0.25)
                .map(|()| 0.0),
        ];
        for refusal in refusals {
            assert!(
                matches!(&refusal, Err(LoopError::EnableIncomplete { detail })
                    if detail.contains("no session feedback")),
                "{refusal:?}"
            );
        }
        assert!(loop_ctrl.position_setpoints().is_none(), "nothing latched");
        assert_ne!(loop_ctrl.control_mode(), ControlMode::Position);
        let completion = loop_ctrl.enable_completion();
        assert!(
            matches!(&completion, Err(LoopError::EnableIncomplete { detail })
                if detail.contains("no session feedback")),
            "{completion:?}"
        );

        queue_all_status(
            loop_ctrl.supervisor_mut(),
            Some(("right_shoulder_pitch", 0.3, 0.0)),
        );
        loop_ctrl
            .supervisor_mut()
            .drain_feedback()
            .expect("first session status");
        loop_ctrl.enable_completion().expect("enable complete");
        loop_ctrl.enter_position_hold().expect("hold-on");
        let pitch = loop_ctrl
            .joint_names()
            .iter()
            .position(|name| name == "right_shoulder_pitch")
            .expect("pitch");
        let held = loop_ctrl.position_setpoints().expect("latched")[pitch];
        assert!((held - 0.3).abs() < 1e-3, "measured q latched, got {held}");
    }

    #[test]
    fn enable_completion_requires_active() {
        let loop_ctrl = test_loop();
        assert!(matches!(
            loop_ctrl.enable_completion(),
            Err(LoopError::EnableIncomplete { detail }) if detail.contains("not Active")
        ));
    }

    #[test]
    fn position_mode_without_latched_setpoint_errors() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl.set_control_mode(ControlMode::Position);
        let err = loop_ctrl.tick(None).expect_err("missing setpoint");
        assert!(matches!(err, LoopError::MissingSetpoint { .. }));
    }

    #[test]
    fn enter_position_hold_latches_setpoints() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl.enter_position_hold().expect("hold");
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
        let sp = loop_ctrl.position_setpoints().expect("latched setpoints");
        assert_eq!(sp.len(), loop_ctrl.joint_names.len());
    }

    #[test]
    fn hold_at_sets_joint_target() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.42)
            .expect("hold-at");
        let sp = loop_ctrl.position_setpoints().expect("setpoints");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint index");
        assert!((sp[i] - 0.42).abs() < 1e-9);
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
    }

    #[test]
    fn same_target_hold_at_preserves_active_wave() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        let joint = "right_shoulder_pitch";
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.1)
            .expect("hold-at");
        // Peak A·ω² = 0.1·(π/1.0)² ≈ 1 rad/s², inside master pitch's 1.5 rad/s² cap.
        loop_ctrl
            .start_position_wave(joint, 0.0, 0.2, 2, 1.0)
            .expect("wave");
        assert!(loop_ctrl.position_wave_active());
        loop_ctrl
            .set_joint_position_setpoint(joint, 0.1)
            .expect("same-target hold-at");
        assert!(
            loop_ctrl.position_wave_active(),
            "same-target hold-at must not cancel an in-loop wave"
        );
        loop_ctrl
            .set_joint_position_setpoint(joint, 0.25)
            .expect("changed hold-at");
        assert!(
            !loop_ctrl.position_wave_active(),
            "changed target must cancel the wave"
        );
    }

    #[test]
    fn leaving_position_mode_clears_setpoints() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl.enter_position_hold().expect("hold");
        assert!(loop_ctrl.position_setpoints().is_some());
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(loop_ctrl.position_setpoints().is_none());
        assert_eq!(loop_ctrl.control_mode(), ControlMode::GravityComp);
    }

    #[test]
    fn set_torque_cmd_enters_torque_only_and_latches() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
        assert!((loop_ctrl.torque_cmd(joint)).abs() < 1e-12);
        loop_ctrl.set_torque_cmd(joint, 0.25).expect("set");
        assert_eq!(loop_ctrl.control_mode(), ControlMode::TorqueOnly);
        assert!((loop_ctrl.torque_cmd(joint) - 0.25).abs() < 1e-12);
        // Leaving TorqueOnly clears the latch (clear_torque_cmd removed as test-only API).
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!((loop_ctrl.torque_cmd(joint)).abs() < 1e-12);
    }

    #[test]
    fn leaving_torque_only_clears_torque_cmds() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        loop_ctrl.set_torque_cmd(joint, 0.4).expect("set");
        assert!((loop_ctrl.torque_cmd(joint) - 0.4).abs() < 1e-12);
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(
            (loop_ctrl.torque_cmd(joint)).abs() < 1e-12,
            "leave TorqueOnly must clear τ_cmd latch"
        );
    }

    #[test]
    fn enter_torque_only_zero_clears_nonzero_latch() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        loop_ctrl.set_torque_cmd(joint, 0.5).expect("set");
        assert!((loop_ctrl.torque_cmd(joint) - 0.5).abs() < 1e-12);
        loop_ctrl.enter_torque_only_zero();
        assert_eq!(loop_ctrl.control_mode(), ControlMode::TorqueOnly);
        assert!(
            (loop_ctrl.torque_cmd(joint)).abs() < 1e-12,
            "gravity-off / enter_torque_only_zero must force τ_cmd≡0"
        );
    }

    #[test]
    fn set_torque_cmd_rejects_unknown_joint() {
        let mut loop_ctrl = test_loop();
        let err = loop_ctrl
            .set_torque_cmd("not_a_joint", 0.1)
            .expect_err("unknown");
        assert!(matches!(err, LoopError::UnknownJoint { .. }));
    }

    #[test]
    fn set_torque_cmd_rejects_non_finite() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        let err = loop_ctrl.set_torque_cmd(joint, f64::NAN).expect_err("nan");
        assert!(matches!(err, LoopError::NonFiniteTorqueCmd { .. }));
        let err = loop_ctrl
            .set_torque_cmd(joint, f64::INFINITY)
            .expect_err("inf");
        assert!(matches!(err, LoopError::NonFiniteTorqueCmd { .. }));
    }

    #[test]
    fn torque_only_tick_packs_latched_tau_cmd_on_wire() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        let joint = "right_shoulder_pitch";
        loop_ctrl.set_torque_cmd(joint, 0.35).expect("set");
        // Allow the configured torque slew to reach the requested step.
        std::thread::sleep(Duration::from_millis(20));
        loop_ctrl.supervisor_mut().bus_mut().clear_trace();
        loop_ctrl.tick(None).expect("tick");
        let frame = loop_ctrl
            .supervisor()
            .bus()
            .frames()
            .iter()
            .find(|frame| frame.id & 0xff == 1)
            .expect("pitch MIT output");
        assert_eq!((frame.id >> 24) & 0x1f, 1);
        // Literal master pitch: device 1, direction -1, ratio 1, RS03
        // torque full scale 60 Nm. Do not invoke the encoder as its own oracle.
        let torque_word = f64::from((frame.id >> 8) & 0xffff);
        let joint_torque = -(torque_word / 32767.0 - 1.0) * 60.0;
        assert!(
            (joint_torque - 0.35).abs() < 0.002,
            "wire joint torque {joint_torque}"
        );
        assert_eq!(frame.data, [0x7f, 0xff, 0x7f, 0xff, 0, 0, 0, 0]);
    }

    #[test]
    fn gravity_comp_enter_clears_sticky_gain_overrides() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        loop_ctrl
            .apply_gain_override(
                "right_shoulder_pitch",
                GainOverride {
                    kp: 50.0,
                    kd: 5.0,
                    ki: 0.0,
                    fc: 1.0,
                },
            )
            .expect("valid gain override");
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_some());
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        assert!(
            loop_ctrl.gain_override("right_shoulder_pitch").is_none(),
            "Impedance→GravityComp must clear Testing overrides"
        );
    }

    /// L-berthier-10: a gain override outside Impedance/Position was dropped
    /// but reported Ok; it is now refused with the mode in the error.
    #[test]
    fn apply_gain_override_refused_under_gravity_comp() {
        let mut loop_ctrl = test_loop();
        loop_ctrl.set_control_mode(ControlMode::GravityComp);
        let err = loop_ctrl
            .apply_gain_override(
                "right_shoulder_pitch",
                GainOverride {
                    kp: 50.0,
                    kd: 5.0,
                    ki: 0.0,
                    fc: 1.0,
                },
            )
            .expect_err("GravityComp has no runtime gains");
        assert!(matches!(
            err,
            LoopError::GainOverrideNotApplicable {
                mode: ControlMode::GravityComp,
                ..
            }
        ));
        assert!(
            loop_ctrl.gain_override("right_shoulder_pitch").is_none(),
            "must not stash overrides under GravityComp"
        );
        // Enter Impedance must not resurrect planted stiffness.
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
    }

    #[test]
    fn apply_gain_override_refused_under_disabled() {
        let mut loop_ctrl = test_loop();
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Disabled);
        let err = loop_ctrl
            .apply_gain_override(
                "right_shoulder_pitch",
                GainOverride {
                    kp: 50.0,
                    kd: 5.0,
                    ki: 0.0,
                    fc: 1.0,
                },
            )
            .expect_err("Disabled has no runtime gains");
        assert!(matches!(
            err,
            LoopError::GainOverrideNotApplicable {
                mode: ControlMode::Disabled,
                ..
            }
        ));
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
    }

    #[test]
    fn position_hold_tick_runs_when_active() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        assert_eq!(loop_ctrl.supervisor_mut().mode(), OperationalMode::Active);
    }

    #[test]
    fn position_hold_advances_trapezoid_on_first_tick() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 0.25)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint");
        loop_ctrl.tick(None).expect("tick");
        let cmd = loop_ctrl.position_hold_commands().expect("cmd")[i];
        assert!(
            cmd > 0.0,
            "trapezoid planner must advance q_traj on first tick"
        );
    }

    #[test]
    fn clamp_trajectory_setpoint_brakes_when_q_outruns_traj() {
        let q_des = clamp_trajectory_setpoint(0.11, 1.10, 1.57, 0.03, None, 0.0);
        assert!((q_des - 1.07).abs() < 1e-12);
    }

    #[test]
    fn clamp_always_bounds_lead_when_lag_exceeds_max_lead() {
        // Bench grind case: q stuck, q_traj already at target — must not open-loop to 0.15.
        let q_des = clamp_trajectory_setpoint(0.15, 0.02, 0.15, 0.10, None, 0.0);
        assert!(
            (q_des - 0.12).abs() < 1e-12,
            "q_des must be q+max_lead=0.12, got {q_des}"
        );
    }

    #[test]
    fn clamp_does_not_pull_back_when_slightly_ahead_approaching() {
        let q_des = clamp_trajectory_setpoint(0.035, 0.058, 0.1, 0.10, None, 0.0);
        assert!((q_des - 0.058).abs() < 1e-12);
    }

    #[test]
    fn clamp_brakes_when_arm_runs_far_ahead_of_planner() {
        let q_des = clamp_trajectory_setpoint(1.32, 1.36, 1.57, 0.10, None, 0.0);
        assert!(
            q_des < 1.36,
            "must not command q_des at measured q when lead > resync band"
        );
        assert!((q_des - 1.32).abs() < 1e-12);
    }

    #[test]
    fn clamp_caps_ahead_setpoint_at_target_on_approach() {
        let q_des = clamp_trajectory_setpoint(0.091, 0.105, 0.1, 0.10, None, 0.0);
        assert!((q_des - 0.1).abs() < 1e-12);
    }

    #[test]
    fn clamp_does_not_pull_forward_near_home_return() {
        let q_des = clamp_trajectory_setpoint(0.039, 0.006, 0.0, 0.10, None, -0.6);
        assert!(
            q_des <= 0.006,
            "near-home return must not brake descent by commanding ahead: {q_des}"
        );
    }

    #[test]
    fn clamp_latches_setpoint_at_target_on_large_hold_overshoot() {
        // Hold: the planner reference sits at the target.
        let q_des = clamp_trajectory_setpoint(1.57, 1.64, 1.57, 0.10, None, 0.0);
        assert!(
            (q_des - 1.57).abs() < 1e-12,
            "hold overshoot must latch at target"
        );
        let q_des_negative = clamp_trajectory_setpoint(-0.64, -0.70, -0.64, 0.10, None, 0.0);
        assert!(
            (q_des_negative + 0.64).abs() < 1e-12,
            "negative hold overshoot must latch at target"
        );
        // Planner still above the target (a descent in progress): no snap, and no chase of
        // q_traj above measured q.
        let q_des_descent = clamp_trajectory_setpoint(1.65, 1.64, 1.57, 0.10, None, -0.3);
        assert!(
            (q_des_descent - 1.64).abs() < 1e-12,
            "descent must not chase q_traj above measured q: {q_des_descent}"
        );
    }

    #[test]
    fn clamp_does_not_jump_to_negative_target_before_planner_arrives() {
        let q_des = clamp_trajectory_setpoint(-0.0005, -0.0004, -0.6417, 0.10, None, -0.024);
        assert!(
            q_des > -0.02,
            "sub-home retarget must follow q_traj, not jump to target: {q_des}"
        );
        assert!(
            q_des > -0.6417,
            "q_des must not command the lower-limit target before planner reaches it"
        );
    }

    #[test]
    fn clamp_does_not_snap_to_target_on_descent_toward_positive_target() {
        // Bench 2026-10-04T07:36:40Z elbow, t=31.76 s: 0.75 → 0.5 descent, planner 0.14 short
        // of target on the arm's side. The snap commanded q_des=0.5 (tau_p −1.70 Nm) and the
        // elbow fell at −2.46 rad/s into the Davout feedback-velocity fault.
        let q_des = clamp_trajectory_setpoint(0.640, 0.642, 0.5, 0.15, None, -0.85);
        assert!(
            (q_des - 0.640).abs() < 1e-12,
            "descent must follow q_traj until the planner arrives: {q_des}"
        );
    }

    #[test]
    fn clamp_does_not_snap_to_target_on_ascent_toward_negative_target() {
        // Bench 2026-10-04T08:46:15Z pitch, t=8.16 s: −0.518 → −0.254, planner 0.10 short.
        // The snap stepped tau_p 1.36 → 3.09 Nm and dq reached 2.13 rad/s (plan 0.79).
        let q_des = clamp_trajectory_setpoint(-0.355, -0.426, -0.254, 0.12, None, 0.975);
        assert!(
            (q_des + 0.355).abs() < 1e-12,
            "move toward home must follow q_traj until the planner arrives: {q_des}"
        );
        // Same run, t=3.96 s: a 0.05 rad move −0.556 → −0.508 became a single step.
        let q_des_small = clamp_trajectory_setpoint(-0.555, -0.556, -0.508, 0.12, None, 0.09);
        assert!(
            (q_des_small + 0.555).abs() < 1e-12,
            "small move must not step to target: {q_des_small}"
        );
    }

    #[test]
    fn planner_latches_on_overshoot_hold_when_at_rest() {
        assert!(planner_should_latch_on_overshoot_hold(
            1.64, 1.57, 1.57, 0.0, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            1.64, 1.57, 1.57, 0.15, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            1.58, 1.57, 1.57, 0.0, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            0.105, 0.0, 0.0, 0.0, 0.02
        ));
        assert!(!planner_should_latch_on_overshoot_hold(
            -0.016, -0.016, -0.641, 0.0, 0.02
        ));
        assert!(planner_should_latch_on_overshoot_hold(
            -0.70, -0.641, -0.641, 0.0, 0.02
        ));
        // Descent retarget start (wave reverse): q_traj still above target — must NOT latch.
        assert!(!planner_should_latch_on_overshoot_hold(
            0.76, 0.76, 0.4, 0.0, 0.02
        ));
    }

    #[test]
    fn clamp_does_not_jump_on_descent_retarget_start() {
        // Wave reverse: q_traj still at measured q above a lower target — follow lead, don't snap.
        let q_des = clamp_trajectory_setpoint(0.76, 0.76, 0.4, 0.12, None, 0.0);
        assert!(
            (q_des - 0.76).abs() < 0.13,
            "descent retarget must not command target before planner arrives: {q_des}"
        );
        assert!(
            (q_des - 0.4).abs() > 0.2,
            "q_des must stay near q, not jump to 0.4: {q_des}"
        );
    }

    #[test]
    fn planner_premature_hold_when_virtual_target_reached_before_arm() {
        let mut planner = JointPositionPlanner::new_for_target(0.05, 0.1);
        planner.q_traj = 0.1;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(planner_premature_hold(&planner, 0.052, 0.1));
        assert!(!planner_drifted_from_measurement(
            &planner, 0.052, 0.1, 0.10
        ));
        reopen_planner_from_premature_hold(&mut planner, 0.052, 0.1, 0.15);
        assert_eq!(planner.phase(), TrapezoidPhase::Cruise);
        assert!((planner.q_traj - 0.1).abs() < 1e-12);
        assert!((planner.dq_traj - 0.15).abs() < 1e-12);
    }

    #[test]
    fn planner_reopens_when_overshot_past_hold_while_moving() {
        let mut planner = JointPositionPlanner::new_for_target(1.5, 1.57);
        planner.q_traj = 1.57;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(planner_overshoot_hold_while_moving(
            &planner, 1.62, 1.57, 0.08, 0.02
        ));
        reopen_planner_from_premature_hold(&mut planner, 1.62, 1.57, 1.65);
        assert_eq!(planner.phase(), TrapezoidPhase::Cruise);
        assert!(planner.dq_traj < 0.0);
    }

    #[test]
    fn mit_kd_engages_on_velocity_during_motion() {
        // Moving toward or past target with dq > deadband → kd engages
        assert!((position_hold_mit_kd(2.0, 0.08, 0.02) - 2.0).abs() < 1e-12);
        // Below deadband → kd remains 0.0
        assert!((position_hold_mit_kd(2.0, 0.01, 0.02)).abs() < 1e-12);
        // Moving toward target (not yet past) → kd engages (new behavior)
        assert!((position_hold_mit_kd(2.0, 0.08, 0.02) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn planner_drifted_when_hold_latched_but_arm_not_settled() {
        let planner = JointPositionPlanner::new_at(0.0);
        assert!(planner_premature_hold(&planner, 0.087, 0.0));
        assert!(!planner_drifted_from_measurement(
            &planner, 0.087, 0.0, 0.10
        ));
    }

    #[test]
    fn home_premature_hold_when_arm_above_target_within_old_resync_band() {
        let mut planner = JointPositionPlanner::new_at(0.0);
        planner.force_hold_for_test();
        // 0.031 rad was observed on bench-20260620T003624Z — inside 0.03 resync, above 0.005 home band.
        assert!(planner_premature_hold(&planner, 0.031, 0.0));
        assert!(!planner_drifted_from_measurement(
            &planner, 0.031, 0.0, 0.10
        ));
    }

    #[test]
    fn planner_not_drifted_when_arm_runs_ahead_on_approach() {
        let mut planner = JointPositionPlanner::new_for_target(0.88, 1.57);
        planner.q_traj = 0.88;
        assert!(!planner_drifted_from_measurement(
            &planner, 1.02, 1.57, 0.10
        ));
    }

    #[test]
    fn planner_not_drifted_on_small_hold_overshoot() {
        let planner = JointPositionPlanner::new_at(0.1);
        assert!(!planner_drifted_from_measurement(
            &planner, 0.112, 0.1, 0.10
        ));
    }

    #[test]
    fn return_onset_pulls_q_des_below_q_when_stuck() {
        use crate::position_setpoint::POSITION_DESCENT_STUCK_LEAD_RAD;
        assert!(descent_stuck_mit_pull(
            -0.09, 0.105, 0.015, 0.0, 0.02, false
        ));
        let q_des = clamp_trajectory_setpoint(0.105, 0.105, 0.015, 0.10, None, 0.0)
            .min(0.105 - POSITION_DESCENT_STUCK_LEAD_RAD);
        assert!((q_des - 0.075).abs() < 1e-12);
    }

    #[test]
    fn sub_home_negative_move_does_not_enable_descent_mit_pull() {
        assert!(!descent_stuck_mit_pull(-0.64, 0.0, -0.64, 0.0, 0.02, false));
        assert!(!descent_stuck_mit_pull(
            -0.50, -0.14, -0.64, 0.0, 0.02, false
        ));
    }

    #[test]
    fn onset_mit_velocity_follows_planner_while_arm_stuck() {
        let v = position_hold_mit_velocity(0.0, 0.18, 0.02, 0, true);
        assert!((v - 0.18).abs() < 1e-12);
    }

    #[test]
    fn onset_mit_velocity_zeros_after_onset_window() {
        let v = position_hold_mit_velocity(0.0, 0.18, 0.02, 301, true);
        assert!((v).abs() < 1e-12);
    }

    #[test]
    fn onset_max_lead_boosts_outbound_only() {
        let boosted = position_hold_effective_max_lead(0.10, 0, true, 1.57, 0.0);
        assert!((boosted - 0.15).abs() < 1e-12);
        let mid_span = position_hold_effective_max_lead(0.10, 0, true, 0.69, 0.878);
        assert!((mid_span - 0.10).abs() < 1e-12);
        let home = position_hold_effective_max_lead(0.10, 0, false, -1.57, 1.5);
        assert!((home - 0.10).abs() < 1e-12);
        let small = position_hold_effective_max_lead(0.10, 0, true, 0.02, 0.0);
        assert!(
            (small - 0.10).abs() < 1e-12,
            "small outbound below descent-seed threshold: onset-only, no sustained knee boost"
        );
        let return_high = position_hold_effective_max_lead(0.10, 0, true, -1.545, 1.645);
        assert!(
            (return_high - 0.15).abs() < 1e-12,
            "return from high q gets onset lead boost"
        );
        let outbound_knee = position_hold_effective_max_lead(0.10, 500, true, 0.05, 0.10);
        assert!(
            (outbound_knee - 0.10).abs() < 1e-12,
            "outbound knee after onset: no sustained lead boost"
        );
        let staging_descent = position_hold_effective_max_lead(0.10, 500, false, -0.15, 0.25);
        assert!(
            (staging_descent - 0.15).abs() < 1e-12,
            "sustained boost on staged descent near 15 deg"
        );
        let lower_limit = position_hold_effective_max_lead(0.10, 0, true, -0.64, 0.0);
        assert!(
            (lower_limit - 0.10).abs() < 1e-12,
            "limit-directed negative move must not get return assist boost"
        );
    }

    #[test]
    fn outbound_hold_at_015_no_sustained_lead_boost_after_onset() {
        let lead = position_hold_effective_max_lead(0.10, 500, true, 0.05, 0.10);
        assert!(
            (lead - 0.10).abs() < 1e-12,
            "hold-at 0.15 after onset must keep max_lead=0.10 so stuck-lead resync can fire"
        );
    }

    #[test]
    fn low_angle_knee_boost_stays_inside_its_0_to_030_band() {
        // The knee band is ~0–30° above home. A joint well below home (pitch at −0.6 holding
        // toward −0.9, planner not approaching) is outside it and keeps max_lead.
        let below_home = position_hold_effective_max_lead(0.10, 5000, false, -0.3, -0.6);
        assert!(
            (below_home - 0.10).abs() < 1e-12,
            "no sustained knee boost below home: {below_home}"
        );
        let in_band = position_hold_effective_max_lead(0.10, 5000, false, -0.1, 0.2);
        assert!(
            (in_band - 0.15).abs() < 1e-12,
            "in-band return keeps the sustained knee boost: {in_band}"
        );
    }

    #[test]
    fn planner_resyncs_when_stuck_at_lead_cap() {
        let mut planner = JointPositionPlanner::new_for_target(1.02, 1.57);
        planner.q_traj = 0.92;
        assert!(planner_should_resync_stuck_lead(
            &planner, 1.02, 1.57, 0.0, 0.10, 0.02
        ));
        assert!(!planner_should_resync_stuck_lead(
            &planner, 1.02, 1.57, 0.15, 0.10, 0.02
        ));
    }

    #[test]
    fn planner_no_resync_when_lagging_on_home_return() {
        let mut planner = JointPositionPlanner::new_for_target(1.65, 0.0);
        planner.q_traj = 1.50;
        planner.dq_traj = -0.8;
        assert!(!planner_should_resync_stuck_lead(
            &planner, 1.65, 0.0, 0.0, 0.10, 0.02
        ));
    }

    #[test]
    fn planner_not_drifted_when_arm_lags_on_home_return() {
        let mut planner = JointPositionPlanner::new_for_target(1.65, 0.0);
        planner.q_traj = 1.50;
        planner.dq_traj = -1.2;
        assert!(!planner_drifted_from_measurement(&planner, 1.65, 0.0, 0.10));
    }

    // === FIX A regression tests: effective max_lead in drift/resync thresholds ===
    // These tests lock the fix for the pi/2 hold limit cycle. The onset boost
    // (position_hold_effective_max_lead) raises max_lead from 0.05 to 0.15 during
    // the 300ms post-retarget window. Before FIX A, advance_position_commands
    // passed raw max_lead to the drift/resync checks, dead-lettering the boost:
    // q_traj was reset to q whenever lead > 0.05, so q_des could never reach q+0.15,
    // capping P-torque at 0.4 Nm — insufficient to break static friction.

    #[test]
    fn planner_drift_uses_effective_max_lead_during_onset() {
        // At home (q≈0) stuck with target=π/2, during onset, q_traj=q+0.10 must
        // NOT drift-reset when using effective max_lead (0.15). With raw 0.05 it
        // would — this is the bug FIX A addresses.
        let q = 0.004;
        let target = std::f64::consts::FRAC_PI_2;
        let mut planner = JointPositionPlanner::new_for_target(q, target);
        planner.q_traj = q + 0.10;
        planner.dq_traj = 0.4;
        let raw_max_lead = 0.05;
        let effective = position_hold_effective_max_lead(raw_max_lead, 100, true, target - q, q);
        assert!((effective - 0.15).abs() < 1e-9, "onset boost active");
        assert!(
            !planner_drifted_from_measurement(&planner, q, target, effective),
            "FIX A: drift uses effective max_lead; q+0.10 within 0.15 lead → no reset"
        );
        assert!(
            planner_drifted_from_measurement(&planner, q, target, raw_max_lead),
            "bug repro: raw max_lead resets at q+0.10 > 0.05, dead-lettering the boost"
        );
    }

    #[test]
    fn planner_drift_reverts_to_raw_max_lead_after_onset() {
        // After 300ms onset window, effective max_lead reverts to raw; drift
        // reset resumes at the raw threshold. Guards against persistent boost.
        let q = 0.004;
        let target = std::f64::consts::FRAC_PI_2;
        let mut planner = JointPositionPlanner::new_for_target(q, target);
        planner.q_traj = q + 0.10;
        planner.dq_traj = 0.4;
        let raw_max_lead = 0.05;
        let effective = position_hold_effective_max_lead(raw_max_lead, 301, true, target - q, q);
        assert!((effective - raw_max_lead).abs() < 1e-9, "onset expired");
        assert!(
            planner_drifted_from_measurement(&planner, q, target, effective),
            "drift reset resumes at raw max_lead after onset expires"
        );
    }

    #[test]
    fn planner_resync_uses_effective_max_lead_during_onset() {
        // Resync_stuck_lead must also use effective max_lead. During onset,
        // q_traj=q+0.10 is within 0.15 lead → no resync. Raw 0.05 would resync.
        let q = 0.004;
        let target = std::f64::consts::FRAC_PI_2;
        let mut planner = JointPositionPlanner::new_for_target(q, target);
        planner.q_traj = q + 0.10;
        planner.dq_traj = 0.4;
        let raw_max_lead = 0.05;
        let effective = position_hold_effective_max_lead(raw_max_lead, 100, true, target - q, q);
        assert!(
            !planner_should_resync_stuck_lead(&planner, q, target, 0.0, effective, 0.02),
            "FIX A: resync uses effective max_lead; q+0.10 within 0.15 → no resync"
        );
        assert!(
            planner_should_resync_stuck_lead(&planner, q, target, 0.0, raw_max_lead, 0.02),
            "bug repro: raw max_lead resyncs at q+0.10 > 0.05"
        );
    }

    #[test]
    fn onset_lead_boost_expires_at_300ms_boundary() {
        // The onset boost must expire at POSITION_HOLD_ONSET_MS (300ms).
        // At 300ms: boost active (0.15). At 301ms: expired (raw 0.05).
        let raw = 0.05;
        let q = 0.004;
        let settle_error = std::f64::consts::FRAC_PI_2 - q;
        let at_boundary = position_hold_effective_max_lead(raw, 300, true, settle_error, q);
        assert!((at_boundary - 0.15).abs() < 1e-9, "boost active at 300ms");
        let after_boundary = position_hold_effective_max_lead(raw, 301, true, settle_error, q);
        assert!(
            (after_boundary - raw).abs() < 1e-9,
            "boost expired at 301ms"
        );
    }

    #[test]
    fn descent_mit_pull_clears_after_breakaway_latch() {
        assert!(!descent_stuck_mit_pull(
            -0.09, 0.105, 0.015, 0.0, 0.02, true
        ));
        assert!(descent_stuck_mit_pull(
            -0.09, 0.105, 0.015, 0.0, 0.02, false
        ));
        assert!(descent_breakaway_confirmed(-0.09, -0.026, 0.02));
        // Cruise motion alone must not imply breakaway without a stuck episode.
        assert!(!descent_stuck_mit_pull(
            -0.09, 0.08, 0.015, -0.03, 0.02, false
        ));
    }

    #[test]
    fn ascent_stall_enters_bounded_recovery() {
        let q = 0.02;
        let target = 0.15;
        let q_traj = 0.125;
        let to_target = target - q;
        let deadband = 0.02;
        assert!(planner_should_recover_ascent_stall(
            false, target, q, q_traj, to_target, 0.0, deadband
        ));
        assert!(!planner_should_recover_ascent_stall(
            false,
            target,
            q,
            q + 0.01,
            to_target,
            0.0,
            deadband
        ));
        assert!(
            planner_should_recover_ascent_stall(
                true,
                target,
                q,
                q + 0.01,
                to_target,
                0.0,
                deadband
            ),
            "stuck-lead resync must preserve recovery"
        );
        // Motion toward target → exit
        assert!(!planner_should_recover_ascent_stall(
            true, target, q, q_traj, to_target, 0.03, deadband
        ));
        // Home return owned by descent freeze
        assert!(!planner_should_recover_ascent_stall(
            false, 0.0, 0.08, 0.02, -0.08, 0.0, deadband
        ));
    }

    #[test]
    fn lead_follow_stuck_residual_requires_outbound_gap_and_zero_velocity() {
        let deadband = 0.02;
        assert!(lead_follow_stuck_residual(true, 1.333, 1.40, 0.0, deadband));
        assert!(!lead_follow_stuck_residual(
            false, 1.333, 1.40, 0.0, deadband
        ));
        assert!(!lead_follow_stuck_residual(
            true, 0.125, 0.15, 0.0, deadband
        ));
        assert!(!lead_follow_stuck_residual(true, -0.05, 0.0, 0.0, deadband));
        assert!(!lead_follow_stuck_residual(
            true, 1.333, 1.40, deadband, deadband
        ));
    }

    #[test]
    fn hold_on_one_feedback_count_off_zero_latches_home() {
        let joint = "right_shoulder_pitch";
        // (raw hold-on q, latched target) through Davout's installed RS03 grid.
        let hold_on = |counts: f64| {
            let mut loop_ctrl = test_loop();
            let count = 2.0
                * loop_ctrl
                    .supervisor()
                    .joint_position_progress_threshold(joint)
                    .expect("installed grid");
            virtual_ready_active_at(&mut loop_ctrl, Some((joint, counts * count, 0.0)));
            loop_ctrl.enter_position_hold().expect("hold-on");
            let i = loop_ctrl
                .joint_names()
                .iter()
                .position(|name| name == joint)
                .expect("configured joint");
            let raw = loop_ctrl
                .position_hold
                .raw_targets_for_test()
                .expect("raw latch")[i];
            let latched = loop_ctrl.position_setpoints().expect("latch")[i];
            (raw, latched)
        };
        let (exact_raw, exact) = hold_on(0.0);
        assert_eq!((exact_raw, exact), (0.0, 0.0));
        let (one_raw, one) = hold_on(1.0);
        assert!(
            one_raw.abs() > POSITION_SETTLE_TOLERANCE_RAD,
            "precondition: one count is above the old 1e-4 home test; got {one_raw}"
        );
        assert_eq!(one, exact, "one-count hold-on must classify as home");
        let (five_raw, five) = hold_on(5.0);
        assert_eq!(five, five_raw, "outbound latches keep their value");
        assert!(five.abs() > 4.0 * one_raw.abs());
    }

    #[test]
    fn ascent_stall_faults_tick_after_bounded_recovery() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        virtual_ready_active_at(&mut loop_ctrl, Some((joint, 0.02, 0.0)));
        loop_ctrl
            .enter_position_hold_at(Some(joint), 0.15)
            .expect("hold-at");
        let mut faulted = false;
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
        for _ in 0..600 {
            replay_feedback(&mut loop_ctrl, joint, 0.02, 0.0);
            assert_eq!(
                loop_ctrl
                    .supervisor()
                    .joint_feedback(joint)
                    .expect("actual stationary pose")
                    .velocity_rad_s,
                0.0
            );
            match loop_ctrl.tick(None) {
                Ok(()) => {}
                Err(err) => {
                    assert!(
                        matches!(
                            &err,
                            LoopError::AscentStall { joint: j, ms, .. }
                                if j == joint && *ms >= POSITION_ASCENT_STALL_FAULT_MS
                        ),
                        "unexpected tick error: {err:?}"
                    );
                    faulted = true;
                    break;
                }
            }
        }
        assert!(
            faulted,
            "stuck outbound ascent must AscentStall within ~3s of recovery"
        );
        assert!(loop_ctrl.supervisor().safety_snapshot().is_latched());
        assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    }

    #[test]
    fn lead_follow_residual_within_resync_band_does_not_ascent_stall() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        let q = 0.125;
        let target = 0.15;
        virtual_ready_active_at(&mut loop_ctrl, Some((joint, f64::from(q), 0.0)));
        loop_ctrl
            .enter_position_hold_at(Some(joint), target)
            .expect("hold-at");
        loop_ctrl
            .test_force_planner_hold_at(joint, target)
            .expect("force Hold@target");
        for _ in 0..500 {
            replay_feedback(&mut loop_ctrl, joint, q, 0.0);
            loop_ctrl
                .tick(None)
                .expect("lead-follow residual within resync band must not AscentStall");
        }
    }

    #[test]
    fn lead_follow_stuck_residual_faults_closed() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        let q = 1.333;
        let target = 1.40;
        virtual_ready_active_at(&mut loop_ctrl, Some((joint, f64::from(q), 0.0)));
        loop_ctrl
            .enter_position_hold_at(Some(joint), target)
            .expect("hold-at");
        loop_ctrl
            .test_force_planner_hold_at(joint, target)
            .expect("force Hold@target");

        let mut fault = None;
        assert_eq!(loop_ctrl.control_mode(), ControlMode::Position);
        for _ in 0..500 {
            replay_feedback(&mut loop_ctrl, joint, q, 0.0);
            assert_eq!(
                loop_ctrl
                    .supervisor()
                    .joint_feedback(joint)
                    .expect("actual stationary pose")
                    .velocity_rad_s,
                0.0
            );
            match loop_ctrl.tick(None) {
                Ok(()) => {}
                Err(err) => {
                    fault = Some(err);
                    break;
                }
            }
        }

        assert!(
            matches!(
                fault,
                Some(LoopError::AscentStall { joint: ref j, ms, .. })
                    if j == joint && ms >= POSITION_ASCENT_STALL_FAULT_MS
            ),
            "lead-follow stuck residual must AscentStall within 2.5s; got {fault:?}"
        );
        assert!(loop_ctrl.supervisor().safety_snapshot().is_latched());
        assert_eq!(loop_ctrl.supervisor().mode(), OperationalMode::Disabled);
    }

    #[test]
    fn hold_short_of_target_lead_follows_from_measured_q() {
        // Bench creep: Hold at 0.15 with q≈0.13 (inside premature 30 mrad band).
        let mut planner = JointPositionPlanner::new_for_target(0.13, 0.15);
        planner.q_traj = 0.15;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(
            !planner_premature_hold(&planner, 0.13, 0.15),
            "0.02 rad short is inside return_settle_band — premature path does not fire"
        );
        assert!(planner_should_lead_follow_hold_short(
            &planner, 0.13, 0.15, 0.0, 0.02
        ));
        assert!(!planner_should_lead_follow_hold_short(
            &planner, 0.148, 0.15, 0.0, 0.02
        ));
        apply_lead_follow_hold_short(&mut planner, 0.13, 0.15, 0.12, 0.35);
        assert!(
            (planner.q_traj - 0.15).abs() < 1e-12,
            "residual < max_lead ⇒ q_traj at target (full remaining lead)"
        );
        assert!(
            planner.dq_traj > 0.0,
            "non-zero cruise dq keeps traj friction during residual finish"
        );
        assert_eq!(planner.phase(), TrapezoidPhase::Cruise);
        // Must keep lead-following while Cruise@target so tick stays frozen.
        assert!(planner_should_lead_follow_hold_short(
            &planner, 0.13, 0.15, 0.0, 0.02
        ));
    }

    #[test]
    fn home_return_lead_follows_from_above_not_past_home() {
        // Bench 1.57 return: stuck Hold at q≈0.018 with q_traj=0 — must finish downward.
        let mut above = JointPositionPlanner::new_for_target(0.018, 0.0);
        above.q_traj = 0.0;
        above.dq_traj = 0.0;
        above.force_hold_for_test();
        assert!(
            planner_should_lead_follow_hold_short(&above, 0.018, 0.0, 0.0, 0.02),
            "home residual from above must lead-follow downward"
        );
        apply_lead_follow_hold_short(&mut above, 0.018, 0.0, 0.12, 0.35);
        assert!(
            above.dq_traj < 0.0,
            "home residual cruise must be toward home (negative)"
        );

        // Past-home (pull-through undershoot): must NOT lead-follow back up — that oscillates.
        let mut past = JointPositionPlanner::new_for_target(-0.006, 0.0);
        past.q_traj = 0.0;
        past.dq_traj = 0.0;
        past.force_hold_for_test();
        assert!(
            !planner_should_lead_follow_hold_short(&past, -0.006, 0.0, 0.0, 0.02),
            "past-home Hold must not lead-follow upward"
        );
        // Active Cruise@target with +dq (the oscillating bench state) also rejected.
        past.resume_cruise_toward(0.0, 0.35);
        assert!(
            !planner_should_lead_follow_hold_short(&past, -0.006, 0.0, 0.08, 0.02),
            "past-home Cruise@target +dq must not keep lead-following"
        );
    }

    #[test]
    fn stuck_premature_hold_skips_reopen() {
        let mut planner = JointPositionPlanner::new_for_target(0.02, 0.15);
        planner.q_traj = 0.15;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        assert!(planner_premature_hold(&planner, 0.02, 0.15));
        assert!(
            !planner_should_reopen_premature_hold(&planner, 0.02, 0.15, 0.0, 0.02),
            "stuck premature hold must not reopen (Reset oscillator)"
        );
        assert!(
            planner_should_lead_follow_hold_short(&planner, 0.02, 0.15, 0.0, 0.02),
            "large shortfall lead-follows from measured q"
        );
        // Moving toward target may reopen
        assert!(planner_should_reopen_premature_hold(
            &planner, 0.02, 0.15, 0.05, 0.02
        ));
        // Overshoot while moving still reopens
        let mut overshoot = JointPositionPlanner::new_for_target(1.5, 1.57);
        overshoot.q_traj = 1.57;
        overshoot.dq_traj = 0.0;
        overshoot.force_hold_for_test();
        assert!(planner_should_reopen_premature_hold(
            &overshoot, 1.62, 1.57, 0.08, 0.02
        ));
    }

    #[test]
    fn stuck_premature_hold_does_not_thrash_to_cruise_at_target() {
        // Integration-style gate: q_traj=target=0.15, q=0.02, dq=0.
        // Old path reopened Cruise with q_traj still 0.15 → Hold → Reset storm.
        let mut planner = JointPositionPlanner::new_for_target(0.02, 0.15);
        planner.q_traj = 0.15;
        planner.dq_traj = 0.0;
        planner.force_hold_for_test();
        let q = 0.02;
        let target = 0.15;
        let max_lead = 0.10;
        let deadband = 0.02;
        assert!(!planner_should_reopen_premature_hold(
            &planner, q, target, 0.0, deadband
        ));
        assert!(planner_should_resync_stuck_lead(
            &planner, q, target, 0.0, max_lead, deadband
        ));
        planner.reset_target(q, target);
        assert!(
            (planner.q_traj - q).abs() < 1e-12,
            "resync must snap q_traj to measured q, not leave it at target"
        );
        assert!(
            (planner.q_traj - target).abs() > 1e-6,
            "must not keep q_traj parked at target while arm is stuck short"
        );
        assert_ne!(
            planner.phase(),
            TrapezoidPhase::Cruise,
            "resync must not reopen Cruise at the latched target"
        );
    }

    #[test]
    fn envelope_hold_clamp_allows_home_target_from_high_q() {
        use crate::position_setpoint::{envelope_dq_cmd_for_hold_clamp, POSITION_HOME_SETTLE_RAD};
        use armee_kinematics::{JointLimitBounds, JointLimitPolicy, LimitMarginConfig};

        let policy = JointLimitPolicy {
            bounds: JointLimitBounds::from_hard_and_soft(0.0, 3.14159, None, None).expect("bounds"),
            margin: LimitMarginConfig {
                min_rad: 0.01,
                k_v_s: 0.02,
                k_stop: 0.5,
                decel_rad_s2: 4.5,
                velocity_deadband_rad_s: 0.02,
                measured_fault_slack_rad: 0.005,
            },
            velocity: 1.25,
            effort: 5.0,
            tau_ff_max: 5.0,
        };
        let q = 0.286;
        let requested = 0.0;
        let dq_cruise = -1.25;
        let slew = 0.15;
        let dq_env = envelope_dq_cmd_for_hold_clamp(Some(&policy), q, requested, dq_cruise, slew);
        assert!(
            dq_env.abs() <= slew + 1e-9,
            "envelope dq should use slew, got {dq_env}"
        );
        let clamped = armee_kinematics::clamp_hold_target(&policy, q, dq_env, requested);
        assert!(
            clamped.abs() <= POSITION_HOME_SETTLE_RAD,
            "home hold-at must not clamp to margin band: got {clamped}"
        );
    }

    #[test]
    fn home_final_approach_stuck_enables_mit_pull_and_unfreezes_planner() {
        use crate::position_setpoint::{
            home_final_approach_stuck, home_final_approach_stuck_pull_rad,
            POSITION_HOME_FINAL_PULL_THROUGH_RAD,
        };
        assert!(home_final_approach_stuck(0.031, 0.0));
        assert!(!home_final_approach_stuck(0.003, 0.0));
        assert!(home_final_approach_stuck(0.06, 0.0));
        assert!(home_final_approach_stuck(0.14, 0.0));
        assert!(!home_final_approach_stuck(0.16, 0.0));
        assert!(descent_stuck_mit_pull(-0.031, 0.031, 0.0, 0.0, 0.02, false));
        assert!(descent_stuck_mit_pull(-0.14, 0.14, 0.0, 0.0, 0.02, false));
        assert!(
            (home_final_approach_stuck_pull_rad(0.031, 0.0)
                - (0.031 + POSITION_HOME_FINAL_PULL_THROUGH_RAD))
                .abs()
                < 1e-9
        );
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 0.031, -0.031, 0.031, -0.15, 0.0, 0.02, 0.10,
        ));
        assert!(!planner_should_freeze_on_descent(
            true, 0.0, 0.031, -0.031, 0.031, -0.15, 0.0, 0.02, 0.10,
        ));
    }

    #[test]
    fn planner_freeze_skipped_in_home_final_approach_band() {
        let deadband = 0.02;
        // MIT pull-through replaces planner freeze in the 5–150 mrad home band.
        for q in [0.01_f64, 0.04, 0.05, 0.12, 0.15] {
            assert!(!planner_should_freeze_on_descent(
                false, 0.0, q, -q, q, -0.10, 0.0, deadband, 0.10
            ));
            assert!(!planner_should_freeze_on_descent(
                true, 0.0, q, -q, q, -0.10, -0.024, deadband, 0.10
            ));
        }
    }

    #[test]
    fn planner_freeze_skips_far_from_home_overshoot_return() {
        let deadband = 0.02;
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 0.102, -0.102, 0.030, -0.15, 0.0, deadband, 0.10
        ));
    }

    #[test]
    fn planner_freeze_skips_intermediate_overshoot_and_synced_return() {
        let deadband = 0.02;
        // Overshoot past 0.8 rad hold — same lag signature, must not freeze.
        assert!(!planner_should_freeze_on_descent(
            false, 0.8, 0.878, -0.078, 0.059, -0.062, 0.0, deadband, 0.10
        ));
        // Return nearly synced (~10 mrad lead) — must not latch forever at rest.
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 1.518, -1.508, 0.010, -0.180, 0.0, deadband, 0.10
        ));
        assert!(!planner_should_freeze_on_descent(
            true, 0.0, 1.518, -1.508, 0.010, -0.180, 0.0, deadband, 0.10
        ));
    }

    #[test]
    fn planner_freeze_skips_high_angle_return_overshoot() {
        let deadband = 0.02;
        assert!(!planner_should_freeze_on_descent(
            false, 0.0, 1.645, -1.645, 0.032, -0.27, 0.0, deadband, 0.10
        ));
    }

    #[test]
    fn slew_max_lead_clamps_command_ahead_of_measured_q() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 1.2)
            .expect("hold-at");
        loop_ctrl.tick(None).expect("tick");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint");
        let cmd = loop_ctrl.position_hold_commands().expect("cmd")[i];
        assert!(cmd < 0.2, "first tick must not jump to 1.2 rad target");
    }

    #[test]
    fn hold_at_ramps_command_not_instant_setpoint() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), 1.2)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint index");
        let target = loop_ctrl.position_setpoints().expect("target")[i];
        assert!((target - 1.2).abs() < 1e-9);
        let cmd0 = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(cmd0.abs() < 1e-6, "command starts at measured q≈0");
        loop_ctrl.tick(None).expect("tick");
        let cmd1 = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(
            cmd1 > cmd0 && cmd1 < target,
            "first tick slews toward target"
        );
    }

    #[test]
    fn planner_drifted_detects_stale_init_before_large_hold_at() {
        let stale = JointPositionPlanner::new_at(0.0);
        assert!(planner_premature_hold(&stale, 2.9, 0.0));
        assert!(!planner_drifted_from_measurement(&stale, 2.9, 0.0, 0.15));
        let aligned = JointPositionPlanner::new_for_target(2.9, 0.0);
        assert!(!planner_drifted_from_measurement(&aligned, 2.9, 0.0, 0.15));
    }

    #[test]
    fn trapezoid_trajectory_accumulates_independently_of_max_lead() {
        let target = 1.2;
        let max_lead = 0.03;
        let mut planner = JointPositionPlanner::new_for_target(0.0, target);
        let dt = 0.005;
        for _ in 0..100 {
            planner.tick(target, dt, 0.08, 0.36);
        }
        assert!(
            planner.q_traj > 0.01,
            "planner trajectory must advance at trapezoid rate even when q lags"
        );
        let q_des = clamp_trajectory_setpoint(planner.q_traj, 0.0, target, max_lead, None, 0.0);
        assert!(
            q_des > 0.0,
            "MIT setpoint follows planner when measured q lags"
        );
        assert!(
            planner.q_traj >= q_des,
            "trajectory runs ahead of or meets clamped MIT command"
        );
    }

    fn shoulder_limit_policy() -> JointLimitPolicy {
        JointLimitPolicy {
            bounds: armee_kinematics::JointLimitBounds::from_hard_and_soft(
                -0.9,
                3.17,
                Some(-0.872665),
                Some(3.141593),
            )
            .expect("bounds"),
            margin: armee_kinematics::LimitMarginConfig {
                min_rad: 0.01,
                k_v_s: 0.02,
                k_stop: 0.5,
                velocity_deadband_rad_s: 0.02,
                measured_fault_slack_rad: 0.005,
                decel_rad_s2: 4.8,
            },
            velocity: 2.0,
            effort: 10.0,
            tau_ff_max: 5.0,
        }
    }

    #[test]
    fn overshoot_past_soft_bottom_stays_above_hard_envelope() {
        let policy = shoulder_limit_policy();
        let target = -0.872665;
        let q = -0.92;
        let q_des = clamp_trajectory_setpoint(-0.877, q, target, 0.10, Some(&policy), -1.5);
        assert!(
            q_des >= policy.hard_lower(),
            "q_des {q_des} must not command past hard lower {}",
            policy.hard_lower()
        );
    }

    #[test]
    fn limit_clamped_sub_home_target_does_not_seed_downward_return() {
        let mut loop_ctrl = test_loop();
        virtual_ready_active(&mut loop_ctrl);
        // Past soft/hard lower (~-1.09/-1.12) so clamp_hold_target must raise the goal.
        loop_ctrl
            .enter_position_hold_at(Some("right_shoulder_pitch"), -1.2)
            .expect("hold-at");
        let i = loop_ctrl
            .joint_names()
            .iter()
            .position(|n| n == "right_shoulder_pitch")
            .expect("joint index");
        let target = loop_ctrl.position_setpoints().expect("setpoints")[i];
        assert!(
            target > -1.2,
            "requested lower-limit probe must clamp before planner reset"
        );
        let (_q_traj, dq_traj) = loop_ctrl
            .test_planner_state("right_shoulder_pitch")
            .expect("planner");
        assert!(
            dq_traj.abs() < 1e-12,
            "clamped negative target must not seed downward velocity"
        );
        loop_ctrl.tick(None).expect("tick");
        let cmd = loop_ctrl.position_hold_commands().expect("commands")[i];
        assert!(
            cmd > -0.20,
            "first tick must ramp from measured q, not jump to clamped target: {cmd}"
        );
        assert!(
            cmd > target,
            "planner must approach the clamped target gradually: cmd={cmd} target={target}"
        );
    }

    fn assert_small_move_slew_output(initial: f32, target: f64) {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        virtual_ready_active_at(&mut loop_ctrl, Some((joint, f64::from(initial), 0.0)));
        loop_ctrl
            .enter_position_hold_at(Some(joint), target)
            .expect("hold-at");
        let index = loop_ctrl
            .joint_names()
            .iter()
            .position(|name| name == joint)
            .expect("pitch index");
        let mut previous = loop_ctrl
            .position_hold_commands()
            .expect("initial planner output")[index];
        let mut max_speed: f64 = 0.0;
        let mut final_reference = previous;
        for _ in 0..120 {
            // Stationary raw encoder observations are independent of the planner.
            // This verifies controller output, not simulated plant tracking.
            replay_feedback(&mut loop_ctrl, joint, initial, 0.0);
            loop_ctrl.supervisor_mut().bus_mut().clear_trace();
            loop_ctrl.tick(None).expect("stationary output tick");
            let reference = loop_ctrl.position_hold_commands().expect("planner output")[index];
            // Literal master small-move contract: 0.15 rad/s at 200 Hz.
            assert!(
                (reference - previous).abs() <= 0.15 * 0.005 + 1e-9,
                "planner jumped from {previous} to {reference}"
            );
            previous = reference;
            final_reference = reference;
            let frame = loop_ctrl
                .supervisor()
                .bus()
                .frames()
                .iter()
                .find(|frame| (frame.id >> 24) & 0x1f == 1 && frame.id & 0xff == 1)
                .expect("actual pitch MIT output");
            let speed = -(f64::from(u16::from_be_bytes([frame.data[2], frame.data[3]])) / 32767.0
                - 1.0)
                * 20.0;
            max_speed = max_speed.max(speed.abs());
            assert!(
                speed.abs() <= 0.152,
                "wire speed exceeded small-move contract: {speed}"
            );
            let command =
                -(f64::from(u16::from_be_bytes([frame.data[0], frame.data[1]])) / 32767.0 - 1.0)
                    * 4.0
                    * std::f64::consts::PI;
            assert!(
                (command - f64::from(initial)).abs() <= 0.121,
                "wire setpoint exceeded configured lead: {command}"
            );
        }
        assert!(max_speed > 0.01, "test must reach moving setpoints");
        assert!(
            (final_reference - target).abs() < (f64::from(initial) - target).abs() - 0.01,
            "planner must progress toward target, got {final_reference}"
        );
    }

    #[test]
    fn layer2_hold_at_uses_slew_profile_not_trajectory() {
        assert_small_move_slew_output(0.0, 0.1);
    }

    #[test]
    fn layer2_return_home_stays_on_slew_profile() {
        assert_small_move_slew_output(0.1, 0.0);
    }

    // ════════════════════════════════════════════════════════════════
    // Gain override tests
    // ════════════════════════════════════════════════════════════════

    fn apply_override_in_impedance(
        loop_ctrl: &mut ControlLoop<SimulationBus>,
        joint: &str,
        ov: GainOverride,
    ) {
        loop_ctrl.set_control_mode(ControlMode::Impedance);
        loop_ctrl
            .apply_gain_override(joint, ov)
            .expect("valid gain override");
    }

    #[test]
    fn apply_gain_override_clamps_kp_to_kp_max() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 6000.0,
                kd: 1.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!(
            (stored.kp - 5000.0).abs() < 1e-9,
            "kp clamped: {}",
            stored.kp
        );
    }

    #[test]
    fn apply_gain_override_clamps_kd_to_kd_max() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 10.0,
                kd: 200.0,
                ki: 0.0,
                fc: 1.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!(
            (stored.kd - 100.0).abs() < 1e-9,
            "kd clamped: {}",
            stored.kd
        );
    }

    #[test]
    fn apply_gain_override_clamps_fc_to_tau_ff_max_nm() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 10.0,
                kd: 1.0,
                ki: 0.0,
                fc: 10.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!((stored.fc - 5.0).abs() < 1e-9, "fc clamped: {}", stored.fc);
    }

    #[test]
    fn apply_gain_override_clamps_ki_to_kp_max() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 10.0,
                kd: 1.0,
                ki: 10000.0,
                fc: 1.0,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!(
            (stored.ki - 5000.0).abs() < 1e-9,
            "ki clamped: {}",
            stored.ki
        );
    }

    #[test]
    fn clear_gain_override_removes_entry() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 100.0,
                kd: 10.0,
                ki: 0.0,
                fc: 2.0,
            },
        );
        assert!(loop_ctrl.gain_override(joint).is_some());
        loop_ctrl.clear_gain_override(joint);
        assert!(loop_ctrl.gain_override(joint).is_none());
    }

    #[test]
    fn no_override_config_gains_unchanged() {
        let loop_ctrl = test_loop();
        assert!(loop_ctrl.gain_override("right_shoulder_pitch").is_none());
        let cfg = loop_ctrl
            .supervisor
            .control
            .control
            .joints
            .get("right_shoulder_pitch")
            .expect("joint cfg exists");
        // Config gains exist and are non-zero (exact values depend on repo config).
        assert!(
            cfg.impedance.kp > 0.0,
            "kp must be > 0: {}",
            cfg.impedance.kp
        );
        assert!(
            cfg.impedance.kd > 0.0,
            "kd must be > 0: {}",
            cfg.impedance.kd
        );
    }

    #[test]
    fn apply_gain_override_stores_within_limits_as_is() {
        let mut loop_ctrl = test_loop();
        let joint = "right_shoulder_pitch";
        apply_override_in_impedance(
            &mut loop_ctrl,
            joint,
            GainOverride {
                kp: 50.0,
                kd: 5.0,
                ki: 0.5,
                fc: 1.5,
            },
        );
        let stored = loop_ctrl.gain_override(joint).expect("override stored");
        assert!((stored.kp - 50.0).abs() < 1e-9);
        assert!((stored.kd - 5.0).abs() < 1e-9);
        assert!((stored.ki - 0.5).abs() < 1e-9);
        assert!((stored.fc - 1.5).abs() < 1e-9);
    }
}

#[cfg(test)]
#[path = "loop_wp_d_tests.rs"]
mod wp_d_tests;
