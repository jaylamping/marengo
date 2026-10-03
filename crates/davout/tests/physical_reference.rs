//! Qualified physical Robstride reference workflow (ADR 0036) through public
//! `Supervisor` APIs against a test-only firmware emulator. No hardware transport.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "support/mod.rs"]
mod support;

mod physical_firmware;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use davout::simulation::SimulationBus;
use davout::{
    DavoutError, JointHomingState, OperationalMode, ReferenceAudit, ReferenceError,
    ReferenceOutcome, Supervisor, BURST_GROUP_SPACING, IDENTITY_ADMISSION_SPACING,
    POST_SET_ZERO_BLACKOUT_FROM, POST_SET_ZERO_QUIET,
};
use marengo_config::{load_homing_config_from, load_motors_config_from, HomingMethod};
use physical_firmware::{
    Firmware, FirmwareBus, SharedFirmware, LATEST_SET_ZERO_BLACKOUT, SET_ZERO_BLACKOUT,
};
use robstride::{CommunicationType, DEFAULT_HOST_ID};
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
        let journal = directory.path().join("reference-journal.sqlite3");
        let firmware = firmware_for(&root);
        let supervisor = Supervisor::from_repo_with_physical_reference(
            &root,
            FirmwareBus(firmware.clone()),
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
        self.supervisor.sync_active_reporting();
        let _ = self.supervisor.drain_feedback();
    }

    /// Control-loop drains, one per control period, until `done` holds. On an
    /// echoing bus each staggered target's type-24 Off must be read back from
    /// the wire at least one control period before its Enable is written, and a
    /// target zeroed less than `POST_SET_ZERO_QUIET` ago waits for that quiet.
    fn drain_until(&mut self, done: impl Fn(&Self) -> bool) -> Result<(), DavoutError> {
        let window = Duration::from_millis(self.supervisor.control.control.comm_watchdog_ms);
        let give_up = Instant::now() + POST_SET_ZERO_QUIET + window;
        while !done(self) {
            assert!(
                Instant::now() < give_up,
                "staggered writes did not complete"
            );
            std::thread::sleep(PUMP_PERIOD);
            self.supervisor.drain_feedback()?;
        }
        Ok(())
    }

    /// Drain until every staggered Enable of this session is written.
    fn settle_enable(&mut self) -> Result<(), DavoutError> {
        self.drain_until(|bench| !bench.supervisor.enable_writes_pending())
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
    bench.settle_enable().expect("Off settles, then Enable");
    assert_eq!(bench.sent_any(CommunicationType::Enable), 1);
    assert_eq!(bench.sent(CommunicationType::Enable, PITCH), 1);
    bench.supervisor.disable_all().expect("stop");
}

