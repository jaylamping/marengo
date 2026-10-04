//! Single-drive loss while Active (ADR 0038): subtree shed, degraded episode,
//! Davout deadline. Physical owner against the test-only firmware emulator.
#![allow(clippy::expect_used, clippy::panic)]

#[path = "support/mod.rs"]
mod support;

mod physical_firmware;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use davout::{
    ControlMode, DavoutError, DegradedEnd, DriveLossPlan, FaultClass, OperationalMode,
    ReferenceAudit, ReferenceOutcome, Supervisor, SHED_REPLY_GRACE,
};
use marengo_config::{
    load_control_config_from, load_homing_config_from, load_motors_config_from,
    write_control_config_from, DriveLossConfig, OnDriveLoss,
};
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
/// Scheduling slack of a wall-clock bound check: a few loop periods.
const SLACK: Duration = Duration::from_millis(40);
/// Fixture episode timing: short so the tests run in seconds.
const HOLD_WINDOW_S: f64 = 0.3;
const LOWER_SETTLE_S: f64 = 0.2;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Master config + URDF copy with short reference deadlines, short episode
/// timing, and `shed_subtree` on the elbow and the lower-arm yaw only.
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
    let mut control = load_control_config_from(&config).expect("copied control");
    for (joint, entry) in control.control.joints.iter_mut() {
        entry.on_drive_loss = if joint == ELBOW || joint == LOWER_YAW {
            OnDriveLoss::ShedSubtree
        } else {
            OnDriveLoss::DisableAll
        };
    }
    control.control.drive_loss = Some(DriveLossConfig {
        hold_window_s: HOLD_WINDOW_S,
        lower_velocity_rad_s: 0.25,
        lower_settle_s: LOWER_SETTLE_S,
        lower_max_s: 1.0,
        tau_margin_nm: 0.5,
    });
    write_control_config_from(&config, &control).expect("write fixture control");
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
    /// Five joints referenced and Active in GravityComp, with the elbow and
    /// lower-arm yaw plans installed as the controller would.
    fn active(label: &str) -> Self {
        let directory = TestDirectory::new(label);
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
        let mut bench = Self {
            supervisor,
            firmware,
            _directory: directory,
        };
        bench
            .supervisor
            .install_drive_loss_plans(vec![
                DriveLossPlan {
                    joint: ELBOW.into(),
                    shed: vec![ELBOW.into(), LOWER_YAW.into()],
                },
                DriveLossPlan {
                    joint: LOWER_YAW.into(),
                    shed: vec![LOWER_YAW.into()],
                },
            ])
            .expect("admitted plans");
        bench.home_all();
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

    fn home_all(&mut self) {
        for joint in FIVE {
            let handle = self
                .supervisor
                .request_reference(
                    joint,
                    true,
                    ReferenceAudit {
                        operator: "bench-operator".into(),
                        session: "drive-loss".into(),
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
                    ReferenceOutcome::Current { .. } => break,
                    ReferenceOutcome::Failed { message } => panic!("{joint}: {message}"),
                    ReferenceOutcome::InProgress => {
                        assert!(Instant::now() < give_up, "{joint}: reference finishes");
                        std::thread::sleep(PERIOD);
                    }
                }
            }
        }
    }

    /// One control tick in Berthier's order: drain, a zero-gain hold of the
    /// joints Active after that drain, drain.
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

    fn device(&self, joint: &str) -> u8 {
        self.firmware.borrow().drive(joint).device_id
    }

    fn sent(&self, comm_type: CommunicationType, joint: &str) -> usize {
        let device = self.device(joint);
        self.firmware.borrow().sent(comm_type, device)
    }

    /// Every frame type written to `joint` since `mark` (an index into the trace).
    fn written_since(&self, joint: &str, mark: usize) -> Vec<u8> {
        let device = u32::from(self.device(joint));
        self.firmware.borrow().tx[mark..]
            .iter()
            .filter(|frame| frame.id & 0xff == device)
            .map(|frame| ((frame.id >> 24) & 0x1f) as u8)
            .collect()
    }

    fn trace_len(&self) -> usize {
        self.firmware.borrow().tx.len()
    }

    /// `joint`'s drive goes silent for good; tick until Davout answers.
    fn lose(&mut self, joint: &str) -> Result<(), DavoutError> {
        let window = Duration::from_millis(self.supervisor.control.control.comm_watchdog_ms);
        let lost_at = Instant::now();
        self.firmware.borrow_mut().drive_mut(joint).silent_until =
            Some(lost_at + Duration::from_secs(60));
        loop {
            std::thread::sleep(PERIOD);
            self.tick()?;
            if self.supervisor.degraded_episode().is_some() {
                return Ok(());
            }
            assert!(
                Instant::now() < lost_at + window + SLACK,
                "{joint}: loss not answered {:?} after it went silent",
                lost_at.elapsed()
            );
        }
    }

    fn assert_every_drive_stopped(&self) {
        assert_eq!(self.supervisor.mode(), OperationalMode::Disabled);
        for drive in &self.firmware.borrow().drives {
            assert!(
                !drive.enabled || drive.silent_until.is_some(),
                "{} left enabled",
                drive.joint
            );
        }
    }

    fn latched(&self, class: FaultClass, joint: &str) -> bool {
        self.supervisor
            .safety_snapshot()
            .faults
            .iter()
            .any(|fault| fault.class == class && fault.joint.as_deref() == Some(joint))
    }
}

