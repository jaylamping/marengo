//! Degraded hold and controlled lower after a single drive loss (ADR 0038):
//! the real controller over Davout's physical owner and the firmware emulator.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use berthier::{ControlLoop, ControlMode, DegradedEvent, DegradedLowerCause, LoopError};
use davout::{
    DegradedEnd, DriveLossPlan, FaultClass, OperationalMode, ReferenceAudit, ReferenceOutcome,
};
use marengo_config::{
    load_control_config_from, load_motors_config_from, write_control_config_from,
};

mod support;
use support::FixtureTree;

#[path = "../../davout/tests/physical_firmware/mod.rs"]
mod physical_firmware;
use physical_firmware::{Firmware, FirmwareBus, SharedFirmware};

const PITCH: &str = "right_shoulder_pitch";
const ELBOW: &str = "right_elbow_pitch";
const LOWER_YAW: &str = "right_lower_arm_yaw";
const FIVE: [&str; 5] = [
    PITCH,
    "right_shoulder_roll",
    "right_upper_arm_yaw",
    ELBOW,
    LOWER_YAW,
];
const PERIOD: Duration = Duration::from_millis(5);
const HOLD_WINDOW_S: f64 = 0.3;
const LOWER_VELOCITY: f64 = 0.25;
const PITCH_ELEVATED: f64 = 0.4;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Master tree with a relative history path, short reference deadlines and
/// short episode timing.
fn tree(label: &str) -> FixtureTree {
    let tree = FixtureTree::new(label, &repo_root());
    let config = tree.path().join("config");
    let homing = config.join("homing.yaml");
    let text = std::fs::read_to_string(&homing).expect("copied homing");
    std::fs::write(
        &homing,
        text.replace(
            "/opt/marengo/var/calibration/zero_registry.yaml",
            "var/calibration/zero_registry.yaml",
        )
        .replace("search_timeout_s: 30.0", "search_timeout_s: 5.0"),
    )
    .expect("fixture homing");
    let mut control = load_control_config_from(&config).expect("copied control");
    let drive_loss = control
        .control
        .drive_loss
        .as_mut()
        .expect("master drive_loss block");
    drive_loss.hold_window_s = HOLD_WINDOW_S;
    drive_loss.lower_velocity_rad_s = LOWER_VELOCITY;
    drive_loss.lower_settle_s = 1.5;
    drive_loss.lower_max_s = 4.0;
    write_control_config_from(&config, &control).expect("fixture control");
    tree
}

struct Rig {
    // Drop order: the controller (and its journal worker) before the tree.
    controller: ControlLoop<FirmwareBus>,
    firmware: SharedFirmware,
    _tree: FixtureTree,
}

impl Rig {
    /// Five joints referenced and enabled in one process, pitch and elbow
    /// raised in Position hold, the lower-arm yaw plan installed.
    fn elevated(label: &str) -> Self {
        let tree = tree(label);
        let motors = load_motors_config_from(tree.path().join("config")).expect("motors");
        let firmware = Firmware::from_motors(&motors.motors, &[]);
        for drive in &mut firmware.borrow_mut().drives {
            drive.follow_mit_rad_s = Some(1.0);
        }
        let journal = tree.path().join("reference-journal.sqlite3");
        let controller = ControlLoop::from_repo_with_physical_reference(
            tree.path(),
            FirmwareBus(firmware.clone()),
            &journal,
            200,
            25,
        )
        .expect("physical controller");
        let mut rig = Self {
            controller,
            firmware,
            _tree: tree,
        };
        rig.controller
            .supervisor_mut()
            .install_drive_loss_plans(vec![DriveLossPlan {
                joint: LOWER_YAW.into(),
                shed: vec![LOWER_YAW.into()],
            }])
            .expect("plan");
        rig.home_all();
        let targets: Vec<String> = FIVE.iter().map(|joint| (*joint).to_owned()).collect();
        rig.controller
            .supervisor_mut()
            .enable_targets(&targets)
            .expect("enable");
        rig.controller.set_control_mode(ControlMode::GravityComp);
        let give_up = Instant::now() + Duration::from_secs(3);
        while rig.controller.enable_completion().is_err() {
            assert!(Instant::now() < give_up, "enable completes");
            rig.tick().expect("bootstrap tick");
        }
        rig.controller
            .enter_position_hold_at(Some(PITCH), PITCH_ELEVATED)
            .expect("raise pitch");
        rig.controller
            .enter_position_hold_at(Some(ELBOW), 0.3)
            .expect("raise elbow");
        let give_up = Instant::now() + Duration::from_secs(3);
        while (rig.q(PITCH) - PITCH_ELEVATED).abs() > 0.01 || (rig.q(ELBOW) - 0.3).abs() > 0.01 {
            assert!(
                Instant::now() < give_up,
                "arm raised: pitch {}",
                rig.q(PITCH)
            );
            rig.tick().expect("raise tick");
        }
        rig
    }