#[test]
fn identity_request_transport_failure_latches_and_stops() {
    let mut bench = Bench::physical("physical-identity-send-error");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    bench.firmware.borrow_mut().clear_trace();
    bench
        .firmware
        .borrow_mut()
        .drive_mut(PITCH)
        .fail_writes
        .push(CommunicationType::GetDeviceId.as_u8());

    let error = bench
        .supervisor
        .enable_targets(&[PITCH.to_owned()])
        .expect_err("identity send failure refuses Enable");
    assert!(matches!(error, DavoutError::Bus(_)));
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert!(!bench.firmware.borrow().failed_tx.is_empty());
    bench.assert_all_drives_stopped();
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

/// ADR 0026:62-66 / audit gap 6: every exit of a reference after the target
/// was armed ends with the all-address stop, including a bus read error that
/// leaves `advance_reference` through a `?`.
#[test]
fn receive_error_after_arming_stops_every_drive_and_clears_the_reservation() {
    let mut bench = Bench::physical("physical-recv-error-after-arm");
    bench.firmware.borrow_mut().fail_rx_after_enable = true;
    let error = bench
        .calibrate(PITCH)
        .expect_err("a receive error must refuse the calibration");
    assert!(
        matches!(error, DavoutError::HomingVerify { .. }),
        "unexpected error {error}"
    );
    assert!(
        bench.sent(CommunicationType::Enable, PITCH) >= 1,
        "the target was armed before the read error"
    );
    assert!(
        !bench.supervisor.reference_work_pending(),
        "no armed reservation may survive the error exit"
    );
    bench.assert_all_drives_stopped();
    // Every configured drive got a Disable after the target's Enable.
    let firmware = bench.firmware.borrow();
    let last_enable = firmware
        .tx
        .iter()
        .rposition(|frame| (frame.id >> 24) & 0x1f == u32::from(CommunicationType::Enable.as_u8()))
        .expect("an Enable was transmitted");
    for drive in &firmware.drives {
        assert!(
            firmware.tx[last_enable..].iter().any(|frame| {
                (frame.id >> 24) & 0x1f == u32::from(CommunicationType::Disable.as_u8())
                    && (frame.id & 0xff) as u8 == drive.device_id
            }),
            "{} was not stopped after the armed Enable",
            drive.joint
        );
    }
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
    let joints: Vec<String> = bench
        .supervisor
        .motors
        .motors
        .iter()
        .map(|motor| motor.joint.clone())
        .collect();
    assert!(bench.supervisor.enable_targets(&joints).is_err());
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
    // Asked again while silent, never past the admission window.
    let requests = bench.sent(CommunicationType::GetDeviceId, PITCH);
    assert!((2..=10).contains(&requests), "{requests} identity requests");
    assert_eq!(bench.sent_any(CommunicationType::Enable), 0);
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
}

#[test]
fn identity_request_dropped_in_post_set_zero_blackout_is_asked_again() {
    // 2026-10-03 15:34:09 bench (candump-20261003T153408Z): right_shoulder_pitch
    // transmitted nothing from 1.1264 s to 1.1798 s, 534 ms after its SetZero,
    // as each drive did once after its own (45-61 ms over 124 measured
    // blackouts). Enable admission's type-0 reached it at 1.1756 s and was
    // never answered, so `enable` failed "device identity reply missing at
    // admission". Longest measured blackout, rounded up:
    const BLACKOUT: Duration = Duration::from_millis(61);
    let mut bench = Bench::physical("physical-enable-uid-blackout");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(20));
    let answers_from = Instant::now() + BLACKOUT;
    {
        let mut firmware = bench.firmware.borrow_mut();
        firmware.drive_mut(PITCH).silent_until = Some(answers_from);
        firmware.clear_trace();
    }
    bench
        .supervisor
        .enable_targets(&[PITCH.to_owned()])
        .expect("identity asked again after the blackout matches the grant");
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    {
        let device = bench.device(PITCH);
        let firmware = bench.firmware.borrow();
        let requested: Vec<Instant> = firmware
            .tx
            .iter()
            .zip(&firmware.tx_at)
            .filter(|(frame, _)| {
                (frame.id >> 24) & 0x1f == u32::from(CommunicationType::GetDeviceId.as_u8())
                    && (frame.id & 0xff) as u8 == device
            })
            .map(|(_, at)| *at)
            .collect();
        // The first request fell inside the blackout; a later one was answered.
        assert!(requested.len() >= 2, "{requested:?}");
        assert!(requested[0] < answers_from);
        assert!(requested[requested.len() - 1] >= answers_from);
    }
    bench.settle_enable().expect("Off settles, then Enable");
    assert_eq!(bench.sent(CommunicationType::Enable, PITCH), 1);
    assert_eq!(bench.state(PITCH), JointHomingState::Verified);
    bench.supervisor.disable_all().expect("stop");
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
    bench.firmware.borrow_mut().emit_report(PITCH);
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

/// The marengo-pi reference queue cancels its own bookkeeping on E-stop, but
/// Davout's in-flight transaction needs no help: `set_hardware_estop` aborts it
/// with the all-address stop in the same call (audit lead L-marengo-pi-21).
#[test]
fn estop_during_an_armed_reference_stops_the_transaction_itself() {
    let mut bench = Bench::physical("physical-estop-mid-reference");
    let handle = bench
        .supervisor
        .request_reference(
            PITCH,
            true,
            ReferenceAudit {
                operator: OPERATOR.into(),
                session: "estop-mid-reference".into(),
            },
        )
        .expect("physical request admitted");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !bench.firmware.borrow().drive(PITCH).enabled {
        assert!(Instant::now() < deadline, "target never armed");
        bench
            .supervisor
            .advance_reference_work()
            .expect("owner work advances");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(bench.supervisor.reference_work_pending());
    bench.firmware.borrow_mut().clear_trace();

    bench.supervisor.set_hardware_estop(true);

    assert!(
        !bench.supervisor.reference_busy(),
        "the in-flight transaction is over"
    );
    // The request's bookkeeping drains on the next owner work, not a transaction.
    bench
        .supervisor
        .advance_reference_work()
        .expect("owner work after the abort");
    assert!(!bench.supervisor.reference_work_pending());
    bench.assert_all_drives_stopped();
    assert!(bench.sent_any(CommunicationType::Disable) >= INITIAL.len());
    assert!(matches!(
        bench.supervisor.reference_outcome(&handle),
        Ok(ReferenceOutcome::Failed { .. })
    ));
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
    bench
        .settle_enable()
        .expect("the target's Off settles, then its Enable is written");
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
    bench.pump(POST_SET_ZERO_QUIET);
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

// ---------------------------------------------------------------- reporting Off before Enable

/// Every drive still streams type-24 reports left On by an earlier process,
/// which this owner has no record of, and a report the drive built before it
/// acted on an Enable follows that Enable on the wire.
fn inherited_streams(label: &str) -> Bench {
    let bench = Bench::physical(label);
    for drive in &mut bench.firmware.borrow_mut().drives {
        drive.reporting = true;
        drive.stale_report_after_enable = true;
    }
    bench
}

fn loop_period(bench: &Bench) -> Duration {
    Duration::from_micros(1_000_000 / u64::from(bench.supervisor.control.control.loop_hz))
}

/// Trace index and write time of the first `comm_type` frame to `joint` whose
/// payload matches `matches`.
fn first_write(
    bench: &Bench,
    comm_type: CommunicationType,
    joint: &str,
    matches: impl Fn(&[u8; 8]) -> bool,
) -> Option<(usize, Instant)> {
    let device = u32::from(bench.device(joint));
    let firmware = bench.firmware.borrow();
    firmware
        .tx
        .iter()
        .position(|frame| {
            (frame.id >> 24) & 0x1f == u32::from(comm_type.as_u8())
                && frame.id & 0xff == device
                && matches(&frame.data)
        })
        .map(|index| (index, firmware.tx_at[index]))
}

/// In the current trace `joint`'s type-24 Off precedes its Enable by at least
/// one control period (its echo is read in between).
fn assert_off_settled_before_enable(bench: &Bench, joint: &str) {
    let (off, off_at) = first_write(bench, CommunicationType::ActiveReporting, joint, |data| {
        data[6] == 0
    })
    .unwrap_or_else(|| panic!("{joint}: reporting Off written"));
    let (enable, enable_at) = first_write(bench, CommunicationType::Enable, joint, |_| true)
        .unwrap_or_else(|| panic!("{joint}: Enable written"));
    assert!(off < enable, "{joint}: stream silenced before Enable");
    assert!(
        enable_at.duration_since(off_at) >= loop_period(bench),
        "{joint}: Off on the wire a control period before Enable"
    );
}

#[test]
fn stream_left_on_by_an_earlier_process_is_off_before_each_target_enable() {
    // 2026-10-03 14:55:24 bench, `home <five right-arm joints> sign-tested`:
    // right_lower_arm_yaw failed "unexpected drive mode Reset for Disabled".
    // Drives keep type-24 reporting across host processes (14:51:33 candump:
    // all five streaming before marengo-pi started) and the paced sync had
    // applied only the first Ons, so the applied-only baseline Off left the
    // target streaming through ArmTarget. A report built before the drive
    // acted on Enable was read after the Enable's echo, still in Reset.
    let mut bench = inherited_streams("physical-inherited-stream");
    for (joint, _) in INITIAL {
        bench.acquire(joint);
        assert!(!bench.supervisor.has_latched_fault(), "{joint}");
        assert_off_settled_before_enable(&bench, joint);
        bench.firmware.borrow_mut().clear_trace();
        // One control-loop tick between joints, as in marengo-pi.
        bench.pump_once();
    }
    for (joint, _) in INITIAL {
        assert_eq!(bench.state(joint), JointHomingState::Verified, "{joint}");
    }
}

#[test]
fn target_left_in_reset_after_its_enable_still_faults_with_inherited_streams() {
    // The baseline Off removes only pre-Enable reports: the drive's own Reset
    // reply after the Enable's echo is a real dropout and still latches.
    let mut bench = inherited_streams("physical-inherited-stream-ignored");
    bench.firmware.borrow_mut().drive_mut(ROLL).ignore_enable = true;
    bench.assert_refused(ROLL, "unexpected drive mode Reset");
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 0);
    bench.assert_all_drives_stopped();
}

#[test]
fn missing_reporting_off_echo_refuses_reference_before_enable() {
    let mut bench = Bench::physical("physical-reference-off-echo-lost");
    bench.firmware.borrow_mut().lost_echoes = vec![CommunicationType::ActiveReporting.as_u8()];
    bench.assert_refused(PITCH, "TimedOut");
    assert!(first_write(
        &bench,
        CommunicationType::ActiveReporting,
        PITCH,
        |data| data[6] == 0
    )
    .is_some());
    assert_eq!(
        bench.sent_any(CommunicationType::Enable),
        0,
        "Enable withheld"
    );
    assert_eq!(bench.sent_any(CommunicationType::SetZeroPosition), 0);
    bench.assert_all_drives_stopped();
}

#[test]
fn inherited_stream_is_off_before_each_staggered_enable() {
    // The same race on the Active path: the 14:51:40 bench activation wrote the
    // Enables of right_elbow_pitch and right_lower_arm_yaw while their streams
    // still ran, because Offs paced in config order lagged the Enable order.
    let mut bench = inherited_streams("physical-inherited-stream-active");
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(30));
    bench.acquire(ROLL);
    bench.pump(POST_SET_ZERO_QUIET);
    for drive in &mut bench.firmware.borrow_mut().drives {
        drive.reporting = true;
    }
    bench.firmware.borrow_mut().clear_trace();
    enable_both(&mut bench);
    bench
        .settle_enable()
        .expect("no pre-Enable report follows an Enable's echo");
    assert!(!bench.supervisor.has_latched_fault());
    for joint in [PITCH, ROLL] {
        assert_off_settled_before_enable(&bench, joint);
        assert!(
            bench.supervisor.joint_feedback(joint).is_some(),
            "{joint}: the Run reply after its own echo is session pose"
        );
    }
    bench.supervisor.disable_all().expect("stop");
}

#[test]
fn missing_reporting_off_echo_withholds_enable_and_fails_closed() {
    let mut bench = Bench::physical("physical-enable-off-echo-lost");
    bench.acquire(ROLL);
    bench.pump(POST_SET_ZERO_QUIET);
    {
        let mut firmware = bench.firmware.borrow_mut();
        firmware.clear_trace();
        firmware.lost_echoes = vec![CommunicationType::ActiveReporting.as_u8()];
    }
    bench
        .supervisor
        .enable_targets(&[ROLL.to_owned()])
        .expect("identity admits the target");
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    let give_up = Instant::now() + window * 3;
    let mut healthy_drains = 0;
    let error = loop {
        bench.firmware.borrow_mut().emit_report(ROLL);
        match bench.supervisor.drain_feedback() {
            Ok(_) => healthy_drains += 1,
            Err(error) => break error,
        }
        assert!(bench.supervisor.joint_feedback(ROLL).is_none());
        assert!(
            Instant::now() < give_up,
            "a missing Off echo must fail closed"
        );
        std::thread::sleep(PUMP_PERIOD);
    };
    assert!(
        healthy_drains > 0,
        "the bound is the watchdog, not immediate"
    );
    assert!(first_write(
        &bench,
        CommunicationType::ActiveReporting,
        ROLL,
        |data| data[6] == 0
    )
    .is_some());
    assert_eq!(
        bench.sent(CommunicationType::Enable, ROLL),
        0,
        "Enable withheld"
    );
    assert!(
        matches!(&error, DavoutError::InvalidFeedback { joint, message }
            if joint == ROLL && message.contains("type-24 Off not observed on the bus")),
        "{error}"
    );
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    bench.assert_all_drives_stopped();
}

// ---------------------------------------------------------------- staggered enable

/// PITCH and ROLL referenced; both on can0, so their Enables are staggered.
/// Both post-SetZero quiets have elapsed, so only the stagger paces them.
/// The trace is cleared: counts below are this enable session's writes.
fn two_joint_bench(label: &str) -> Bench {
    let mut bench = Bench::physical(label);
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(30));
    bench.acquire(ROLL);
    bench.pump(POST_SET_ZERO_QUIET);
    bench.firmware.borrow_mut().clear_trace();
    bench
}

