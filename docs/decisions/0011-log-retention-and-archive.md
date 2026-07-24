# ADR 0011: SQLite hybrid log retention and gateway log API

**Status:** Accepted  
**Date:** 2026-06-15

## Context

Pi logging was split across bench files, candump, systemd journal, and ephemeral Consul browser buffer. Operators lost history on page refresh and had no cross-session search.

## Decision

1. **`marengo-store`** — SQLite at `$MARENGO_ROOT/var/marengo.db` (WAL, batched writes).
2. **Hybrid blobs** — gzip under `var/log/blobs/` for bench/candump/position-trace; session metadata and frame counts in SQL; candump parsing via `marengo-candump` (no frame-index table).
3. **`marengo-log-cli`** — shell/MCP archive entrypoint (no bash sqlite3).
4. **`marengo-gateway`** — log ring + SQL writer; HTTP log API; live stream stays **WebTransport** `logs/structured` ([ADR 0008](0008-chappe-webtransport-transport.md)).
5. **Retention** — 50 hot session files; 30-day purge (`MARENGO_LOG_ARCHIVE_DAYS`); optional `MARENGO_GATEWAY_LOG_TOKEN` for log HTTP routes.
6. **Journal import** — nightly `marengo-log-cli journal-import` → `log_events` with `target` prefix `systemd:`.

## HTTP endpoints (gateway)

- `GET /snapshot/logs/recent` — backfill on Consul connect
- `GET /logs/sessions`, `/logs/structured`, `/logs/sessions/:id/{bench,candump,trace}`, `/logs/sessions/latest/candump`

## Consequences

- Deploy must not delete `var/marengo.db` or blobs.
- Gateway owns primary SQL writer; CLI uses same DB for session archive.
