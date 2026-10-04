//! Degraded hold and controlled lower after a single drive loss (ADR 0038).
//!
//! Davout sheds the lost joint's subtree and keeps the proximal joints Active.
//! Berthier then freezes the holding joints in Position hold at their current
//! pose, with the shed joints' angles frozen in the gravity model, and after the
//! operator's `lower` or the hold window lowers them to rest at the configured
//! speed cap. Davout ends every episode with an all-address stop.

use std::time::{Duration, Instant};

use armee_dynamics::drive_loss::{subtree_loss_torque_bounds, HoldingTorqueBound};
use armee_dynamics::UrdfGravityModel;
use davout::{DegradedEnd, DriveLossPlan, MotorBus, Supervisor};
use marengo_config::{motor_type_key, OnDriveLoss};
use tracing::warn;

use crate::LoopError;

/// A holding joint counts as at rest within this distance of its target.
pub const DEGRADED_REST_TOLERANCE_RAD: f64 = 0.05;

/// Why the lower to rest started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradedLowerCause {
    /// The operator asked (`lower`).
    Operator,
    /// Nobody acted within the hold window.
    Timeout,
}

impl DegradedLowerCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Timeout => "timeout",
        }
    }
}

/// Episode transitions, in order, for the owner to report.
#[derive(Debug, Clone, PartialEq)]
pub enum DegradedEvent {
    /// Davout shed `shed` after losing `joint`; `holding` hold in place.
    DriveLost {
        joint: String,
        shed: Vec<String>,
        holding: Vec<String>,
        auto_lower_in: Duration,
    },
    LowerStarted {
        cause: DegradedLowerCause,
    },
    /// The holding joints reached rest; every drive was stopped.
    LowerComplete,
    /// Davout ended the episode before the lower completed.
    Ended {
        lost_joint: String,
        end: DegradedEnd,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DegradedPhase {
    Hold,
    LowerRequested(DegradedLowerCause),
    Lowering,
}

/// Berthier's side of an episode Davout started.
#[derive(Debug, Clone)]
pub(crate) struct DegradedControl {
    pub(crate) lost_joint: String,
    /// Indices into the loop's joint names.
    pub(crate) shed: Vec<usize>,
    pub(crate) holding: Vec<usize>,
    /// Gravity-model angle of each shed joint.
    pub(crate) frozen: Vec<(usize, f64)>,
    pub(crate) auto_lower_at: Instant,
    pub(crate) phase: DegradedPhase,
}

impl DegradedControl {
    pub(crate) fn is_shed(&self, index: usize) -> bool {
        self.shed.contains(&index)
    }
}

/// One admitted joint: what is shed, and the gravity bound that admitted it.
#[derive(Debug, Clone)]
pub struct DriveLossAdmission {
    pub plan: DriveLossPlan,
    pub bounds: Vec<HoldingTorqueBound>,
}

/// Admit every `on_drive_loss: shed_subtree` joint against the offline gravity
/// bound and build its plan. On every holding joint,
/// `max |τ_g| + max |Δτ_g| + tau_margin_nm` must stay within its feed-forward
/// cap (the smaller of its limit policy `tau_ff_max` and its motor type's
/// `tau_ff_max_nm`), with the shed subtree anywhere in its soft range and the
/// holding joints anywhere in theirs. A subtree that leaves nothing holding is
/// refused.
pub(crate) fn admit_drive_loss<B: MotorBus>(
    dynamics: &UrdfGravityModel,
    joint_names: &[String],
    supervisor: &Supervisor<B>,
) -> Result<Vec<DriveLossAdmission>, LoopError> {
    let control = &supervisor.control.control;
    let refuse = |joint: &str, message: String| LoopError::DriveLossAdmission {
        joint: joint.to_owned(),
        message,
    };
    let shedding: Vec<&String> = joint_names
        .iter()
        .filter(|joint| {
            control
                .joints
                .get(*joint)
                .is_some_and(|entry| entry.on_drive_loss == OnDriveLoss::ShedSubtree)
        })
        .collect();
    if shedding.is_empty() {
        return Ok(Vec::new());
    }
    let Some(drive_loss) = control.drive_loss.as_ref() else {
        return Err(refuse(
            shedding[0],
            "shed_subtree requires a control.drive_loss block".into(),
        ));
    };
    let mut ranges = Vec::with_capacity(joint_names.len());
    let mut caps = Vec::with_capacity(joint_names.len());
    for joint in joint_names {
        let Some(policy) = supervisor.joint_limit_policy(joint) else {
            // A narrowed process (MARENGO_JOINT_SUBSET) cannot bound the full
            // model: every loss stops every drive, as before ADR 0038.
            warn!(
                joint = %joint,
                "drive-loss admission skipped: joint not configured in this process"
            );
            return Ok(Vec::new());
        };
        let type_cap = control
            .joints
            .get(joint)
            .and_then(|entry| {
                control
                    .motor_type_defaults
                    .get(motor_type_key(entry.motor_type))
            })
            .map(|defaults| defaults.tau_ff_max_nm)
            .ok_or_else(|| refuse(joint, "no motor type torque cap".into()))?;
        ranges.push((policy.soft_lower(), policy.soft_upper()));
        caps.push(policy.tau_ff_max.min(type_cap));
    }
    let mut admitted = Vec::with_capacity(shedding.len());
    for joint in shedding {
        let subtree = dynamics.subtree_joints(joint)?;
        if subtree.len() >= joint_names.len() {
            return Err(refuse(
                joint,
                "its subtree covers every joint; nothing would hold".into(),
            ));
        }
        let bounds = subtree_loss_torque_bounds(dynamics, &subtree, &ranges)?;
        for bound in &bounds {
            let need = bound.max_abs_nm + bound.max_delta_nm + drive_loss.tau_margin_nm;
            // NaN fails closed: only a comparable `need <= cap` admits.
            if need.partial_cmp(&caps[bound.index]) != Some(std::cmp::Ordering::Less)
                && need != caps[bound.index]
            {
                return Err(refuse(
                    joint,
                    format!(
                        "holding joint {}: max |τ_g| {:.3} + max |Δτ_g| {:.3} + margin {:.3} = {:.3} Nm exceeds its {:.3} Nm feed-forward cap",
                        bound.joint,
                        bound.max_abs_nm,
                        bound.max_delta_nm,
                        drive_loss.tau_margin_nm,
                        need,
                        caps[bound.index]
                    ),
                ));
            }
        }
        let mut shed = vec![joint.clone()];
        shed.extend(
            subtree
                .iter()
                .map(|&index| &joint_names[index])
                .filter(|name| *name != joint)
                .cloned(),
        );
        admitted.push(DriveLossAdmission {
            plan: DriveLossPlan {
                joint: joint.clone(),
                shed,
            },
            bounds,
        });
    }
    Ok(admitted)
}
