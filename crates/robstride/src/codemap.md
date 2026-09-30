# crates/robstride/src/

## Responsibility
Low-level Robstride protocol implementation and bus backends.

## Design
| Module | Role |
|--------|------|
| `bus.rs` | Required one-read `CanBus`, checked `MotorBus`, RX envelope/conversion, deque MemoryBus, fair SocketCAN router and RuntimeBus |
| `receive.rs` | Shared raw receive engine, 64-frame/256-attempt bounds, deadlines, full source-pass completion, retained prefix/error |
| `feedback.rs` | Status/detail/malformed observations and transport frames with common raw ordinals; compatibility projection |
| `comm.rs` | 29-bit extended ID pack/unpack, `CommunicationType` |
| `mit.rs` | MIT Mode 0 encode/decode, `MitCommand`, `MitFeedback` |
| `lifecycle.rs` | Enable, disable, set-zero frames |
| `params.rs` | Firmware parameter read/write |
| `state.rs` | Per-motor feedback cache |
| `motor_type.rs` | RS00–RS04 type constants |
| `command.rs` | Typed numeric command validation errors |

## Flow
TX: `MitCommand` → `encode_mit` → `pack_ext_id` → CAN socket
RX: one nonblocking source read → length/class envelope + host time → shared bounded report → exact-eight Data decode or malformed/transport evidence → ordered report for Davout; `MotorState` is only a compatibility view.

## Integration
- No upstream crate dependencies beyond marengo-config for motor type metadata
- Davout is the only production consumer
