# crates/marengo-host-metrics/

## Responsibility
Host-level **CPU, memory, thermal, disk, network, service and clock** metrics for the Consul host card (`HostMetrics` on `host/metrics/pi`).

## Design
- Linux collectors (`/proc`, `/sys`, bounded `df`/`ip`/`systemctl`/`timedatectl`/`vcgencmd` with a 2 s timeout each). An unreadable source is published as unknown (`meminfo_known`, `cpu_known`, `throttle_known`, `rates_valid`, `capacity_known`, `mount_status_known`, `UNKNOWN` CAN state; failed service queries are omitted), never as a healthy zero.
- Disks are reported per configured mount point; an unmounted `/boot/firmware` is unknown, not the root filesystem.
- `throttled_now` covers under-voltage, throttling and the soft temperature limit (vcgencmd bits 0, 2, 3).
- Non-Linux builds publish a stub marked `simulated = true`.
- Deploy revision comes from `marengo-deploy` (`read_deploy_rev`) and log disk usage from
  `marengo-store` (`log_disk_usage_bytes`); this crate keeps no copy of either parser/walker

## Integration
- **Consumed by**: `bins/marengo-pi` (`host_metrics` module: Chappe queue counters and the measured gateway probe latency), gateway state API

**Detailed map**: [src/codemap.md](src/codemap.md)
