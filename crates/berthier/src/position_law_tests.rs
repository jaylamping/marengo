//! ADR 0039 test plan, Berthier unit tests: reference continuity under retarget storms,
//! friction shape, and an integral that never steps.

use super::*;

const DT: f64 = 0.005;
const V_MAX: f64 = 1.25;
const A_MAX: f64 = 4.5;
const E1: f64 = 0.12;
const E0: f64 = E1 / 4.0;

fn friction() -> ReferenceFriction {
    ReferenceFriction {
        fc: 0.08,
        fs: 0.14,
        fv: 0.01,
        fo: 0.0,
        k: 10.0,
        v_b: 0.05,
    }
}

fn gains() -> ScaledPdGains {
    ScaledPdGains {
        kd: 3.0,
        e0: E0,
        e1: E1,
        integral_band: 0.1,
        integral_leak_s: 0.5,
        friction_error_gain: 0.0,
        inertia: 0.0,
        friction: Some(friction()),
    }
}

/// Deterministic xorshift so storms are reproducible without a dependency.
struct Rng(u64);

impl Rng {
    fn next_unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn retarget_storm_keeps_reference_continuous_and_lead_bounded_on_a_stuck_plant() {
    for seed in 1..=8_u64 {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ seed);
        let q = 0.3; // stuck plant: measured q never moves
        let (mut q_ref, mut v_ref) = (q, 0.0);
        let mut state = ScaledPdState::default();
        let mut target = q;
        let mut max_dv: f64 = 0.0;
        let mut max_dq: f64 = 0.0;
        let mut max_lead: f64 = 0.0;
        for tick in 0..4000 {
            if tick % 23 == 0 || rng.next_unit() < 0.02 {
                target = q + (rng.next_unit() - 0.5) * 2.0;
            }
            let (q_next, v_next, _) = advance_reference(
                &mut state,
                &gains(),
                q_ref,
                v_ref,
                q,
                target,
                V_MAX,
                A_MAX,
                DT,
            );
            max_dv = max_dv.max((v_next - v_ref).abs());
            max_dq = max_dq.max((q_next - q_ref).abs());
            q_ref = q_next;
            v_ref = v_next;
            max_lead = max_lead.max((q_ref - q).abs());
        }
        assert!(max_dv <= A_MAX * DT + 1e-9, "seed {seed}: |Δv| {max_dv}");
        assert!(
            max_dq <= V_MAX * DT + 1e-4,
            "seed {seed}: |Δq_ref| {max_dq}"
        );
        // A reference already moving when its lead starts growing brakes at a_max.
        let bound = E1 + V_MAX * V_MAX / (2.0 * A_MAX) + V_MAX * DT;
        assert!(max_lead <= bound, "seed {seed}: lead {max_lead} > {bound}");
    }
}

#[test]
fn stuck_joint_from_rest_holds_a_stationary_reference_near_e1() {
    let q = 0.0;
    let (mut q_ref, mut v_ref) = (q, 0.0);
    let mut state = ScaledPdState::default();
    for _ in 0..2000 {
        (q_ref, v_ref, _) =
            advance_reference(&mut state, &gains(), q_ref, v_ref, q, 1.0, V_MAX, A_MAX, DT);
    }
    assert_eq!(v_ref, 0.0, "reference stops");
    assert_eq!(state.s, 0.0);
    // From rest the reference reaches e0 at sqrt(2·a·e0), so it brakes within another e0.
    assert!(q_ref > E0 && q_ref <= E1 + E0, "lead {q_ref}");
    // The joint breaks free: the lead closes and the reference resumes without a step.
    let q_moved = q_ref - E0;
    let (_, v_next, _) = advance_reference(
        &mut state,
        &gains(),
        q_ref,
        v_ref,
        q_moved,
        1.0,
        V_MAX,
        A_MAX,
        DT,
    );
    assert!(v_next > 0.0 && v_next <= A_MAX * DT + 1e-12);
}

#[test]
fn retarget_back_toward_a_stuck_joint_is_never_frozen() {
    // Reference parked e1 ahead of a stuck joint; the operator retargets behind it.
    let q = 0.0;
    let (mut q_ref, mut v_ref) = (q, 0.0);
    let mut state = ScaledPdState::default();
    for _ in 0..2000 {
        (q_ref, v_ref, _) =
            advance_reference(&mut state, &gains(), q_ref, v_ref, q, 1.0, V_MAX, A_MAX, DT);
    }
    for _ in 0..2000 {
        (q_ref, v_ref, _) = advance_reference(
            &mut state,
            &gains(),
            q_ref,
            v_ref,
            q,
            -0.05,
            V_MAX,
            A_MAX,
            DT,
        );
    }
    assert_eq!((q_ref, v_ref), (-0.05, 0.0));
}

#[test]
fn reversal_and_short_retarget_never_snap_velocity() {
    // Cruise at v_max, then a target just ahead, then one behind.
    let (mut q, mut v) = (0.0, V_MAX);
    for target in [0.01, -0.5] {
        for _ in 0..400 {
            let (q_next, v_next, _) = reference_step(q, v, target, V_MAX, A_MAX, DT);
            assert!((v_next - v).abs() <= A_MAX * DT + 1e-12, "{v} -> {v_next}");
            q = q_next;
            v = v_next;
        }
    }
    assert_eq!((q, v), (-0.5, 0.0));
}

