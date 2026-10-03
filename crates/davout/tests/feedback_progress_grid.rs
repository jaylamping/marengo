// V2 adds literal full-wire endpoint pairs to preserved unexecuted v1 28e7c6b6.
// New-interface numeric conformance only. Parent binds/runs the copied test.
// No current-reference fixture, Enable, physical gearing qualification or formula mirror.
#![allow(clippy::expect_used)]

#[path = "../../berthier/tests/support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use davout::simulation::{InitialVirtualReference, SimulationBus};
use davout::{DavoutError, JointHomingState, OperationalMode, Supervisor};
use robstride::{CanFrame, ReceivedCanFrame};

const JOINT: &str = "right_shoulder_pitch";

fn numeric_fixture(source: &Path, gear: f64, direction: i8) -> support::FixtureTree {
    let fixture = support::fixture_tree_without_diagnostics("feedback-progress-grid", source);
    let motors_path = fixture.path().join("config/motors.yaml");
    let mut motors: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&motors_path).expect("copied motors"))
            .expect("valid copied motor YAML");
    let pitch = motors["motors"]
        .as_sequence_mut()
        .expect("motor rows")
        .iter_mut()
        .find(|row| row["joint"].as_str() == Some(JOINT))
        .expect("installed pitch RS03 row");
    pitch["gear_ratio"] = serde_yaml::to_value(gear).expect("finite declarative gear");
    pitch["direction"] = serde_yaml::to_value(direction).expect("declarative direction");
    std::fs::write(
        &motors_path,
        serde_yaml::to_string(&motors).expect("serialize copied policy"),
    )
    .expect("only preconstruction copied gear/direction changed; all limits retained");
    fixture
}

fn raw_count(code: u16) -> ReceivedCanFrame {
    let [high, low] = code.to_be_bytes();
    ReceivedCanFrame::full_data(
        Some("can0".to_string()),
        CanFrame {
            id: 0x0280_01fd,
            data: [high, low, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8],
            extended: true,
        },
    )
}

#[test]
fn installed_grid_admits_adjacent_raw_counts_and_ignores_later_public_remap() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original_motors = std::fs::read(source.join("config/motors.yaml")).expect("master motors");
    let original_control =
        std::fs::read(source.join("config/control.yaml")).expect("master control");
    for gear in [0.5, 2.0, 100.0, 1e20] {
        for direction in [-1_i8, 1] {
            let fixture = numeric_fixture(&source, gear, direction);
            let fixture_path = fixture.path().to_path_buf();
            let mut supervisor = Supervisor::from_simulation(
                fixture.path(),
                SimulationBus::default(),
                InitialVirtualReference::Unreferenced,
            )
            .expect("closed unreferenced numeric Supervisor with copied resources");
            let threshold = supervisor
                .joint_position_progress_threshold(JOINT)
                .expect("representable installed grid reaches public getter");
            let mut positions = [0.0; 7];
            let mut drain_counts = [0; 7];
            for (index, code) in [0x0000, 0x0001, 0x7ffe, 0x7fff, 0x8000, 0xfffe, 0xffff]
                .into_iter()
                .enumerate()
            {
                supervisor
                    .bus_mut()
                    .queue_received(raw_count(code))
                    .expect("literal addressed count");
                drain_counts[index] = supervisor
                    .drain_feedback()
                    .expect("actual raw receive boundary");
                positions[index] = supervisor
                    .joint_feedback(JOINT)
                    .expect("actual converted pose, never a seeded cache")
                    .position_rad;
            }
            let mode = supervisor.mode();
            let homing = supervisor.joint_homing_state(JOINT);
            let latched = supervisor.has_latched_fault();
            let no_motion = !supervisor
                .bus()
                .transmissions()
                .iter()
                .any(|tx| matches!(tx.frame.id >> 24, 1 | 3));
            let pitch = supervisor
                .motors
                .motors
                .iter_mut()
                .find(|motor| motor.joint == JOINT)
                .expect("public pitch policy");
            pitch.gear_ratio = gear * 2.0;
            pitch.direction = -direction;
            let threshold_after_remap = supervisor.joint_position_progress_threshold(JOINT);
            let unknown_rejected = matches!(
                supervisor.joint_position_progress_threshold("not-an-installed-joint"),
                Err(DavoutError::UnknownJoint { joint }) if joint == "not-an-installed-joint"
            );
            drop(supervisor);
            drop(fixture);

            assert!(!fixture_path
                .try_exists()
                .expect("exclusive fixture cleanup"));
            assert_eq!(
                std::fs::read(source.join("config/motors.yaml")).expect("master motors retained"),
                original_motors
            );
            assert_eq!(
                std::fs::read(source.join("config/control.yaml")).expect("master control retained"),
                original_control
            );
            assert_eq!(
                drain_counts, [1; 7],
                "all literal inputs reached raw receive"
            );
            assert!(positions.iter().all(|position| position.is_finite()));
            assert_eq!(positions[3], 0.0, "literal centered code is exactly zero");
            assert!(threshold.is_finite() && threshold > 0.0);
            // Independent literal adjacent pairs at both full-wire endpoints and center.
            // Disabled diagnostics retain real decoded evidence without changing hard limits.
            for (left, right) in [(0, 1), (2, 3), (3, 4), (5, 6)] {
                let delta = positions[right] - positions[left];
                assert!(delta.abs() > threshold,
                    "new-grid conformance: adjacent raw levels must exceed threshold; gear={gear} direction={direction} pair={left}/{right} positions={positions:?} threshold={threshold}");
                assert!(
                    delta * f64::from(direction) > 0.0,
                    "actual installed direction must determine every adjacent joint-space ordering"
                );
            }
            assert!(
                matches!(threshold_after_remap, Ok(value) if value == threshold),
                "read-only grid stays bound to installed conversion after public policy mutation"
            );
            assert!(unknown_rejected);
            assert_eq!(mode, OperationalMode::Disabled);
            assert_eq!(homing, JointHomingState::Unhomed);
            assert!(!latched && no_motion);
        }
    }
}

