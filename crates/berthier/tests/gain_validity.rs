//! Invalid testing gains must not become persistent controller state.

#![allow(clippy::expect_used)]

use berthier::{ControlLoop, ControlMode, GainOverride};
use davout::MemoryBus;

fn controller() -> ControlLoop<MemoryBus> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut controller =
        ControlLoop::from_repo(root, MemoryBus::default(), 200, 50).expect("repository fixture");
    controller.set_control_mode(ControlMode::Impedance);
    controller
}

fn gains() -> GainOverride {
    GainOverride {
        kp: 1.0,
        kd: 0.1,
        ki: 0.0,
        fc: 0.0,
    }
}

#[test]
fn invalid_gain_fields_cannot_replace_an_installed_override() {
    let mut controller = controller();
    let original = gains();
    controller
        .apply_gain_override("right_shoulder_pitch", original.clone())
        .expect("valid seed");
    for field in 0..4 {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
            let mut invalid = gains();
            match field {
                0 => invalid.kp = value,
                1 => invalid.kd = value,
                2 => invalid.ki = value,
                _ => invalid.fc = value,
            }
            assert!(controller
                .apply_gain_override("right_shoulder_pitch", invalid)
                .is_err());
            assert_eq!(
                controller.gain_override("right_shoulder_pitch"),
                Some(&original),
                "field={field} value={value} replaced valid controller state"
            );
        }
    }
}

#[test]
fn invalid_or_unknown_joint_gain_leaves_existing_gains_intact() {
    // Batch API removed as test-only; per-joint validation must still leave
    // installed controller state untouched on refusal.
    for invalid_joint in ["right_shoulder_roll", "unmapped_joint"] {
        let mut controller = controller();
        let original = gains();
        controller
            .apply_gain_override("right_shoulder_pitch", original.clone())
            .expect("valid seed");
        let mut invalid = gains();
        if invalid_joint == "right_shoulder_roll" {
            invalid.kd = -1.0;
        }
        assert!(controller
            .apply_gain_override(invalid_joint, invalid)
            .is_err());
        assert_eq!(
            controller.gain_override("right_shoulder_pitch"),
            Some(&original)
        );
        assert!(controller.gain_override(invalid_joint).is_none());
    }
}
