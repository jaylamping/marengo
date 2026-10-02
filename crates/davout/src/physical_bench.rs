//! Closed real five-drive owners for finite bench checks at mechanical home.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use marengo_config::{load_motors_config, MotorType};
use robstride::protocol::{ProtocolObservation, ProtocolReply};
use robstride::{MotorAddress, MotorBus, ParameterId, RuntimeBus};
use serde::Serialize;

use super::feedback_consumer::ReceiveContext;
use super::protocol_inspection::{ProtocolQueryContext, Query};
use super::reference::PhysicalBenchBinding;
use super::{
    BenchHomeQualification, DavoutError, MitJointCommand, ProtocolReadReceipt, StopReport,
    Supervisor, BENCH_TIMEOUT_COUNTS,
};

const OWNER_LIFETIME: Duration = Duration::from_secs(5);
pub(super) const MAX_HOME_DRIFT: f64 = 0.05;
const ENABLE_DURATION: Duration = Duration::from_millis(500);
const YAW_STEP_DURATION: Duration = Duration::from_secs(1);
pub(super) const LOWER_YAW: &str = "right_lower_arm_yaw";

/// Validated immutable gains for the finite 20-mrad home-band response.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LowerYawBenchGains {
    kp: f64,
    kd: f64,
}
impl LowerYawBenchGains {
    pub fn new(kp: f64, kd: f64) -> Result<Self, DavoutError> {
        if !(10.0..=30.0).contains(&kp) || !(0.4..=2.0).contains(&kd) {
            return Err(bench_error(
                "lower-yaw gains require finite kp10..30 and kd0.4..2",
            ));
        }
        Ok(Self { kp, kd })
    }
    pub fn kp(self) -> f64 {
        self.kp
    }
    pub fn kd(self) -> f64 {
        self.kd
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum BenchOutput {
    Neutral,
    LowerYawStep(LowerYawBenchGains),
}

impl BenchOutput {
    pub(super) fn is_lower_yaw(self) -> bool {
        matches!(self, Self::LowerYawStep(_))
    }
    fn gains(self) -> Option<LowerYawBenchGains> {
        match self {
            Self::Neutral => None,
            Self::LowerYawStep(gains) => Some(gains),
        }
    }
    fn duration(self) -> Duration {
        match self {
            Self::Neutral => ENABLE_DURATION,
            Self::LowerYawStep(_) => YAW_STEP_DURATION,
        }
    }
}
const PROFILE: [(&str, MotorType, i8, [u8; 8]); 5] = [
    (
        "right_shoulder_pitch",
        MotorType::Rs03,
        -1,
        [0x45, 0x7B, 0x30, 2, 0x0C, 0x32, 0x38, 0x17],
    ),
    (
        "right_shoulder_roll",
        MotorType::Rs03,
        1,
        [0x78, 0x56, 0x30, 2, 0x0C, 0x34, 0x37, 1],
    ),
    (
        "right_upper_arm_yaw",
        MotorType::Rs02,
        -1,
        [0x9D, 0x6A, 0x3B, 0x85, 0x9C, 2, 0x30, 0x19],
    ),
    (
        "right_elbow_pitch",
        MotorType::Rs02,
        -1,
        [0x8A, 6, 0x3B, 0x8F, 0x24, 0x50, 0xB0, 0x0D],
    ),
    (
        "right_lower_arm_yaw",
        MotorType::Rs00,
        -1,
        [0x62, 0x2C, 0x30, 2, 0x0C, 0x34, 0x37, 1],
    ),
];

#[derive(Debug, Clone, Serialize)]
pub struct BenchNeutralFeedback {
    pub joint: String,
    pub position_rad: f64,
    pub velocity_rad_s: f64,
    pub torque_nm: f64,
}

/// No bus injection, receipt import, mutable Supervisor or nonneutral output API.
pub struct PhysicalNeutralBench {
    owner: FiniteBenchOwner<RuntimeBus>,
}

/// One small lower-arm-yaw position step; all other MIT fields stay neutral.
pub struct PhysicalLowerYawBench {
    owner: FiniteBenchOwner<RuntimeBus>,
}

struct FiniteBenchOwner<B: MotorBus> {
    supervisor: Supervisor<B>,
    joints: Vec<String>,
    audit_path: PathBuf,
    enable_attempted: bool,
    closed: bool,
    stop_report: Option<StopReport>,
    accepted_protocol: Vec<ProtocolObservation>,
    active_deadline: Option<Instant>,
    output: BenchOutput,
}

impl PhysicalNeutralBench {
    /// Blocking acquisition/audit, outside any control tick. Opens real CAN itself.
    pub fn acquire(
        root: impl AsRef<Path>,
        operator: &str,
        confirmed_home: bool,
        sign_attested: bool,
        confirmed_neutral_enable: bool,
    ) -> Result<Self, DavoutError> {
        acquire_owner(
            root.as_ref(),
            operator,
            confirmed_home,
            sign_attested,
            confirmed_neutral_enable,
            BenchOutput::Neutral,
        )
        .map(|owner| Self { owner })
    }

    pub fn joints(&self) -> &[String] {
        self.owner.joints()
    }
    pub fn audit_path(&self) -> &Path {
        self.owner.audit_path()
    }
    pub fn begin(&mut self, batch: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        self.owner.begin(batch)
    }
    pub fn tick(
        &mut self,
        batch: Vec<MitJointCommand>,
    ) -> Result<Vec<BenchNeutralFeedback>, DavoutError> {
        self.owner.tick(batch)
    }
    pub fn finish(&mut self) -> Result<StopReport, DavoutError> {
        self.owner.finish()
    }
    pub fn active_deadline(&self) -> Option<Instant> {
        self.owner.active_deadline
    }
}

impl PhysicalLowerYawBench {
    pub fn acquire(
        root: impl AsRef<Path>,
        operator: &str,
        confirmed_home: bool,
        sign_attested: bool,
        confirmed_motion: bool,
        gains: LowerYawBenchGains,
    ) -> Result<Self, DavoutError> {
        acquire_owner(
            root.as_ref(),
            operator,
            confirmed_home,
            sign_attested,
            confirmed_motion,
            BenchOutput::LowerYawStep(gains),
        )
        .map(|owner| Self { owner })
    }
    pub fn joints(&self) -> &[String] {
        self.owner.joints()
    }
    pub fn audit_path(&self) -> &Path {
        self.owner.audit_path()
    }
    pub fn begin(&mut self, neutral: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        self.owner.begin(neutral)
    }
    pub fn tick(
        &mut self,
        batch: Vec<MitJointCommand>,
    ) -> Result<Vec<BenchNeutralFeedback>, DavoutError> {
        self.owner.tick(batch)
    }
    pub fn finish(&mut self) -> Result<StopReport, DavoutError> {
        self.owner.finish()
    }
    pub fn active_deadline(&self) -> Option<Instant> {
        self.owner.active_deadline
    }
}

fn acquire_owner(
    root: &Path,
    operator: &str,
    confirmed_home: bool,
    sign_attested: bool,
    confirmed_enable: bool,
    output: BenchOutput,
) -> Result<FiniteBenchOwner<RuntimeBus>, DavoutError> {
    if !confirmed_home || !sign_attested || !confirmed_enable {
        return Err(bench_error(
            "home, sign and bounded-enable approval are required",
        ));
    }
    let motors = load_motors_config(root)?;
    let bus = RuntimeBus::socketcan_from_motors(&motors)?;
    let supervisor = Supervisor::from_repo_for_protocol_inspection(root, bus)?;
    let mut owner = FiniteBenchOwner {
        joints: supervisor.robot.robot.joints.clone(),
        supervisor,
        audit_path: PathBuf::new(),
        enable_attempted: false,
        closed: false,
        stop_report: None,
        accepted_protocol: Vec::new(),
        active_deadline: None,
        output,
    };
    let acquisition = owner.supervisor.acquire_physical_bench_reference(
        root,
        operator,
        confirmed_home,
        sign_attested,
        output,
    );
    let (audit_path, accepted) = acquisition.map_err(|error| owner.finish_after_error(error))?;
    owner.audit_path = audit_path;
    owner.accepted_protocol = accepted;
    Ok(owner)
}

impl<B: MotorBus> FiniteBenchOwner<B> {
    fn joints(&self) -> &[String] {
        &self.joints
    }

    pub fn audit_path(&self) -> &Path {
        &self.audit_path
    }

    /// Exactly one ordinary Davout enable session; no recovery/re-arm.
    pub fn begin(&mut self, neutral: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        let result = (|| {
            if self.closed || self.enable_attempted {
                return Err(bench_error("neutral bench enable already consumed"));
            }
            validate_neutral_batch(&self.joints, &neutral)?;
            self.enable_attempted = true;
            // The last disabled report must still be Reset before normal enable.
            self.supervisor
                .inspection_receive(&self.accepted_protocol)?;
            let deadline = Instant::now() + self.output.duration();
            self.active_deadline = Some(deadline);
            self.supervisor.enable_targets(&self.joints)?;
            if self.output.is_lower_yaw() {
                self.supervisor
                    .set_control_mode(super::ControlMode::Position);
            }
            self.supervisor.send_mit_batch(neutral.clone())?;
            self.supervisor.verify_bench_home(
                0x60,
                Some((&neutral, deadline)),
                &mut self.accepted_protocol,
            )?;
            self.supervisor.check_fault_authority()?;
            Ok(())
        })();
        result.map_err(|error| self.finish_after_error(error))
    }

    /// Nonblocking receive/send path used by Berthier's finite 200 Hz sequence.
    pub fn tick(
        &mut self,
        neutral: Vec<MitJointCommand>,
    ) -> Result<Vec<BenchNeutralFeedback>, DavoutError> {
        let result = (|| {
            if self.closed
                || !self.enable_attempted
                || self.active_deadline.is_none_or(|end| Instant::now() >= end)
            {
                return Err(bench_error("neutral bench is not running"));
            }
            validate_bench_batch(self.output, &self.joints, &neutral)?;
            self.supervisor.ensure_reference_for(&self.joints)?;
            self.supervisor
                .protocol_receive(&self.accepted_protocol, &ReceiveContext::Operational)?;
            self.supervisor.send_mit_batch(neutral)?;
            self.supervisor
                .protocol_receive(&self.accepted_protocol, &ReceiveContext::Operational)?;
            let mut feedback = Vec::with_capacity(self.joints.len());
            for joint in &self.joints {
                if let Some(sample) = self.supervisor.joint_feedback(joint) {
                    feedback.push(BenchNeutralFeedback {
                        joint: joint.clone(),
                        position_rad: sample.position_rad,
                        velocity_rad_s: sample.velocity_rad_s,
                        torque_nm: sample.torque_nm,
                    });
                }
            }
            Ok(feedback)
        })();
        result.map_err(|error| self.finish_after_error(error))
    }

    /// Honest write-delivery report, not physical stop acknowledgement.
    pub fn finish(&mut self) -> Result<StopReport, DavoutError> {
        self.closed = true;
        let report = self
            .stop_report
            .get_or_insert_with(|| self.supervisor.perform_stop(false));
        if report.failed_writes() != 0 {
            return Err(DavoutError::StopDelivery {
                failed_writes: report.failed_writes(),
            });
        }
        Ok(report.clone())
    }

    fn finish_after_error(&mut self, error: DavoutError) -> DavoutError {
        self.supervisor.record_runtime_error(&error);
        match self.finish() {
            Ok(_) => error,
            Err(stop) => bench_error(&format!("{error}; final stop failed: {stop}")),
        }
    }
}

impl<B: MotorBus> Drop for FiniteBenchOwner<B> {
    fn drop(&mut self) {
        if !self.closed {
            if let Err(error) = self.finish() {
                tracing::error!(%error, "neutral bench drop stop failed; use physical E-stop");
            }
        }
    }
}

impl<B: MotorBus> Supervisor<B> {
    /// Private generic body is testable, but only the closed real constructor can call it
    /// in production. Audit/readback data cannot be supplied by a caller.
    fn acquire_physical_bench_reference(
        &mut self,
        root: &Path,
        operator: &str,
        confirmed_home: bool,
        sign_attested: bool,
        output: BenchOutput,
    ) -> Result<(PathBuf, Vec<ProtocolObservation>), DavoutError> {
        validate_profile(self)?;
        let identities = PROFILE.map(|(_, _, _, identity)| identity);
        let qualification = self.qualify_bench_home_for_profile(
            confirmed_home,
            sign_attested,
            operator,
            Some(&identities),
            output.is_lower_yaw(),
        )?;
        let audit = write_audit(root, self, &qualification, output)?;
        // Filesystem completion cannot substitute for live drive/home continuity.
        let mut accepted = Vec::with_capacity(20);
        self.verify_bench_home(0x40, None, &mut accepted)?;
        self.reference_authority
            .select_physical_bench(
                self.robot.robot.joints.iter().cloned().collect(),
                &self.motors,
                &self.homing_config,
                &self.control,
                PhysicalBenchBinding {
                    model: self.installed_model.stamp(),
                    stop_generation: self.stop_generation(),
                    expires_at: Instant::now() + OWNER_LIFETIME,
                    output,
                },
            )
            .ok_or_else(|| bench_error("reference generation exhausted"))?;
        self.set_homing_complete()?;
        Ok((audit.path, accepted))
    }

    fn verify_bench_home(
        &mut self,
        first_host: u8,
        active: Option<(&[MitJointCommand], Instant)>,
        accepted: &mut Vec<ProtocolObservation>,
    ) -> Result<Vec<ProtocolReadReceipt>, DavoutError> {
        let context = if active.is_some() {
            ReceiveContext::Operational
        } else {
            ReceiveContext::DisabledInspection
        };
        let mut receipts = Vec::with_capacity(10);
        for (i, motor) in self.motors.motors.clone().iter().enumerate() {
            if active.is_some() && !self.reference_binding_valid() {
                return Err(bench_error("physical bench reference expired or revoked"));
            }
            for (offset, parameter) in [ParameterId::MechanicalPosition, ParameterId::CanTimeout]
                .into_iter()
                .enumerate()
            {
                let (reply, receipt) = self.query_protocol_in_context(
                    &motor.joint,
                    &MotorAddress::from(motor),
                    Query::Parameter(parameter),
                    first_host + (i * 2 + offset) as u8,
                    accepted,
                    &ProtocolQueryContext {
                        receive: &context,
                        neutral_keepalive: active.map(|(batch, _)| batch),
                        owner_deadline: active.map(|(_, deadline)| deadline),
                    },
                )?;
                let ProtocolReply::Parameter { value, .. } = reply.reply else {
                    return Err(bench_error("missing physical bench parameter reply"));
                };
                let valid = match parameter {
                    ParameterId::CanTimeout => u32::from_le_bytes(value) == BENCH_TIMEOUT_COUNTS,
                    _ => {
                        let position = f64::from(f32::from_le_bytes(value));
                        position.is_finite()
                            && position.abs()
                                <= MAX_HOME_DRIFT
                                    .min(self.homing_config.homing.zero_verify_tolerance_rad)
                    }
                };
                if !valid {
                    return Err(bench_error("fresh physical home/timeout readback changed"));
                }
                accepted.push(reply);
                receipts.push(receipt);
            }
        }
        self.protocol_receive(accepted, &context)?;
        Ok(receipts)
    }
}

fn validate_profile<B: MotorBus>(owner: &Supervisor<B>) -> Result<(), DavoutError> {
    if owner.motors.motors.len() != PROFILE.len() || owner.robot.robot.joints.len() != PROFILE.len()
    {
        return Err(bench_error(
            "neutral bench requires the complete five-joint arm",
        ));
    }
    for (i, (motor, (joint, model, direction, _))) in
        owner.motors.motors.iter().zip(PROFILE).enumerate()
    {
        if motor.joint != joint
            || owner.robot.robot.joints[i] != joint
            || motor.motor_type != model
            || motor.direction != direction
            || motor.gear_ratio != 1.0
            || motor.can_interface != "can0"
            || usize::from(motor.device_id) != i + 1
        {
            return Err(bench_error(
                "installed motor mapping differs from qualified physical arm",
            ));
        }
    }
    Ok(())
}

fn validate_neutral_batch(joints: &[String], batch: &[MitJointCommand]) -> Result<(), DavoutError> {
    validate_bench_batch(BenchOutput::Neutral, joints, batch)
}

pub(super) fn validate_bench_batch(
    output: BenchOutput,
    joints: &[String],
    batch: &[MitJointCommand],
) -> Result<(), DavoutError> {
    let mut seen = HashSet::new();
    if batch.len() != joints.len()
        || batch.iter().any(|cmd| {
            !joints.contains(&cmd.joint)
                || !seen.insert(&cmd.joint)
                || cmd.velocity_rad_s != 0.0
                || cmd.torque_ff_nm != 0.0
                || !(cmd.kp == 0.0 && cmd.kd == 0.0 && cmd.position_rad == 0.0
                    || output.gains().is_some_and(|gains| {
                        cmd.joint == LOWER_YAW
                            && cmd.kp == gains.kp()
                            && cmd.kd == gains.kd()
                            && (0.0..=0.02).contains(&cmd.position_rad)
                    }))
        })
    {
        return Err(bench_error(
            "closed bench refuses output outside its finite profile",
        ));
    }
    Ok(())
}

struct DurableBenchAudit {
    path: PathBuf,
}

fn write_audit<B: MotorBus>(
    root: &Path,
    owner: &Supervisor<B>,
    qualification: &BenchHomeQualification,
    output: BenchOutput,
) -> Result<DurableBenchAudit, DavoutError> {
    let result = (|| -> Result<DurableBenchAudit, Box<dyn std::error::Error>> {
        let directory = root.join("var/calibration/neutral-bench");
        fs::create_dir_all(&directory)?;
        let acquired = SystemTime::now().duration_since(UNIX_EPOCH)?;
        let path = directory.join(format!(
            "{}-{}.json",
            acquired.as_nanos(),
            std::process::id()
        ));
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 1,
            "evidence_class": if output == BenchOutput::Neutral { "physical_manual_home_neutral" } else { "physical_manual_home_lower_yaw_step" },
            "history_only": true,
            "neutral_only": output == BenchOutput::Neutral,
            "enable_confirmed": true,
            "neutral_enable_confirmed": output == BenchOutput::Neutral,
            "bounded_motion_confirmed": output.is_lower_yaw(),
            "enabled_budget_ms": output.duration().as_millis(),
            "lower_yaw_profile": output.gains().map(|gains| serde_json::json!({
                "max_target_rad": 0.02, "kp": gains.kp(), "kd": gains.kd(),
                "other_joints_neutral": true, "measured_home_drift_max_rad": MAX_HOME_DRIFT,
                "velocity_guard_joint": LOWER_YAW, "measured_velocity_max_rad_s": 0.25,
                "measured_velocity_source": "active_position_delta_per_drain_or_initial_raw;ready_raw" })),
            "timeout_volatility_qualified": false,
            "owner_lifetime_ms": OWNER_LIFETIME.as_millis(),
            "acquired_unix_ns": acquired.as_nanos(),
            "qualification": qualification,
            "installed_robot": owner.robot,
            "installed_motors": owner.motors,
            "installed_control": owner.control,
            "installed_homing": owner.homing_config,
            "installed_urdf": urdf_rs::write_to_string(&owner.urdf_robot)?,
        }))?;
        if bytes.len() > 1024 * 1024 {
            return Err("bench audit exceeds 1 MiB".into());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        for parent in directory
            .ancestors()
            .take_while(|path| path.starts_with(root))
        {
            File::open(parent)?.sync_all()?;
        }
        Ok(DurableBenchAudit { path })
    })();
    result.map_err(|error| bench_error(&format!("durable neutral bench audit: {error}")))
}

