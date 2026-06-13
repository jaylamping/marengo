# Control tuning (4-DOF arm bring-up)

Operator workflow for OpenArm-style impedance after gravity compensation is validated. See [ADR 0004](decisions/0004-control-modes-and-mit.md).

## Prerequisites

- Per-joint sign test complete ([safety.md](safety.md)).
- `motor-repl gravity-preview` matches expected directions.
- `GravityComp` stable on bench (no free-fall in upright poses).

## Zero pose

OpenArm convention: arms straight down when possible. Record encoder zeros with `SetZero` per motor after mechanical zeroing (`motor-repl set-zero` / `pi_set_zero` — Robstride lifecycle uses host id **0xFD** and set-zero data byte **0x01**, matching Motor Studio).

**Do not confuse:**

| Knob | File | Purpose |
|------|------|---------|
| `home_offset_rad` | `config/homing.yaml` | Maps Hall home reference → semantic zero |
| `position_hold_trim_rad` | `config/control.yaml` | Small hold-target fudge only; not homing |

See [homing.md](homing.md).

## Gain sweep

1. Start `config/control.yaml` impedance `kp`/`kd` at documented defaults.
2. **RS03 / RS04 joints** (shoulders, waist, outer hip, **hip pitch**, **knee**): `kd` up to 100; do not copy to RS02/RS00 joints.
3. **RS02 / RS00 joints** (ankles, arm yaw/elbow, wrists): `kd` max **5** per vendor table.
4. Increase `kp` until contact feels crisp without oscillation; back off 20%.

## Friction

Tune `fc`, `fv`, `fo`, `k` under each joint in `config/control.yaml` (Berthier `friction_torque` model).

## Modes

These are Marengo control modes, not Robstride firmware `run_mode` values. Marengo `Position` still sends MIT operation-control frames with non-zero gains; it does not switch the drive to firmware Position mode (`run_mode=1`).

| Marengo mode | Firmware path | When |
|--------------|---------------|------|
| `GravityComp` | MIT `run_mode=0` | First hardware enable, elevated poses |
| `Impedance` | MIT `run_mode=0` | Compliant at current pose (setpoint tracks measured q) |
| `Position` | MIT `run_mode=0` | **Hold-and-return:** latched setpoint via `marengo-pi` `hold-on` / `hold-at` |
| `Position` (legacy) | MIT `run_mode=0` | Only when not holding elevated configurations without G-comp |

### Position hold (`hold-on`)

1. Validate `GravityComp` at arm-down and mid-range first.
2. `enable bench` → `hold-on` (latches current encoder pose) or `hold-at <rad>`.
3. **`hold-at` ramps** toward the target at each joint's `position_slew_rad_s` (default **0.25 rad/s**). `position_slew_max_lead_rad` caps how far the MIT setpoint may run ahead of measured `q` so P gain stays bounded while the planner catches up.
4. **Planner vs setpoint:** Berthier keeps a rate-limited trajectory in joint space; each tick computes `q_des = clamp(q_traj, q, target, max_lead)`. Gravity FF uses measured `q`; friction and damping FF use **tracking error**, not error to the distant latch target. See [rust-patterns.md](rust-patterns.md) §7 and [ADR 0007](decisions/0007-bench-position-trajectory-control.md#update-2026-06-13-hold-at-plannermit-split).
5. Small push (~±0.1 rad); verify return without oscillation or limit fault.
6. Sweep `impedance.kp` / `kd` in bring-up `control.yaml`; back off 20% from first oscillation.
7. `hold-off` / `disable` before leaving bench.

Firmware Speed mode (`run_mode=2`, `spd_ref`) is a bench diagnostic path only and is disabled unless `control.bench.allow_firmware_speed_mode` is set explicitly.
