//! Cross-crate sign convention tests for tau_g.
//!
//! Verifies the sign flows correctly from armee-dynamics → berthier → davout:
//!
//! - tau_g from armee-dynamics is positive at positive q (gravity pulls down,
//!   motor must push up against gravity = positive joint torque).
//! - Davout flips the sign when motor `direction = -1` (joint→motor transform).
//!
//! These protect against sign-flip regressions when tau_g computation changes.

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use armee_dynamics::DynamicsModel;
use robstride::comm::unpack_ext_id;

/// Repo root relative to `CARGO_MANIFEST_DIR` of the berthier crate.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Set `MARENGO_CONFIG_DIR` to the shoulder-pitch-right-only bench profile and
/// return a guard that restores the original value when dropped.
fn with_bench_config() -> scopeguard_guard {
    let prev = std::env::var("MARENGO_CONFIG_DIR").ok();
    std::env::set_var(
        "MARENGO_CONFIG_DIR",
        repo_root().join("config/bringup/arm_2dof_right"),
    );
    scopeguard_guard(prev)
}

/// Scoped guard that restores an env var on drop.
#[allow(non_camel_case_types)]
struct scopeguard_guard(Option<String>);
impl Drop for scopeguard_guard {
    fn drop(&mut self) {
        match &self.0 {
            Some(v) => std::env::set_var("MARENGO_CONFIG_DIR", v),
            None => std::env::remove_var("MARENGO_CONFIG_DIR"),
        }
    }
}

#[test]
fn tau_g_sign_positive_at_positive_q() {
    // At q=0.3 rad, tau_g should be POSITIVE for the right_shoulder_pitch joint.
    // Gravity pulls the arm down; the motor must push up (= positive joint torque).
    // Golden value from bench-weighted-700g-results.md Phase 1.
    let model_path = repo_root().join("assets/urdf/arm_2dof_right.urdf");
    let model =
        armee_dynamics::gravity_model_from_urdf(&model_path, &["right_shoulder_pitch".to_string()])
            .expect("build UrdfGravityModel");
    let tau = model.gravity_torques(&[0.3]).expect("tau_g at q=0.3");
    assert!(
        tau[0] > 0.0,
        "tau_g at positive q must be positive: got {}",
        tau[0],
    );
    // Also confirm the magnitude is reasonable (~0.93 Nm per golden values).
    assert!(
        (tau[0] - 0.9278).abs() < 0.05,
        "tau_g at q=0.3 expected ~0.9278, got {}",
        tau[0],
    );
}

#[test]
fn motor_space_sign_flipped_by_direction() {
    // The right_shoulder_pitch motor has direction=-1, gear_ratio=1.0 in
    // config/bringup/arm_2dof_right/motors.yaml.
    // motor_position_scale = direction * gear_ratio = -1.0.
    //
    // When a joint-space torque_ff flows through send_mit_joint:
    //   wire.torque_ff_nm = filtered.torque_ff_nm / scale
    //                     = positive_torque / (-1.0)
    //                     = negative_torque  (sign flipped)
    //
    // We verify this indirectly: send a positive torque_ff and check that the
    // encoded CAN frame carries a negative torque value (extra_data < 0x7FFF).
    use davout::{ControlMode, Supervisor};
    use robstride::bus::MemoryBus;

    // ---- setup ----
    let _guard = with_bench_config();
    let bus = MemoryBus::default();
    let mut sup = Supervisor::from_repo(repo_root(), bus).expect("Supervisor");

    // Mark all joints homed and enable to Active.
    {
        let motors = sup.motors.motors.clone();
        sup.homing_registry_mut()
            .bench_mark_all_verified(&motors)
            .expect("bench homing");
    }
    sup.set_homing_complete().expect("homing complete");
    sup.request_enable(true).expect("enable");

    // Set control mode to GravityComp (required for sending mit commands).
    sup.set_control_mode(ControlMode::GravityComp);

    // Seed synthetic feedback so the watchdog / clamping pass.
    sup.seed_synthetic_feedback();

    // ---- act ----
    // Send a MIT command with positive torque_ff_nm = 1.0 Nm.
    use davout::MitJointCommand;
    let motor = sup
        .motors
        .motors
        .iter()
        .find(|m| m.joint == "right_shoulder_pitch")
        .expect("right_shoulder_pitch motor entry")
        .clone();
    sup.send_mit_joint(
        MitJointCommand {
            joint: "right_shoulder_pitch".to_string(),
            kp: 0.0,
            kd: 0.0,
            position_rad: 0.0,
            velocity_rad_s: 0.0,
            torque_ff_nm: 1.0,
        },
        &motor,
    )
    .expect("send_mit_joint");

    // ---- verify ----
    // Get the transmitted CAN frame from the memory bus.
    let bus = sup.bus_mut();
    let frame = bus.tx.last().expect("at least one CAN frame transmitted");

    // Decode the CAN ID. The torque_ff is in the extra_data field.
    let ext_id = unpack_ext_id(frame.id).expect("valid extended CAN ID");

    // For direction=-1, torque_ff_nm = 1.0 / (-1.0) = -1.0.
    // The signed encoder maps 0 → 0x7FFF, positive → > 0x7FFF, negative → < 0x7FFF.
    // So we expect extra_data < 0x7FFF.
    assert!(
        ext_id.extra_data < 0x7FFF,
        "torque_ff extra_data={:#06x} should be < 0x7FFF (negative) when direction=-1 flips positive joint torque",
        ext_id.extra_data,
    );
}

#[test]
fn tau_g_at_pi_over_2_positive() {
    // Confirm sign is positive even at large q.
    let model_path = repo_root().join("assets/urdf/arm_2dof_right.urdf");
    let model =
        armee_dynamics::gravity_model_from_urdf(&model_path, &["right_shoulder_pitch".to_string()])
            .expect("build UrdfGravityModel");
    let tau = model
        .gravity_torques(&[std::f64::consts::FRAC_PI_2])
        .expect("tau_g at q=π/2");
    assert!(
        tau[0] > 0.0,
        "tau_g at q=π/2 must be positive: got {}",
        tau[0],
    );
}
