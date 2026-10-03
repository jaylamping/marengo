# bins/marengo-pi/src/

## Responsibility
Pi runtime implementation modules.

## Design
| Module | Role |
|--------|------|
| `main.rs` | Entry, REPL, control loop, Chappe bridge, command parsing |
| `reference_queue.rs` | One-at-a-time physical reference queue (stdin `home <joints> sign-tested`, Consul Set Zero), deferral of other stdin commands, cancel; stdout contract lines |
| `reference_queue_tests.rs` | Queue state machine against a scripted driver: refusals, ordering, failure skips, deferral, cancel, E-stop |
| `reference_dispatch_tests.rs` | `home` parsing and stdin dispatch/deferral/cancel against a plain (unsupported) owner |
| `enable_gate.rs` | `EnableGate`: stdin `enable` prints `waiting for enable to complete (operator=…)`, then `enabled (operator=…) targets=…` only once `ControlLoop::enable_completion` holds; `hold-on`/`hold-at`/`wave` arriving earlier are deferred and retried after each tick in order. Bounded by `berthier::ENABLE_COMPLETION_TIMEOUT` (2 s): an incomplete Enable prints `enable failed:` and stops every drive; a deferred arm prints `<cmd> failed:`. Disable cancels both; hold-off cancels deferred arms |
| `motion_owner.rs` | Single motion owner: `MotionLease` (process-lifetime claim `--motion-owner stdin\|chappe` / `MARENGO_MOTION_OWNER`, default chappe), `CommandClass` Stop/Observe/Motion, stdin classification, `motion_refused` / audit `ActionEvent` publishing. Stop is always admitted; Motion only from the owner |
| `motion_owner_tests.rs`, `motion_owner_chappe_tests.rs` | Lease matrix, non-owner refusal, stop from both sources, reference-queue gate for Testing batches, lagged `robot/enable` fail-closed stop, operator-disable latch, gains-after-mode ordering |
| `enable_gate_tests.rs` | Gate against a closed simulation owner: delayed `enabled`, deferred hold-on latching measured q, Enable and arm timeouts, Disable/hold-off cancel |
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
