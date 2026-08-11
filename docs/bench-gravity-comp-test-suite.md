# Gravity compensation enhancement — physical bench test suite

Operator-runnable physical bench test suite for the Marengo gravity-compensation
enhancement. Target rig: **single right-shoulder-pitch actuator** (Robstride RS03,
`can0`, `device_id=1`, `direction=-1`, `gear_ratio=1.0`) with a **700 g weighted
arm at ~14 in effective COM** (URDF COM corrected from 18 in).

This document is the authority for the bench phase of the enhancement. Each
protocol is self-contained: an operator who has read [docs/safety.md](safety.md)
and this file can execute it with no further questions. All motion tool calls
require `confirm: true` and `confirm_weighted_motion: true`; the operator must
physically support the arm for the first enable at any elevated pose.

## System under test

| # | Enhancement | Verification hook |
|---|---|---|
| 1 | `PureGravityTorque` newtype — `gravity_torques()` returns gravity only | T1 tau_g vs tau_meas |
| 2 | `ModeIsolation` property tests — modes independent | T2, T5 |
| 3 | Mode-switch kp/kd ramp — 20-tick (~100 ms) smooth transition | T5 kp ramp |
| 4 | Rate-limiter SEED on mode transition — no unclamped torque step | T5 tau_ff_cmd delta |
| 5 | URDF COM correction 18 in → 14 in, tau_g at π/2 ≈ 2.47 Nm (was 3.14) | T1, T3 |
| 6 | Wrong-sign watchdog — config-driven, GravityComp only, <50 ms trip | T6 |
| 7 | Pre-flight tau_g saturation check — blocks enable if max(|tau_g|) > tau_motor_max | T1 pre-flight |
| 8 | FK chain caching — performance optimization, tau_g values unchanged | T1 regression |
| 9 | Friction-FF graded fade — extended fade zone (2× deadband), halved jerk | T4 Layer 2 gate |

## Bench hardware

- Actuator: Robstride RS03, `can0`, `device_id=1`, `direction=-1`, `gear_ratio=1.0`.
- Load: 700 g arm at ~14 in (0.3556 m) effective COM.
- Config dir: `config/bringup/arm_3dof_right/`.
- Control: `kp=8, kd=2, slew=0.15, v=1.45, fc=0.15, tau_ff_rate_limit=60`.
- Bench limits: position −0.9 to 3.17 rad, velocity 2.0 rad/s, torque 5.0 Nm.
- Loop 200 Hz, Chappe state 25 Hz.

## Bench artifacts (Pi: `/opt/marengo/var/log/`)

| Artifact | Symlink | Purpose |
|---|---|---|
| `bench-<TS>.log` | `bench-latest.log` | text log |
| `position-trace-<TS>.csv` | `position-trace-latest.csv` | 200 Hz, 30 cols |
| `candump-<TS>.log` | `candump-latest.log` | CAN wire truth |

Position-trace columns (30): `tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event`.

## Analysis scripts

```bash
python scripts/analyze-position-trace.py /opt/marengo/var/log/position-trace-latest.csv
python scripts/analyze-position-trace.py trace.csv --gate layer2 --tau-ff-rate-limit 60
marengo-log-cli candump summary --file /opt/marengo/var/log/candump-latest.log --timestamp delta --format text
```

## Standard pre-flight (run before every protocol)

```text
pi_health
pi_can_status
pi_homing_status
pi_motor_repl_status
```

All four must return clean (CAN `can0` UP, homing `Verified`, `fault=0x0000`,
deploy rev matches `git rev-parse HEAD`) before any motion. If homing is not
`Verified`, run `pi_set_zero` at mechanical arm-down before continuing.

## Operator safety contract

- E-stop reachable, workspace clear, no bystanders behind the arm arc.
- Support the arm by hand for the first enable at any pose above 0.3 rad.
- Keep hands off during scripted motion; intervene only on fault or unexpected
  motion.
