# Bench position hold — systematic tuning

Weighted shoulder pitch `hold-at` is **not** eighteen independent knobs. It is four layers tuned **in order**, with CSV trace analysis between each step.

## Telemetry (already on every `pi_hold_on` / harness run)

Each bench motion sets `MARENGO_POSITION_TRACE` automatically (see `tools/marengo-pi-mcp` `benchLogWrapper`):

| Artifact | Path on Pi |
|----------|------------|
| Text log (1 Hz diagnostics) | `/opt/marengo/var/log/bench-*.log` |
| **Position trace CSV** | **`loop_hz` (200 Hz default)** | `/opt/marengo/var/log/position-trace-*.csv` |
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

- Target: smooth, no fault, `lead_sat` < 10% in analyzer
- Known-good starting point (commit `f6d55b3`): `kp=12`, `kd=1`, `slew=0.20`, `max_lead=0.05`, `fc=0.5`
- Only after PASS → increase distance (0.3 → 0.8 → 1.57 rad)

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

## Recommended next bench session

1. Revert `shoulder_pitch_right_only/control.yaml` toward small-move baseline (`v=0.12`, `kp=12`, `kd=2`, `max_lead=0.05`, `fc=0.5`, `slew=0.20`)
2. `pi_sync_bench_config`
3. Weighted `hold-at 0.1` only — analyze trace; must PASS
4. Step `v` by +25% per run until analyzer flags stutter or overshoot
5. Then tune `kp` for 90° hold settle error only

See also [tuning.md](tuning.md), [ADR 0007](decisions/0007-bench-position-trajectory-control.md).
