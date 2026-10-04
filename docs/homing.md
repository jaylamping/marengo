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

`Ready` requires **all configured joints Verified** (Davout's private
current-reference permission, via `set_homing_complete`) and no latched faults.

Every startup begins `Unhomed`. Legacy calibration history is retired (WP-T):
no history file is read, no history path is accepted, and malformed or missing
legacy rows cannot fail construction or grant reference. The reserved history
location (`homing.yaml calibration_record_path`, never read) only locates the
reference journal beside it. See
[ADR 0022](decisions/0022-calibration-history-and-current-reference.md) and its
WP-T retirement note.

## Sensor truth table (3-Hall layout, deferred)

Hall-sensor homing is deferred (D-4): the sensor module is removed and no live
workflow reads Hall inputs. The mechanical layout in
[hardware/docs/homing-sensors.md](../hardware/docs/homing-sensors.md) is
retained for the future GPIO adapter. The truth table below documents the
intended interpretation when that work lands.

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
| `hall_three_sensor` | **Retired** — the Hall sensor module is removed (D-4); this method has no live workflow and is rejected as unsupported. |
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

`zero_verify_tolerance_rad` must be finite, positive, and at most `0.1` rad;
`search_timeout_s` must be finite, positive, and at most `300` seconds.
Homing config is schema-checked: unknown YAML keys are rejected rather than
silently ignored.

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

**Checking homing.** Only the live `marengo-pi` knows its grants. A fresh
`motor-repl homing-status` always reads `Unhomed` while its Supervisor still
opens SocketCAN and sends type-24 frames, so no install, deploy or health path
runs it. `pi_health` and `pi_homing_status` show the running `marengo-pi`'s
RobotState via the gateway; with no `marengo-pi` they print `no live marengo-pi
session: reference grants are process-local (ADR 0036)` and the latest journal
rows (`scripts/reference-journal-tail.py`, SQLite read-only). Journal rows are
history and grant nothing.

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
10 ms while it stays silent, and must answer within 100 ms of the first.
511-614 ms after a SetZero every drive goes quiet for 45-61 ms and drops any
type-0 it receives meanwhile, so an `enable` right after `home` can land in that
gap.

**Enable after a fresh home.** A frame received in that gap is never acted on,
so on SocketCAN Davout writes no Enable (nor its preceding type-24 Off) to a
drive until 800 ms (`POST_SET_ZERO_QUIET`) after its last SetZero. An `enable`
right after `home` succeeds; just-zeroed targets are enabled up to about
800 ms later, and the log says "Enable held until the post-SetZero quiet
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

`motor-repl disable` is independent of configuration and history loading: it
needs only the drive addresses in `motors.yaml`, sends each drive a Disable and
then a type-24 Off (a Disable does not end active reporting, see
[safety.md](safety.md) *Reporting Off at exit*), and reports every frame's
outcome (exit 1 if any was not sent). It is still
not a qualified emergency-stop mechanism. An accepted socket write or software
Disabled state does not prove physical stop, and Disable does not clear a
latched drive fault.

Positive software tests use scripted buses with literal Robstride frames, and a
closed in-memory simulator with an explicit initial virtual reference fixture.
Neither proves firmware behavior or physical readiness. See
[ADR0023](decisions/0023-private-current-reference-authority.md).

## Out-of-range recovery requirements

If feedback is outside effective limits or zero is stale:

1. Request stop from the installed owner and retain its delivery outcome; use the independent physical E-stop when needed.
2. Manually move to a known safe pose. (Constrained Hall homing is deferred with the sensor module.)
3. Re-run sign test if direction may have changed.
4. Establish a qualified current reference (`home <joints> sign-tested` in the owning `marengo-pi`) before enable.

Blind position hunting without sensors or operator reference is **not** allowed.

## Retired calibration history

Host-side history (`var/calibration/zero_registry.yaml` by default) is retired
(WP-T, D-3): nothing reads it, nothing writes it, and the `marengo-homing`
crate (scalar verifier, YAML history, legacy registry, Hall sensors) is folded
into `davout::homing_facets` or deleted. The `homing.yaml`
`calibration_record_path` setting is retained because it locates the reference
journal beside it (`MARENGO_REFERENCE_JOURNAL`, else the journal next to the
reserved history path). A saved row or firmware `SetZero` alone never
establishes current reference.

## Stale-zero triggers

Re-calibrate when:

- Motor firmware ID changed
- Motor or gearbox disassembled
- Hall magnet or sensor replaced
- `direction` or URDF limit changed
- Verification fails after `set-zero`
- New owner/process startup: current reference is unknown; there is no history to consult

## Related docs

- [ADR 0006: Homing, zero, and joint reference](decisions/0006-homing-zero-reference.md)
- [hardware/docs/homing-sensors.md](../hardware/docs/homing-sensors.md) — mechanical Hall layout
- [tuning.md](tuning.md) — `position_hold_trim_rad` vs `home_offset_rad`