- After any motion tool: run `pi_candump_summary` and `pi_logs_last_fault`
  before declaring the step done. CAN is wire truth vs the code trace.
- Never exceed the 5.0 Nm motor torque cap. The pre-flight tau_g saturation
  check (enhancement #7) blocks enable if `max(|tau_g|) > tau_motor_max`; if it
  refuses enable, do not override, lower the target pose instead.

---

## T1 — Static torque sweep with corrected COM

Verify the URDF COM correction (18 in → 14 in) by comparing modeled `tau_g`
against measured `tau_meas` across the static pose envelope. Also confirms the
pre-flight tau_g saturation check (enhancement #7) and FK-chain caching
regression (enhancement #8, tau_g values unchanged).

### Prerequisites

- Standard pre-flight clean.
- Deploy rev matches `git rev-parse HEAD`.
- Arm at mechanical home (q ≈ 0), homing `Verified`.
- `pi_gravity_preview` available (read-only, no motion).

### Safety preconditions

- Operator confirms E-stop reachable, workspace clear.
- Operator supports arm by hand for the first enable at each elevated pose.
- No pose in the sweep exceeds the 5.0 Nm torque cap (preview confirms).

### Procedure

1. Read-only tau_g preview at six angles across the envelope:

```json
{ "tool": "pi_gravity_preview", "angles": [0, 0.3, 0.785, 1.484, 2.0, -0.3] }
```

   Expected (corrected COM, 700 g, 0.3556 m): tau_g at π/2 (1.484 rad) ≈
   2.47 Nm; at 1.5708 rad ≈ 2.47 Nm. If the preview returns ~3.14 Nm at π/2,
   the URDF COM correction (enhancement #5) is not deployed; stop and re-run
   `pi_sync_main`.

2. Confirm pre-flight saturation check passes: `max(|tau_g|)` over the sweep
   must be < 5.0 Nm. Record the value.

3. For each angle in `[0.0, 0.3, 0.785, 1.484, 2.0]`, run a compliant hold and
   extract `tau_meas` at steady state. Example for 0.785 rad:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 0.785,
  "timeout_sec": 15,
  "return_home_sec": 6,
  "operator": "bench"
}
```

   Between holds, run `pi_hold_off` with `confirm: true` and let the arm
   return to home before the next pose.

4. Pull the latest trace and extract `tau_meas` and `tau_g` at each steady-state
   window (last 500 ms before `return_home_sec`):

```json
{ "tool": "pi_read_file", "path": "/opt/marengo/var/log/position-trace-latest.csv", "tail": 2000 }
```

### Pass/fail criteria

| Gate | Threshold | Notes |
|---|---|---|
| Pre-flight saturation | `max(|tau_g|) < 5.0 Nm` | enhancement #7 |
| COM correction | `tau_g(π/2) ∈ [2.30, 2.65] Nm` | was 3.14 |
| RMS tracking | `sqrt(mean((tau_meas − tau_g)²)) < 0.05 · max(|tau_g|)` | <5% of peak |
| Per-pose | `|tau_meas − tau_g| < 0.15 Nm` at each angle | |
| FK cache regression | tau_g values match a prior known-good sweep ±0.01 Nm | enhancement #8 |

### Telemetry artifacts

- `position-trace-<TS>.csv` — columns `q, tau_g, tau_meas, tau_ff_cmd, phase`.
- `bench-<TS>.log` — pre-flight saturation log line.
- `candump-<TS>.log` — one per elevated hold.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

Record RMS, per-pose residuals, and the saturation-check value in
`docs/bench-weighted-700g-results.md` under a new T1 row.

---

## T2 — Gravity-on vs gravity-off tracking A/B comparison

Prove `ModeIsolation` (enhancement #2): GravityComp and the non-gravity mode
are independent, and gravity comp materially reduces steady-state error.

### Prerequisites

- T1 PASS.
- Arm at home, homing `Verified`, `fault=0x0000`.
- `pi_marengo_pi_script` available for scripted mode transitions.

### Safety preconditions

- Operator supports arm for the gravity-off enable (arm will sag without comp).
- Workspace clear through the full 0 → 0.785 → 0 arc.

### Procedure

1. **Gravity-on leg.** Scripted round trip with GravityComp active:

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "script": [
    "enable",
    "gravity-on",
    "hold-at 0",
    "sleep 3",
    "hold-at 0.785",
    "sleep 8",
    "hold-at 0",
    "sleep 8",
    "disable",
    "quit"
  ],
  "timeout_sec": 30
}
```

2. **Gravity-off leg.** Same trajectory, gravity comp disabled (TorqueOnly with
   `tau_ff=0`, or Position mode with zeroed gravity). Use the script form that
   matches the deployed mode vocabulary; if only `gravity-on`/`gravity-off` are
   available, substitute `gravity-off` for `gravity-on`:

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "script": [
    "enable",
    "gravity-off",
    "hold-at 0",
    "sleep 3",
    "hold-at 0.785",
    "sleep 8",
    "hold-at 0",
    "sleep 8",
    "disable",
    "quit"
  ],
  "timeout_sec": 30
}
```

3. Pull both traces and compute steady-state error at the 0.785 rad dwell (mean
   `q − 0.785` over the last 2 s of the dwell).

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| Gravity-on steady-state error | `|mean(q − 0.785)| < 0.02 rad` |
| Gravity-off steady-state error | `|mean(q − 0.785)| > 0.10 rad` |
| Error ratio | `err_off / err_on > 10` |
| No fault | `pi_logs_last_fault` clean on both legs |

### Telemetry artifacts

- Two `position-trace-<TS>.csv` files (on, off). Columns: `q, target, phase,
  tau_g, tau_meas, tau_ff_cmd`.
- Two `bench-<TS>.log` files.
- Two `candump-<TS>.log` files.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

---

## T3 — Payload robustness (bare motor, 700 g, 900 g)

Confirm the COM-corrected model tracks across three payloads by re-running the
T1 sweep with the URDF mass updated for each. Proves the correction generalizes,
not just fit to 700 g.

### Prerequisites

- T1 PASS.
- Three payload configurations physically staged: bare motor, 700 g arm, 900 g
  arm (700 g rig + one 200 g added mass).
- `pi_sync_bench_config` available to push URDF mass changes per config.

### Safety preconditions

- Operator re-supports the arm after each payload change and before first
  enable.
- For the 900 g leg, confirm `pi_gravity_preview` peak tau_g < 5.0 Nm before
  enable (enhancement #7 pre-flight). At 900 g · 0.3556 m, peak tau_g ≈ 3.14 Nm
  at π/2, well under cap.

### Procedure

For each payload config `P ∈ {bare_motor, weighted_single_arm (700 g),
weighted_single_arm + 200 g (900 g)}`:

1. Update URDF mass for the right shoulder pitch link to match `P` (0.0 kg,
   0.7 kg, 0.9 kg). Keep COM at 0.3556 m.
2. Sync config:

```json
{ "tool": "pi_sync_bench_config", "profile": "arm_3dof_right", "install_to_opt": true }
```

3. Read-only preview to confirm the model reflects the new mass:

```json
{ "tool": "pi_gravity_preview", "angles": [0, 0.3, 0.785, 1.484, 2.0] }
```

4. Run the T1 hold sweep at `[0.3, 0.785, 1.484]` using `pi_hold_on` with
   `profile` set to `P` (use `weighted_single_arm` for both loaded configs;
   `bare_motor` for the unloaded leg). Example for 0.785 rad:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 0.785,
  "timeout_sec": 15,
  "return_home_sec": 6,
  "operator": "bench"
}
```

5. Extract `tau_meas` vs `tau_g` at each dwell.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| Per-config RMS | `< 5% of max(|tau_g|)` for all three configs |
| Per-pose residual | `|tau_meas − tau_g| < 0.20 Nm` (loosened for bare motor noise) |
| Mass scaling | `tau_g(0.785)` scales linearly with mass within ±5% |
| Pre-flight saturation | `max(|tau_g|) < 5.0 Nm` for all three |

### Telemetry artifacts

- Three `position-trace-<TS>.csv` files (one per config).
- Three `bench-<TS>.log` files.
- Three `candump-<TS>.log` files.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

Restore the 700 g URDF mass and re-sync before continuing to T4.

---

## T4 — Friction identification sweep

Identify Coulomb friction `fc` and viscous `fv` from constant-velocity cruise
phases, and verify the friction-FF graded fade (enhancement #9) passes the
Layer 2 smoothness gate.

### Prerequisites

- T1 PASS, 700 g URDF restored.
- Arm at home, `fault=0x0000`.

### Safety preconditions

- Operator supports arm for first enable.
- Full arc clear; the sweep reaches 1.484 rad.

### Procedure

For each speed `v ∈ {0.1, 0.5, 1.0} rad/s` and each direction
`dir ∈ {+ (0 → 1.484), − (1.484 → 0)}`:

1. Set trajectory velocity via a scripted hold-at with the speed baked into the
   script (the deployed `hold-at` honors `position_trajectory_velocity_rad_s`
   from config; for per-run speed changes, edit `control.yaml`
   `position_trajectory_velocity_rad_s` and `pi_sync_bench_config`):

```json
{ "tool": "pi_sync_bench_config", "profile": "arm_3dof_right", "install_to_opt": true }
```

2. Run the move. Positive direction at the configured speed:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 1.484,
  "timeout_sec": 20,
  "return_home_sec": 6,
  "operator": "bench"
}
```

   For the negative direction, start from 1.484 (jog there first) and
   `position_rad: 0`.

3. From the trace, extract the cruise phase (`phase == Cruise`) and compute
   `tau_meas − tau_g` mean. Fit `fc` (sign-dependent mean) and `fv` (slope of
   `tau_meas − tau_g` vs `dq`).

4. Run the Layer 2 gate on each trace:

```bash
python scripts/analyze-position-trace.py /opt/marengo/var/log/position-trace-latest.csv --gate layer2 --tau-ff-rate-limit 60
```

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| Identified `fc` | within 50% of config: `0.075 ≤ fc ≤ 0.225` (config 0.15) |
| `fv` sign | positive (or ~0 within noise) |
| Layer 2 `jerk_rms` | `< 800 rad/s²` |
| Layer 2 `tau_ff` slew | `< 120 Nm/s` (2× rate limit) |
| `tau_f` sign flips | `≤ 2` per segment |
| No fault | `pi_logs_last_fault` clean |

### Telemetry artifacts

- Six `position-trace-<TS>.csv` files (3 speeds × 2 directions). Columns:
  `q, dq, phase, tau_g, tau_meas, tau_f, tau_ff_cmd, friction_mode`.
- Six `bench-<TS>.log` files.
- Six `candump-<TS>.log` files.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

Restore `position_trajectory_velocity_rad_s: 1.45` and re-sync.

---

## T5 — Mode-switch transient test

Verify the mode-switch kp/kd ramp (enhancement #3, 20-tick ~100 ms) and the
rate-limiter SEED on mode transition (enhancement #4, no unclamped torque
step).

### Prerequisites

- T1, T2 PASS.
- `pi_marengo_pi_script` available.
- Arm at home, `fault=0x0000`.

### Safety preconditions

- Operator supports arm at the 0.5 rad hold pose for first enable.
- Workspace clear.

### Procedure

1. Hold at 0.5 rad, then cycle GravityComp → Position → GravityComp with short
   dwells to capture the transition in the trace:

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "script": [
    "enable",
    "gravity-on",
    "hold-at 0.5",
    "sleep 5",
    "mode position",
    "sleep 3",
    "mode gravity",
    "sleep 3",
    "hold-at 0",
    "sleep 5",
    "disable",
    "quit"
  ],
  "timeout_sec": 30
}
```

   If the deployed script vocabulary differs, substitute the equivalent
   mode-switch commands; the goal is two transitions captured in one trace.

2. Pull the trace and isolate the two transition windows (tick where `kp`
   changes). Extract `kp`, `kd`, `tau_ff_cmd` across each transition.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| kp ramp duration | `≥ 10 ticks (50 ms)` and `≤ 30 ticks (150 ms)` |
| kp monotonic | no single-tick kp jump > 1.0 (of the configured 8.0) |
| `tau_ff_cmd` single-tick delta | `< 0.5 Nm` during transition |
| `tau_ff_cmd` slew | `< 60 Nm/s` (rate limit respected, no SEED spike) |
| No fault | `pi_logs_last_fault` clean |

### Telemetry artifacts

- One `position-trace-<TS>.csv`. Columns: `tick, kp, kd, tau_ff_cmd, tau_g,
  tau_meas, phase, planner_event`.
- One `bench-<TS>.log`.
- One `candump-<TS>.log`.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

---

## T6 — Wrong-sign watchdog validation

Verify the config-driven wrong-sign watchdog (enhancement #6) trips in <50 ms on
sustained sign opposition, GravityComp only, and that the grace period
(`grace_period_ticks: 20` = 100 ms) prevents spurious trips on legitimate
transients.

### Prerequisites

- T1, T2, T5 PASS.
- `pi_sync_bench_config` available to push a temporary direction inversion.
- Operator standing by to catch the arm on trip (motor disables).

### Safety preconditions

- Operator hand on arm for the inverted-direction enable. On watchdog trip the
  motor goes limp; the arm will fall.
- E-stop reachable.
- This test deliberately induces a fault; ensure `pi_motor_recover` is the
  immediate next step.

### Procedure

**Part A — trip test.**

1. Edit `config/bringup/arm_3dof_right/motors.yaml`:
   `direction: -1` → `direction: 1` (inverted). Sync:

```json
{ "tool": "pi_sync_bench_config", "profile": "arm_3dof_right", "install_to_opt": true }
```

2. Enable with gravity-on at a pose where tau_g is non-trivial (0.5 rad):

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "script": [
    "enable",
    "gravity-on",
    "hold-at 0.5",
    "sleep 5",
    "disable",
    "quit"
  ],
  "timeout_sec": 15
}
```

3. Immediately check the fault log:

```json
{ "tool": "pi_logs_last_fault" }
```

   Expect `WrongSignWatchdog` trip. Extract the trip timestamp and the
   enable timestamp from `bench-latest.log`; compute `t_trip − t_enable`.

**Part B — grace period test.**

4. Revert `direction` to `-1` and sync. Then run a legitimate transient that
   could momentarily oppose gravity (a fast retarget through 0) and confirm no
   spurious trip:

```json
{ "tool": "pi_sync_bench_config", "profile": "arm_3dof_right", "install_to_opt": true }
```

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "script": [
    "enable",
    "gravity-on",
    "hold-at 0.5",
    "sleep 3",
    "hold-at -0.3",
    "sleep 3",
    "hold-at 0.5",
    "sleep 3",
    "disable",
    "quit"
  ],
  "timeout_sec": 25
}
```

5. Check the fault log; expect no `WrongSignWatchdog` trip.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| Part A trip | `WrongSignWatchdog` fault logged |
| Part A trip latency | `t_trip − t_enable < 50 ms` after `grace_period_ticks` (100 ms) |
| Part A mode scope | fault only fires in GravityComp (not Position) |
| Part B no spurious trip | no `WrongSignWatchdog` on legitimate transient |
| Part B grace honored | transient opposition < 100 ms does not trip |

### Telemetry artifacts

- Two `position-trace-<TS>.csv` (A, B). Columns: `tick, q, dq, tau_g, tau_meas,
  tau_ff_cmd, phase`.
- Two `bench-<TS>.log` (fault line in A).
- `candump-<TS>.log` for both.

### Post-test

```json
{ "tool": "pi_motor_recover", "confirm": true }
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
```

Confirm `direction: -1` is restored in the repo and on Pi before continuing.

---

## T7 — Negative-retarget descent gate

After the COM correction, verify negative-direction retargets no longer
free-accelerate past the velocity guard (the prior 2.56 rad/s trip from
`bench-weighted-700g-results.md` Phase 6).

### Prerequisites

- T1 PASS, COM correction deployed (tau_g at π/2 ≈ 2.47 Nm).
- `direction: -1` restored (T6 post-test).
- Arm at home, `fault=0x0000`.

### Safety preconditions

- Operator supports arm for the first negative retarget.
- Workspace clear through the negative arc.
- E-stop reachable; a trip here is expected to be a fail, not a hazard, but the
  arm will sag on disable.

### Procedure

1. Gentle negative hold-at to gate the descent:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": -0.3,
  "timeout_sec": 15,
  "return_home_sec": 6,
  "operator": "bench"
}
```

   Pull the trace; verify position-derived `dq` peak < 1.5 rad/s. If PASS,
   continue. If the velocity guard trips, stop and record; do not run the
   ladder.

2. Ladder of negative targets, each as a separate `pi_hold_on` from home:

```text
-0.3  →  -0.5  →  -0.7  →  -0.85
```

   Example for −0.5:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": -0.5,
  "timeout_sec": 15,
  "return_home_sec": 6,
  "operator": "bench"
}
```

   Run `pi_hold_off` between each rung.

3. From each trace, extract peak `|dq|` (position-derived, column `dq`) during
   the descent.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| Gentle gate (−0.3) | peak `|dq| < 1.5 rad/s` |
| Ladder rungs | peak `|dq| < 2.0 rad/s` for all of −0.3, −0.5, −0.7, −0.85 |
| No velocity trip | `pi_logs_last_fault` clean on every rung |
| Limit envelope | `q_env_lo` clamps before −0.9 (soft limit) |

### Telemetry artifacts

- Five `position-trace-<TS>.csv`. Columns: `q, dq, q_env_lo, q_env_hi, target,
  target_raw, lead_sat, phase`.
- Five `bench-<TS>.log`.
- Five `candump-<TS>.log`.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

---

## T8 — Recovery protocol

Verify the `pi_motor_recover` path restores the actuator from a deliberate
fault to a clean re-hold within the time budget.

### Prerequisites

- T1, T6, T7 PASS.
- `pi_motor_recover` available.
- Arm at home, `fault=0x0000`.

### Safety preconditions

- Operator supports arm; the fault is induced by pushing the arm past the
  velocity limit by hand.
- E-stop reachable.
- Operator ready to catch on disable.

### Procedure

1. Hold at 0.5 rad:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 0.5,
  "timeout_sec": 10,
  "return_home_sec": 6,
  "operator": "bench"
}
```

2. With the motor enabled and holding, the operator pushes the arm rapidly
   (by hand) past the 2.0 rad/s velocity guard. Davout should disable.

3. Recover:

```json
{ "tool": "pi_motor_recover", "confirm": true, "profile": "weighted_single_arm" }
```

   Expect `RECOVER_OK`. Record `t_recover_start` (fault) and `t_recover_ok`.

4. Re-hold at 0:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 0,
  "timeout_sec": 10,
  "return_home_sec": 6,
  "operator": "bench"
}
```

   Record `t_hold_ok` and the settled `q`.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| Fault induced | velocity guard trips, `fault != 0x0000` |
