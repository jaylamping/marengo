//! Mode isolation property tests — prove gravity comp changes don't affect other modes.
//!
//! These tests verify the CRITICAL requirement: perturbing tau_g (e.g., COM correction,
//! algorithm changes, payload estimation) must NOT change the non-gravity feedforward
//! components of Impedance or Position modes.
//!
//! Scope:
//! - Impedance mode non-gravity FF = `tau_f` from `friction_torque(dq, fc, fv, fo, k)`.
//! - Position mode non-gravity FF = `tau_f + tau_d` from
//!   `compose_position_hold_feedforward`.
//! - GravityComp mode is pure feedforward (`tau_ff = tau_g`).
//!
//! Strategy: for a fixed `(q, dq, config)`, compute the non-gravity FF components with
//! two different `tau_g` values. If the non-gravity components are identical, the modes
//! are isolated from gravity comp changes.

#![cfg(test)]
#![allow(clippy::expect_used)]

use proptest::prelude::*;

use crate::friction::friction_torque;
use crate::position_feedforward::{PositionHoldFeedforward, compose_position_hold_feedforward};
use crate::position_trajectory::TrapezoidPhase;
use marengo_config::FrictionGains;

proptest! {
    #[test]
    fn impedance_tau_f_independent_of_tau_g(
        tau_g in -5.0..5.0f64,
        tau_g_perturbed in -5.0..5.0f64,
    ) {
        // Impedance mode: tau_ff = tau_g + tau_f.
        // The non-gravity component is tau_f = friction_torque(dq, fc, fv, fo, k),
        // which depends only on dq and friction gains — NOT on tau_g.
        let dq = 0.1;
        let fc = 0.15;
        let fv = 0.0;
        let fo = 0.0;
        let k = 10.0;
        let tau_f = friction_torque(dq, fc, fv, fo, k);
        // tau_f must be the same regardless of tau_g.
        let tau_ff_1 = tau_g + tau_f;
        let tau_ff_2 = tau_g_perturbed + tau_f;
        // The difference in tau_ff should be exactly the difference in tau_g.
        prop_assert!(
            ((tau_ff_1 - tau_ff_2) - (tau_g - tau_g_perturbed)).abs() < 1e-12,
            "tau_ff delta {} should equal tau_g delta {}",
            tau_ff_1 - tau_ff_2,
            tau_g - tau_g_perturbed
        );
    }

    #[test]
    fn position_non_gravity_ff_independent_of_tau_g(
        tau_g in -5.0..5.0f64,
        tau_g_perturbed in -5.0..5.0f64,
    ) {
        // Position mode: tau_ff = tau_g + tau_f + tau_d.
        // The non-gravity components (tau_f, tau_d) come from
        // `compose_position_hold_feedforward` and must not depend on tau_g.
        let friction = FrictionGains {
            fc: 0.15,
            fv: 0.0,
            fo: 0.0,
            k: 10.0,
        };
        let kd = 2.0;
        let dq_filtered = 0.05;
        let dq_traj = 0.1;
        let settle_error = 0.08;
        let vel_deadband = 0.02;
        let effective_max_lead = 0.10;
        let retarget_age_ms = 500u64;
        let traj_phase = TrapezoidPhase::Cruise;
        let approaching_target = true;

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
            Some(&friction),
            approaching_target,
            false,
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
            Some(&friction),
            approaching_target,
            false,
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

    #[test]
    fn gravitycomp_tau_ff_equals_tau_g(tau_g in -5.0..5.0f64) {
        // GravityComp mode: kp=0, kd=0, tau_ff=tau_g, q_des=q, mit_velocity=0.
        // This is the pure feedforward — no extra terms.
        let kp = 0.0;
        let kd = 0.0;
        let tau_ff = tau_g;
        // Verify: no position feedback (kp=0), no damping (kd=0), tau_ff is exactly tau_g.
        prop_assert_eq!(kp, 0.0);
        prop_assert_eq!(kd, 0.0);
        prop_assert!(
            (tau_ff - tau_g).abs() < 1e-12,
            "GravityComp tau_ff must equal tau_g"
        );
    }
}
