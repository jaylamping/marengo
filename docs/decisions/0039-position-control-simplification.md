# ADR 0039: Position control simplification: drive-side PD on a feedback-scaled reference

Status: Proposed, October 4, 2026. Phases 0 and 2 implemented October 4, 2026 (trace columns;
the scaled-PD law in simulation, selectable per joint, every joint on the legacy law). Open
question 1 closed the same day: Davout bounds the predicted total MIT torque. When
accepted, this supersedes these parts of
[ADR 0007](0007-bench-position-trajectory-control.md):

- "Feedforward and damping policy" (host `tau_d`);
- "Commanded MIT setpoints" (firmware `kd = 0`);
- "Use firmware damping again" (rejected alternative).

It keeps ADR 0007's trajectory generator, limits and safety boundaries.

## Context

The right arm moves jerkily in `ControlMode::Position`. The audit of the 2026-10-04 bench traces
(`var/gravity-calibration/20261004T084615Z`, `…084416Z`, `…073640Z`) found the following.

Shoulder pitch, kp 18, kd 3, 200 Hz:

- 30–37% of moving trace rows have measured `dq` exactly 0.
- Velocity overshoot p95 is +95–108% of plan, and the maximum is +170%.
- 138–159 rows show a host `τ_ff` change above 0.3 Nm in 20 ms; the largest is 2.74 Nm.

Elbow:

- It stalled 6 mrad short of 0.75 for 2 s.
- On a retarget to 0.5 it fell at −2.46 rad/s (planned −0.80) into Davout's
  feedback-velocity fault.

The causes are structural:

1. **Host "damping" is a gated velocity-error push.** `tau_d = kd·(dq_traj − dq_f)` is held at
   0 while `|dq_f|` is below a 0.02 rad/s deadband. Measured `dq` is quantized to ~0.075 rad/s,
   so the gate releases the whole `kd·|dq_traj|` in one tick (0 → −1.61 Nm at breakaway). With
   the EMA lag (~15 ms) it then pushes while the joint accelerates. It brakes only after the
   overshoot. The brake cap applies in one direction only.
2. **Double damping.** Within 0.1 rad of the target the drive also gets `kd_mit = kd`. That kd
   is switched on and off by the same deadband (seen in 23% of pitch 1 Hz samples).
3. **An open-loop planner plus repair heuristics.** The trapezoid advances regardless of `q`.
   About twenty special cases repair the consequences, each stepping `q_des`, `dq_traj`, `τ_f`
   or `kd_mit` at band edges one or a few encoder counts wide:
   - planner state changes: resync, drift reset, premature-hold reopen, lead-follow hold-short,
     descent freeze, ascent recovery, overshoot latch;
   - setpoint overrides: overshoot snap, outrun clamp, stuck pull, pull-through;
   - gain and reference shaping: the onset and low-angle lead boosts, the downward seed;
   - friction recipes: three of them, plus onset, stuck and overspeed scales.

   One of these, the overshoot snap, stepped `q_des` by up to 0.14 rad and caused the elbow
   trip. It was fixed in `db492cd3`. The others stay as long as this architecture stays.

The evidence above comes from the position traces and the 1 Hz `position hold command` log of
those sessions. Phase 4 records the audit summary in `docs/position-hold-control-review.md`.

ADR 0007 rejected firmware damping because "the drive's raw velocity estimate has been noisy".
The host path uses *the same* estimate: Davout decodes the drive's reported velocity. The host
only adds an EMA and ~20 ms of loop and transport delay. Until 2026-10-03, RS03 wire velocity
was also decoded 2.5× high and `v_des` arrived at 0.4× intent (`docs/safety.md`, RS03 MIT
velocity scale). [INFERENCE: some of the "noisy firmware damping" seen in mid-2026 may be that
scale error.]

## Decision

### Control law (per joint, per 5 ms tick)

```text
reference:   (q_r, v_r, a_r) from the trapezoid planner, advanced by time scaling s
time scale:  e   = q_r − q
             s   = clamp(1 − (|e| − e0) / (e1 − e0), 0, 1)   (rate-limited, see below)
             v_c = s · v_r          a_c = s · a_r   (q_r advances by v_c·dt)
MIT frame:   position = clamp_envelope(q_r)
             velocity = v_c
             kp       = kp            (wire kp, ramped on mode entry by gain_runtime)
             kd       = kd            (constant while in Position; no gating)
             τ_ff     = τ_g(q) + τ_fric(v_c) [+ J_eff·a_c] [+ ki·I]
friction:    τ_fric(v) = (fc + (fs − fc)·exp(−|v|/v_b)) · tanh(v / v_s) + fv·v
integral:    İ = e_target  when |e_target| < i_band, else İ = −I/τ_leak;  |ki·I| ≤ 0.5 Nm
```

