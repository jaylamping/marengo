# proto/

## Responsibility
**Protobuf wire schema** root — defines Chappe message types shared across Rust (armee-proto) and TypeScript (Consul buf codegen). Proto-first API changes (ADR 0001).

## Design
- `marengo/v1/marengo.proto`: all v1 messages under `package marengo.v1`
- Versioned namespace allows future v2 without breaking consumers
- Envelope pattern: `Envelope { topic, timestamp_ms, payload }` wraps typed sub-messages

## Flow
1. Edit `.proto` → rebuild armee-proto + regenerate Consul TS types
2. Producers encode sub-message → wrap in Envelope → Chappe publish
3. Consumers decode Envelope → dispatch by topic

## Integration
- **Rust**: `crates/armee-proto` (prost build.rs)
- **TypeScript**: `consul/src/gen/` (buf generate)
- **Topics defined here**: RobotState, SafetyState, EnableRequest, MitCommandBatch, HostMetrics, LogEvent, GatewaySubscribe

**Detailed map**: [marengo/v1/codemap.md](marengo/v1/codemap.md)
