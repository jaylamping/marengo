//! Immutable typed worker input and inspection-only decoded history.

use std::sync::Arc;

use marengo_config::{ControlConfigFile, HomingConfigFile, MotorsConfigFile, RobotConfigFile};
use serde::{Deserialize, Serialize};

use crate::reference_commit::ReferenceAudit;
use crate::reference_model::InstalledModelStamp;
use crate::reference_urdf_codec::Model;
use crate::{ReferenceReceiveSummary, ReferenceReportingAttempt, StopAction, StopReport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TypedPolicy {
    pub(super) motors: MotorsConfigFile,
    pub(super) control: ControlConfigFile,
    pub(super) homing: HomingConfigFile,
}
impl TypedPolicy {
    pub(super) fn matches(
        &self,
        motors: &MotorsConfigFile,
        control: &ControlConfigFile,
        homing: &HomingConfigFile,
    ) -> bool {
        #[derive(Serialize)]
        struct View<'a> {
            motors: &'a MotorsConfigFile,
            control: &'a ControlConfigFile,
            homing: &'a HomingConfigFile,
        }
        match (
            super::reference_codec::encode(self),
            super::reference_codec::encode(&View {
                motors,
                control,
                homing,
            }),
        ) {
            (Ok(pinned), Ok(current)) => pinned == current,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Address {
    pub(super) interface: String,
    pub(super) device_id: u8,
}
impl From<&robstride::MotorAddress> for Address {
    fn from(value: &robstride::MotorAddress) -> Self {
        Self {
            interface: value.interface.clone(),
            device_id: value.device_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Action {
    ZeroSpeed,
    NeutralMit,
    Disable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Attempt {
    address: Address,
    action: Action,
    error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Cleanup {
    generation: u64,
    attempts: Vec<Attempt>,
}
impl From<&StopReport> for Cleanup {
    fn from(report: &StopReport) -> Self {
        Self {
            generation: report.generation,
            attempts: report
                .attempts
                .iter()
                .map(|attempt| Attempt {
                    address: (&attempt.address).into(),
                    action: match attempt.action {
                        StopAction::ZeroSpeed => Action::ZeroSpeed,
                        StopAction::NeutralMit => Action::NeutralMit,
                        StopAction::Disable => Action::Disable,
                    },
                    error: attempt.error.clone(),
                })
                .collect(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Reporting {
    joint: String,
    address: Address,
    error: Option<String>,
}
impl From<&ReferenceReportingAttempt> for Reporting {
    fn from(attempt: &ReferenceReportingAttempt) -> Self {
        Self {
            joint: attempt.joint.clone(),
            address: (&attempt.address).into(),
            error: attempt.error.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Completion {
    Idle,
    Quiet,
    WorkLimit,
    Deadline,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Receive {
    completion: Completion,
    raw_frames: u32,
    read_attempts: u32,
}
impl TryFrom<ReferenceReceiveSummary> for Receive {
    type Error = &'static str;
    fn try_from(value: ReferenceReceiveSummary) -> Result<Self, Self::Error> {
        Ok(Self {
            completion: match value.completion {
                robstride::ReceiveCompletion::Idle => Completion::Idle,
                robstride::ReceiveCompletion::Quiet => Completion::Quiet,
                robstride::ReceiveCompletion::WorkLimit => Completion::WorkLimit,
                robstride::ReceiveCompletion::Deadline => Completion::Deadline,
                robstride::ReceiveCompletion::Failed => Completion::Failed,
            },
            raw_frames: u32::try_from(value.raw_frames).map_err(|_| "raw frame count overflow")?,
            read_attempts: u32::try_from(value.read_attempts)
                .map_err(|_| "read attempt count overflow")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Capture {
    pub(super) acquisition_sequence: u64,
    pub(super) joint: String,
    pub(super) address: Address,
    pub(super) confirmed: bool,
    pub(super) sign_verified: bool,
    pub(super) position_rad: f32,
    pub(super) raw_pop_order: u32,
    pub(super) can_id: u32,
    pub(super) device_epoch: u64,
    pub(super) original_deadline_secs: u64,
    pub(super) original_deadline_nanos: u32,
    pub(super) stamp_reference_generation: u64,
    pub(super) stamp_stop_generation: u64,
    pub(super) accepted_reference_generation: u64,
    pub(super) cleanup: Cleanup,
    pub(super) reporting: Vec<Reporting>,
    pub(super) receive: Receive,
    pub(super) audit: ReferenceAudit,
}

pub(super) struct Input {
    pub(super) model: InstalledModelStamp,
    pub(super) policy: Arc<TypedPolicy>,
    pub(super) capture: Capture,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Body {
    version: u32,
    evidence_class: String,
    pub(super) diagnostic_session: u64,
    pub(super) job_sequence: u64,
    pub(super) installed_model_generation: u64,
    pub(super) capture: Capture,
    pub(super) policy: TypedPolicy,
    pub(super) robot: RobotConfigFile,
    pub(super) urdf: Model,
}

impl Input {
    pub(super) fn encode(
        &self,
        session: u64,
        job: u64,
    ) -> Result<Vec<u8>, super::reference_codec::CodecError> {
        let (robot, urdf) = self.model.descriptor();
        let body = Body {
            version: 1,
            evidence_class: "closed_virtual".into(),
            diagnostic_session: session,
            job_sequence: job,
            installed_model_generation: self.model.generation(),
            capture: self.capture.clone(),
            policy: (*self.policy).clone(),
            robot: robot.clone(),
            urdf: urdf.into(),
        };
        super::reference_codec::encode(&body)
    }
}

impl Body {
    pub(super) fn validate(&self) -> bool {
        let capture = &self.capture;
        let Some(target) = self
            .policy
            .motors
            .motors
            .iter()
            .find(|motor| motor.joint == capture.joint)
        else {
            return false;
        };
        let Some(homing) = self.policy.homing.homing.effective_joint(&capture.joint) else {
            return false;
        };
        let expected = self.policy.motors.motors.iter().flat_map(|motor| {
            let address = Address {
                interface: motor.can_interface.clone(),
                device_id: motor.device_id,
            };
            [Action::ZeroSpeed, Action::NeutralMit, Action::Disable]
                .into_iter()
                .map(move |action| (address.clone(), action))
        });
        let cleanup_matches =
            capture.cleanup.attempts.len() == self.policy.motors.motors.len() * 3
                && capture.cleanup.attempts.iter().zip(expected).all(
                    |(attempt, (address, action))| {
                        attempt.address == address
                            && attempt.action == action
                            && attempt.error.is_none()
                    },
                );
        self.version == 1
            && self.evidence_class == "closed_virtual"
            && self.diagnostic_session > 0
            && self.job_sequence > 0
            && self.installed_model_generation > 0
            && self.capture.acquisition_sequence > 0
            && self.capture.confirmed
            && self.capture.position_rad.is_finite()
            && capture.audit.validate()
            && (!homing.sign_test_required || capture.sign_verified)
            && capture.address.interface == target.can_interface
            && capture.address.device_id == target.device_id
            && f64::from(capture.position_rad).abs()
                <= self.policy.homing.homing.zero_verify_tolerance_rad
            && capture.accepted_reference_generation > capture.stamp_reference_generation
            && capture.accepted_reference_generation < u64::MAX
            && capture.device_epoch > 0
            && capture.cleanup.generation > capture.stamp_stop_generation
            && cleanup_matches
            && self.capture.original_deadline_nanos < 1_000_000_000
            && self.capture.cleanup.generation < u64::MAX
            && self
                .capture
                .cleanup
                .attempts
                .iter()
                .all(|attempt| attempt.error.is_none())
            && self.capture.reporting.len() <= self.policy.motors.motors.len()
            && self.capture.reporting.iter().all(|attempt| {
                attempt.error.is_none()
                    && self.policy.motors.motors.iter().any(|motor| {
                        motor.joint == attempt.joint
                            && motor.can_interface == attempt.address.interface
                            && motor.device_id == attempt.address.device_id
                    })
            })
            && matches!(
                self.capture.receive.completion,
                Completion::Idle | Completion::Quiet
            )
            && self.capture.receive.raw_frames <= 64
            && self.capture.receive.read_attempts <= 256
            && self.capture.raw_pop_order < self.capture.receive.raw_frames
    }
}

/// Owned inspection data only. It cannot be submitted as a completion or permit.
#[derive(Debug)]
pub struct ReferenceHistoryRecord {
    session: u64,
    job: u64,
    capture: Capture,
    policy: TypedPolicy,
    robot: RobotConfigFile,
    urdf: urdf_rs::Robot,
    checksum: [u8; 32],
}

impl ReferenceHistoryRecord {
    pub(super) fn from_body(bytes: &[u8], checksum: [u8; 32]) -> Result<Self, String> {
        let body: Body =
            super::reference_codec::decode(bytes).map_err(|error| error.to_string())?;
        if !body.validate() {
            return Err("invalid virtual history body".into());
        }
        Ok(Self {
            session: body.diagnostic_session,
            job: body.job_sequence,
            capture: body.capture,
            policy: body.policy,
            robot: body.robot,
            urdf: body.urdf.into(),
            checksum,
        })
    }
    pub fn diagnostic_session(&self) -> u64 {
        self.session
    }
    pub fn job_sequence(&self) -> u64 {
        self.job
    }
    pub fn acquisition_sequence(&self) -> u64 {
        self.capture.acquisition_sequence
    }
    pub fn joint(&self) -> &str {
        &self.capture.joint
    }
    pub fn position_rad(&self) -> f32 {
        self.capture.position_rad
    }
    pub fn audit(&self) -> &ReferenceAudit {
        &self.capture.audit
    }
    pub fn motors(&self) -> &MotorsConfigFile {
        &self.policy.motors
    }
    pub fn control(&self) -> &ControlConfigFile {
        &self.policy.control
    }
    pub fn homing(&self) -> &HomingConfigFile {
        &self.policy.homing
    }
    pub fn robot(&self) -> &RobotConfigFile {
        &self.robot
    }
    pub fn urdf(&self) -> &urdf_rs::Robot {
        &self.urdf
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
}