- **Drive-side PD.** All damping is `kd·(v_c − dq)`, computed in the drive's servo loop on its
  own velocity estimate, with no host round trip. Berthier sends `kd` unchanged every tick.
  Nothing in `τ_ff` depends on measured `dq`.
- **The reference never jumps.** A retarget replans from the reference's own state
  `(q_r, v_r)`, not from `q`. Only the first arm starts at `(q, 0)`. Velocity is therefore
  continuous at every retarget, and `|Δv_c| ≤ a_max·dt` plus the time-scale slew.
- **Feedback coupling by time scaling, not by resets.**
  - While `|q_r − q| ≤ e0` the reference runs at full rate.
  - Between `e0` and `e1` it slows linearly, and at `e1` it stops (s = 0, so v_c = 0).
  - A stuck joint gets a stationary reference held `e1` ahead, so kp·e1 is the bounded
    breakaway push. When the joint moves, s recovers continuously.
  - `s` is rate-limited (`|Δs| ≤ a_max·dt / max(|v_r|, ε)`) so `v_c` respects `a_max`.
  - A joint ahead of the reference gets full P and D braking back toward it (no outrun
    branch).
  - Lead is bounded by construction, `|q_r − q| ≤ e1` plus one tick, so no setpoint clamp other
    than the limit envelope is needed.
- **Friction from the reference velocity.** Direction and magnitude come from `v_c`, which is
  continuous and free of dither. A small Stribeck term (`fs > fc` for `|v| < v_b`) supplies
  breakaway while the reference starts moving, instead of a measured "stuck" detector. At rest
  (`v_c = 0`) `τ_fric = 0`. P and the leaky integral handle the static dead zone
  `(fs − fc)/kp`.
- **Acceleration feed-forward** (`J_eff·a_c`) is optional and defaults off, until `J_eff` is
  identified.

### Heuristics deleted, and why each is unnecessary

| Deleted | Why the new law does not need it |
|---|---|
| Host `tau_d`, its deadband substitution (`dq_for_d`), spike brake cap, Hold-branch `−kd·dq_f` | Drive kd·(v_c − dq) is continuous, symmetric and has no host delay |
| dq EMA (`POSITION_DAMPING_DQ_FILTER_ALPHA`), dq seeding at retarget | No host torque term uses measured dq |
| `kd_mit` gating (\|e\| < 0.1, \|dq\| ≥ deadband); `v_des` onset gating (`POSITION_HOLD_ONSET_MS`) | kd is constant; v_des = v_c always |
| `planner_should_resync_stuck_lead`, `planner_drifted_from_measurement` reset | Time scaling keeps the reference within e1 of q; it never drifts |
| Premature-hold reopen, lead-follow hold-short and `apply_lead_follow_hold_short`, overshoot latch, the `planner_overshot_*` predicates | The reference reaches the target only when the joint is within e1. The rest is ordinary P+I settling, with no Hold ↔ Cruise toggling |
| `planner_should_freeze_on_descent`, ascent-recovery planner policy | Time scaling is the single freeze, and it is continuous |
| `clamp_trajectory_setpoint` brake/follow/snap branches | q_des = q_r with only the limit-envelope clamp; the lead is bounded by construction |
| Onset lead boost, low-angle sustained boost (`POSITION_HOLD_ONSET_MAX_LEAD_RAD`) | Breakaway push is kp·e1 plus Stribeck friction FF; there are no time windows |
| Descent stuck pull, home-final pull and pull-through, breakaway latches | These were gain boosts in disguise. Residual error is closed by I. A gravity shortfall is fixed in the model (GravityComp rule) |
| Downward return seed velocity | The reference starts from its own state; with a correct τ_g, descents need no seed |
| Friction modes `traj_vel`/`settle`/cross-target, stuck, onset and overspeed scales, settle fade | One smooth function of v_c |

