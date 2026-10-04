//! Type-24 reporting at process exit (`docs/safety.md`, *Reporting Off at
//! exit*). Robstride drives keep reporting across host processes and a Disable
//! does not end it. On 2026-10-04 a session ended on a Feedback fault with all
//! five drives streaming (the Disabled owner's reporting sync had turned them
//! On), and every later start latched Transport on its first bounded drain.
//! Physical owner against the test-only firmware emulator.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "support/mod.rs"]
mod support;

mod physical_firmware;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use davout::{
    ControlMode, DavoutError, ExitReportingOff, FaultClass, OperationalMode, ReferenceAudit,
    ReferenceOutcome, Supervisor, POST_SET_ZERO_BLACKOUT_FROM, POST_SET_ZERO_QUIET,
};
use marengo_config::{load_homing_config_from, load_motors_config_from};
use physical_firmware::{Firmware, FirmwareBus, SharedFirmware};
use robstride::CommunicationType;
use support::TestDirectory;

const PITCH: &str = "right_shoulder_pitch";
const ROLL: &str = "right_shoulder_roll";
const UPPER_YAW: &str = "right_upper_arm_yaw";
const ELBOW: &str = "right_elbow_pitch";
const LOWER_YAW: &str = "right_lower_arm_yaw";
const FIVE: [&str; 5] = [PITCH, ROLL, UPPER_YAW, ELBOW, LOWER_YAW];
const PERIOD: Duration = Duration::from_millis(5);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Master config (`active_reporting_diagnostics: true`) + URDF copy with a
/// short reference deadline.
fn fixture_root(directory: &Path) -> PathBuf {
    let source = repo_root();
    let root = directory.join("repo");
    let config = root.join("config");
    fs::create_dir_all(&config).expect("fixture config directory");
    for name in ["robot.yaml", "motors.yaml", "control.yaml"] {
        fs::copy(source.join("config").join(name), config.join(name)).expect("copy fixture");
    }
    let mut homing = load_homing_config_from(source.join("config")).expect("master homing");
    homing.homing.calibration_record_path = "var/calibration/zero_registry.yaml".into();
    homing.homing.defaults.search_timeout_s = 5.0;
    for entry in homing.homing.joints.values_mut() {
        entry.overrides.search_timeout_s = None;
    }
    fs::write(
        config.join("homing.yaml"),
        serde_yaml::to_string(&homing).expect("homing yaml"),
    )
    .expect("write fixture homing");
    let model = root.join("assets/urdf");
    fs::create_dir_all(&model).expect("fixture model directory");
    fs::copy(
        source.join("assets/urdf/marengo.urdf"),
        model.join("marengo.urdf"),
    )
    .expect("copy model");
    root
}

struct Bench {
    supervisor: Supervisor<FirmwareBus>,
    firmware: SharedFirmware,
    _directory: TestDirectory,
}

impl Bench {
    fn disabled(label: &str) -> Self {
        let directory = TestDirectory::in_memory(label);
        let root = fixture_root(directory.path());
        let motors = load_motors_config_from(root.join("config")).expect("fixture motors");
        let firmware = Firmware::from_motors(&motors.motors, &[]);
        let journal = directory.path().join("reference-journal.sqlite3");
        let supervisor = Supervisor::from_repo_with_physical_reference(
            &root,
            FirmwareBus(firmware.clone()),
            &journal,
        )
        .expect("physical owner");
        Self {
            supervisor,
            firmware,
            _directory: directory,
        }
    }

    /// Five joints referenced and Active in GravityComp at zero gains.
    fn active(label: &str) -> Self {
        let mut bench = Self::disabled(label);
        for joint in FIVE {
            bench.reference(joint);
        }
        let targets: Vec<String> = FIVE.iter().map(|joint| (*joint).to_owned()).collect();
        bench
            .supervisor
            .enable_targets(&targets)
            .expect("identity admits");
        let give_up = Instant::now() + Duration::from_secs(3);
        loop {
            assert!(Instant::now() < give_up, "the Enables complete");
            bench
                .tick()
                .unwrap_or_else(|error| panic!("bootstrap: {error}"));
            if !bench.supervisor.enable_writes_pending()
                && FIVE
                    .iter()
                    .all(|joint| bench.supervisor.joint_feedback(joint).is_some())
            {
                break;
            }
            std::thread::sleep(PERIOD);
        }
        bench.supervisor.set_control_mode(ControlMode::GravityComp);
        for _ in 0..5 {
            std::thread::sleep(PERIOD);
            bench.tick().expect("steady tick");
        }
        assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
        bench
    }

