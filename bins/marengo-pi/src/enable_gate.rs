//! Stdin Enable and Position-mode arms wait for enable completion.
//!
//! Every Enable path first runs the gravity saturation preflight
//! ([`GravitySweep`]), one bounded slice per control tick so the loop keeps
//! draining CAN. While it runs, the request waits here ([`PreflightFor`]);
//! later stdin commands except `disable`/`quit`, and later Chappe
//! `robot/enable` requests, wait behind it in arrival order. When the sweep
//! passes, the request continues and the waiting commands run afterwards, as
//! if the Enable had completed synchronously. When it refuses or is voided
//! ([`SweepAbort`]), the request is refused and the waiting commands are
//! discarded.
//!
//! `enable_targets` returns once the session is Active, but a target's Enable
//! can still be held (staggered, or for its drive's post-SetZero quiet) and no
//! target has session pose yet. `enabled (operator=…)` is printed only once
//! [`ControlLoop::enable_completion`] holds, so clients that wait for it never
//! arm Position mode early. `hold-on`, `hold-at` and `wave` that arrive before
//! then are deferred ("waiting for enable to complete") and retried after each
//! control tick, in order. Both waits are bounded by
//! [`ENABLE_COMPLETION_TIMEOUT`]: an Enable that does not complete is refused
//! (`enable failed:`) and every drive is stopped; a deferred arm is refused
//! (`<command> failed:`).

use std::collections::VecDeque;
use std::time::Instant;

use armee_proto::{EnableRequest, MitCommandBatch};
use berthier::{ControlLoop, LoopError, ENABLE_COMPLETION_TIMEOUT};
use davout::{ControlMode, MotorBus};
use tracing::warn;

use crate::gravity_preflight::{GravitySweep, SweepAbort};
use crate::reference_queue::MAX_DEFERRED_COMMANDS;
use crate::PiCommand;

/// What runs once the gravity preflight passes.
pub(crate) enum PreflightFor {
    StdinEnable {
        operator_id: String,
    },
    ChappeEnable(EnableRequest),
    /// A Testing Position batch that would auto-enable the drives.
    TestingBatch(MitCommandBatch),
}

struct PendingPreflight {
    sweep: GravitySweep,
    then: PreflightFor,
}

/// A sweep that ended in refusal or was voided, with its request.
pub(crate) struct PreflightRefusal {
    pub(crate) then: PreflightFor,
    /// `None`: the sweep finished and the preflight refused.
    pub(crate) abort: Option<SweepAbort>,
}

/// One stdout or stderr line, in emission order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GateLine {
    Out(String),
    Err(String),
}

pub(crate) fn emit(lines: Vec<GateLine>) {
    for line in lines {
        match line {
            GateLine::Out(text) => println!("{text}"),
            GateLine::Err(text) => eprintln!("{text}"),
        }
    }
}

struct PendingEnable {
    operator_id: String,
    targets: Vec<String>,
    deadline: Instant,
}

struct DeferredArm {
    cmd: PiCommand,
    deadline: Instant,
}

#[derive(Default)]
pub(crate) struct EnableGate {
    enable: Option<PendingEnable>,
    deferred: VecDeque<DeferredArm>,
    preflight: Option<PendingPreflight>,
    /// stdin commands received while the sweep runs, in arrival order.
    deferred_stdin: VecDeque<PiCommand>,
    /// Chappe `robot/enable` requests received while the sweep runs.
    deferred_chappe: VecDeque<EnableRequest>,
    /// Ended Chappe-side sweeps whose refusal still has to be published.
    unreported: Vec<PreflightRefusal>,
}

impl EnableGate {
    pub(crate) fn preflight_pending(&self) -> bool {
        self.preflight.is_some()
    }

    /// Hold `then` until `sweep` reaches a verdict. Only one sweep runs at a
    /// time: while one is pending, `then` is handed back.
    pub(crate) fn begin_preflight(
        &mut self,
        sweep: GravitySweep,
        then: PreflightFor,
    ) -> Result<(), PreflightFor> {
        if self.preflight.is_some() {
            return Err(then);
        }
        self.preflight = Some(PendingPreflight { sweep, then });
        Ok(())
    }

    pub(crate) fn sweep_mut(&mut self) -> Option<&mut GravitySweep> {
        self.preflight.as_mut().map(|pending| &mut pending.sweep)
    }

    /// Hold a stdin command until the sweep ends; false when the bound is hit.
    pub(crate) fn defer_stdin(&mut self, cmd: PiCommand) -> bool {
        if self.deferred_stdin.len() >= MAX_DEFERRED_COMMANDS {
            return false;
        }
        self.deferred_stdin.push_back(cmd);
        true
    }

