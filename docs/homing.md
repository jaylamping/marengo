# Homing and zero reference

Marengo separates **home reference**, **semantic zero**, and **verified startup state**. Read this with [safety.md](safety.md) and [pi-commissioning.md](pi-commissioning.md).

## Terminology

| Term | Meaning |
|------|---------|
| **Home reference** | A repeatable physical feature (Hall sensor, limit switch, hardstop, encoder index) found at startup. |
| **Semantic zero** | The joint angle used by URDF, gravity, and control (`q = 0`). |
| **Home offset** | `home_offset_rad`: maps detected home reference to semantic zero. `semantic_zero = home_reference + home_offset_rad`. |
| **Firmware zero** | Robstride `SetZero` — encoder count stored in the motor drive. |
| **Verified** | Current-process reference state required for normal enable; a historical row cannot establish it. |
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

Every new registry starts all configured joints `Unhomed`, including joints with
apparently matching calibration history. Reading a saved row does not verify the
current motor, reference or process. History stays available for inspection;
missing history starts empty, while malformed or unreadable history returns an
error without overwriting it. See
[ADR 0022](decisions/0022-calibration-history-and-current-reference.md).

## Sensor truth table (3-Hall layout)

One magnet on the rotating member; three fixed Hall sensors on the housing.
Truth table values are **logical active states after polarity normalization**.
A3144-style Hall switches are electrically active-low when used with MCP23017 pull-ups, but should still appear as `1` when active in homing logic.

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
| `manual_reference` | **Target contract** — supported mechanical placement, explicit sign attestation and qualified evidence after Set Zero. Current cached-pose verification does not satisfy this contract. |
| `hall_three_sensor` | **Unimplemented live workflow** — slow search, edge detect, backoff/re-approach, apply `home_offset_rad`, optional firmware `SetZero`. |
| `none` | No physical reference workflow; it cannot establish live bench reference. |

## Current commissioning limitation

The former sequence of separate `motor-repl set-zero`, `home`, then Pi `enable`
processes depended on saved history granting readiness. It now refuses at the
fresh-process reference gate. `home` checks current readiness; it does not acquire
a physical reference. The saved rows remain available and do not convey a grant
between processes.

The reference repair is still in progress: current Set Zero checks cached pose,
and the CLI calibration path can enable peers or exit without reliable cleanup.
Do not use unchecked readiness setters or synthetic bench grants to restore that
sequence. A live commissioning procedure requires the remaining target-only
preflight, stop-before-storage, qualified postcommand evidence and single-owner
request/receipt work tracked as CS05/CS06/CS07 in the
[repair roadmap](reviews/2026-09-29/implementation-roadmap.md).

Keep the arm supported and the physical E-stop reachable for any later supervised
commissioning. The fresh `motor-repl disable` path also requires full startup
configuration and history loading; a corrupt resource can prevent it from
reaching its stop writes. It is not a qualified emergency-stop mechanism. An
accepted socket write or software Disabled state does not prove physical stop.

## Out-of-range recovery requirements

If feedback is outside effective limits or zero is stale:

1. Request stop from the installed owner and retain its delivery outcome; use the independent physical E-stop when needed.
2. Manually move to a known safe pose **or** run constrained homing when Hall sensors exist.
3. Re-run sign test if direction may have changed.
4. Establish a qualified current reference before enable once the owner workflow is implemented.

Blind position hunting without sensors or operator reference is **not** allowed.

## Calibration record

Host-side history: `var/calibration/zero_registry.yaml` by default, or an absolute bench path such as `/opt/marengo/var/calibration/zero_registry.yaml` for live Pi profiles. The path is configurable via `homing.yaml`; Supervisor composition accepts a runtime override with `MARENGO_CALIBRATION_RECORD`. A relative override retains its process-working-directory interpretation. The pure homing library uses its supplied path and does not read environment variables.

Records per joint: device ID, interface, method, offset, timestamp, config revision, verification result, sign-test status and operator. Construction preserves loaded rows and existing bytes. The current writer replaces the previous row for a joint; it is not yet an immutable transaction audit log. Firmware `SetZero` or a saved row alone does not establish current reference.

## Stale-zero triggers

Re-calibrate when:

- Motor firmware ID changed
- Motor or gearbox disassembled
- Hall magnet or sensor replaced
- `direction` or URDF limit changed
- Verification fails after `set-zero`
- New owner/process startup: current reference is unknown even when history exists

## Related docs

- [ADR 0006: Homing, zero, and joint reference](decisions/0006-homing-zero-reference.md)
- [hardware/docs/homing-sensors.md](../hardware/docs/homing-sensors.md) — mechanical Hall layout
- [tuning.md](tuning.md) — `position_hold_trim_rad` vs `home_offset_rad`
