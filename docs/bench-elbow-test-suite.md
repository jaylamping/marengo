# Elbow pitch commissioning — physical bench test suite

Operator-runnable physical bench tests for **right elbow pitch** on the attached
4-DOF arm (`arm_4dof_right`). Elbow is Robstride RS02 on `can0`, `device_id=4`,
provisional `direction=1`, discovery limits **0…1.2 rad** (widen after E5).

**Not in scope:** CAD-accurate τ_g magnitude, golden gravity vectors, elbow as
`nativeWave` oscillator. See [bench-yaw-test-suite.md](bench-yaw-test-suite.md)
and [bench-test-backlog.md](bench-test-backlog.md).

## System under test

| Item | Value |
|------|-------|
| Config dir | `config/bringup/arm_4dof_right/` |
| URDF | `assets/urdf/arm_4dof_right.urdf` (provisional masses) |
| Elbow joint | `right_elbow_pitch`, software discovery 0…1.2 rad |
| Proximal joints during E1–E3 | held near 0 unless noted |
| Loop | 200 Hz control, 25 Hz Chappe state |

## Mechanical elbow zero (E2 reference)

Straight arm (forearm aligned with upper arm) = **0 rad** per kinematics.
Mark the fixture so re-zero after E1 is repeatable.

## Bench artifacts (Pi: `/opt/marengo/var/log/`)

| Artifact | Symlink | Purpose |
|----------|---------|---------|
| `bench-<TS>.log` | `bench-latest.log` | text log |
| `position-trace-<TS>.csv` | `position-trace-latest.csv` | 200 Hz joint trace |
| `candump-<TS>.log` | `candump-latest.log` | CAN wire truth |

## Standard pre-flight (E0)

```text
pi_health
pi_can_status
pi_homing_status
pi_motor_repl_status
```

Pass: `can0` UP, **all four joints** homing **Verified**, `fault=0x0000`.
Runtime config: `MARENGO_CONFIG_DIR=.../arm_4dof_right` in `/etc/marengo/env`.

## Operator safety contract

- E-stop reachable; workspace clear.
- Support the arm for first enable after attach or re–set-zero.
- **Upright hazard:** elevated pitch + flexed elbow — GravityComp sign must PASS
  (E6) before unsupported elevated holds. Wave raise stays arm-supported until
  Wave-pose G-comp gate is signed.
- After motion: `pi_candump_summary` and `pi_logs_last_fault`.
- Motion tools require `confirm: true` and `confirm_weighted_motion: true`.

---

## E1 — Identity / CAN presence

**Pass:** candump or motor status shows device id **4** on `can0`; joint name
`right_elbow_pitch` in homing list.

**2026-07-19:** Software profile live (`arm_4dof_right`, four joints). `pi_set_zero`
on `right_elbow_pitch` enabled Davout for id 4 but **no feedback** after SetZero;
candump top IDs showed `028001FD`/`028002FD`/`028003FD` only (no id-4 MIT stream).
Blocked on firmware ID assignment / power / CAN wiring for the new RS02.

## E2 — Set-zero at straight arm

**Pass:** `pi_set_zero` includes `right_elbow_pitch`; homing **Verified**.

## E3 — Sign probe (flexion)

**Pass:** small positive command (~0.2–0.3 rad) flexes elbow in the expected
URDF +Y sense; return to 0; no fault. Flip `direction` in motors.yaml if wrong.

## E4 — Hold ladder (near home)

**Pass:** hold at 0.3 rad then 0; stable; fault clear. Stay inside measured soft
limits. Document measured envelopes in this file after probing.

## E5 — Soft-limit discovery (supported)

**Pass:** incremental supported probe; update `motors.yaml` bench
`position_*_rad` from measured safe envelopes. Never command to unverified 2.5.

## E6 — GravityComp sign (modest → Wave pose)

**Pass (sign only):** GravityComp at modest pitch + elbow flexed, then at
Wave-pose envelope (pitch ~3.0 + elbow ~1.0) with arm **supported**. No runaway.
Record “sign PASS / magnitude provisional / Wave-pose hold supported.”
Wrong-sign watchdog is **disabled** on this profile during bring-up (see
`control.yaml`); do not claim watchdog coverage.

## E7 — Automated smoke (`elbow_attached`)

```json
{
  "tool": "pi_bench_harness",
  "confirm": true,
  "confirm_weighted_motion": true,
  "profile": "elbow_attached",
  "config_dir": "arm_4dof_right",
  "skip_set_zero": true
}
```

**Pass:** `pass_kind: "smoke"`; `operator_signoff_required: true`. Not
commissioning complete. Elevated steps only after E6.

## Gates

| Gate | Unlocks |
|------|---------|
| E0–E4 PASS | Teach-record / Wave raise with elbow (arm supported) |
| E6 PASS + candump clean | Unsupported Wave raise consideration |
| E7 smoke + operator review | Shipping elbow on non-Wave presets later |

## Measured limits (fill after E5)

| Bound | Value (rad) | Notes |
|-------|-------------|-------|
| Soft lower | | |
| Soft upper | | |
