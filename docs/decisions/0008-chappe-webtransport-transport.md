# ADR 0008: Chappe operator gateway (HTTP CRUD + WebTransport)

**Status:** Accepted  
**Date:** 2026-05-26

## Context

`marengo-pi` publishes `RobotState`, `SafetyState`, and `Heartbeat` on in-process Chappe topics. Consul needs live telemetry and command surfaces without SSH paste workflows. Roadmap M7 called for a transport ADR before multi-process runtime; bench bring-up needs live UI within weeks.

## Decision

Add **`marengo-gateway`**, a thin Rust operator service that:

1. **HTTP/1.1 CRUD** (default port `8080`) — health, protobuf snapshots, command posts.
2. **WebTransport over HTTP/3 + TLS** (default port `8443`) — bidirectional streams carrying binary `Envelope` protobuf (ADR 0001). Primary realtime transport.
3. **HTTP long-lived stream fallback** — `GET /stream/chappe?topics=…` on `:8080` / `:8444`; same length-prefixed `Envelope` bytes as WebTransport; TCP-tunnelable when QUIC is blocked.
4. **Unix socket IPC** (`MARENGO_CHAPPE_SOCKET`, default `/run/marengo/chappe.sock`) — `marengo-pi` forwards publishes; gateway ingests and fans out to HTTP cache + WebTransport/HTTP-stream subscribers. Commands from gateway → runtime use the reverse direction on the same socket.

Wire types remain in [`proto/marengo/v1/marengo.proto`](../../proto/marengo/v1/marengo.proto). Consul decodes with `@bufbuild/protobuf` from `consul/src/gen/`.

### HTTP contract (phase 1)

| Method | Path | Body | Response |
|--------|------|------|----------|
| GET | `/health` | — | JSON `{ "ok": true, "node": "marengo-gateway" }` |
| GET | `/snapshot/robot/state` | — | `application/x-protobuf` `RobotState` |
| GET | `/snapshot/robot/safety` | — | `application/x-protobuf` `SafetyState` |
| GET | `/snapshot/robot/heartbeat` | — | `application/x-protobuf` `Heartbeat` |
| GET | `/snapshot/sensors/imu/torso` | — | `application/x-protobuf` `ImuSample` |
| GET | `/snapshot/host/metrics/pi` | — | `application/x-protobuf` `HostMetrics` |
| GET | `/snapshot/host/metrics/jetson` | — | `application/x-protobuf` `HostMetrics` |
| GET | `/stream/chappe?topics=…` | — | `application/vnd.marengo.chappe-stream` chunked Envelope stream |
| POST | `/command/enable` | `application/x-protobuf` `EnableRequest` | JSON `{ "ok": true }` |
| POST | `/command/disable` | empty (no credential; Origin checked) | JSON `{ "ok": true }` |

### WebTransport contract (phase 1)

- URL path: `/chappe` (session accepted for any path under gateway in dev).
- First client→server bidi message: length-prefixed `GatewaySubscribe` protobuf (topic list).
- Server→client: repeated length-prefixed `Envelope` bytes for matching topics.
- Allowed subscribe topics: `robot/state`, `robot/safety`, `robot/heartbeat`, `sensors/imu/torso`, `logs/structured`, `host/metrics/pi`, `host/metrics/jetson`.

### HTTP stream fallback

- Same topic allowlist as WebTransport.
- Response body: repeated 4-byte little-endian length + `Envelope` protobuf (identical framing to WebTransport server→client).
- Use when WebTransport/QUIC unavailable (SSH tunnel to `:8444`, blocked UDP, browsers without `WebTransport`).

### Auth (bench)

- Phase 1: bind **`[::]:8080`** (HTTP API), **`[::]:8444`** (HTTPS Consul UI + API), and **`[::]:8443`** (WebTransport/QUIC) on the Pi LAN (dual-stack; `marengo.local` is often IPv6-first on macOS). Bench-only — not intended for the public internet.
- **Robot-hosted Consul:** `https://marengo.local:8444` serves the built SPA from `/opt/marengo/www`; endpoints are derived at runtime (no `VITE_CHAPPE_*` on the robot).
- **Local dev:** `VITE_CHAPPE_HTTP_URL=http://marengo.local:8080`, `VITE_CHAPPE_WEBTRANSPORT_URL=https://marengo.local:8443/chappe` in `consul/.env.local`. **`ssh -L` does not forward QUIC**; do not tunnel `:8443` for WebTransport.
- TLS: self-signed cert generated at startup (`rcgen`); accept the browser warning once on `:8444` (secure origin for WebTransport cert pinning).
- Future: bearer token or mTLS; optional `127.0.0.1`-only bind for localhost-only installs.

### Relation to NATS

Gateway is the **reference WebTransport transport** for Chappe. A future `--transport nats` mode in the same binary (or sidecar) will publish identical `Envelope` bytes so Consul and Jetson do not fork schemas.

## Current state (2026-10-28)

- `/snapshot/host/metrics/jetson` and the `host/metrics/jetson` topic were removed with the Jetson scaffolds in the 2026-10-03 crate audit ([ADR 0014](0014-jetson-perception-semantic-motion.md)). `chappe::transport` was removed too; NATS or a Pi↔Jetson bridge would be new work.
- Topic strings now live in one place, `crates/chappe/src/topics.rs`. The gateway's subscribe allowlist (`ALLOWED_TOPICS` in `bins/marengo-gateway/src/state.rs`) adds the runtime-connection, testing-telemetry, `robot/actuator/limits` and `robot/audit/*` topics to the list above. Gateway → runtime commands are limited to the `COMMAND_TOPICS` allowlist and refused when more than 1000 ms old.
- The gateway is the Chappe IPC **listener** on `/run/marengo/chappe.sock`. `marengo-pi` connects as a client only when `MARENGO_CHAPPE_SOCKET` is set.
- Auth is no longer bench-open: [ADR 0033](0033-gateway-runtime-access.md) defines the shared gateway access policy and runtime credentials.

## Consequences

- New workspace member `bins/marengo-gateway`; `chappe` gains `transport` + `ipc` modules.
- `marengo-pi` sets `MARENGO_CHAPPE_SOCKET` when gateway runs on Pi (`systemd` / env).
- Consul: robot-hosted HTTPS derives endpoints; local dev uses `VITE_CHAPPE_*`; mock data when neither applies.
- CI: gateway + ipc tested without hardware; WebTransport requires TLS stack (quinn/rustls).

## Alternatives considered

- **GraphQL** — second schema; realtime still needs streams.
- **WebSocket fallback** — rejected for phase 1 per operator preference.
- **Express/Node gateway** — no native WebTransport on Node HTTP stack; would split proto tooling.
- **SSH-only relay** — insufficient for sub-100 ms UI loops at 50 Hz.
