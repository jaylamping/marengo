//! Bound on the total MIT torque a drive computes (ADR 0039, open question 1).
//!
//! In operation-control (MIT) mode the drive computes
//! `τ = kp·(q_des − q) + kd·(dq_des − dq) + τ_ff` in its own servo loop. Davout caps `τ_ff`
//! and refuses `kp`/`kd` above the motor-type maximum, but nothing documents a drive-side clamp
//! on the total: the vendor manual lists `limit_torque` (0x700B) with no mode qualifier, and
//! Marengo never reads or writes it. So Davout predicts the total from the latest feedback and,
//! when the prediction exceeds the joint's torque cap, moves `q_des` toward `q` and `dq_des`
//! toward `dq` by one common factor `λ ∈ [0, 1]`.
//!
//! The drive's torque depends on `λ` only through `λ·(kp·e + kd·ė)`, and that product is a
//! continuous function of the inputs, so the clamp never steps the commanded torque. A command
//! whose prediction is already within the cap is returned untouched, bit for bit.

use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;
use tracing::warn;

use crate::{store_by_joint, DavoutError, MitJointCommand};

/// Velocity-error margin of the prediction: the drive's velocity estimate and Davout's
/// position-derived `dq` may disagree by about one reported quantum (~0.077 rad/s on the RS03
/// pitch at 200 Hz, ADR 0039).
pub(crate) const TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S: f64 = 0.1;

/// Control periods of joint motion the position-error margin covers: one tick of transport
/// delay on the feedback, plus the period the command stays in force.
pub(crate) const TOTAL_TORQUE_HORIZON_TICKS: f64 = 2.0;

/// Minimum time between two total-torque clamp warnings; clamps in between are counted.
const TOTAL_TORQUE_CLAMP_LOG_INTERVAL: Duration = Duration::from_secs(1);

/// A command whose predicted total torque exceeded the cap and was scaled toward feedback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TotalTorqueClamp {
    /// Worst-case `|τ|` of the request, margins included (Nm, joint space).
    pub(crate) predicted_nm: f64,
    /// Worst-case `|τ|` after the clamp; above `cap_nm` only when `τ_ff` plus the margins
    /// alone exceed it (the clamp never raises torque by moving away from feedback).
    pub(crate) bounded_nm: f64,
    pub(crate) cap_nm: f64,
    /// Common factor applied to `q_des − q` and `dq_des − dq`.
    pub(crate) scale: f64,
}

/// Per-joint clamp counts since process start, and the rate limit of their warning.
#[derive(Debug, Default)]
pub(crate) struct TotalTorqueClampLog {
    counts: FxHashMap<String, u64>,
    last_warn: Option<Instant>,
    /// Clamps (any joint) since the last warning.
    unlogged: u64,
}

impl TotalTorqueClampLog {
    pub(crate) fn count(&self, joint: &str) -> u64 {
        self.counts.get(joint).copied().unwrap_or(0)
    }

    /// Count one sent clamp; warn at most once per [`TOTAL_TORQUE_CLAMP_LOG_INTERVAL`].
    pub(crate) fn record(&mut self, joint: &str, clamp: TotalTorqueClamp, now: Instant) {
        let count = self.count(joint).saturating_add(1);
        store_by_joint(&mut self.counts, joint, count);
        self.unlogged = self.unlogged.saturating_add(1);
        if self.last_warn.is_some_and(|last| {
            now.saturating_duration_since(last) < TOTAL_TORQUE_CLAMP_LOG_INTERVAL
        }) {
            return;
        }
        warn!(
            joint,
            predicted_nm = clamp.predicted_nm,
            bounded_nm = clamp.bounded_nm,
            cap_nm = clamp.cap_nm,
            scale = clamp.scale,
            joint_clamps = count,
            clamps_since_last_warning = self.unlogged,
            "MIT total torque clamped: setpoints moved toward feedback"
        );
        self.last_warn = Some(now);
        self.unlogged = 0;
    }
}

/// Worst-case `|τ|` the drive may compute for `cmd` given feedback `(q, dq)`, joint space.
///
/// `kp·(q_des − q) + kd·(dq_des − dq) + τ_ff`, plus `kp·e_max` for the motion
/// `max(|dq|, |dq_des|)·horizon_s` the joint may make before the drive applies the command, plus
/// `kd·`[`TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S`].
pub(crate) fn predicted_total_torque_nm(
    cmd: &MitJointCommand,
    q: f64,
    dq: f64,
    horizon_s: f64,
) -> f64 {
    let pd = cmd.kp * (cmd.position_rad - q) + cmd.kd * (cmd.velocity_rad_s - dq);
    (pd + cmd.torque_ff_nm).abs() + margin_nm(cmd, dq, horizon_s)
}