**Kept:** the trapezoid planner and its v/a limits, the velocity caps (ADR 0010), the Berthier
and Davout limit envelopes (ADR 0009), home classification, the integral (made leaky), and the
three fuses (AscentStall, HoldTracking, WaveStall). HoldTracking's "net commanded torque" becomes
the wire terms: `kp·(q_r − q) + kd·(v_c − dq) + τ_ff`.

### Parameter migration (`config/control.yaml`)

This ADR changes no values. A changed value needs its own bench evidence (test plan below).

| Today | Proposed | Note |
|---|---|---|
| `impedance.kp` | MIT kp | unchanged meaning |
| `impedance.kd` | MIT kd, always on in Position | today it is host `tau_d` plus an intermittent drive kd (effective 2·kd near target) |
| `impedance.ki` | ki, leaky integral | the reset becomes a leak (`τ_leak`) |
| `position_slew_max_lead_rad` | `e1` (time-scale stop band) | same role: the maximum lead |
| — | `e0` (time-scale full-rate band) | new; start at e1/4 |
| `position_trajectory_velocity_deadband_rad` | removed | was rad/s under a rad name |
| `friction.fc`, `fv`, `fo` | same | |
| `friction.k` | `v_s = 1/k` (tanh width, rad/s) | |
| — | `friction.fs`, `friction.v_b` | new; `fs` defaults to `fc` (no Stribeck) until identified |
| `position_slew_rad_s`, `position_trajectory_{threshold,velocity,accel}` | unchanged | planner |
| Constants `POSITION_HOLD_ONSET_MS`, `POSITION_STUCK_EXIT_VELOCITY_RATIO`, `POSITION_DAMPING_*`, `POSITION_DESCENT_STUCK_LEAD_RAD`, `POSITION_HOME_FINAL_PULL_THROUGH_RAD`, `POSITION_RETURN_DESCENT_SEED_RAD`, `POSITION_RETURN_FREEZE_Q_MAX_RAD`, `POSITION_HOLD_ONSET_MAX_LEAD_RAD`, `POSITION_HOLD_FRICTION_FADE_RAD` | deleted | |

**Implemented keys (Phase 2).** Every key is optional, per joint, refused when invalid
(`deny_unknown_fields`, `validate_joint_numbers`, `validate_entry_gains_against_motor_type`), and
left out of serialized YAML while unset, so the master `control.yaml` is unchanged.

| Key | Default | Validation | Why this default |
|---|---|---|---|
| `position_law` | `legacy` | `legacy` or `scaled_pd`; `scaled_pd` needs `position_slew_max_lead_rad > 0` | bench behaviour unchanged until a joint is selected |
| `position_time_scale_e0_rad` | `e1/4` | finite, `0 ≤ e0 < position_slew_max_lead_rad` | this ADR's starting point |
| `position_integral_band_rad` | 0.1 | finite, `> 0` | the legacy integral window, unchanged meaning |
| `position_integral_leak_s` | 0.5 | finite, `> 0` | bounds the decay of the 0.5 Nm integral to 1 Nm/s (0.005 Nm per tick) and drops a stale term within ~1.5 s |
| `friction.fs` | `fc` | finite, `fc ≤ fs ≤ tau_ff_max_nm` | no Stribeck term until Phase 1 identifies it |
| `friction.v_b` | 0.05 rad/s | finite, `> 0` | inert while `fs = fc` |

`J_eff·a_c` has no key: it stays off until `J_eff` is identified, and an unused key would only
invite an unmeasured value.

### Davout interactions

- **τ_ff cap and rate limit.** τ_ff becomes `τ_g + τ_fric (+J·a) (+ki·I)`. Its slope is bounded
  by `|∂τ_g/∂q|·|v|` plus the tanh slope times `a_max`. That is well under 60 Nm/s
  (pitch: ~2.7 Nm/rad × 2.5 rad/s ≈ 7 Nm/s). The rate limiter becomes a non-binding guard. Any
  tick where it binds is logged and fails the bench metric.
- **Drive kd is outside the τ_ff cap and the rate limit.** The torque bound is
  `kp·e1 + kd·|v_c − dq| + |τ_ff|`. Davout already checks `kd/s² ≤ kd_max`. On pitch
  `kd·(|v_c| + |dq|)` alone reaches 3·(1.25 + 3.0) ≈ 13 Nm at the feedback-velocity fault
  threshold, against `torque_limit_nm` 5. Phase 2 bounded only the inputs Davout owns: kp and kd
  above the motor-type maximum are refused (never clamped), `|v_des|` above the cap is refused,
  and τ_ff is capped and rate limited whatever kd is
  (`torque_output_contract_drive_damping_is_bounded_and_tau_ff_stays_capped`).
