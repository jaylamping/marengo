# bins/ — Thin runtimes

9 binaries. Rule: **thin `main`, logic in `crates/`.** Each bin calls `marengo_support::init_tracing()` (or `chappe::tracing_layer::init_subscriber` for Chappe producers).

## Binaries

| Binary | Host | Purpose | Status |
|--------|------|---------|--------|
| `marengo-pi` | Raspberry Pi | Control + CAN + Chappe | Active — main runtime |
| `marengo-gateway` | Pi | HTTP gateway, log store, Chappe bridge | Active |
| `marengo-jetson` | Jetson | Planner, Fouché, Chappe | Scaffold |
| `marengo-log-cli` | Dev | Query archived bench sessions (SQL store) | Active |
| `motor-repl` | Dev (bench) | Interactive motor exercise: status/enable/jog/set-zero/gravity | Active |
| `imu-probe` | Pi | BNO085 I2C quaternion probe | Active |
| `probe` | Dev | Bus / diagnostics | Scaffold |
| `wave-demo` | Dev | Demo trajectories | Scaffold |
| `teleop` | Dev | Teleoperation input | Scaffold |

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Pi control loop wiring | `marengo-pi/src/main.rs` |
| HTTP gateway / health | `marengo-gateway/src/` |
| Bench motor commands | `motor-repl/src/main.rs` (status/enable/jog/set-zero/gravity-on/gravity-preview) |
| IMU probe | `imu-probe/src/main.rs` |
| Log archive queries | `marengo-log-cli/src/` |

## CONVENTIONS

- Chappe producers (`marengo-pi`, `marengo-gateway`) → `chappe::tracing_layer::init_subscriber` (publishes `LogEvent` on `logs/structured`).
- Other bins → `marengo_support::init_tracing()` (stdout/journal).
- `anyhow::Result` in `main` is fine; print/log errors for operators.
- `motor-repl` uses SocketCAN only — no Motor Studio dependency.
- Env vars: `MARENGO_ROOT`, `MARENGO_CONFIG_DIR`, `MARENGO_CAN_INTERFACE`.

## ANTI-PATTERNS

- Logic in `bins/` — move to `crates/`.
- `println!` for runtime logs in Chappe producers → `tracing`.
- Direct CAN access from bins → go through Davout via Berthier `ControlLoop`.
