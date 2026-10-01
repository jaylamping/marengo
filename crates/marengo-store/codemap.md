# crates/marengo-store/

## Responsibility
Time-series **key-value store** for telemetry replay and gateway log archival (SQLite-backed).

## Design
- `Store` struct: session-scoped writes, query API for marengo-log-cli and gateway
- Used by marengo-gateway for bench session persistence
- `recover_known_v2`: deliberate, bounded recovery of a recognized complete v2
  schema with a stale marker, preserving the source and a verified standalone backup

## Integration
- **Consumed by**: `bins/marengo-gateway`, `bins/marengo-log-cli`

**Detailed map**: [src/codemap.md](src/codemap.md)
