# Right-arm first lower-yaw motion — October 2, 2026

Intentional lower-arm-yaw motion was executed remotely under Joseph's existing
development-session authorization. The unchanged supported assembly, clear
workspace, working physical E-stop and stopped runtime were confirmed earlier
in the session. Routine fixes and retries used that authorization.

The latest three executions used source
`f2beb0e686619c0d3186089f8a2924443c098dd5` and Linux SocketCAN executable SHA256
`37d7e163b094d1c2dd18026c6f22b8e274449050885bb1e62156b7effe9ff786`.
Each commanded lower yaw from zero to +20 mrad over 400 ms, back over 400 ms,
then zero for the remaining one-second enabled window. Other joints received
fully neutral MIT commands. Gains were selected once per owner and recorded.

Raw receipts, history-only audits, passive captures, command lines, hashes and
authorization records for all nine attempts are in the
[evidence directory](2026-10-02-right-arm-lower-yaw/).
[analysis.json](2026-10-02-right-arm-lower-yaw/analysis.json) is derived offline
by [analyze.py](2026-10-02-right-arm-lower-yaw/analyze.py), which never contacts
the robot. Reproduce it with:

```bash
python3 docs/commissioning/evidence/2026-10-02-right-arm-lower-yaw/analyze.py
```

## Completed motion results

All quantities are reported joint-space measurements. RMS compares feedback
with the requested target at each recorded tick, without shifting for lag.

| Source / attempt suffix | kp / kd | Peak position (mrad) | Final position (mrad) | RMS tracking error (mrad) | Maximum canonical speed (rad/s) |
|---|---|---:|---:|---:|---:|
| 0f2bb72 / J6R1sx | 10 / 0.4 | 1.534 | 0.383 | 9.650 | 0.077 |
| 0a99e61 / CLyLXZ | 30 / 1 | 10.738 | 0.383 | 5.455 | 0.154 |
| f2beb0e / isl5YO | 30 / 0.4 | 19.942 | 6.519 | 5.191 | 0.153 |
| f2beb0e / R4DIL3 | 30 / 1 | 15.341 | 6.136 | 4.306 | 0.153 |
| f2beb0e / fngbiM | 30 / 2 | 12.656 | 4.985 | 4.891 | 0.177 |

The 30/0.4 trial reached about 1.14 degrees, close to the requested peak.
Among the three corrected single trials, 30/1 had the lowest observed RMS error;
30/2 had the smallest return error. These are preliminary comparisons, not
repeatability qualification or a recommendation to persist runtime gains.
The weak 10/0.4 response is consistent with friction, but no friction model
has been identified by these tests.

Each completed run recorded 195 full five-drive samples, elapsed about
1,000,056 us and delivered all 15 stop writes with zero failures. The three
latest maximum tick delays were 55, 54 and 53 us. Captures show one Enable per
drive, gained MIT output only to ID 5, neutral commands to the neighbors and
zero torque feedforward. The latest passive captures place the last Disable
about 1,005 ms after first Enable. Ordinary status headers were fault-free
Reset or Run. Accepted writes and status modes do not measure physical stop
distance or current decay.

## Retained aborts and diagnosis

The first attempt (`2c5ef85 / uSABqx`) aborted in neutral bootstrap on a
stationary shoulder-roll raw velocity estimate; no gained output was sent.
The additional profile velocity guard was incorrectly applied to neutral
neighbors. `0f2bb72` confined it to the exercised lower-yaw joint, leaving
ordinary neighbor feedback policy intact.

Three `0a99e61` attempts then aborted on lower-yaw raw velocity:

| Attempt suffix / kp / kd | First trigger after Enable (ms) | Raw velocity (rad/s) | Position-derived velocity over preceding sample interval (rad/s) |
|---|---:|---:|---:|
| 9p6QxO / 30 / 0.4 | 322.7 | 0.304 | 0.077 |
| b3HXLY / 30 / 1 | 777.8 | -0.290 | -0.077 |
| xzfOIC / 30 / 2 | 427.7 | 0.253 | 0.077 |

These observations showed a mismatch between the extra guard's raw field and
the runtime's velocity metric. They do not reconstruct instantaneous motion
between encoder samples. The exact `xzfOIC` frame pair reproduced rejection
twice in the normal receive/guard path. `f2beb0e` retained the 0.25-rad/s ceiling
but applies it to the canonical once-per-drain position-derived estimate while
Active, with initial raw fallback. Ready Enable replies retain the raw
per-observation guard. Raw position hazards and faults remain before coalescing.
Regression tests cover the capture, actual position-derived overspeed with raw
velocity zero, first-raw overspeed, and Ready overspeed before gained output.
All three corrected gain trials then completed the original motion sequence.

## Preserved state and remaining work

Every attempt's installed YAML, URDF and main runtime executable hashes match
before and after. MainPID remains zero and the runtime inactive. Each owner
acquired fresh near-home qualification; Set Zero may update the current drive
coordinate, and prior calibration history is not imported as permission. Audits
are history only, and each finished owner revokes its reference.

First lower-yaw direction and deliberate motion are established. Accurate return
to zero, repeatability, friction characterization, the other four joint responses,
gravity compensation and the full limb suite remain unfinished. Motor-power
timeout volatility and physical output-stop behavior remain unmeasured.