    fn reference(&mut self, joint: &str) {
        let handle = self
            .supervisor
            .request_reference(
                joint,
                true,
                ReferenceAudit {
                    operator: "bench-operator".into(),
                    session: "exit-reporting".into(),
                },
            )
            .expect("request admitted");
        let give_up = Instant::now() + Duration::from_secs(5);
        loop {
            self.firmware.borrow_mut().emit_reports();
            self.supervisor.sync_active_reporting();
            if self.supervisor.reference_work_pending() {
                self.supervisor
                    .advance_reference_work()
                    .expect("owner work advances");
            } else {
                let _ = self.supervisor.drain_feedback();
            }
            match self.supervisor.reference_outcome(&handle).expect("outcome") {
                ReferenceOutcome::Current { .. } => return,
                ReferenceOutcome::Failed { message } => panic!("{joint}: {message}"),
                ReferenceOutcome::InProgress => {
                    assert!(Instant::now() < give_up, "{joint}: reference finishes");
                    std::thread::sleep(PERIOD);
                }
            }
        }
    }

    /// One control tick in Berthier's order: drain, a zero-gain hold of the
    /// Active joints, drain.
    fn tick(&mut self) -> Result<(), DavoutError> {
        self.firmware.borrow_mut().emit_reports();
        self.supervisor.begin_tick_feedback();
        self.supervisor.drain_feedback()?;
        let batch: Vec<davout::MitJointCommand> = FIVE
            .iter()
            .filter(|joint| self.supervisor.active_joints().contains(**joint))
            .map(|joint| davout::MitJointCommand {
                joint: (*joint).to_owned(),
                kp: 0.0,
                kd: 0.0,
                position_rad: 0.0,
                velocity_rad_s: 0.0,
                torque_ff_nm: 0.0,
            })
            .collect();
        self.supervisor.send_mit_batch(batch)?;
        self.supervisor.drain_feedback().map(|_| ())
    }

    /// marengo-pi's Disabled loop iteration: reports, reporting sync, drain.
    fn pump(&mut self, duration: Duration) {
        let end = Instant::now() + duration;
        while Instant::now() < end {
            self.firmware.borrow_mut().emit_reports();
            self.supervisor.sync_active_reporting();
            let _ = self.supervisor.drain_feedback();
            std::thread::sleep(PERIOD);
        }
    }

    fn streaming(&self) -> Vec<String> {
        self.firmware
            .borrow()
            .drives
            .iter()
            .filter(|drive| drive.reporting)
            .map(|drive| drive.joint.clone())
            .collect()
    }

    fn device(&self, joint: &str) -> u8 {
        self.firmware.borrow().drive(joint).device_id
    }

    /// Type-24 writes to `joint` since `mark` (an index into the trace), with
    /// their acceptance instants.
    fn reporting_writes_since(&self, joint: &str, mark: usize) -> Vec<Instant> {
        let device = u32::from(self.device(joint));
        let firmware = self.firmware.borrow();
        firmware.tx[mark..]
            .iter()
            .zip(&firmware.tx_at[mark..])
            .filter(|(frame, _)| {
                frame.id & 0xff == device
                    && (frame.id >> 24) & 0x1f
                        == u32::from(CommunicationType::ActiveReporting.as_u8())
            })
            .map(|(_, at)| *at)
            .collect()
    }

    /// marengo-pi `finish_owner_shutdown`: mandatory reference cleanup, the
    /// exit stop, then the exit Offs.
    fn shutdown(&mut self) -> davout::ExitReportingReport {
        let _ = self.supervisor.cancel_reference_for_shutdown();
        let _ = self.supervisor.disable_all();
        self.supervisor
            .release_reporting_for_exit(Instant::now() + Duration::from_secs(5))
    }
}

/// The 2026-10-04 sequence: Feedback fault while Active, the tick-error stop,
/// the Disabled loop turning every stream On, then quit.
#[test]
fn a_fault_stop_then_shutdown_leaves_no_drive_reporting() {
    let mut bench = Bench::active("exit-reporting-fault");
    let scale = bench.firmware.borrow().drive(ELBOW).scale;
    let give_up = Instant::now() + Duration::from_secs(1);
    let error = loop {
        assert!(Instant::now() < give_up, "the elbow overspeed latches");
        // 0.03 rad per 5 ms: 6 rad/s against the 2 rad/s fault threshold.
        bench.firmware.borrow_mut().drive_mut(ELBOW).raw_motor_rad += 0.03 * scale;
        std::thread::sleep(PERIOD);
        if let Err(error) = bench.tick() {
            break error;
        }
    };
    assert!(error.to_string().contains("velocity"), "{error}");
    assert!(bench.supervisor.has_latched_fault());
    assert_eq!(
        bench.supervisor.safety_snapshot().faults[0].class,
        FaultClass::Feedback
    );
    // stop_after_tick_error, then the Disabled loop until the operator quits.
    let _ = bench.supervisor.disable_all();
    bench.pump(Duration::from_millis(100));
    assert_eq!(
        bench.streaming().len(),
        FIVE.len(),
        "the Disabled owner turned every stream On (diagnostics)"
    );

    let report = bench.shutdown();

    assert!(
        bench.streaming().is_empty(),
        "still streaming after exit: {:?}",
        bench.streaming()
    );
    assert!(report.all_sent(), "{report:?}");
    assert_eq!(report.drives.len(), FIVE.len());
    // Nothing this process does afterwards turns a stream back On.
    let mark = bench.firmware.borrow().tx.len();
    bench.pump(Duration::from_millis(30));
    for joint in FIVE {
        assert!(
            bench.reporting_writes_since(joint, mark).is_empty(),
            "{joint}"
        );
    }
    assert!(bench.streaming().is_empty());
}

