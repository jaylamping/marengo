# config/

Declarative YAML for robot runtime: motors, control law, homing, and URDF pointer.

## Master SoT

| File | Role |
|------|------|
| `robot.yaml` | Joint list, URDF path (`assets/urdf/marengo.urdf`), bench caps |
| `motors.yaml` | CAN map, motor types, bench hard limits |
| `control.yaml` | Gains, velocity caps (ADR 0010), danger zones |
| `homing.yaml` | Per-joint homing methods |

Pi install: `/opt/marengo/config/` (`MARENGO_CONFIG_DIR`).

## Flow

1. `marengo-pi` loads four YAML files from `MARENGO_CONFIG_DIR` (or `<repo>/config` in dev).
2. Davout validates motors ⊆ `robot.joints` and applies limits from URDF ∩ motors.
3. Set Limits write-behind (ADR 0012) persists to the same master tree + `marengo.urdf`.

## Related

- Loaders: [crates/marengo-config/codemap.md](../crates/marengo-config/codemap.md)
- URDF: [assets/urdf/marengo.urdf](../assets/urdf/marengo.urdf), archives under [assets/urdf/archive/](../assets/urdf/archive/)
- Humanoid templates: `robot_humanoid.yaml`, `motors_humanoid.yaml` (not active bench)
