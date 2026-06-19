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
- **Robot-hosted Consul:** `https://marengo.local:8444` serves the built SPA from `/opt/marengo/www`; endpoints are derived at runtime from the page origin (no baked `VITE_CHAPPE_*` in the Pi bundle).
- **Local dev:** `VITE_CHAPPE_HTTP_URL=http://marengo.local:8080`, `VITE_CHAPPE_WEBTRANSPORT_URL=https://marengo.local:8443/chappe` in `consul/.env.local` only — never copied to the Pi. **`ssh -L` does not forward QUIC**; do not tunnel `:8443` for WebTransport.

### Production Consul build (Pi deploy)

Robot-hosted dist MUST NOT embed dev `VITE_CHAPPE_*` URLs from a developer machine's `consul/.env.local`.

| Layer | Mechanism |
|-------|-----------|
| Vite mode | `consul/.env.production` sets empty `VITE_CHAPPE_HTTP_URL=` / `VITE_CHAPPE_WEBTRANSPORT_URL=` anchors so production mode overrides `.env.local` |
| Shell scrub | `deploy-pi.sh` runs `env -u VITE_CHAPPE_HTTP_URL -u VITE_CHAPPE_WEBTRANSPORT_URL npm run build` (shell `VITE_*` outrank all `.env*`) |
| Backstop | `scripts/check-consul-dist.sh` fails deploy and CI if any `consul/dist/*.js` contains `127.0.0.1:8080` or the literal `VITE_CHAPPE_` |

Deploy always rebuilds Consul (`build_consul_assets` does not skip on stale `dist/`). After deploy fixes merge, run **`pi_sync_main`** (or `./scripts/deploy-pi.sh --install`) to replace a poisoned Pi bundle.

Canonical deploy revision: **`/opt/marengo/.deploy-rev`** — `{local git rev-parse HEAD} {UTC ISO8601}\n`, written by `install-pi.sh` from the staged bundle (not from on-Pi git state).
- TLS: self-signed cert generated at startup (`rcgen`); accept the browser warning once on `:8444` (secure origin for WebTransport cert pinning).
- Future: bearer token or mTLS; optional `127.0.0.1`-only bind for localhost-only installs.

### Relation to NATS

Gateway is the **reference WebTransport transport** for Chappe. A future `--transport nats` mode in the same binary (or sidecar) will publish identical `Envelope` bytes so Consul and Jetson do not fork schemas.

## Consequences

- New workspace member `bins/marengo-gateway`; `chappe` gains `transport` + `ipc` modules.
- `marengo-pi` sets `MARENGO_CHAPPE_SOCKET` when gateway runs on Pi (`systemd` / env).
- Consul: robot-hosted HTTPS derives endpoints; local dev uses `VITE_CHAPPE_*` in `.env.local`; mock data when neither applies. Deploy pipeline scrubs baked env; CHAPPE ERR tooltip distinguishes baked misconfig vs gateway down (see [chappe-consul-ingestion.md](../chappe-consul-ingestion.md)).
- CI: gateway + ipc tested without hardware; WebTransport requires TLS stack (quinn/rustls).

## Alternatives considered

- **GraphQL** — second schema; realtime still needs streams.
- **WebSocket fallback** — rejected for phase 1 per operator preference.
- **Express/Node gateway** — no native WebTransport on Node HTTP stack; would split proto tooling.
- **SSH-only relay** — insufficient for sub-100 ms UI loops at 50 Hz.
