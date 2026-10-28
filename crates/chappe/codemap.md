# crates/chappe/

## Responsibility
Inter-process **pub/sub message bus** for protobuf `Envelope` bytes between Pi runtime, Consul UI, and tools. Transport only — no motor control or safety logic.

## Design
- **Observer / Pub-Sub**: `Bus::publish(topic, source, type, &msg)` wraps a prost message in an `Envelope`; `Bus::publish_bytes(topic, bytes)` sends encoded envelopes; `Bus::subscribe(topic)` returns a `broadcast::Receiver` of envelope bytes.
- **Transport**: in-process `Bus`, plus a Unix-domain socket between processes. The runtime side is `IpcFanout::spawn_client` (bounded outbox: latest-value slots for state, a FIFO for events; reads allowlisted, fresh gateway commands back onto the `Bus`); the gateway side is `IpcListener`.
- **Tracing layer**: `tracing_layer::ChappeLogLayer` publishes `tracing` events as `LogEvent` on `logs/structured` (40/s, plus 10/s for warn/error); `init_subscriber` installs it with the fmt layer and an INFO-default `EnvFilter`.
- Default channel capacity: 256 messages per topic.

## Flow
1. Producer (marengo-pi) calls `Bus::publish("robot/state", envelope_bytes)`
2. In-process subscribers receive via `broadcast::Receiver`
3. `IpcFanout` forwards runtime topics through the outbox to the gateway's `IpcListener`
4. Gateway streams to Consul via WebTransport or HTTP

## Integration
- **Depends on**: `armee-proto` (Envelope, RobotState, etc.)
- **Producers**: `marengo-pi` (RobotState, SafetyState, Heartbeat)
- **Consumers**: `marengo-gateway`, Consul `chappe-client.ts`
- Topics: canonical constants in `src/topics.rs` (telemetry, log/audit events, gateway→runtime commands)

**Detailed map**: [src/codemap.md](src/codemap.md)
