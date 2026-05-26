# ADR 0006: Homing, zero, and joint reference strategy

**Status:** Accepted  
**Date:** 2026-05-25

## Context

Bench bring-up relied on manually placing the arm and running `SetZero`. Marengo’s `home` command only set supervisor `Ready` without verifying encoders, sensors, or calibration. Shoulder roll housing design adds three Hall sensors per joint (home, min limit, max limit).

## Decision

### Separate concepts

1. **Home reference** — physical feature found at startup (Hall edge, limit, hardstop).
2. **Semantic zero** — URDF/control zero (`q = 0`).
3. **Home offset** — `home_offset_rad` in `config/homing.yaml`.
4. **Verified state** — per-joint software gate before `Ready` / `Active`.

### Zero authority

- **Firmware `SetZero`** remains the encoder reference for Robstride drives.
- **Host calibration record** (`var/calibration/zero_registry.yaml`) is the audit trail and stale-zero gate.
- **`home_offset_rad`** maps Hall-detected home to semantic zero when using `hall_three_sensor`.
- **`position_hold_trim_rad`** in `control.yaml` is a hold-target fudge only; not a substitute for homing.

### Startup sequence

```text
Precheck (CAN, faults, E-stop)
  → Sensor health (when configured)
  → Homing (method per joint)
  → Apply home_offset_rad
  → Verify plausibility
  → Ready
  → Active (operator enable)
```

### Interim vs target

| Phase | Method | Enable gate |
|-------|--------|-------------|
| Now | `manual_reference` | `\|q\| < tolerance` after `set-zero` + calibration record |
| Next | `hall_three_sensor` | Sensor health + search + offset + verification |

### Sensor policy

- Startup sensor health check before homing when Hall inputs configured.
- Truth table for 3-sensor layout documented in [homing.md](../homing.md).
- Impossible combinations fault; do not enable.

### API changes

- `motor-repl home` runs verification; does not blindly set Ready.
- `motor-repl enable` requires all joints Verified.
- `set-zero` writes calibration record and marks joint Verified on success.
- Chappe `robot/enable` requires prior verified homing (same as bench).

## Consequences

- New crate `marengo-homing`: state machine, sensor truth table, calibration I/O.
- New config file `config/homing.yaml` loaded by `marengo-config`.
- Davout `Supervisor` consults homing registry before `request_enable`.
- MCP tools gain `pi_home_verify`, sensor readback when GPIO wired.

## References

- [ODRI robot_fingers homing](https://open-dynamic-robot-initiative.github.io/robot_fingers/doc/homing.html)
- [Kollmorgen AKD homing](https://webhelp.kollmorgen.com/AKD/English/Content/UsersManual/Homing.htm)
- [homing.md](../homing.md)