#[test]
fn distal_loss_sheds_only_its_subtree_with_one_disable_and_the_rest_keeps_holding() {
    let mut bench = Bench::active("drive-loss-distal");
    let disables_before = bench.sent(CommunicationType::Disable, LOWER_YAW);
    let mark = bench.trace_len();
    bench
        .lose(LOWER_YAW)
        .expect("a qualifying loss is no tick error");
    let episode = bench
        .supervisor
        .degraded_episode()
        .expect("episode")
        .clone();
    assert_eq!(episode.lost_joint, LOWER_YAW);
    assert_eq!(episode.shed_joints, [LOWER_YAW]);
    assert_eq!(episode.holding_joints, [PITCH, ROLL, UPPER_YAW, ELBOW]);
    assert_eq!(episode.frozen_positions.len(), 1);
    assert_eq!(
        bench.sent(CommunicationType::Disable, LOWER_YAW),
        disables_before + 1,
        "exactly one Disable to the lost address"
    );
    let shed_mark = bench.trace_len();
    for _ in 0..10 {
        std::thread::sleep(PERIOD);
        bench.tick().expect("holding ticks");
    }
    // The holding drives were commanded every tick across the shed.
    let pitch = u32::from(bench.device(PITCH));
    let firmware = bench.firmware.borrow();
    let mit_at: Vec<Instant> = firmware.tx[mark..]
        .iter()
        .zip(&firmware.tx_at[mark..])
        .filter(|(frame, _)| {
            frame.id & 0xff == pitch
                && (frame.id >> 24) & 0x1f == u32::from(CommunicationType::OperationControl.as_u8())
        })
        .map(|(_, at)| *at)
        .collect();
    drop(firmware);
    assert!(mit_at.len() > 10);
    let widest = mit_at.windows(2).map(|w| w[1] - w[0]).max().expect("gaps");
    assert!(
        widest < PERIOD * 4,
        "pitch MIT gap {widest:?} across the shed"
    );
    assert!(
        bench
            .written_since(LOWER_YAW, shed_mark)
            .iter()
            .all(|comm| *comm == CommunicationType::Disable.as_u8()),
        "nothing but Disable reaches the shed address"
    );
    assert!(!bench
        .written_since(LOWER_YAW, mark)
        .contains(&CommunicationType::ActiveReporting.as_u8()));
    for joint in [PITCH, ROLL, UPPER_YAW, ELBOW] {
        assert!(
            bench
                .written_since(joint, shed_mark)
                .contains(&CommunicationType::OperationControl.as_u8()),
            "{joint}: MIT continues"
        );
        assert!(bench.supervisor.joint_drive_active(joint), "{joint}");
    }
    assert!(bench.supervisor.joint_feedback(LOWER_YAW).is_none());
    assert!(!bench.supervisor.has_latched_fault());
    assert_eq!(bench.supervisor.mode(), OperationalMode::Active);
    // A batch that still names the shed joint is refused, not skipped.
    let stale = vec![davout::MitJointCommand {
        joint: LOWER_YAW.into(),
        kp: 0.0,
        kd: 0.0,
        position_rad: 0.0,
        velocity_rad_s: 0.0,
        torque_ff_nm: 0.0,
    }];
    assert!(matches!(
        bench.supervisor.send_mit_batch(stale),
        Err(DavoutError::InactiveJoint { .. })
    ));
    // Every stop afterwards writes only Disable to the shed address.
    let stop_mark = bench.trace_len();
    bench.supervisor.disable_all().expect("operator stop");
    assert_eq!(
        bench.written_since(LOWER_YAW, stop_mark),
        [CommunicationType::Disable.as_u8()]
    );
}

