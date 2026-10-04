//! Mode isolation property tests — prove gravity comp changes don't affect other modes.
//!
//! These tests verify the CRITICAL requirement: perturbing tau_g (e.g., COM correction,
//! algorithm changes, payload estimation) must NOT change the non-gravity feedforward
//! components of Impedance or Position modes.
//!
//! Scope:
//! - Impedance friction mode selection is tested through actual controller output
//!   in `tests/friction_mode_output.rs`; this module tests Position composition.
//! - Position mode non-gravity FF = `tau_f + tau_d` from
//!   `compose_position_hold_feedforward`.
//! - GravityComp mode is pure feedforward (`tau_ff = tau_g`).
//!
//! Strategy: for a fixed `(q, dq, config)`, compute the non-gravity FF components with
//! two different `tau_g` values. If the non-gravity components are identical, the modes
//! are isolated from gravity comp changes.

#![allow(clippy::expect_used)]

use proptest::prelude::*;

use crate::position_feedforward::{compose_position_hold_feedforward, PositionHoldFeedforward};
use crate::position_trajectory::TrapezoidPhase;
use marengo_config::FrictionGains;

proptest! {
    #[test]
    fn position_non_gravity_ff_independent_of_tau_g(
        tau_g in -5.0..5.0f64,
        tau_g_perturbed in -5.0..5.0f64,
        kd in 0.0..20.0f64,
        dq_filtered in -3.0..3.0f64,
        dq_traj in -3.0..3.0f64,
        settle_error in 0.0..1.0f64,
        vel_deadband in 0.001..0.1f64,
        effective_max_lead in 0.01..0.3f64,
        retarget_age_ms in 0..100_000u64,
        traj_phase in prop_oneof![
            Just(TrapezoidPhase::Accelerate),
            Just(TrapezoidPhase::Cruise),
            Just(TrapezoidPhase::Decelerate),
            Just(TrapezoidPhase::Hold),
        ],
        friction in prop::option::of((
            0.0..1.0f64,
            0.0..1.0f64,
            0.0..1.0f64,
            0.0..50.0f64,
        )),
        approaching_target in prop::bool::ANY,
        sustained_low_angle_breakaway in prop::bool::ANY,
    ) {
        // Position mode: tau_ff = tau_g + tau_f + tau_d.
        // The non-gravity components (tau_f, tau_d) come from
        // `compose_position_hold_feedforward` and must not depend on tau_g.
        let friction = friction.map(|(fc, fv, fo, k)| FrictionGains {
            fc,
            fv,
            fo,
            k,
            fs: None,
            v_b: None,
        });
        let friction_ref = friction.as_ref();

        let out1: PositionHoldFeedforward = compose_position_hold_feedforward(
            tau_g,
            kd,
            dq_filtered,
            dq_traj,
            settle_error,
            vel_deadband,
            effective_max_lead,
            retarget_age_ms,
            traj_phase,
            friction_ref,
            approaching_target,
            sustained_low_angle_breakaway,
        );
        let out2: PositionHoldFeedforward = compose_position_hold_feedforward(
            tau_g_perturbed,
            kd,
            dq_filtered,
            dq_traj,
            settle_error,
            vel_deadband,
            effective_max_lead,
            retarget_age_ms,
            traj_phase,
            friction_ref,
            approaching_target,
            sustained_low_angle_breakaway,
        );

        // The non-gravity components (tau_f, tau_d) must be identical.
        prop_assert!(
            (out1.tau_f - out2.tau_f).abs() < 1e-12,
            "tau_f changed with tau_g: {} vs {}",
            out1.tau_f,
            out2.tau_f
        );
        prop_assert!(
            (out1.tau_d - out2.tau_d).abs() < 1e-12,
            "tau_d changed with tau_g: {} vs {}",
            out1.tau_d,
            out2.tau_d
        );
        // tau_ff_cmd difference should equal tau_g difference.
        let ff_delta = out1.tau_ff_cmd - out2.tau_ff_cmd;
        let g_delta = tau_g - tau_g_perturbed;
        prop_assert!(
            (ff_delta - g_delta).abs() < 1e-12,
            "tau_ff_cmd delta {} should equal tau_g delta {}",
            ff_delta,
            g_delta
        );
    }

}
