# crates/marengo-store/src/

## Responsibility
SQLite schema, insert/query operations for telemetry samples and session metadata.

`store.rs` exposes Store operations and owns the connection mutex. `migrations.rs`
owns version inspection and each Immediate transaction containing one schema
transition and its matching marker (ADR0029). Its helpers use that connection
directly; they must not reacquire the Store mutex through public methods.
