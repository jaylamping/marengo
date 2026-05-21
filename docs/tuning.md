# Control tuning (4-DOF arm bring-up)

Operator workflow for OpenArm-style impedance after gravity compensation is validated. See [ADR 0004](decisions/0004-control-modes-and-mit.md).

## Prerequisites

- Per-joint sign test complete ([safety.md](safety.md)).
- `motor-repl gravity-preview` matches expected directions.
- `GravityComp` stable on bench (no free-fall in upright poses).

## Zero pose

OpenArm convention: arms straight down when possible. Record encoder zeros with `SetZero` per motor after mechanical zeroing.

## Gain sweep

1. Start `config/control.yaml` impedance `kp`/`kd` at documented defaults.
2. **RS03 / RS04 joints** (shoulders, waist, outer hip, **hip pitch**, **knee**): `kd` up to 100; do not copy to RS02 joints.
3. **RS02 joints** (ankles, arm yaw/elbow/wrist): `kd` max **5** per vendor table.
4. Increase `kp` until contact feels crisp without oscillation; back off 20%.

## Friction

Tune `fc`, `fv`, `fo`, `k` under each joint in `config/control.yaml` (Berthier `friction_torque` model).

## Modes

| Mode | When |
|------|------|
| `GravityComp` | First hardware enable, elevated poses |
| `Impedance` | After G-comp signed off |
| `Position` | Only when not holding elevated configurations |