fn margin_nm(cmd: &MitJointCommand, dq: f64, horizon_s: f64) -> f64 {
    let e_max = dq.abs().max(cmd.velocity_rad_s.abs()) * horizon_s;
    cmd.kp * e_max + cmd.kd * TOTAL_TORQUE_VELOCITY_MARGIN_RAD_S
}

/// Scale `cmd`'s position and velocity errors toward feedback `(q, dq)` until the predicted
/// total torque is within `cap_nm`. `None` leaves `cmd` unchanged.
///
/// Fails closed on any non-finite input, a negative cap or a non-positive horizon.
pub(crate) fn clamp_total_torque(
    cmd: &mut MitJointCommand,
    q: f64,
    dq: f64,
    cap_nm: f64,
    horizon_s: f64,
) -> Result<Option<TotalTorqueClamp>, DavoutError> {
    for (field, value) in [
        ("position", cmd.position_rad),
        ("velocity", cmd.velocity_rad_s),
        ("kp", cmd.kp),
        ("kd", cmd.kd),
        ("torque_ff", cmd.torque_ff_nm),
    ] {
        if !value.is_finite() {
            return Err(invalid(cmd, format!("{field} must be finite")));
        }
    }
    if !q.is_finite() || !dq.is_finite() {
        return Err(DavoutError::InvalidFeedback {
            joint: cmd.joint.clone(),
            message: "total-torque bound needs finite position and velocity feedback".into(),
        });
    }
    if !cap_nm.is_finite() || cap_nm < 0.0 || !horizon_s.is_finite() || horizon_s <= 0.0 {
        return Err(DavoutError::Limit {
            joint: cmd.joint.clone(),
            message: format!("total-torque bound needs a finite cap ({cap_nm}) and horizon"),
        });
    }
    let e = cmd.position_rad - q;
    let e_dot = cmd.velocity_rad_s - dq;
    let pd = cmd.kp * e + cmd.kd * e_dot;
    let margin = margin_nm(cmd, dq, horizon_s);
    if !pd.is_finite() || !margin.is_finite() {
        return Err(invalid(cmd, "predicted total torque overflows".into()));
    }
    // `pd + τ_ff` must stay in [−cap + margin, cap − margin].
    let upper = cap_nm - margin - cmd.torque_ff_nm;
    let lower = -cap_nm + margin - cmd.torque_ff_nm;
    let target = if lower <= upper {
        pd.clamp(lower, upper)
    } else {
        // τ_ff plus the margins alone exceed the cap: aim the PD part at zero total torque.
        -cmd.torque_ff_nm
    };
    // Only λ ∈ [0, 1] is reachable: the clamp moves the setpoints toward feedback, never past it
    // nor away from it.
    let reachable = target.clamp(pd.min(0.0), pd.max(0.0));
    if reachable == pd {
        return Ok(None);
    }
    let predicted_nm = predicted_total_torque_nm(cmd, q, dq, horizon_s);
    // `reachable != pd` implies `pd != 0`.
    let scale = reachable / pd;
    let position = q + scale * e;
    let velocity = dq + scale * e_dot;
    if !position.is_finite() || !velocity.is_finite() || !(0.0..=1.0).contains(&scale) {
        return Err(invalid(
            cmd,
            "total-torque clamp produced a non-finite setpoint".into(),
        ));
    }
    cmd.position_rad = position;
    cmd.velocity_rad_s = velocity;
    Ok(Some(TotalTorqueClamp {
        predicted_nm,
        bounded_nm: (reachable + cmd.torque_ff_nm).abs() + margin,
        cap_nm,
        scale,
    }))
}

