# Right arm, 2026-10-02: PR #254 findings (history)

[PR #254](https://github.com/jaylamping/marengo/pull/254)
(`codex/right-arm-commissioning`, 13 commits `dfbf098f`..`fca53e9d`, based on
`ba0fff72`) ran the first enabled tests on the five-joint right arm through a
finite bench owner parallel to the then-unsupported physical reference. It was
closed without merging: [ADR 0036](../decisions/0036-physical-robstride-reference.md)
now owns physical reference, the bench owner enabled 35-57 ms after SetZero
(inside main's 800 ms `POST_SET_ZERO_QUIET`), and the motion ran remotely under a
development-session authorization that main policy does not adopt
(`docs/safety.md`: operators at the robot). This note keeps the measured facts.
Raw receipts and candumps stay on the PR branch under
`docs/commissioning/evidence/2026-10-02-right-arm-*`; none are copied to main.

## Drive facts

- **Firmware is mixed** (disabled protocol inspection, ~20:14 UTC, source
  `e5599c60`): ids 1-2 (RS03) 0.3.1.42, ids 3-4 (RS02) 0.2.3.34, id 5 (RS00)
  0.0.3.32. RunMode read MIT (0), ZeroSta 0, AddOffset 0 and CanTimeout 0 on all
  five. `config/motors.yaml` now records these versions.
- **CanTimeout was written.** The disabled bench-home qualification (21:02:55
  UTC, source `cd033808`, commit `8d8976e9`) wrote CanTimeout (0x7028) = 600
  counts (~30 ms) to all five drives and read 600 back; every later lower-yaw
  run read 600 again. Nothing records the value being cleared or the drives
  being power-cycled, so treat the drive timeout as unknown until 0x7028 is read
  back (`docs/safety.md`, WP-I D2).
- **No blackout after a Disabled SetZero.** In six lower-yaw captures (30
  SetZeros, host id `0xD4`), SetZero went to Disabled drives, which were
  Enabled 35-56 ms later and replied every 5 ms (max gap 5.1 ms) from 450 to
  720 ms after it. Main's blackouts all followed SetZero to an Enabled drive.
  Unverified; see the open item in
  [robstride-firmware-behavior.md](firmware/robstride-firmware-behavior.md).

## Neutral enable

Source `af28b864`: one Enable per drive, 500 ms enabled window of all-zero MIT
commands, 15 accepted stop writes. Lower yaw drifted 9.6 mrad (~0.55°); the
other joints stayed within 0.8 mrad.

## First lower-yaw motion

Commits `b3c243df`..`f2beb0e6`, recorded in `fca53e9d`. Each run commanded lower
yaw 0 → +20 mrad over 400 ms, back over 400 ms, then held zero for the rest of a
1 s enabled window; the other four joints received neutral MIT commands and
torque feed-forward was zero. Each completed run logged 195 five-drive samples
and delivered all 15 stop writes. RMS error compares feedback with the target
at each tick, without lag compensation.

| Source / attempt | kp / kd | Peak (mrad) | Final (mrad) | RMS error (mrad) | Max speed (rad/s) |
|---|---|---:|---:|---:|---:|
| `0f2bb72` / J6R1sx | 10 / 0.4 | 1.5 | 0.4 | 9.65 | 0.077 |
| `0a99e61` / CLyLXZ | 30 / 1 | 10.7 | 0.4 | 5.46 | 0.154 |
| `f2beb0e` / isl5YO | 30 / 0.4 | 19.9 | 6.5 | 5.19 | 0.153 |
| `f2beb0e` / R4DIL3 | 30 / 1 | 15.3 | 6.1 | 4.31 | 0.153 |
| `f2beb0e` / fngbiM | 30 / 2 | 12.7 | 5.0 | 4.89 | 0.177 |

Among the three `f2beb0e` trials, 30/1 had the lowest RMS error and 30/2 the
smallest return error; 30/0.4 nearly reached the 20 mrad peak. 10/0.4 barely
moved, consistent with friction (not modelled). These are single trials, not a
gain recommendation; `config/control.yaml` was not changed.

Three `0a99e61` attempts aborted on the vendor raw velocity field: it read
0.25-0.30 rad/s while the position-derived speed over the same sample interval
was 0.077 rad/s. Raw-velocity guards are noisy at small motions.

## Not established

Return accuracy, repeatability, friction, the other four joints' responses,
gravity compensation, CanTimeout volatility and physical stop distance.
