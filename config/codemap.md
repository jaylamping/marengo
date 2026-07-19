# config/

## Responsibility
**Declarative robot configuration** — YAML files loaded by marengo-config at runtime. Defines joints, motors, control gains, homing, and network for each bench profile.

## Design
| File | Purpose |
|------|---------|
| `robot.yaml` | Robot name, URDF path, joint list, deprecated bench caps |
| `motors.yaml` | Per-joint motor mapping: `device_id`, `motor_type`, `direction`, `gear_ratio` |
| `control.yaml` | Loop Hz, kp/kd, torque caps, danger zones, comm watchdog |
| `homing.yaml` | Homing methods, encoder offsets |
| `network.yaml` | Chappe socket paths, gateway addresses |

Bringup **profiles** under `bringup/` swap complete config trees for specific bench setups (right-only arm, weighted, dual pitch, etc.).

## Flow
1. Operator sets `MARENGO_CONFIG_DIR=/opt/marengo/config/bringup/arm_3dof_right`
2. marengo-config loaders parse all YAML files
3. Davout builds limit tables; Berthier loads gains; robstride maps device_ids
4. MCP `pi_sync_bench_config` rsyncs local profile edits to Pi

## Integration
- **Loaded by**: marengo-config crate → all runtime bins
- **Validated against**: URDF in `assets/urdf/`, `hardware/docs/kinematics.md`
- **Profiles**: [bringup/codemap.md](bringup/codemap.md)