fn enable_both(bench: &mut Bench) {
    bench
        .supervisor
        .enable_targets(&[PITCH.to_owned(), ROLL.to_owned()])
        .expect("identity admits both targets");
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
}

#[test]
fn staggered_enable_writes_one_target_per_interface_per_control_period() {
    // 2026-10-03: Enable + RunMode to five drives in one call (plus every
    // drive's reply) overran the mcp251x two-frame receive buffer.
    let mut bench = two_joint_bench("physical-enable-stagger");
    enable_both(&mut bench);
    assert_eq!(
        bench.sent_any(CommunicationType::Enable),
        0,
        "no Enable before its target's Off has settled on the wire"
    );
    assert!(bench.supervisor.enable_writes_pending());
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    let give_up = Instant::now() + window;
    while bench.supervisor.enable_writes_pending() {
        assert!(Instant::now() < give_up, "the stagger completes");
        std::thread::sleep(PUMP_PERIOD);
        // A target not yet enabled is in Reset: neither a dropout nor pose.
        bench.firmware.borrow_mut().emit_report(PITCH);
        bench.firmware.borrow_mut().emit_report(ROLL);
        bench
            .supervisor
            .drain_feedback()
            .expect("a target not yet enabled may report Reset");
        for joint in [PITCH, ROLL] {
            if bench.sent(CommunicationType::Enable, joint) == 0 {
                assert!(bench.supervisor.joint_feedback(joint).is_none(), "{joint}");
            }
        }
    }
    assert!(!bench.supervisor.has_latched_fault());
    for joint in [PITCH, ROLL] {
        assert_off_settled_before_enable(&bench, joint);
        assert!(
            bench.supervisor.joint_feedback(joint).is_some(),
            "{joint}: the Run reply after its own echo is session pose"
        );
    }
    let firmware = bench.firmware.borrow();
    let enables: Vec<(u32, u8, Instant)> = firmware
        .tx
        .iter()
        .zip(&firmware.tx_at)
        .map(|(frame, at)| ((frame.id >> 24) & 0x1f, (frame.id & 0xff) as u8, *at))
        .filter(|(kind, _, _)| *kind == 3 || *kind == 18)
        .collect();
    let (pitch, roll) = (bench.device(PITCH), bench.device(ROLL));
    assert_eq!(
        enables
            .iter()
            .map(|(kind, device, _)| (*kind, *device))
            .collect::<Vec<_>>(),
        [(3, pitch), (18, pitch), (3, roll), (18, roll)],
        "Enable then RunMode per target"
    );
    assert!(
        enables[2].2.duration_since(enables[0].2) >= loop_period(&bench),
        "one can0 target per control period"
    );
    drop(firmware);
    bench.supervisor.disable_all().expect("stop");
}

