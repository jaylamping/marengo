# bins/

## Responsibility
Thin **runtime binaries** — entry points for Pi control, gateway, and bench tools. Each bin wires crates together; minimal logic lives here.

## Design
| Binary | Role | Entry |
|--------|------|-------|
| `marengo-pi` | Pi control loop + Chappe + stdin REPL | [marengo-pi/codemap.md](marengo-pi/codemap.md) |
| `marengo-gateway` | HTTP/WebTransport bridge to Chappe IPC | [marengo-gateway/codemap.md](marengo-gateway/codemap.md) |
| `motor-repl` | Bench motor CLI (status, disable, set-zero, gravity-preview) | [motor-repl/codemap.md](motor-repl/codemap.md) |
| `marengo-log-cli` | Query archived bench sessions | [marengo-log-cli/codemap.md](marengo-log-cli/codemap.md) |
| `marengo-limit-sync` | Durable-gated Set Limits sync into the local checkout | [marengo-limit-sync/codemap.md](marengo-limit-sync/codemap.md) |
| `imu-probe` | Read-only BNO085 hardware check | [imu-probe/codemap.md](imu-probe/codemap.md) |

## Flow
Typical Pi bench session:
1. `motor-repl status` or `marengo-pi` starts → load config from `MARENGO_CONFIG_DIR`
2. Construct `RuntimeBus` (SocketCAN) → Davout `Supervisor` → Berthier `ControlLoop`
3. `marengo-gateway` listens on Chappe Unix socket → serves Consul
4. Operator uses Consul or stdin commands → enable → control loop ticks

## Integration
- Chappe producers (`marengo-pi`, `marengo-gateway`) use `chappe::tracing_layer::init_subscriber`; all other bins use `marengo-support::init_tracing()` at startup
- Config via `MARENGO_CONFIG_DIR`, CAN via `MARENGO_CAN_INTERFACE`
- Deployed to Pi via `scripts/deploy-pi.sh` → systemd units
