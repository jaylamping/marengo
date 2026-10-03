# Homing and zero reference

Marengo separates **home reference**, **semantic zero**, and **verified startup state**. Read this with [safety.md](safety.md) and [pi-commissioning.md](pi-commissioning.md).

## Terminology

| Term | Meaning |
|------|---------|
| **Home reference** | A repeatable physical feature (Hall sensor, limit switch, hardstop, encoder index) found at startup. |
| **Semantic zero** | The joint angle used by URDF, gravity, and control (`q = 0`). |
| **Home offset** | `home_offset_rad`: maps detected home reference to semantic zero. `semantic_zero = home_reference + home_offset_rad`. |
| **Firmware zero** | Robstride `SetZero` — encoder count stored in the motor drive. |
| **Verified** | Davout's private current-reference permission projected into live state; history or a caller-set flag cannot establish it. |
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
| `manual_reference` | Supported mechanical placement at home, explicit sign attestation and qualified physical evidence after Set Zero ([ADR 0036](decisions/0036-physical-robstride-reference.md)). |
| `hall_three_sensor` | **Unimplemented live workflow** — slow search, edge detect, backoff/re-approach, apply `home_offset_rad`, optional firmware `SetZero`. |
| `none` | No physical reference workflow; it cannot establish live bench reference. |

## Qualified physical reference workflow

Davout keeps current-reference permission private and checks it at Ready, every
Enable path and motor output. History, legacy flags and the journal never grant
it. Only the explicit physical owners
(`Supervisor::from_repo_with_physical_reference`,
`ControlLoop::from_repo_with_physical_reference`) can acquire. Plain
`from_repo` still refuses with `ReferenceUnsupported`. See
[ADR 0036](decisions/0036-physical-robstride-reference.md).

Each joint runs on its own: an all-address stop with reporting off (for the
target too, even if this process never turned its stream on), a drain, and a
type-0 identity read. On SocketCAN it then waits until the target's reporting
Off has been read back from the wire at least one control period earlier.
Then comes Enable to the target only, a drain, and SetZero. Next it
needs a type-2 ack from the target after SetZero, within
`zero_verify_tolerance_rad`, and a type-17 `mechPos` (0x7019) readback requested
after the ack, within the same tolerance. Then come the all-address stop, the
durable journal row and the grant. Grants accumulate per joint and bind the MCU
UID.

**Same process.** Grants never cross processes, so home and enable through
one `marengo-pi`:

```text
home right_shoulder_pitch right_elbow_pitch sign-tested
reference right_shoulder_pitch current pos=0.0012
reference right_elbow_pitch current pos=-0.0008
home            # readiness check (all configured joints granted) → Ready
enable bench
```

Joints are acquired in order. A failure prints `reference <joint> failed:
<message>`, and every later queued joint prints `reference <joint> skipped:
earlier joint failed`. Refusals at parse time (no `sign-tested`, unknown joint,
queue busy) print `home failed: <message>`. Other commands wait until the queue
drains. `disable`, `quit` and E-stop cancel it (`failed: cancelled`). A plain
`home` keeps its readiness check.

`motor-repl set-zero <joint> --sign-tested` runs the same qualified workflow and
journal. Its grant ends when the process exits, so it cannot prepare a later
`marengo-pi enable`.

**Journal path.** `MARENGO_REFERENCE_JOURNAL`, or else
`reference-journal.sqlite3` next to the calibration record
(`MARENGO_CALIBRATION_RECORD`, else `homing.yaml` `calibration_record_path`).
On the Pi that is `/opt/marengo/var/calibration/reference-journal.sqlite3`. The
path is made absolute and must differ from the calibration record.

**What revokes a grant.** Revoked per joint: a UID change, a missing or
mismatched UID in the type-0 check at Enable, a coordinate discontinuity,
Calibration drive mode, or no feedback for longer than `comm_watchdog_ms`
outside reference work. Revoked for all joints: a fault, E-stop, uncertain stop,
shutdown, or a change to the model or relevant policy. A successful ordinary
Disable keeps grants.

