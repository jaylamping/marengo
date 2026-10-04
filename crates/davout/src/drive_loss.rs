//! Single-drive loss while Active: subtree shed, degraded hold, hard deadline
//! (ADR 0038).
//!
//! A qualifying loss disables the lost joint and every joint distal to it with
//! one type-4 Disable per address, inside the call that judged the lapse, and
//! keeps the proximal joints Active. The controller then holds and lowers them;
//! Davout owns the episode's deadline and ends every episode with an
//! all-address stop and a latched `Communication` fault (restart required).
//! A shed address is never commanded again in this process except Disable.

use std::time::{Duration, Instant};

use marengo_config::OnDriveLoss;
use robstride::{DriveMode, MotorAddress};
use tracing::warn;

use crate::{
    motor_for_joint, ControlMode, DavoutError, DeviceFaultEvidence, FaultClass, MitBatchScratch,
    MotorBus, OperationalMode, Supervisor,
};

/// Replies to host writes made before a shed may still be read this long after
/// it: one control period of writes plus the longest measured MIT reply
/// latency (4.65 ms), doubled. A non-Reset frame from a shed address read later
/// latches `DriveState` and stops every drive.
pub const SHED_REPLY_GRACE: Duration = Duration::from_millis(20);

/// A joint whose drive loss may shed its subtree, installed by the controller
/// once the offline gravity bound admitted it (`on_drive_loss: shed_subtree`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveLossPlan {
    pub joint: String,
    /// `joint` and every configured joint distal to it.
    pub shed: Vec<String>,
}

/// A degraded episode in progress: the shed subtree is disabled, the holding
/// joints stay Active until the controller lowers them or the deadline passes.
#[derive(Debug, Clone, PartialEq)]
pub struct DegradedEpisode {
    pub lost_joint: String,
    /// The lost joint and every joint distal to it, all disabled.
    pub shed_joints: Vec<String>,
    /// Active joints that keep holding, in configured motor order.
    pub holding_joints: Vec<String>,
    /// Last admitted position of each shed joint (joint space, rad): the gravity
    /// model's frozen angles.
    pub frozen_positions: Vec<(String, f64)>,
    pub since: Instant,
    /// Davout stops every drive at this instant whatever the controller does.
    pub deadline: Instant,
}

/// How a degraded episode ended. Every end stops every drive and latches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradedEnd {
    /// The controller reported the holding joints at rest.
    LowerComplete,
    /// Davout's deadline passed first.
    Deadline,
    /// Any other all-address stop: operator disable, E-stop, a fault, a second loss.
    Stopped,
}

impl DegradedEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LowerComplete => "lower complete",
            Self::Deadline => "deadline",
            Self::Stopped => "stop",
        }
    }
}

/// The retained end of the process's degraded episode.
#[derive(Debug, Clone, PartialEq)]
pub struct DegradedOutcome {
    pub lost_joint: String,
    pub end: DegradedEnd,
    pub at: Instant,
}

/// Process-lifetime degraded-episode state owned by the Supervisor.
#[derive(Debug, Default)]
pub(crate) struct DriveLossState {
    pub(crate) plans: Vec<DriveLossPlan>,
    pub(crate) episode: Option<DegradedEpisode>,
    pub(crate) outcome: Option<DegradedOutcome>,
    /// Reason the next all-address stop ends the episode with.
    pub(crate) pending_end: Option<DegradedEnd>,
    /// Shed addresses: Disable only, never pose, for the rest of the process.
    pub(crate) shed: Vec<MotorAddress>,
    pub(crate) lost_address: Option<MotorAddress>,
    pub(crate) shed_at: Option<Instant>,
}

impl DriveLossState {
    pub(crate) fn is_shed(&self, address: &MotorAddress) -> bool {
        self.shed.contains(address)
    }
}

/// Healthy-state facts a lapse must meet to be answered by a shed.
struct ShedDecision {
    lost: String,
    shed: Vec<(String, MotorAddress, f64)>,
    holding: Vec<String>,
    since: Instant,
    deadline: Instant,
}

