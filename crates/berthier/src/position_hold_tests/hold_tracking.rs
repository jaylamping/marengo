// Hold-tracking fuse and grid-aware home classification (bench 2026-10-03 pitch trip).
// Intended parent namespace: position_hold.

use super::*;

const JOINT: &str = "right_shoulder_pitch";
const PERIOD_S: f64 = 0.005;
const HZ: u32 = 200;
/// Incident model torque at the hanging pose (joint space, Nm).
const BAD_MODEL_TAU_G: f64 = -1.6;

/// RS03 joint-space progress threshold, computed as Davout does for gear 1.
fn rs03_progress_threshold() -> f64 {
    rs03_feedback_count() / 2.0 + 16.0 * f64::from(f32::EPSILON) * rs03_span()
}

fn rs03_span() -> f64 {
    f64::from(4.0 * std::f32::consts::PI)
}

fn rs03_feedback_count() -> f64 {
    rs03_span() / 32767.0
}

/// Decoded encoder level nearest `q`.
fn on_grid(q: f64) -> f64 {
    (q / rs03_feedback_count()).round() * rs03_feedback_count()
}

fn pitch_params() -> HoldJointParams {
    // Live pitch impedance gains from the incident (kp 18, kd 3, ki 5).
    HoldJointParams {
        kp: 18.0,
        kd: 3.0,
        ki: 5.0,
        max_lead: 0.15,
        vel_deadband: 0.02,
        advance_max_lead: ADVANCE_MAX_LEAD_DEFAULT,
        advance_vel_deadband: POSITION_HOLD_ERROR_DEADBAND_RAD,
        slew_rad_s: 0.15,
        trajectory_v_max: 2.0,
        trajectory_threshold_rad: 0.15,
        a_max: 4.8,
        velocity_cap: Some(2.0),
        friction: None,
        limit_policy: None,
        tau_meas: 0.0,
        law: HoldLaw::Legacy,
    }
}

#[derive(Debug)]
struct Run {
    terminal: Option<(HoldError, u32)>,
    max_hold_tracking_ms: u64,
    max_ascent_stall_ms: u64,
}

/// Hold-on at `latched`, then replay `q_at(step)` with `tau_g` for up to `steps` ticks.
fn run_hold(latched: f64, target: f64, tau_g: f64, steps: u32, q_at: impl Fn(u32) -> f64) -> Run {
    let mut hold = PositionHold::with_progress_thresholds(vec![rs03_progress_threshold()]);
    hold.arm(&[latched], &[target], 0);
    let params = [pitch_params()];
    let names = [String::from(JOINT)];
    let mut run = Run {
        terminal: None,
        max_hold_tracking_ms: 0,
        max_ascent_stall_ms: 0,
    };
    let mut previous_q = latched;
    for step in 1..=steps {
        let q = q_at(step);
        let dq = (q - previous_q) / PERIOD_S;
        previous_q = q;
        let mut wave = None;
        match hold.tick(HoldWorld {
            q: &[q],
            dq_meas: &[dq],
            tau_g: &[tau_g],
            joints: &params,
            joint_names: &names,
            dt: PERIOD_S,
            hz: HZ,
            tick_count: u64::from(step),
            wave: &mut wave,
        }) {
            Ok(out) => {
                run.max_hold_tracking_ms =
                    run.max_hold_tracking_ms.max(out.diag[0].hold_tracking_ms);
                run.max_ascent_stall_ms = run.max_ascent_stall_ms.max(out.diag[0].ascent_stall_ms);
            }
            Err(err) => {
                run.terminal = Some((err, step));
                break;
            }
        }
    }
    run
}

/// Incident replay: the arm sags ~36 mrad below a zero latch in ~0.9 s, then stops dead.
fn incident_sag(step: u32) -> f64 {
    let sag_steps = 180.0;
    let fraction = (f64::from(step) / sag_steps).min(1.0);
    on_grid(fraction * -0.0357)
}

#[test]
fn retarget_preserves_hold_tracking_budget() {
    let mut hold = PositionHold::with_progress_thresholds(vec![rs03_progress_threshold()]);
    hold.arm(&[0.0], &[0.0], 0);
    let params = [pitch_params()];
    let names = [String::from(JOINT)];
    let stuck = on_grid(-0.0357);
    for step in 1..=200 {
        let mut wave = None;
        let tick = hold.tick(HoldWorld {
            q: &[stuck],
            dq_meas: &[0.0],
            tau_g: &[BAD_MODEL_TAU_G],
            joints: &params,
            joint_names: &names,
            dt: PERIOD_S,
            hz: HZ,
            tick_count: step,
            wave: &mut wave,
        });
        assert!(tick.is_ok(), "1 s is inside the budget: {tick:?}");
    }
    assert!(hold.hold_tracking_ms_at(0) >= 900);
    assert!(hold.apply_retarget(HoldRetarget {
        joint_idx: 0,
        clamped: 0.3,
        requested: 0.3,
        q: stuck,
        tick: 201,
        dq_seed: Some(0.0),
        downward_seed: None,
    }));
    assert!(
        hold.hold_tracking_ms_at(0) >= 900,
        "a retarget never renews a safety budget; only progress or the condition ending does"
    );
}

