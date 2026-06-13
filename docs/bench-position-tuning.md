# Bench position hold — systematic tuning

Weighted shoulder pitch `hold-at` is **not** eighteen independent knobs. It is four layers tuned **in order**, with CSV trace analysis between each step.

## Telemetry (already on every `pi_hold_on` / harness run)

Each bench motion sets `MARENGO_POSITION_TRACE` automatically (see `tools/marengo-pi-mcp` `benchLogWrapper`):

| Artifact | Path on Pi |
|----------|------------|
| Text log (1 Hz diagnostics) | `/opt/marengo/var/log/bench-*.log` |
| Position trace CSV (200 Hz default) | `/opt/marengo/var/log/position-trace-*.csv` |
| Symlink to latest trace | `position-trace-latest.csv` |
| **CAN candump** | **`candump-*.log`** (auto on motion) → `candump-latest.log` |

Set `MARENGO_POSITION_TRACE_HZ=50` only to shrink CSVs during long sweeps.

### CAN / control loop rates (RS03)

| Layer | Rate | Source |
|-------|------|--------|
| CAN **bitrate** | 1 Mbit/s | `hardware/docs/decisions/0001-can-and-motors.md` |
| Marengo **MIT command** | **`control.loop_hz` = 200 Hz** | `config/.../control.yaml` — one `OperationControl` frame per motor per tick |
| RS03 MIT **feedback** | **Host-paced** (~200 Hz expected) | Firmware responds to each MIT command with `OperationStatus`; no separate Marengo param |
| Berthier control tick | 200 Hz | Same as `loop_hz` |
| Position trace CSV | **200 Hz** (matches loop unless `MARENGO_POSITION_TRACE_HZ` set) | `MARENGO_POSITION_TRACE` |
| Text `position hold command` log | 1 Hz | Berthier `log_position_diag` |
| marengo-pi `feedback` log | 1 Hz | `debug_status` heartbeat |
| Chappe `RobotState` | 50 Hz | `chappe_state_hz` |
| Comm watchdog | ≥20 Hz implied | `comm_watchdog_ms: 50` — disable if no RX ~50 ms |

RS03 does **not** expose a fixed “CAN frequency” in our stack — MIT mode is **passive**: exchange rate = whatever the host sends (200 Hz today). Inner motor current loop is faster; that is not visible on CAN.

**Verify on bench:** while holding, run `scripts/measure-can-mit-rate.sh can0 2` — expect **~400 frames/s** on can0 with one right shoulder RS03 (200 TX + 200 RX).

**Debug jerk:** compare **200 Hz trace** (code/planner) vs **candump** (wire) vs **1 Hz text log** (operator eyeball only). If trace looks smooth but arm jerks, suspect firmware/MIT or mechanical load; if trace shows stairs at 200 Hz, suspect Berthier/Davout.

### Log retention

Timestamped `bench-*.log` and `position-trace-*.csv` per session; symlinks `bench-latest.log` / `position-trace-latest.csv` always point at newest.

- **Auto-prune:** harness / `pi_hold_on` keep **100 newest** of each pattern after every run
- **Manual:** `scripts/bench-log-prune.sh [/opt/marengo/var/log] [keep]`
- **Rough size:** ~40–80 KB/log, ~60–150 KB/trace @ 50 Hz for ~30 s; ~4× @ 200 Hz (default trace rate)

Check usage: `pi_logs_list` (includes total `du -sh`). After motion: **`pi_candump_summary`** for wire frame rate.

Local analysis:
```bash
python scripts/analyze-candump-log.py candump-latest.log
```

### Analyze a run locally

```bash
python scripts/analyze-position-trace.py /path/to/position-trace-latest.csv
python scripts/analyze-position-trace.py trace.csv --json > report.json
```

The script splits on **target changes** (approach vs return) and reports:

- overshoot and final settle error
- `lead_sat` fraction (arm outrunning planner)
- velocity lag (`dq` vs `dq_traj`)
- `tau_f` sign flips (friction fighting)
- `tau_ff` slew peaks (Davout rate-limit clipping)
- `dq_traj` stutter events (planner decel / lag fights)
- jerk RMS on measured `q`

**Do not change more than one layer between runs.** Compare JSON reports side-by-side.

### CSV columns (after trace enrichment)

| Column | Meaning |
|--------|---------|
| `q`, `dq` | Measured joint state |
| `q_traj`, `dq_traj` | Internal planner reference |
| `q_des`, `lead`, `lead_sat` | MIT setpoint and clamp |
| `tracking_error` | `q_traj − q` |
| `settle_error` | `target − q` |
| `dist_to_target` | `target − q_traj` (planner remaining) |
| `tau_p` | `kp × lead` (MIT P contribution) |
| `tau_ff_cmd` | Berthier FF before Davout |
| `estimated_tau` | `tau_ff_cmd + tau_p` (expected total) |
| `tau_meas` | Drive feedback torque |

Gap today: **post–rate-limit** `tau_ff` sent on CAN is not logged (Davout clips at `tau_ff_rate_limit_nm_per_s`). High `tau_ff peak slew` in the analyzer means clipping is likely.

## Tuning layers (order matters)

```text
1. Gravity comp only     → tau_g matches load at static poses
2. Small hold-at         → 0.1 rad move (proven workflow, ADR 0007)
3. Trajectory speed      → v, a, slew (planner only)
4. Impedance             → kp, kd (MIT P + FF damping)
5. Friction assist       → fc last (breakaway / coulomb)
```

### Layer 1 — Gravity (`GravityComp` mode)

- `motor-repl gravity-preview` / weighted sign procedure ([bench-weighted-gravity-sign.md](bench-weighted-gravity-sign.md))
- At arm-down and ~90°, `tau_meas ≈ 0` with zero command
- **Do not tune kp/kd or traj speed until this passes**

### Layer 2 — Small move (acceptance gate)

Use **`hold-at 0.1`** from arm-down (not full π rad):

**All** must pass (analyzer alone is not enough):

| Criterion | Target |
|-----------|--------|
| Operator | Smooth start/stop — no stair-step jerk or stick-slip |
| Fault | `fault=0` |
| `lead_sat` | < 10% |
| `jerk_rms` | < 800 rad/s² on approach segment (weighted 0.1 rad) |
| `tau_ff` peak slew | < 2× `tau_ff_rate_limit_nm_per_s` (else Davout clips → torque stairs) |
| `tau_f` sign flips | ≤ 2 on approach |

Impedance baseline: `kp=8`, `kd=1`, `slew=0.10`, `max_lead=0.05`, `fc=0.35`, `tau_ff_rate_limit=60`.

Moves below `position_trajectory_threshold_rad` (0.15) use **slew** only; large-move `v`/`a` do not apply.

Only after PASS → increase distance (0.3 → 0.8 → 1.57 rad).

**Analyzer gate:** `python scripts/analyze-position-trace.py <trace.csv> --gate layer2 --tau-ff-rate-limit 60` — checks numeric rows above; operator smoothness still required for final PASS.

**Gate FAIL (2026-06-13):** `bench-20260613T130424Z` — operator: **jerky / not smooth** despite `lead_sat=0%`.

| Segment | lead_sat | jerk_rms | tau_ff slew peak | Issue |
|---------|----------|----------|------------------|-------|
| → 0.1 rad | 0% | **1587** | **210 Nm/s** (limit 20) | FF clipped → torque stairs; `tau_f` hit ±0.5 Nm |
| → 0 rad | 0% | **1034** | 67 Nm/s | Same clipping pattern on return |

Trace shows `dq` jumping ~0.1 rad/s per 5 ms tick and `tau_f` saturating at full `fc` during slew — matches felt jerk.

**Next single knob (Layer 2 retry):** `tau_ff_rate_limit_nm_per_s` **20 → 60**; re-run weighted `hold-at 0.1` before distance steps.