**Identity check at Enable.** Each target gets a type-0 request, repeated every
10 ms while it stays silent, and must answer within 100 ms of the first. About
535 ms after a SetZero every drive goes quiet for 48-57 ms and drops any type-0
it receives meanwhile, so an `enable` right after `home` can land in that gap.

**Enable after a fresh home.** A frame received in that gap is never acted on,
so on SocketCAN Davout writes no Enable (nor its preceding type-24 Off) to a
drive until 650 ms (`POST_SET_ZERO_QUIET`) after its last SetZero. An `enable`
right after `home` succeeds; just-zeroed targets are enabled up to about
650 ms later, and the log says "Enable held until the post-SetZero quiet
elapses".

### Pi bench procedure

Arm supported at mechanical home, hands off, physical E-stop reachable:

1. `pi_sync_main` (deploy rev matches, gateway healthy).
2. `pi_can_up`, then `pi_health`.
3. `pi_marengo_pi_script` with lines:
   `home right_shoulder_pitch right_shoulder_roll right_upper_arm_yaw right_elbow_pitch right_lower_arm_yaw sign-tested`,
   `sleep 5`, `status`, `home`, `enable bench`, then the supported test
   commands and `disable`. Expect one `reference <joint> current pos=` line per
   joint, then `homing verified → Ready` and `enabled (operator=bench)`.
4. `pi_candump_summary`. For each drive `n`, expect `0000FD0n` → `00000nFE`
   (identity), `0600FD0n` followed by a type-2 `02…` from that drive, and
   `1100FD0n#1970…` → `11000nFD#1970…` (readback).
5. `pi_logs_tail` and `pi_logs_last_fault`.

If a line reports `failed:`, stop, read the message and the last fault, and do
not enable. The firmware assumptions in ADR 0036 (SetZero accepted while
enabled, in-order readback, persistence unknown) stay unqualified until this
procedure passes on the bench.

The fresh `motor-repl disable` path also requires full startup configuration
and history loading; a corrupt resource can prevent it from reaching its stop
writes. It is not a qualified emergency-stop mechanism. An accepted socket write
or software Disabled state does not prove physical stop.

Positive software tests use scripted buses with literal Robstride frames, and a
closed in-memory simulator with an explicit initial virtual reference fixture.
Neither proves firmware behavior or physical readiness. See
[ADR0023](decisions/0023-private-current-reference-authority.md).

## Out-of-range recovery requirements

If feedback is outside effective limits or zero is stale:

1. Request stop from the installed owner and retain its delivery outcome; use the independent physical E-stop when needed.
2. Manually move to a known safe pose **or** run constrained homing when Hall sensors exist.
3. Re-run sign test if direction may have changed.
4. Establish a qualified current reference (`home <joints> sign-tested` in the owning `marengo-pi`) before enable.

Blind position hunting without sensors or operator reference is **not** allowed.

## Calibration record

Host-side history: `var/calibration/zero_registry.yaml` by default, or an absolute bench path such as `/opt/marengo/var/calibration/zero_registry.yaml` for live Pi profiles. The path is configurable via `homing.yaml`; Supervisor composition accepts a runtime override with `MARENGO_CALIBRATION_RECORD`. A relative override retains its process-working-directory interpretation. The pure homing library uses its supplied path and does not read environment variables.

Records per joint: device ID, interface, method, offset, timestamp, config revision, verification result, sign-test status and operator. Construction preserves loaded rows and existing bytes. The current writer replaces the previous row for a joint; it is not yet an immutable transaction audit log. Firmware `SetZero` or a saved row alone does not establish current reference.

The legacy scalar manual-history validator rejects nonfinite pose/bounds/
tolerance/offset, negative tolerance, empty/reversed bounds, joint mismatches
and unsupported Hall/None methods before state or history mutation. A valid
scalar history record still conveys no output permission or device evidence.

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
