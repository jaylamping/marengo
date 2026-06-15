# Chappe → Consul ingestion

Operator path for live telemetry, structured logs, and host metrics in Consul.

## Transports

| Priority | Transport | URL | Notes |
|----------|-----------|-----|-------|
| 1 | WebTransport (QUIC) | `https://<host>:8443/chappe` | Primary; cert pin via `GET /tls/fingerprint` |
| 2 | HTTP stream | `GET /stream/chappe?topics=…` on `:8080` or `:8444` | Same length-prefixed `Envelope` framing; works over TCP / SSH tunnel |

Consul client: [`consul/src/lib/chappe-client.ts`](../consul/src/lib/chappe-client.ts) — tries WebTransport, falls back to HTTP stream.

## Topics (phase 1)

| Topic | Message | Rate |
|-------|---------|------|
| `robot/state` | `RobotState` | ~50 Hz |
| `robot/safety` | `SafetyState` | control `chappe_state_hz` |
| `robot/heartbeat` | `Heartbeat` | 1 Hz |
| `sensors/imu/torso` | `ImuSample` | IMU config |
| `logs/structured` | `LogEvent` | tracing layer (rate-limited) |
| `host/metrics/pi` | `HostMetrics` | 1 Hz |
| `host/metrics/jetson` | `HostMetrics` | 1 Hz |

Wire format: [ADR 0001](decisions/0001-protobuf-wire-types.md), gateway contract [ADR 0008](decisions/0008-chappe-webtransport-transport.md).

## Producers

- **Robot telemetry** — `marengo-pi` + Berthier → Chappe IPC → `marengo-gateway`
- **Logs** — `chappe::tracing_layer::ChappeLogLayer` on `marengo-pi` (and optional gateway)
- **Host metrics** — `marengo-host-metrics` crate, 1 Hz task in `marengo-pi`

## Local dev

```bash
# Terminal 1 — gateway demo (no Pi)
cargo run -p marengo-gateway -- --demo --http-listen 127.0.0.1:8080 --wt-listen 127.0.0.1:8443

# Terminal 2 — Consul
cd consul && npm run dev
```

Set `consul/.env.local`:

```
VITE_CHAPPE_HTTP_URL=http://127.0.0.1:8080
VITE_CHAPPE_WEBTRANSPORT_URL=https://127.0.0.1:8443/chappe
```

## SSH tunnel note

Tunnel HTTPS `:8444` for the SPA and HTTP stream fallback. **Do not** tunnel `:8443` — QUIC does not forward over `ssh -L`.

## Log volume

Default `RUST_LOG` should stay at `info` on bench. The tracing layer caps **40 events/sec** on the Chappe publish path; when over cap, `trace`/`debug`/`info` are dropped but `warn`/`error` always forward. Consul further throttles UI ingest to **10 events/sec** (decode cap 12/s) with the same warn/error bypass.

See [logging-taxonomy.md](logging-taxonomy.md) for the full funnel and S0–S3 tiers.

## Log persistence and archive ([ADR 0011](decisions/0011-log-retention-and-archive.md))

| Path | Role |
|------|------|
| WebTransport `logs/structured` | Live stream (primary) |
| `GET /snapshot/logs/recent` | HTTP backfill on Consul connect |
| `GET /logs/sessions`, `/logs/structured`, `/logs/sessions/:id/*` | Archive browse, bench/candump/trace blobs |
| `var/marengo.db` | Structured logs, session metadata, candump frame index |
| `var/log/blobs/` | gzip bench/candump/trace archives |

Optional `MARENGO_GATEWAY_LOG_TOKEN` / `VITE_MARENGO_LOG_TOKEN` for log HTTP routes on LAN.

## Deploy identity

Host cards show `build.deploy_rev` from `$MARENGO_ROOT/.deploy-rev` (written by `deploy-pi.sh` / `pi_sync_main`).