#[test]
fn friction_is_odd_continuous_and_zero_at_rest() {
    let f = friction();
    assert_eq!(f.torque(0.0), 0.0);
    let mut previous = f.torque(-2.0);
    let mut v = -2.0;
    while v < 2.0 {
        let next_v = v + 1e-4;
        let next = f.torque(next_v);
        assert!((f.torque(v) + f.torque(-v)).abs() < 1e-15, "odd at {v}");
        // Lipschitz bound: tanh slope k·fs plus fv, plus the Stribeck slope.
        assert!((next - previous).abs() <= 1e-4 * (f.k * f.fs + f.fv + f.fs / f.v_b) + 1e-12);
        previous = next;
        v = next_v;
    }
    // Breakaway excess at low speed (above the same law without a Stribeck term), Coulomb at
    // speed.
    let coulomb = ReferenceFriction { fs: f.fc, ..f };
    assert!(f.torque(0.05) > coulomb.torque(0.05) + 0.01);
    assert!((f.torque(2.0) - (f.fc + f.fv * 2.0)).abs() < 1e-6);
}

#[test]
fn friction_defaults_have_no_stribeck_term() {
    let gains = FrictionGains {
        fc: 0.08,
        fv: 0.0,
        fo: 0.0,
        k: 10.0,
        fs: None,
        v_b: None,
    };
    let f = ReferenceFriction::from_gains(&gains, None);
    assert_eq!(f.fs, f.fc);
    assert!((f.torque(0.05) - 0.08 * (0.5_f64).tanh()).abs() < 1e-15);
    // An override below the configured fs keeps fs; above it raises fs to the override.
    let raised = ReferenceFriction::from_gains(&gains, Some(0.2));
    assert_eq!((raised.fc, raised.fs), (0.2, 0.2));
}

#[test]
fn leaky_integral_never_steps() {
    let ki = 5.0;
    let band = 0.1;
    let mut tau_i = 0.0;
    let mut max_step: f64 = 0.0;
    // Inside the band, then out of it, then a large error, then back.
    let errors = (0..400)
        .map(|_| 0.05)
        .chain((0..400).map(|_| 0.5))
        .chain((0..400).map(|_| -0.08))
        .chain((0..400).map(|_| 0.0));
    for e in errors {
        let next = integral_step(tau_i, e, ki, band, 0.5, DT);
        max_step = max_step.max((next - tau_i).abs());
        assert!(next.abs() <= SCALED_PD_INTEGRAL_MAX_NM);
        tau_i = next;
    }
    // Largest increment is ki·band·dt; the largest leak step is cap·dt/τ.
    let bound = (ki * band * DT).max(SCALED_PD_INTEGRAL_MAX_NM * DT / 0.5);
    assert!(max_step <= bound + 1e-12, "step {max_step} > {bound}");
    // With ki = 0 the integral only decays.
    let decayed = integral_step(0.4, 0.01, 0.0, band, 0.5, DT);
    assert!(decayed < 0.4 && decayed > 0.39);
}

#[test]
fn governor_slows_only_a_growing_lead() {
    assert_eq!(governor_scale(0.02, 0.0, 1.0, E0, E1), 1.0);
    assert!((governor_scale(0.075, 0.0, 1.0, E0, E1) - 0.5).abs() < 1e-12);
    assert_eq!(governor_scale(0.2, 0.0, 1.0, E0, E1), 0.0);
    // Closing the lead (target behind the reference, or joint ahead) is never slowed.
    assert_eq!(governor_scale(0.2, 0.0, -1.0, E0, E1), 1.0);
    assert_eq!(governor_scale(0.0, 0.2, 1.0, E0, E1), 1.0);
}

#[test]
fn feedforward_uses_reference_velocity_not_measurement() {
    let mut state = ScaledPdState::default();
    let at_rest = compose_feedforward(&mut state, &gains(), 0.0, 0.0, 0.0, 1.5, DT);
    assert_eq!(at_rest.tau_fric, 0.0);
    assert_eq!(at_rest.tau_ff, 1.5);
    // A steady reference velocity (no acceleration): τ_dyn settles on τ_fric(v_c).
    state.v_c = 0.5;
    state.v_prev = 0.5;
    let mut moving = at_rest;
    for _ in 0..100 {
        moving = compose_feedforward(&mut state, &gains(), 0.0, 0.0, 0.0, 1.5, DT);
    }
    assert!((moving.tau_ff - (1.5 + friction().torque(0.5))).abs() < 1e-15);
}

#[test]
fn acceleration_feedforward_is_j_times_a_and_never_steps_tau_ff() {
    // A reference that accelerates at 1.5 rad/s², cruises, then brakes at 1.5 rad/s² (the
    // pitch trapezoid): τ_dyn reaches τ_fric(v) + J·a while |a| is held, and never moves
    // more than the slew limit per tick, although J·a itself steps by J·a_max = 0.21 Nm.
    let inertia = 0.14;
    let a_max = 1.5;
    let g = ScaledPdGains { inertia, ..gains() };
    let mut state = ScaledPdState::default();
    let mut last = compose_feedforward(&mut state, &g, 0.0, 0.0, 0.0, 0.0, DT).tau_ff;
    let mut profile = vec![a_max; 100];
    profile.extend(vec![0.0; 50]);
    profile.extend(vec![-a_max; 100]);
    let step_max = SCALED_PD_DYNAMIC_FF_RATE_NM_S * DT;
    for (n, a) in profile.iter().enumerate() {
        state.v_c += a * DT;
        let ff = compose_feedforward(&mut state, &g, 0.0, 0.0, 0.0, 0.0, DT);
        assert!(
            (ff.tau_ff - last).abs() <= step_max + 1e-12,
            "tick {n}: τ_ff step {}",
            ff.tau_ff - last
        );
        last = ff.tau_ff;
        if n == 99 || n == 249 {
            let want = friction().torque(state.v_c) + inertia * a;
            assert!(
                (ff.tau_fric - want).abs() < 1e-9,
                "tick {n}: {} vs {want}",
                ff.tau_fric
            );
        }
    }
}