**Gate FAIL (2026-06-13 retry):** `bench-20260613T130736Z` — `tau_ff_rate_limit=60` synced; analyzer still FAIL.

| Segment | lead_sat | jerk_rms | tau_ff slew peak | tau_f flips | Issue |
|---------|----------|----------|------------------|-------------|-------|
| → 0.1 rad | 0% | **1207** | **180 Nm/s** (>2×60) | 2 | FF still clipping; jerk above 800 |
| → 0 rad | 0% | **1664** | 99 Nm/s | 0 | Return jerk worse; `tau_f` hit ±0.5 Nm on descent |

**Next single knob:** lower **`fc` 0.5 → 0.35** *or* **`kd` 2 → 1** (one at a time); re-run `hold-at 0.1` with `--gate layer2`.

**Gate FAIL (2026-06-13):** `bench-20260613T130736Z` — operator: **still very jerky**; `tau_ff_rate_limit=60` did not fix feel.

**Gate FAIL (2026-06-13):** `bench-20260613T130849Z` — knob **`fc` 0.5 → 0.35**; FF clipping improved, jerk still high, return missed home.

| Segment | lead_sat | jerk_rms | tau_ff slew peak | settle_err | Issue |
|---------|----------|----------|------------------|------------|-------|
| → 0.1 rad | 0% | **1358** | 71 Nm/s ✓ | +3 mrad | jerk still >>800 |
| → 0 rad | 0% | **1429** | 58 Nm/s ✓ | **−26 mrad** | `tau_f` still ±0.35; did not reach home |

**Next single knob:** **`kd` 2 → 1**; re-run `hold-at 0.1`.

**Gate FAIL (2026-06-13):** `bench-20260613T130930Z` — knob **`kd` 2 → 1** (with `fc=0.35`).

| Segment | lead_sat | jerk_rms | tau_ff slew peak | settle_err | Issue |
|---------|----------|----------|------------------|------------|-------|
| → 0.1 rad | 0% | **1086** | **140 Nm/s** | −7 mrad | approach FF still clips |
| → 0 rad | 0% | **1379** | 52 Nm/s ✓ | **−5 mrad** ✓ | return home OK; jerk still high |

Return tracking improved vs `130849Z`; jerk still >>800 on both segments.

**Next single knob:** **`position_slew_rad_s` 0.20 → 0.10** (small-move profile — gravity ramps faster than constant slew can absorb).

**Gate FAIL (2026-06-13):** `bench-20260613T131043Z` — knob **`slew` 0.20 → 0.10**; operator: no visible improvement.

| Segment | lead_sat | jerk_rms | tau_ff slew | tau_f flips | Issue |
|---------|----------|----------|-------------|-------------|-------|
| → 0.1 rad | 0% | **1585** | 140 Nm/s | **6** | worse flips than kd=1 run |
| → 0 rad | 0% | **1144** | 53 Nm/s | 0 | settle −13 mrad |

**Diagnostic FAIL (2026-06-13):** `bench-20260613T131118Z` — **`fc=0`** (Coulomb assist off).

| Segment | lead_sat | jerk_rms | tau_ff slew | Issue |
|---------|----------|----------|-------------|-------|
| → 0.1 rad | 0% | **1665** | 25 Nm/s ✓ | jerk not friction-only |
| → 0 rad | **82%** | **1728** | 9 Nm/s ✓ | **never reached home** (−90 mrad) |

Proves breakaway friction contributes jerk, but removing it breaks gravity return (`lead_sat`).

**Gate FAIL (2026-06-13):** `bench-20260613T131200Z` — **`kp` 12 → 8** (fc=0.35, slew=0.10); return still −24 mrad, transient `lead_sat` on descent.

**YAML-only tuning stall:** impedance/friction/slew knobs not passing Layer 2 feel or analyzer. Likely needs **code** changes:

