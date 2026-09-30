# crates/robstride/

## Responsibility
Hardware CAN transport driver for Robstride RS00–RS04 actuators (MIT Mode 0). Encodes and decodes the vendor-specific 29-bit extended CAN identifier protocol. **No control policy, no safety filtering** — bytes on the bus and lossless receive evidence.

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

- `CanBus` — low-level: `send_frame`, `send_frame_to`, route preflight (`validate_address`), raw and addressed receive methods, and `recv_timed_frames_from` / `recv_timed_frames_from_nonblocking`. Concrete SocketCAN stamps each successful read; compatibility backends stamp batch delivery. Every adapter preserves a delivered prefix on error.
- `MotorBus` — checked commands, lifecycle, parameters, and `recv_feedback_report(types, budget, quiet)`. Zero budget performs one nonblocking drain; positive budgets retain budget/quiet polling. The report contains every configured observation in transport delivery order plus an optional terminal error. Across interfaces this order is observed acquisition order, not synchronized physical order.
- `recv_all` / `recv_all_addressed` — retained compatibility cache APIs. They preserve delivered evidence before returning transport errors, but later healthy status can replace a fault indication. **Davout safety must consume `FeedbackReport`, not these projections.**

### Modules
- `comm` — 29-bit extended CAN ID pack/unpack. `CommunicationType` enum defines: OperationControl(1), OperationStatus(2), Enable(3), Disable(4), SetZeroPosition(6), ReadParameter(17), WriteParameter(18), FaultReport(21), ActiveReporting(24). `pack_ext_id`, `unpack_ext_id`, `DEFAULT_HOST_ID(0xFD)`.
- `mit` — Checked MIT Mode 0 command/feedback encoding. Position/velocity/kp/kd are big-endian u16 payload fields; torque feedforward occupies extended-ID `extra_data`. `decode_mit_feedback` retains six status fault flags from ID bits 16–21 and drive mode from bits 22–23 separately from pose.
- `feedback` — `FeedbackReport`, addressed `FeedbackObservation`, `FeedbackEvent::Status` / `DetailedFault`, and `DriveMode`. Type-21 preserves all eight raw bytes with distinct four-byte detailed fault and warning domains. Nonzero unknown fault bytes remain visible. **Type-21 byte order and installed firmware schema remain unqualified**; no decoded detailed-fault identity or recovery proof is claimed. `update_state` is an explicitly lossy diagnostic projection.
- `lifecycle` — Frame encoding for enable/disable/set-zero-position/active-reporting. All use `CommunicationType` + `DEFAULT_HOST_ID`.
- `params` — Parameter read/write frame encoding: `RunMode`, `ParameterId`, `ParameterKind`, `ParameterValue` (u8/u16/u32/f32). Float write/reference encoders are checked; the register kind must match, gains/limits must be nonnegative, and signed position/speed/current targets remain allowed. Typed RunMode encoding is infallible.
- `command` — `CommandError` and `CommandField` identify invalid numeric data or register kinds.
- `state` — `MotorState` contains pose, temperature, compatibility fault indication and the last pose/status RX timestamp. Detailed fault indication is only 0/1, not a mixed status/detail identity. Fault-only reports preserve pose time; a fault without prior pose has none. Warnings never become pose evidence or a detailed-fault indication.
- `motor_type` — Maps `MotorType` enum (from config) to MIT field scaling constants: `MitRanges` (position, velocity, torque, kp, kd scales).
- `vcan` — Virtual CAN setup helper for test fixtures.

### Bus backends
- `MemoryBus`: in-memory tx/rx queues for unit tests without hardware.
- `SocketCanBus`: stamps each read before decoding and preserves prior frames on terminal errors in both blocking and nonblocking paths. Restoring socket mode can also report failure. CAN DLC is still padded away by the legacy fixed-size frame type; malformed-length qualification is pending.
- `SocketCanRouter`: send routing uses `MotorAddress.interface`; receive attempts every interface despite a peer read error, retaining the first terminal error and all delivered observations.
- `RuntimeBus`: delegates timed receive directly to the concrete backend, retaining original host read times.

## Flow
```
Davout → MotorBus::mit_control_all_at(cmd)
   │
   ├─ mit::encode_mit → (can_id, [u8; 8]) payload + extended-ID torque field
   ├─ CanBus::send_frame_to → SocketCan socket.write(frame)
   ▼
   CAN bus

Next tick:
   CanBus::recv_timed_frames_from → socket.read() → [TimedCanFrame]
   MotorBus::recv_feedback_report → decode each frame:
      ├─ OperationStatus / ActiveReporting → pose + status flags + drive mode
      └─ FaultReport → complete raw detailed-fault and warning domains
   → ordered FeedbackObservation list + optional terminal error → Davout authority
```

## Integration
- **Depends on**: `marengo-config` (MotorType enum, MotorEntry, MotorsConfigFile). Optionally `socketcan` crate (Linux).
- **Called by**: `davout` (sole production caller). MemoryBus used by tests across the workspace.
- **Does not**: decide torque/position limits, manage enable state, apply direction/gear_ratio, run periodic control, load config files.
- **Wire spec**: `hardware/docs/decisions/0002-robstride-protocol.md`.
- **Tests**: public literal raw-frame regressions in `tests/fault_evidence_regressions.rs`; report order, routing, domains, timestamps and error contracts in `tests/feedback_report.rs`. One report test is explicitly gated to virtual SocketCAN, with no physical robot.
