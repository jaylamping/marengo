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

- `CanBus` — low-level send and route preflight, plus required `recv_one_nonblocking`. Each attempt performs at most one source read, without waiting or internally retrying interruptions. `receive_source_count` stays fixed throughout a poll; `begin_receive` rotates a router's first source. There is no default bulk-drain fallback.
- `receive` — shared engine for every raw/decoded legacy path. A poll has at most 64 raw frames and 256 read attempts; noise, idle reads and interruptions spend those same quotas. Callers may narrow capacities or add a deadline, never increase capacities. Zero budget never waits. Positive budgets permit adaptive waits after a whole idle source pass and preserve the quiet interval. Completed idle/quiet differs from WorkLimit/Deadline/Failed; a partial source pass cannot prove quiescence. Clock-budget overflow fails before IO.
- `MotorBus` — checked commands, lifecycle, parameters, `recv_feedback_report(types, budget, quiet)` and its `with_limits` variant. Reports retain configured observations, typed kernel transport frames, global counters, completion and any first terminal read error. All events have a common raw-frame ordinal independent of equal host timestamps; ignored traffic may leave ordinal gaps. A terminal error occurs before later peer frames with the same ordinal. This is host delivery order, not physical cause or a synchronized motor clock.
- `recv_all` / `recv_all_addressed` — compatibility cache APIs project the same engine. They preserve valid delivered prefixes but return errors for malformed/transport evidence or incomplete work. Healthy status can replace a fault indication. **Davout safety must consume `FeedbackReport`, not these projections.** Legacy `recv_frames` also fails when fixed eight-byte Data output cannot preserve a received envelope.

### Modules
- `comm` — 29-bit extended CAN ID pack/unpack. `CommunicationType` enum defines: OperationControl(1), OperationStatus(2), Enable(3), Disable(4), SetZeroPosition(6), ReadParameter(17), WriteParameter(18), FaultReport(21), ActiveReporting(24). `pack_ext_id`, `unpack_ext_id`, `DEFAULT_HOST_ID(0xFD)`.
- `mit` — Checked MIT Mode 0 command/feedback encoding. Position/velocity/kp/kd are big-endian u16 payload fields; torque feedforward occupies extended-ID `extra_data`. `decode_mit_feedback` retains six status fault flags from ID bits 16–21 and drive mode from bits 22–23 separately from pose.
- `feedback` — `FeedbackReport`, addressed `FeedbackObservation`, `FeedbackEvent::Status` / `DetailedFault` / `Malformed`, transport evidence and `DriveMode`. Only extended Data with exactly eight bytes becomes status or type-21 detail. Malformed evidence retains actual bytes/length/class/reason, and status ID flags/mode only for Data; Remote provides no vendor header proof. It never refreshes pose or becomes a complete detailed word. Type-21 has distinct raw four-byte detailed fault and warning domains. **Type-21 byte order and installed firmware schema remain unqualified**; `update_state` is an explicitly lossy projection.
- `lifecycle` — Frame encoding for enable/disable/set-zero-position/active-reporting. All use `CommunicationType` + `DEFAULT_HOST_ID`.
- `params` — Parameter read/write frame encoding: `RunMode`, `ParameterId`, `ParameterKind`, `ParameterValue` (u8/u16/u32/f32). Float write/reference encoders are checked; the register kind must match, gains/limits must be nonnegative, and signed position/speed/current targets remain allowed. Typed RunMode encoding is infallible.
- `command` — `CommandError` and `CommandField` identify invalid numeric data or register kinds.
- `state` — `MotorState` contains pose, temperature, compatibility fault indication and the last pose/status RX timestamp. Detailed fault indication is only 0/1, not a mixed status/detail identity. Fault-only reports preserve pose time; a fault without prior pose has none. Warnings never become pose evidence or a detailed-fault indication.
- `motor_type` — Maps `MotorType` enum (from config) to MIT field scaling constants: `MitRanges` (position, velocity, torque, kp, kd scales).
- `vcan` — Virtual CAN setup helper for test fixtures.

### Bus backends
- `MemoryBus`: deque-backed RX with O(1) one-frame handoff. Limited polls retain the FIFO suffix without copying the whole backlog. Synthetic `CanFrame` is explicitly full eight-byte Data; use `ReceivedCanFrame` constructors to model actual short/class evidence.
- `SocketCanBus`: opens classic CAN, safely clones its descriptor and wraps the dependency's single-read receiver without enabling FD. Socket stays nonblocking; Interrupted consumes one attempt. Kernel error subscriptions accept all classes and setup fails on subscription errors. `ReceivedCanFrame::from_socketcan` preserves actual DLC and full error mask, rejects invalid classic lengths and unexpected FD. Host time is stamped after each successful read. TX uses one atomic nonblocking write of dependency-provided bytes, reporting interruptions/short writes without retry. Neither RX time nor TX acceptance proves physical acquisition/delivery.
- `SocketCanRouter`: sorted stable interface inventory, one attempt per source per round, and rotating first source between polls. A hot port cannot drain ahead of an already queued peer. Read errors preserve the prefix/first error and permit remaining peers within the bounded round.
- `RuntimeBus`: delegates required one-read source/count/rotation seams to the concrete backend; its reports use the same engine.

## Flow
```
Davout → MotorBus::mit_control_all_at(cmd)
   │
   ├─ mit::encode_mit → (can_id, [u8; 8]) payload + extended-ID torque field
   ├─ CanBus::send_frame_to → SocketCan socket.write(frame)
   ▼
   CAN bus

Next tick:
   CanBus::recv_raw_report → bounded one-read source rounds → [TimedCanFrame] + completion/error
   MotorBus::recv_feedback_report → decode each frame:
      ├─ OperationStatus / ActiveReporting → pose + status flags + drive mode
      ├─ FaultReport → complete raw detailed-fault and warning domains
      ├─ recognized malformed → raw partial evidence, no pose
      └─ kernel Error → transport evidence, no vendor decoding
   → common raw order + counters/completion + optional read error → Davout authority
```

## Integration
- **Depends on**: `marengo-config` (MotorType enum, MotorEntry, MotorsConfigFile). Optionally `socketcan` crate (Linux).
- **Called by**: `davout` (sole production caller). MemoryBus used by tests across the workspace.
- **Does not**: decide torque/position limits, manage enable state, apply direction/gear_ratio, run periodic control, load config files.
- **Wire spec**: `hardware/docs/decisions/0002-robstride-protocol.md`.
- **Tests**: unchanged-public literal regressions in `tests/transport_regressions.rs` and `tests/fault_evidence_regressions.rs`; new receive contracts in `tests/receive_contract.rs`; typed dependency conversion in Linux `tests/socketcan_conversion.rs`; actual RuntimeBus/SocketCAN short/RTR/fairness in `tests/transport_boundary_vcan.rs`. vCAN needs a supporting Linux kernel. Physical faults, motor timing/acknowledgement and firmware qualification remain separate acceptance work.
