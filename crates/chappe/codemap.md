# crates/chappe/

## Responsibility
Inter-process **pub/sub message bus** for protobuf `Envelope` bytes between Pi runtime, Jetson, Consul UI, and tools. Transport only — no motor control or safety logic.

## Design
- **Observer / Pub-Sub**: `Bus::publish(topic, bytes)` → broadcast channel; `Bus::subscribe(topic)` → receiver.
- **Typed helpers**: `publish_proto` / `subscribe_proto` for prost-encoded messages.
- **Transport abstraction**: in-process `Bus` + Unix-domain socket `IpcListener` / `IpcClient` for cross-process.
- **Tracing layer**: optional OpenTelemetry integration via `tracing_layer` module.
- Default channel capacity: 256 messages per topic.

## Flow
1. Producer (marengo-pi) calls `Bus::publish("robot/state", envelope_bytes)`
2. In-process subscribers receive via `broadcast::Receiver`
3. `IpcListener` fans out to connected Unix socket clients (marengo-gateway)
4. Gateway streams to Consul via WebTransport or HTTP

## Integration
- **Depends on**: `armee-proto` (Envelope, RobotState, etc.)
- **Producers**: `marengo-pi` (RobotState, SafetyState, Heartbeat)
- **Consumers**: `marengo-gateway`, `marengo-jetson`, Consul `chappe-client.ts`
- Topics: `robot/state`, `safety/state`, `heartbeat`, enable/homing/testing commands

**Detailed map**: [src/codemap.md](src/codemap.md)
