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
   tick after any reshaped output: the τ_ff step guard acted, Berthier's or Davout's envelope
   clamped `q_des` (Davout judges it at the larger of the commanded and measured speed, so it can
   bind alone) or a Davout `clamp_velocity` danger zone limited `dq_des`, Davout counted a new
   total-torque clamp, or Davout sent a τ_ff different from the one commanded.
3. **Whole τ_ff step guard** `position_ff_step_max_nm` limits each sent tick's change of the
   complete Position τ_ff (gravity + dynamic + integral). On mode entry it starts from the τ_ff
   Davout last sent, so a supported elevated transition never jumps from 0. Davout's own τ_ff
   rate limiter keeps that value too: neither Position entry nor a `hold-at` retarget reseeds it
   from the measured torque, which includes the drive's PD effort. Only a joint with no τ_ff sent
   this session starts from the measured torque.
4. **The friction assist (λ) is removed**, together with `position_friction_error_gain_per_s` and
   the fuses' assist exclusion. The HoldTracking fuse again judges the full commanded torque.

Pitch trial values: kp 40, drive kd 6, H 2 Nm·s/rad, filter 10 ms, ki 5, leaky integral with 2 s
leak, band 0.02 rad, cap 0.25 Nm, guard 0.048 Nm. The audit recommended kd 5. In the in-tree
simple law gate (`pitch_bench_trial_meets_bench_criteria`), kd 5 overshot the 20 % 50 ms speed
bar on the heavy-inertia variant (20.5 %); kd 6 gives a worst case of 16.8 % across all five
plants, with predicted peak total torque 3.71 of 5 Nm and no clamps. **Those two figures hold
only for that simple gate**: Coulomb friction (fs 0.37 Nm, no Stribeck), URDF inertia 0.071
(heavy variant 0.111) kg·m², no position ripple, one estimator (5 ms count difference), one tick
of delay, calibration-like moves. They do not carry over to the audit's richer plant (next
section), where kd 6 fails.

### Richer-plant check: kd 6 fails, kd 5 passes (2026-10-28)

`bench_replay.rs` now replays both gains, everything else the master pitch entry, against the
audit's plant: J 0.14 kg·m², Stribeck friction fs 0.65 → fc 0.37 Nm with v_b 0.08 rad/s, 0.4 Nm
ripple at 28.6 cycles/rad, gravity fitted / ×0.96 / ×1.04 and the 1.05 Nm crest; drive
estimators 5 ms count, 20 ms count and exact; one extra old feedback sample; Davout's danger-zone
clamp, τ_ff limiter, total-torque prediction and reshape (fed back to the law) and RS03 wire
quantization. Sessions are the 2026-10-04 suite plus the audit's extended stops and slow waves.
The pass bar is the existing one, per move: slow track ≤ 30 mrad, fast 50 ms speed excess ≤ 20 %,
stop ≤ 10 mrad, τ_ff step ≤ 0.05 Nm, no predicted total-torque clamp, final-second hold range
≤ 5 mrad, repeat spread ≤ 10 mrad, no fault. The kd 5 rows land close to the audit's own
candidate table: identical fast excess (14.87 / 19.87 %), stops within 0.8 mrad, slow track within
2.1 mrad, and on the grid 10 (audit: 12) of 81 cases over the speed bar with the same 27.36 % worst.

Worst value over the audit's prescribed table (4 plants × 3 estimators = 12 cases):