#[test]
fn staggered_target_reset_after_its_own_echo_still_faults() {
    let mut bench = two_joint_bench("physical-stagger-ignored");
    bench.firmware.borrow_mut().drive_mut(ROLL).ignore_enable = true;
    enable_both(&mut bench);
    let error = bench
        .settle_enable()
        .expect_err("Reset after ROLL's Enable reached the wire is a dropout");
    assert_eq!(bench.sent(CommunicationType::Enable, ROLL), 1);
    assert!(
        matches!(&error, DavoutError::InvalidFeedback { joint, message }
            if joint == ROLL && message.contains("unexpected drive mode Reset")),
        "{error}"
    );
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    assert!(!bench.supervisor.enable_writes_pending());
    bench.assert_all_drives_stopped();
}

#[test]
fn stale_enable_echo_cannot_arm_a_target_not_yet_written() {
    // An older echo of ROLL's Enable (here, the exact frame), read after this
    // session activated but before ROLL's own staggered Enable is written,
    // must not hold ROLL's Reset traffic to Run or admit it as pose.
    for _attempt in 0..5 {
        let mut bench = two_joint_bench("physical-stagger-stale-echo");
        enable_both(&mut bench);
        bench
            .drain_until(|bench| bench.sent(CommunicationType::Enable, PITCH) > 0)
            .expect("PITCH's Enable");
        if bench.sent(CommunicationType::Enable, ROLL) > 0 {
            continue;
        }
        let roll = bench.device(ROLL);
        let stale_echo = {
            let firmware = bench.firmware.borrow();
            let pitch_enable = firmware
                .tx
                .iter()
                .find(|frame| {
                    (frame.id >> 24) & 0x1f == u32::from(CommunicationType::Enable.as_u8())
                })
                .expect("PITCH's Enable");
            robstride::CanFrame {
                id: (pitch_enable.id & !0xff) | u32::from(roll),
                ..pitch_enable.clone()
            }
        };
        {
            let mut firmware = bench.firmware.borrow_mut();
            firmware.inject_rx(stale_echo);
            firmware.emit_report(ROLL);
        }
        let drained = bench.supervisor.drain_feedback();
        if bench.sent(CommunicationType::Enable, ROLL) > 0 {
            // The drain crossed into ROLL's period and wrote its own Enable
            // first; retry inside one control period.
            continue;
        }
        drained.expect("ROLL's Reset precedes its own Enable on the wire");
        assert!(!bench.supervisor.has_latched_fault());
        assert!(bench.supervisor.joint_feedback(ROLL).is_none());
        assert!(bench.supervisor.enable_writes_pending());
        bench.supervisor.disable_all().expect("stop");
        return;
    }
    panic!("no drain ran inside one control period");
}

