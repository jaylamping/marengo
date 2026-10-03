//! Envelope behaviour on the **live** `config/` + `assets/urdf/marengo.urdf` joints.
//!
//! Policies are built the way Davout's `build_limits` does it (URDF ∩ `motors.yaml` bench for
//! hard, `control.yaml` for soft and margins), but with struct literals so the tests do not
//! depend on the `JointLimitBounds` constructor. Audit leads K2/K3/K5 (2026-10-03).

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use armee_kinematics::{
    clamp_hold_target, clamp_position_in_envelope, effective_command_bounds, joint_limits,
    limit_margin_rad, load_urdf, measured_position_fault, JointLimitBounds, JointLimitPolicy,
    LimitMarginConfig,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn live_policy(joint: &str) -> JointLimitPolicy {
    let config = repo_root().join("config");
    let robot = marengo_config::load_robot_config_from(&config).expect("robot.yaml");
    let control = marengo_config::load_control_config_from(&config).expect("control.yaml");
    let motors = marengo_config::load_motors_config_from(&config).expect("motors.yaml");
    let urdf = load_urdf(repo_root().join(&robot.robot.urdf)).expect("live URDF");
    let urdf_lim = joint_limits(&urdf, joint).expect("urdf limits");
    let motor = motors
        .motors
        .iter()
        .find(|m| m.joint == joint)
        .expect("motor entry");
    let cfg = control.control.joints.get(joint).expect("control entry");
    let hard_lower = urdf_lim.lower.max(motor.bench.position_lower_rad);
    let hard_upper = urdf_lim.upper.min(motor.bench.position_upper_rad);
    JointLimitPolicy {
        bounds: JointLimitBounds {
            hard_lower,
            hard_upper,
            soft_lower: cfg
                .position_soft_lower_rad
                .unwrap_or(hard_lower)
                .clamp(hard_lower, hard_upper),
            soft_upper: cfg
                .position_soft_upper_rad
                .unwrap_or(hard_upper)
                .clamp(hard_lower, hard_upper),
        },
        margin: LimitMarginConfig {
            min_rad: cfg.position_limit_margin_min_rad,
            k_v_s: cfg.position_limit_margin_k_v_s,
            k_stop: cfg.position_limit_margin_k_stop,
            velocity_deadband_rad_s: cfg.position_trajectory_velocity_deadband_rad,
            measured_fault_slack_rad: cfg.position_limit_measured_fault_slack_rad,
            decel_rad_s2: cfg.position_trajectory_accel_rad_s2,
        },
        velocity: 2.0,
        effort: 5.0,
        tau_ff_max: 5.0,
    }
}

/// K2: on right pitch (`hard_lower ≈ −0.90`) a target a few hundred mrad inside the soft
/// bottom must get the kinetic margin when approached fast from above. The old gate
/// `max(hard_lower + 5 mrad, 5 mrad)` exempted every target ≤ +5 mrad.
#[test]
fn lower_target_on_wide_negative_joint_gets_kinetic_margin() {
    let p = live_policy("right_shoulder_pitch");
    assert!(p.hard_lower() < -0.5, "premise: {}", p.hard_lower());
    let dq = -1.25;
    let target = p.soft_lower() + 0.01;
    let (lo, _) = effective_command_bounds(&p, 0.5, dq);
    assert!(
        lo > target + 0.05,
        "premise: kinetic floor {lo} must sit above target {target}"
    );
    let clamped = clamp_hold_target(&p, 0.5, dq, target);
    assert!(
        (clamped - lo).abs() < 1e-9,
        "target {target} must be raised to the kinetic floor {lo}, got {clamped}"
    );
}

/// K2 counterpart: an exact stop target and operator zero-home stay exempt.
#[test]
fn stop_target_and_zero_home_remain_exempt() {
    let roll = live_policy("right_shoulder_roll");
    let home = clamp_hold_target(&roll, 0.75, -1.25, 0.0);
    assert!(
        home.abs() < 1e-9,
        "zero-home must not kinetic-clamp: {home}"
    );

    // The operator's "go to the bottom" (soft_lower, or anything below it, soft-clamped) is a
    // goal even at cruise speed; the per-tick envelope bounds the setpoint on the way.
    let pitch = live_policy("right_shoulder_pitch");
    for requested in [pitch.soft_lower(), pitch.hard_lower() + 0.004, -5.0] {
        let goal = clamp_hold_target(&pitch, 0.5, -1.25, requested);
        assert!(
            (goal - pitch.soft_lower()).abs() < 1e-9,
            "bottom goal {requested} must stay the soft bottom, got {goal}"
        );
    }
}

/// K3: when the kinetic margin swallows the whole soft range the envelope must collapse
/// next to `q`, not to the soft midpoint.
#[test]
fn collapsed_envelope_holds_near_q() {
    let mut p = live_policy("right_shoulder_roll");
    p.bounds = JointLimitBounds {
        hard_lower: 0.0,
        hard_upper: 1.0,
        soft_lower: 0.10,
        soft_upper: 0.20,
    };
    p.margin.decel_rad_s2 = 4.8;
    let dq = -2.0;
    assert!(
        limit_margin_rad(dq, &p.margin) > p.soft_upper(),
        "premise: the kinetic margin must consume the whole soft range"
    );
    for (q, expect) in [(0.19, 0.19), (0.12, 0.12), (0.5, 0.20), (-0.3, 0.10)] {
        let (lo, hi) = effective_command_bounds(&p, q, dq);
        assert!(
            (lo - expect).abs() < 1e-9 && (hi - expect).abs() < 1e-9,
            "q={q}: collapsed envelope ({lo}, {hi}) must be the soft-range point nearest q ({expect})"
        );
    }
    let mid = effective_command_bounds(&p, f64::NAN, dq);
    assert!((mid.0 - 0.15).abs() < 1e-9 && (mid.1 - 0.15).abs() < 1e-9);
}

/// K5: NaN never leaves the clamps, and a NaN measurement is a fault.
#[test]
fn non_finite_inputs_fail_closed() {
    let p = live_policy("right_elbow_pitch");
    let q = 0.3;
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let held = clamp_position_in_envelope(&p, q, 0.0, bad);
        assert!(
            held.is_finite() && (held - q).abs() < 1e-9,
            "{bad} -> {held}"
        );
        let target = clamp_hold_target(&p, q, 0.0, bad);
        assert!(
            target.is_finite() && (target - q).abs() < 1e-9,
            "{bad} -> {target}"
        );
        assert!(measured_position_fault(bad, &p), "{bad} measured q");
    }
    let both = clamp_position_in_envelope(&p, f64::NAN, 0.0, f64::NAN);
    assert!(both.is_finite() && both >= p.soft_lower() && both <= p.soft_upper());
    assert!(!measured_position_fault(q, &p));
}
