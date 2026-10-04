---
name: trace-forensics
description: Explain a Marengo joint's motion from its position-trace.csv - score it against the ADR 0039 bench bar, decompose torque into gravity, friction, inertia, gain and clamp contributions, and attribute tracking error, overshoot, drift or sag to a cause with numbers. Use after every motion or calibration suite, and whenever a joint moves, holds or settles differently than expected (lags, overshoots, oscillates, creeps, sags, stalls, slips, or trips AscentStall/HoldTracking/WaveStall).
---

# Trace forensics

`position-trace.csv` records, per tick, what the law asked for, what Davout sent and what the joint did. The task is to find which torque term is wrong, by how much, and where in the motion. A finding names a number, the rows it comes from, and the term responsible.

## 1. Get the trace and its context

- Suites write `var/motion-suite/<TS>/` or `var/gravity-calibration/<TS>/`: `position-trace.csv`, `plan.json` (intended moves), `bench-session.txt` (events), `config/` and `pi-marengo.urdf` (the gains and model the run actually used, which may differ from the repo).
- Other sessions leave `/opt/marengo/var/log/position-trace-<TS>.csv` on the Pi (copy it as in `can-timeline`).
- Check coverage: the swept joint must be traced every tick (`MARENGO_POSITION_TRACE_FULL_RATE_JOINTS`, which the suites set) for acceleration and per-tick checks. Decimated rows support position statistics only.

**Columns.** `t_ms` is ticks × 5 ms.

- **Position:**
  - `q`, `dq`: measured, joint space;
  - `target_raw` → `target`: the commanded goal before and after the limit envelope;
  - `q_ref`, `dq_ref`: the law's reference (scaled PD; legacy uses `q_traj`, `dq_traj`);
  - `q_des`: `q_ref` after the envelope clamp;
  - `q_env_lo`, `q_env_hi`: the envelope;
  - `lead` = `q_des − q`;
  - `phase`: trapezoid phase.
- **Host torque terms:**
  - `tau_p` = `kp·lead`;
  - `tau_g`: the model;
  - `tau_f`: friction feed-forward;
  - `tau_d`: legacy damping (0 under scaled PD);
  - `tau_i`: integral;
  - `tau_ff_cmd`: Berthier's τ_ff.
- **On the wire:**
  - `tau_ff_wire`: τ_ff after Davout's cap and rate limiter (NaN before the first send);
  - `kd_mit`: wire kd;
  - `dq_mit`: MIT velocity field.
- **Drive:** `tau_meas`, the drive's own torque estimate.
- **Flags:** `lead_sat`, `planner_event` (e.g. `EnvelopeClamp`), `law` (`legacy` | `scaled_pd`), `time_scale` (the reference governor's `s`).

The torque the drive applies is approximately `kp·(q_des − q) + kd_mit·(dq_mit − dq) + tau_ff_wire`.

## 2. Score

```bash
python3 scripts/analyze-position-trace.py <dir>/position-trace.csv --score-bench \
  --joint <joint> --bench-log <dir>/bench-session.txt
```

The motion suite has already written this to `score.txt`. Read the `=== ADR 0039 bench score` block: per move, speed overshoot, slow-move tracking (`|q − q_ref|` ≤ 0.03 rad), stop error (≤ 0.01 rad), τ_ff step (≤ 0.05 Nm), plus hold drift (≤ 0.005 rad), repeat spread (≤ 0.01 rad) and total-torque clamps (0). Note which criterion fails on which moves. The per-segment warnings printed above that block misfire on every-tick traces; ignore their jerk and slew magnitudes.

## 3. Decompose

Run the bundled script, located next to this file:

```bash
uv run --no-project --with numpy --with matplotlib python3 \
  .agents/skills/trace-forensics/scripts/decompose.py <csv> --joint <joint> [--plot <dir>] [--json]
```

It reports:

- **coverage:** every-tick share and law;
- **tracking:** `q − q_ref` and `q − target`, per phase;
- **drive:** the host's model of the applied torque against `tau_meas`;
- **shaping:** rows where `tau_ff_wire` ≠ `tau_ff_cmd` (Davout reshaped τ_ff), envelope clamps, `lead_sat`, planner events;
- **residual fit:** `torque − tau_g = J·a + Fc·sign(v) + B·v + c` on moving rows, once with `tau_meas` and once with the commanded torque;
- **up/down bins:** per 0.1 rad of q, gravity error = mean of up and down residuals, friction = half their difference.

The bins are the most robust output. The linear fit is crude, so a low R² means the residual has structure it does not model (Stribeck, ripple, delay); look at the plots. `tau_meas` under-reads on pitch: commanded torque against measured acceleration gives an inertia of 0.13–0.14 kg·m², while `tau_meas` gives 0.05–0.06 (`crates/berthier/src/position_hold_tests/bench_replay.rs`). Never treat either torque source alone as ground truth.

Done when every failing criterion from step 2 has numbers from this step attached to the moves it failed on.

## 4. Attribute

Match each failure's shape to its cause:

| Pattern in the numbers | Cause |
|---|---|
| Residual depends on q, same sign in both directions (bin gravity error ≠ 0); HoldTracking trip at home | Gravity model (τ_g) error. Calibrate; never raise gains to compensate |
| Residual follows the sign of v at roughly constant magnitude | Coulomb friction under- or over-compensated (`fc`) |
| Residual peaks at low speed (≈0.6 Nm at 0.03–0.09 rad/s on pitch), breakaway spread, rest off target then a slip | Stribeck / stiction (`fs`), plus pitch's position-periodic ripple (28.6 cycles/rad) |
| Error ∝ acceleration (Accelerate/Decelerate phases) | Missing or wrong J·a feed-forward (inertia) |
| Error grows with speed | Viscous term, kd, or the velocity scale (RS03 MIT velocity is ±20 rad/s) |
| Error confined to rows Davout reshaped | The limiter, not the law: τ_ff rate limit, total-torque clamp, danger zone, envelope |
| Oscillation at a steady frequency | Gain/delay interaction. Compare its frequency with √(kp/J)/2π and the 5 ms tick plus one tick of delay |
| `tau_meas` departs from the host's torque model | The drive is not doing what the host assumes: velocity estimate (one count per 5 ms), quantisation, a drive-side CanTimeout stop, a scale error |

Done when each failure maps to one cause with its number, or to "unexplained" with the evidence that rules the listed causes out.

## 5. Report and route

Report each finding as: criterion → failing moves → the decomposed number → cause → evidence rows or plot. Then:

- model error: `pi_gravity_calibrate` / `pi_joint_calibrate` through `bench-run`;
- the control law itself is suspect: brief the `control-auditor` agent with the subsystem and this trace, then change it through `control-rewrite`, which allows replacing the law outright when the evidence supports it;
- a fix is proposed: `bench-to-test`. The fitted-plant replay reproduces motion-quality failures in `cargo test`.