1. Widen slew-phase friction fade (`POSITION_HOLD_FRICTION_FADE_RAD` 5 mrad → ~20 mrad) or velocity-only Coulomb during `|dq_traj| > 0`.
2. Raise `max_lead` or allow faster gravity-assisted return slew.
3. Trapezoid accel/decel on small-move slew (not bang-on `dq_traj`).

**Next bench action:** friction fade **0.005 → 0.02 rad** in `berthier::friction` (coded, pending `pi_sync_main` — repo dirty).

### Layer 3 — Trajectory (planner)

| Knob | Affects |
|------|---------|
| `position_trajectory_velocity_rad_s` | Cruise speed large moves |
| `position_trajectory_accel_rad_s2` | Ramp smoothness |
| `position_slew_rad_s` | Small retargets after settle |
| `position_slew_max_lead_rad` | How far MIT setpoint may lead/lag measured `q` |

**Symptom → knob**

| Analyzer hint | Fix direction |
|---------------|---------------|
| High `lead_sat` | Lower `kp` or raise `max_lead`; do not raise `v` first |
| `dq_traj stutter` | Lower `v`/`a`; gravity descent outruns planner |
| Return incomplete | More `return_home_sec`, not necessarily higher `v` |

### Layer 4 — Impedance

| Knob | Affects |
|------|---------|
| `impedance.kp` | Hold stiffness; too high → arm outruns planner on approach |
| `impedance.kd` | FF damping (`tau_d`); trajectory and settle phases |

Raise `kp` only when settle error at hold is consistent; raise `kd` when velocity lag RMS is high without overshoot.

### Layer 5 — Friction

| Knob | Affects |
|------|---------|
| `friction.fc` | Breakaway assist; full ±fc during trajectory motion |
| `friction.k` | Velocity-near-zero friction shape |

Lower `fc` when `tau_f sign flips` is high or motion feels sticky/jerky.

### Global limits

| Knob | Location |
|------|----------|
| `tau_ff_rate_limit_nm_per_s` | `control.yaml` root — caps FF slew (20 default; 60 in recent trials) |
| `max_joint_velocity_rad_s` | `robot.yaml` — Davout disable threshold |

## Why recent full-range runs felt worse

Raising **v, kp, and fc together** guarantees fighting:

1. Fast planner + high `kp` → arm runs ahead → `lead_sat` brakes → jerk
2. High `fc` → ±0.9 Nm flips during trajectory
3. High `tau_ff` demand → rate limiter clips → torque stairs

**Recovery:** revert to Layer 2 baseline, PASS small move, then add **one** change per run with trace diff.

## Current config (`shoulder_pitch_right_only`)

Synced on Pi after Layer 2 recovery (2026-06-13):

| Layer | Values |
|-------|--------|
| Impedance | `kp=12`, `kd=2` |
| Slew / lead | `0.20` / `0.05` |
| Trajectory (≥ 0.15 rad) | **`v=0.72`**, **`a=0.36`** |
| Friction | `fc=0.5` |
| FF rate limit | **`60`** Nm/s (was 20 — caused clipping/jerk on gate attempt) |

Large-move speed is **pre-set** for fast approach and return; impedance/friction were reverted after failed `kp=18` trials. Do not raise `kp` or `fc` while stepping distance.

## Recommended next bench session

Layer 2 **FAIL** — retry gate before distance steps:

1. Config: `tau_ff_rate_limit=60` synced (`pi_sync_bench_config`)
2. Weighted `hold-at 0.1` → return; operator + analyzer must pass **all** Layer 2 criteria
3. If still jerky with slew OK: next single knob is **`fc` down** (0.5 → 0.35) or **`kd` down** (2 → 1) — not both
4. After Layer 2 PASS → distance steps: 0.3 → 0.8 → 1.57 rad
5. Full 90°: use **`return_home_sec: 20`**

MCP example (90° after intermediate steps pass):

```json
{
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 1.570796,
  "timeout_sec": 25,
  "return_home_sec": 20,
  "set_zero": false
}
```

See also [tuning.md](tuning.md), [ADR 0007](decisions/0007-bench-position-trajectory-control.md).
