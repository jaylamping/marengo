//! ADR 0039 scaled-PD position law: drive-side PD on a feedback-governed reference.
//!
//! Selected per joint by `control.yaml` `position_law: scaled_pd` (ADR 0039 Phase 3). The
//! legacy law in [`crate::position_hold`] stays the default until the Phase 4 cutover.
//!
//! Per joint and tick:
//!
//! ```text
//! e     = q_ref − q
//! s     = clamp(1 − (|e| − e0)/(e1 − e0), 0, 1)  while moving toward the target grows |e|,
//!         else 1
//! (q_ref, v_ref) ← trapezoid step toward the target with speed limit s·v_max, |Δv_ref| ≤ a_max·dt
//! MIT:  position = clamp_envelope(q_ref), velocity = v_ref, kp, kd (constant),
//!       τ_ff = τ_g + τ_dyn + fo + τ_I
//! τ_dyn → τ_fric(v_ref) + J·a_ref + H·(v_ref − v̂), moving at most SCALED_PD_DYNAMIC_FF_RATE_NM_S·dt
//!         per tick (v̂: measured velocity low-passed over `velocity_filter_s`; H = 0 by default)
//! τ_I   ← band integral of the target error (default), or a leaky reference-error integral
//!         frozen after a reshaped output (ADR 0040)
//! τ_ff  → whole-feed-forward step guard when configured (ADR 0040)
//! ```
//!
//! The time scale `s` governs the reference *speed limit*, not its clock: the reference keeps
//! braking and reversing in real time, so a retarget back toward a stuck joint is never frozen,
//! and the commanded velocity changes by at most `a_max·dt` per tick.
//!
//! Damping is the drive's `kd·(v_ref − dq)`; ADR 0040 adds an optional small host damper on a
//! filtered, timestamped position-derived velocity, always on (no deadband or onset switch).
//!
//! `a_ref` is the reference's own velocity change per tick, so the feed-forward supplies the
//! torque the reference's acceleration and braking need instead of a P lead of `J·a/kp` (a stop
//! overshoot of that size once friction holds the joint). The trapezoid's acceleration steps
//! between 0 and ±a_max, and the friction slope at rest is `fs·k`; the rate limit keeps both
//! from stepping τ_ff by more than the bench's per-tick bound.

use marengo_config::FrictionGains;

use crate::position_trajectory::TrapezoidPhase;

/// Reference position tolerance for arrival (same as the legacy trapezoid).
const REFERENCE_TOLERANCE_RAD: f64 = 1e-4;

/// Ceiling on the integral torque `|τ_I|` (Nm), shared with the legacy law.
pub const SCALED_PD_INTEGRAL_MAX_NM: f64 = 0.5;

/// Slew limit of the dynamic feed-forward `τ_fric + J·a` (Nm/s): 0.03 Nm per 5 ms tick, so
/// with the gravity term's own change (≤ A·v_max·dt ≈ 0.017 Nm on the pitch) one tick's τ_ff
/// step stays under the 0.05 Nm bench bound.
pub const SCALED_PD_DYNAMIC_FF_RATE_NM_S: f64 = 6.0;

/// Smooth friction feed-forward evaluated on the reference velocity (ADR 0039).
///
/// `τ_fric(v) = (fc + (fs − fc)·exp(−|v|/v_b))·tanh(k·v) + fv·v` (`k = 1/v_s`), plus the
/// constant `fo` offset kept from the legacy model. `τ_fric(0) = 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceFriction {
    pub fc: f64,
    /// Static friction; never below `fc` (a lower override `fc` raises it to `fc`).
    pub fs: f64,
    pub fv: f64,
    pub fo: f64,
    /// tanh steepness (1/rad/s); `0` disables the Coulomb and Stribeck terms.
    pub k: f64,
    /// Stribeck velocity (rad/s), `> 0`.
    pub v_b: f64,
}

