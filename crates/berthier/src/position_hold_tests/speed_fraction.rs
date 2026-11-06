// ADR 0040 amendment (2026-10-28): a hold-at speed fraction scales the planned speeds of the
// move, on both laws, and never the acceleration limit. Intended parent namespace:
// position_hold.
#![allow(clippy::expect_used)]

use super::law_gates::master_pitch;
use super::*;

const PERIOD_S: f64 = 0.005;
const HZ: u32 = 200;
const TICKS: u64 = 1_400;

/// Legacy-law joint (the master yaw's cruise and accel, its RS02 velocity cap).
fn legacy_params() -> HoldJointParams {
    HoldJointParams {
        kp: 12.0,
        kd: 1.5,
        ki: 0.0,
        max_lead: 0.12,
        vel_deadband: 0.02,
        advance_max_lead: 0.12,
        advance_vel_deadband: POSITION_HOLD_ERROR_DEADBAND_RAD,
        slew_rad_s: 0.15,
        trajectory_v_max: 1.43,
        trajectory_threshold_rad: 0.15,
        a_max: 3.0,
        velocity_cap: Some(2.0),
        friction: None,
        limit_policy: None,
        tau_meas: 0.0,
        law: HoldLaw::Legacy,
        descent_cap: None,
    }
}

/// Planner reference for 0 → `target` at `speed`, the joint tracking it one tick late (an
/// ideal plant, so no fuse, governor or catch-up path acts).
fn planned(params: &HoldJointParams, target: f64, speed: SpeedFraction) -> Vec<f64> {
    let mut hold = PositionHold::new(1);
    hold.set_joint_law(0, params.law.kind());
    let joints = vec![params.clone()];
    let names = vec!["j0".to_string()];
    let mut wave = None;
    hold.arm(&[0.0], &[0.0], 0);
    assert!(hold.apply_retarget(HoldRetarget {
        joint_idx: 0,
        clamped: target,
        requested: target,
        q: 0.0,
        tick: 0,
        dq_seed: Some(0.0),
        downward_seed: None,
        speed,
    }));
    assert_eq!(hold.speed_fraction(0), speed);
    let mut reference = vec![0.0];
    let mut dq = 0.0;
    for tick in 1..=TICKS {
        let q = *reference.last().expect("reference");
        hold.tick(HoldWorld {
            q: &[q],
            dq_meas: &[dq],
            tau_g: &[0.0],
            joints: &joints,
            joint_names: &names,
            dt: PERIOD_S,
            hz: HZ,
            tick_count: tick,
            reshaped: &[],
            ff_last_sent: &[],
            wave: &mut wave,
        })
        .expect("tracked move ticks cleanly");
        let next = hold.q_traj().expect("armed")[0];
        dq = (next - q) / PERIOD_S;
        reference.push(next);
    }
    reference
}

fn peak_speed(reference: &[f64]) -> f64 {
    reference
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs() / PERIOD_S)
        .fold(0.0, f64::max)
}

/// Reference acceleration over the first ticks of the move (the ramp from rest).
fn launch_accel(reference: &[f64]) -> f64 {
    let w = &reference[..3];
    ((w[2] - w[1]) - (w[1] - w[0])) / (PERIOD_S * PERIOD_S)
}

fn quarter() -> SpeedFraction {
    SpeedFraction::new(0.25).expect("valid fraction")
}

#[test]
fn quarter_speed_fraction_quarters_the_planned_peak_velocity() {
    let (pitch, _, _) = master_pitch();
    assert!(
        matches!(pitch.law, HoldLaw::ScaledPd(_)),
        "fixture is scaled PD"
    );
    // Each move is long enough to cruise at full speed, so both peaks are cruise speeds.
    for (label, params, target) in [("legacy", legacy_params(), 1.0), ("scaled_pd", pitch, 1.2)] {
        let full = planned(&params, target, SpeedFraction::FULL);
        let slow = planned(&params, target, quarter());
        let (v_full, v_slow) = (peak_speed(&full), peak_speed(&slow));
        assert!(
            (v_full - params.trajectory_v_max).abs() < 1e-6,
            "{label}: full-speed move cruises at {} (got {v_full})",
            params.trajectory_v_max
        );
        assert!(
            (v_slow - 0.25 * v_full).abs() <= 1e-6,
            "{label}: peak {v_slow} rad/s is not a quarter of {v_full} rad/s"
        );
        // The acceleration limit is kept, not scaled: both moves leave rest at a_max.
        for reference in [&full, &slow] {
            let a = launch_accel(reference);
            assert!(
                (a - params.a_max).abs() <= 1e-6 * params.a_max,
                "{label}: launch accel {a}, a_max {}",
                params.a_max
            );
        }
        // Exactly the move of a joint configured at a quarter of its speeds, same a_max.
        let configured_slow = HoldJointParams {
            slew_rad_s: 0.25 * params.slew_rad_s,
            trajectory_v_max: 0.25 * params.trajectory_v_max,
            velocity_cap: params.velocity_cap.map(|cap| 0.25 * cap),
            ..params.clone()
        };
        assert_eq!(
            slow,
            planned(&configured_slow, target, SpeedFraction::FULL),
            "{label}"
        );
        // Slower, never shorter: both moves end at the target.
        for reference in [&full, &slow] {
            let end = *reference.last().expect("reference");
            assert!((end - target).abs() < 1e-9, "{label}: ends at {end}");
        }
    }
}