#[test]
fn missing_echo_of_a_staggered_enable_fails_closed() {
    let mut bench = two_joint_bench("physical-stagger-echo-lost");
    bench.firmware.borrow_mut().lost_echoes = vec![CommunicationType::Enable.as_u8()];
    enable_both(&mut bench);
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    let give_up = Instant::now() + window * 3;
    let error = loop {
        bench.firmware.borrow_mut().emit_reports();
        if let Err(error) = bench.supervisor.drain_feedback() {
            break error;
        }
        for joint in [PITCH, ROLL] {
            assert!(
                bench.supervisor.joint_feedback(joint).is_none(),
                "unconfirmed traffic cannot authorize {joint}"
            );
        }
        assert!(Instant::now() < give_up, "missing echoes must fail closed");
        std::thread::sleep(PUMP_PERIOD);
    };
    assert_eq!(
        bench.sent(CommunicationType::Enable, ROLL),
        1,
        "the stagger completed before the echo bound"
    );
    assert!(
        matches!(&error, DavoutError::InvalidFeedback { message, .. }
            if message.contains("Enable not observed on the bus")),
        "{error}"
    );
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
    bench.assert_all_drives_stopped();
}

// ---------------------------------------------------------------- post-SetZero quiet

/// Write time of `joint`'s SetZero in the current trace.
fn set_zero_written_at(bench: &Bench, joint: &str) -> Instant {
    first_write(bench, CommunicationType::SetZeroPosition, joint, |_| true)
        .unwrap_or_else(|| panic!("{joint}: SetZero written"))
        .1
}

/// Every frame written to `joint` (outbound low byte) with its write time.
fn writes_to(bench: &Bench, joint: &str) -> Vec<(u32, Instant)> {
    let device = u32::from(bench.device(joint));
    let firmware = bench.firmware.borrow();
    firmware
        .tx
        .iter()
        .zip(&firmware.tx_at)
        .filter(|(frame, _)| {
            frame.id & 0xff == device && (frame.id >> 8) & 0xff == u32::from(DEFAULT_HOST_ID)
        })
        .map(|(frame, at)| ((frame.id >> 24) & 0x1f, *at))
        .collect()
}

