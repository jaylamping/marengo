# bins/marengo-gateway/src/

## Responsibility
HTTP, WebTransport, framing, and gateway state modules.

## Design
| Module | Role |
|--------|------|
| `main.rs` | Server startup, IpcListener spawn, TLS config |
| `http.rs` | Axum routes: health, snapshots, config, commands, static files |
| `config.rs` | `GET /config/snapshot`, `POST /config/patch` (Consul PID/limits UI) |
| `restart.rs` | `POST /control/restart-marengo-pi` (canonical `pi-restart-marengo-pi.sh`) |
| `webtransport.rs` | WT session handler, envelope stream |
| `state.rs` | Latest telemetry cache for HTTP handlers |
| `framing.rs` | Length-prefixed message codec |
| `logs.rs` | Log service integration with Store |
