# ADR 0040: Scaled-PD slow-speed law: stiffer PD, filtered host damping, leaky reference integral

Status: Proposed, October 4, 2026. Implemented as per-joint options that default off; only
`right_shoulder_pitch` selects them, as a reduced-speed bench trial. Amends
[ADR 0039](0039-position-control-simplification.md): ADR 0039 says no torque term uses measured
`dq`, and this ADR adds one small, filtered, always-on host damper. Everything else in ADR 0039
(reference governor, drive-side PD, reference friction, `J·a` feed-forward, fuses, Davout's
envelope, caps and total-torque bound) stands.

## Context

After ADR 0039 Phase 3, pitch (kp 18, kd 5) stalled at slow-wave turnarounds: friction plus
position ripple left a dead zone of about `(fs + ripple)/kp ≈ 58 mrad`. Commit `c5890e3c` added a
friction assist that aimed the friction feed-forward at `v_ref + λ·(q_ref − q)` (λ = 20). It
fixed the turnaround tracking on the bench (long_moves_1 13/13), but:

- near rest it is a hidden stiffness of `fs·k·λ ≈ 195 Nm/rad`, ten times the explicit kp;
- gravity-assisted descents stopped 10.5–14.2 mrad past target (bar 10 mrad);
- the slow-motion "fighting" feel stayed.

The control audit `docs/reviews/2026-10-04-control-audit-pitch-slow-speed.md` rederived the
regime against the fitted plant and the recorded suites. Its findings, and one check made since:

- **The jitter is not drive velocity quantization.** Davout replaces the drive's velocity with a
  timestamped position difference (`davout/src/feedback_consumer.rs`), so the stepped `dq` in
  traces is a host artifact. The decoded wire (candump `20261004T211926Z`, 27 442 slow pitch
  replies) shows the drive's reported velocity has 0.6 mrad/s resolution but is noisy (0.067
  rad/s RMS against a 50 ms fit). Fitting the measured torque to the MIT law, the drive's damping
  matches a 50 ms-smooth velocity (residual 0.128 Nm, fitted `D = 5.09 ≈ kd`) far better than
  its own reported field (0.353 Nm) or the host difference (0.213 Nm). The drive damps on a
  smooth internal estimate, so lowering kd does not quiet anything real.
- **Moving smoothness is limited by friction and ripple against a soft kp.** Raising kp alone
  closes the dead zone but barely improves the true velocity error RMS `V`; a modest extra damper
  on a filtered velocity does.
- **The band integral grows without bound inside its band and ignores downstream reshaping.**

## Decision

Per joint, in `control.yaml`, all off by default:

1. **Host damping** `position_host_damping_nm_s_per_rad: H` adds `H·(v_ref − v̂)` to the slewed
   dynamic feed-forward, where `v̂` is Davout's measured velocity low-passed with time constant
   `position_host_damping_filter_s` (default 10 ms). It is always on: no deadband, onset switch
   or stall gate. It sits inside the existing 6 Nm/s dynamic slew, so it cannot step τ_ff.
2. **Leaky reference integral** `position_integral_mode: leaky_reference` decays every tick with
   `position_integral_leak_s`, integrates `ki·(q_ref − q)` only inside `position_integral_band_rad`,
   and stays within `position_integral_cap_nm` (≤ 0.5). A constant error therefore settles at
   `ki·leak·e` instead of ramping to the cap. Its input is frozen, and only the leak runs, on the
   tick after any reshaped output: the τ_ff step guard acted, the envelope clamped `q_des`, Davout
   counted a new total-torque clamp, or Davout sent a τ_ff different from the one commanded.
3. **Whole τ_ff step guard** `position_ff_step_max_nm` limits each sent tick's change of the
   complete Position τ_ff (gravity + dynamic + integral). On mode entry it starts from the τ_ff
   Davout last sent, so a supported elevated transition never jumps from 0.
4. **The friction assist (λ) is removed**, together with `position_friction_error_gain_per_s` and
   the fuses' assist exclusion. The HoldTracking fuse again judges the full commanded torque.

Pitch trial values: kp 40, drive kd 6, H 2 Nm·s/rad, filter 10 ms, ki 5, leaky integral with 2 s
leak, band 0.02 rad, cap 0.25 Nm, guard 0.048 Nm. The audit recommended kd 5. In the in-tree
fitted-plant gate (`pitch_bench_trial_meets_bench_criteria`), kd 5 overshot the 20 % 50 ms speed
bar on the heavy-inertia variant (20.5 %); kd 6 gives a worst case of 16.8 % across all five
plants, with predicted peak total torque 3.71 of 5 Nm and no clamps.

## Consequences

- The PD is stiffer (kp 40 also applies to pitch's impedance mode), and torque margin shrinks
  slightly; the gate still predicts no total-torque clamp.
- `v̂` uses Davout's position difference, which repeats the previous value on a stale tick
  rather than snapping to 0. A long feedback gap is already a Davout fault.
- Pitch's fast-move overshoot margin is thin across the audit's wider uncertainty grid (12/81
  cases over 20 % at kd 5). Full-speed qualification waits for bench evidence.
- GravityComp is unchanged: strictly model-only.

## Verification

- Unit: `host_damping_acts_on_filtered_velocity_error`,
  `leaky_reference_integral_settles_freezes_and_caps`,
  `ff_step_guard_bounds_every_step_and_starts_from_the_last_sent` (`position_law_tests.rs`);
  the fitted-plant gate `pitch_bench_trial_meets_bench_criteria` (`law_gates.rs`).
- Bench, reduced speed first (audit §"reduced-speed deciding experiment"): the candidate must cut
  the matched slow velocity-error RMS by at least 15 % (target ≈ 0.045 rad/s, from ≈ 0.055) with
  no worse stopping, no clamps and no hold hunting; repeat spread ≤ 10 mrad and final-second
  drift ≤ 5 mrad. If tracking improves but velocity RMS does not, reject the damping rationale
  and investigate the drive's ripple transfer instead of adding stiffness.
