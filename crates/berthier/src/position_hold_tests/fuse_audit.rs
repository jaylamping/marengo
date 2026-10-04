// Phase B / WP-D fuse audit: outbound direction, retarget-proof budgets, wave fuse,
// exemptions, integral window and velocity cap. Intended parent namespace: position_hold.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

const PERIOD_S: f64 = 0.005;
const HZ: u32 = 200;

fn params() -> HoldJointParams {
    HoldJointParams {
        kp: 18.0,
        kd: 3.0,
        ki: 0.0,
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

/// RS03 joint-space progress threshold, computed as Davout does for gear 1.
fn rs03_progress_threshold() -> f64 {
    let span = f64::from(4.0 * std::f32::consts::PI);
    span / 32767.0 / 2.0 + 16.0 * f64::from(f32::EPSILON) * span
}

fn rs03_count() -> f64 {
    f64::from(4.0 * std::f32::consts::PI) / 32767.0
}

fn on_grid(q: f64) -> f64 {
    (q / rs03_count()).round() * rs03_count()
}

/// Drives `PositionHold::tick` with caller-supplied measurements.
struct Rig {
    hold: PositionHold,
    params: Vec<HoldJointParams>,
    names: Vec<String>,
    wave: Option<PositionWave>,
    step: u64,
}

impl Rig {
    fn new(n: usize) -> Self {
        Self::with_hold(PositionHold::new(n), n)
    }

    fn with_hold(hold: PositionHold, n: usize) -> Self {
        Self {
            hold,
            params: vec![params(); n],
            names: (0..n).map(|i| format!("j{i}")).collect(),
            wave: None,
            step: 0,
        }
    }

    fn tick(&mut self, q: &[f64], dq: &[f64], tau_g: &[f64]) -> Result<HoldTickOut, HoldError> {
        self.step += 1;
        self.hold.tick(HoldWorld {
            q,
            dq_meas: dq,
            tau_g,
            joints: &self.params,
            joint_names: &self.names,
            dt: PERIOD_S,
            hz: HZ,
            tick_count: self.step,
            wave: &mut self.wave,
        })
    }

    fn retarget(&mut self, joint_idx: usize, target: f64, q: f64) -> bool {
        self.hold.apply_retarget(HoldRetarget {
            joint_idx,
            clamped: target,
            requested: target,
            q,
            tick: self.step,
            dq_seed: Some(0.0),
            downward_seed: None,
        })
    }

    /// Tick a stationary single joint until the first error, at most `steps` ticks.
    fn stall_until_error(&mut self, q: f64, tau_g: f64, steps: u64) -> Option<(HoldError, u64)> {
        for _ in 0..steps {
            if let Err(err) = self.tick(&[q], &[0.0], &[tau_g]) {
                return Some((err, self.step));
            }
        }
        None
    }
}

// ---- L-berthier-02: the ascent-stall fuse is direction-agnostic -------------------------

#[test]
fn outbound_direction_follows_the_target_side_of_home() {
    // Positive side (unchanged).
    assert_eq!(outbound_stall_direction(0.5, 0.4), Some(1.0));
    assert_eq!(outbound_stall_direction(0.5, 0.031), Some(1.0));
    assert_eq!(
        outbound_stall_direction(0.5, 0.03),
        None,
        "inside the settle band"
    );
    assert_eq!(
        outbound_stall_direction(0.5, -0.4),
        None,
        "descent is not outbound"
    );
    // Negative side mirrors it.
    assert_eq!(outbound_stall_direction(-0.5, -0.4), Some(-1.0));
    assert_eq!(outbound_stall_direction(-0.5, -0.031), Some(-1.0));
    assert_eq!(outbound_stall_direction(-0.5, -0.03), None);
    assert_eq!(
        outbound_stall_direction(-0.5, 0.4),
        None,
        "moving back toward home is not outbound"
    );
    // Home and non-finite targets never arm it.
    assert_eq!(outbound_stall_direction(0.0, 0.4), None);
    assert_eq!(outbound_stall_direction(f64::NAN, 0.4), None);
}

#[test]
fn ascent_stall_covers_negative_outbound_target() {
    // Upper-arm-yaw-like joint: working range below home. Gravity-neutral model.
    let mut rig = Rig::new(1);
    rig.hold.arm(&[-0.1], &[-0.5], 0);
    let (err, step) = rig
        .stall_until_error(-0.1, 0.0, 600)
        .expect("a stalled negative outbound move must fuse");
    let HoldError::AscentStall { ms, trip, .. } = err else {
        panic!("expected AscentStall, got {err:?}");
    };
    assert!((POSITION_ASCENT_STALL_FAULT_MS..POSITION_ASCENT_STALL_FAULT_MS + 10).contains(&ms));
    assert!(step <= 401, "trip too late at step {step}");
    assert_eq!(trip.target, -0.5);
}

#[test]
fn ascent_stall_negative_outbound_with_mirrored_gravity_model() {
    // Gravity pulls +q, so holding torque is negative and pushes toward the target.
    let mut rig = Rig::new(1);
    rig.hold.arm(&[-0.1], &[-0.5], 0);
    let (err, _) = rig
        .stall_until_error(-0.1, -1.0, 600)
        .expect("mirrored joint must fuse");
    assert!(matches!(err, HoldError::AscentStall { .. }), "{err:?}");
}

#[test]
fn ascent_stall_ignores_a_wrong_gravity_sign() {
    // The fuse never consults τ_g: a model with the wrong sign cannot switch it off.
    for tau_g in [-3.0, 0.0, 3.0] {
        let mut rig = Rig::new(1);
        rig.hold.arm(&[0.2], &[0.8], 0);
        let (err, _) = rig
            .stall_until_error(0.2, tau_g, 600)
            .unwrap_or_else(|| panic!("no fuse for tau_g {tau_g}"));
        assert!(
            matches!(
                err,
                HoldError::AscentStall { .. } | HoldError::HoldTracking { .. }
            ),
            "tau_g {tau_g}: {err:?}"
        );
    }
}

#[test]
fn negative_progress_renews_the_budget_in_its_own_direction() {
    let mut rig = Rig::new(1);
    rig.hold.arm(&[-0.1], &[-0.9], 0);
    // Creeping 1 mrad per 10 ticks outbound (negative): each new low level renews the budget.
    for step in 0..1_500 {
        let q = -0.1 - 0.001 * f64::from(step / 10);
        rig.tick(&[q], &[-0.02], &[0.0])
            .unwrap_or_else(|err| panic!("progressing joint tripped at step {step}: {err:?}"));
    }
}

#[test]
fn outbound_direction_flip_starts_a_new_episode() {
    let mut rig = Rig::new(1);
    rig.hold.arm(&[0.1], &[0.6], 0);
    for _ in 0..300 {
        rig.tick(&[0.1], &[0.0], &[0.0]).unwrap();
    }
    assert!(rig.hold.ascent_stall_ms_at(0) >= 1_500);
    // Retarget to the other side of home: positive-direction credit means nothing there.
    assert!(rig.retarget(0, -0.6, 0.1));
    rig.tick(&[0.1], &[0.0], &[0.0]).unwrap();
    assert!(
        rig.hold.ascent_stall_ms_at(0) <= 10,
        "direction change restarts the episode, got {} ms",
        rig.hold.ascent_stall_ms_at(0)
    );
}

// ---- L-berthier-04 (index) / L8: retargets must not renew a safety budget ----------------

#[test]
fn small_retarget_stream_cannot_defeat_the_ascent_fuse() {
    let mut rig = Rig::new(1);
    rig.hold.arm(&[0.2], &[0.8], 0);
    let mut fault = None;
    for step in 1..=700_u64 {
        if step % 300 == 0 {
            // A 10 mrad trim every 1.5 s, alternating.
            let target = if (step / 300) % 2 == 1 { 0.81 } else { 0.8 };
            assert!(
                rig.retarget(0, target, 0.2),
                "trim must register as a retarget"
            );
        }
        if let Err(err) = rig.tick(&[0.2], &[0.0], &[0.0]) {
            fault = Some((err, step));
            break;
        }
    }
    let (err, step) = fault.expect("retarget stream kept a stalled joint unfused");
    assert!(matches!(err, HoldError::AscentStall { .. }), "{err:?}");
    assert!(step <= 410, "retargets delayed the fuse to step {step}");
}

#[test]
fn small_retarget_stream_cannot_defeat_hold_tracking() {
    let thr = rs03_progress_threshold();
    let mut rig = Rig::with_hold(PositionHold::with_progress_thresholds(vec![thr]), 1);
    rig.hold.arm(&[0.0], &[0.0], 0);
    let q = on_grid(-0.0357);
    let mut fault = None;
    for step in 1..=700_u64 {
        if step % 150 == 0 {
            let target = if (step / 150) % 2 == 1 { 0.01 } else { 0.0 };
            assert!(rig.retarget(0, target, q));
        }
        if let Err(err) = rig.tick(&[q], &[0.0], &[-1.6]) {
            fault = Some((err, step));
            break;
        }
    }
    let (err, step) = fault.expect("retarget stream kept a sagging joint unfused");
    assert!(matches!(err, HoldError::HoldTracking { .. }), "{err:?}");
    assert!(step <= 410, "retargets delayed the fuse to step {step}");
}

#[test]
fn apply_retarget_refuses_non_finite_input_without_touching_state() {
    let mut rig = Rig::new(1);
    rig.hold.arm(&[0.2], &[0.5], 7);
    for (target, q) in [
        (f64::NAN, 0.2),
        (f64::INFINITY, 0.2),
        (f64::NEG_INFINITY, 0.2),
        (0.6, f64::NAN),
    ] {
        assert!(!rig.retarget(0, target, q), "target {target} q {q}");
        assert_eq!(rig.hold.targets(), Some(&[0.5][..]));
        assert_eq!(rig.hold.retarget_tick_at(0), Some(7));
    }
}

// ---- L-berthier-13: integral is dropped outside its window --------------------------------

#[test]
fn stale_integral_does_not_reapply_on_window_reentry() {
    let mut rig = Rig::new(1);
    rig.params[0].ki = 5.0;
    // Residual 29 mrad: inside the integral window, below the outbound band.
    rig.hold.arm(&[0.0], &[0.029], 0);
    for _ in 0..400 {
        rig.tick(&[0.0], &[0.0], &[0.0]).unwrap();
    }
    assert!(rig.hold.integral_at(0) > 0.01, "integral must have accrued");
    // Leave the window (|e| >= 0.1) for one tick, then re-enter.
    rig.tick(&[0.5], &[0.0], &[0.0]).unwrap();
    assert_eq!(
        rig.hold.integral_at(0),
        0.0,
        "integral dropped outside window"
    );
    rig.tick(&[0.0], &[0.0], &[0.0]).unwrap();
    assert!(
        rig.hold.integral_at(0) < 0.001,
        "stale integral re-applied: {}",
        rig.hold.integral_at(0)
    );
}

// ---- L-berthier-03 (+ G5): wave joint fuse, exemptions -------------------------------------

fn wave_q(shadow: &mut PositionWave, tick: u64, last: f64) -> f64 {
    shadow
        .target_and_velocity_at_tick(tick, HZ)
        .map_or(last, |(target, _)| target)
}

#[test]
fn wave_with_a_jammed_joint_trips_wave_stall() {
    let mut rig = Rig::new(1);
    rig.hold.arm(&[0.3], &[0.3], 0);
    rig.wave = Some(PositionWave::new(0, 0.2, 0.6, 0, 400, 3));
    let (err, step) = rig
        .stall_until_error(0.3, 0.0, 1_000)
        .expect("a jammed wave joint must fuse");
    let HoldError::WaveStall { joint, ms, .. } = err else {
        panic!("expected WaveStall, got {err:?}");
    };
    assert_eq!(joint, "j0");
    assert!((POSITION_ASCENT_STALL_FAULT_MS..POSITION_ASCENT_STALL_FAULT_MS + 10).contains(&ms));
    assert!(step <= 520, "trip too late at step {step}");
}

#[test]
fn healthy_wave_never_trips_any_fuse() {
    for half_period_ticks in [200_u64, 400, 800] {
        let wave = PositionWave::new(0, 0.2, 0.6, 0, half_period_ticks, 3);
        let mut shadow = wave.clone();
        let mut rig = Rig::new(1);
        rig.hold.arm(&[0.2], &[0.2], 0);
        rig.wave = Some(wave);
        let mut q = 0.2;
        let total = half_period_ticks * 2 * 3 + 50;
        for tick in 1..=total {
            let next = wave_q(&mut shadow, tick, q);
            let dq = (next - q) / PERIOD_S;
            q = next;
            rig.tick(&[q], &[dq], &[0.0]).unwrap_or_else(|err| {
                panic!("healthy wave (half period {half_period_ticks}) tripped at {tick}: {err:?}")
            });
        }
    }
}

#[test]
fn wave_joint_is_exempt_from_hold_tracking() {
    // q tracks the wave 50 mrad behind, under a model whose net torque opposes the target.
    // Off a wave, this arms HoldTracking and trips within 2 s; on a wave joint it must not.
    let wave = PositionWave::new(0, 0.2, 0.6, 0, 800, 1);
    let mut shadow = wave.clone();
    let mut rig = Rig::new(1);
    rig.hold.arm(&[0.15], &[0.15], 0);
    rig.wave = Some(wave);
    let mut q = 0.15;
    let mut opposed_ticks = 0;
    for tick in 1..=1_500 {
        let target_q = wave_q(&mut shadow, tick, q + 0.05) - 0.05;
        let dq = (target_q - q) / PERIOD_S;
        q = target_q;
        let out = rig
            .tick(&[q], &[dq], &[-3.0])
            .unwrap_or_else(|err| panic!("wave joint tripped at {tick}: {err:?}"));
        let d = &out.diag[0];
        assert_eq!(
            d.hold_tracking_ms, 0,
            "wave joint never accrues tracking budget"
        );
        if hold_tracking_opposed(d.q, d.target, d.tau_p + d.tau_ff_cmd) {
            opposed_ticks += 1;
        }
    }
    assert!(
        opposed_ticks > 1_000,
        "scenario must really oppose the target (got {opposed_ticks} ticks)"
    );
}

#[test]
fn uncommanded_peer_is_exempt_beside_a_wave_joint_but_commanded_peer_trips() {
    let run = |peer_commanded: bool| -> Option<(HoldError, u64)> {
        let wave = PositionWave::new(0, 0.2, 0.6, 0, 800, 1);
        let mut shadow = wave.clone();
        let mut rig = Rig::new(2);
        let sag = on_grid(-0.0357);
        rig.hold.arm(&[0.2, 0.0], &[0.2, 0.0], 0);
        rig.hold.set_commanded_joint(1, peer_commanded);
        rig.wave = Some(wave);
        let mut q0 = 0.2;
        for tick in 1..=900_u64 {
            let next = wave_q(&mut shadow, tick, q0);
            let dq0 = (next - q0) / PERIOD_S;
            q0 = next;
            if let Err(err) = rig.tick(&[q0, sag], &[dq0, 0.0], &[0.0, -1.6]) {
                return Some((err, tick));
            }
        }
        None
    };
    assert!(
        run(false).is_none(),
        "an uncommanded peer must not fault the wave owner"
    );
    let (err, _) = run(true).expect("the commanded peer sags under opposing torque");
    assert!(
        matches!(&err, HoldError::HoldTracking { joint, .. } if joint == "j1"),
        "{err:?}"
    );
}

// ---- L-berthier-19 / G2: Berthier clamps planner speed to the Davout cap -------------------

fn max_planner_speed(cap: Option<f64>) -> (f64, f64) {
    let mut rig = Rig::new(1);
    rig.params[0].velocity_cap = cap;
    rig.hold.arm(&[0.0], &[1.0], 0);
    let (mut q, mut max_dq, mut max_v_max_eff) = (0.0, 0.0_f64, 0.0_f64);
    for _ in 0..300 {
        let out = rig.tick(&[q], &[0.0], &[0.0]).unwrap();
        let d = &out.diag[0];
        max_dq = max_dq.max(d.dq_traj.abs());
        max_v_max_eff = max_v_max_eff.max(d.v_max_eff);
        q = d.q_traj;
    }
    (max_dq, max_v_max_eff)
}

#[test]
fn planner_speed_never_exceeds_the_velocity_cap() {
    let (capped_dq, capped_v) = max_planner_speed(Some(0.5));
    assert!(
        capped_dq <= 0.5 + 1e-9,
        "planner speed {capped_dq} above cap 0.5"
    );
    assert!(capped_v <= 0.5 + 1e-9, "v_max_eff {capped_v} above cap 0.5");
    let (free_dq, _) = max_planner_speed(None);
    assert!(
        free_dq > 1.0,
        "control: without a cap the same move reaches {free_dq} rad/s"
    );
}
