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

Set `consul/.env.local` (dev machine only — **not** used by Pi deploy):

```
VITE_CHAPPE_HTTP_URL=http://127.0.0.1:8080
VITE_CHAPPE_WEBTRANSPORT_URL=https://127.0.0.1:8443/chappe
```

## Robot-hosted deploy

Pi deploy builds Consul with production env scrub ([ADR 0008](decisions/0008-chappe-webtransport-transport.md)):

1. `consul/.env.production` — empty `VITE_CHAPPE_*` anchors override `.env.local`
2. `deploy-pi.sh` — `env -u VITE_CHAPPE_* npm run build` + always rebuild (no stale-dist skip)
3. `scripts/check-consul-dist.sh` — rejects dist containing `127.0.0.1:8080` or `VITE_CHAPPE_` literals

**Canonical redeploy:** `pi_sync_main` (MCP) or `./scripts/deploy-pi.sh --install joey@marengo.local` after merge. Verify with `pi_health`: `.deploy-rev` first field matches local `git rev-parse HEAD`.

## Connection failures (CHAPPE ERR)

Hover the **CHAPPE ERR** badge for resolved HTTP and WebTransport URLs plus a triage hint.

| Symptom | Badge tooltip | Likely cause | Fix |
|---------|---------------|--------------|-----|
| CHAPPE ERR on `https://marengo.local:8444` | URLs show `127.0.0.1` + "Baked dev URLs" | Pre-fix bundle or deploy built with dev `.env.local` baked in | `pi_sync_main` after deploy-pipeline merge; confirm `check-consul-dist` passes locally |
| CHAPPE ERR, derived URLs, "Gateway unreachable" | HTTP/WT match page origin | `marengo-gateway` down or blocked | `systemctl status marengo-gateway`; accept TLS cert on `:8444` once |
| **WIREFRAME** (not CHAPPE ERR) | — | Page loaded over HTTP (no live endpoints) | Open `https://…:8444` on Pi LAN or set `.env.local` for local dev |

**HTTP stream fallback** works over TCP/`ssh -L` to `:8444`; **WebTransport requires LAN UDP** to `:8443`.

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

Host cards show `build.deploy_rev` from **`/opt/marengo/.deploy-rev`** (single writer: `install-pi.sh` copies staged `{local HEAD} {UTC}\n` from the deploy bundle). `pi_sync_main` and `deploy-pi.sh --install` both use this path — not `~/marengo/.deploy-rev` or on-Pi `git rev-parse`.
