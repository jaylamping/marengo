# Right-arm calibration suite

Status: in build (2026-10-04). Profile `arm_attached`, all five joints referenced
in every session (ADR 0036). Trust nothing from CAD or earlier tuning: each phase
measures gravity and friction on the current hardware, fits, applies, and is
validated before the next phase may run.

## Why

2026-10-04, pitch 0.8 elbow sweep: elbow measured τ ≈ 1.5 × URDF τ_g and
static friction ≈ ±0.14 Nm (config `fc` 0.05). On a downward step the
under-compensated forearm fell at 2.46 rad/s (planned 0.8) and tripped the feedback
velocity fault. The earlier pitch fit (session 20261004T045804Z) was refused:
forearm and upper-arm mass were not separable from one sweep.

## Order and rules

Shoulder first, then down the chain, then compound. Low-gravity poses before
loaded ones. One joint moves per session; the others hold fixed poses.

| Phase | Sweep joint | Fixed poses | Measures |
|---|---|---|---|
| 0 | none | rest | 5-cycle enable soak (wire health) |
| 1 | shoulder_pitch | all others 0 | whole-arm mass × COM about pitch; pitch friction |
| 2 | shoulder_roll | all others 0 | whole-arm mass × COM about roll; roll friction |
| 3a | upper_arm_yaw | all 0 (axis ≈ vertical) | yaw friction only |
| 3b | upper_arm_yaw | pitch 0.5, elbow 0.5 | forearm COM offset about yaw |
| 4a | elbow_pitch | all 0 | forearm + hand mass × COM |
| 4b | elbow_pitch | pitch 0.5 | same, loaded (fit check) |
| 5a | lower_arm_yaw | all 0 | wrist-yaw friction only |
| 5b | lower_arm_yaw | elbow 0.5 | hand COM offset |
| 6 | validation | grid of fixed poses | residual gate, no refit |
| 7 | compound | waypoint sequences | tracking, coupling, repeatability |

Each sweep session (`pi_joint_calibrate`, `method: "wave"` by default):

1. **Local waves** at each pose: park at pose − a, then a raised-cosine `wave`
   [pose − a, pose + a] at 2–3 peak speeds. A static hold rests anywhere in the
   ±F_s stiction band (2026-10-04 pitch: every hold fits `2.35 sin q − 0.09 cos q
   ± 0.37 Nm`), so it cannot place gravity better than F_s; a wave drives through
   stiction both ways. Sizing, from the Pi's config and the model:
   - amplitude a = (F_s + 0.6 × max|τ_g| at the poses) / kp (control.yaml
     friction `fs`, else `fc`; impedance `kp`; 0.6 = the τ guard's distrust
     factor − 1), at least 0.025 rad, refused above 0.1 rad (`wave_amplitude_rad`
     overrides);
   - peak speeds from 0.2 rad/s (gravity-fit's centre bin stays above its 0.15 rad/s
     moving deadband) to what marengo-pi admits within 80 % of the velocity cap
     (`wave_speeds_rad_s` overrides);
   - cycles per speed so the centre bin collects 1.5 × 10 samples per direction
     (`wave_cycles` overrides); poses more than 2a apart; default poses keep 0.1 rad
     plus the 0.05 rad inset inside the window.
   Amplitude grows by phase run: 25 %, 50 %, then 90 % of the soft window.
2. **`method: "static"`** keeps the earlier session: holds approached from below
   *and* from above (½Δ = Coulomb friction, mean = gravity), then
   constant-velocity passes (`velocity_passes`) in the gravity-free middle.
3. **Pre-flight guard**: refuse any pose whose model τ_g × 1.6 (model distrust
   factor) exceeds 80 % of the joint's τ_ff cap (wave method: over ±0.1 rad around
   every pose).

After each phase: fit (gravity-fit, all sessions so far fused), review, apply
the URDF patch (`pi_sync_bench_urdf`, ADR 0017) and friction patch to
`control.yaml` (Pi first; Pi is the source of truth), then rerun that phase's
session as a no-refit check before the next phase.

