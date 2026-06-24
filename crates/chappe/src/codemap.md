# crates/chappe/src/

## Responsibility
Bus implementation, IPC transport, and tracing integration.

## Design
| Module | Role |
|--------|------|
| `lib.rs` | `Bus`, `BusError`, publish/subscribe API |
| `transport.rs` | `SharedBus`, `Transport` trait for IPC bridging |
| `ipc.rs` | Unix socket listener/client, length-prefixed framing |
| `tracing_layer.rs` | Optional trace event publishing |

## Flow
`Bus::publish` → hash topic → lookup/create `broadcast::Sender` → send bytes
`IpcListener::accept` → spawn client handler → subscribe to SharedBus topics → write framed envelopes

## Integration
- Used by marengo-pi (in-process + IPC publish) and marengo-gateway (IPC listen + HTTP/WT fan-out)
