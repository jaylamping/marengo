# Bench position hold — systematic tuning

Weighted shoulder pitch `hold-at` uses **one joint-space control law** (ADR 0007 one-pass). See [rust-patterns.md](rust-patterns.md) §7 for the full formula. Talleyrand will supply joint refs from Cartesian primitives later; bench `hold-at` hits the same executor.

## Position hold control law (reference)

| Layer | Rule |
|-------|------|
| Planner | Always trapezoid toward latched target; small moves (≤ ~60 mrad) cap `v_max` at `position_slew_rad_s` |
| MIT setpoint | `q_des = clamp(q_ref, q, target, max_lead)` |
| Stiffness | `tau_p = kp * (q_des − q)` |
| Damping FF | `tau_d = kd * (dq_ref − dq)` while `\|dq_ref\| > deadband`; else `-kd * dq` when settled |
| Friction FF | `traj_vel` if `\|dq_ref\| > deadband` and moving toward target; else `settle` fade. Within `traj_vel`: full `fc` pulse for 300 ms after retarget while stuck; ramp Coulomb with `\|dq_ref\|` when stuck past onset; zero Coulomb if measured `dq` outruns `dq_ref` (overspeed brake). See ADR 0007 (2026-06-15 update). |
| MIT wire | `velocity_rad_s = dq_ref`, `kd_mit = 0`, `tau_ff = tau_g + tau_f + tau_d` |

**Config (right-only bench, locked 2026-06-13):** `position_trajectory_threshold_rad: 0`, `kp=8`, `kd=1.25`, `slew=0.15`, `max_lead=0.10`, **`v=2.0`**, **`a=4.8`**, `fc=0.25`, `tau_ff_rate_limit=60`. Target **~1.2 s** for weighted 0→90° (π/2 rad ≈ 1.571 rad; mean **1.31 rad/s / 75 deg/s**); `v` at rs03 cap.

**Layer 2 script:** 5 s settle dwell after `hold-at 0`; analyzer `--require-home-start` enforces `\|q\| < 5 mrad` at approach onset.

## Session handoff (2026-06-13)

Copy this block into a new chat to resume Layer 2 bring-up.

### Goal

**Layer 2 gate:** weighted right shoulder pitch **`hold-at 0.1 rad`** from arm-down, return home. **All** must pass:

| Criterion | Target |
|-----------|--------|
| Operator | Smooth — no stair-step jerk or stick-slip |
| Fault | `fault=0` |
| `lead_sat` | < 10% |
| `jerk_rms` | < 800 rad/s² (both segments) |
| `tau_ff` peak slew | < 2× `tau_ff_rate_limit_nm_per_s` |
| `tau_f` sign flips | ≤ 2 on approach |

Analyzer: `python scripts/analyze-position-trace.py <trace.csv> --gate layer2 --tau-ff-rate-limit 60`

Operator smoothness is **required** even if analyzer passes.

### Environment

| Item | Value |
|------|-------|
| Pi | `joey@joey-robot.tail0b414.ts.net` |
| Config | `shoulder_pitch_right_only` |
| Motor | `can0/id2` (`right_shoulder_pitch`) |
| Deploy rev | `2d95ee5` local Pi build includes accel-limited small slew + `dq_traj` FF handoff fix; repo commit `2d95ee5` fixes MCP candump summary parsing |
| Profile | `weighted_single_arm` via `pi_hold_on` |

### Testing done (2026-06-13)

Systematic one-knob-at-a-time tuning after repeated operator **FAIL — still very jerky**:

| Session / change | Approach jerk | Return | lead_sat | Notes |
|------------------|---------------|--------|----------|-------|
| `130424Z` baseline kp=12, fc=0.5, slew=0.20, τ_ff limit 20 | 1587 | 1034 | 0% | FF clipped 210 Nm/s |
| `130736Z` τ_ff limit 20→60 | 1207 | 1664 | 0% | Still jerky |
| `130849Z` fc 0.5→0.35 | 1358 | 1429 | 0% | Return −26 mrad |
| `130930Z` kd 2→1 | 1086 | 1379 | 0% | Return −5 mrad (best home so far) |
| `131043Z` slew 0.20→0.10 | 1585 | 1144 | 0% | 6 τ_f flips on approach |
| `131118Z` **fc=0 diagnostic** | 1665 | 1728 | **82% return** | Jerk not friction-only; return never home |
| `131200Z` kp 12→8 | ~650* | −24 mrad | transient | *Unfair start ~0.09 rad |
| `131510Z` post-deploy 05b8bf6 | 1736 | 85% lead_sat | stuck −102 mrad | Arm not at arm-down |
| `131808Z` **max_lead 0.05→0.10** | 1627* | 976 | **0%** | *Started already at 0.105 — bogus approach |
| **`131850Z`** fair run 0.032→0.1→0 | **1318** | **1047** | **0% both** | Return ends **−28 mrad**; fault=0; τ_ff slew OK |
| **`142555Z`** post code fix 0.018→0.1→0 | **2161** | **1714** | **0% both** | Fault=0; target settles +1.25 mrad; home +0.77 mrad; approach overshoot +13.1 mrad |

