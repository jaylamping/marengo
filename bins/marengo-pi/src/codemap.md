# bins/marengo-pi/src/

## Responsibility
Pi runtime implementation modules.

## Design
| Module | Role |
|--------|------|
| `main.rs` | Entry, REPL, control loop, Chappe bridge, command parsing |
| `host_metrics.rs` | Periodic host metric publish |
| `imu.rs` | Optional BNO085 read loop (linux-i2c feature) |

## Flow
See parent [codemap.md](../codemap.md) — `run_control_loop` and `handle_command` are the core paths.