- **Total-torque bound (open question 1, closed October 4, 2026).**
  - *Firmware.* No drive-side clamp can be relied on. The RS03 manual lists `limit_torque`
    (0x700B, 0–60 Nm, W/R) with no mode qualifier, and gives the operation-mode law
    `t_ref = Kd·(v_set − v) + Kp·(p_set − p) + t_ff` with no clamp. `limit_cur` (0x7018) is
    scoped to the velocity and position modes. Marengo never reads or writes 0x700B
    (`protocol-inspect` reads six other registers), so even a clamp would sit at an unread
    value.
  - *Davout.* `crates/davout/src/total_torque.rs` runs on every MIT command, after the τ_ff cap
    and rate limiter, with the same per-joint cap (`tau_ff_max` ∩ `bench.torque_limit_nm` ∩
    motor-type `tau_ff_max_nm` ∩ danger-zone `clamp_torque`). Predicted worst case from the
    latest published `q` and `dq`:
    `|kp·(q_des − q) + kd·(dq_des − dq) + τ_ff| + kp·max(|dq|, |dq_des|)·2/loop_hz + kd·0.1`.
    The second term is the motion over one tick of transport delay plus the command's own
    period. The third is the velocity-estimate margin, one reported quantum (~0.077 rad/s).
  - *Clamp.* Above the cap, `q_des` moves toward `q` and `dq_des` toward `dq` by one factor
    `λ ∈ [0, 1]`, the largest that fits. kp, kd and τ_ff are untouched. The drive torque
    depends on λ only through `λ·(kp·e + kd·ė)`, which is continuous, so the clamp never steps
    torque. It never moves a setpoint past or away from feedback. If τ_ff plus the margins
    alone exceed the cap, the PD part goes to zero only when it adds to τ_ff.
  - *Semantics.* A command already under the cap is sent bit for bit, so both laws behave as
    before there. Clamps are counted per joint (`Supervisor::total_torque_clamp_count`) and
    logged at most once per second. Non-finite inputs, cap or horizon are refused, as is a
    clamped setpoint outside the hard limits (possible only while `q` is outside them).
  - *Tests.* `torque_output_contract_predicted_total_torque_is_clamped_continuously` (wire
    decode; without the clamp, 5.02 Nm at the first over-cap lead).
    `scaled_pd_pitch_never_predicts_total_torque_above_the_cap`: pitch on `scaled_pd`, jammed
    and then dragged down at 2.4 rad/s. Without the clamp it predicts 11.0 Nm.
  - *Remaining bench check (Phase 3 entry).* With the arm supported, grep each qualification
    run's bench log for `MIT total torque clamped`. A clamp on a normal move means the law or
    its gains ask for more than the cap. The count is not yet in the trace or `SafetyState`.
- **Danger zones.** `clamp_velocity` starts working: with kd > 0 the clamped `v_des` brakes.
  This changes behaviour for `elevated_shoulder_pitch_fall` (0.45 rad/s) and must be re-verified
  with the arm supported. `clamp_torque` is unchanged. The total-torque clamp runs after them:
  it may move `v_des` from the zone's limit toward `dq`, which caps braking at the torque cap.
- **Envelope.** Unchanged. Davout clamps q_des using `max(|v_des|, |dq_meas|)`. `v_des = v_c`
  never exceeds the velocity cap, so Davout's `|v_des| > cap` refusal is never hit.
- **Feedback velocity fault** (cap + 0.5 rad/s): unchanged. With continuous braking it should
  never trip in normal motion. A trip is a test failure.

### ADR 0038 degraded hold and the GravityComp rule

- **Degraded hold.** The holding joints use the same law. The freeze arms the reference at
  `(q, 0)`. The lower uses a planner capped at `lower_velocity_rad_s`.
  - τ_ff now contains only `τ_g + τ_fric (+ki·I)`. So the offline admission bound
    `max|τ_g| + max|Δτ_g| + tau_margin_nm ≤ cap` covers the real τ_ff once
    `tau_margin_nm ≥ fc + 0.5` (the I cap). Today τ_d is outside the bound.
  - Admission must be rerun with this margin before the cutover. The cutover must not ship if
    any joint fails admission.
  - Shed-subtree, deadline and refusal semantics do not change.