impl<B: MotorBus> Supervisor<B> {
    /// Install the joints whose drive loss may shed their subtree. Each needs
    /// `on_drive_loss: shed_subtree`, a configured motor, and a shed list that
    /// starts with the joint and names only configured joints.
    pub fn install_drive_loss_plans(
        &mut self,
        plans: Vec<DriveLossPlan>,
    ) -> Result<(), DavoutError> {
        for plan in &plans {
            let configured = self
                .control
                .control
                .joints
                .get(&plan.joint)
                .is_some_and(|entry| entry.on_drive_loss == OnDriveLoss::ShedSubtree);
            if !configured {
                return Err(DavoutError::DriveLossPlan {
                    joint: plan.joint.clone(),
                    message: "on_drive_loss is not shed_subtree".into(),
                });
            }
            if plan.shed.first() != Some(&plan.joint) {
                return Err(DavoutError::DriveLossPlan {
                    joint: plan.joint.clone(),
                    message: "shed list must start with the lost joint".into(),
                });
            }
            if let Some(unknown) = plan
                .shed
                .iter()
                .find(|joint| motor_for_joint(&self.motors, joint).is_none())
            {
                return Err(DavoutError::DriveLossPlan {
                    joint: plan.joint.clone(),
                    message: format!("shed joint {unknown} has no configured motor"),
                });
            }
        }
        self.drive_loss.plans = plans;
        Ok(())
    }

    /// Installed drive-loss plans (empty: every loss stops every drive).
    pub fn drive_loss_plans(&self) -> &[DriveLossPlan] {
        &self.drive_loss.plans
    }

    /// The degraded episode in progress, if any.
    pub fn degraded_episode(&self) -> Option<&DegradedEpisode> {
        self.drive_loss.episode.as_ref()
    }

    /// How this process's degraded episode ended, once it has.
    pub fn degraded_outcome(&self) -> Option<&DegradedOutcome> {
        self.drive_loss.outcome.as_ref()
    }

    /// The controller brought the holding joints to rest: stop every drive and
    /// latch the episode's fault. Without an episode this is a plain stop.
    pub fn complete_degraded_lower(&mut self) -> Result<(), DavoutError> {
        self.stop_ending_episode(DegradedEnd::LowerComplete)
    }

    /// Stop every drive once the episode's deadline has passed, whatever the
    /// controller does. Returns the latched fault after such a stop.
    pub fn enforce_degraded_deadline(&mut self) -> Result<(), DavoutError> {
        let expired = self
            .drive_loss
            .episode
            .as_ref()
            .is_some_and(|episode| Instant::now() >= episode.deadline);
        if expired {
            let _ = self.stop_ending_episode(DegradedEnd::Deadline);
            self.require_fault_clear()?;
        }
        Ok(())
    }

    /// Refuse an operation that needs every joint while an episode runs.
    pub(crate) fn refuse_during_episode(&self, message: &'static str) -> Result<(), DavoutError> {
        match &self.drive_loss.episode {
            Some(episode) => Err(DavoutError::DegradedEpisode {
                joint: episode.lost_joint.clone(),
                message,
            }),
            None => Ok(()),
        }
    }

    fn stop_ending_episode(&mut self, end: DegradedEnd) -> Result<(), DavoutError> {
        if self.drive_loss.episode.is_some() {
            self.drive_loss.pending_end = Some(end);
        }
        self.disable_all()
    }

    /// Called by every all-address stop after the drives were addressed: an
    /// episode in progress ends here and latches `Communication` on the lost
    /// joint, so the process needs a restart.
    pub(crate) fn end_degraded_episode_on_stop(&mut self) {
        let end = self
            .drive_loss
            .pending_end
            .take()
            .unwrap_or(DegradedEnd::Stopped);
        let Some(episode) = self.drive_loss.episode.take() else {
            return;
        };
        let message = format!(
            "drive lost; degraded episode ended ({}); restart required",
            end.as_str()
        );
        self.fault_authority.record(
            FaultClass::Communication,
            Some(episode.lost_joint.clone()),
            self.drive_loss.lost_address.clone(),
            &message,
            DeviceFaultEvidence::default(),
        );
        warn!(
            lost_joint = %episode.lost_joint,
            end = end.as_str(),
            elapsed = ?episode.since.elapsed(),
            "degraded episode ended; every drive stopped"
        );
        self.drive_loss.outcome = Some(DegradedOutcome {
            lost_joint: episode.lost_joint,
            end,
            at: Instant::now(),
        });
    }