#[test]
fn exact_zero_latch_sag_with_opposing_torque_trips_hold_tracking() {
    let run = run_hold(0.0, 0.0, BAD_MODEL_TAU_G, 1_000, incident_sag);
    assert!(
        matches!(run.terminal, Some((HoldError::HoldTracking { .. }, _))),
        "exact-0 hold that sags under opposing torque must trip; got {run:#?}"
    );
    let Some((HoldError::HoldTracking { joint, ms, trip }, step)) = run.terminal else {
        return;
    };
    assert_eq!(joint, JOINT);
    assert!(
        (POSITION_ASCENT_STALL_FAULT_MS..POSITION_ASCENT_STALL_FAULT_MS + 10).contains(&ms),
        "fuse keeps the 2000 ms budget; got {ms}"
    );
    // Sag (≤0.9 s) plus the 2 s budget, at 5 ms ticks.
    assert!(step <= 180 + 400 + 1, "trip too late at step {step}");
    assert_eq!(trip.target, 0.0);
    assert!(
        (trip.q - on_grid(-0.0357)).abs() < 1e-12,
        "trip q {}",
        trip.q
    );
    assert_eq!(trip.tau_g, BAD_MODEL_TAU_G);
    assert!(
        trip.tau_p > 0.0 && trip.tau_p + trip.tau_ff < 0.0,
        "P pulls toward target but net command pushes away: {trip:?}"
    );
    assert_eq!(
        run.max_ascent_stall_ms, 0,
        "home hold never arms the ascent fuse"
    );
    let message = HoldError::HoldTracking {
        joint: JOINT.to_string(),
        ms,
        trip,
    }
    .to_string();
    for field in [
        "hold tracking",
        "tau_p=",
        "tau_ff=",
        "tau_g=",
        "q=",
        "target=",
    ] {
        assert!(message.contains(field), "{field} missing from {message}");
    }
}

#[test]
fn one_count_latch_classifies_exactly_like_exact_zero() {
    let one_count = on_grid(rs03_feedback_count());
    assert!(one_count > POSITION_SETTLE_TOLERANCE_RAD);

    let mut hold = PositionHold::with_progress_thresholds(vec![rs03_progress_threshold()]);
    hold.arm(&[one_count], &[one_count], 0);
    assert_eq!(hold.targets().map(<[f64]>::to_vec), Some(vec![0.0]));
    assert_eq!(
        hold.setpoints_raw.clone(),
        Some(vec![one_count]),
        "operator request stays visible"
    );

    // The incident replay (same encoder samples) ends in the same fault at the same tick.
    let exact = run_hold(0.0, 0.0, BAD_MODEL_TAU_G, 1_000, incident_sag);
    let one = run_hold(one_count, one_count, BAD_MODEL_TAU_G, 1_000, incident_sag);
    let outcome = |run: &Run| {
        run.terminal.as_ref().map(|(err, step)| match err {
            HoldError::HoldTracking { trip, .. } => ("hold_tracking", trip.target, trip.q, *step),
            HoldError::AscentStall { trip, .. } => ("ascent_stall", trip.target, trip.q, *step),
            _ => ("other", f64::NAN, f64::NAN, *step),
        })
    };
    assert_eq!(
        outcome(&one),
        outcome(&exact),
        "one-count latch must not change classification or fuse; exact={exact:#?} one={one:#?}"
    );
    assert!(matches!(outcome(&one), Some(("hold_tracking", ..))));
    assert_eq!(
        one.max_ascent_stall_ms, 0,
        "a home latch never arms the ascent fuse"
    );

    // Outside the grid tolerance an outbound latch keeps its value.
    let three_counts = on_grid(3.0 * rs03_feedback_count());
    hold.arm(&[three_counts], &[three_counts], 0);
    assert_eq!(
        hold.targets().map(<[f64]>::to_vec),
        Some(vec![three_counts])
    );
}

#[test]
fn genuine_outbound_ascent_stall_still_reports_ascent_stall() {
    // Too weak to lift: model gravity holds toward the target, net command pushes up.
    let run = run_hold(0.02, 0.15, 0.3, 1_000, |_| 0.02);
    assert!(
        matches!(run.terminal, Some((HoldError::AscentStall { .. }, _))),
        "stuck outbound move must report AscentStall; got {run:#?}"
    );
    let Some((HoldError::AscentStall { joint, ms, trip }, _)) = run.terminal else {
        return;
    };
    assert_eq!(joint, JOINT);
    assert!(ms >= POSITION_ASCENT_STALL_FAULT_MS);
    assert_eq!(trip.target, 0.15);
    assert_eq!(trip.q, 0.02);
    assert_eq!(trip.tau_g, 0.3);
    assert!(trip.tau_p + trip.tau_ff > 0.0, "{trip:?}");
    assert_eq!(
        run.max_hold_tracking_ms, 0,
        "net command toward target never arms hold tracking"
    );
    let message = HoldError::AscentStall {
        joint: JOINT.to_string(),
        ms,
        trip,
    }
    .to_string();
    assert!(message.contains("outbound ascent stall"), "{message}");
    assert!(message.contains("tau_p="), "{message}");
}

#[test]
fn hold_within_band_does_not_trip() {
    // Same opposing model torque, but the arm rests inside the 0.03 rad band.
    let run = run_hold(0.0, 0.0, BAD_MODEL_TAU_G, 1_200, |_| on_grid(-0.02));
    assert!(run.terminal.is_none(), "{run:#?}");
    assert_eq!(run.max_hold_tracking_ms, 0);
    assert_eq!(run.max_ascent_stall_ms, 0);
}

#[test]
fn hold_off_band_with_net_torque_toward_target_does_not_trip() {
    // Weak but correctly signed: outside the band, command pulls toward target.
    let run = run_hold(0.0, 0.0, 0.0, 1_200, |_| on_grid(-0.05));
    assert!(run.terminal.is_none(), "{run:#?}");
    assert_eq!(run.max_hold_tracking_ms, 0);
}