- **GravityComp rule.** τ_g stays model-only. No term in the new law raises kp or ki, or pulls
  harder, when the joint lags.
  - The deleted stuck pulls were exactly such masks, and the elbow trip shows the hazard.
  - A gravity or friction shortfall now shows up as steady time-scale stalls (`s → 0`) and as
    HoldTracking or AscentStall fuse trips. Both are the intended signal to fix the model.
  - GravityComp mode (`kp = kd = 0`, `τ_ff = τ_g`) is untouched.

### Observability (precondition)

The position trace gains `kd_mit` (wire), `v_des`, the post-Davout τ_ff, `s`, and `I`. It drops
`tau_d` and `friction_mode`. Decimation stays configurable, but bench qualification runs at
every tick.

*Phase 0 status.* Done. The trace appends `law`, `q_ref`, `dq_ref` (the reference the law
commands, for both laws), `time_scale` (`s`; legacy writes 0 while its planner is frozen, else 1),
`tau_i` (the integral torque), `kd_mit` and `tau_ff_wire` (Davout's last sent τ_ff, after the cap
and the rate limiter: binding ticks are `tau_ff_wire ≠ tau_ff_cmd`). `dq_mit` is `v_des`. Rows are
written after the Davout send. `tau_d` and `friction_mode` stay until the Phase 4 cutover because
the legacy law still fills them and `gravity-fit`, `analyze-position-trace.py` and the MCP read
the schema by name; the scaled law writes `tau_d = 0` and `friction_mode = reference`.

## Test plan

**Unit tests (Berthier):**

- Over random retarget storms: `|Δv_c| ≤ a_max·dt + ε`, `|Δq_des| ≤ v_max·dt + ε`, and
  `|q_r − q| ≤ e1 + v_max·dt` for a stuck plant (`q` constant).
- `τ_fric` is continuous and odd in v.
- The leaky integral never steps.

**Simulation** (`SimulationBus` and `ControlLoop`, `sim/` fixtures):

- Plant: per-joint inertia, Coulomb + static friction (fs 0.14, fc 0.08 Nm), gravity with
  ±50% model error.
- Feedback: q quantized to 0.383 mrad, dq quantized to 0.075 rad/s, one tick of transport
  delay.
- Gates:
  - no host τ_ff step > 0.05 Nm/tick outside retargets, and ≤ the rate limit at retargets;
  - velocity overshoot < 20%;
  - stuck-row share < 10%;
  - a 0.75 → 0.5 elbow descent with a −50% model: |dq| stays below cap + 0.5 and the fuses
    behave per `docs/safety.md`.

**FirmwareBus / wire tests:**

- Decode the MIT frames: kd constant across the move, `v_des` continuous, no frame with `kd`
  toggling, and τ_ff deltas within the gate.

**Bench metrics** (per joint, every-tick trace at 200 Hz, MCP `pi_joint_calibrate` and
`pi_bench_harness`), against today's baselines:

| Metric | Today (pitch) | Gate |
|---|---|---|
| stuck rows (`dq == 0` while `|v_c| > 0.05` and `s = 1`) | 30–37% | < 10% |
| velocity overshoot p95 / max | +95% / +170% | < 15% / < 30% |
| \|q − q_r\| p95 / max | 0.10 max | < 0.02 / ≤ e1 |
| host τ_ff step per tick (outside retarget) | up to ~1.6 Nm | ≤ 0.05 Nm |
| Davout τ_ff rate-limit binding ticks | not logged | 0 |
| Davout feedback-velocity trips | 1 (elbow) | 0 across the calibration suite |
| `v_des` or `kd` toggles at rest | ~500 per 75 s (elbow) | 0 |
| final error after 1 s | 4–6 mrad | ≤ max(2 counts, (fs − fc)/kp) |

### Phase 2 results (simulation)

`crates/berthier/src/position_hold_tests/law_gates.rs` drives both laws through production
`PositionHold::{apply_retarget, tick}` against a pitch-like plant: inertia 0.12 kg·m², gravity
2.7·sin q Nm, fs 0.14 / fc 0.08 / fv 0.02 Nm, a drive running the MIT law on its own velocity at
4 kHz with the 5 Nm limit, Davout's τ_ff cap and 60 Nm/s limiter, q on the RS03 grid with one
count of encoder noise, dq as the 5 ms grid difference, one tick of delay. The session is a
calibration-like 0 → 0.75 (1.25 rad/s), 0.80 and 0.70 (0.15 rad/s slews), → 0. Master pitch
gains; every scaled-PD parameter at its default. Plants: true gravity ×1, ×0.67, ×1.5, ×2 of the
model.

