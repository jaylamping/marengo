//! Closed real five-drive owner for one finite neutral qualification.

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
use super::protocol_inspection::Query;
use super::reference::PhysicalBenchBinding;
use super::{
    BenchHomeQualification, DavoutError, MitJointCommand, ProtocolReadReceipt, StopReport,
    Supervisor, BENCH_TIMEOUT_COUNTS,
};

const OWNER_LIFETIME: Duration = Duration::from_secs(5);
const MAX_HOME_DRIFT: f64 = 0.05;
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
    owner: NeutralBenchOwner<RuntimeBus>,
}

struct NeutralBenchOwner<B: MotorBus> {
    supervisor: Supervisor<B>,
    joints: Vec<String>,
    audit_path: PathBuf,
    enable_attempted: bool,
    closed: bool,
    stop_report: Option<StopReport>,
    accepted_protocol: Vec<ProtocolObservation>,
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
        if !confirmed_home || !sign_attested || !confirmed_neutral_enable {
            return Err(bench_error(
                "home, sign and neutral-enable approval are required",
            ));
        }
        let root = root.as_ref();
        let motors = load_motors_config(root)?;
        let bus = RuntimeBus::socketcan_from_motors(&motors)?;
        let supervisor = Supervisor::from_repo_for_protocol_inspection(root, bus)?;
        let mut owner = NeutralBenchOwner {
            joints: supervisor.robot.robot.joints.clone(),
            supervisor,
            audit_path: PathBuf::new(),
            enable_attempted: false,
            closed: false,
            stop_report: None,
            accepted_protocol: Vec::new(),
        };
        let acquisition = owner.supervisor.acquire_neutral_bench_reference(
            root,
            operator,
            confirmed_home,
            sign_attested,
        );
        let (audit_path, accepted) =
            acquisition.map_err(|error| owner.finish_after_error(error))?;
        owner.audit_path = audit_path;
        owner.accepted_protocol = accepted;
        Ok(Self { owner })
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
}

impl<B: MotorBus> NeutralBenchOwner<B> {
    fn joints(&self) -> &[String] {
        &self.joints
    }

    pub fn audit_path(&self) -> &Path {
        &self.audit_path
    }

    /// Exactly one ordinary Davout enable session; no recovery/re-arm.
    pub fn begin(&mut self, neutral: Vec<MitJointCommand>) -> Result<(), DavoutError> {
        if self.closed || self.enable_attempted {
            return Err(bench_error("neutral bench enable already consumed"));
        }
        validate_neutral_batch(&self.joints, &neutral)?;
        self.enable_attempted = true;
        let result = (|| {
            // The last disabled report must still be Reset before normal enable.
            self.supervisor
                .inspection_receive(&self.accepted_protocol)?;
            self.supervisor.enable_targets(&self.joints)?;
            self.supervisor.send_mit_batch(neutral)?;
            self.supervisor
                .verify_neutral_bench_home(0x60, true, &mut self.accepted_protocol)?;
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
            if self.closed || !self.enable_attempted {
                return Err(bench_error("neutral bench is not running"));
            }
            validate_neutral_batch(&self.joints, &neutral)?;
            self.supervisor.ensure_reference_for(&self.joints)?;
            self.supervisor
                .protocol_receive(&self.accepted_protocol, &ReceiveContext::Operational)?;
            self.supervisor.send_mit_batch(neutral)?;
            self.supervisor
                .protocol_receive(&self.accepted_protocol, &ReceiveContext::Operational)?;
            let mut feedback = Vec::with_capacity(self.joints.len());
            for joint in &self.joints {
                if let Some(sample) = self.supervisor.joint_feedback(joint) {
                    if sample.position_rad.abs() > MAX_HOME_DRIFT {
                        self.supervisor.latch_control_fault(
                            "neutral bench moved outside supported home tolerance",
                            Some(joint),
                        );
                        return Err(bench_error("neutral bench home drift exceeds 0.05 rad"));
                    }
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

impl<B: MotorBus> Drop for NeutralBenchOwner<B> {
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
    fn acquire_neutral_bench_reference(
        &mut self,
        root: &Path,
        operator: &str,
        confirmed_home: bool,
        sign_attested: bool,
    ) -> Result<(PathBuf, Vec<ProtocolObservation>), DavoutError> {
        validate_profile(self)?;
        let qualification =
            self.qualify_bench_home_disabled(confirmed_home, sign_attested, operator)?;
        for (observed, (_, _, _, identity)) in qualification.before.iter().zip(PROFILE) {
            if observed.identity_wire_bytes != identity {
                return Err(bench_error(
                    "MCU identity does not match qualified physical arm",
                ));
            }
        }
        let audit = write_audit(root, self, &qualification)?;
        // Filesystem completion cannot substitute for live drive/home continuity.
        let mut accepted = Vec::with_capacity(20);
        self.verify_neutral_bench_home(0x40, false, &mut accepted)?;
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
                },
            )
            .ok_or_else(|| bench_error("reference generation exhausted"))?;
        self.set_homing_complete()?;
        Ok((audit.path, accepted))
    }

    fn verify_neutral_bench_home(
        &mut self,
        first_host: u8,
        active: bool,
        accepted: &mut Vec<ProtocolObservation>,
    ) -> Result<Vec<ProtocolReadReceipt>, DavoutError> {
        let context = if active {
            ReceiveContext::Operational
        } else {
            ReceiveContext::DisabledInspection
        };
        let mut receipts = Vec::with_capacity(10);
        for (i, motor) in self.motors.motors.clone().iter().enumerate() {
            if active && !self.reference_binding_valid() {
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
                    &context,
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
    let mut seen = HashSet::new();
    if batch.len() != joints.len()
        || batch.iter().any(|cmd| {
            !joints.contains(&cmd.joint)
                || !seen.insert(&cmd.joint)
                || cmd.kp != 0.0
                || cmd.kd != 0.0
                || cmd.position_rad != 0.0
                || cmd.velocity_rad_s != 0.0
                || cmd.torque_ff_nm != 0.0
        })
    {
        return Err(bench_error(
            "closed neutral bench refuses nonneutral or incomplete output",
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
            "evidence_class": "physical_manual_home_neutral",
            "history_only": true,
            "neutral_only": true,
            "neutral_enable_confirmed": true,
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

    fn owner(bus: ProbeBus, root: &AuditRoot) -> NeutralBenchOwner<ProbeBus> {
        let mut supervisor = supervisor(bus);
        let (audit_path, accepted_protocol) = supervisor
            .acquire_neutral_bench_reference(&root.0, "test", true, true)
            .expect("actual fixture acquisition and durable audit");
        NeutralBenchOwner {
            joints: supervisor.robot.robot.joints.clone(),
            supervisor,
            audit_path,
            enable_attempted: false,
            closed: false,
            stop_report: None,
            accepted_protocol,
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
                .acquire_neutral_bench_reference(&root.0, "test", true, true)
                .is_err());
            assert!(supervisor
                .reference_authority
                .physical_bench_binding()
                .is_none());
            assert!(supervisor.bus.tx.iter().all(|f| f.id >> 24 != 3));
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
        // The initial active status already reveals drift before further ticks.
        owner
            .begin(batch.clone())
            .expect("parameter readback remains at home");
        assert!(owner.tick(batch).is_err());
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
}
