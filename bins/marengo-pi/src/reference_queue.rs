//! Operator reference queue: acquires joints one at a time through Davout's
//! qualified physical reference workflow and defers other operator commands
//! while it is busy. Pure state machine over [`ReferenceDriver`] so the
//! stdin contract is testable without hardware.

use std::collections::VecDeque;
use std::fmt;

use davout::{MotorBus, ReferenceAudit, ReferenceHandle, ReferenceOutcome, Supervisor};

#[cfg(test)]
#[path = "reference_queue_tests.rs"]
mod tests;

/// Owner operations the queue needs; implemented by [`Supervisor`].
pub(crate) trait ReferenceDriver {
    type Handle;

    fn hardware_estop_asserted(&self) -> bool;

    fn request(&mut self, joint: &str, audit: ReferenceAudit) -> Result<Self::Handle, String>;

    fn outcome(&self, handle: &Self::Handle) -> Result<ReferenceOutcome, String>;
}

impl<B: MotorBus> ReferenceDriver for Supervisor<B> {
    type Handle = ReferenceHandle;

    fn hardware_estop_asserted(&self) -> bool {
        self.safety_snapshot().hardware_estop_asserted
    }

    fn request(&mut self, joint: &str, audit: ReferenceAudit) -> Result<ReferenceHandle, String> {
        // Admission already required the operator's sign-tested attestation.
        self.request_reference(joint, true, audit)
            .map_err(|e| e.to_string())
    }

    fn outcome(&self, handle: &ReferenceHandle) -> Result<ReferenceOutcome, String> {
        self.reference_outcome(handle).map_err(|e| e.to_string())
    }
}

/// One operator-visible queue transition. `Display` is the stdout contract.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReferenceEvent {
    Current { joint: String, position_rad: f32 },
    Failed { joint: String, message: String },
    Skipped { joint: String },
    DeferredDiscarded { count: usize },
}

impl fmt::Display for ReferenceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Current {
                joint,
                position_rad,
            } => write!(f, "reference {joint} current pos={position_rad:.4}"),
            Self::Failed { joint, message } => write!(f, "reference {joint} failed: {message}"),
            Self::Skipped { joint } => write!(f, "reference {joint} skipped: earlier joint failed"),
            Self::DeferredDiscarded { count } => write!(
                f,
                "discarded {count} deferred command(s): reference queue cancelled"
            ),
        }
    }
}

pub(crate) const CANCELLED: &str = "cancelled";

#[derive(Debug)]
struct QueuedReference {
    joint: String,
    operator: String,
}

/// Joints still to acquire, the in-flight request and commands deferred
/// until the queue drains. `C` is the deferred command type.
#[derive(Debug)]
pub(crate) struct ReferenceQueue<H, C> {
    session: String,
    pending: VecDeque<QueuedReference>,
    in_flight: Option<(QueuedReference, H)>,
    deferred: VecDeque<C>,
}

impl<H, C> ReferenceQueue<H, C> {
    pub(crate) fn new(session: String) -> Self {
        Self {
            session,
            pending: VecDeque::new(),
            in_flight: None,
            deferred: VecDeque::new(),
        }
    }

    /// A request is in flight or joints remain queued.
    pub(crate) fn is_busy(&self) -> bool {
        self.in_flight.is_some() || !self.pending.is_empty()
    }

    /// Validate and queue `joints` in order; refusals queue nothing.
    pub(crate) fn admit(
        &mut self,
        joints: &[String],
        sign_tested: bool,
        operator: &str,
        is_known_joint: impl Fn(&str) -> bool,
    ) -> Result<(), String> {
        if !sign_tested {
            return Err("sign-tested required (operator attestation)".into());
        }
        if joints.is_empty() {
            return Err("no joints given".into());
        }
        if let Some(joint) = joints.iter().find(|joint| !is_known_joint(joint)) {
            return Err(format!("unknown joint {joint}"));
        }
        if self.is_busy() {
            return Err("reference queue busy".into());
        }
        self.pending
            .extend(joints.iter().map(|joint| QueuedReference {
                joint: joint.clone(),
                operator: operator.to_owned(),
            }));
        Ok(())
    }

    pub(crate) fn defer(&mut self, command: C) {
        self.deferred.push_back(command);
    }

    /// Next deferred command once the queue has drained.
    pub(crate) fn take_ready_deferred(&mut self) -> Option<C> {
        if self.is_busy() {
            None
        } else {
            self.deferred.pop_front()
        }
    }

    /// Advance after a control tick: poll the in-flight request and start the
    /// next joint when it completes. Hardware E-stop cancels the queue.
    pub(crate) fn pump<D: ReferenceDriver<Handle = H>>(
        &mut self,
        driver: &mut D,
    ) -> Vec<ReferenceEvent> {
        let mut events = Vec::new();
        if !self.is_busy() {
            return events;
        }
        if driver.hardware_estop_asserted() {
            return self.cancel();
        }
        if let Some((_, handle)) = &self.in_flight {
            let result = match driver.outcome(handle) {
                Ok(ReferenceOutcome::InProgress) => return events,
                Ok(ReferenceOutcome::Current { position_rad }) => Ok(position_rad),
                Ok(ReferenceOutcome::Failed { message }) | Err(message) => Err(message),
            };
            let Some((entry, _)) = self.in_flight.take() else {
                return events;
            };
            match result {
                Ok(position_rad) => events.push(ReferenceEvent::Current {
                    joint: entry.joint,
                    position_rad,
                }),
                Err(message) => {
                    events.push(ReferenceEvent::Failed {
                        joint: entry.joint,
                        message,
                    });
                    self.skip_pending(&mut events);
                    return events;
                }
            }
        }
        // Start the next joint; its outcome is polled after the next tick.
        let Some(next) = self.pending.pop_front() else {
            return events;
        };
        let audit = ReferenceAudit {
            operator: next.operator.clone(),
            session: self.session.clone(),
        };
        match driver.request(&next.joint, audit) {
            Ok(handle) => self.in_flight = Some((next, handle)),
            Err(message) => {
                events.push(ReferenceEvent::Failed {
                    joint: next.joint,
                    message,
                });
                self.skip_pending(&mut events);
            }
        }
        events
    }

    /// Cancel every unfinished joint and discard deferred commands. Callers
    /// stop the owner's transaction themselves (Disable / shutdown).
    pub(crate) fn cancel(&mut self) -> Vec<ReferenceEvent> {
        let mut events: Vec<ReferenceEvent> = self
            .in_flight
            .take()
            .map(|(entry, _)| entry)
            .into_iter()
            .chain(self.pending.drain(..))
            .map(|entry| ReferenceEvent::Failed {
                joint: entry.joint,
                message: CANCELLED.into(),
            })
            .collect();
        if !self.deferred.is_empty() {
            events.push(ReferenceEvent::DeferredDiscarded {
                count: self.deferred.len(),
            });
            self.deferred.clear();
        }
        events
    }

    fn skip_pending(&mut self, events: &mut Vec<ReferenceEvent>) {
        events.extend(
            self.pending
                .drain(..)
                .map(|entry| ReferenceEvent::Skipped { joint: entry.joint }),
        );
    }
}
