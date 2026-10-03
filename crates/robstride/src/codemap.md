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
| `mit.rs` | MIT Mode 0 encode/decode, `MitCommand`, `MitFeedback`; `decode_mit_command_fields` (raw type-1 command codes) |
| `lifecycle.rs` | Enable, disable, set-zero (type 6) frames |
| `identity.rs` | Type-0 `encode_get_device_id` / `encode_default_get_device_id`, `decode_device_id_reply` (ext id `device<<8 \| 0xFE`), `DeviceUid` (8-byte MCU UID, `as_u64` LE, lowercase hex) |
| `params.rs` | Firmware parameter read/write; `MechPos` (0x7019, read-only), `ZeroSta` (0x7029), `AddOffset` (0x702B); type-17 `decode_read_parameter_reply` → `ParameterReadReply` (status 0 = success) |
| `state.rs` | Per-motor feedback cache |
| `motor_type.rs` | RS00–RS04 type constants |
| `command.rs` | Typed numeric command validation errors |
| `wire.rs` | Offline direction-aware frame classification (`classify_frame` → host command vs drive frame, `MitCommandFields::is_neutral`) for candump analysis |

## Flow
TX: `MitCommand` → `encode_mit` → `pack_ext_id` → CAN socket
RX: one nonblocking source read → length/class envelope + host time → shared bounded report → exact-eight Data decode or malformed/transport evidence → ordered report for Davout; `MotorState` is only a compatibility view.

## Integration
- No upstream crate dependencies beyond marengo-config for motor type metadata
- Davout is the only production consumer
