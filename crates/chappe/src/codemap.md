# crates/chappe/src/

## Responsibility
Bus implementation, IPC transport, and tracing integration.

## Design
| Module | Role |
|--------|------|
| `lib.rs` | `Bus`, `BusError`, publish/subscribe API |
| `ipc.rs` | `IpcFanout` (runtime client), `IpcListener` (gateway server), length-prefixed framing, command allowlist and freshness |
| `ipc_outbox.rs` | Bounded runtime→gateway outbox: latest-value slots, event FIFO, counters |
| `topics.rs` | Topic constants and `COMMAND_TOPICS` |
| `tracing_layer.rs` | `ChappeLogLayer` (`LogEvent` on `logs/structured`) and `init_subscriber` |

## Flow
`Bus::publish` → hash topic → lookup/create `broadcast::Sender` → send bytes
Runtime: `Bus::publish_bytes` → `IpcFanout` outbox → client thread writes framed envelopes; its reader thread admits gateway commands onto the `Bus`.
Gateway: `IpcListener` accepts one runtime peer, hands each frame to its callback, and writes commands back.

## Integration
- Used by marengo-pi (in-process + IPC publish) and marengo-gateway (IPC listen + HTTP/WT fan-out)