#[test]
fn intermediate_loss_disables_the_whole_subtree() {
    let mut bench = Bench::active("drive-loss-elbow");
    let before = (
        bench.sent(CommunicationType::Disable, ELBOW),
        bench.sent(CommunicationType::Disable, LOWER_YAW),
    );
    bench.lose(ELBOW).expect("qualifying loss");
    let episode = bench
        .supervisor
        .degraded_episode()
        .expect("episode")
        .clone();
    assert_eq!(episode.shed_joints, [ELBOW, LOWER_YAW]);
    assert_eq!(episode.holding_joints, [PITCH, ROLL, UPPER_YAW]);
    assert_eq!(
        (
            bench.sent(CommunicationType::Disable, ELBOW),
            bench.sent(CommunicationType::Disable, LOWER_YAW)
        ),
        (before.0 + 1, before.1 + 1)
    );
    // The healthy distal drive answers its Disable in Reset: expected.
    for _ in 0..10 {
        std::thread::sleep(PERIOD);
        bench.tick().expect("holding ticks");
    }
    assert!(!bench.firmware.borrow().drive(LOWER_YAW).enabled);
    assert!(!bench.supervisor.has_latched_fault());
    assert!(!bench.supervisor.active_joints().contains(LOWER_YAW));
}

#[test]
fn proximal_or_non_admitted_loss_stops_every_drive_at_once() {
    for joint in [PITCH, UPPER_YAW] {
        let mut bench = Bench::active(&format!("drive-loss-not-admitted-{joint}"));
        let error = bench
            .lose(joint)
            .expect_err("a loss without a plan stops everything");
        assert!(
            matches!(
                error,
                DavoutError::Homing { .. } | DavoutError::CommWatchdog { .. }
            ),
            "{joint}: {error}"
        );
        assert!(bench.supervisor.degraded_episode().is_none());
        bench.assert_every_drive_stopped();
    }
}

#[test]
fn a_second_loss_during_the_episode_stops_every_drive_and_latches() {
    let mut bench = Bench::active("drive-loss-second");
    bench.lose(LOWER_YAW).expect("first loss sheds");
    bench.firmware.borrow_mut().drive_mut(ROLL).silent_until =
        Some(Instant::now() + Duration::from_secs(60));
    let give_up = Instant::now() + Duration::from_millis(300);
    let error = loop {
        std::thread::sleep(PERIOD);
        if let Err(error) = bench.tick() {
            break error;
        }
        assert!(Instant::now() < give_up, "second loss answered");
    };
    assert!(
        matches!(
            error,
            DavoutError::Homing { .. } | DavoutError::CommWatchdog { .. }
        ),
        "{error}"
    );
    bench.assert_every_drive_stopped();
    assert!(bench.supervisor.degraded_episode().is_none());
    assert_eq!(
        bench.supervisor.degraded_outcome().map(|o| o.end),
        Some(DegradedEnd::Stopped)
    );
    assert!(bench.latched(FaultClass::Communication, LOWER_YAW));
}

#[test]
fn a_shed_drive_reporting_run_stops_every_drive() {
    let mut bench = Bench::active("drive-loss-run-frame");
    bench.lose(LOWER_YAW).expect("loss sheds");
    // The drive was deaf to its Disable and still runs; once it speaks after
    // the reply grace, a Run frame from it is a drive this owner no longer commands.
    std::thread::sleep(SHED_REPLY_GRACE + PERIOD);
    bench
        .firmware
        .borrow_mut()
        .drive_mut(LOWER_YAW)
        .silent_until = None;
    bench.firmware.borrow_mut().emit_report(LOWER_YAW);
    let error = bench.tick().expect_err("Run from a shed address latches");
    assert!(
        matches!(error, DavoutError::InvalidFeedback { .. }),
        "{error}"
    );
    assert!(bench.latched(FaultClass::DriveState, LOWER_YAW));
    bench.assert_every_drive_stopped();
    assert!(bench.supervisor.degraded_episode().is_none());
}