fn invalid(cmd: &MitJointCommand, message: String) -> DavoutError {
    DavoutError::InvalidCommand {
        joint: cmd.joint.clone(),
        message,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// Master pitch gains and the 5 Nm cap at 200 Hz.
    const KP: f64 = 18.0;
    const KD: f64 = 3.0;
    const CAP: f64 = 5.0;
    const HORIZON: f64 = TOTAL_TORQUE_HORIZON_TICKS / 200.0;

    fn command(position: f64, velocity: f64, torque_ff: f64) -> MitJointCommand {
        MitJointCommand {
            joint: "right_shoulder_pitch".into(),
            kp: KP,
            kd: KD,
            position_rad: position,
            velocity_rad_s: velocity,
            torque_ff_nm: torque_ff,
        }
    }

    #[test]
    fn total_torque_within_cap_is_untouched() {
        // Legacy hold near target: kd·0 + kp·0.05 + 2 Nm gravity.
        for cmd in [
            command(0.55, 0.0, 2.0),
            command(0.5, 0.3, -1.0),
            MitJointCommand {
                kp: 0.0,
                kd: 0.0,
                ..command(3.0, 0.0, CAP)
            },
        ] {
            let mut out = cmd.clone();
            let clamp = clamp_total_torque(&mut out, 0.5, 0.2, CAP, HORIZON).expect("finite");
            assert_eq!(clamp, None);
            assert_eq!(out, cmd, "a command under the cap must pass bit for bit");
        }
    }

    #[test]
    fn total_torque_over_cap_is_bounded_toward_feedback() {
        // Scaled-PD worst case: lead e1 = 0.12 while the joint falls at 2.9 rad/s against a
        // 1.25 rad/s reference, gravity 2 Nm: kp·0.12 + kd·4.15 + 2 ≈ 16.6 Nm.
        let (q, dq) = (0.5, -2.9);
        let mut cmd = command(0.62, 1.25, 2.0);
        let before = predicted_total_torque_nm(&cmd, q, dq, HORIZON);
        assert!(before > 3.0 * CAP, "fixture must exceed the cap: {before}");
        let clamp = clamp_total_torque(&mut cmd, q, dq, CAP, HORIZON)
            .expect("finite")
            .expect("clamped");
        let after = predicted_total_torque_nm(&cmd, q, dq, HORIZON);
        assert!(after <= CAP + 1e-9, "predicted {after} Nm > {CAP}");
        assert!((after - clamp.bounded_nm).abs() < 1e-9);
        assert!((clamp.predicted_nm - before).abs() < 1e-9);
        // Setpoints move toward feedback, never past it; gains and τ_ff are untouched.
        assert!(cmd.position_rad > q && cmd.position_rad < 0.62);
        assert!(cmd.velocity_rad_s > dq && cmd.velocity_rad_s < 1.25);
        assert_eq!((cmd.kp, cmd.kd, cmd.torque_ff_nm), (KP, KD, 2.0));
    }

    #[test]
    fn total_torque_clamp_is_continuous_across_the_cap() {
        // Sweep the tracking error through the cap: the bounded torque never steps.
        let (q, dq) = (0.4, -1.0);
        let mut previous: Option<f64> = None;
        for step in 0..=4000 {
            let lead = -0.2 + f64::from(step) * 1e-4;
            let mut cmd = command(q + lead, 0.8, 1.5);
            clamp_total_torque(&mut cmd, q, dq, CAP, HORIZON).expect("finite");
            let pd = KP * (cmd.position_rad - q) + KD * (cmd.velocity_rad_s - dq);
            let tau = pd + cmd.torque_ff_nm;
            assert!(predicted_total_torque_nm(&cmd, q, dq, HORIZON) <= CAP + 1e-9);
            if let Some(previous) = previous {
                // The unclamped slope is kp·1e-4 = 0.0018 Nm per step.
                assert!(
                    (tau - previous).abs() <= KP * 1e-4 + 1e-9,
                    "step at lead {lead}"
                );
            }
            previous = Some(tau);
        }
    }

    #[test]
    fn total_torque_never_moves_away_from_feedback() {
        // τ_ff at the cap: margins alone exceed it. A PD part that opposes τ_ff is kept, one
        // that adds to it is removed; neither is pushed past feedback.
        let (q, dq) = (0.3, 0.0);
        let mut opposing = command(0.29, 0.0, CAP);
        let original = opposing.clone();
        assert_eq!(
            clamp_total_torque(&mut opposing, q, dq, CAP, HORIZON).expect("finite"),
            None
        );
        assert_eq!(opposing, original);

        let mut adding = command(0.35, 0.2, CAP);
        let clamp = clamp_total_torque(&mut adding, q, dq, CAP, HORIZON)
            .expect("finite")
            .expect("clamped");
        assert_eq!(clamp.scale, 0.0);
        assert_eq!((adding.position_rad, adding.velocity_rad_s), (q, dq));
    }

    #[test]
    fn total_torque_fails_closed_on_non_finite_inputs() {
        for (q, dq, cap, horizon) in [
            (f64::NAN, 0.0, CAP, HORIZON),
            (0.0, f64::INFINITY, CAP, HORIZON),
            (0.0, 0.0, f64::NAN, HORIZON),
            (0.0, 0.0, -1.0, HORIZON),
            (0.0, 0.0, CAP, 0.0),
        ] {
            let mut cmd = command(0.1, 0.0, 0.0);
            assert!(clamp_total_torque(&mut cmd, q, dq, cap, horizon).is_err());
        }
        let mut cmd = command(0.1, 0.0, 0.0);
        cmd.torque_ff_nm = f64::NAN;
        assert!(clamp_total_torque(&mut cmd, 0.0, 0.0, CAP, HORIZON).is_err());
    }
}
