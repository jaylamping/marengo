# crates/armee-proto/

## Responsibility
Generated Protocol Buffer types from `proto/`. The single source of wire-format type definitions for all Chappe messages. Not hand-editable — change `.proto` files under `proto/` and rebuild.

## Design

### Code generation
- `include!(concat!(env!("OUT_DIR"), "/marengo.v1.rs"))` — includes the prost-generated Rust code from `build.rs` compilation.
- Re-exports `prost` for caller convenience (`Message` trait, encoding helpers).
- Generated types live in the `marengo.v1` module namespace.

### Key proto types (from `proto/marengo/v1/*.proto`)
- `Envelope` — universal message wrapper: `timestamp_ms`, `source_node`, `message_type`, `payload` (raw bytes). Used by Chappe `Transport::publish` for typed message wrapping.
- `RobotState` — published on topic `robot/state`: `joints: Vec<JointState>`, `control_mode`, `operational_mode`, `timestamp_ms`. Berthier publishes this at reduced rate (e.g. 20 Hz).
- `JointState` — per-joint state: `position`, `velocity`, `torque`, `temperature`, `motor_fault`.
- `Heartbeat` — process liveness: `timestamp_ms`, `node_id`.
- `Fault` — structured fault report: `severity`, `code`, `message`, `joint`, `timestamp_ms`.
- `FaultSeverity` — enum: `Info`, `Warning`, `Error`, `Critical`.
- `SafetyState` — safety system status: `estop_asserted`, `operational_mode`, `control_mode`.
- `EnableRequest` — enable/disable command from gateway/runtime.
- `ImuSample` — IMU data: `accelerometer`, `gyroscope`, `temperature`, `timestamp_ms`.
- `LogEvent` — structured log forwarded on topic `logs/structured`: `level`, `target`, `message`, `fields_json`, `module_path`, `file`, `line`, `timestamp_ms`.
- `ControlMode`, `OperationalMode` — enums matching Davout/Berthier mode types.

### Roundtrip tests
- Each message type has a roundtrip test: encode to bytes with `encode_to_vec()`, decode with `Message::decode()`, assert field equality. Tests: Heartbeat, RobotState, SafetyState, EnableRequest, Envelope, ImuSample.

## Flow
```
Proto definitions (proto/marengo/v1/*.proto)
        │
        ▼ build.rs — protobuf compiler (prost)
        │
        ▼
  Generated Rust types in OUT_DIR/marengo.v1.rs
        │
        ▼ include!()
        │
  armee-proto crate → re-exported to workspace
        │
        ├─ chappe → Envelope wrapping, publish
        ├─ berthier → RobotState, ControlMode, JointState
        ├─ davout → OperationalMode, ControlMode
        └─ marengo-pi/gateway → all types for IPC
```

## Integration
- **Depends on**: `prost` (protobuf codec), `prost-build` (build script). No workspace crate dependencies.
- **Depended upon by**: `chappe`, `berthier`, `davout`, `marengo-pi`, `marengo-gateway`, `consul`.
- **Does not**: contain control logic, open files, interact with hardware, or run any runtime computation.
- **Build**: after editing `.proto` files in `proto/`, run `cargo build -p armee-proto` or the workspace script `consul/npm run gen:proto` for TypeScript parallel types.
