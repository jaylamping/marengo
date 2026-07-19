# Roll commissioning — physical bench test suite

Operator-runnable physical bench tests for **right shoulder roll** on the attached
3-DOF arm (`arm_3dof_right`). Roll is Robstride RS03 on `can0`, `device_id=1`,
`direction=1`, limits **0 → π** (arm down → sky). Pitch (`device_id=2`,
`direction=-1`) and upper-arm yaw (`device_id=3`, RS02) are enabled; pitch is held at **q=0** during roll motion so the
multi-motor enable path matches production.

**Not in scope:** URDF mass/COM tuning, gravity-comp accuracy, payload grids.
See [bench-test-backlog.md](bench-test-backlog.md) for deferred gravity tests.

## System under test

| Item | Value |
|------|-------|
| Config dir | `config/bringup/arm_3dof_right/` |
| URDF | `assets/urdf/arm_3dof_right.urdf` (0.7 kg stub — intentional) |
| Roll joint | `right_shoulder_roll`, limits 0–3.14159 rad |
| Pitch during roll tests | held at 0 rad (same marengo-pi session) |
| Loop | 200 Hz control, 25 Hz Chappe state |

## Bench artifacts (Pi: `/opt/marengo/var/log/`)

| Artifact | Symlink | Purpose |
|----------|---------|---------|
| `bench-<TS>.log` | `bench-latest.log` | text log |
| `position-trace-<TS>.csv` | `position-trace-latest.csv` | 200 Hz joint trace |
| `candump-<TS>.log` | `candump-latest.log` | CAN wire truth |

```bash
python scripts/analyze-position-trace.py /opt/marengo/var/log/position-trace-latest.csv
python scripts/analyze-candump-log.py /opt/marengo/var/log/candump-latest.log
```

## Standard pre-flight (R0)

Run before every protocol:

```text
pi_health
pi_can_status
pi_homing_status
pi_motor_repl_status
```

Pass: CAN `can0` UP, both joints homing **Verified**, `fault=0x0000`, deploy rev
matches local `git rev-parse HEAD`. If not Verified, re–set-zero at mechanical
arm-down (`pi_set_zero` with `config_dir: arm_3dof_right`).

**Runtime config:** set `MARENGO_CONFIG_DIR` to `arm_3dof_right` in
`/etc/marengo/env`, or pass `config_dir` on every MCP motion call until updated.

## Operator safety contract

- E-stop reachable; workspace clear; no bystanders in the roll arc.
- Arm mechanically at home (roll q≈0, pitch q≈0) before enable.
- Support the arm for the **first** enable after attach or re–set-zero.
- After motion: `pi_candump_summary` and `pi_logs_last_fault`. CAN is wire truth.
- All motion tools require `confirm: true` and `confirm_weighted_motion: true`.

---

## R1 — Sign probe

**Pass:** +0.15 rad hold on roll rotates **away from the body** (positive roll from
home).

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "roll_attached",
  "config_dir": "arm_3dof_right",
  "script": [
    "home",
    "enable bench",
    "hold-at right_shoulder_pitch 0",
    "sleep 2",
    "hold-at right_shoulder_roll 0.15",
    "sleep 8",
    "hold-at right_shoulder_roll 0",
    "sleep 8",
    "status",
    "disable",
    "quit"
  ],
  "timeout_sec": 40
}
```

If motion is **inward**, stop and fix `direction` in `motors.yaml` (roll must be
`1` while pitch is `-1`).

---

## R2 — Re-zero + homing

At arm-down home, both joints Verified:

```json
{
  "tool": "pi_set_zero",
  "confirm": true,
  "config_dir": "arm_3dof_right",
  "joint": "right_shoulder_roll"
}
```

Repeat for `right_shoulder_pitch`. Then `pi_homing_status` — both **Verified**.

---

## R3 — Hold sweep

Pitch at 0; roll holds at 0.15, 0.785, 1.57 rad. **Pass:** each target reached
±50 mrad, no fault, no sustained `outside [0, 3.14159]`.

Use `pi_bench_harness` profile `roll_attached` (automated `roll_hold_sweep` step)
or the same hold-at pattern as R1 with intermediate angles.

---

## R4 — Round trip

Roll 0 → 1.57 → 0 with pitch at 0. **Pass:** completes without fault; candump
~200 Hz during move; trace shows no sustained limit violations.

Full 0→π traverse is a stretch goal, not a hard gate.

---

## R5 — Limits

At home: no `outside [0, 3.14159]` on roll. After hold at 1.57 rad, clean return
to 0 without envelope fault.

---

## R8 — Disable / recover

After scripted motion, `pi_motor_disable` or harness `final_disable`. If
`fault≠0`, run `pi_motor_recover` before re-test.

---

## R9 — Telemetry

During R4: candump and position trace archived under `var/log/`. No sustained
watchdog or `outside` lines in trace for roll joint.

---

## Automated suite

Single invocation (all R1–R5 motion steps + disable):

```json
{
  "tool": "pi_bench_harness",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "roll_attached",
  "config_dir": "arm_3dof_right",
  "skip_set_zero": true
}
```

**Commissioning complete** when JSON `pass: true` and operator confirms R1 direction
visually on the first run after attach.

## Deferred (backlog)

| ID | Test | Reason |
|----|------|--------|
| R6 | Gravity preview accuracy | Arm mass/CAD changing |
| R7 | Gravity-on roll | Same |
| — | Full 0→π sky gate | Optional stretch |
| — | τ_g vs τ_meas | Gravity-comp program |
