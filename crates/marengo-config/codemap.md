# crates/marengo-config/

## Responsibility
**Declarative YAML configuration** loader — typed parsers for `robot.yaml`, `motors.yaml`, `control.yaml`, `homing.yaml`, `network.yaml`. Single source for bench parameters; no realtime logic.

## Design
- **Repository/Loader** pattern: one struct per config file (`RobotConfigFile`, `MotorsConfigFile`, etc.).
- Validation at load time: motor joints ⊆ robot joints, URDF path exists (`resolve_urdf_path`).
- `MotorType` enum (RS00–RS04) shared with robstride encoding.
- `DangerZoneRule`, control gains, loop Hz, comm watchdog thresholds from control.yaml.

## Flow
1. Bin sets `MARENGO_CONFIG_DIR` or passes `--config-dir`
2. `load_robot_config`, `load_motors_config`, `load_control_config` at startup
3. Davout/Berthier hold parsed structs for loop lifetime
4. Bringup profiles under `config/bringup/` swap full config trees per bench setup

## Integration
- **Consumed by**: davout, berthier, armee-dynamics, robstride, marengo-homing, all bins
- **Config root**: `config/` (see [config/codemap.md](../../config/codemap.md))

**Detailed map**: [src/codemap.md](src/codemap.md)