| Recover result | `RECOVER_OK` |
| Recover time | `t_recover_ok − t_recover_start < 10 s` |
| Re-hold settle | `|q − 0| < 0.006 rad (6 mrad)` after re-hold |
| Final fault | `pi_logs_last_fault` clean after re-hold |

### Telemetry artifacts

- `bench-<TS>.log` spanning fault → recover → re-hold.
- `position-trace-<TS>.csv` for the re-hold. Columns: `q, dq, fault, phase`.
- `candump-<TS>.log`.

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
{ "tool": "pi_hold_off", "confirm": true }
```

---

## T9 — Disable-drop behavior

Verify a clean disable mid-hold produces a smooth sag to home, no torque spike,
and CAN feedback freezes at the last frame (expected robstride disabled
behavior).

### Prerequisites

- T1, T8 PASS.
- Arm at home, `fault=0x0000`.

### Safety preconditions

- Operator supports arm; on disable the arm sags under gravity.
- Workspace clear below the arm arc.
- E-stop reachable.

### Procedure

1. Hold at 0.5 rad:

```json
{
  "tool": "pi_hold_on",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "weighted_single_arm",
  "joint": "right_shoulder_pitch",
  "position_rad": 0.5,
  "timeout_sec": 10,
  "return_home_sec": 6,
  "operator": "bench"
}
```

2. Once settled (wait ~3 s), disable:

```json
{ "tool": "pi_motor_disable", "confirm": true, "profile": "weighted_single_arm" }
```

3. Observe the arm sag. Capture the candump during and after disable:

```json
{ "tool": "pi_candump_once" }
```

4. Pull the trace and the candump log. Inspect the last 200 ms before disable
   and 500 ms after for any `tau_meas` spike.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| No torque spike | `|tau_meas|` in last 200 ms before disable < 0.5 Nm over steady-state |
| No fault | `pi_logs_last_fault` clean (disable is not a fault) |
| CAN freeze | candump shows feedback frames stop within 50 ms of disable (stale last frame) |
| Smooth sag | arm reaches home without bounce or stall; operator confirms |

### Telemetry artifacts

- `position-trace-<TS>.csv`. Columns: `q, dq, tau_meas, tau_ff_cmd, phase` at
  the disable boundary.
- `bench-<TS>.log`.
- `candump-<TS>.log` (pre- and post-disable).

### Post-test

```json
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
```

---

## T10 — Full suite re-run and verdict

Re-execute T1 through T9 in sequence on the final deployed build. If all gates
pass, update `docs/bench-weighted-700g-results.md` verdict from
"PASS WITH WARNINGS" to "PASS" and record the result in the results doc.

### Prerequisites

- T1 through T9 each PASS at least once individually.
- Final build deployed: `pi_sync_main` run, `pi_health` shows deploy rev matches
  `git rev-parse HEAD`.
- All three warnings from the prior verdict addressed:
  1. Gravity over-compensation → URDF COM corrected to 14 in (T1 confirms).
  2. Layer 2 smoothness → friction-FF graded fade (T4 confirms).
  3. Negative-retarget velocity trip → COM correction + descent gate (T7
     confirms).

### Safety preconditions

- Full bench safety contract in force for every protocol.
- Operator fresh; this is a long sequence. Take breaks between protocols.
- E-stop reachable throughout.

### Procedure

1. Run T1, T2, T3, T4, T5, T6, T7, T8, T9 in order, each per its own
   procedure. Do not skip the standard pre-flight or the post-test fault check
   on any protocol.

2. After T9, run a final health sweep:

```json
{ "tool": "pi_health" }
{ "tool": "pi_can_status" }
{ "tool": "pi_motor_repl_status" }
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
```

3. Tally the gates. If every gate across all nine protocols is PASS, update
   `docs/bench-weighted-700g-results.md`:

   - Change the `## Verdict` heading line from `PASS WITH WARNINGS` to `PASS`.
   - Add a dated subsection noting the enhancement build (git SHA), the nine
     protocol results, and the artifact timestamps.
   - Strike through the three prior warnings with resolution references.

