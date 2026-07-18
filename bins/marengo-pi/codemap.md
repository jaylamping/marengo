# bins/marengo-pi/

## Responsibility
**Primary Pi runtime** — CAN I/O, Berthier control loop, Chappe telemetry publish, stdin operator REPL, and optional IMU/host metrics.

## Design
- **Event loop**: `run_control_loop` at configured Hz; parallel tokio tasks for stdin and Chappe command drain.
- `PiCommand` enum: enable, disable, status, set-zero, hold-on, hold-at, gravity-on, quit.
- Chappe subscribers for `EnableRequest`, homing commands, testing panel commands from Consul.
- Preflight `preflight_gravity_saturation` before enable (refuses if τ_g exceeds motor limits).

## Flow
1. `main` → parse args → load config → `RuntimeBus::open(can_interface)`
2. Build `ControlLoop<RuntimeBus>` with dynamics model
3. Spawn stdin reader + Chappe IPC bridge
4. `run_control_loop`: tick → publish RobotState/SafetyState/Heartbeat on Chappe
5. `handle_command` for operator stdin; `drain_chappe_commands` for remote enable

## Integration
- **Crates**: berthier, davout, robstride, chappe, marengo-config, armee-dynamics, marengo-host-metrics
- **Consumed by**: systemd `marengo-pi.service` on bench Pi
- **Peers**: marengo-gateway (Chappe IPC), Consul (via gateway)

**Detailed map**: [src/codemap.md](src/codemap.md)