#[test]
fn getter_refuses_subnormal_or_overflowing_joint_feedback_profiles() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original_motors = std::fs::read(source.join("config/motors.yaml")).expect("master motors");
    let original_control =
        std::fs::read(source.join("config/control.yaml")).expect("master control");
    for gear in [1e37, 1e-40] {
        let fixture = numeric_fixture(&source, gear, -1);
        let fixture_path = fixture.path().to_path_buf();
        let constructed = Supervisor::from_simulation(
            fixture.path(),
            SimulationBus::default(),
            InitialVirtualReference::Unreferenced,
        );
        let (constructor_error, getter_result, no_motion, mode, homing) = match constructed {
            Ok(supervisor) => {
                let result = supervisor.joint_position_progress_threshold(JOINT);
                let no_motion = !supervisor
                    .bus()
                    .transmissions()
                    .iter()
                    .any(|tx| matches!(tx.frame.id >> 24, 1 | 3));
                let mode = supervisor.mode();
                let homing = supervisor.joint_homing_state(JOINT);
                drop(supervisor);
                (None, Some(result), no_motion, Some(mode), Some(homing))
            }
            Err(error) => (Some(error.to_string()), None, false, None, None),
        };
        drop(fixture);

        assert!(!fixture_path
            .try_exists()
            .expect("exclusive fixture cleanup"));
        assert_eq!(
            std::fs::read(source.join("config/motors.yaml")).expect("master motors retained"),
            original_motors
        );
        assert_eq!(
            std::fs::read(source.join("config/control.yaml")).expect("master control retained"),
            original_control
        );
        assert!(constructor_error.is_none(),
            "new-getter conformance requires actual boundary reachability, not constructor refusal: gear={gear} error={constructor_error:?}");
        assert!(matches!(&getter_result,
            Some(Err(DavoutError::InvalidMotorConfig { joint, .. })) if joint == JOINT),
            "new-grid conformance: unrepresentable final-f32 profile must be refused by the actual getter: gear={gear} result={getter_result:?}");
        assert!(no_motion);
        assert_eq!(mode, Some(OperationalMode::Disabled));
        assert_eq!(homing, Some(JointHomingState::Unhomed));
    }
}