4. Record the verdict in `docs/bench-weighted-700g-results.md` (dated subsection with git SHA and T1–T9 outcomes). No separate memory store.

### Pass/fail criteria

| Gate | Threshold |
|---|---|
| T1–T9 | every protocol's own gates PASS |
| Final health | `pi_health` clean, deploy rev matches, `fault=0x0000` |
| Verdict update | `bench-weighted-700g-results.md` verdict line reads `PASS` |

### Telemetry artifacts

- All T1–T9 artifacts (traces, bench logs, candumps) retained.
- Final `pi_health`, `pi_can_status`, `pi_logs_last_fault`, `pi_candump_summary`
  outputs appended to the results doc.

### Post-test

```json
{ "tool": "pi_hold_off", "confirm": true }
{ "tool": "pi_motor_disable", "confirm": true, "profile": "weighted_single_arm" }
{ "tool": "pi_logs_last_fault" }
{ "tool": "pi_candump_summary" }
```

Confirm the bench is left in a safe state: motor disabled, arm at home, no
fault, CAN quiet.

---

## Appendix — quick reference

### MCP tool parameter cheat sheet

| Tool | Required params | Notes |
|---|---|---|
| `pi_gravity_preview` | (optional) `angles: number[]` | read-only |
| `pi_hold_on` | `confirm, confirm_weighted_motion, profile` | motion |
| `pi_hold_off` | `confirm` | motion stop |
| `pi_bench_harness` | `confirm, confirm_weighted_motion, profile` | automated |
| `pi_marengo_pi_script` | `confirm, confirm_weighted_motion, script` | motion |
| `pi_motor_enable` | `confirm` (motion) | short probe |
| `pi_motor_disable` | `confirm` | fault clear |
| `pi_motor_recover` | `confirm` | fault recovery |
| `pi_motor_repl_status` | — | read-only |
| `pi_homing_status` | — | read-only |
| `pi_set_zero` | `confirm` | calibration |
| `pi_jog` | `confirm, joint, position_rad` | motion |
| `pi_health` | — | read-only |
| `pi_can_status` / `pi_can_up` | — | CAN |
| `pi_candump_once` / `pi_candump_summary` | — | CAN wire truth |
| `pi_logs_tail` / `pi_logs_grep` / `pi_logs_last_fault` / `pi_logs_list` / `pi_logs_archive_list` | (varies) | logs |
| `pi_read_file` | `path` | Pi files |
| `pi_journal` | (optional) `lines` | systemd |
| `pi_sync_main` / `pi_sync_bench_config` | (varies) | deploy |
| `pi_gateway_health` | — | gateway |

