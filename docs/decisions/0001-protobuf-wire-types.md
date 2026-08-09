# ADR 0001: Protocol Buffers for wire types

**Status:** Accepted  
**Date:** 2025-05-19

## Context

Marengo runs multiple processes (Pi, Jetson, Consul, future Python tooling) that must exchange typed data over [Chappe](../../crates/chappe/) without schema drift between languages.

## Decision

Use **Protocol Buffers** as the source of truth for all inter-service wire types.

- Schemas live in [`proto/`](../../proto/) — never hand-edit generated code.
- [`armee-proto`](../../crates/armee-proto/) is a thin **prost-build** wrapper for Rust consumers.
- [Consul](../../consul/) generates TypeScript via **Buf** (`@bufbuild/protobuf`) into `consul/src/gen/` (gitignored).
- Chappe carries **binary protobuf** payloads (`prost::Message::encode_to_vec` / `decode`). JSON is not the wire format.
- Future NATS/MQTT clients treat payloads as `Vec<u8>` and serialize through the same `.proto` definitions.

## Rationale

- Single language-agnostic schema for Rust, TypeScript, and future Python (Isaac Lab, analysis).
- Field numbering and optional semantics support safe schema evolution.
- Compact on the wire vs JSON; common pattern for robotics IPC.

## Consequences

- **Tooling:** `protoc` required for Rust builds; `buf` CLI required for Consul codegen.
- **CI:** Install both; run `cargo build` and `consul` `gen:proto` in pipeline.
- **Workflow:** API changes start in `proto/marengo/v1/*.proto`, then regenerate Rust/TS.

### Candump HTTP/proto evolution (2026-07)

Gateway candump diagnostics (`CandumpFrame`, `CandumpPage`, `CandumpSummary`) grew enrichment and clearer counters while keeping wire compatibility for older Consul clients:

- **Deprecated projections** (still populated where useful): `CandumpFrame.delta_s` → prefer `offset_s`; `CandumpPage.total_frames` → prefer `parsed_frames` / `total_lines`; `CandumpSummary` fields 1–2 and 4–6 (`frame_count`, `bytes`, `approx_hz`, `interfaces`, `top_ids`) → prefer fields 7–12 (`parsed_frames`, `total_lines`, `top_id_counts`, `interface_summaries`, `parsed_frame_hz`, `source_bytes`).
- **New optional enrichment** on `CandumpFrame`: `timestamp_unix_us`, `comm_type`, `comm_type_name`, `joint` (Robstride decode-only labels for operator forensics).
- **Explicit timestamp mode** via `CandumpTimestampMode` on `CandumpPage` (Delta or Absolute; no Auto).

Consumers must treat deprecated fields as compatibility shims and prefer the non-deprecated names. Log archive/HTTP surface for candump sessions is owned by [ADR 0011](0011-log-retention-and-archive.md); parsing/enrichment lives in `marengo-candump`.

## Alternatives considered

- JSON / serde-only: simpler debugging, weaker cross-language contracts and evolution story.
- Cap'n Proto / FlatBuffers: stronger performance tradeoffs; smaller ecosystem for TS + prost.
