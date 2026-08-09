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

Gateway candump diagnostics (`CandumpFrame`, `CandumpPage`, `CandumpSummary`) grew enrichment and clearer counters. Protobuf and HTTP are related but not identical:

- **True one-release HTTP aliases:** `CandumpFrame.delta_s` (= `offset_s`), `CandumpPage.total_frames` (= `parsed_frames` clamped to u32), `CandumpSummary.frame_count` / `bytes` (= `parsed_frames` / `source_bytes`). Prefer the non-alias names in new clients.
- **HTTP shape change (not a silent shim):** gateway JSON keeps keys `interfaces`, `top_ids`, and `approx_hz`, but `interfaces`/`top_ids` are now object arrays (`{name,parsed_frames,approx_hz}` / `{can_id,count}`) and `approx_hz` is `number | null`. Proto’s parallel names (`interface_summaries`, `top_id_counts`, `parsed_frame_hz`) describe the richer model; gateway currently serves the enriched objects under the old keys.
- **New optional enrichment** on frames: `timestamp_unix_us`, `comm_type`, `comm_type_name`, `joint` (Robstride decode-only labels for operator forensics).
- **Timestamp mode:** proto `CandumpPage.timestamp_mode` is explicit (Delta or Absolute; no Auto). Gateway/store archive and hot paths currently always inspect as Delta; Absolute is used by CLI when requested. Page JSON may omit `timestamp_mode` until that field is plumbed through HTTP.

Log archive/HTTP surface for candump sessions is owned by [ADR 0011](0011-log-retention-and-archive.md); parsing/enrichment lives in `marengo-candump` (accepts both `candump -L` `ID#HEX` and default ASCII `ID [dlc] XX…` lines).

## Alternatives considered

- JSON / serde-only: simpler debugging, weaker cross-language contracts and evolution story.
- Cap'n Proto / FlatBuffers: stronger performance tradeoffs; smaller ecosystem for TS + prost.
