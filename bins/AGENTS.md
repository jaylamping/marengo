# bins/ — Thin runtimes

6 binaries. Rule: **thin `main`, logic in `crates/`.** Chappe producers (`marengo-pi`, `marengo-gateway`) call `chappe::tracing_layer::init_subscriber`; every other bin calls `marengo_support::init_tracing()`.

## Binaries

| Binary | Host | Purpose | Status |
|--------|------|---------|--------|
| `marengo-pi` | Raspberry Pi | Control + CAN + Chappe | Active — main runtime |
| `marengo-gateway` | Pi | HTTP gateway, log store, Chappe bridge | Active |
| `marengo-log-cli` | Dev | Query archived bench sessions (SQL store) | Active |
| `marengo-limit-sync` | Dev | Apply a Durable-gated Set Limits patch to the local checkout | Active |
| `motor-repl` | Dev (bench) | Status/homing diagnostics, gravity preview, qualified Set Zero, independent disable/exit stop | Active |
| `imu-probe` | Pi | BNO085 I2C quaternion probe | Active |

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Pi control loop wiring | `marengo-pi/src/main.rs` |
| HTTP gateway / health | `marengo-gateway/src/` |
| Bench motor diagnostics and independent stop/reference commands | `motor-repl/src/main.rs` (`status`, `disable`, `set-zero`, `gravity-preview`); reference and enable inside the `marengo-pi` owner only |
| IMU probe | `imu-probe/src/main.rs` |
| Log archive queries | `marengo-log-cli/src/` |

## CONVENTIONS

- Chappe producers (`marengo-pi`, `marengo-gateway`) → `chappe::tracing_layer::init_subscriber` (publishes `LogEvent` on `logs/structured`).
- Other bins → `marengo_support::init_tracing()` (stdout/journal).
- `main` returns `ExitCode`; print/log errors for operators.
- `motor-repl` uses SocketCAN only — no Motor Studio dependency.
- Env vars: `MARENGO_ROOT`, `MARENGO_CONFIG_DIR`, `MARENGO_CAN_INTERFACE`.

## ANTI-PATTERNS

- Logic in `bins/` — move to `crates/`.
- `println!` for runtime logs in Chappe producers → `tracing`.
- Direct CAN access from bins → go through Davout via Berthier `ControlLoop`.
