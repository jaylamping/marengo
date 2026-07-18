# bins/motor-repl/

## Responsibility
**Bench motor CLI** — interactive and one-shot commands for bring-up: status, enable, disable, jog, set-zero, gravity-preview, homing-status. All motion through Davout.

## Design
- Subcommand parser: `status`, `enable`, `disable`, `jog`, `set-zero`, `gravity-on`, `gravity-preview`, `homing-status`, `hold-on`, `hold-at`
- Shares `ControlLoop<RuntimeBus>` construction with marengo-pi
- `preflight_gravity_saturation` gate before enable
- `--config-dir` and `MARENGO_CONFIG_DIR` for bringup profile selection

## Flow
1. Parse bus args (`--can`, `--config-dir`)
2. Open SocketCAN → Supervisor → ControlLoop
3. Execute subcommand (single-shot or interactive REPL)
4. `disable` on exit

## Integration
- **Primary bench tool** for MCP `pi_hold_on`, `pi_motor_recover`, `pi_set_zero`
- **Crates**: berthier, davout, robstride, marengo-config, armee-dynamics

**Detailed map**: [src/codemap.md](src/codemap.md)