    /// Answer a lapsed Active grant by shedding the lost joint's subtree.
    /// `Ok(true)`: shed, the holding joints stay Active. `Ok(false)`: the lapse
    /// does not qualify; the caller stops every drive as before. `Err`: a
    /// Disable write failed; every drive was stopped.
    pub(crate) fn shed_for_drive_loss(&mut self) -> Result<bool, DavoutError> {
        let Some(decision) = self.shed_decision() else {
            return Ok(false);
        };
        let now = decision.since;
        for (_, address, _) in &decision.shed {
            self.drive_loss.shed.push(address.clone());
        }
        self.drive_loss.shed_at = Some(now);
        for (_, address, _) in &decision.shed {
            if let Err(error) = self.bus.disable_drive_at(address) {
                let error = DavoutError::Bus(error);
                self.record_runtime_error(&error);
                let _ = self.disable_all();
                return Err(error);
            }
        }
        for (joint, address, _) in &decision.shed {
            self.reference_authority.revoke_joint(joint);
            self.active_joints.remove(joint);
            self.unanswered_solicits.remove(address);
            self.last_tau_ff.remove(joint);
            self.wrong_sign_state.remove(joint);
            self.feedback_velocity_trips.remove(joint);
            self.last_feedback_samples.remove(joint);
        }
        let lost_address = decision
            .shed
            .iter()
            .find(|(joint, _, _)| *joint == decision.lost)
            .map(|(_, address, _)| address.clone());
        let shed_joints: Vec<String> = decision.shed.iter().map(|(j, _, _)| j.clone()).collect();
        warn!(
            lost_joint = %decision.lost,
            shed = ?shed_joints,
            holding = ?decision.holding,
            deadline_in = ?decision.deadline.saturating_duration_since(now),
            "drive lost: subtree shed with one Disable per address; holding joints stay Active"
        );
        self.drive_loss.lost_address = lost_address;
        self.drive_loss.episode = Some(DegradedEpisode {
            lost_joint: decision.lost,
            frozen_positions: decision
                .shed
                .into_iter()
                .map(|(joint, _, position)| (joint, position))
                .collect(),
            shed_joints,
            holding_joints: decision.holding,
            since: now,
            deadline: decision.deadline,
        });
        Ok(true)
    }

    /// The pose watchdog judged an Active pose stale after the grant check of
    /// this admission passed: both count the same unanswered solicit, judged a
    /// moment apart. Judge the grant again; if the lapse qualifies, shed and
    /// drop the shed joints from the staged batch, then re-check the rest.
    /// `Ok(false)`: no shed, the caller stops as before.
    pub(crate) fn shed_after_stale_pose(
        &mut self,
        scratch: &mut MitBatchScratch,
    ) -> Result<bool, DavoutError> {
        if self.mode != OperationalMode::Active {
            return Ok(false);
        }
        let _ = self.reference_binding_valid();
        if self.active_references_held() || !self.shed_for_drive_loss()? {
            return Ok(false);
        }
        let mut keep = 0;
        for index in 0..scratch.wires.len().min(scratch.staged.len()) {
            if !self.drive_loss.is_shed(&scratch.wires[index].address) {
                scratch.wires.swap(keep, index);
                scratch.staged.swap(keep, index);
                keep += 1;
            }
        }
        scratch.wires.truncate(keep);
        scratch.staged.truncate(keep);
        if let Err(error) = self.check_comm_watchdog(false) {
            self.stop_after_runtime_error(&error);
            return Err(error);
        }
        Ok(true)
    }

