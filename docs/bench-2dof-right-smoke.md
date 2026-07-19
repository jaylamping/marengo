# 2-DOF right arm — motion smoke

Light **pitch + roll** motion smoke on `arm_3dof_right`. Run **once** after the
[roll commissioning suite](bench-roll-test-suite.md) passes (`roll_attached`
harness `pass: true`).

## Position-hold baseline — signed off (2026-06-22)

**Operator sign-off:** best motion quality on this bench to date. Do not change
`control.yaml` or Berthier hold-at path without an A/B against this baseline.

Yaw commissioning: [bench-yaw-test-suite.md](bench-yaw-test-suite.md) (`yaw_attached`).
Wave in Consul adds yaw on **raise** only (provisional 0) and keeps roll `nativeWave`
until Teach Record Apply. Do not ship yaw on other presets until Y3–Y4 + candump.

Locked after roll 15° round-trip probe and **`arm_2dof_smoke` harness PASS**
(deploy `ff9d554`). Roll ascent ~1 s, return to **q ≈ 0.0015 rad** at disable.
Same joint tuning on **roll and pitch** in
`config/bringup/arm_3dof_right/control.yaml`:

| Parameter | Value |
|-----------|-------|
| impedance kp / kd / ki | 18 / 3 / 5 |
| position_slew_rad_s | 0.15 |
| position_slew_max_lead_rad | 0.12 |
| position_trajectory_velocity_rad_s | 1.25 |
| position_trajectory_accel_rad_s2 | 4.5 |
| friction fc | 0.08 |
| actuator_group velocity_max_rad_s | 2.5 (both) |
| comm_watchdog_ms | 100 |

**Pre-flight:** re-zero each joint at mechanical arm-down home before enable
(roll limit is q ≥ 0). **Software:** Berthier re-enables on `hold-at` after
comm-watchdog disable; `hold-at 0` no longer clamped by limit envelope.

**Not in scope:** coupled gravity preview, dual `gravity-on`, coordinated sky
poses, full D4–D7 regression (see [bench-test-backlog.md](bench-test-backlog.md)).

## Prerequisites

- Roll suite PASS (R0–R5, R8–R9).
- Config: `config/bringup/arm_3dof_right/`.
- Both joints Verified at arm-down home.

## Standard pre-flight (D0)

Same as roll R0: health, CAN, homing Verified, `fault=0`.

## Tests

| ID | Test | Pass criteria |
|----|------|---------------|
| D0 | Preflight | health, CAN up, both Verified, fault=0 |
| D1 | Pitch hold | roll at 0; pitch hold 0.3 rad and return; no fault |
| D2 | Roll hold | pitch at 0; roll hold 0.785 rad and return; no fault |
| D3 | Cross-talk | while holding one joint, passive joint \|Δq\| < 0.03 rad |

## D1 — Pitch hold (manual script)

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "arm_2dof_smoke",
  "config_dir": "arm_3dof_right",
  "script": [
    "home",
    "enable bench",
    "hold-at right_shoulder_roll 0",
    "sleep 2",
    "hold-at right_shoulder_pitch 0.3",
    "sleep 10",
    "hold-at right_shoulder_pitch 0",
    "sleep 10",
    "status",
    "disable",
    "quit"
  ],
  "timeout_sec": 40
}
```

## D2 — Roll hold

Same pattern with pitch at 0 and roll target 0.785 rad (see harness
`smoke_roll_hold`).

## D3 — Cross-talk

Hold pitch 0.3 with roll nominally at 0, then hold roll 0.785 with pitch at 0.
Analyze position trace: passive joint peak \|Δq\| < 30 mrad.

## Automated smoke

```json
{
  "tool": "pi_bench_harness",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "arm_2dof_smoke",
  "config_dir": "arm_3dof_right",
  "skip_set_zero": true
}
```

**2026-06-22 run (PASS):** `bench-20260622T002026Z.log`,
`position-trace-20260622T002026Z.csv`, `candump-20260622T002026Z.log` on Pi
(`/opt/marengo/var/log/`). Marked done in
[bench-test-backlog.md](bench-test-backlog.md).
