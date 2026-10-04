//! # Berthier — realtime control (outer loop)
//!
//! Berthier owns **what to command each tick**: read joint state, compute feedforward and
//! gains, assemble MIT setpoints. It does **not** talk to CAN, enforce safety limits, or
//! encode vendor frames.
//!
//! ## Responsibilities
//!
//! - [`ControlLoop::tick`](loop::ControlLoop::tick): `recv` → `q` → `tau_g(q)` → MIT batch → Davout.
//! - Control modes: gravity comp, impedance, position, torque-only ([`ControlMode`]).
//! - Position hold law lives in [`position_hold::PositionHold`] (targets, planners, MIT compose).
//! - MIT feedforward for GravityComp / Impedance / TorqueOnly: [`mit_feedforward::MitFeedforward`].
//! - TorqueOnly operator latch: [`torque_cmd::TorqueCmdLatch`] (`τ_cmd`; cleared on leave).
//! - [`ControlLoop::inhibit_motion_for_shutdown`]: discard retained intent before
//!   mandatory acquisition cleanup, configured Davout exit stop and persistence drain.
//! - A busy Davout reference reservation owns the tick's bounded receive/cleanup;
//!   Berthier inhibits intent without a competing drain or motion keepalive.
//! - Optional friction feedforward (`friction` module) in impedance and position modes.
//! - Publish [`RobotState`](armee_proto::RobotState) on Chappe (lower rate than the motor loop).
//! - Legacy [`Controller`]: single-joint position commands through Davout (REPL / bring-up).
//! - Concrete closed simulation construction through [`ControlLoop::from_simulation`]
//!   uses the same controller implementation with an explicit virtual initial
//!   reference condition. It provides software output coverage, not reference
//!   acquisition or physical commissioning proof.
//! - Explicit current-consuming virtual journal construction exercises actual reference
//!   acquisition and durable selected permission through the same owner/tick path.
//!
//! ## Does not
//!
//! - Open SocketCAN or call `robstride` (motor path is Davout → robstride only).
//! - Apply torque/position limits, E-stop, comm watchdog, or danger zones (Davout).
//! - Parse URDF for limits (uses [`armee-dynamics`] for `tau_g`, config for gains).
//!
//! ## Dependencies (allowed direction)
//!
//! ```text
//! armee-dynamics (tau_g) ──► berthier ──► davout ──► robstride
//! marengo-config (gains) ──┘              chappe (telemetry)
//! armee-proto (wire types)
//! ```
//!
//! ## Two mode enums (read with care)
//!
//! | Enum | Crate | Meaning |
//! |------|-------|---------|
//! | [`davout::OperationalMode`] | Davout | Disabled / Ready / Active — **may motors move?** |
//! | [`ControlMode`] | Berthier + Davout | GravityComp / Impedance / … — **how** to command when Active |
//!
//! See [ADR 0004](../../docs/decisions/0004-control-modes-and-mit.md).

mod degraded;
mod friction;
mod gain_runtime;
mod r#loop;
mod mit_feedforward;
mod position_feedforward;
mod position_hold;
mod position_profile;
mod position_setpoint;
mod position_trace;
mod position_trajectory;
mod position_wave;
mod torque_cmd;

#[cfg(test)]
mod mode_isolation;
#[cfg(test)]
mod reference_grant_tests;
#[cfg(test)]
mod reference_journal_tests;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_support;

pub use davout::ControlMode;
pub use degraded::{
    DegradedEvent, DegradedLowerCause, DriveLossAdmission, DEGRADED_REST_TOLERANCE_RAD,
};
pub use gain_runtime::{mode_allows_gain_override, GainOverride, GainShapeError};
pub use position_hold::HoldFuseTrip;
pub use r#loop::{
    proto_control_mode, ControlLoop, LoopError, TickPhaseAverages, ENABLE_COMPLETION_TIMEOUT,
};