    /// Next stdin command held behind a sweep that passed.
    pub(crate) fn take_ready_stdin(&mut self) -> Option<PiCommand> {
        if self.preflight.is_some() {
            None
        } else {
            self.deferred_stdin.pop_front()
        }
    }

    /// Hold a Chappe enable until the sweep ends; false when the bound is hit.
    pub(crate) fn defer_chappe_enable(&mut self, request: EnableRequest) -> bool {
        if self.deferred_chappe.len() >= MAX_DEFERRED_COMMANDS {
            return false;
        }
        self.deferred_chappe.push_back(request);
        true
    }

    /// Next Chappe enable held behind a sweep that passed.
    pub(crate) fn take_ready_chappe_enable(&mut self) -> Option<EnableRequest> {
        if self.preflight.is_some() {
            None
        } else {
            self.deferred_chappe.pop_front()
        }
    }

    /// The sweep passed: hand back its request. Held commands run after it.
    pub(crate) fn pass_preflight(&mut self) -> Option<PreflightFor> {
        self.preflight.take().map(|pending| pending.then)
    }

    /// The sweep refused (`abort` is `None`) or was voided: refuse its request
    /// and discard every held command. A stdin Enable's refusal is returned
    /// now; a Chappe request's waits for [`Self::take_unreported`].
    pub(crate) fn end_preflight(&mut self, abort: Option<SweepAbort>) -> Vec<GateLine> {
        let mut lines = Vec::new();
        let Some(pending) = self.preflight.take() else {
            return lines;
        };
        match pending.then {
            PreflightFor::StdinEnable { .. } => lines.push(GateLine::Err(match &abort {
                None => "enable refused: gravity saturation preflight failed closed".into(),
                Some(abort) => {
                    format!("enable refused: gravity saturation preflight aborted: {abort}")
                }
            })),
            then => self.unreported.push(PreflightRefusal { then, abort }),
        }
        let count = self.deferred_stdin.len();
        if count > 0 {
            self.deferred_stdin.clear();
            lines.push(GateLine::Out(format!(
                "discarded {count} deferred command(s)"
            )));
        }
        for request in self.deferred_chappe.drain(..) {
            warn!(
                operator = %request.operator_id,
                "Chappe enable discarded: the gravity preflight it waited behind did not pass"
            );
        }
        lines
    }

    /// Chappe-side refusals not yet published.
    pub(crate) fn take_unreported(&mut self) -> Vec<PreflightRefusal> {
        std::mem::take(&mut self.unreported)
    }

    /// `enable_targets` accepted `targets`; `enabled` follows once complete.
    pub(crate) fn begin_enable(
        &mut self,
        operator_id: String,
        targets: Vec<String>,
        now: Instant,
    ) -> Vec<GateLine> {
        let line = GateLine::Out(format!(
            "waiting for enable to complete (operator={operator_id}) targets={}",
            targets.join(",")
        ));
        self.enable = Some(PendingEnable {
            operator_id,
            targets,
            deadline: now + ENABLE_COMPLETION_TIMEOUT,
        });
        vec![line]
    }

    /// Run a Position-mode arm now, or defer it while an Enable is pending,
    /// an earlier arm is deferred, or it refuses with
    /// [`LoopError::EnableIncomplete`].
    pub(crate) fn submit<B: MotorBus>(
        &mut self,
        loop_ctrl: &mut ControlLoop<B>,
        cmd: PiCommand,
        now: Instant,
    ) -> Vec<GateLine> {
        let name = arm_name(&cmd);
        if self.enable.is_some() || !self.deferred.is_empty() {
            self.deferred.push_back(DeferredArm {
                cmd,
                deadline: now + ENABLE_COMPLETION_TIMEOUT,
            });
            return vec![GateLine::Out(format!(
                "{name} waiting for enable to complete"
            ))];
        }
        match run_arm(loop_ctrl, &cmd) {
            Ok(lines) => lines,
            Err(LoopError::EnableIncomplete { detail }) => {
                self.deferred.push_back(DeferredArm {
                    cmd,
                    deadline: now + ENABLE_COMPLETION_TIMEOUT,
                });
                vec![GateLine::Out(format!(
                    "{name} waiting for enable to complete: {detail}"
                ))]
            }
            Err(error) => vec![GateLine::Err(format!("{name} failed: {error}"))],
        }
    }