| Gate | Legacy (worst plant) | Scaled PD (worst plant) |
|---|---|---|
| host τ_ff step per tick outside retargets ≤ 0.05 Nm | 2.57 Nm (fails every plant) | 0.021 Nm |
| velocity excess ≤ max(20 % of plan, fs/kd) | 3.3–6.7× the bound with a model error (+104 %, +111 %, +209 %) | 0.91× the bound |
| kd / v_des / phase changes in the last 1 s of each rest | 12–190 | 0 |
| stuck rows (< 10 %) | 3–13 % | 3–4 % |
| fuse trips, max speed | none, 1.61 rad/s | none, 1.41 rad/s |

The wire gate `crates/berthier/tests/position_law_wire.rs` runs a dithering rest hold through
ControlLoop, Davout and the robstride encoder: `scaled_pd` sends one kd and one v_des code in all
300 frames; the legacy default toggles kd.

Deviations from the plan above, each forced by the simulation or by arithmetic:

- **The governor scales the reference speed limit, not its clock.** `v_ref` advances by a
  trapezoid step whose speed limit is `s·v_max`, with `|Δv_ref| ≤ a_max·dt`, `s` as specified
  while advancing would grow `|q_r − q|`, and `s = 1` while it closes the lead. Scaling virtual
  time freezes the reference's own braking at `s = 0`, so a retarget back toward a stuck joint
  never moves, and `s·v_r` with a frozen nominal `v_r` releases a stale velocity when `s`
  recovers. As a result `|Δv_c| ≤ a_max·dt` with no extra slew term.
- **The reference rides a discrete braking curve** and reverses, overshoots a too-short
  retarget and returns, all within `a_max·dt` per tick. It snaps to `(target, 0)` only from a
  speed within one tick of rest. The legacy trapezoid snaps velocity on reversal.
- **Stuck-plant lead bound.** From rest the lead stays below `e1 + e0`. A joint that jams at
  speed carries the reference up to `e1 + v²/(2·a_max)`; `e1 + v_max·dt` is unreachable for any
  continuous reference (pitch: 1.25²/9 = 0.17 rad > e1).
- **Static dead zone is `fs/kp`, not `(fs − fc)/kp`**, because `τ_fric(0) = 0` at rest.
- **Velocity-overshoot gate.** A PD with drive damping catches a lag `e` up at `(kp/kd)·e` above
  the reference speed, and a joint resting in its dead zone starts a move up to `fs/kp` behind.
  So the excess has a floor of `fs/kd` (pitch: 0.047 rad/s, 31 % of a 0.15 rad/s slew) for any
  law that does not snap its reference to `q`. The gate is therefore
  `excess ≤ max(20 % of plan, fs/kd)`. Against a strict 20 % the scaled law fails twice: 28 % on
  the exact-model 0.15 rad/s descent (legacy 17 %, because it resets its planner to `q`), and
  23 % on the ×1.5 slow approach. `J_eff·a_c` (once identified) and an identified `fs` are the
  levers; raising gains is not.
- **The same arithmetic applies to the bench metric.** One measured-velocity quantum
  (0.077 rad/s) is 51 % of a 0.15 rad/s slew. [INFERENCE: much of today's +95–108 % p95 is
  quantization.] Phase 3 must compute overshoot from Δq over ≥ 50 ms, or only where the plan is
  ≥ 0.4 rad/s.
- **Integral.** Stored as torque, so a gain change never steps it. It accumulates only with
  `ki > 0`, leaks otherwise, and is never reset by a retarget (the legacy law zeroed it).
- **`fo`** stays a constant term of τ_ff, outside the odd `τ_fric(v)`.
- **HoldTracking** judges the wire terms `kp·(q_des − q) + kd·(v_c − dq) + τ_ff`. Measured dq
  appears only in that fuse, never in torque.
- **Wave.** A wave-owned joint uses the wave sample as its reference, without the governor.
- **Not modelled:** the ADR's `J_eff·a_c` (no key), and the elbow 0.75 → 0.5 case as a separate
  scenario. The ×2 plant's 0.70 → 0 descent covers a −50 % model (+3 %, 1.29 rad/s, no trip).