| Metric | kd 5 | kd 6 |
|---|---:|---:|
| Cases passing the whole bar | **12 / 12** | **7 / 12** |
| Predicted total-torque clamps | 0 | 8 ticks in 5 cases |
| Peak predicted total torque (cap 5 Nm) | 4.95 Nm | 5.24 Nm |
| Slow track | 22.73 mrad | 22.73 mrad |
| Fast 50 ms speed excess | 19.87 % | 17.37 % |
| Stop | 5.91 mrad | 5.91 mrad |
| τ_ff step (law and decoded wire) | 0.0476 Nm | 0.0476 Nm |
| Hold range / repeat spread | 0.38 / 0.38 mrad | 0.38 / 0.77 mrad |
| Smoothness V, fitted plants (crest) | 0.043–0.047 (0.018) rad/s | 0.038–0.042 (0.016) rad/s |
| Audit uncertainty grid (81 cases) passing | 71 (10 speed, worst 27.36 %) | 45 (34 clamp, 1 of them also speed; 2 speed; worst 22.37 %) |

kd 6 fails on gravity×1.04 (5 and 20 ms estimators) and on the crest (all three estimators),
only on the clamp. The mechanism: at the gravity extremes (|τ_ff| ≈ 2.8–3.2 Nm), while a slow
move sticks on friction and ripple, Davout's position-difference velocity reads one encoder count
backwards (−0.077 rad/s) against a 0.09–0.15 rad/s reference, and the predicted damping
`kd·(v − dq) + kd·0.1 rad/s` reaches 1.6–2.0 Nm at kd 6, about 0.3 Nm more than at kd 5. That
takes the prediction to 5.01–5.24 Nm, so Davout would reshape, which the bench scores as a FAIL.
kd 6 is still the smoother gain (about 12 % lower V) and overshoots less, but on this plant it
spends the torque margin kd 5 keeps.

**The chosen gain is not changed here.** `control.yaml` keeps kd 6 for the reduced-speed bench
trial; whether to return to kd 5 (or trade kp or the torque margin) is the owner's call, on bench
evidence: a predicted total-torque clamp during the reduced-speed trial at the gravity extremes
would confirm this result.

## Consequences

- The PD is stiffer (kp 40 also applies to pitch's impedance mode), and torque margin shrinks.
  The simple law gate predicts no total-torque clamp at kd 6; the richer-plant replay predicts
  clamps at the gravity extremes at kd 6 and none at kd 5 (above).
- `v̂` uses Davout's position difference, which repeats the previous value on a stale tick
  rather than snapping to 0. A long feedback gap is already a Davout fault.
- Pitch's fast-move overshoot margin is thin across the audit's wider uncertainty grid (12/81
  cases over 20 % at kd 5). Full-speed qualification waits for bench evidence.
- GravityComp is unchanged: strictly model-only.

## Verification

- Unit: `host_damping_acts_on_filtered_velocity_error`,
  `leaky_reference_integral_settles_freezes_and_caps`,
  `ff_step_guard_bounds_every_step_and_starts_from_the_last_sent` (`position_law_tests.rs`);
  the simple law gate `pitch_bench_trial_meets_bench_criteria` (`law_gates.rs`).
- Richer plant (`bench_replay.rs`): `pitch_bench_suite_replay_meets_bench_bar` (master entry,
  stop now gated on the ±4 % gravity variants too: worst 5.46 mrad at both gains);
  `pitch_kd6_clamps_on_the_richer_plant_where_kd5_passes` (four representative cases); the full
  kd 5 / kd 6 sweep, opt-in: `cargo test -p berthier --lib pitch_kd_richer_plant_full_sweep --
  --ignored --nocapture` (about 30 s on four cores).
- ControlLoop, decoded wire τ_ff: `position_entry_from_gravity_comp_starts_from_the_last_sent_tau_ff`
  and `mid_move_retarget_never_steps_the_tau_ff_past_the_guard` (`tests/position_law_wire.rs`);
  `davout_envelope_clamp_freezes_the_leaky_integral` (`loop_wp_d_tests.rs`).
- Bench, reduced speed first (audit §"reduced-speed deciding experiment"): the candidate must cut
  the matched slow velocity-error RMS by at least 15 % (target ≈ 0.045 rad/s, from ≈ 0.055) with
  no worse stopping, no clamps and no hold hunting; repeat spread ≤ 10 mrad and final-second
  drift ≤ 5 mrad. If tracking improves but velocity RMS does not, reject the damping rationale
  and investigate the drive's ripple transfer instead of adding stiffness.

## Amendment 2026-10-28: hold-at speed fraction

The reduced-speed bench trial ("reduced speed fractions first") needs moves slower than the
configured planner speed. Until now only a single-cycle wave could run at a chosen speed; every
`hold-at` (and so every hold, repeat, mid-move retarget and the return to 0 that ends a session)
ran at `position_slew_rad_s` / `position_trajectory_velocity_rad_s`.

- **Surface.** `hold-at` takes an optional speed fraction `f`: stdin `hold-at [joint] <rad>
  [speed=<f>]`; a Testing POSITION batch entry's `MitJointCommand.speed_fraction` (proto field 9,
  `optional double`); Berthier `ControlLoop::enter_position_hold_at_speed` and
  `set_joint_position_setpoint_at_speed`. Without `f` a hold-at is exactly the old one.