impl ReferenceFriction {
    /// Resolve from config gains; `fc_override` is a Testing gain override of `fc`.
    pub fn from_gains(gains: &FrictionGains, fc_override: Option<f64>) -> Self {
        let fc = fc_override.unwrap_or(gains.fc);
        Self {
            fc,
            fs: gains.static_nm().max(fc),
            fv: gains.fv,
            fo: gains.fo,
            k: gains.k,
            v_b: gains.stribeck_velocity_rad_s(),
        }
    }

    /// Velocity-dependent friction torque (odd in `v`, continuous, zero at rest).
    pub fn torque(&self, v: f64) -> f64 {
        let shape = if self.k > 0.0 {
            (self.k * v).tanh()
        } else {
            0.0
        };
        let stribeck = if self.v_b > 0.0 {
            (self.fs - self.fc) * (-v.abs() / self.v_b).exp()
        } else {
            0.0
        };
        (self.fc + stribeck) * shape + self.fv * v
    }
}

/// How the scaled-PD integral `τ_I` integrates (ADR 0040).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegralMode {
    /// ADR 0039: integrate `ki·(target − q)` inside the band, leak only outside it.
    TargetBand,
    /// ADR 0040: leak every tick, integrate `ki·(q_ref − q)` inside the band unless the previous
    /// output was reshaped (step guard, envelope or Davout), so the state is a bounded
    /// disturbance estimate rather than an ever-growing push.
    LeakyReference,
}

impl From<marengo_config::PositionIntegralMode> for IntegralMode {
    fn from(mode: marengo_config::PositionIntegralMode) -> Self {
        match mode {
            marengo_config::PositionIntegralMode::TargetBand => Self::TargetBand,
            marengo_config::PositionIntegralMode::LeakyReference => Self::LeakyReference,
        }
    }
}

/// Per-joint scaled-PD parameters resolved for one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledPdGains {
    /// Wire kd sent every tick (override > ramp > YAML; constant once the mode ramp ends).
    pub kd: f64,
    /// Full-rate band (rad).
    pub e0: f64,
    /// Stop band (rad), `> e0`.
    pub e1: f64,
    /// Integral input band (rad): `|target − q|` ([`IntegralMode::TargetBand`]) or
    /// `|q_ref − q|` ([`IntegralMode::LeakyReference`]).
    pub integral_band: f64,
    /// Integral leak time constant (s): outside the band ([`IntegralMode::TargetBand`]), or
    /// every tick ([`IntegralMode::LeakyReference`]).
    pub integral_leak_s: f64,
    pub integral_mode: IntegralMode,
    /// `|τ_I|` ceiling (Nm), at most [`SCALED_PD_INTEGRAL_MAX_NM`].
    pub integral_cap_nm: f64,
    /// Inertia (kg·m²) the reference acceleration is fed forward with; `0` disables `J·a`.
    pub inertia: f64,
    /// Host damping `H` (Nm·s/rad) on `v_ref − v̂`, inside the slewed dynamic term; `0` = none.
    pub host_damping: f64,
    /// Time constant (s) of the first-order filter giving `v̂` from measured velocity.
    pub velocity_filter_s: f64,
    /// Largest change of the whole τ_ff between sent ticks (Nm); `None` = no guard.
    pub ff_step_max_nm: Option<f64>,
    /// `None` when the joint has no friction model (no friction feed-forward).
    pub friction: Option<ReferenceFriction>,
}

/// Mutable per-joint law state beside the shared reference planner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledPdState {
    /// Time scale `s ∈ [0, 1]` applied to the reference speed limit on the latest advance.
    pub s: f64,
    /// Commanded reference velocity from the latest advance (rad/s).
    pub v_c: f64,
    /// Integral torque `τ_I` (Nm), stored in torque so gain changes never step it.
    pub tau_i: f64,
    /// Reference velocity at the previous feed-forward (rad/s), for `a_ref`.
    pub v_prev: f64,
    /// Slew-limited `τ_fric + J·a` (Nm).
    pub tau_dyn: f64,
    /// Filtered measured velocity `v̂` (rad/s); `None` until the first finite sample.
    pub v_hat: Option<f64>,
    /// Whole τ_ff sent on the previous tick, for the step guard (`None` on mode entry).
    pub ff_prev: Option<f64>,
    /// The previous output was reshaped by the law itself (step guard or envelope clamp).
    pub reshaped_local: bool,
}

