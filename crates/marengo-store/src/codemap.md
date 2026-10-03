# crates/marengo-store/src/

## Responsibility
SQLite schema, insert/query operations for telemetry samples and session metadata.

`store.rs` exposes Store operations and owns the connection mutex. `migrations.rs`
owns version inspection and each Immediate transaction containing one schema
transition and its matching marker (ADR0029). Its helpers use that connection
directly; they must not reacquire the Store mutex through public methods.

`recovery.rs` owns the explicit ADR0030 known-v2 operation. It recognizes the full
trusted schema under a read-only snapshot, preserves committed WAL content through
bounded SQLite backup, verifies a standalone backup before repairing a separate
output, and invokes the unchanged normal migration owner. Its private staging
directories have explicit identity-checked cleanup and no-overwrite publication.

`retention.rs` enforces the persisted `log_archive_days` and `log_disk_budget_bytes`
settings (`Store::enforce_retention`, run by `marengo-log-cli purge`): age purge, then
oldest-first session eviction until `log_disk_usage_bytes` fits the budget. The newest
session is never evicted by the budget pass.