    /// Every ADR 0038 condition at the moment of the lapse, or `None`.
    fn shed_decision(&self) -> Option<ShedDecision> {
        if self.mode != OperationalMode::Active
            || self.has_latched_fault()
            || self.hardware_estop
            || self.drive_loss.episode.is_some()
            || !self.drive_loss.shed.is_empty()
            || self.reference_busy()
            || !self.enable_writes_pending.is_empty()
            || !self.enable_echo_pending.is_empty()
            || !matches!(
                self.control_mode,
                ControlMode::GravityComp | ControlMode::Position
            )
        {
            return None;
        }
        let mut lost = self
            .active_joints
            .iter()
            .filter(|joint| !self.reference_authority.contains(joint));
        let lost = lost.next().filter(|_| lost.next().is_none())?.clone();
        if !self.reference_authority.revoked_silent(&lost) {
            return None;
        }
        let drive_loss = self.control.control.drive_loss.as_ref()?;
        let admitted = self
            .control
            .control
            .joints
            .get(&lost)
            .is_some_and(|entry| entry.on_drive_loss == OnDriveLoss::ShedSubtree);
        let plan = self
            .drive_loss
            .plans
            .iter()
            .find(|plan| plan.joint == lost)
            .filter(|_| admitted)?;
        let now = Instant::now();
        let mut shed = Vec::with_capacity(plan.shed.len());
        for joint in &plan.shed {
            let address = MotorAddress::from(motor_for_joint(&self.motors, joint)?);
            let position = f64::from(self.motor_states.get(&address)?.position_rad);
            shed.push((joint.clone(), address, position));
        }
        let mut holding = Vec::new();
        let mut distance = 0.0_f64;
        for motor in &self.motors.motors {
            if !self.active_joints.contains(&motor.joint) || plan.shed.contains(&motor.joint) {
                continue;
            }
            let address = MotorAddress::from(motor);
            let state = self.motor_states.get(&address)?;
            if !self.reference_authority.contains(&motor.joint)
                || self.invalid_feedback.contains(&address)
                || !self.pose_is_current(&address, state, now)
            {
                return None;
            }
            let rest = self
                .control
                .control
                .joints
                .get(&motor.joint)
                .map_or(0.0, |entry| entry.position_hold_trim_rad);
            distance = distance.max((f64::from(state.position_rad) - rest).abs());
            holding.push(motor.joint.clone());
        }
        if holding.is_empty() {
            return None;
        }
        let hold = Duration::try_from_secs_f64(drive_loss.hold_window_s).ok()?;
        let lower = Duration::try_from_secs_f64(
            distance / drive_loss.lower_velocity_rad_s + drive_loss.lower_settle_s,
        )
        .ok()?
        .min(Duration::try_from_secs_f64(drive_loss.lower_max_s).ok()?);
        let deadline = now.checked_add(hold.checked_add(lower)?)?;
        Some(ShedDecision {
            lost,
            shed,
            holding,
            since: now,
            deadline,
        })
    }

    /// A frame from a shed address: never pose. Reset is expected (a stopped
    /// or rebooted drive); any other mode read after [`SHED_REPLY_GRACE`] means
    /// the drive runs without this owner commanding it and latches `DriveState`.
    /// Returns whether a new fault transition was recorded.
    pub(crate) fn inspect_shed_frame(
        &mut self,
        joint: &str,
        address: &MotorAddress,
        drive_mode: DriveMode,
        received_at: Instant,
        device: DeviceFaultEvidence,
    ) -> Option<(DavoutError, bool)> {
        let grace = self
            .drive_loss
            .shed_at
            .is_some_and(|shed_at| received_at <= shed_at + SHED_REPLY_GRACE);
        if drive_mode == DriveMode::Reset || grace {
            return None;
        }
        let error = DavoutError::InvalidFeedback {
            joint: joint.to_owned(),
            message: format!(
                "shed drive reported {drive_mode:?} after its Disable; it is never commanded again"
            ),
        };
        let transition = self.fault_authority.record(
            FaultClass::DriveState,
            Some(joint.to_owned()),
            Some(address.clone()),
            &error.to_string(),
            device,
        );
        Some((error, transition))
    }
}