impl Default for ScaledPdState {
    fn default() -> Self {
        Self {
            s: 1.0,
            v_c: 0.0,
            tau_i: 0.0,
            v_prev: 0.0,
            tau_dyn: 0.0,
            v_hat: None,
            ff_prev: None,
            reshaped_local: false,
        }
    }
}

/// Feed-forward parts of one scaled-PD tick (Nm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledPdFeedforward {
    /// Slew-limited `τ_fric(v_c) + J·a_c`, plus `fo`.
    pub tau_fric: f64,
    pub tau_i: f64,
    /// `τ_g + tau_fric + tau_i`, after the step guard.
    pub tau_ff: f64,
    /// The step guard limited this tick's τ_ff.
    pub ff_guarded: bool,
}

/// One tick's inputs to [`compose_feedforward`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedforwardInput {
    pub ki: f64,
    /// `target − q` (rad).
    pub target_error: f64,
    /// `q_ref − q` (rad).
    pub reference_error: f64,
    /// Davout's measured joint velocity (rad/s): the position difference over receive times.
    pub dq_meas: f64,
    /// Davout reshaped the previous command (total-torque clamp or τ_ff cap/rate limit).
    pub reshaped_downstream: bool,
    /// τ_ff Davout last sent for this joint, to start the step guard bumplessly.
    pub ff_last_sent: Option<f64>,
    pub tau_g: f64,
    pub dt: f64,
}

/// Time scale for reference error magnitude `error_abs`: 1 up to `e0`, 0 from `e1`.
pub fn time_scale_target(error_abs: f64, e0: f64, e1: f64) -> f64 {
    if !error_abs.is_finite() {
        return 0.0;
    }
    if e1 <= e0 {
        return if error_abs <= e0 { 1.0 } else { 0.0 };
    }
    (1.0 - (error_abs - e0) / (e1 - e0)).clamp(0.0, 1.0)
}

/// Governor scale for this tick: [`time_scale_target`] of `|q_ref − q|` while advancing toward
/// the target would grow that lead, else 1 (closing the lead is never slowed).
pub fn governor_scale(q_ref: f64, q: f64, target: f64, e0: f64, e1: f64) -> f64 {
    let lead = q_ref - q;
    if lead * (target - q_ref) <= 0.0 {
        return 1.0;
    }
    time_scale_target(lead.abs(), e0, e1)
}

/// Highest speed from which per-tick braking by `dv` (position updated with the new speed)
/// stops within `dist`: the discrete form of `sqrt(2·a·dist)`, so a reference that rides it
/// lands on the target instead of hunting around it.
fn braking_speed(dist: f64, dv: f64, a_max: f64) -> f64 {
    (dv * dv / 4.0 + 2.0 * a_max * dist).sqrt() - dv / 2.0
}