## Wave fit (`gravity-fit`, wave method)

- **Bins**: each wave in a/2-wide bins at pose and pose ± a/2, split by direction
  (≥ 10 moving samples each way). Per direction the mean of τ − I·q̈ (I from the
  URDF about the swept axis, q̈ the measured second difference); gravity = the
  up/down mean, friction = half the difference at the mean |q̇|.
- **Lumped fit**: with the other joints fixed, the swept joint's gravity is exactly
  `A·sin q + B·cos q` (`armee_dynamics::lumped`). One sweep identifies only A and
  B, not which link is wrong. With ≥ 2 speeds the URDF inertia error ΔI (rotor
  inertia is not in the URDF) is fitted with them; with one speed only the
  centre bins (q̈ ≈ 0) are used.
- **URDF patch**: the smallest COM shift of the carried right-arm links that
  reproduces every fitted A, B, masses unchanged; refused above 0.05 m. The record
  lists how much it moves every other joint's gravity.
- **Friction patch**: fc + fv·|q̇| over the pose/speed bins; `fs` only when the
  Stribeck term (control.yaml `v_b`) is significant, fv only when significant (else
  fc is the Coulomb mean and fv 0, never a slope extrapolated from a narrow speed
  span); independent of the gravity verdict.

## Gates

- Wave method, derived from the data: σ_cross = pooled spread of per-session pose
  means; gate = max(3·σ_cross, torque readout step). Accepted only with ≥ 2 sessions
  sharing a pose, ≥ 3 replicated poses, gate ≤ 0.10 Nm (the data must be repeatable
  enough to check the suite's residual), every replicated pose residual ≤ gate, and
  σ_A, σ_B ≤ gate. A single-bin pose (one wave step of one session, e.g. a wave-edge
  bin) stays in the fit and is reported, but never gates.
- Friction: σ_cross must be repeatable (3·σ_cross ≤ 0.10 Nm), but the residual gate
  and fv/fs significance use the bin noise, max(RMS sampling error of the bins,
  σ_cross): gate = 3·noise, floored at the readout step, capped at 0.10 Nm. A wave
  repeats the same torque swings at the same q every session, so σ_cross understates
  how well a smooth friction model can match bins at other poses.
- Static method: residual |τ_meas − (τ_g + friction sign)| ≤ 0.10 Nm per joint per
  pose; identifiability min singular value ≥ 0.05 Nm, condition ≤ 100.
- No fault, fuse, watchdog or CAN error; no `rx_over` growth; candump + trace
  reviewed after every motion session.
- Phase 6: every grid pose passes the residual gate with the fitted model.
- Phase 7 needs the operator's forbidden-pose list (collisions with torso,
  stand, cables) before any compound move is commanded.

## Tooling

- `pi_joint_calibrate` (MCP): one session = one sweep joint, fixed poses,
  local waves (or static holds + velocity passes), pre-flight guard, ≤ 300 s.
- `marengo-log-cli gravity-fit`: any sweep joint, fused sessions, partial
  sessions (completed steps only); `--method auto|wave|static` (auto follows
  plan.json `method`; `wave` also bins older sessions' velocity-pass waves).
  Proposes the URDF patch and the `control.yaml` friction patch. Nothing is
  applied automatically.

2026-10-04 re-fit of the pitch sessions 084416Z, 084615Z, 102328Z with
`--method wave` (their only waves span ±0.15 rad around 0): B 0.008 ± 0.005 Nm,
A 2.86 ± 0.14 Nm (CAD 2.83 / 0.034), fitted ΔI ≈ 0; refused on identifiability
(σ_A above the 0.023 Nm gate). Friction fc 0.277 Nm, fv 0.199 Nm·s/rad proposed.
Without I·q̈ the same bins give A ≈ 2.2–2.4; with it they agree with CAD within
σ_A, so these narrow waves can neither confirm nor rule out the 17–24 % deficit
the static holds suggested. A wave session across the window decides it.
