# Robstride MIT field ranges

MIT operation-control (type 1) commands and status (type 2 / type 24) feedback carry
position, velocity and torque as `u16` codes mapped linearly onto `±scale`, and
kp/kd onto `0..scale`. A wrong scale is silent: frames still encode and decode,
but every value is off by the scale ratio. Marengo's scales live in
`MitRanges::for_motor_type` ([`crates/robstride/src/motor_type.rs`](../../../crates/robstride/src/motor_type.rs)).

## Ranges

| Model | Code (P / V / KP / KD / T) | RS manuals 251112, 260112, 260713 | Seeed SDK | Bench | Status |
|-------|----------------------------|-----------------------------------|-----------|-------|--------|
| RS00 | ±4π / **±50** / 500 / 5 / **±17** | ±12.57 / ±33 / 500 / 5 / ±14 | V 50, T 17 | untested | **Disputed** (open question below) |
| RS02 | ±4π / ±44 / 500 / 5 / ±17 | ±12.57 / ±44 / 500 / 5 / ±17 | V 44, T 17 | consistent (small motion) | OK |
| RS03 | ±4π / **±20** / 5000 / 100 / ±60 | ±12.57 / ±20 / 5000 / 100 / ±60 | V 50 | K = 20.05 rad/s | Fixed 2026-10-03 (was ±50) |
| RS04 | ±4π / ±15 / 5000 / 100 / ±120 | ±12.57 / ±15 / 5000 / 100 / ±120 | V 15, T 120 | not installed | OK |

The only firmware-dependent range in the manuals is position (12.5 before
firmware 0.0.2.6, 12.57 after); `4π` = 12.566 is within rounding. No manual
revision has a firmware-dependent velocity range.

## RS03 velocity: ±20, not ±50

- **Manual:** RS03 User Manual §4.1.2 gives "-20rad/s~20rad/s" for the type-1
  target velocity and the type-2 feedback velocity; the §4.4 program sample
  defines `V_MIN -20.0f` / `V_MAX 20.0f`. All three revisions agree.
- **Bench:** capture `cd-20261003T145133Z`, right_shoulder_pitch (id 1,
  firmware 0.3.1.42, gear 1). The status velocity code regressed against
  d(position)/dt gives a full scale of 20.05 rad/s (r = 0.999, n = 227; 20/30/50 ms
  windows give 20.03-20.06). Integrating velocity over each ramp gives 19.87-19.93.
  A true 1.74 rad/s peak decoded as 4.3-4.5 rad/s under ±50.
- **Command path:** the same capture's type-1 frames carry `0x8332`/`0x7CCC`
  at cruise, i.e. ±1.25 rad/s under ±50, matching the right_shoulder_pitch
  `position_trajectory_velocity_rad_s: 1.25`. The drive read them as ±0.50 rad/s.
- The Seeed SDK's RS03 = 50 (Python `table.py` and C++ `protocol.h`) is
  contradicted by both. Regression: `crates/robstride/tests/rs03_velocity_scale.rs`
  decodes nine literal type-2 frames from the capture and requires the integrated
  velocity to match the position change within 10% (0.996 at ±20, 2.49 at ±50).

### What the ±50 scale did (RS03: right_shoulder_pitch, right_shoulder_roll)

- **Feedback velocity** decoded 2.5× high. Davout's Active-mode velocity guards
  use Δposition/Δt, so the trip limit was unaffected, but the first sample after
  reset, non-Active published `dq`, gateway telemetry and the "uncorroborated
  velocity spike" debug path saw the inflated value.
- **Commanded `v_des`** reached the drive at 0.4×. The drive's damping term
  `kd·(v_des − v)` therefore pulled toward 0.4·v_des: at kd 3.0 and 1.25 rad/s it
  dragged by about 2.25 Nm. In the capture kd was 0 for most of the ramp and 3.0
  near settle.
- **Danger-zone `clamp_velocity`** limits reached the drive at 0.4×
  (`elevated_shoulder_pitch_fall` 0.45 rad/s arrived as 0.18).
- **Reference continuity bound** (`ContinuityBounds.max_rate_rad_s` =
  `velocity_scale / gear`) accepted 50 rad/s instead of 20, i.e. 0.25 rad
  instead of 0.10 rad per 5 ms.

After the fix all four carry their intended value, so RS03 ramps and settles
behave differently even with unchanged config.

### Config tuned while the drive saw 0.4× `v_des`

Not changed by the fix; re-check on the bench:

| File | Key | Value | Why suspect |
|------|-----|-------|-------------|
| `config/control.yaml` | `joints.right_shoulder_pitch.position_trajectory_velocity_rad_s` / `_accel_rad_s2` | 1.25 / 4.5 | Chosen with drive damping toward 0.5 rad/s |
| `config/control.yaml` | `joints.right_shoulder_pitch.impedance.kd` | 3.0 | Damping target was 0.4·v_des |
| `config/control.yaml` | `joints.right_shoulder_pitch.position_slew_rad_s` | 0.15 | Slew `v_des` reached the drive at 0.06 |
| `config/control.yaml` | `joints.right_shoulder_roll.position_trajectory_velocity_rad_s` / `_accel_rad_s2` | 0.7 / 4.0 | Tuned for stick-slip and decel feel with the extra drag |
| `config/control.yaml` | `joints.right_shoulder_roll.position_slew_rad_s` | 0.35 | "Slew too slow through the friction knee" observed under drag |
| `config/control.yaml` | `joints.right_shoulder_roll.impedance.kd` | 3.0 | Damping target was 0.4·v_des |
| `config/control.yaml` | `joints.right_shoulder_{pitch,roll}.friction.fc` | 0.08 | Identified with the extra damping drag present |
| `config/control.yaml` | `danger_zones.elevated_shoulder_pitch_fall.max_velocity_rad_s` | 0.45 | "~3-4 s return" judged while the drive saw 0.18 |
| `config/control.yaml` | `joints.right_{upper_arm_yaw,elbow_pitch,lower_arm_yaw}.position_trajectory_velocity_rad_s` | 1.43 / 1.43 / 1.10 | Derived from the roll cruise 0.70 (ratio comment), so inherit it |

Caps (`velocity_max_rad_s`, `motors.yaml` `velocity_limit_rad_s`, homing
`search_velocity_rad_s`) are joint-space limits checked before encoding and were
not tuned against the wire scale. Homing search velocity is validated only; it
is not sent as `v_des`.

## Open question: RS00 velocity and torque

Code keeps the Seeed SDK values (V ±50, T ±17). Every RS00 manual revision gives
V ±33 rad/s and T ±14 Nm (the 260713 `#define` omits the minus sign on `V_MIN`;
the frame text reads −33~33). right_lower_arm_yaw (id 5) never moved in either
October capture, so the bench cannot decide. Needed: an RS00 capture with real
motion (and a known torque load), then the same velocity-vs-Δposition fit used
for RS03. If the manual is right, RS00 feedback velocity reads 1.5× high and
`v_des` reaches the drive at 0.66×; torque reads and commands 1.21× off.