fn bench_error(message: &str) -> DavoutError {
    DavoutError::Homing {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::protocol_test_support::ProbeBus;
    use crate::{JointHomingState, OperationalMode};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct AuditRoot(PathBuf);
    impl AuditRoot {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "marengo-neutral-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("exclusive test root");
            Self(path)
        }
    }
    impl Drop for AuditRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn supervisor(bus: ProbeBus) -> Supervisor<ProbeBus> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        Supervisor::from_repo_for_protocol_inspection(root, bus).expect("installed supervisor")
    }

    fn owner(bus: ProbeBus, root: &AuditRoot) -> FiniteBenchOwner<ProbeBus> {
        owner_with_output(bus, root, BenchOutput::Neutral)
    }

    fn owner_with_output(
        bus: ProbeBus,
        root: &AuditRoot,
        output: BenchOutput,
    ) -> FiniteBenchOwner<ProbeBus> {
        owner_from_supervisor(supervisor(bus), root, output)
    }

    fn owner_from_supervisor(
        mut supervisor: Supervisor<ProbeBus>,
        root: &AuditRoot,
        output: BenchOutput,
    ) -> FiniteBenchOwner<ProbeBus> {
        let (audit_path, accepted_protocol) = supervisor
            .acquire_physical_bench_reference(&root.0, "test", true, true, output)
            .expect("actual fixture acquisition and durable audit");
        FiniteBenchOwner {
            joints: supervisor.robot.robot.joints.clone(),
            supervisor,
            audit_path,
            enable_attempted: false,
            closed: false,
            stop_report: None,
            accepted_protocol,
            active_deadline: None,
            output,
        }
    }

    fn neutral(joints: &[String]) -> Vec<MitJointCommand> {
        joints
            .iter()
            .map(|joint| MitJointCommand {
                joint: joint.clone(),
                kp: 0.0,
                kd: 0.0,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_ff_nm: 0.0,
            })
            .collect()
    }

    #[test]
    fn owned_acquisition_durably_audits_before_ready_and_rearm_is_impossible() {
        let root = AuditRoot::new();
        let mut owner = owner(ProbeBus::default(), &root);
        let audit: serde_json::Value =
            serde_json::from_slice(&fs::read(owner.audit_path()).expect("durable audit"))
                .expect("audit JSON");
        assert_eq!(audit["evidence_class"], "physical_manual_home_neutral");
        assert_eq!(audit["timeout_volatility_qualified"], false);
        assert_eq!(owner.supervisor.mode(), OperationalMode::Ready);
        assert!(owner
            .joints
            .iter()
            .all(|j| owner.supervisor.joint_homing_state(j) == JointHomingState::Verified));
        assert!(
            owner
                .supervisor
                .reference_authority
                .consumed_binding()
                .is_none(),
            "no virtual epoch binding"
        );
        let batch = neutral(&owner.joints);
        owner.begin(batch.clone()).expect("one enable");
        assert_eq!(owner.tick(batch.clone()).expect("neutral tick").len(), 5);
        let stop = owner.finish().expect("all-address stop");
        assert_eq!(stop.attempts.len(), 15);
        assert!(owner
            .joints
            .iter()
            .all(|j| owner.supervisor.joint_homing_state(j) == JointHomingState::Unhomed));
        let enables = owner
            .supervisor
            .bus
            .tx
            .iter()
            .filter(|f| f.id >> 24 == 3)
            .count();
        assert!(owner.begin(batch).is_err());
        assert_eq!(
            owner
                .supervisor
                .bus
                .tx
                .iter()
                .filter(|f| f.id >> 24 == 3)
                .count(),
            enables
        );
        assert!(owner
            .supervisor
            .bus
            .tx
            .iter()
            .filter(|f| f.id >> 24 == 24)
            .all(|f| f.data[6] == 0));
    }

    #[test]
    fn audit_failure_and_unknown_identity_never_select_or_enable() {
        for wrong_identity in [false, true] {
            let root = AuditRoot::new();
            fs::write(root.0.join("var"), b"blocks audit directory").expect("fixture");
            let mut supervisor = supervisor(ProbeBus {
                wrong_identity,
                ..ProbeBus::default()
            });
            assert!(supervisor
                .acquire_physical_bench_reference(&root.0, "test", true, true, BenchOutput::Neutral)
                .is_err());
            assert!(supervisor
                .reference_authority
                .physical_bench_binding()
                .is_none());
            assert!(supervisor.bus.tx.iter().all(|f| f.id >> 24 != 3));
            if wrong_identity {
                assert!(supervisor.bus.tx.iter().all(|f| f.id >> 24 != 6));
                assert!(supervisor.bus.tx.iter().all(|f| {
                    f.id >> 24 != 18
                        || f.data[..2] != ParameterId::CanTimeout.as_u16().to_le_bytes()
                }));
            }
            assert!(supervisor.set_homing_complete().is_err());
        }
    }

    #[test]
    fn expiry_revokes_before_enable_and_cannot_be_restored() {
        let root = AuditRoot::new();
        let mut owner = owner(ProbeBus::default(), &root);
        owner
            .supervisor
            .reference_authority
            .expire_physical_bench_for_test();
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        assert!(owner.supervisor.bus.tx.iter().all(|f| f.id >> 24 != 3));
    }

    #[test]
    fn nonneutral_and_incomplete_batches_are_rejected_before_enable() {
        for field in 0..6 {
            let root = AuditRoot::new();
            let mut owner = owner(ProbeBus::default(), &root);
            let mut batch = neutral(&owner.joints);
            match field {
                0 => batch[0].kp = 1.0,
                1 => batch[0].kd = 0.1,
                2 => batch[0].torque_ff_nm = 0.01,
                3 => batch[0].position_rad = 0.01,
                4 => batch[0].velocity_rad_s = 0.01,
                _ => {
                    batch.pop();
                }
            }
            assert!(owner.begin(batch).is_err());
            assert!(owner.supervisor.bus.tx.iter().all(|f| f.id >> 24 != 3));
        }
    }

    #[test]
    fn changed_timeout_after_enable_stops_without_grant_recovery() {
        let root = AuditRoot::new();
        let mut owner = owner(
            ProbeBus {
                reset_timeout_after_enable: true,
                ..ProbeBus::default()
            },
            &root,
        );
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        assert!(owner.closed);
        assert!(!owner.supervisor.reference_binding_valid());
    }

    #[test]
    fn unexpected_home_drift_stops_and_keeps_reporting_off() {
        let root = AuditRoot::new();
        let mut owner = owner(
            ProbeBus {
                drift_after_enable: true,
                ..ProbeBus::default()
            },
            &root,
        );
        let batch = neutral(&owner.joints);
        // Status drift refuses immediately, even when parameter reads say home.
        assert!(owner.begin(batch).is_err());
        assert!(owner.supervisor.has_latched_fault());
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        assert!(owner
            .supervisor
            .bus
            .tx
            .iter()
            .filter(|f| f.id >> 24 == 24)
            .all(|f| f.data[6] == 0));
    }

    #[test]
    fn active_readback_wait_keeps_watchdog_and_neutral_output_running() {
        for suppress_pose in [false, true] {
            let root = AuditRoot::new();
            let mut owner = owner(ProbeBus::default(), &root);
            owner.supervisor.bus.suppress_active_params = true;
            owner.supervisor.bus.suppress_active_pose = suppress_pose;
            let started = Instant::now();
            let error = owner
                .begin(neutral(&owner.joints))
                .expect_err("missing readback");
            if suppress_pose {
                assert!(error.to_string().contains("watchdog"), "{error}");
                assert!(started.elapsed() < Duration::from_millis(250));
            } else {
                assert!(error.to_string().contains("timed out"), "{error}");
            }
            assert!(
                owner
                    .supervisor
                    .bus
                    .tx
                    .iter()
                    .filter(|f| f.id >> 24 == 1)
                    .count()
                    > 25
            );
            assert!(owner.closed);
            assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        }
    }

    #[test]
    fn enabled_budget_includes_slow_parameter_checks() {
        let root = AuditRoot::new();
        let mut owner = owner(ProbeBus::default(), &root);
        owner.supervisor.bus.active_param_delay = Duration::from_millis(60);
        let started = Instant::now();
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        let elapsed = started.elapsed();
        assert!(elapsed >= ENABLE_DURATION);
        assert!(elapsed < Duration::from_millis(650));
        assert!(owner.closed);
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
    }

    #[test]
    fn transient_drift_followed_by_healthy_status_still_closes_owner() {
        let root = AuditRoot::new();
        let mut owner = owner(ProbeBus::default(), &root);
        owner.supervisor.bus.transient_drift = true;
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        assert!(owner.supervisor.has_latched_fault());
        assert!(owner.closed);
    }

    #[test]
    fn transient_enable_reply_drift_is_retained_while_host_is_still_ready() {
        let root = AuditRoot::new();
        let mut owner = owner(ProbeBus::default(), &root);
        owner.supervisor.bus.transient_enable_drift = true;
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        assert!(owner.supervisor.has_latched_fault());
        assert!(owner.closed);
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        assert!(owner.supervisor.bus.tx.iter().all(|f| {
            f.id >> 24 != 17 || robstride::unpack_ext_id(f.id).expect("query ID").extra_data != 0x60
        }));
    }

    #[test]
    fn second_begin_stops_the_active_owner() {
        let root = AuditRoot::new();
        let mut owner = owner(ProbeBus::default(), &root);
        let batch = neutral(&owner.joints);
        owner.begin(batch.clone()).expect("first enable");
        assert!(owner.begin(batch).is_err());
        assert!(owner.closed);
        assert!(!owner.supervisor.reference_binding_valid());
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
    }

    fn yaw_output() -> BenchOutput {
        BenchOutput::LowerYawStep(LowerYawBenchGains::new(10.0, 0.4).expect("bounded gains"))
    }

    fn yaw_commands(joints: &[String]) -> Vec<MitJointCommand> {
        let mut batch = neutral(joints);
        batch[4].kp = 10.0;
        batch[4].kd = 0.4;
        batch[4].position_rad = 0.02;
        batch
    }

    #[test]
    fn lower_yaw_profile_uses_joint_transform_and_keeps_other_axes_neutral() {
        let root = AuditRoot::new();
        let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
        let audit: serde_json::Value =
            serde_json::from_slice(&fs::read(owner.audit_path()).expect("audit")).expect("JSON");
        assert_eq!(audit["neutral_only"], false);
        assert_eq!(audit["enabled_budget_ms"], 1000);
        owner
            .begin(neutral(&owner.joints))
            .expect("neutral bootstrap");
        let first = owner.supervisor.bus.tx.len();
        owner
            .tick(yaw_commands(&owner.joints))
            .expect("bounded step");
        let mit: Vec<_> = owner.supervisor.bus.tx[first..]
            .iter()
            .filter(|f| f.id >> 24 == 1)
            .collect();
        assert_eq!(mit.len(), 5);
        for frame in &mit[..4] {
            assert_eq!(frame.data, [0x7F, 0xFF, 0x7F, 0xFF, 0, 0, 0, 0]);
        }
        let yaw = mit[4];
        assert_eq!(yaw.id & 255, 5);
        assert_eq!((yaw.id >> 8) & 65535, 0x7FFF);
        assert!(
            u16::from_be_bytes([yaw.data[0], yaw.data[1]]) < 0x7FFF,
            "negative motor direction for positive joint step"
        );
        assert_eq!(u16::from_be_bytes([yaw.data[4], yaw.data[5]]), 1311);
        assert_eq!(u16::from_be_bytes([yaw.data[6], yaw.data[7]]), 5243);
        owner.finish().expect("stop");
    }

    #[test]
    fn motion_profile_refuses_other_axes_and_unbounded_fields_then_stops() {
        for variant in 0..7 {
            let root = AuditRoot::new();
            let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
            owner.begin(neutral(&owner.joints)).expect("bootstrap");
            let mut batch = yaw_commands(&owner.joints);
            match variant {
                0 => batch[0].kp = 10.0,
                1 => batch[4].position_rad = 0.0201,
                2 => batch[4].position_rad = -0.001,
                3 => batch[4].torque_ff_nm = 0.01,
                4 => batch[4].velocity_rad_s = 0.01,
                5 => batch[4].kp = f64::NAN,
                _ => batch[4].kd = 0.5,
            }
            let first = owner.supervisor.bus.tx.len();
            assert!(owner.tick(batch).is_err());
            assert!(owner.closed);
            assert!(owner.supervisor.bus.tx[first..]
                .iter()
                .filter(|f| f.id >> 24 == 1)
                .all(|f| f.data == [0x7F, 0xFF, 0x7F, 0xFF, 0, 0, 0, 0]));
            assert!(!owner.supervisor.reference_binding_valid());
        }
    }

    #[test]
    fn first_motion_cannot_mask_out_of_home_position_with_a_new_zero() {
        let root = AuditRoot::new();
        let mut supervisor = supervisor(ProbeBus {
            bad_zero: true,
            ..ProbeBus::default()
        });
        assert!(supervisor
            .acquire_physical_bench_reference(&root.0, "test", true, true, yaw_output())
            .is_err());
        assert!(supervisor
            .bus
            .tx
            .iter()
            .all(|f| !matches!(f.id >> 24, 3 | 6)));
    }

    #[test]
    fn taught_soft_limit_cannot_widen_the_filtered_motion_profile() {
        let root = AuditRoot::new();
        let mut supervisor = supervisor(ProbeBus::default());
        supervisor
            .control
            .control
            .joints
            .get_mut(LOWER_YAW)
            .expect("joint")
            .position_soft_lower_rad = Some(0.03);
        supervisor.rebuild_limits().expect("valid taught bounds");
        let mut owner = owner_from_supervisor(supervisor, &root, yaw_output());
        owner
            .begin(neutral(&owner.joints))
            .expect("neutral bootstrap");
        let first = owner.supervisor.bus.tx.len();
        assert!(owner.tick(yaw_commands(&owner.joints)).is_err());
        assert!(owner.closed);
        assert!(owner.supervisor.bus.tx[first..]
            .iter()
            .filter(|f| f.id >> 24 == 1)
            .all(|f| f.data == [0x7F, 0xFF, 0x7F, 0xFF, 0, 0, 0, 0]));
    }

    #[test]
    fn captured_stationary_roll_velocity_estimate_does_not_block_yaw_bootstrap() {
        let root = AuditRoot::new();
        let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
        owner.supervisor.bus.stationary_roll_velocity_spike = true;
        owner
            .begin(neutral(&owner.joints))
            .expect("stationary pose spike is handled by ordinary feedback guards");
        let feedback = owner
            .tick(yaw_commands(&owner.joints))
            .expect("bounded yaw tick");
        let roll = feedback
            .iter()
            .find(|sample| sample.joint == "right_shoulder_roll")
            .expect("roll feedback");
        assert_eq!(roll.position_rad, 0.0);
        assert_eq!(roll.velocity_rad_s, 0.0);
        owner.finish().expect("stop");
    }

    #[test]
    fn lower_yaw_profile_velocity_guard_still_stops_its_commanded_joint() {
        let root = AuditRoot::new();
        let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
        owner.supervisor.bus.lower_yaw_velocity_spike = true;
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        assert!(owner.closed);
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
    }

    #[test]
    fn lower_yaw_ready_enable_reply_overspeed_stops_before_gained_output() {
        let root = AuditRoot::new();
        let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
        owner.supervisor.bus.lower_yaw_enable_velocity_spike = true;
        let first = owner.supervisor.bus.tx.len();
        assert!(owner.begin(neutral(&owner.joints)).is_err());
        assert!(owner.closed);
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        assert!(owner.supervisor.bus.tx[first..]
            .iter()
            .filter(|frame| frame.id >> 24 == 1)
            .all(|frame| frame.data == [0x7F, 0xFF, 0x7F, 0xFF, 0, 0, 0, 0]));
        assert_eq!(owner.finish().expect("cached stop").attempts.len(), 15);
    }

    fn replay_yaw_pose(
        owner: &mut FiniteBenchOwner<ProbeBus>,
        previous_data: [u8; 8],
        data: [u8; 8],
        spacing: Duration,
    ) -> super::super::feedback_consumer::FeedbackConsumption {
        let received_at = Instant::now();
        let previous =
            robstride::mit::decode_mit_feedback(MotorType::Rs00, 0x0280_05FD, &previous_data)
                .expect("captured previous lower-yaw frame");
        owner.supervisor.last_feedback_samples.insert(
            LOWER_YAW.into(),
            super::super::FeedbackSample {
                position_rad: -f64::from(previous.position_rad),
                received_at: received_at - spacing,
            },
        );
        owner.supervisor.bus.rx.clear();
        owner.supervisor.bus.rx.push_back(robstride::TimedCanFrame {
            received_at,
            received: robstride::ReceivedCanFrame::full_data(
                Some("can0".into()),
                robstride::CanFrame {
                    id: 0x0280_05FD,
                    data,
                    extended: true,
                },
            ),
        });
        let report = owner.supervisor.bus.recv_feedback_report(
            &owner.supervisor.motor_types,
            Duration::ZERO,
            Duration::ZERO,
        );
        owner.supervisor.consume_feedback_report(report)
    }

    #[test]
    fn captured_lower_yaw_velocity_spike_uses_position_derived_speed() {
        let root = AuditRoot::new();
        let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
        owner.begin(neutral(&owner.joints)).expect("bootstrap");
        let consumed = replay_yaw_pose(
            &mut owner,
            [0x7F, 0xE0, 0x7F, 0x9A, 0x7F, 0x95, 0, 0xF0],
            [0x7F, 0xDF, 0x7F, 0x59, 0x7F, 0x56, 0, 0xF0],
            Duration::from_micros(4996),
        );
        assert!(
            consumed.first_error.is_none(),
            "captured 0.253-rad/s estimate with 0.077-rad/s position change: {:?}",
            consumed.first_error
        );
        let yaw = owner.supervisor.joint_feedback(LOWER_YAW).expect("yaw");
        assert!((yaw.velocity_rad_s - 0.07676).abs() < 0.002);
        owner.finish().expect("stop");
    }

    #[test]
    fn lower_yaw_position_derived_overspeed_stops_despite_zero_raw_velocity() {
        let root = AuditRoot::new();
        let mut owner = owner_with_output(ProbeBus::default(), &root, yaw_output());
        owner.begin(neutral(&owner.joints)).expect("bootstrap");
        let consumed = replay_yaw_pose(
            &mut owner,
            [0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xF0],
            [0x7F, 0xFB, 0x7F, 0xFF, 0x7F, 0xFF, 0, 0xF0],
            Duration::from_millis(5),
        );
        let error = consumed.first_error.expect("0.307-rad/s pose derivative");
        assert!(matches!(error, DavoutError::Limit { .. }));
        owner.finish_after_error(error);
        assert!(owner.closed);
        assert_eq!(owner.supervisor.mode(), OperationalMode::Disabled);
        assert_eq!(owner.finish().expect("cached stop").attempts.len(), 15);
    }

    #[test]
    fn tuning_gains_are_bounded_and_immutable_for_an_acquired_owner() {
        for (kp, kd) in [
            (f64::NAN, 1.0),
            (30.0, f64::INFINITY),
            (9.99, 1.0),
            (30.01, 1.0),
            (10.0, 0.39),
            (10.0, 2.01),
        ] {
            assert!(LowerYawBenchGains::new(kp, kd).is_err());
        }
        let root = AuditRoot::new();
        let gains = LowerYawBenchGains::new(30.0, 1.0).expect("bounded tuning gains");
        let mut owner =
            owner_with_output(ProbeBus::default(), &root, BenchOutput::LowerYawStep(gains));
        owner.begin(neutral(&owner.joints)).expect("bootstrap");
        let mut batch = yaw_commands(&owner.joints);
        batch[4].kp = gains.kp();
        batch[4].kd = gains.kd();
        let first = owner.supervisor.bus.tx.len();
        owner.tick(batch).expect("selected gains");
        let yaw = owner.supervisor.bus.tx[first..]
            .iter()
            .find(|f| f.id >> 24 == 1 && f.id & 255 == 5)
            .expect("yaw frame");
        assert_eq!(u16::from_be_bytes([yaw.data[4], yaw.data[5]]), 3932);
        assert_eq!(u16::from_be_bytes([yaw.data[6], yaw.data[7]]), 13107);
        assert!(
            owner.tick(yaw_commands(&owner.joints)).is_err(),
            "gain change requires a new finite owner"
        );
        assert!(owner.closed);
    }
}