    fn home_all(&mut self) {
        for joint in FIVE {
            let handle = self
                .controller
                .supervisor_mut()
                .request_reference(
                    joint,
                    true,
                    ReferenceAudit {
                        operator: "bench-operator".into(),
                        session: "degraded-hold".into(),
                    },
                )
                .expect("request admitted");
            let give_up = Instant::now() + Duration::from_secs(5);
            loop {
                self.tick().expect("reference tick");
                match self
                    .controller
                    .supervisor()
                    .reference_outcome(&handle)
                    .expect("outcome")
                {
                    ReferenceOutcome::Current { .. } => break,
                    ReferenceOutcome::Failed { message } => panic!("{joint}: {message}"),
                    ReferenceOutcome::InProgress => {
                        assert!(Instant::now() < give_up, "{joint}: reference finishes");
                    }
                }
            }
        }
    }

    /// One `marengo-pi` loop iteration: reports, reporting sync, tick, pacing.
    fn tick(&mut self) -> Result<Vec<DegradedEvent>, LoopError> {
        std::thread::sleep(PERIOD);
        self.firmware.borrow_mut().emit_reports();
        self.controller.supervisor_mut().sync_active_reporting();
        self.controller.tick(None)?;
        Ok(self.controller.take_degraded_events())
    }

    fn q(&self, joint: &str) -> f64 {
        self.firmware.borrow().drive(joint).position_joint_rad()
    }

    /// The lower-arm yaw drive goes silent; tick until the controller reports it.
    fn lose_lower_yaw(&mut self) {
        self.firmware.borrow_mut().drive_mut(LOWER_YAW).silent_until =
            Some(Instant::now() + Duration::from_secs(60));
        let give_up = Instant::now() + Duration::from_millis(300);
        loop {
            let events = self
                .tick()
                .unwrap_or_else(|error| panic!("a distal loss is no tick error: {error}"));
            if let Some(DegradedEvent::DriveLost {
                joint,
                shed,
                holding,
                ..
            }) = events.first()
            {
                assert_eq!(joint, LOWER_YAW);
                assert_eq!(shed, &[LOWER_YAW]);
                assert_eq!(holding.len(), 4);
                return;
            }
            assert!(Instant::now() < give_up, "loss reported");
        }
    }

    fn latched_communication(&self) -> bool {
        self.controller
            .supervisor()
            .safety_snapshot()
            .faults
            .iter()
            .any(|fault| {
                fault.class == FaultClass::Communication
                    && fault.joint.as_deref() == Some(LOWER_YAW)
            })
    }
}