- **Range, fail-closed.** `0 < f ≤ 1` (`marengo_config::SpeedFraction`, one rule for every
  surface). Zero, negative, above one, NaN, ±∞ or unparseable text are refused, never clamped:
  stdin prints the error and runs nothing; the gateway answers 400 and publishes nothing;
  marengo-pi refuses the whole Testing batch (published `motion_refused`) before any enable,
  gravity preflight, mode or gain change; Berthier returns `LoopError::InvalidSpeedFraction`
  before it touches the drives or the hold. A refused hold-at leaves the latched targets, the
  planner reference and the move's fraction as they were. A `speed_fraction` on a wave entry or
  in a non-POSITION batch also refuses the batch.
- **Only slows.** The fraction scales the joint's planner speeds for that move: slew, trajectory
  cruise and the velocity cap (`HoldJointParams::at_speed_fraction`), and the downward-return
  seed. Every speed the legacy planner or the scaled-PD reference governor picks is therefore
  `f ×` the configured one or less; the configured limits, the danger-zone descent cap and the
  envelope approach cap stay ceilings on top. The acceleration limit `a_max` is kept, like the
  ADR 0039 governor, which also scales speed only: the profile stays a valid trapezoid inside
  the configured acceleration, and the planner keeps its full braking authority, so a slowed
  retarget issued mid-move never coasts above its new cruise longer than an unslowed one. Gains,
  leads, deadbands, fuses and Davout's limits are unchanged. The envelope target clamp keeps
  using the configured (larger) speed, so its kinetic margin is never smaller.
- **Latched with the target.** The fraction belongs to the move it came with. A same-target
  hold-at stays a full no-op, fraction included; a new target without `f` is a full-speed move;
  `hold-on`, a fresh latch, a wave on the joint and leaving Position reset it. A degraded lower
  retargets without a fraction, under its own `lower_velocity_rad_s` cap.
- **Motion suite.** `pi_motion_suite` passes its largest `speed_fractions` value as `speed=<f>`
  to every hold-at it sends (holds, short moves, gravity-extreme moves, repeats, reversal
  retargets and the stop at 0) and times each wait from the scaled speed; waves keep one per
  fraction.

Verification: `quarter_speed_fraction_quarters_the_planned_peak_velocity` (legacy and scaled PD,
peak cruise exactly ¼, launch acceleration still `a_max`) and `a_wave_runs_at_its_own_speed_after_a_slowed_hold_at`
(`position_hold_tests/speed_fraction.rs`); `out_of_range_speed_fraction_*`,
`hold_at_without_a_fraction_plans_exactly_the_old_move` and `the_fraction_is_latched_with_the_target`
(`loop_speed_fraction_tests.rs`); the stdin, Testing-batch and gateway refusals
(`stdin_command_tests`, `motion_owner_chappe_tests`, `command_hardening_test`); the motion-suite
plan and session tests (`tools/marengo-pi-mcp/test/motion-suite.test.ts`).