/// One reference step toward `target` that never changes velocity by more than `a_max·dt`.
///
/// The speed along the target direction moves by at most `a_max·dt` toward
/// `min(v_max, braking_speed(distance))`. Unlike the legacy trapezoid it never snaps velocity: a
/// reference moving away from the target (a reversing retarget) brakes through zero, a lowered
/// `v_max` bleeds at `a_max`, and a reference that cannot stop in time (a short retarget at
/// speed) decelerates at `a_max`, overshoots and returns. It snaps to `(target, 0)` only once
/// the remaining speed is within one tick of acceleration.
///
/// Fails closed like the legacy trapezoid: a non-positive `v_max` means no cruise speed, and an
/// invalid `a_max` leaves the reference where it is.
pub fn reference_step(
    q: f64,
    v: f64,
    target: f64,
    v_max: f64,
    a_max: f64,
    dt: f64,
) -> (f64, f64, TrapezoidPhase) {
    let v_max = if v_max.is_finite() && v_max > 0.0 {
        v_max
    } else {
        0.0
    };
    let remaining = target - q;
    let dist = remaining.abs();
    if !(a_max.is_finite() && a_max > 0.0) {
        return if dist <= REFERENCE_TOLERANCE_RAD {
            (target, 0.0, TrapezoidPhase::Hold)
        } else {
            (q, 0.0, TrapezoidPhase::Accelerate)
        };
    }
    let dv = a_max * dt;
    if dist <= REFERENCE_TOLERANCE_RAD && v.abs() <= dv {
        return (target, 0.0, TrapezoidPhase::Hold);
    }
    // At the target with speed left, brake against the motion.
    let dir = if dist <= REFERENCE_TOLERANCE_RAD {
        -v.signum()
    } else {
        remaining.signum()
    };
    let v_along = v * dir;
    let desired = v_max.min(braking_speed(dist, dv, a_max));
    let v_along_new = desired.max(v_along - dv).min(v_along + dv);
    let phase = if v_along_new < v_along - 1e-12 {
        TrapezoidPhase::Decelerate
    } else if v_max > 0.0 && v_along_new >= v_max - 1e-9 {
        TrapezoidPhase::Cruise
    } else {
        TrapezoidPhase::Accelerate
    };
    let v_new = dir * v_along_new;
    let q_new = q + v_new * dt;
    let arrived =
        (target - q_new) * remaining <= 0.0 || (target - q_new).abs() <= REFERENCE_TOLERANCE_RAD;
    // Snap only from a speed within one tick of rest, so the snap itself is a bounded step.
    if dist > REFERENCE_TOLERANCE_RAD && arrived && v.abs() <= dv {
        return (target, 0.0, TrapezoidPhase::Hold);
    }
    (q_new, v_new, phase)
}

/// Advance the leaky integral one tick and return the new `τ_I` (Nm).
///
/// Inside the band it accumulates `ki·e·dt`; outside it (or with `ki = 0`) it decays with time
/// constant `leak_s`. Always within `±SCALED_PD_INTEGRAL_MAX_NM`; it never resets.
pub fn integral_step(
    tau_i: f64,
    target_error: f64,
    ki: f64,
    band: f64,
    leak_s: f64,
    dt: f64,
) -> f64 {
    let next = if ki > 0.0 && target_error.abs() < band {
        tau_i + ki * target_error * dt
    } else if leak_s > 0.0 {
        tau_i * (-dt / leak_s).exp()
    } else {
        tau_i
    };
    if next.is_finite() {
        next.clamp(-SCALED_PD_INTEGRAL_MAX_NM, SCALED_PD_INTEGRAL_MAX_NM)
    } else {
        0.0
    }
}

/// Advance the leaky reference-error integral one tick (ADR 0040): always decay with `leak_s`,
/// add `ki·e_ref·dt` only inside the band and when the previous output was not reshaped, and
/// keep `|τ_I| ≤ cap`. A constant error settles at `ki·leak_s·e_ref` instead of ramping.
#[allow(clippy::too_many_arguments)]
pub fn leaky_integral_step(
    tau_i: f64,
    reference_error: f64,
    ki: f64,
    band: f64,
    leak_s: f64,
    cap: f64,
    frozen: bool,
    dt: f64,
) -> f64 {
    let mut next = if leak_s > 0.0 {
        tau_i * (-dt / leak_s).exp()
    } else {
        tau_i
    };
    if ki > 0.0 && !frozen && reference_error.abs() < band {
        next += ki * reference_error * dt;
    }
    let cap = cap.clamp(0.0, SCALED_PD_INTEGRAL_MAX_NM);
    if next.is_finite() {
        next.clamp(-cap, cap)
    } else {
        0.0
    }
}

