# crates/marengo-store/

## Responsibility
Time-series **key-value store** for telemetry replay and gateway log archival (SQLite-backed).

## Design
- `Store` struct: session-scoped writes, query API for marengo-log-cli and gateway
- Used by marengo-gateway for bench session persistence

## Integration
- **Consumed by**: `bins/marengo-gateway`, `bins/marengo-log-cli`

**Detailed map**: [src/codemap.md](src/codemap.md)