### Phase 3 pitch trial (simulation, October 4, 2026)

Master selects `scaled_pd` for `right_shoulder_pitch` only. Its model is the applied 2026-10-04
wave fit (A 2.661, B 0.038 Nm; fc 0.353 Nm, fv 0). `law_gates`
`pitch_bench_trial_meets_bench_criteria` reads the master pitch entry and URDF and drives a
fitted plant: URDF inertia 0.074 kg·m², fs 0.37 Nm (the static-hold band), fc 0.353 Nm. The
session is 0 → 0.75 → 0.80 → 0.70 → 0 → −0.5 → 0. Variants: fv 0.33 (the unresolved
slope), inertia +2σ of the fitted ΔI (0.111), and gravity ±2σ_A (×0.96, ×1.04). kp stays 18.

Bench pass criteria, applied per move to the sim first:
- velocity overshoot ≤ 20 % of the planned speed (Δq over 50 ms);
- ≤ 0.01 rad past each stop;
- no single-tick τ_ff step > 0.05 Nm, retargets included;
- no predicted total-torque clamp.

With the ADR defaults (e1 0.12, e0 e1/4, integral band 0.1, leak 0.5, a_max 4.5), the trial fails:
- **τ_ff step 0.077–0.081 Nm.** `fc·k·a_max·dt` with the fitted fc.
- **Stop overshoot up to 27 mrad (fitted plant 18).** Without `J_eff·a_c` the PD supplies the
  braking torque `I·a_max` from a lead of `I·a_max/kp`, and a joint stopped within `fs/kp`
  (20 mrad) stays there.
- **Speed overshoot up to 21 %** on the slow 0.80 → 0.70 reversal: a 0.1 rad integral band
  carries the integral wound up on the previous approach into the next slew.

Tuned for the trial (pitch only):
- `position_trajectory_accel_rad_s2` 4.5 → 1.5. This is the reference's `|Δv_c| ≤ a_max·dt`.
- `position_integral_band_rad` 0.1 → 0.02, about the static dead zone `fs/kp`.
- e1 (0.12) and e0 (0.03) changed nothing in the sweep (the lead never reached e0), so they keep the
  defaults.
- Results, worst variant:
  - τ_ff step 0.028 Nm;
  - speed overshoot 11 % (fitted plant 6 %);
  - stop overshoot 8 mrad (fitted plant 4 mrad);
  - no predicted total-torque clamp (max 3.05 Nm);
  - max speed 1.08 rad/s, no fault.
- At ×0.94 gravity (3σ_A) the stop overshoot is 10 mrad, at the limit.

Costs of the lower acceleration:
- **Wave admission.** `marengo-pi` and `pi_joint_calibrate` admit pitch waves only up to
  `A·ω² ≤ 1.5 rad/s²`, about 0.3 rad/s at calibration amplitudes.
- **Jam lead.** A jam at speed leaves the reference up to `e1 + v²/(2·a_max)` ahead. Davout's
  total-torque clamp bounds the torque that lead could command.

The lever the ADR names, `J_eff·a_c`, would restore the acceleration. `J_eff` is now bounded by
the fit (ΔI −0.028 ± 0.034 kg·m² about the URDF), but the law has no key for it yet.

### Phase 3 pitch bench fix (October 4, 2026)

The motion suite (`var/motion-suite/20261004T1306…1311Z`) failed slow waves and slow moves
(track 0.031–0.049 rad), 0.31 rad/s waves (+22 %), 1.1 rad/s waves (+29–37 %) and one stop
(10.2 mrad). From the every-tick traces:
- **Friction falls with speed**: ≈ 0.6 Nm at 0.05 rad/s, 0.37 above 0.2 rad/s; the slope
  `(fs − fc)/v_b ≈ 3.5 Nm·s/rad` exceeded kd 3, so slow motion stick-slips. Gravity is within
  ±0.05 Nm everywhere (not the cause near 1.2 rad).
- **The danger zone `elevated_shoulder_pitch_fall`** clamped v_des to −0.45 rad/s above 0.5 rad;
  with kd it braked 1.1 rad/s descents (lag 0.1 rad) and released with a ~2 Nm step.
