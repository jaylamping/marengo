# Weighted right shoulder pitch — 700 g bench test suite results

Load: **0.7 kg**, COM **18 in (0.4572 m)** end-loaded arm.
Config: `arm_2dof_right` (loaded URDF, `git 3a87be2`).
Tuning: kp=8, kd=1.25, v=2.0, a=4.8, slew=0.15, τ_ff rate limit 60, friction fc=0.25/k=10.
Velocity guard: Davout disables on corroborated feedback velocity > 2.0 rad/s.

Plan: [`weighted_arm_test_suite_72fde45f`](../../Users/joeyl/.cursor/plans/weighted_arm_test_suite_72fde45f.plan.md). Operator: supervision only (no hand interaction) this session.

---

## Phase 0 — Deploy + calibrate (no motion) — PASS

- Loaded URDF deployed to Pi via direct Windows SSH + per-path `git checkout origin/main` (cross-build via WSL/Docker unavailable on host). Verified live: `mass=0.7`, inertial COM `z=-0.4572`.
- `control.yaml` / `robot.yaml` on Pi match plan authority; robot.yaml → loaded URDF.
- `pi_set_zero` at arm-down with load: `pos=0.0000 rad`, homing **Verified**. CAN can0 UP (1 Mbit, 0 bus errors).

## Phase 1 — Read-only τ_g table — PASS (deploy functionally confirmed)

| q (rad) | τ_g (Nm) | model 0.7·9.81·0.4572·sin q | bound | result |
|---|---|---|---|---|
| 0 | 0.0000 | 0 | <0.3 | PASS |
| 0.3 | 0.9278 | 0.928 | — | PASS |
| 0.785 | 2.2191 | 2.220 | ~2.2 | PASS |
| 1.57 | 3.1396 | 3.140 | 2.8–3.4 | PASS |
| −0.3 | −0.9278 | −0.928 | sign flip | PASS |
| −0.5 | −1.5052 | −1.505 | symmetric | PASS |

Peak τ_g 3.14 Nm < 5.0 Nm cap (no clipping). Sign stable/symmetric. IMU samples present (accuracy Unreliable, non-fatal).

## Phase 0b — Sign check (gravity-on backdrive @ arm-down) — PASS

`gravity-on` at q≈0: `operational=Active`, `GravityComp`, commanded torque ±0.04 Nm, **no runaway**, clean disable, `fault=0x0000`. Sign not inverted. (Hand-backdrive feel deferred — operator not interacting.)

## Phase 2 — Layer 2 hold gate (0 → 0.1 → 0) — FUNCTIONAL PASS / SMOOTHNESS FAIL

Trace `position-trace-20260619T150158Z.csv`. Reached 0.083 rad (settle_err +0.017 < 0.02), returned to 0.003 (< 0.03), `fault=0x0000`, no trip, overshoot 0, lead_sat 0%, τ_f flips 0, home_start OK.

`analyze-position-trace --gate layer2 --tau-ff-rate-limit 60 --require-home-start`: **FAIL**
- `jerk_rms` 1295–1560 (gate <800)
- `tau_ff peak slew` 144 Nm/s (gate <120)

**Root cause:** friction-comp FF (`tau_f`) discontinuity at the Accelerate→Cruise planner phase boundary — `tau_f` pinned +0.25 Nm in Accelerate, abruptly 0 in Cruise; combined with `tau_d` flip, `tau_ff_cmd` steps ~0.72 Nm in one 5 ms tick → 144 Nm/s. Part of stationary jerk is encoder-quantization artifact (segment 1 hold also ~1560). Bounded (0.72 Nm « 5 Nm cap), not a stability/safety issue. Follow-up: soften friction FF across phase boundary (berthier) or lower fc/k.

## Phase 4 — Distance ladder (hands-off, supervised) — PASS

All round-trips 0 → target → 0 at trajectory v=2.0. No velocity trips; raw-velocity noise spikes (2–4 rad/s) ignored by Davout as **uncorroborated** (position-derived velocity stayed < 1.75). `fault=0x0000` every run.

