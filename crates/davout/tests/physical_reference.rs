//! Qualified physical Robstride reference workflow (ADR 0036) through public
//! `Supervisor` APIs against a test-only firmware emulator. No hardware transport.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "../../marengo-homing/tests/support/mod.rs"]
mod support;

mod physical_firmware;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use davout::simulation::SimulationBus;
use davout::{
    DavoutError, JointHomingState, OperationalMode, ReferenceAudit, ReferenceError,
    ReferenceOutcome, Supervisor,
};
use marengo_config::{load_homing_config_from, load_motors_config_from, HomingMethod};
use physical_firmware::{Firmware, FirmwareBus, SharedFirmware};
use robstride::CommunicationType;
use support::TestDirectory;

const PITCH: &str = "right_shoulder_pitch";
const ROLL: &str = "right_shoulder_roll";
const OPERATOR: &str = "bench-operator";
/// Fixture acquisition deadline (master config uses 30 s).
const SEARCH_TIMEOUT_S: f64 = 0.5;
/// Test pump period for periodic type-24 reports; well inside comm_watchdog_ms.
const PUMP_PERIOD: Duration = Duration::from_millis(5);
/// Pre-reference joint poses inside every bench envelope. Pitch is far from
/// zero so a lost firmware zero is a clear coordinate discontinuity.
const INITIAL: [(&str, f64); 5] = [
    (PITCH, 1.5),
    (ROLL, 0.4),
    ("right_upper_arm_yaw", 0.2),
    ("right_elbow_pitch", 0.3),
    ("right_lower_arm_yaw", 0.3),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Copy the master config/URDF into an owned tree with a short reference
/// deadline and a calibration history path relative to that tree.
fn fixture_root(directory: &Path) -> PathBuf {
    let source = repo_root();
    let root = directory.join("repo");
    let config = root.join("config");
    fs::create_dir_all(&config).expect("fixture config directory");
    for name in ["robot.yaml", "motors.yaml", "control.yaml"] {
        fs::copy(source.join("config").join(name), config.join(name))
            .expect("copy immutable fixture input");
    }
    let mut homing = load_homing_config_from(source.join("config")).expect("master homing");
    homing.homing.calibration_record_path = "var/calibration/zero_registry.yaml".into();
    homing.homing.defaults.search_timeout_s = SEARCH_TIMEOUT_S;
    for entry in homing.homing.joints.values_mut() {
        entry.overrides.search_timeout_s = None;
    }
    fs::write(
        config.join("homing.yaml"),
        serde_yaml::to_string(&homing).expect("fixture homing yaml"),
    )
    .expect("write fixture homing");
    let model = root.join("assets/urdf");
    fs::create_dir_all(&model).expect("fixture model directory");
    fs::copy(
        source.join("assets/urdf/marengo.urdf"),
        model.join("marengo.urdf"),
    )
    .expect("copy immutable model input");
    root
}

fn firmware_for(root: &Path) -> SharedFirmware {
    let motors = load_motors_config_from(root.join("config")).expect("fixture motors");
    Firmware::from_motors(&motors.motors, &INITIAL)
}

struct Bench {
    // Declaration order is drop order: the owner and its journal worker close
    // before the directory holding the journal is removed.
    supervisor: Supervisor<FirmwareBus>,
    firmware: SharedFirmware,
    journal: PathBuf,
    _directory: TestDirectory,
}

impl Bench {
    fn physical(label: &str) -> Self {
        let directory = TestDirectory::new(label);
        let root = fixture_root(directory.path());
        let record = directory.path().join("history.yaml");
        let journal = directory.path().join("reference-journal.sqlite3");
        let firmware = firmware_for(&root);
        let supervisor = Supervisor::from_repo_with_physical_reference_and_record_path(
            &root,
            FirmwareBus(firmware.clone()),
            &record,
            &journal,
        )
        .expect("physical reference owner");
        assert_eq!(supervisor.mode(), OperationalMode::Disabled);
        for motor in &supervisor.motors.motors {
            assert_eq!(
                supervisor.joint_homing_state(&motor.joint),
                JointHomingState::Unhomed,
                "every startup is unreferenced"
            );
        }
        firmware.borrow_mut().clear_trace();
        Self {
            supervisor,
            firmware,
            journal,
            _directory: directory,
        }
    }

    fn calibrate(&mut self, joint: &str) -> Result<f64, DavoutError> {
        self.supervisor.calibrate_joint_zero(joint, OPERATOR, true)
    }

    fn state(&self, joint: &str) -> JointHomingState {
        self.supervisor.joint_homing_state(joint)
    }

    fn device(&self, joint: &str) -> u8 {
        self.firmware.borrow().drive(joint).device_id
    }

    fn sent(&self, comm_type: CommunicationType, joint: &str) -> usize {
        let device = self.device(joint);
        self.firmware.borrow().sent(comm_type, device)
    }

    fn sent_any(&self, comm_type: CommunicationType) -> usize {
        self.firmware.borrow().sent_any(comm_type)
    }

    fn tolerance(&self) -> f64 {
        self.supervisor
            .homing_config
            .homing
            .zero_verify_tolerance_rad
    }

    /// The control loop's periodic work while drives stream type-24 reports.
    fn pump(&mut self, duration: Duration) {
        let end = Instant::now() + duration;
        loop {
            self.pump_once();
            if Instant::now() >= end {
                return;
            }
            std::thread::sleep(PUMP_PERIOD);
        }
    }

    /// One report from every streaming drive, then one control-loop drain.
    fn pump_once(&mut self) {
        self.firmware.borrow_mut().emit_reports();
        self.supervisor.tick_active_reporting_leases();
        let _ = self.supervisor.drain_feedback();
    }

    fn acquire(&mut self, joint: &str) {
        let position = self.calibrate(joint).expect("qualified physical reference");
        assert!(position.abs() <= self.tolerance(), "{joint} at {position}");
        assert_eq!(self.state(joint), JointHomingState::Verified, "{joint}");
    }

    /// Blocking calibration refuses with `reason` and leaves no grant or work.
    fn assert_refused(&mut self, joint: &str, reason: &str) {
        let error = self
            .calibrate(joint)
            .expect_err("unqualified evidence must not grant");
        assert!(
            matches!(error, DavoutError::HomingVerify { .. }) && error.to_string().contains(reason),
            "{joint}: expected {reason:?}, got {error}"
        );
        assert_ne!(self.state(joint), JointHomingState::Verified, "{joint}");
        assert!(!self.supervisor.reference_work_pending());
    }

    fn assert_all_drives_stopped(&self) {
        let firmware = self.firmware.borrow();
        for drive in &firmware.drives {
            assert!(!drive.enabled, "{} left enabled", drive.joint);
        }
    }
}

// ---------------------------------------------------------------- positive

#[test]
fn single_joint_reference_arms_only_target_stops_all_and_records_physical_history() {
    let mut bench = Bench::physical("physical-single");
    bench.acquire(PITCH);
    let target = bench.device(PITCH);
    {
        let firmware = bench.firmware.borrow();
        let tx = &firmware.tx;
        let kind = |frame: &robstride::CanFrame| (frame.id >> 24) & 0x1f;
        let low = |frame: &robstride::CanFrame| (frame.id & 0xff) as u8;
        // Enable and SetZero are addressed to the target only, exactly once.
        assert_eq!(firmware.sent_any(CommunicationType::Enable), 1);
        assert_eq!(firmware.sent(CommunicationType::Enable, target), 1);
        assert_eq!(firmware.sent_any(CommunicationType::SetZeroPosition), 1);
        assert_eq!(firmware.sent(CommunicationType::SetZeroPosition, target), 1);
        assert_eq!(firmware.sent_any(CommunicationType::ReadParameter), 1);
        assert_eq!(firmware.sent(CommunicationType::ReadParameter, target), 1);
        // Order: identity → enable → SetZero → mechPos read → all-address stop.
        let position = |comm: CommunicationType| {
            tx.iter()
                .position(|frame| {
                    kind(frame) == u32::from(comm.as_u8())
                        && low(frame) == target
                        && ((frame.id >> 8) & 0xff) as u8 == robstride::DEFAULT_HOST_ID
                })
                .expect("target frame")
        };
        let identity = position(CommunicationType::GetDeviceId);
        let enable = position(CommunicationType::Enable);
        let zero = position(CommunicationType::SetZeroPosition);
        let read = position(CommunicationType::ReadParameter);
        assert!(identity < enable && enable < zero && zero < read);
        for drive in &firmware.drives {
            assert!(
                tx[read..].iter().any(|frame| {
                    kind(frame) == u32::from(CommunicationType::Disable.as_u8())
                        && low(frame) == drive.device_id
                }),
                "terminal stop must disable {}",
                drive.joint
            );
        }
        let drive = firmware.drive(PITCH);
        assert!(drive.position_joint_rad().abs() <= 1e-9);
    }
    bench.assert_all_drives_stopped();
    for motor in bench.supervisor.motors.motors.clone() {
        if motor.joint != PITCH {
            assert_eq!(bench.state(&motor.joint), JointHomingState::Unhomed);
        }
    }
    assert!(!bench.supervisor.has_latched_fault());

    // Periodic reporting keeps the grant live well beyond comm_watchdog_ms.
    bench.pump(Duration::from_millis(300));
    assert_eq!(bench.state(PITCH), JointHomingState::Verified);
    assert!(!bench.supervisor.has_latched_fault());

    let uid = bench.firmware.borrow().drive(PITCH).uid;
    bench.supervisor.close_reference_journal_admission();
    assert!(bench
        .supervisor
        .drain_reference_journal_until(Instant::now() + Duration::from_secs(5))
        .is_complete());
    let records = Supervisor::<SimulationBus>::inspect_reference_journal(&bench.journal, 8)
        .expect("durable physical history");
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert!(record.is_physical());
    assert_eq!(record.joint(), PITCH);
    assert_eq!(record.device_uid(), Some(u64::from_le_bytes(uid)));
    assert_eq!(record.audit().operator, OPERATOR);
    assert!(f64::from(record.position_rad()).abs() <= bench.tolerance());
    // Shutdown collection ends every grant; history never grants.
    assert_ne!(bench.state(PITCH), JointHomingState::Verified);
}

#[test]
fn grants_accumulate_per_joint() {
    let mut bench = Bench::physical("physical-accumulate");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(30));
    bench.acquire(ROLL);
    assert_eq!(bench.state(PITCH), JointHomingState::Verified);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 2);
    assert_eq!(bench.sent(CommunicationType::Enable, PITCH), 1);
    assert_eq!(bench.sent(CommunicationType::Enable, ROLL), 1);
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 2);
    bench.assert_all_drives_stopped();
    bench.pump(Duration::from_millis(300));
    assert_eq!(bench.state(PITCH), JointHomingState::Verified);
    assert_eq!(bench.state(ROLL), JointHomingState::Verified);
}