#[test]
fn distal_loss_holds_then_lowers_to_rest_at_the_capped_speed_and_disables() {
    let mut rig = Rig::elevated("degraded-auto-lower");
    rig.lose_lower_yaw();
    let lost_at = Instant::now();
    assert_eq!(rig.controller.control_mode(), ControlMode::Position);
    // Hold window: the holding joints stay where they were.
    let mut started = None;
    while started.is_none() {
        let events = rig.tick().expect("holding tick");
        assert!(
            (rig.q(PITCH) - PITCH_ELEVATED).abs() < 0.02,
            "pitch held at {}",
            rig.q(PITCH)
        );
        if events.contains(&DegradedEvent::LowerStarted {
            cause: DegradedLowerCause::Timeout,
        }) {
            started = Some(Instant::now());
        }
        assert!(
            lost_at.elapsed() < Duration::from_secs(1),
            "auto-lower starts"
        );
    }
    let waited = lost_at.elapsed().as_secs_f64();
    assert!(
        waited + 0.02 >= HOLD_WINDOW_S,
        "not before the window: {waited}"
    );
    // Lower: the planner reference never moves faster than the cap.
    let index = rig
        .controller
        .joint_names()
        .iter()
        .position(|joint| joint == PITCH)
        .expect("pitch");
    let mut previous: Option<(Instant, f64)> = None;
    let mut fastest = 0.0_f64;
    let give_up = Instant::now() + Duration::from_secs(6);
    loop {
        let events = rig.tick().expect("lowering tick");
        if events.contains(&DegradedEvent::LowerComplete) {
            break;
        }
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, DegradedEvent::Ended { .. })),
            "the lower completes before the deadline: {events:?}"
        );
        if let Some(q_traj) = rig.controller.position_hold_commands() {
            let now = Instant::now();
            if let Some((at, q)) = previous {
                let dt = (now - at).as_secs_f64();
                fastest = fastest.max((q_traj[index] - q).abs() / dt);
            }
            previous = Some((now, q_traj[index]));
        }
        assert!(Instant::now() < give_up, "lower completes");
    }
    assert!(
        fastest <= LOWER_VELOCITY + 1e-6,
        "pitch reference at {fastest} rad/s"
    );
    assert!(fastest > 0.1, "pitch actually lowered: {fastest}");
    assert!(rig.q(PITCH).abs() < 0.06, "pitch at rest: {}", rig.q(PITCH));
    let supervisor = rig.controller.supervisor();
    assert_eq!(supervisor.mode(), OperationalMode::Disabled);
    assert_eq!(
        supervisor.degraded_outcome().map(|outcome| outcome.end),
        Some(DegradedEnd::LowerComplete)
    );
    assert!(rig.latched_communication(), "restart required");
    assert!(rig.controller.degraded_lost_joint().is_none());
}

#[test]
fn operator_lower_starts_before_the_window_and_other_motion_is_refused() {
    let mut rig = Rig::elevated("degraded-operator-lower");
    rig.lose_lower_yaw();
    let refused = |result: Result<(), LoopError>| {
        assert!(
            matches!(&result, Err(LoopError::DegradedHold { joint }) if joint == LOWER_YAW),
            "{result:?}"
        );
    };
    refused(rig.controller.enter_position_hold_at(Some(PITCH), 1.0));
    refused(rig.controller.set_torque_cmd(PITCH, 0.5));
    refused(rig.controller.ensure_active_for_motion());
    refused(
        rig.controller
            .start_position_wave(PITCH, 0.0, 0.2, 1, 1.0)
            .map(|_| ()),
    );
    rig.controller.set_control_mode(ControlMode::GravityComp);
    assert_eq!(rig.controller.control_mode(), ControlMode::Position);
    let asked = Instant::now();
    rig.controller
        .request_degraded_lower()
        .expect("lower accepted");
    let events = rig.tick().expect("tick");
    assert_eq!(
        events,
        [DegradedEvent::LowerStarted {
            cause: DegradedLowerCause::Operator
        }]
    );
    assert!(asked.elapsed().as_secs_f64() < HOLD_WINDOW_S);
    assert!(rig.controller.degraded_lowering());
}

#[test]
fn operator_stop_during_the_hold_disables_everything_and_latches() {
    let mut rig = Rig::elevated("degraded-operator-stop");
    rig.lose_lower_yaw();
    rig.tick().expect("holding tick");
    // `disable` / `hold-off`: everything off at once.
    rig.controller.set_control_mode(ControlMode::Disabled);
    assert_eq!(
        rig.controller.supervisor().mode(),
        OperationalMode::Disabled
    );
    let events = rig.controller.take_degraded_events();
    assert_eq!(
        events,
        [DegradedEvent::Ended {
            lost_joint: LOWER_YAW.into(),
            end: DegradedEnd::Stopped
        }]
    );
    assert!(rig.latched_communication());
    assert!(matches!(
        rig.controller.request_degraded_lower(),
        Err(LoopError::NoDegradedEpisode)
    ));
    for drive in &rig.firmware.borrow().drives {
        assert!(
            !drive.enabled || drive.joint == LOWER_YAW,
            "{} left enabled",
            drive.joint
        );
    }
}
