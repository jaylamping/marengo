# Bench: calibrated 90° round trip (2026-06-20)

Weighted right-shoulder-pitch session after frame calibration and set-zero at
mechanical home. Complements [position-hold-control-review.md](position-hold-control-review.md).

## Sessions

| Session | Script | Outcome |
|---------|--------|---------|
| `bench-20260620T002735Z` | 0 → 1.484 → 0 | **No motion** — arm at q ≈ −0.054 rad; homing verified but past home |
| `bench-20260620T002912Z` | retry + gravity-on | Same — stuck at −0.054 rad |
| `bench-20260620T003624Z` | set-zero then 0 → 1.484 → 0 | **Full trip** — valid overshoot/settle data |

## Frame calibration

- Software **π/2 (1.571 rad)** ≈ visual **~95°** on this bench.
- **`hold-at 1.484 rad`** ≈ visual **90°** (operator confirmed on offset check and round trip).
- Offset ≈ **−0.087 rad (−5°)** between encoder zero and protractor at this mounting.
- After coast past 1.484 in an earlier session, encoder read **−3° at “home”** until operator
  re-seated the arm and **`pi_set_zero`** ran (`pos=0.0000 rad`, homing Verified).

## `bench-20260620T003624Z` — trace summary

Analyzer on `position-trace-20260620T003624Z.csv`:

| Segment | Target | Peak q | Overshoot vs target | lead_sat | Notes |
|---------|--------|--------|---------------------|----------|-------|
| Outbound | 1.484 rad | 1.612 rad (92.4°) | +0.128 rad (+7.3° enc.) | 91% | Operator: stopped on visual 90°, ~2° coast felt |
| Return | 0 | — | 0 | 8% | Smooth descent; PR #50 return fix holds |

### Outbound (operator + trace)

- Commanded **1.484 rad** (mechanical 90°).
- Trace peak **1.612 rad**; operator reports **~2° past** visual 90°, not +7°.
- Residual issue is **decel/coast** (latch churn ~2053 events), not wrong target frame.

### Return (operator + trace)

- Return command at **q ≈ 1.60 rad** (outbound overshoot pose).
- Active descent **~1.4 s** (16.8 s → 18.2 s into session).
- Arm latched **Hold at q ≈ 0.031 rad (~1.8°)** for **~13 s** before disable.
- Operator: return **much better** than pre–PR #50; did **not** reach full home (~2° high).
- Operator: **no visible motion at disable** (no post-disable sag).
- Disable was **not** mid-descent — scripted 15 s dwell after hold-at 0.

### Root cause (return home error)

`POSITION_RETURN_RESYNC_RAD` = **0.03 rad**. Measured stop **0.031 rad** is just
inside that band, so:

- `planner_premature_hold` does not reopen (target=0 branch only handled undershoot below zero).
- `planner_drifted_from_measurement` does not resync (`0.031 ≯ 0.03`).

Planner declares Hold while arm is still above home; settle torque holds ~2° offset.

**Fix (implemented):** `POSITION_HOME_SETTLE_RAD` = **0.005 rad** when `target == 0`;
`planner_premature_hold` symmetric on `|q − target| > band`. Bench verify pending.

## Bench protocol (weighted)

```text
home → enable → hold-at 0 → sleep 5 → hold-at 1.484 → sleep 12 → hold-at 0 → sleep 15 → disable → quit
```

- Mechanical 90° target: **`1.484 rad`**, not `1.570796`.
- Re-seat at mechanical home + **`pi_set_zero`** if encoder reads past home (e.g. q < −0.02).
- `sleep 15` on return is dwell **after** motion should finish — not a substitute for settle tuning.

## Open items

1. ~~Home return settle~~ — implemented (`return_settle_band`); re-run weighted 0 → 1.484 → 0 to verify `|q| < ~0.005`.
2. Outbound coast — latch/decel at 1.484 (overshoot-hold / `kd_mit` branch work).
3. Document `MECHANICAL_90_RAD = 1.484` in bench config or bringup constants.

## Related

- [bench-position-tuning.md](bench-position-tuning.md) — Layer 2 gate
- PR #50 — descent return jitter fix
- Trace: `/opt/marengo/var/log/position-trace-20260620T003624Z.csv`