| Step | Target (rad) | Peak ↑dq / ↓dq (rad/s) | Max pos | Dwell err (rad) | τ_meas / τ_g (Nm) | Return | Trace / candump |
|---|---|---|---|---|---|---|---|
| 4.1 | 0.3 | 0.815 / — | 0.326 | +0.026 | 0.55 / 1.00 | 0.006 | trace-150624 |
| 4.2 | 0.785398 | 1.438 / — | — | +0.053 | 1.66 / 2.33 | 0.004 | trace-150749 |
| 4.3 | 1.570796 | 1.702 / −0.935 | 1.701 | +0.051 | 2.48 / 3.14 | 0.004 | trace-151130 / candump-151130 (4964 fr) |
| 4.4 | 2.0 | 1.750 / −0.911 | 2.141 | +0.041 | 2.22 / 2.80 | 0.007 | trace-151338 / candump-151338 (4348 fr) |

**Headline:** the actuator lifts and holds the 700 g / 18-in arm through **π/2 and 2.0 rad** under gravity comp + position control with **no velocity trip**. The bare-motor descending-retarget trip did not reproduce (descents peaked < 0.95 rad/s under this planner/load).

**Finding — gravity over-compensation (~27%):** hold τ_meas runs ~0.73× the model τ_g at every angle ⇒ effective m·g·L ≈ 2.4 Nm peak vs modeled 3.14 ⇒ real COM ≈ **14 in**, not 18 in (dowel/structure mass distributed inward of the tip). Effect: arm rides +26→+53 mrad above target and overshoots on arrival. Mass (0.7 kg) is correct; refine URDF COM origin z to ≈ −0.36 m to tighten hold accuracy. Plan risk #3 (floaty) confirmed and quantified.

## Phase 5 — Limits & modes — PARTIAL

- **Lower soft-limit envelope clamp — PASS.** `hold-at -0.85` (soft limit −0.873) was clamped by the velocity-scaled envelope to **−0.6417** (`hold-at target clamped to limit envelope`), `lead_sat=true`. Planner refused to drive into the limit. Upper clamp (+3.0) skipped — same symmetric code path; near-inverted pose risks over-center flop for no new info.
- **Hold function** exercised throughout via `hold-at 0` settles (drift < 6 mrad).
- **Impedance push (5.1), upright-release (3.3)** — deferred: require operator hand interaction.

## Phase 6 — Recovery — PASS

- **Negative-retarget velocity trip (Track B, reproduced under load):** the `hold-at -0.85` move (clamped to −0.6417) free-accelerated in the negative direction to **−2.56 rad/s** (raw −6.5; both raw and position-derived > 2.0 ⇒ **corroborated**). Davout WARN `feedback velocity limit exceeded trips=2` → disabled at q≈−0.15. Arm swung back to home. **Direction-specific:** every positive move (≤ 2.0 rad) was trip-free; the first negative retarget tripped.
- **`pi_motor_recover` — RECOVER_OK** (`fault=0x0000`): homing re-verified → Ready, supervisor re-enabled Active, clean status.
- **Re-hold @ 0 after recover — PASS:** `pos=0.0050`, CAN 62.1/s, `fault=0`.
- **Deliberate disable mid-hold @ 0.5 rad:** on `disable` the motor goes limp; arm sags to home under gravity. CAN feedback **freezes** at the last frame (stale `vel` artifact) — expected robstride disabled behavior. No torque spike, `fault=0`.

---

## Verdict — PASS WITH WARNINGS

**Capability confirmed (700 g / 18-in load):** gravity comp engages with correct sign and no runaway; position control lifts and holds through **0.3 → π/2 → 2.0 rad** with no trips; descents stay < 1 rad/s; soft-limit envelope clamps before limits; trip→recover→re-hold cycle is clean.

**Warnings / follow-ups (none safety-blocking):**
1. **Gravity over-compensation ~27%** — refine URDF COM to ≈ −0.36 m (effective ~14 in). Causes +26→+53 mrad hold offset and overshoot.
2. **Layer 2 smoothness FAIL** — friction-FF (`tau_f`) discontinuity at the Accelerate→Cruise planner boundary (0.72 Nm step, 144 Nm/s). Soften FF blend or lower `fc`/`k`.
3. **Negative-retarget velocity trip (Track B)** — negative-direction retargets free-accelerate past the 2.0 guard. Positive direction unaffected. Investigate planner/gravity-sign behavior on negative descents; consider lower bench trajectory v for negative moves.

**Deferred (need operator hands):** hand-backdrive feel, upright-release stability, impedance push-resistance.