**Current config on Pi** (after `142555Z`):

| Knob | Value |
|------|-------|
| Impedance | `kp=8`, `kd=1` |
| Slew / lead | `0.10` / **`0.10`** |
| Friction | `fc=0.35` |
| FF rate limit | `60` Nm/s |
| Large move (≥ 0.15 rad) | `v=0.72`, `a=0.36` (unused at 0.1 rad gate) |

### Findings

1. **YAML-only tuning stalled** on jerk gate (~1000–1700 vs 800 target).
2. **Code fix improved settling:** after accel-limited small slew + `dq_traj` FF handoff, target hold ends within **+1.25 mrad** and return home within **+0.77 mrad**.
3. **`lead_sat` is solved** for the 0.1 rad gate at `max_lead=0.10` (0% on approach and return).
4. **Remaining blocker is dynamic smoothness:** approach still overshoots **+13.1 mrad** and jerk remains above gate (`2161` approach / `1714` return vs `<800` target).
5. **Friction still contributes:** `tau_f` had 17 sign flips on approach in `142555Z`, so breakaway/Coulomb handoff needs another pass.

### Next steps

1. **Operator feel** on `142555Z` — smooth enough, stair-step, or still visibly jerky? The trace says better settle, not yet smooth.
2. **Re-run CAN summary** with fixed `pi_candump_summary` after MCP reload; confirm ~400 frames/s on `can0` and no wire-rate dropout.
3. **Reduce approach overshoot:** lower small-move slew/accel or add earlier decel for the 0.1 rad gate; keep `max_lead=0.10` unchanged.
4. **Tame friction handoff:** reduce `fc` or widen/fade Coulomb near target to cut `tau_f` sign flips without breaking gravity return.
5. After Layer 2 PASS → distance steps **0.3 → 0.8 → 1.57 rad** (use `return_home_sec: 20` for 90°).

### Latest artifacts (Pi)

| Session | Trace | Candump |
|---------|-------|---------|
| `142555Z` (latest post code fix) | `position-trace-20260613T142555Z.csv` | `candump-20260613T142555Z.log` |
| `131850Z` (best fair run) | `position-trace-20260613T131850Z.csv` | `candump-20260613T131850Z.log` |
| Symlinks | `position-trace-latest.csv`, `candump-latest.log` | |

Path: `/opt/marengo/var/log/`

---

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

- **Hot keep:** harness / `pi_hold_on` keep **50 newest** of each pattern; older files gzip to `var/log/blobs/` via `marengo-log-cli archive`
- **Archive:** SQLite `log_sessions` + gateway `GET /logs/sessions` (Consul Archive tab, MCP `pi_logs_archive_list`)
- **Purge:** daily timer — `marengo-log-cli purge --days 30` + journal import
- **Manual:** `scripts/bench-log-prune.sh [/opt/marengo/var/log] [keep]`
- **Rough size:** ~40–80 KB/log, ~60–150 KB/trace @ 50 Hz for ~30 s; ~4× @ 200 Hz (default trace rate)

Check usage: `pi_logs_list` (gateway sessions + hot `du -sh`). After motion: **`pi_candump_summary`** for wire frame rate.

### MCP read-only access (no motion)

| Tool | Use |
|------|-----|
| `pi_logs_list` | Gateway sessions + hot files + disk usage |
| `pi_logs_archive_list` | Archived sessions (gateway SQL or blob dir) |
| `pi_logs_tail` / `pi_logs_grep` | Search bench/journal output |
| `pi_logs_last_fault` | Last motor fault |
| `pi_journal` | systemd / marengo-pi journal |
| `pi_candump_summary` | Frame rate on latest candump |
| `pi_candump_once` | Live CAN snapshot |
| `pi_read_file` | Pull trace/log from Pi |
| `pi_health` | Deploy rev, binaries, CAN up |
| `pi_motor_repl_status` | Motor state without motion |
| `pi_gravity_preview` | Static gravity comp check (Layer 1) |

### Supplementary (not every hold run)

| Source | Rate | Notes |
|--------|------|-------|
| Chappe `RobotState` | 50 Hz | IPC fanout when marengo-pi runs |
| IMU (`torso_imu`) | 50 Hz | i2c publisher thread in marengo-pi |
| `motor-repl` | interactive | Layer 1 gravity sign / static checks |

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
| `dq_traj` | Planner reference velocity |
| `q_traj` | Internal planner position (**1 Hz text log only** — not in CSV header today) |
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