#[test]
fn a_rebooted_shed_drive_in_reset_is_tolerated() {
    let mut bench = Bench::active("drive-loss-reboot");
    let window = Duration::from_millis(bench.supervisor.control.control.comm_watchdog_ms);
    // The 2026-10-04 shape: silent, then back in Reset with type-24 reports.
    bench
        .firmware
        .borrow_mut()
        .drive_mut(LOWER_YAW)
        .reboot(Some(Instant::now() + Duration::from_millis(250)));
    let started = Instant::now();
    while bench.supervisor.degraded_episode().is_none() {
        std::thread::sleep(PERIOD);
        bench.tick().expect("qualifying loss");
        assert!(started.elapsed() < window + SLACK, "loss answered");
    }
    bench.firmware.borrow_mut().drive_mut(LOWER_YAW).reporting = true;
    let back = Instant::now() + Duration::from_millis(250);
    while Instant::now() < back + Duration::from_millis(50) {
        std::thread::sleep(PERIOD);
        bench
            .tick()
            .expect("Reset frames from the shed drive are tolerated");
    }
    assert!(bench.supervisor.degraded_episode().is_some());
    assert!(!bench.supervisor.has_latched_fault());
    let reboot = bench.supervisor.last_drive_reboot().expect("reboot logged");
    assert_eq!(reboot.joint, LOWER_YAW);
}

#[test]
fn the_deadline_stops_every_drive_even_without_a_controller() {
    let mut bench = Bench::active("drive-loss-deadline");
    bench.lose(LOWER_YAW).expect("loss sheds");
    let episode = bench
        .supervisor
        .degraded_episode()
        .expect("episode")
        .clone();
    // Hold window + distance (0 at rest) / speed + settle.
    let expected = Duration::from_secs_f64(HOLD_WINDOW_S + LOWER_SETTLE_S);
    let budget = episode.deadline - episode.since;
    assert!(
        budget >= expected && budget <= expected + Duration::from_millis(50),
        "{budget:?}"
    );
    // The controller stops ticking: no MIT, no completion. The next Davout
    // call after the deadline stops every drive.
    std::thread::sleep(episode.deadline.saturating_duration_since(Instant::now()) + PERIOD);
    let error = bench
        .supervisor
        .enforce_degraded_deadline()
        .expect_err("deadline latches");
    assert!(matches!(error, DavoutError::FaultLatched { .. }), "{error}");
    assert_eq!(
        bench.supervisor.degraded_outcome().map(|o| o.end),
        Some(DegradedEnd::Deadline)
    );
    assert!(bench.latched(FaultClass::Communication, LOWER_YAW));
    bench.assert_every_drive_stopped();
    assert!(
        bench.supervisor.check_fault_authority().is_err(),
        "restart required"
    );
}

#[test]
fn the_deadline_also_ends_an_episode_the_controller_keeps_holding() {
    let mut bench = Bench::active("drive-loss-deadline-holding");
    bench.lose(LOWER_YAW).expect("loss sheds");
    let deadline = bench
        .supervisor
        .degraded_episode()
        .expect("episode")
        .deadline;
    let error = loop {
        std::thread::sleep(PERIOD);
        if let Err(error) = bench.tick() {
            break error;
        }
        assert!(Instant::now() < deadline + SLACK, "deadline enforced");
    };
    assert!(Instant::now() >= deadline);
    assert!(matches!(error, DavoutError::FaultLatched { .. }), "{error}");
    assert_eq!(
        bench.supervisor.degraded_outcome().map(|o| o.end),
        Some(DegradedEnd::Deadline)
    );
    bench.assert_every_drive_stopped();
}

#[test]
fn operator_stop_and_completion_end_the_episode_with_a_latched_fault() {
    for complete in [false, true] {
        let mut bench = Bench::active(&format!("drive-loss-end-{complete}"));
        bench.lose(LOWER_YAW).expect("loss sheds");
        let refused = bench
            .supervisor
            .enable_targets(&[PITCH.to_owned()])
            .expect_err("no enable during an episode");
        assert!(
            matches!(refused, DavoutError::DegradedEpisode { .. }),
            "{refused}"
        );
        if complete {
            bench.supervisor.complete_degraded_lower().expect("stop");
        } else {
            bench.supervisor.disable_all().expect("stop");
        }
        let end = if complete {
            DegradedEnd::LowerComplete
        } else {
            DegradedEnd::Stopped
        };
        assert_eq!(
            bench.supervisor.degraded_outcome().map(|o| o.end),
            Some(end)
        );
        assert!(bench.latched(FaultClass::Communication, LOWER_YAW));
        bench.assert_every_drive_stopped();
        assert!(matches!(
            bench.supervisor.enable_targets(&[PITCH.to_owned()]),
            Err(DavoutError::FaultLatched { .. })
        ));
    }
}