#[test]
fn a_full_speed_fraction_leaves_the_params_unchanged() {
    let params = legacy_params();
    let full = params.at_speed_fraction(SpeedFraction::FULL);
    assert_eq!(full.slew_rad_s, params.slew_rad_s);
    assert_eq!(full.trajectory_v_max, params.trajectory_v_max);
    assert_eq!(full.velocity_cap, params.velocity_cap);
    let slow = params.at_speed_fraction(quarter());
    assert_eq!(slow.slew_rad_s, 0.25 * params.slew_rad_s);
    assert_eq!(slow.trajectory_v_max, 0.25 * params.trajectory_v_max);
    assert_eq!(slow.velocity_cap, Some(0.5));
    assert_eq!(slow.a_max, params.a_max, "acceleration limit kept");
    assert_eq!(
        slow.trajectory_threshold_rad,
        params.trajectory_threshold_rad
    );
}

#[test]
fn a_wave_runs_at_its_own_speed_after_a_slowed_hold_at() {
    let params = legacy_params();
    let mut hold = PositionHold::new(1);
    hold.arm(&[0.0], &[0.0], 0);
    hold.apply_retarget(HoldRetarget {
        joint_idx: 0,
        clamped: 0.2,
        requested: 0.2,
        q: 0.0,
        tick: 0,
        dq_seed: None,
        downward_seed: None,
        speed: quarter(),
    });
    let mut wave = Some(PositionWave::new(0, 0.0, 0.1, 1, 100, 1));
    hold.tick(HoldWorld {
        q: &[0.0],
        dq_meas: &[0.0],
        tau_g: &[0.0],
        joints: &[params],
        joint_names: &["j0".to_string()],
        dt: PERIOD_S,
        hz: HZ,
        tick_count: 1,
        reshaped: &[],
        ff_last_sent: &[],
        wave: &mut wave,
    })
    .expect("wave tick");
    assert_eq!(hold.speed_fraction(0), SpeedFraction::FULL);
}

/// Legacy-law hold short of `target` (planner parked at it, arm stuck at 0) ticked once at
/// `speed`: the planner speed the residual lead-follow picks.
fn lead_follow_speed(params: HoldJointParams, target: f64, speed: SpeedFraction) -> f64 {
    let mut hold = PositionHold::new(1);
    hold.set_joint_law(0, params.law.kind());
    hold.arm(&[0.0], &[0.0], 0);
    assert!(hold.apply_retarget(HoldRetarget {
        joint_idx: 0,
        clamped: target,
        requested: target,
        q: 0.0,
        tick: 0,
        dq_seed: Some(0.0),
        downward_seed: None,
        speed,
    }));
    hold.force_planner_hold_at(0, target).expect("armed");
    let mut wave = None;
    hold.tick(HoldWorld {
        q: &[0.0],
        dq_meas: &[0.0],
        tau_g: &[0.0],
        joints: &[params],
        joint_names: &["j0".to_string()],
        dt: PERIOD_S,
        hz: HZ,
        tick_count: 1,
        reshaped: &[],
        ff_last_sent: &[],
        wave: &mut wave,
    })
    .expect("lead-follow tick");
    let (q_traj, dq_traj) = hold.planner_state(0).expect("armed");
    assert!(q_traj > 0.0, "the residual lead-follows toward the target");
    dq_traj.abs()
}

#[test]
fn the_residual_lead_follow_floor_scales_with_the_speed_fraction() {
    let params = legacy_params();
    let tiny = SpeedFraction::new(0.001).expect("valid fraction");
    let scaled_slew = params.at_speed_fraction(tiny).slew_rad_s;
    let v = lead_follow_speed(params.clone(), 0.02, tiny);
    assert!(
        v > 0.0 && v <= scaled_slew + 1e-12,
        "lead-follow at {v} rad/s exceeds the scaled slew {scaled_slew} rad/s"
    );
    // Unscaled: the floor is unchanged, so a joint configured slower than it finishes at it.
    let crawl = HoldJointParams {
        slew_rad_s: 0.001,
        ..params.clone()
    };
    assert_eq!(lead_follow_speed(crawl, 0.02, SpeedFraction::FULL), 0.005);
    assert_eq!(
        lead_follow_speed(params.clone(), 0.02, SpeedFraction::FULL),
        params.slew_rad_s
    );
}