### Position-trace column index

| idx | col | idx | col |
|---|---|---|---|
| 0 | tick | 15 | phase |
| 1 | t_ms | 16 | friction_mode |
| 2 | joint | 17 | tau_p |
| 3 | q | 18 | tau_g |
| 4 | dq | 19 | tau_f |
| 5 | q_traj | 20 | tau_d |
| 6 | dq_traj | 21 | tau_ff_cmd |
| 7 | q_des | 22 | tau_meas |
| 8 | target | 23 | dq_mit |
| 9 | target_raw | 24 | kp |
| 10 | q_env_lo | 25 | kd |
| 11 | q_env_hi | 26 | joint_stuck |
| 12 | lead | 27 | planner_frozen |
| 13 | lead_sat | 28 | retarget_age_ms |
| 14 | settle_error | 29 | planner_event |

### Enhancement → protocol map

| Enhancement | Primary protocol |
|---|---|
| 1 PureGravityTorque | T1 |
| 2 ModeIsolation | T2, T5 |
| 3 Mode-switch kp/kd ramp | T5 |
| 4 Rate-limiter SEED | T5 |
| 5 URDF COM correction | T1, T3, T7 |
| 6 Wrong-sign watchdog | T6 |
| 7 Pre-flight tau_g saturation | T1, T3 |
| 8 FK chain caching | T1 |
| 9 Friction-FF graded fade | T4 |
