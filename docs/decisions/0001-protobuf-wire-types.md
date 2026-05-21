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

## Alternatives considered

- JSON / serde-only: simpler debugging, weaker cross-language contracts and evolution story.
- Cap'n Proto / FlatBuffers: stronger performance tradeoffs; smaller ecosystem for TS + prost.
