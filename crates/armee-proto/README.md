<p align="center">
  <img src="../../docs/portraits/armee.jpg" alt="Napoleon at Friedland" width="420"/>
</p>

# armee-proto

Part of Armée. Thin Rust wrapper around [`proto/`](../../proto/).

At compile time, `build.rs` runs prost-build on `../../proto/**/*.proto` and this crate re-exports the generated types. Change `.proto` files and rebuild. Do not edit generated code here.

```rust
use armee_proto::{Heartbeat, RobotState};
use prost::Message;

let bytes = Heartbeat { timestamp_ms: 0, node_id: "pi".into() }.encode_to_vec();
```

Consumers depend on `armee-proto = { workspace = true }`. Chappe payloads are binary protobuf ([ADR 0001](../../docs/decisions/0001-protobuf-wire-types.md)).
