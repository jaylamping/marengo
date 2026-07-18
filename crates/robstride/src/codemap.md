# crates/robstride/src/

## Responsibility
Low-level Robstride protocol implementation and bus backends.

## Design
| Module | Role |
|--------|------|
| `bus.rs` | `MotorBus` trait, `MemoryBus`, `SocketCanBus`, `RuntimeBus` |
| `comm.rs` | 29-bit extended ID pack/unpack, `CommunicationType` |
| `mit.rs` | MIT Mode 0 encode/decode, `MitCommand`, `MitFeedback` |
| `lifecycle.rs` | Enable, disable, set-zero frames |
| `params.rs` | Firmware parameter read/write |
| `state.rs` | Per-motor feedback cache |
| `motor_type.rs` | RS00–RS04 type constants |
| `protocol.rs` | Legacy 11-bit stub (tests only) |

## Flow
TX: `MitCommand` → `encode_mit` → `pack_ext_id` → CAN socket
RX: CAN frame → `unpack_ext_id` → `decode_mit` → `MotorState` update

## Integration
- No upstream crate dependencies beyond marengo-config for motor type metadata
- Davout is the only production consumer
