# crates/robstride/

## Responsibility
Hardware CAN transport driver for Robstride RS00–RS04 actuators (MIT Mode 0). Encodes and decodes the vendor-specific 29-bit extended CAN identifier protocol. **No control policy, no safety filtering** — only bytes on the bus and a feedback cache.

## Design

### Trait hierarchy (layered)
```
CanBus (raw frame send/recv)
   │
   └── MotorBus (MIT commands, lifecycle, parameters, feedback drain)
          │
          ├── MemoryBus (in-memory, tests only)
          ├── SocketCanBus (single SocketCAN interface, Linux, `socketcan` feature)
          ├── SocketCanRouter (multi-interface dispatch, Linux, `socketcan` feature)
          └── RuntimeBus (enum dispatch for CLI-selectable backend)
```

- `CanBus` — low-level: `send_frame`, `send_frame_to`, `recv_frames`, `recv_frames_from`, `recv_frames_from_nonblocking`.
- `MotorBus` — mid-level trait with default implementations: `mit_control_all`, `mit_control_all_at`, `enable_drive`, `disable_drive`, `set_zero_position`, `read_parameter`, `write_parameter`, `set_run_mode`, `speed_control`, `enable_active_reporting`, `recv_all`, `recv_all_addressed`. Methods exist in both `device_id` (single-interface) and `MotorAddress` (multi-interface) forms.
- `recv_all` / `recv_all_addressed` — polling loops that drain CAN frames up to a budget/quiet timeout, decode MIT status + fault reports, and populate a `HashMap<u8, MotorState>` or `HashMap<MotorAddress, MotorState>` feedback cache.

### Modules
- `comm` — 29-bit extended CAN ID pack/unpack. `CommunicationType` enum defines: OperationControl(1), OperationStatus(2), Enable(3), Disable(4), SetZeroPosition(6), ReadParameter(17), WriteParameter(18), FaultReport(21), ActiveReporting(24). `pack_ext_id`, `unpack_ext_id`, `DEFAULT_HOST_ID(0xFD)`.
- `mit` — MIT Mode 0 command/feedback encoding: `MitCommand` (device_id, motor_type, position, velocity, kp, kd, tau_ff), `MitFeedback` (position, velocity, torque, temperature, fault). Position/velocity/kp/kd are big-endian u16 payload fields; torque feedforward is the 16-bit `extra_data` in the extended CAN id. `encode_mit` and `decode_mit_feedback`.
- `lifecycle` — Frame encoding for enable/disable/set-zero-position/active-reporting. All use `CommunicationType` + `DEFAULT_HOST_ID`.
- `params` — Parameter read/write frame encoding: `RunMode`, `ParameterId` (various firmware register addresses), `ParameterValue` (u8/u16/f32/string variants). `encode_read_parameter`, `encode_write_parameter`, `encode_set_run_mode`, `encode_speed_ref`, `encode_position_ref`, `encode_current_ref`.
- `state` — `MotorState` struct: position_rad, velocity_rad_s, torque_nm, temperature_c, fault, updated timestamp. `is_stale()` for age checking.
- `motor_type` — Maps `MotorType` enum (from config) to MIT field scaling constants: `MitRanges` (position, velocity, torque, kp, kd scales).
- `protocol` — Legacy 11-bit CAN stub (tests only; do not use on bench).
- `vcan` — Virtual CAN setup helper for test fixtures.

### Bus backends
- `MemoryBus`: in-memory tx/rx queues for unit tests without hardware.
- `SocketCanBus`: wraps `socketcan` crate; configures vcan loopback; sets 1 ms read timeout; encodes/decodes extended CAN frames.
- `SocketCanRouter`: multiple `SocketCanBus` instances keyed by interface name; dispatches send by `MotorAddress.interface` and recv drains from all interfaces.
- `RuntimeBus`: enum dispatch for CLI-selectable backend (e.g. `--bus socketcan:can0`).

## Flow
```
Davout → MotorBus::mit_control_all_at(cmd)
   │
   ├─ mit::encode_mit → (can_id, [u8; 8]) payload + extended-ID torque field
   ├─ CanBus::send_frame_to → SocketCan socket.write(frame)
   ▼
   CAN bus

Next tick:
   CanBus::recv_frames_from → socket.read() → [CanFrame]
   MotorBus::recv_all_addressed → decode each frame:
      ├─ OperationStatus → mit::decode_mit_feedback → MotorState
      └─ FaultReport → decode_fault_report → state.fault
   → HashMap<MotorAddress, MotorState> returned to Davout
```

## Integration
- **Depends on**: `marengo-config` (MotorType enum, MotorEntry, MotorsConfigFile). Optionally `socketcan` crate (Linux).
- **Called by**: `davout` (sole production caller). MemoryBus used by tests across the workspace.
- **Does not**: decide torque/position limits, manage enable state, apply direction/gear_ratio, run periodic control, load config files.
- **Wire spec**: `hardware/docs/decisions/0002-robstride-protocol.md`.