    /// After each control tick: report a completed Enable, refuse one past its
    /// bound (stopping every drive), then retry deferred arms in order.
    pub(crate) fn poll<B: MotorBus>(
        &mut self,
        loop_ctrl: &mut ControlLoop<B>,
        now: Instant,
    ) -> Vec<GateLine> {
        let mut lines = Vec::new();
        if let Some(pending) = &self.enable {
            match loop_ctrl.enable_completion() {
                Ok(()) => {
                    lines.push(GateLine::Out(format!(
                        "enabled (operator={}) targets={}",
                        pending.operator_id,
                        pending.targets.join(",")
                    )));
                    self.enable = None;
                }
                Err(error) => {
                    let lost = loop_ctrl.supervisor().mode() != davout::OperationalMode::Active;
                    if !lost && now < pending.deadline {
                        return lines;
                    }
                    let message = if lost {
                        format!("enable failed: {error}")
                    } else {
                        format!(
                            "enable failed: not complete within {} ms: {error}",
                            ENABLE_COMPLETION_TIMEOUT.as_millis()
                        )
                    };
                    self.enable = None;
                    let _ = loop_ctrl.supervisor_mut().disable_all();
                    loop_ctrl.set_control_mode(ControlMode::Disabled);
                    lines.push(GateLine::Err(message));
                    lines.extend(self.refuse_deferred("enable did not complete"));
                    return lines;
                }
            }
        }
        while let Some(front) = self.deferred.front() {
            let name = arm_name(&front.cmd);
            let deadline = front.deadline;
            match run_arm(loop_ctrl, &front.cmd) {
                Ok(done) => lines.extend(done),
                Err(LoopError::EnableIncomplete { .. }) if now < deadline => break,
                Err(error @ LoopError::EnableIncomplete { .. }) => {
                    lines.push(GateLine::Err(format!(
                        "{name} failed: {error} (no completion within {} ms)",
                        ENABLE_COMPLETION_TIMEOUT.as_millis()
                    )));
                }
                Err(error) => lines.push(GateLine::Err(format!("{name} failed: {error}"))),
            }
            self.deferred.pop_front();
        }
        lines
    }

    /// Disable: no gravity sweep, held command, pending Enable or deferred
    /// arm survives it.
    pub(crate) fn cancel(&mut self) -> Vec<GateLine> {
        let mut lines = self.end_preflight(Some(SweepAbort::Disabled));
        if self.enable.take().is_some() {
            lines.push(GateLine::Err(
                "enable failed: disabled before enable completed".into(),
            ));
        }
        lines.extend(self.refuse_deferred("disabled before enable completed"));
        lines
    }

    /// `hold-off`: deferred arms must not re-arm Position mode afterwards.
    pub(crate) fn cancel_arms(&mut self, reason: &str) -> Vec<GateLine> {
        self.refuse_deferred(reason)
    }

    fn refuse_deferred(&mut self, reason: &str) -> Vec<GateLine> {
        self.deferred
            .drain(..)
            .map(|arm| GateLine::Err(format!("{} failed: {reason}", arm_name(&arm.cmd))))
            .collect()
    }
}

fn arm_name(cmd: &PiCommand) -> &'static str {
    match cmd {
        PiCommand::HoldOn => "hold-on",
        PiCommand::HoldAt { .. } => "hold-at",
        PiCommand::Wave { .. } => "wave",
        _ => "command",
    }
}

fn run_arm<B: MotorBus>(
    loop_ctrl: &mut ControlLoop<B>,
    cmd: &PiCommand,
) -> Result<Vec<GateLine>, LoopError> {
    let mut lines = Vec::new();
    match cmd {
        PiCommand::HoldOn => {
            loop_ctrl.enter_position_hold()?;
            lines.push(GateLine::Out(format!(
                "control mode → Position hold (operational={:?})",
                loop_ctrl.supervisor().mode()
            )));
            if let Some(setpoints) = loop_ctrl.position_setpoints() {
                for (name, q) in loop_ctrl.joint_names().iter().zip(setpoints) {
                    lines.push(GateLine::Out(format!("  hold {name} = {q:.4} rad")));
                }
            }
        }
        PiCommand::HoldAt {
            joint,
            position_rad,
        } => {
            loop_ctrl.enter_position_hold_at(joint.as_deref(), *position_rad)?;
            lines.push(GateLine::Out(format!(
                "control mode → Position hold → target {position_rad:.4} rad (ramping, operational={:?})",
                loop_ctrl.supervisor().mode()
            )));
            if let Some(joint) = joint.as_deref() {
                lines.push(GateLine::Out(format!("  joint {joint}")));
            }
        }
        PiCommand::Wave {
            joint,
            min_rad,
            max_rad,
            cycles,
            half_period_sec,
        } => {
            let duration_sec = loop_ctrl.start_position_wave(
                joint,
                *min_rad,
                *max_rad,
                *cycles,
                *half_period_sec,
            )?;
            lines.push(GateLine::Out(format!(
                "position wave → {joint} {min_rad:.4}↔{max_rad:.4} rad ×{cycles} (~{duration_sec:.2}s, operational={:?})",
                loop_ctrl.supervisor().mode()
            )));
        }
        _ => {}
    }
    Ok(lines)
}
