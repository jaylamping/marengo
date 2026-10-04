//! Single-drive-loss episode wiring (ADR 0038): startup admission report,
//! motion refusals, stdout contract lines and audit events.

use berthier::{ControlLoop, DegradedEvent, DriveLossAdmission};
use chappe::Bus;
use davout::{DegradedEnd, MotorBus};
use tracing::{info, warn};

use crate::motion_owner::{publish_audit, report_refusal};

/// Audit operator for episode transitions the runtime itself makes.
const RUNTIME_OPERATOR: &str = "marengo-pi";

/// Why `command` is refused while an episode runs, if one does.
pub(crate) fn degraded_refusal<B: MotorBus>(
    loop_ctrl: &ControlLoop<B>,
    command: &str,
) -> Option<(String, String)> {
    loop_ctrl.degraded_lost_joint().map(|joint| {
        let reason = format!(
            "{command} refused: degraded hold after losing {joint}; only disable or lower is accepted"
        );
        (joint, reason)
    })
}

/// Refuse a motion command during an episode: printed and published as
/// `motion_refused` on `robot/audit/action`. Returns true when refused.
pub(crate) fn refuse_during_episode<B: MotorBus>(
    loop_ctrl: &ControlLoop<B>,
    chappe: &Bus,
    operator: &str,
    command: &str,
    print: bool,
) -> bool {
    let Some((joint, reason)) = degraded_refusal(loop_ctrl, command) else {
        return false;
    };
    if print {
        eprintln!("{reason}");
    }
    report_refusal(chappe, operator, &joint, command, &reason);
    true
}

fn joined(joints: &[String]) -> String {
    joints.join(",")
}

/// The stdout line scripts wait for, one per episode transition.
pub(crate) fn degraded_event_line(event: &DegradedEvent) -> String {
    match event {
        DegradedEvent::DriveLost {
            joint,
            shed,
            holding,
            auto_lower_in,
        } => format!(
            "drive lost {joint}: shed {}; holding {}; auto-lower in {} s",
            joined(shed),
            joined(holding),
            auto_lower_in.as_secs_f64()
        ),
        DegradedEvent::LowerStarted { cause } => {
            format!("degraded lower started ({})", cause.as_str())
        }
        DegradedEvent::LowerComplete => "degraded lower complete; disabled".to_string(),
        DegradedEvent::Ended { end, .. } => {
            format!("degraded episode ended ({}); disabled", end.as_str())
        }
    }
}

/// Print, log and audit every episode transition. Returns true when a new
/// episode started (the caller cancels deferred motion).
pub(crate) fn report_degraded_events(events: Vec<DegradedEvent>, chappe: &Bus) -> bool {
    let mut started = false;
    for event in events {
        let line = degraded_event_line(&event);
        println!("{line}");
        let (action, joint) = match &event {
            DegradedEvent::DriveLost { joint, .. } => {
                started = true;
                ("drive_loss_shed", joint.as_str())
            }
            DegradedEvent::LowerStarted { .. } => ("degraded_lower", ""),
            DegradedEvent::LowerComplete => ("degraded_lower_complete", ""),
            DegradedEvent::Ended { lost_joint, end } => {
                if *end == DegradedEnd::Deadline {
                    warn!(lost_joint = %lost_joint, "degraded episode deadline expired");
                }
                ("degraded_episode_ended", lost_joint.as_str())
            }
        };
        warn!(event = %line, "degraded episode");
        publish_audit(chappe, action, true, RUNTIME_OPERATOR, joint, &line);
    }
    started
}

/// Log what startup admission installed (or that every loss stops all).
pub(crate) fn log_admission(admitted: &[DriveLossAdmission]) {
    if admitted.is_empty() {
        info!("drive-loss admission: no shed_subtree joint; every drive loss stops every drive");
    }
    for admission in admitted {
        for bound in &admission.bounds {
            info!(
                joint = %admission.plan.joint,
                shed = ?admission.plan.shed,
                holding = %bound.joint,
                max_abs_tau_g_nm = bound.max_abs_nm,
                max_delta_tau_g_nm = bound.max_delta_nm,
                "drive-loss admission: shed_subtree admitted"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use berthier::{DegradedEvent, DegradedLowerCause};
    use davout::DegradedEnd;

    use super::degraded_event_line;

    #[test]
    fn stdout_lines_name_the_episode_transitions() {
        let lost = DegradedEvent::DriveLost {
            joint: "right_lower_arm_yaw".into(),
            shed: vec!["right_lower_arm_yaw".into()],
            holding: vec!["right_shoulder_pitch".into(), "right_elbow_pitch".into()],
            auto_lower_in: Duration::from_secs(5),
        };
        assert_eq!(
            degraded_event_line(&lost),
            "drive lost right_lower_arm_yaw: shed right_lower_arm_yaw; \
             holding right_shoulder_pitch,right_elbow_pitch; auto-lower in 5 s"
        );
        assert_eq!(
            degraded_event_line(&DegradedEvent::LowerStarted {
                cause: DegradedLowerCause::Timeout
            }),
            "degraded lower started (timeout)"
        );
        assert_eq!(
            degraded_event_line(&DegradedEvent::LowerComplete),
            "degraded lower complete; disabled"
        );
        assert_eq!(
            degraded_event_line(&DegradedEvent::Ended {
                lost_joint: "right_lower_arm_yaw".into(),
                end: DegradedEnd::Deadline
            }),
            "degraded episode ended (deadline); disabled"
        );
    }
}
