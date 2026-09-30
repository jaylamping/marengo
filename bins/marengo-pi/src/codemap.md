# bins/marengo-pi/src/

## Responsibility
Pi runtime implementation modules.

## Design
| Module | Role |
|--------|------|
| `main.rs` | Entry, REPL, control loop, Chappe bridge, command parsing |
| `overlay.rs` | Shutdown-aware actuator tuning dispatch; live changes and asynchronous persistence admission |
| `limit_persist.rs` | Closed admission, serialized actual writes/publication, bounded typed drain and observed worker termination |
| `limit_persist_tests.rs` | Gated real writes and matching completion events for retained drafts, publication lifetime and coalescing |
| `limit_persist_qualification_tests.rs` | New drain API conformance for timeout, closed admission and real isolated worker failure |
| `shutdown_tests.rs` | Installed owner lifecycle with addressed transport witnesses and gated real writer |
| `host_metrics.rs` | Periodic host metric publish |
| `imu.rs` | Optional BNO085 read loop (linux-i2c feature) |

## Flow
See parent [codemap.md](../codemap.md) — `run_control_loop` and `handle_command` are the core paths.

`finish_owner_shutdown` inhibits controller intent and retains the configured
Davout stop result/report before waiting for persistence. Its outcomes keep
skipped/failed stop separate from storage completion and physical acceptance.
`run_control_loop` exits immediately on Quit and checks observed shutdown at
each later dispatch/tick boundary. These checks cannot interrupt an already
admitted synchronous operation and do not establish command-flood priority.
