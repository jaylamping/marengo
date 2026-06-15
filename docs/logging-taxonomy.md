# Logging taxonomy

Operator and agent reference for Marengo telemetry tiers, rate limits, and side channels.

## Pipeline tiers

| Tier | Storage | Purpose |
|------|---------|---------|
| **Live** | Consul browser ring (5k) | Operator UI while `/logs` is open |
| **Hot** | Gateway ring (10k) + SQLite `log_events` | Backfill on connect; FTS search |
| **Filesystem** | `var/log/bench-*.log`, `candump-*.log`, `position-trace-*.csv` | High-volume bench artifacts |
| **Archived** | `var/log/blobs/*.gz` + `log_sessions` | Cross-session browse |
| **Journal** | systemd → `journal-import` → `log_events` (`target` prefix `systemd:`) | Unit logs |

See [chappe-consul-ingestion.md](chappe-consul-ingestion.md) and [ADR 0011](decisions/0011-log-retention-and-archive.md).

## Event severity tiers (S0–S3)

| Tier | Default `RUST_LOG` | Consul path | Examples |
|------|-------------------|-------------|----------|
| **S0 Safety** | Always on | Always forwarded (bypasses rate cap) | E-stop, Davout limit trips, disable failures, control tick `error!` |
| **S1 Operational** | `info` | Forwarded when under cap | enable/disable, mode transitions, IPC loss, homing result |
| **S2 Diagnostic** | `debug` (opt-in) | Dropped at default caps | position onset burst, velocity spike corroboration |
| **S3 High-rate** | `trace` or side channel | Never at `info` | robstride CAN trace, berthier 1 Hz position diag, `MARENGO_POSITION_TRACE` CSV |

**Rule:** Libraries use `warn!` + `Err` for recoverable faults; bins use `error!` for unrecoverable control-path failures.

## Rate-limit funnel

```
Rust ChappeLogLayer: 40 events/s (warn/error always pass)
        ↓
Gateway ring + SQLite batch (100 events / 500 ms)
        ↓
Consul decode: 12/s; ingest: 10/s (warn/error/fatal always pass)
```

S0 events are never dropped by design. S2/S3 are intentionally throttled before they reach the browser.

## Initialization paths

| Bin type | Init | Publishes to Consul |
|----------|------|---------------------|
| Chappe producers (`marengo-pi`, `marengo-gateway`) | `chappe::tracing_layer::init_subscriber` | Yes (`logs/structured`) |
| Scaffolds / CLI (`motor-repl`, `marengo-log-cli`, probes) | `marengo_support::init_tracing` | No (stdout/journal only) |

## REPL vs tracing (hybrid)

| Event | stdio | tracing |
|-------|-------|---------|
| Command responses (`status`, `hold-at`, `help`) | Yes | No |
| Startup errors before subscriber init | `eprintln!` | No |
| Enable/disable/homing (Chappe) | Optional | Yes |
| Control tick failure, IPC loss, safety | No | Yes |

## Side channels (not tracing at control-loop rate)

| Tool | When |
|------|------|
| `robstride=trace` | CAN bring-up only |
| `MARENGO_POSITION_TRACE` | Position-hold bench CSV |
| candump files | Wire truth vs code trace |
| `journal-import` | Nightly systemd history |

## Structured fields

`LogEvent.fields_json` carries tracing structured fields to Consul and SQLite ([ADR 0013](decisions/0013-structured-log-fields.md)). Max 2 KB per event.

## Live log-level control

Runtime `RUST_LOG` changes from Consul are **not** implemented yet (roadmap M5). Until then, edit `/etc/marengo/env` or use per-crate overrides documented in [troubleshooting.md](troubleshooting.md).

## Inventory

Run `./scripts/log-inventory.sh` (or `LOG_INVENTORY_FORMAT=json`) for per-file macro counts and init-path coverage.