/// A clean exit from Disabled leaves streams Off too; the shutdown stop's own
/// reporting sync no longer turns one On after the Offs.
#[test]
fn a_clean_disabled_exit_leaves_no_drive_reporting() {
    let mut bench = Bench::disabled("exit-reporting-clean");
    bench.pump(Duration::from_millis(60));
    assert_eq!(bench.streaming().len(), FIVE.len());

    let report = bench.shutdown();

    assert!(bench.streaming().is_empty(), "{:?}", bench.streaming());
    assert!(report.all_sent(), "{report:?}");
}

/// A drive in its possible post-SetZero blackout drops every frame: its Off
/// waits for the quiet's end. Peers are written at once.
#[test]
fn a_drive_in_its_post_set_zero_blackout_is_silenced_when_the_quiet_ends() {
    let mut bench = Bench::disabled("exit-reporting-blackout");
    bench.reference(PITCH);
    let set_zero_at = bench
        .firmware
        .borrow()
        .drive(PITCH)
        .set_zero_at
        .expect("pitch zeroed");
    let inside = set_zero_at + POST_SET_ZERO_BLACKOUT_FROM + Duration::from_millis(50);
    while Instant::now() < inside {
        bench.pump(PERIOD);
    }
    let _ = bench.supervisor.cancel_reference_for_shutdown();
    let _ = bench.supervisor.disable_all();
    let mark = bench.firmware.borrow().tx.len();
    let released_at = Instant::now();

    let report = bench
        .supervisor
        .release_reporting_for_exit(Instant::now() + Duration::from_secs(5));

    assert_eq!(report.drives.len(), FIVE.len());

    for (address, outcome) in &report.drives {
        assert_eq!(*outcome, ExitReportingOff::Sent, "{address:?}");
    }
    let pitch_writes = bench.reporting_writes_since(PITCH, mark);
    assert_eq!(pitch_writes.len(), 1, "one exit Off to pitch");
    assert!(
        pitch_writes[0] >= set_zero_at + POST_SET_ZERO_QUIET,
        "pitch Off {:?} after its SetZero, inside the quiet",
        pitch_writes[0] - set_zero_at
    );
    for joint in [ROLL, UPPER_YAW, ELBOW, LOWER_YAW] {
        let writes = bench.reporting_writes_since(joint, mark);
        assert_eq!(writes.len(), 1, "{joint}");
        assert!(
            writes[0] < set_zero_at + POST_SET_ZERO_QUIET,
            "{joint} not delayed"
        );
        assert!(writes[0] >= released_at);
    }
    assert!(bench.streaming().is_empty(), "{:?}", bench.streaming());
}

/// A quiet that ends after the deadline is reported, and nothing is written
/// into the blackout.
#[test]
fn a_quiet_past_the_deadline_is_reported_as_blackout() {
    let mut bench = Bench::disabled("exit-reporting-deadline");
    bench.reference(PITCH);
    let set_zero_at = bench
        .firmware
        .borrow()
        .drive(PITCH)
        .set_zero_at
        .expect("pitch zeroed");
    let inside = set_zero_at + POST_SET_ZERO_BLACKOUT_FROM + Duration::from_millis(50);
    while Instant::now() < inside {
        bench.pump(PERIOD);
    }
    let _ = bench.supervisor.cancel_reference_for_shutdown();
    let _ = bench.supervisor.disable_all();
    let mark = bench.firmware.borrow().tx.len();

    let report = bench.supervisor.release_reporting_for_exit(Instant::now());

    assert_eq!(report.drives.len(), FIVE.len());

    let pitch = bench.device(PITCH);
    for (address, outcome) in &report.drives {
        let expected = if address.device_id == pitch {
            ExitReportingOff::Blackout
        } else {
            ExitReportingOff::Sent
        };
        assert_eq!(*outcome, expected, "{address:?}");
    }
    assert!(!report.all_sent());
    assert!(bench.reporting_writes_since(PITCH, mark).is_empty());
}
