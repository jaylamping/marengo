# Yaw commissioning — physical bench test suite

Operator-runnable physical bench tests for **right upper-arm yaw** on the attached
3-DOF arm (`arm_3dof_right`). Yaw is Robstride RS02 on `can0`, `device_id=3`,
provisional `direction=1`, limits **±1.57 rad**. Pitch (id 2) and roll (id 1)
are enabled; during yaw probes they are held at **q=0** unless noted (Y4).

**Not in scope:** URDF mass/COM tuning, gravity-comp accuracy, Wave teach overlays.
See [bench-roll-test-suite.md](bench-roll-test-suite.md) and
[bench-test-backlog.md](bench-test-backlog.md).

## System under test

| Item | Value |
|------|-------|
| Config dir | `config/bringup/arm_3dof_right/` |
| URDF | `assets/urdf/arm_3dof_right.urdf` |
| Yaw joint | `right_upper_arm_yaw`, limits −1.57…1.57 rad |
| Pitch / roll during Y1–Y3 | held at 0 rad |
| Loop | 200 Hz control, 25 Hz Chappe state |

## Mechanical yaw zero (Y2 reference)

Place the forearm / upper-arm twist at the **neutral** CAD/home pose used for
set-zero of roll and pitch (arm down, twist neither inward nor outward). Mark
the fixture if needed so re-zero after Y1 is repeatable. Do **not** set-zero at
an approximate post-probe pose.

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

## Standard pre-flight (Y0)

```text
pi_health
pi_can_status
pi_homing_status
pi_motor_repl_status
```

Pass: `can0` UP, **roll + pitch + yaw** homing **Verified**, `fault=0x0000`,
deploy rev matches `git rev-parse HEAD`. If yaw is Unhomed, complete Y2 first.

**Runtime config:** `MARENGO_CONFIG_DIR=.../arm_3dof_right` in `/etc/marengo/env`.

## Operator safety contract

- E-stop reachable; workspace clear; no bystanders in the yaw/forearm arc.
- Arm at home (pitch≈0, roll≈0, yaw≈0) before enable.
- Support the arm for the **first** enable after attach or re–set-zero.
- After motion: `pi_candump_summary` and `pi_logs_last_fault`.
- All motion tools require `confirm: true` and `confirm_weighted_motion: true`.

### Y4 extra (elevated pitch)

- Support the arm (or confirm stable gravity/impedance hold) before elevating pitch.
- Abort if pitch drifts >50 mrad from the hold target or any fault appears.
- Prefer `hold-at right_shoulder_pitch 0.3` with hands ready; do not free-fall.

---

## Y1 — Sign probe (both directions)

**Pass:** +0.15 rad yaw moves in the expected URDF +Z sense; −0.15 rad opposite.
No fault. Return to 0.

```json
{
  "tool": "pi_marengo_pi_script",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "yaw_attached",
  "config_dir": "arm_3dof_right",
  "script": [
    "home",
    "enable bench",
    "hold-at right_shoulder_pitch 0",
    "hold-at right_shoulder_roll 0",
    "sleep 2",
    "hold-at right_upper_arm_yaw 0.15",
    "sleep 8",
    "hold-at right_upper_arm_yaw 0",
    "sleep 6",
    "hold-at right_upper_arm_yaw -0.15",
    "sleep 8",
    "hold-at right_upper_arm_yaw 0",
    "sleep 8",
    "status",
    "disable",
    "quit"
  ],
  "timeout_sec": 55
}
```

If sign is wrong, flip `direction` for `right_upper_arm_yaw` in
`config/bringup/arm_3dof_right/motors.yaml`, sync config, re-run Y1 (do not
change roll/pitch directions).

---

## Y2 — Re-zero + homing

At the mechanical yaw reference (see above):

```json
{
  "tool": "pi_set_zero",
  "confirm": true,
  "config_dir": "arm_3dof_right",
  "joint": "right_upper_arm_yaw"
}
```

Then `pi_homing_status` — yaw **Verified** (roll/pitch remain Verified).

If Consul has a taught Wave overlay, also click **I set-zero'd** on the Teach Record
tab after `pi_set_zero` — MCP set-zero does not bump Consul's calibration epoch by
itself; without that mark, Wave may replay pre-zero landmarks.

---

## Y3 — Hold ladder (both signs)

Pitch and roll at 0. Yaw holds: 0 → 0.3 → 0.6 → 0 → −0.3 → −0.6 → 0.

**Pass:** each target ±50 mrad, no fault, stay inside ±1.57.

Use `pi_bench_harness` profile `yaw_attached` (automated `yaw_hold_ladder`) or
scripted hold-at with the same angles.

---

## Y4 — Cross-talk (pitch elevated)

Hold pitch at **0.3 rad**, roll at 0; yaw ladder 0 → ±0.3 → 0.

**Pass:** yaw reaches targets; pitch stays within ±50 mrad of 0.3; no fault.
Then `pi_candump_summary` + `pi_logs_last_fault`.

---

## Automated suite (smoke — not ±50 mrad gate)

`pi_bench_harness` profile `yaw_attached` runs the Y1/Y3/Y4 *scripts* and fails on
fault / watchdog / nonzero exit (same heuristics as roll). It does **not** assert
±50 mrad hold error or Y4 pitch drift from position-trace.

**Operator sign-off for Y3–Y4** still requires reviewing
`position-trace-latest.csv` (+ candump) against the Pass criteria above. Treat a
green harness alone as **smoke**, not commissioning complete.

Harness JSON includes:

| Field | Meaning for `yaw_attached` |
|-------|----------------------------|
| `pass` | Smoke/heuristic success (steps ok, no faults) |
| `pass_kind` | `"smoke"` — never `"commissioning"` today |
| `commissioning_criteria_met` | `null` (no ±50 mrad gate in harness) |
| `operator_signoff_required` | `true` — do not unlock Wave/teach from harness alone |

```json
{
  "tool": "pi_bench_harness",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "yaw_attached",
  "config_dir": "arm_3dof_right",
  "skip_set_zero": true
}
```

Use `skip_set_zero: true` only when Y2 already left all three joints Verified.

### Soft-invalidate after set-zero

`pi_set_zero` (and motor-repl set-zero) does **not** bump Consul’s teach
calibration epoch. After Y2 (or any teach-joint re-zero), open Teach Record and
click **I set-zero'd** so taught Wave overlays soft-invalidate. Home alone does
not bump.

## Sign-off

| Gate | Required before |
|------|-----------------|
| Y0–Y2 PASS | Teach-record / Wave raise with yaw |
| Y3–Y4 PASS + candump clean | Shipping yaw on non-Wave presets or taught Wave as default |