- **No `J·a` feed-forward**: braking came from a P lead of `J·a/kp`.
- **Position-periodic torque ripple** (28.6 cycles/rad) drives the 0.31 rad/s speed ripple.

Changes: `J·a` feed-forward with the URDF inertia at the zero pose, and `τ_fric + J·a` slewed at
6 Nm/s (0.03 Nm per tick); pitch kd 3 → 5, friction fs 0.65, v_b 0.08, k 10 → 15; the scaled-PD
reference and wave admission (`marengo-pi`, `pi_motion_suite`, `pi_joint_calibrate`) keep descents
above every `clamp_velocity` zone threshold within its speed (wave: `peak·√(1 − u²)`,
`u = (max(above, c) − c)/A`). The 0.45 rad/s zone value is an open operator question.
`position_hold_tests/bench_replay.rs` replays the suite against a plant fitted to the traces.

## Implementation plan

1. **Phase 0: quick fixes and observability.** Done.
   - `db492cd3` (overshoot snap) and `8f9cff94` (low-angle band).
   - The trace columns (see Observability).
2. **Phase 1: models first.** Shoulder pitch identified on October 4, 2026 (Phase 3 trial
   above); the other joints have not started.
   - Fix the elbow gravity model (~1.5× light).
   - Identify fs, fc and fv per joint from slow constant-velocity sweeps (GravityComp rule:
     before any gain change).
3. **Phase 2: the law, in simulation.** Done.
   - `crates/berthier/src/position_law.rs` holds the reference step, governor, friction and
     integral. `position_hold.rs` dispatches per joint and keeps the legacy path untouched.
   - The per-joint `position_law` selector is in place, with every joint on `legacy`.
   - Gates: see Phase 2 results.
4. **Phase 3: bench qualification, one joint at a time.** Pitch selects the law for its bench
   trial; score each run with `scripts/analyze-position-trace.py --score-bench`.
   - Entry gate: open question 1 is closed (Davout bounds the total; see Davout interactions).
     Check every run's bench log for total-torque clamps.
   - Order: pitch bare → pitch weighted → roll → elbow → yaws.
   - Use a per-joint `position_law: scaled_pd` key that exists only for the duration of this
     phase.
   - Verify the total-torque clamp and the danger-zone `clamp_velocity` behaviour with the arm
     supported.
5. **Phase 4: cutover and deletion, in one change.**
   - Remove the selection key, every heuristic and constant in the table above,
     `position_feedforward.rs`, most of `position_setpoint.rs`, and the friction modes.
   - Update ADR 0007 (superseded sections), `docs/safety.md` (position-hold fuses, the RS03
     note), `docs/position-hold-control-review.md`, `docs/tuning.md` and the codemaps.
6. **Phase 5: degraded admission.**
   - Rerun ADR 0038 admission with the new margin.
   - Run a degraded-lower drill on the bench with the arm supported.

## Alternatives considered

- **Keep host damping, but blend the gate continuously.** This removes the step in F2 but keeps
  the ~20 ms delay, the quantized-dq push during acceleration, and the double damping with the
  drive. It also keeps every repair heuristic.
- **Host velocity observer** (a Kalman filter on q plus the model) feeding host damping. This
  gives a better estimate, but the 200 Hz loop and CAN delay still limit the usable kd. It may
  come later as an input to fuses and telemetry, not to torque.
- **Raise kp, fc or ki to beat stiction.** Forbidden as a cure for model faults
  (`docs/safety.md`, GravityComp rule). It also amplifies the step kicks.
- **Drop the reference-velocity feed and send only q_des.** That loses the damping target; the
  drive kd would brake all motion toward zero velocity.

## Open questions

1. Does Robstride firmware clamp the *total* MIT torque at `limit_torque` (parameter 0x700B)
   in operation mode? **Closed October 4, 2026.** The vendor manual does not say so, and
   Marengo never reads the value. Davout now bounds the predicted total itself (Davout
   interactions, *Total-torque bound*).
2. What is the drive's velocity estimator window? It sets the usable kd. The reported quantum
   is ~0.075 rad/s; kd 3 means 0.23 Nm per count.
3. Should Phase 3 keep `e1 = position_slew_max_lead_rad` (0.10–0.12), or start smaller (lower
   breakaway push, more stall time)?
4. RS00 MIT ranges (disputed ±50/±17 vs ±33/±14) must be settled before `right_lower_arm_yaw`
   qualifies, because kd acts on the decoded velocity scale.