#[test]
fn non_blocking_request_reaches_current_through_owner_work() {
    let mut bench = Bench::physical("physical-nonblocking");
    let handle = bench
        .supervisor
        .request_reference(
            PITCH,
            true,
            ReferenceAudit {
                operator: OPERATOR.into(),
                session: "physical-reference-test".into(),
            },
        )
        .expect("physical request admitted");
    assert!(bench.supervisor.reference_work_pending());
    let deadline = Instant::now() + Duration::from_secs(5);
    let position = loop {
        if bench.supervisor.reference_work_pending() {
            bench
                .supervisor
                .advance_reference_work()
                .expect("owner work advances");
        }
        match bench
            .supervisor
            .reference_outcome(&handle)
            .expect("outcome")
        {
            ReferenceOutcome::Current { position_rad } => break position_rad,
            ReferenceOutcome::Failed { message } => panic!("reference failed: {message}"),
            ReferenceOutcome::InProgress => {
                assert!(Instant::now() < deadline, "reference did not finish");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    };
    assert!(f64::from(position).abs() <= bench.tolerance());
    assert!(!bench.supervisor.reference_work_pending());
    assert_eq!(bench.state(PITCH), JointHomingState::Verified);
    bench.assert_all_drives_stopped();
}

#[test]
fn matching_identity_admits_target_enable() {
    let mut bench = Bench::physical("physical-enable-admitted");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    bench.firmware.borrow_mut().clear_trace();
    bench
        .supervisor
        .enable_targets(&[PITCH.to_owned()])
        .expect("identity re-read matches the grant");
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    assert_eq!(bench.sent(CommunicationType::GetDeviceId, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 1);
    assert_eq!(bench.sent(CommunicationType::Enable, PITCH), 1);
    bench.supervisor.disable_all().expect("stop");
}

// ---------------------------------------------------------------- acquisition refusals

#[test]
fn missing_set_zero_ack_times_out_before_readback() {
    let mut bench = Bench::physical("physical-no-ack");
    bench
        .firmware
        .borrow_mut()
        .drive_mut(PITCH)
        .drop_set_zero_ack = true;
    bench.assert_refused(PITCH, "TimedOut");
    assert_eq!(bench.sent(CommunicationType::SetZeroPosition, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::ReadParameter), 0);
    bench.assert_all_drives_stopped();
}

#[test]
fn stale_pre_set_zero_status_is_not_an_ack() {
    let mut bench = Bench::physical("physical-stale-ack");
    {
        let mut firmware = bench.firmware.borrow_mut();
        let drive = firmware.drive_mut(PITCH);
        drive.hold_enable_reply_until_set_zero = true;
        drive.drop_set_zero_ack = true;
    }
    bench.assert_refused(PITCH, "TimedOut");
    assert_eq!(bench.sent(CommunicationType::SetZeroPosition, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::ReadParameter), 0);
    bench.assert_all_drives_stopped();
}

#[test]
fn readback_outside_tolerance_refuses() {
    let mut bench = Bench::physical("physical-readback-offset");
    bench
        .firmware
        .borrow_mut()
        .drive_mut(PITCH)
        .readback_offset_joint_rad = 0.2;
    bench.assert_refused(PITCH, "outside tolerance");
    assert_eq!(bench.sent(CommunicationType::ReadParameter, PITCH), 1);
    assert!(!bench.supervisor.has_latched_fault());
    bench.assert_all_drives_stopped();
}

#[test]
fn readback_with_nonzero_status_refuses() {
    let mut bench = Bench::physical("physical-readback-status");
    bench.firmware.borrow_mut().drive_mut(PITCH).readback_status = 1;
    bench.assert_refused(PITCH, "refused mechPos read (status 1)");
    assert_eq!(bench.sent(CommunicationType::ReadParameter, PITCH), 1);
    assert!(!bench.supervisor.has_latched_fault());
    bench.assert_all_drives_stopped();
}

#[test]
fn missing_identity_at_acquisition_sends_no_enable_or_zero() {
    let mut bench = Bench::physical("physical-no-uid");
    bench.firmware.borrow_mut().drive_mut(PITCH).answer_identity = false;
    bench.assert_refused(PITCH, "TimedOut");
    assert_eq!(bench.sent(CommunicationType::GetDeviceId, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 0);
}

#[test]
fn identity_answering_at_two_addresses_refuses_before_arming() {
    let mut bench = Bench::physical("physical-duplicate-uid");
    bench.acquire(PITCH);
    {
        let mut firmware = bench.firmware.borrow_mut();
        let uid = firmware.drive(PITCH).uid;
        firmware.drive_mut(ROLL).uid = uid;
        firmware.clear_trace();
    }
    bench.assert_refused(ROLL, "also answers at another installed address");
    assert_eq!(bench.sent(CommunicationType::GetDeviceId, ROLL), 1);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 0);
    assert!(!bench.supervisor.has_latched_fault());
}

#[test]
fn reboot_after_ack_refuses_whether_drive_answers_or_stays_silent() {
    // Back at once: mechPos reports the pre-zero pose. Still rebooting: no reply.
    for (silence, reason) in [
        (
            Duration::ZERO,
            "mechPos readback 1.5 rad is outside tolerance",
        ),
        (Duration::from_secs(2), "TimedOut"),
    ] {
        let mut bench = Bench::physical("physical-reboot-in-transaction");
        bench
            .firmware
            .borrow_mut()
            .drive_mut(PITCH)
            .reboot_after_ack = Some(silence);
        bench.assert_refused(PITCH, reason);
        assert_eq!(bench.sent(CommunicationType::SetZeroPosition, PITCH), 1);
        assert_eq!(bench.sent(CommunicationType::ReadParameter, PITCH), 1);
        let firmware = bench.firmware.borrow();
        let drive = firmware.drive(PITCH);
        assert!(!drive.enabled);
        assert!(
            (drive.position_joint_rad() - 1.5).abs() < 1e-9,
            "zero was lost"
        );
    }
}

#[test]
fn baseline_stop_write_failure_refuses_before_arming() {
    let mut bench = Bench::physical("physical-baseline-stop");
    bench
        .firmware
        .borrow_mut()
        .drive_mut("right_elbow_pitch")
        .fail_writes
        .push(CommunicationType::Disable.as_u8());
    bench.assert_refused(PITCH, "baseline stop was uncertain");
    assert!(!bench.firmware.borrow().failed_tx.is_empty());
    assert_eq!(bench.sent_any(CommunicationType::GetDeviceId), 0);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 0);
    assert!(bench.supervisor.has_latched_fault());
}

#[test]
fn terminal_stop_failure_refuses_storage_and_grant() {
    let mut bench = Bench::physical("physical-terminal-stop");
    bench
        .firmware
        .borrow_mut()
        .drive_mut(PITCH)
        .fail_writes_after_readback
        .push(CommunicationType::Disable.as_u8());
    bench.assert_refused(PITCH, "StopUncertain");
    assert_eq!(bench.sent(CommunicationType::SetZeroPosition, PITCH), 1);
    assert_eq!(bench.sent(CommunicationType::ReadParameter, PITCH), 1);
    assert!(!bench.firmware.borrow().failed_tx.is_empty());
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(
        Supervisor::<SimulationBus>::inspect_reference_journal(&bench.journal, 8)
            .map(|records| records.len())
            .unwrap_or(0),
        0,
        "uncertain stop must not reach durable history"
    );
}

// ---------------------------------------------------------------- grant revocation

#[test]
fn identity_mismatch_at_enable_revokes_without_enable() {
    let mut bench = Bench::physical("physical-enable-uid-swap");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    {
        let mut firmware = bench.firmware.borrow_mut();
        firmware.drive_mut(PITCH).uid[0] ^= 0xff;
        firmware.clear_trace();
    }
    let error = bench
        .supervisor
        .enable_targets(&[PITCH.to_owned()])
        .expect_err("swapped drive must not be enabled");
    assert!(matches!(error, DavoutError::HomingVerify { .. }), "{error}");
    assert_eq!(bench.sent(CommunicationType::GetDeviceId, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    // Restoring the original drive does not restore the revoked grant.
    bench.firmware.borrow_mut().drive_mut(PITCH).uid[0] ^= 0xff;
    bench.pump(Duration::from_millis(20));
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    assert!(bench.supervisor.request_enable(true).is_err());
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
}

#[test]
fn missing_identity_at_enable_revokes_without_enable() {
    let mut bench = Bench::physical("physical-enable-no-uid");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    {
        let mut firmware = bench.firmware.borrow_mut();
        firmware.drive_mut(PITCH).answer_identity = false;
        firmware.clear_trace();
    }
    let error = bench
        .supervisor
        .enable_targets(&[PITCH.to_owned()])
        .expect_err("silent identity must not be enabled");
    assert!(matches!(error, DavoutError::HomingVerify { .. }), "{error}");
    assert_eq!(bench.sent(CommunicationType::GetDeviceId, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
}

#[test]
fn reboot_silence_beyond_watchdog_revokes_only_that_joint() {
    let mut bench = Bench::physical("physical-reboot-silence");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    bench.acquire(ROLL);
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    bench
        .firmware
        .borrow_mut()
        .drive_mut(PITCH)
        .reboot(Some(Instant::now() + window * 3));
    bench.pump(window * 2);
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    assert_eq!(bench.state(ROLL), JointHomingState::Verified);
    // The drive comes back without its zero; the grant stays revoked.
    bench.pump(window * 4);
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    assert_eq!(bench.state(ROLL), JointHomingState::Verified);
    assert!(!bench.supervisor.has_latched_fault());
}

#[test]
fn coordinate_discontinuity_after_grant_revokes() {
    let mut bench = Bench::physical("physical-discontinuity");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(30));
    assert_eq!(bench.state(PITCH), JointHomingState::Verified);
    // Firmware zero lost while reporting continues: the very next report jumps
    // 1.5 rad within about a millisecond, beyond the drive's velocity range.
    bench
        .firmware
        .borrow_mut()
        .drive_mut(PITCH)
        .zero_offset_motor_rad = 0.0;
    bench.pump_once();
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    // Continuous reporting at the new coordinate does not restore the grant.
    bench.pump(Duration::from_millis(20));
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    assert!(!bench.supervisor.has_latched_fault());
}

#[test]
fn estop_revokes_every_grant() {
    let mut bench = Bench::physical("physical-estop");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    bench.acquire(ROLL);
    bench.firmware.borrow_mut().clear_trace();
    bench.supervisor.set_hardware_estop(true);
    assert_ne!(bench.state(PITCH), JointHomingState::Verified);
    assert_ne!(bench.state(ROLL), JointHomingState::Verified);
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert!(bench.sent_any(CommunicationType::Disable) >= INITIAL.len());
}

#[test]
fn control_fault_revokes_grant() {
    let mut bench = Bench::physical("physical-fault");
    bench.acquire(PITCH);
    bench
        .supervisor
        .latch_control_fault("injected control fault", Some(PITCH));
    assert_eq!(bench.state(PITCH), JointHomingState::Faulted);
    assert!(bench
        .supervisor
        .enable_targets(&[PITCH.to_owned()])
        .is_err());
    assert_eq!(bench.sent(CommunicationType::Enable, PITCH), 1);
}

// ---------------------------------------------------------------- enable wire order

/// Reference `joint`, then Enable it while the controller still queues the
/// Enable frame: SocketCAN accepted the write but the drive has not seen it.
fn active_with_enable_queued(label: &str, joint: &str) -> Bench {
    let mut bench = Bench::physical(label);
    bench.acquire(joint);
    bench.pump(Duration::from_millis(20));
    bench.firmware.borrow_mut().hold_tx_from_enable = true;
    bench
        .supervisor
        .enable_targets(&[joint.to_owned()])
        .expect("identity admits the target");
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    assert!(
        !bench.firmware.borrow().drive(joint).enabled,
        "Enable is accepted but not yet on the wire"
    );
    bench
}

#[test]
fn reset_report_on_the_wire_before_enable_is_not_post_enable_evidence() {
    // Bench candump: roll's Reset type-24 is on the wire before the queued
    // Enable, but the host reads it only after declaring the session Active.
    let mut bench = active_with_enable_queued("physical-enable-before-wire", ROLL);
    bench.firmware.borrow_mut().emit_report(ROLL);
    bench
        .supervisor
        .drain_feedback()
        .expect("a report that predates the Enable on the wire is not a drive dropout");
    assert!(!bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    assert!(
        bench.supervisor.joint_feedback(ROLL).is_none(),
        "pre-Enable traffic cannot authorize the session"
    );

    bench.firmware.borrow_mut().emit_report(ROLL);
    bench.firmware.borrow_mut().release_tx();
    assert!(bench.firmware.borrow().drive(ROLL).enabled);
    bench
        .supervisor
        .drain_feedback()
        .expect("echo, then the drive's Run reply");
    assert!(!bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    assert!(
        bench.supervisor.joint_feedback(ROLL).is_some(),
        "the Run reply after the Enable echo is session pose"
    );
    bench.supervisor.disable_all().expect("stop");
}

#[test]
fn reset_report_after_the_enable_echo_still_faults() {
    let mut bench = active_with_enable_queued("physical-enable-ignored", ROLL);
    {
        let mut firmware = bench.firmware.borrow_mut();
        firmware.drive_mut(ROLL).ignore_enable = true;
        firmware.release_tx();
        firmware.emit_report(ROLL);
    }
    let error = bench
        .supervisor
        .drain_feedback()
        .expect_err("Reset after the Enable reached the drive is a dropout");
    assert!(
        matches!(&error, DavoutError::InvalidFeedback { joint, message }
            if joint == ROLL && message.contains("unexpected drive mode Reset")),
        "{error}"
    );
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    bench.assert_all_drives_stopped();
}

#[test]
fn missing_enable_echo_fails_closed_within_watchdog() {
    let mut bench = Bench::physical("physical-enable-echo-lost");
    bench.acquire(ROLL);
    bench.pump(Duration::from_millis(20));
    bench.firmware.borrow_mut().lost_echoes = vec![CommunicationType::Enable.as_u8()];
    bench
        .supervisor
        .enable_targets(&[ROLL.to_owned()])
        .expect("identity admits the target");
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    let give_up = Instant::now() + window * 3;
    let mut healthy_drains = 0;
    let error = loop {
        // The drive did enable and reports Run, but without the echo nothing
        // shows those reports followed the Enable on the wire.
        bench.firmware.borrow_mut().emit_report(ROLL);
        match bench.supervisor.drain_feedback() {
            Ok(_) => healthy_drains += 1,
            Err(error) => break error,
        }
        assert!(
            bench.supervisor.joint_feedback(ROLL).is_none(),
            "unconfirmed traffic cannot authorize the session"
        );
        assert!(
            Instant::now() < give_up,
            "a missing Enable echo must not leave the Run check unarmed"
        );
        std::thread::sleep(PUMP_PERIOD);
    };
    assert!(
        healthy_drains > 0,
        "the bound is the watchdog, not immediate"
    );
    assert!(
        matches!(&error, DavoutError::InvalidFeedback { joint, message }
            if joint == ROLL && message.contains("Enable not observed on the bus")),
        "{error}"
    );
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    bench.assert_all_drives_stopped();
}

#[test]
fn missing_enable_echo_refuses_reference_before_set_zero() {
    let mut bench = Bench::physical("physical-reference-echo-lost");
    bench.firmware.borrow_mut().lost_echoes = vec![CommunicationType::Enable.as_u8()];
    bench.assert_refused(PITCH, "TimedOut");
    assert_eq!(bench.sent(CommunicationType::Enable, PITCH), 1);
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 0);
    bench.assert_all_drives_stopped();
}

#[test]
fn own_frame_echoes_are_never_drive_feedback_or_liveness() {
    let mut bench = Bench::physical("physical-echo-not-feedback");
    bench.acquire(ROLL);
    bench.pump(Duration::from_millis(20));
    bench
        .supervisor
        .enable_targets(&[ROLL.to_owned()])
        .expect("identity admits the target");
    bench.firmware.borrow_mut().emit_report(ROLL);
    bench
        .supervisor
        .drain_feedback()
        .expect("post-enable Run report");
    let position_rad = bench
        .supervisor
        .joint_feedback(ROLL)
        .expect("session pose")
        .position_rad;
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    bench.firmware.borrow_mut().drive_mut(ROLL).silent_until = Some(Instant::now() + window * 10);
    let neutral = || davout::MitJointCommand {
        joint: ROLL.into(),
        kp: 0.0,
        kd: 0.0,
        position_rad,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.0,
    };
    let give_up = Instant::now() + window * 3;
    let error = loop {
        // Only the host's own MIT and reporting writes come back now.
        bench.supervisor.begin_tick_feedback();
        let tick = bench
            .supervisor
            .send_mit_batch(vec![neutral()])
            .and_then(|()| {
                bench.supervisor.tick_active_reporting_leases();
                bench.supervisor.drain_feedback()
            });
        if let Err(error) = tick {
            break error;
        }
        assert_eq!(
            bench.supervisor.last_refresh_frame_count(),
            0,
            "an echo is not a decoded drive frame"
        );
        assert!(Instant::now() < give_up, "echoes must not renew liveness");
        std::thread::sleep(PUMP_PERIOD);
    };
    // Either the session watchdog or the grant's silence bound fires first.
    assert!(
        matches!(&error, DavoutError::CommWatchdog { joint, .. } if joint == ROLL)
            || matches!(&error, DavoutError::Homing { message } if message.contains("permission")
                || message.contains("revoked")),
        "{error}"
    );
    assert_ne!(bench.state(ROLL), JointHomingState::Verified);
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    let _ = bench.supervisor.disable_all();
}

// ---------------------------------------------------------------- unsupported owners

#[test]
fn plain_owner_refuses_reference_without_transmitting() {
    let directory = TestDirectory::new("physical-plain-owner");
    let root = fixture_root(directory.path());
    let firmware = firmware_for(&root);
    let mut supervisor =
        Supervisor::from_repo(&root, FirmwareBus(firmware.clone())).expect("plain owner");
    firmware.borrow_mut().clear_trace();
    assert!(matches!(
        supervisor.calibrate_joint_zero(PITCH, OPERATOR, true),
        Err(DavoutError::ReferenceUnsupported { .. })
    ));
    assert!(matches!(
        supervisor.request_reference(
            PITCH,
            true,
            ReferenceAudit {
                operator: OPERATOR.into(),
                session: "plain".into(),
            },
        ),
        Err(ReferenceError::Unsupported)
    ));
    assert!(firmware.borrow().tx.is_empty());
    assert!(!directory.path().join("repo/var").exists());
    assert_eq!(
        supervisor.joint_homing_state(PITCH),
        JointHomingState::Unhomed
    );
}

#[test]
fn hall_and_none_methods_refuse_on_physical_owner() {
    for method in [HomingMethod::HallThreeSensor, HomingMethod::None] {
        let mut bench = Bench::physical("physical-method");
        let entry = bench
            .supervisor
            .homing_config
            .homing
            .joints
            .get_mut(PITCH)
            .expect("homing policy");
        entry.method = method;
        if method == HomingMethod::HallThreeSensor {
            let input = |gpio| marengo_config::SensorInput {
                gpio,
                active_high: true,
            };
            entry.overrides.sensors = Some(marengo_config::HomingSensors {
                home: input(10),
                min_limit: input(11),
                max_limit: input(12),
            });
        }
        assert!(
            matches!(
                bench.calibrate(PITCH),
                Err(DavoutError::ReferenceUnsupported { .. })
            ),
            "method {method:?}"
        );
        assert!(bench
            .supervisor
            .request_reference(
                PITCH,
                true,
                ReferenceAudit {
                    operator: OPERATOR.into(),
                    session: "method".into(),
                },
            )
            .is_err());
        assert!(bench.firmware.borrow().tx.is_empty(), "method {method:?}");
        assert!(!bench.supervisor.has_latched_fault());
        assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    }
}