Impedance baseline: `kp=8`, `kd=1`, `slew=0.10`, `max_lead=0.10`, `fc=0.35`, `tau_ff_rate_limit=60`.

Moves below `position_trajectory_threshold_rad` (0.15) use **slew** only; large-move `v`/`a` do not apply.

Only after PASS → increase distance (0.3 → 0.8 → 1.57 rad).

**Analyzer gate:** `python scripts/analyze-position-trace.py <trace.csv> --gate layer2 --tau-ff-rate-limit 60` — checks numeric rows above; operator smoothness still required for final PASS.

Per-segment metrics emitted: overshoot, settle error, `lead_sat` fraction, tracking RMS, velocity lag RMS, `tau_f` sign flips, `tau_ff` peak slew, `dq_traj` stutter events, jerk RMS on `q`, phase counts, Layer 2 pass/fail breakdown. Use `--json` for diffing runs.

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

**Next bench action:** friction fade **0.005 → 0.02 rad** deployed in `05b8bf6`. **`max_lead` 0.05 → 0.10** synced on Pi — fixes return `lead_sat`; home offset ~28 mrad persists. Layer 2 still FAIL on jerk; see **Session handoff** at top.

**Gate FAIL (2026-06-13):** `bench-20260613T131850Z` — fair run from q≈0.032; `max_lead=0.10`.

| Segment | lead_sat | jerk_rms | tau_ff slew peak | settle_err | Issue |
|---------|----------|----------|------------------|------------|-------|
| → 0.1 rad | 0% | **1318** | 45 Nm/s ✓ | −7 mrad | jerk >>800; τ_f flips=2 |
| → 0 rad | 0% | **1047** | 55 Nm/s ✓ | **−28 mrad** | no lead_sat; still short of home |

**Gate FAIL (2026-06-13):** `bench-20260613T131808Z` — `max_lead=0.10`; arm already at 0.105 (not arm-down). Approach segment not comparable.

**Gate FAIL (2026-06-13):** `bench-20260613T131510Z` — post-deploy `05b8bf6`; return lead_sat 85%, −102 mrad.

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

**Locked on Pi 2026-06-13** after Berthier return-freeze fixes (`3f66ea2`) and full-range retest:

| Layer | Values |
|-------|--------|
| Impedance | `kp=8`, `kd=1.25` |
| Slew / lead | **`0.15`** / **`0.10`** |
| Trajectory (moves > ~60 mrad) | **`v=2.0`**, **`a=4.8`** (~1.2 s planner for π/2 rad at motor cap) |
| Friction | `fc=0.25` |
| FF rate limit | **`60`** Nm/s |

**Verified weighted script** (`hold-at 0` → `1.570796` → `0`, `weighted_single_arm`):

| Leg | Wall time (2026-06-13) | Notes |
|-----|------------------------|-------|
| 0 → 90° | **~1.0–1.2 s** | `dq_traj` cruises at 2.0 rad/s; brief overshoot ~97° |
| 90° → 0 | **~3 s** | No `planner_frozen`; `dq_traj=−2.0` on descent |
| Faults | `fault=0x0000` | |

Moves ≤ **`POSITION_SMALL_MOVE_VMAX_RAD` (~60 mrad)** cap cruise at `position_slew_rad_s` (0.15); full sweeps use trajectory `v`/`a`.

Do not raise `kp` or `fc` while stepping distance. Lower `v`/`a` if operator reports harsh overshoot or velocity trips.

## Recommended next bench session

Layer 2 **FAIL** — see **Session handoff** at top. Before distance steps:

1. Arm at mechanical down (~0 rad) for fair 0→0.1→0 gate
2. Weighted `hold-at 0.1` → return; operator + `--gate layer2` analyzer
3. If still jerky: **code** changes (velocity Coulomb, slew accel/decel) — YAML-only likely exhausted
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

## Phase C — firmware kd experiment (deferred)

**Do not enable until Layer 2 analyzer + operator pass on the one-pass baseline.**

Preconditions:

- ≥3 consecutive weighted Layer 2 sessions with `--gate layer2 --require-home-start --tau-ff-rate-limit 60`.

Protocol (document only until then):

1. Add config flag `position_use_firmware_kd: false` (default off).
2. When enabled: `kd_mit = 0.25 × kd`; reduce FF `tau_d` by the same fraction to avoid double damping.
3. Compare traces: jerk RMS, `tau_ff` slew, operator feel at approach onset.
4. Abort if Davout velocity-limit trips increase or approach hitch regresses vs baseline.