/// Govern the speed limit from the current lead, then advance the reference one tick.
///
/// Returns the new `(q_ref, v_ref, phase)`; `state.s` and `state.v_c` are updated.
#[allow(clippy::too_many_arguments)]
pub fn advance_reference(
    state: &mut ScaledPdState,
    gains: &ScaledPdGains,
    q_ref: f64,
    v_ref: f64,
    q: f64,
    target: f64,
    v_max: f64,
    a_max: f64,
    dt: f64,
) -> (f64, f64, TrapezoidPhase) {
    state.s = governor_scale(q_ref, q, target, gains.e0, gains.e1);
    let (q_next, v_next, phase) = reference_step(q_ref, v_ref, target, state.s * v_max, a_max, dt);
    state.v_c = v_next;
    (q_next, v_next, phase)
}

/// Feed-forward for one tick: `τ_g + τ_dyn + τ_I`, where `τ_dyn` slews by at most
/// [`SCALED_PD_DYNAMIC_FF_RATE_NM_S`]`·dt` toward `τ_fric(v_c) + fo + J·a_c + H·(v_c − v̂)` with
/// `a_c = (v_c − v_prev)/dt`, then the whole τ_ff step guard when configured.
pub fn compose_feedforward(
    state: &mut ScaledPdState,
    gains: &ScaledPdGains,
    input: &FeedforwardInput,
) -> ScaledPdFeedforward {
    let dt = input.dt;
    let friction = gains
        .friction
        .map_or(0.0, |friction| friction.torque(state.v_c) + friction.fo);
    let accel = if dt > 0.0 {
        (state.v_c - state.v_prev) / dt
    } else {
        0.0
    };
    state.v_prev = state.v_c;
    let damping = if gains.host_damping > 0.0 && input.dq_meas.is_finite() {
        let v_hat = match state.v_hat {
            Some(v_hat) if gains.velocity_filter_s > 0.0 && dt > 0.0 => {
                v_hat + (1.0 - (-dt / gains.velocity_filter_s).exp()) * (input.dq_meas - v_hat)
            }
            Some(_) => input.dq_meas,
            None => input.dq_meas,
        };
        state.v_hat = Some(v_hat);
        gains.host_damping * (state.v_c - v_hat)
    } else {
        0.0
    };
    let target = friction + gains.inertia * accel + damping;
    let step = SCALED_PD_DYNAMIC_FF_RATE_NM_S * dt.max(0.0);
    let next = state.tau_dyn + (target - state.tau_dyn).clamp(-step, step);
    state.tau_dyn = if next.is_finite() { next } else { 0.0 };
    state.tau_i = match gains.integral_mode {
        IntegralMode::TargetBand => integral_step(
            state.tau_i,
            input.target_error,
            input.ki,
            gains.integral_band,
            gains.integral_leak_s,
            dt,
        ),
        IntegralMode::LeakyReference => leaky_integral_step(
            state.tau_i,
            input.reference_error,
            input.ki,
            gains.integral_band,
            gains.integral_leak_s,
            gains.integral_cap_nm,
            state.reshaped_local || input.reshaped_downstream,
            dt,
        ),
    };
    let wanted = input.tau_g + state.tau_dyn + state.tau_i;
    let (tau_ff, ff_guarded) = match gains.ff_step_max_nm {
        Some(max_step) if max_step > 0.0 && wanted.is_finite() => {
            let previous = state.ff_prev.or(input.ff_last_sent).unwrap_or(wanted);
            let sent = previous + (wanted - previous).clamp(-max_step, max_step);
            (sent, (sent - wanted).abs() > 1e-12)
        }
        _ => (wanted, false),
    };
    state.ff_prev = Some(tau_ff);
    ScaledPdFeedforward {
        tau_fric: state.tau_dyn,
        tau_i: state.tau_i,
        tau_ff,
        ff_guarded,
    }
}

#[cfg(test)]
#[path = "position_law_tests.rs"]
mod tests;
