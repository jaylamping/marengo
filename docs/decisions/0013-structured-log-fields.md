# ADR 0013: Structured log fields on LogEvent

**Status:** Accepted  
**Date:** 2026-06-15

## Context

[`ChappeLogLayer`](../crates/chappe/src/tracing_layer.rs) forwarded only the tracing message string to Consul. Rich structured fields (`joint`, `error`, `operator_id`, position-hold diagnostics) were visible in journal/stdout but lost on the `logs/structured` wire path and in SQLite FTS search.

## Decision

1. Add optional `fields_json` to `LogEvent` (field 6) and `StructuredLogEntry` (field 7) in [`proto/marengo/v1/marengo.proto`](../proto/marengo/v1/marengo.proto).
2. `ChappeLogLayer` collects all non-`message` tracing fields into a JSON object; omit the proto field when empty (backward compatible).
3. Cap serialized `fields_json` at **2048 bytes**; set `"_truncated": true` when truncated.
4. Extend SQLite `log_events.fields_json` and FTS index (schema migration v2 in `marengo-store`).
5. **Live `RUST_LOG` reload from Consul** remains out of scope (roadmap M5); operators use `/etc/marengo/env` until then.

## Consequences

- Proto change requires regenerating Rust and TypeScript (`npm run gen:proto`).
- Existing Consul/gateway builds ignore unknown field 6/7 until upgraded (protobuf compatible).
- Search UI can filter on `joint`, `error`, and other keys without cramming them into `message`.

## References

- [logging-taxonomy.md](../logging-taxonomy.md)
- [ADR 0011](0011-log-retention-and-archive.md)
- [ADR 0008](0008-chappe-webtransport-transport.md)
