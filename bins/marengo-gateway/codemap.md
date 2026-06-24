# bins/marengo-gateway/

## Responsibility
**Operator gateway** — bridges Chappe Unix-socket IPC to HTTP REST and WebTransport streams for the Consul frontend.

## Design
- **Adapter** pattern: `IpcListener` (Chappe) → in-memory `Bus` → HTTP/WebTransport fan-out
- Axum HTTP server: `/health`, snapshot CRUD, enable/disable POST, static Consul assets
- WebTransport on `:8443` for low-latency telemetry; HTTP stream fallback
- `marengo-store::Store` for session log archival
- TLS optional via rustls cert/key paths

## Flow
1. `main` → parse listen addrs, socket path, web root
2. `IpcListener::bind(socket_path)` → subscribe to Chappe topics from marengo-pi
3. HTTP handlers serve latest RobotState, SafetyState, host metrics
4. WebTransport streams length-prefixed Envelope bytes to Consul clients

## Integration
- **Depends on**: chappe, marengo-store, armee-proto
- **Consumed by**: Consul `chappe-client.ts` (`connectChappeStream`, `fetchGatewayHealth`)
- Default: HTTP `127.0.0.1:8080`, WT `127.0.0.1:8443`

**Detailed map**: [src/codemap.md](src/codemap.md)
