# crates/marengo-host-metrics/

## Responsibility
Host-level **CPU, memory, disk** metrics collection for Consul health dashboard.

## Design
- Platform-specific readers (Linux `/proc`, etc.)
- Published via Chappe or gateway HTTP for operator visibility
- Deploy revision comes from `marengo-deploy` (`read_deploy_rev`) and log disk usage from
  `marengo-store` (`log_disk_usage_bytes`); this crate keeps no copy of either parser/walker

## Integration
- **Consumed by**: `bins/marengo-pi` (`host_metrics` module), gateway state API

**Detailed map**: [src/codemap.md](src/codemap.md)
