//! Single runtime motion owner (WP-F, L-marengo-pi-01).
//!
//! marengo-pi has two command sources: the operator's stdin (MCP scripted
//! bench sessions, a person on SSH) and Chappe (Consul through the gateway).
//! Exactly one of them owns motion for the life of the process. The owner is
//! claimed at launch (`--motion-owner stdin|chappe`, or `MARENGO_MOTION_OWNER`)
//! and released when the process exits; there is no runtime hand-over, so two
//! operators can never both steer the same arm.
//!
//! Commands are classified, not trusted by origin:
//!
//! * [`CommandClass::Stop`] (disable, hold-off, quit) is admitted from either
//!   source, always. Stopping never needs ownership.
//! * [`CommandClass::Observe`] (status, diagnostics) is admitted from either
//!   source; it cannot energise, move or retune a drive.
//! * [`CommandClass::Motion`] (enable, reference/set-zero, hold, wave, torque,
//!   mode changes, Testing batches, runtime gains) is admitted only from the
//!   owner. A non-owner is refused with a reason that is logged and published
//!   as an `ActionEvent` on `robot/audit/action`.
//!
//! Configuration-plane Chappe commands (Set Limits, config overlay tuning) are
//! not motion and are not gated here; they stay refused while a reference is
//! busy.

use std::fmt;
use std::str::FromStr;

use armee_proto::ActionEvent;
use chappe::Bus;
use thiserror::Error;
use tracing::warn;

use crate::limit_persist::next_audit_revision;
use crate::overlay::publish_action_event;
use crate::{timestamp_ms, PiCommand};

/// Audit `ActionEvent.action` for every refused motion command.
pub(crate) const MOTION_REFUSED_ACTION: &str = "motion_refused";

/// Environment fallback for `--motion-owner`.
pub(crate) const MOTION_OWNER_ENV: &str = "MARENGO_MOTION_OWNER";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandSource {
    /// Piped or typed stdin grammar (`hold-at`, `enable bench`, …).
    Stdin,
    /// Chappe commands published by the gateway on behalf of Consul.
    Chappe,
}

impl fmt::Display for CommandSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stdin => "stdin",
            Self::Chappe => "chappe",
        })
    }
}

impl FromStr for CommandSource {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "stdin" => Ok(Self::Stdin),
            "chappe" => Ok(Self::Chappe),
            other => Err(format!(
                "unknown motion owner {other:?} (expected stdin or chappe)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandClass {
    /// Disable / hold-off / quit: always admitted.
    Stop,
    /// Read-only or diagnostic: admitted from any source.
    Observe,
    /// Energises, moves, references or retunes: owner only.
    Motion,
}

/// Why a non-owner command was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "{command} refused: motion is owned by {owner}, not {from}; only disable/stop is accepted \
     from {from} (start marengo-pi with --motion-owner {from} to own motion)"
)]
pub(crate) struct MotionRefusal {
    pub(crate) command: &'static str,
    pub(crate) from: CommandSource,
    pub(crate) owner: CommandSource,
}

/// The process-lifetime motion ownership claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MotionLease {
    owner: CommandSource,
}

impl MotionLease {
    pub(crate) const fn new(owner: CommandSource) -> Self {
        Self { owner }
    }

    /// Admit `command` of `class` from `from`, or say why not.
    pub(crate) fn admit(
        self,
        from: CommandSource,
        class: CommandClass,
        command: &'static str,
    ) -> Result<(), MotionRefusal> {
        match class {
            CommandClass::Stop | CommandClass::Observe => Ok(()),
            CommandClass::Motion if from == self.owner => Ok(()),
            CommandClass::Motion => Err(MotionRefusal {
                command,
                from,
                owner: self.owner,
            }),
        }
    }
}

/// Classify one stdin command.
pub(crate) fn classify_stdin(cmd: &PiCommand) -> (CommandClass, &'static str) {
    match cmd {
        PiCommand::Disable => (CommandClass::Stop, "disable"),
        PiCommand::Quit => (CommandClass::Stop, "quit"),
        // Both only drop the control mode to neutral (kp = kd = τ = 0).
        PiCommand::HoldOff => (CommandClass::Stop, "hold-off"),
        PiCommand::ImpedanceOff => (CommandClass::Stop, "impedance-off"),
        PiCommand::Status => (CommandClass::Observe, "status"),
        PiCommand::Home => (CommandClass::Motion, "home"),
        PiCommand::HomeJoints { .. } => (CommandClass::Motion, "home <joints>"),
        PiCommand::Enable { .. } => (CommandClass::Motion, "enable"),
        PiCommand::GravityOn => (CommandClass::Motion, "gravity-on"),
        PiCommand::GravityOff => (CommandClass::Motion, "gravity-off"),
        PiCommand::TorqueCmd { .. } => (CommandClass::Motion, "torque-cmd"),
        PiCommand::ImpedanceOn => (CommandClass::Motion, "impedance-on"),
        PiCommand::HoldOn => (CommandClass::Motion, "hold-on"),
        PiCommand::HoldAt { .. } => (CommandClass::Motion, "hold-at"),
        PiCommand::Wave { .. } => (CommandClass::Motion, "wave"),
    }
}

/// Resolve the owner: CLI flag, else [`MOTION_OWNER_ENV`], else Chappe (the
/// systemd service has no stdin and is steered by Consul).
pub(crate) fn resolve_owner(
    flag: Option<&str>,
    env_value: Option<&str>,
) -> Result<CommandSource, String> {
    match flag.or(env_value) {
        Some(value) => value.parse(),
        None => Ok(CommandSource::Chappe),
    }
}

/// Publish one audit `ActionEvent` (no persistence involved).
pub(crate) fn publish_audit(
    chappe: &Bus,
    action: &str,
    accepted: bool,
    operator_id: &str,
    joint: &str,
    reason: &str,
) {
    let event = ActionEvent {
        timestamp_ms: timestamp_ms(),
        session_id: String::new(),
        operator_id: operator_id.to_owned(),
        joint: joint.to_owned(),
        action: action.to_owned(),
        revision: next_audit_revision(),
        accepted,
        reject_reason: reason.to_owned(),
        persist_status: armee_proto::PersistStatus::NotApplicable as i32,
        config_revision: String::new(),
    };
    if let Err(error) = publish_action_event(chappe, &event) {
        warn!(%error, action, "failed to publish audit event");
    }
}

/// Log a refusal and publish it so Consul and the audit trail can show why.
pub(crate) fn report_refusal(
    chappe: &Bus,
    operator_id: &str,
    joint: &str,
    command: &str,
    reason: &str,
) {
    warn!(command, joint, reason, "motion command refused");
    publish_audit(
        chappe,
        MOTION_REFUSED_ACTION,
        false,
        operator_id,
        joint,
        reason,
    );
}

#[cfg(test)]
#[path = "motion_owner_tests.rs"]
mod tests;