/// PITCH, then ROLL (the last reference) referenced; `enable_targets` is
/// called 530 ms after ROLL's SetZero, before ROLL's blackout, which is the
/// latest modeled one ([`LATEST_SET_ZERO_BLACKOUT`], 625-690 ms; bench worst
/// case: right_elbow_pitch silent 614-667 ms after its SetZero on 2026-10-03),
/// with PITCH's blackout already over. Unheld, ROLL's Off and Enable would
/// follow within a few control periods, and an Enable held only 650 ms would
/// land inside ROLL's blackout, where the drive never acts on it. Returns each
/// joint's SetZero write time; the trace then holds only this enable session's
/// writes.
fn enable_just_before_last_blackout(label: &str) -> (Bench, [(&'static str, Instant); 2]) {
    let mut bench = Bench::physical(label);
    bench
        .firmware
        .borrow_mut()
        .drive_mut(ROLL)
        .set_zero_blackout = LATEST_SET_ZERO_BLACKOUT;
    bench.acquire(PITCH);
    bench.pump(Duration::from_millis(100));
    bench.acquire(ROLL);
    let zeroed = [
        (PITCH, set_zero_written_at(&bench, PITCH)),
        (ROLL, set_zero_written_at(&bench, ROLL)),
    ];
    let call_at = zeroed[1].1 + Duration::from_millis(530);
    bench.pump(
        call_at
            .saturating_duration_since(Instant::now())
            .saturating_sub(PUMP_PERIOD),
    );
    std::thread::sleep(call_at.saturating_duration_since(Instant::now()));
    bench.firmware.borrow_mut().clear_trace();
    enable_both(&mut bench);
    (bench, zeroed)
}

/// Control-loop drains until every held Enable is written, then one more with
/// a status from each target. Status is solicited every period (the control
/// loop's neutral MIT); a blacked-out drive sends none.
fn drain_held_enables(bench: &mut Bench, roll_zeroed: Instant) -> Result<(), DavoutError> {
    let give_up = roll_zeroed
        + POST_SET_ZERO_QUIET
        + Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    loop {
        let pending = bench.supervisor.enable_writes_pending();
        assert!(Instant::now() < give_up, "the held Enables complete");
        std::thread::sleep(PUMP_PERIOD);
        bench.firmware.borrow_mut().emit_report(PITCH);
        bench.firmware.borrow_mut().emit_report(ROLL);
        bench.supervisor.drain_feedback()?;
        if !pending {
            return Ok(());
        }
    }
}

#[test]
fn enable_right_after_the_last_reference_is_held_past_the_set_zero_blackout() {
    // Bench candumps: each drive went silent for 45-61 ms 511-614 ms after its
    // SetZero; an Enable written then was never acted on, the drive stayed in
    // Reset and the session latched DriveState (rev 15542aa).
    let (mut bench, zeroed) = enable_just_before_last_blackout("physical-quiet-held");
    assert!(bench.supervisor.enable_writes_pending());
    assert_eq!(
        bench.sent(CommunicationType::Enable, ROLL),
        0,
        "ROLL's Enable is held, not refused"
    );
    drain_held_enables(&mut bench, zeroed[1].1).expect("held Enables are acted on");
    assert!(!bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    for (joint, set_zero_at) in zeroed {
        let (_, enabled_at) = first_write(&bench, CommunicationType::Enable, joint, |_| true)
            .unwrap_or_else(|| panic!("{joint}: Enable written"));
        assert!(
            enabled_at.duration_since(set_zero_at) >= POST_SET_ZERO_QUIET,
            "{joint}: Enable held for the post-SetZero quiet"
        );
        assert!(
            bench.firmware.borrow().drive(joint).enabled,
            "{joint} in Run"
        );
        assert!(
            bench.supervisor.joint_feedback(joint).is_some(),
            "{joint}: Run report after its own echo is session pose"
        );
        assert_off_settled_before_enable(&bench, joint);
    }
    // Neither ROLL's Off nor its Enable was written inside any modeled blackout.
    let (blackout_start, blackout_end) = SET_ZERO_BLACKOUT;
    for (comm_type, at) in writes_to(&bench, ROLL) {
        if comm_type == u32::from(CommunicationType::Enable.as_u8())
            || comm_type == u32::from(CommunicationType::ActiveReporting.as_u8())
        {
            let since = at.saturating_duration_since(zeroed[1].1);
            assert!(
                !(blackout_start..=blackout_end).contains(&since),
                "type {comm_type} written {since:?} after SetZero"
            );
        }
    }
    bench.supervisor.disable_all().expect("stop");
}

#[test]
fn reset_after_a_held_enable_reaches_the_wire_still_faults() {
    let (mut bench, zeroed) = enable_just_before_last_blackout("physical-quiet-ignored");
    bench.firmware.borrow_mut().drive_mut(ROLL).ignore_enable = true;
    let error = drain_held_enables(&mut bench, zeroed[1].1)
        .expect_err("Reset after ROLL's held Enable reached the wire is a dropout");
    assert!(
        matches!(&error, DavoutError::InvalidFeedback { joint, message }
            if joint == ROLL && message.contains("unexpected drive mode Reset")),
        "{error}"
    );
    assert_eq!(bench.sent(CommunicationType::Enable, ROLL), 1);
    let (_, enabled_at) =
        first_write(&bench, CommunicationType::Enable, ROLL, |_| true).expect("ROLL's Enable");
    assert!(enabled_at.duration_since(zeroed[1].1) >= POST_SET_ZERO_QUIET);
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Disabled);
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
    bench.settle_enable().expect("Off settles, then Enable");
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
    // Every drive goes silent: peers would otherwise answer reporting writes.
    for drive in &mut bench.firmware.borrow_mut().drives {
        drive.silent_until = Some(Instant::now() + window * 10);
    }
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
                bench.supervisor.sync_active_reporting();
                bench.supervisor.drain_feedback()
            });
        if let Err(error) = tick {
            break error;
        }
        assert_eq!(
            tick.expect("successful echo-only drain"),
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

// ---------------------------------------------------------------- host-caused silence
//
// 2026-10-03 `pi_enable_soak` at e6add09: 14 of 20 cycles failed `home` with
// "no private current-reference permission" right after the five references,
// and one lost a grant mid-enable. Streams silenced by the host's own Offs and
// restarted into a drive's post-SetZero blackout, or a first Run reply read a
// tick after its Enable echo, were counted as drive silence.

const FIVE: [&str; 5] = [
    PITCH,
    ROLL,
    "right_upper_arm_yaw",
    "right_elbow_pitch",
    "right_lower_arm_yaw",
];

fn millis(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

impl Bench {
    /// One `marengo-pi` loop iteration: reports on the wire, the reporting
    /// sync, then owner work while a reference is pending and a plain drain
    /// otherwise.
    fn runtime_tick(&mut self) {
        self.firmware.borrow_mut().emit_reports();
        self.supervisor.sync_active_reporting();
        if self.supervisor.reference_work_pending() {
            self.supervisor
                .advance_reference_work()
                .expect("owner work advances");
        } else {
            let _ = self.supervisor.drain_feedback();
        }
    }

    /// `home <joints> sign-tested` as the stdin queue runs it: every reference
    /// starts as the previous one ends, on 5 ms ticks. Returns when the last
    /// one is current.
    fn home_in_sequence(&mut self, joints: &[&str]) -> Instant {
        for joint in joints {
            let handle = self
                .supervisor
                .request_reference(
                    joint,
                    true,
                    ReferenceAudit {
                        operator: OPERATOR.into(),
                        session: "physical-liveness".into(),
                    },
                )
                .expect("physical request admitted");
            let give_up = Instant::now() + Duration::from_secs(5);
            loop {
                self.runtime_tick();
                match self.supervisor.reference_outcome(&handle).expect("outcome") {
                    ReferenceOutcome::Current { .. } => break,
                    ReferenceOutcome::Failed { message } => panic!("{joint}: {message}"),
                    ReferenceOutcome::InProgress => {
                        assert!(Instant::now() < give_up, "{joint}: reference finishes");
                        std::thread::sleep(PUMP_PERIOD);
                    }
                }
            }
        }
        Instant::now()
    }

    fn all_verified(&self) -> Option<&'static str> {
        FIVE.into_iter()
            .find(|joint| self.state(joint) != JointHomingState::Verified)
    }
}

/// Type-24 writes to `joint` with their offset after its SetZero write.
fn reporting_writes_since_set_zero(bench: &Bench, joint: &str) -> Vec<Duration> {
    let set_zero = set_zero_written_at(bench, joint);
    writes_to(bench, joint)
        .into_iter()
        .filter(|(comm_type, _)| {
            *comm_type == u32::from(CommunicationType::ActiveReporting.as_u8())
        })
        .map(|(_, at)| at.saturating_duration_since(set_zero))
        .filter(|since| *since > Duration::ZERO)
        .collect()
}

#[test]
fn every_grant_is_valid_at_every_offset_after_the_last_reference() {
    // The pitch blackout sweeps the measured start range at the longest
    // modeled length: the drive drops whatever it is sent for up to 65 ms. The
    // host's Off for the next reference and the On after it used to land on
    // opposite sides of that blackout, leaving pitch's stream Off until the
    // 200 ms stale retry, past the 100 ms grant liveness bound.
    for start in [500, 530, 560, 590, 620] {
        let mut bench = Bench::physical(&format!("physical-grant-offsets-{start}"));
        {
            let mut firmware = bench.firmware.borrow_mut();
            firmware.drive_mut(PITCH).set_zero_blackout = (millis(start), millis(65));
        }
        let last = bench.home_in_sequence(&FIVE);
        // `home` can arrive at any offset (the soak's feeder: 0.1-0.25 s).
        while last.elapsed() <= millis(1000) {
            bench.runtime_tick();
            let offset = last.elapsed();
            assert_eq!(
                bench.all_verified(),
                None,
                "pitch blackout from {start} ms: a grant lapsed {offset:?} after the last reference"
            );
            std::thread::sleep(PUMP_PERIOD);
        }
        assert!(!bench.supervisor.has_latched_fault());
        // No type-24 write reached any drive inside a blackout the emulator can
        // model (500-690 ms after its SetZero write).
        let (blackout_start, blackout_end) = SET_ZERO_BLACKOUT;
        for joint in FIVE {
            for since in reporting_writes_since_set_zero(&bench, joint) {
                assert!(
                    !(blackout_start..=blackout_end).contains(&since),
                    "{joint}: type-24 written {since:?} after its SetZero"
                );
            }
        }
    }
}

#[test]
fn five_references_do_not_overrun_the_receive_buffers() {
    // 2026-10-03 17:09:07: the all-address stop that finishes a reference wrote
    // 15 frames back to back; their replies overran the mcp251x (rx_over_errors
    // 5 to 6) and the kernel's error frame latched Transport.
    let mut bench = Bench::physical("physical-references-no-overrun");
    bench.firmware.borrow_mut().rx_fifo.enforce = true;
    bench.home_in_sequence(&FIVE);
    bench.pump(millis(300));
    assert_eq!(bench.firmware.borrow().rx_fifo.overruns, 0);
    assert!(!bench.supervisor.has_latched_fault());
    assert_eq!(bench.all_verified(), None);
}

#[test]
fn reference_write_bursts_are_spaced_on_the_bus() {
    let mut bench = Bench::physical("physical-burst-spacing");
    bench.home_in_sequence(&[PITCH, ROLL, "right_upper_arm_yaw"]);
    let firmware = bench.firmware.borrow();
    // Each address's stop starts a group (speed zero is its first frame), and
    // each peer's type-24 Off is one more.
    let mut previous: Option<(Instant, u32)> = None;
    for (frame, at) in firmware.tx.iter().zip(&firmware.tx_at) {
        let comm_type = (frame.id >> 24) & 0x1f;
        let group_start = comm_type == u32::from(CommunicationType::WriteParameter.as_u8())
            || (comm_type == u32::from(CommunicationType::ActiveReporting.as_u8())
                && frame.data[6] == 0);
        if !group_start {
            continue;
        }
        if let Some((last_at, last_device)) = previous {
            if last_device != frame.id & 0xff {
                assert!(
                    at.duration_since(last_at) >= Duration::from_micros(1900),
                    "group starts {:?} apart",
                    at.duration_since(last_at)
                );
            }
        }
        previous = Some((*at, frame.id & 0xff));
    }
    assert_eq!(firmware.rx_fifo.overruns, 0);
}

#[test]
fn an_unpaced_stop_burst_overruns_the_receive_model() {
    // The model must bite: fifteen stop frames and their replies, back to back,
    // are what overran the bench controller (rx_over_errors 5 to 6, 17:09:07).
    let mut bench = Bench::physical("physical-unpaced-stop-overrun");
    bench.firmware.borrow_mut().rx_fifo.enforce = true;
    bench.supervisor.disable_all().expect("stop");
    assert!(bench.firmware.borrow().rx_fifo.overruns > 0);
    let _ = bench.supervisor.drain_feedback();
    assert!(
        bench.supervisor.has_latched_fault(),
        "the overflow error frame latches Transport"
    );
}

#[test]
fn a_streaming_drive_that_goes_silent_beyond_the_bound_still_revokes() {
    // Stream applied On: its silence is the drive's, before and inside the
    // window where the host holds type-24 writes back.
    for pump_before in [millis(20), POST_SET_ZERO_BLACKOUT_FROM + millis(50)] {
        let mut bench = Bench::physical("physical-silent-stream");
        bench.acquire(PITCH);
        let zeroed = set_zero_written_at(&bench, PITCH);
        bench.pump(pump_before.saturating_sub(zeroed.elapsed()));
        let window = millis(bench.supervisor.control.control.comm_watchdog_ms);
        assert_eq!(bench.state(PITCH), JointHomingState::Verified);
        bench.firmware.borrow_mut().drive_mut(PITCH).silent_until =
            Some(Instant::now() + window * 4);
        bench.pump(window * 2);
        assert_eq!(
            bench.state(PITCH),
            JointHomingState::Unhomed,
            "silent {:?} after its SetZero",
            zeroed.elapsed()
        );
        assert!(!bench.supervisor.has_latched_fault());
    }
}

#[test]
fn a_drive_that_died_while_the_host_held_its_stream_off_loses_its_grant() {
    // PITCH dies right after its reference. The host's Offs silence its stream
    // and, with the next On due inside the blackout window, hold it back until
    // the quiet ends; that silence is excused, but only until the quiet ends
    // plus the liveness bound.
    let mut bench = Bench::physical("physical-dead-while-held");
    bench.home_in_sequence(&[PITCH]);
    let zeroed = set_zero_written_at(&bench, PITCH);
    bench.firmware.borrow_mut().drive_mut(PITCH).silent_until =
        Some(Instant::now() + Duration::from_secs(60));
    bench.home_in_sequence(&FIVE[1..]);
    let window = millis(bench.supervisor.control.control.comm_watchdog_ms);
    let revoked_by = zeroed + POST_SET_ZERO_QUIET + window + millis(50);
    while Instant::now() < revoked_by {
        bench.runtime_tick();
        std::thread::sleep(PUMP_PERIOD);
    }
    bench.runtime_tick();
    assert_eq!(bench.state(PITCH), JointHomingState::Unhomed);
    for joint in &FIVE[1..] {
        assert_eq!(bench.state(joint), JointHomingState::Verified, "{joint}");
    }
}

#[test]
fn a_grant_survives_the_gap_between_the_enable_echo_and_the_first_run_reply() {
    // Enable-to-Run reply latency is 1.4-5.2 ms (behaviour doc). The write tick
    // reads the echo, the next tick the reply; the address's traffic was
    // withheld from pose until the echo, so its last pose is as old as the
    // session. PITCH's Enable is held to its post-SetZero quiet, 0.66 s after
    // activation, and liveness counted that age the moment the echo was read.
    // (Two joints only: PITCH's stream is never Off across its blackout here.)
    let joints = [PITCH, ROLL];
    for delay in [millis(2), millis(4)] {
        let mut bench = Bench::physical("physical-enable-reply-gap");
        for drive in &mut bench.firmware.borrow_mut().drives {
            drive.enable_reply_delay = delay;
        }
        bench.home_in_sequence(&joints);
        let targets: Vec<String> = joints.iter().map(|joint| (*joint).to_owned()).collect();
        bench
            .supervisor
            .enable_targets(&targets)
            .expect("identity admits");
        let give_up = Instant::now() + POST_SET_ZERO_QUIET + millis(500);
        loop {
            assert!(
                Instant::now() < give_up,
                "reply {delay:?}: the Enables complete"
            );
            bench.firmware.borrow_mut().emit_reports();
            // The loop's neutral status solicit.
            let neutral = joints
                .iter()
                .map(|joint| davout::MitJointCommand {
                    joint: (*joint).to_owned(),
                    kp: 0.0,
                    kd: 0.0,
                    position_rad: 0.0,
                    velocity_rad_s: 0.0,
                    torque_ff_nm: 0.0,
                })
                .collect();
            bench
                .supervisor
                .send_mit_batch(neutral)
                .unwrap_or_else(|error| panic!("reply {delay:?}: {error}"));
            bench
                .supervisor
                .drain_feedback()
                .unwrap_or_else(|error| panic!("reply {delay:?}: {error}"));
            if !bench.supervisor.enable_writes_pending()
                && joints
                    .iter()
                    .all(|joint| bench.supervisor.joint_feedback(joint).is_some())
            {
                break;
            }
            std::thread::sleep(PUMP_PERIOD);
        }
        assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
        for joint in joints {
            assert_eq!(
                bench.state(joint),
                JointHomingState::Verified,
                "reply {delay:?}: {joint}"
            );
        }
        bench.supervisor.disable_all().expect("stop");
    }
}

#[test]
fn identity_admission_requests_leave_one_spacing_apart() {
    let mut bench = Bench::physical("physical-admission-spacing");
    bench.home_in_sequence(&FIVE);
    bench.pump(POST_SET_ZERO_QUIET);
    bench.firmware.borrow_mut().clear_trace();
    let all: Vec<String> = FIVE.iter().map(|joint| (*joint).to_owned()).collect();
    bench
        .supervisor
        .enable_targets(&all)
        .expect("identity admits");
    let firmware = bench.firmware.borrow();
    let requests: Vec<Instant> = firmware
        .tx
        .iter()
        .zip(&firmware.tx_at)
        .filter(|(frame, _)| {
            (frame.id >> 24) & 0x1f == u32::from(CommunicationType::GetDeviceId.as_u8())
        })
        .map(|(_, at)| *at)
        .collect();
    assert_eq!(requests.len(), FIVE.len(), "one request per target");
    for pair in requests.windows(2) {
        assert!(
            pair[1].duration_since(pair[0]) >= IDENTITY_ADMISSION_SPACING,
            "admission requests {:?} apart",
            pair[1].duration_since(pair[0])
        );
    }
    drop(firmware);
    bench.supervisor.disable_all().expect("stop");
}

#[test]
fn status_solicit_disables_leave_one_spacing_apart() {
    let mut bench = Bench::physical("physical-solicit-spacing");
    bench
        .supervisor
        .control
        .control
        .bench
        .active_reporting_diagnostics = false;
    bench.firmware.borrow_mut().clear_trace();
    bench.supervisor.solicit_status_feedback().expect("solicit");
    let firmware = bench.firmware.borrow();
    let disables: Vec<Instant> = firmware
        .tx
        .iter()
        .zip(&firmware.tx_at)
        .filter(|(frame, _)| {
            (frame.id >> 24) & 0x1f == u32::from(CommunicationType::Disable.as_u8())
        })
        .map(|(_, at)| *at)
        .collect();
    assert_eq!(disables.len(), FIVE.len());
    for pair in disables.windows(2) {
        assert!(pair[1].duration_since(pair[0]) >= BURST_GROUP_SPACING);
    }
}
