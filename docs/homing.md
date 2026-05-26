# Homing and zero reference

Marengo separates **home reference**, **semantic zero**, and **verified startup state**. Read this with [safety.md](safety.md) and [pi-commissioning.md](pi-commissioning.md).

## Terminology

| Term | Meaning |
|------|---------|
| **Home reference** | A repeatable physical feature (Hall sensor, limit switch, hardstop, encoder index) found at startup. |
| **Semantic zero** | The joint angle used by URDF, gravity, and control (`q = 0`). |
| **Home offset** | `home_offset_rad`: maps detected home reference to semantic zero. `semantic_zero = home_reference + home_offset_rad`. |
| **Firmware zero** | Robstride `SetZero` — encoder count stored in the motor drive. |
| **Verified** | Software has confirmed zero validity for a joint and allows normal enable. |
| **Stale zero** | Calibration record or firmware zero is no longer trusted (motor swap, ID change, disassembly, failed verification). |

## Startup states

Per joint:

| State | Meaning |
|-------|---------|
| `Unhomed` | Zero validity unknown; normal motion blocked. |
| `Homing` | Constrained search or calibration in progress. |
| `Verified` | Zero/reference accepted; joint may enable with the rest of the arm. |
| `Faulted` | Sensor, timeout, or plausibility failure; requires operator recovery. |

Per sensor input (`home`, `min_limit`, `max_limit`):

| State | Meaning |
|-------|---------|
| `Unknown` | Not yet checked this boot. |
| `Healthy` | Electrical read OK; expected transitions observed when required. |
| `Faulted` | Stuck, missing, impossible combination, or polarity mismatch. |

Supervisor operational mode (unchanged):

```text
Disabled → Ready → Active
```

`Ready` requires **all configured joints Verified** and no latched homing/sensor faults.

## Sensor truth table (3-Hall layout)

One magnet on the rotating member; three fixed Hall sensors on the housing.

| home | min | max | Interpretation |
|------|-----|-----|----------------|
| 1 | 0 | 0 | Valid home reference detected. |
| 0 | 1 | 0 | At negative travel edge; recovery/homing only. |
| 0 | 0 | 1 | At positive travel edge; recovery/homing only. |
| 0 | 0 | 0 | Mid-travel; valid but not homed. |
| \>1 active | — | — | Fault unless `allow_sensor_overlap: true` in config. |
| Expected edge never seen during search | — | — | Homing fault (timeout). |
| Impossible combo at boot | — | — | Wiring/polarity/stuck-sensor fault. |

Hall sensors are **references**, not structural stops. Software limits in Davout remain authoritative after homing.

## Homing methods

Configured per joint in `config/homing.yaml` (see [ADR 0006](decisions/0006-homing-zero-reference.md)).

| Method | When to use |
|--------|-------------|
| `manual_reference` | **Interim bench** — operator places arm at mechanical reference, runs `set-zero`, software verifies `\|q\| < tolerance`. |
| `hall_three_sensor` | **Target** — slow search, edge detect, backoff/re-approach, apply `home_offset_rad`, optional firmware `SetZero`. |
| `none` | Simulation or joints without homing (not for live bench). |

## Interim manual procedure (until Hall hardware)

Use until shoulder-roll Hall mounts are installed:

1. E-stop reachable; arm supported.
2. `motor-repl status` — CAN OK, no faults.
3. Sign test per joint if not yet recorded.
4. Place joint at mechanical reference (arm down for shoulder pitch).
5. `motor-repl set-zero <joint>` — verifies `\|q\| < zero_verify_tolerance_rad`, writes calibration record.
6. `motor-repl home` — marks supervisor Ready when all joints Verified.
7. `marengo-pi` → `enable` → `gravity-on` / hold tests.

Do **not** re-zero during hold scripts unless intentionally recalibrating.

## Out-of-range recovery

If feedback is outside effective limits or zero is stale:

1. `disable` — drives off.
2. Manually move to a known safe pose **or** run constrained homing when Hall sensors exist.
3. Re-run sign test if direction may have changed.
4. Re-zero or re-home before enable.

Blind position hunting without sensors or operator reference is **not** allowed.

## Calibration record

Host-side registry: `var/calibration/zero_registry.yaml` by default, or an absolute bench path such as `/opt/marengo/var/calibration/zero_registry.yaml` for live Pi profiles. The path is configurable via `homing.yaml`; override at runtime with `MARENGO_CALIBRATION_RECORD`.

Records per joint: device ID, method, offset, timestamp, config revision, verification result, sign-test status. Firmware `SetZero` alone is not an audit trail.

## Stale-zero triggers

Re-calibrate when:

- Motor firmware ID changed
- Motor or gearbox disassembled
- Hall magnet or sensor replaced
- `direction` or URDF limit changed
- Verification fails after `set-zero`
- Calibration record missing on boot (manual method)

## Related docs

- [ADR 0006: Homing, zero, and joint reference](decisions/0006-homing-zero-reference.md)
- [hardware/docs/homing-sensors.md](../hardware/docs/homing-sensors.md) — mechanical Hall layout
- [tuning.md](tuning.md) — `position_hold_trim_rad` vs `home_offset_rad`
