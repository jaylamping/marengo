# bins/marengo-gateway/src/

## Responsibility
HTTP, WebTransport, framing, and gateway state modules.

## Design
| Module | Role |
|--------|------|
| `main.rs` | Server startup, IpcListener spawn, TLS config |
| `http.rs` | Axum routes: health, snapshots, commands, static files |
| `webtransport.rs` | WT session handler, envelope stream |
| `state.rs` | Latest telemetry cache for HTTP handlers |
| `framing.rs` | Length-prefixed message codec |
| `logs.rs` | Log service integration with Store |
