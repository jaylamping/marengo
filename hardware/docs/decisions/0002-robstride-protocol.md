# ADR 0006 (hardware): Robstride RS00–RS04 MIT protocol

**Status:** Accepted  
**Date:** 2025-05-19

> File name `0002-robstride-protocol.md` under `hardware/docs/decisions/` — companion to repo-root ADR 0004/0005.

## Primary sources

| Model | Manual |
|-------|--------|
| RS00 | [RS00 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS00/RS00User%20Manual260428.pdf) |
| RS02 | [RS02 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS02/RS02User%20Manual260428.pdf) |
| RS03 | [RS03 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS03/RS03User%20Manual260428.pdf) |
| RS04 | [RS04 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS04/RS04User%20Manual260428.pdf) |

**Secondary:** [Seeed RobStride control](https://wiki.seeedstudio.com/robstride_control/), [crates.io `robstride`](https://docs.rs/robstride/latest/robstride/) (`robstride00`–`04`).

## Protocol family (shared)

- **CAN 2.0B extended** frames (29-bit arbitration).
- **MIT Mode 0** — five-tuple: `p_des`, `v_des`, `kp`, `kd`, `t_ff`.
- **MIT command ID:** `0x200 + motor_id` (TX).
- **Feedback:** model-specific RX IDs; decode position, velocity, torque, mode, fault.
- **Lifecycle:** enable, disable, stop, `SetZero` per manual (parameter IDs may differ by model).

## Rated limits (Seeed table — confirm in each PDF before production)

| Model | Peak torque | Max speed | KP max | KD max |
|-------|-------------|-----------|--------|--------|
| RS04 | 120 Nm | 15 rad/s | 5000 | 100 |
| RS03 | 60 Nm | 50 rad/s | 5000 | 100 |
| RS02 | 17 Nm | 44 rad/s | 500 | 5 |
| RS00 | 17 Nm | 50 rad/s | 500 | 5 |

## Marengo implementation

- `motor_type` in [`config/motors.yaml`](../../../config/motors.yaml): `rs00` | `rs02` | `rs03` | `rs04`.
- `crates/robstride`: `encode_mit(cmd, MotorType)` / `decode_feedback` — **delete** legacy 11-bit `0x140` stub.
- **Phase 2 deliverable:** RS02 + RS03 encode/decode + hardware/vcan roundtrip.
- **RS00 / RS04:** same API; unit tests from PDF quantization before motors are mounted.

## Gate before bench gravity comp

1. Extended-frame roundtrip on all configured `device_id`s.
2. Per-joint **sign test**: small `+t_ff`, verify direction vs URDF axis.
3. `SetZero` / homing documented per motor before trusting `q` for `tau_g`.

## Related

- [0001-can-and-motors.md](0001-can-and-motors.md) — bus topology
- [docs/decisions/0004-control-modes-and-mit.md](../../../docs/decisions/0004-control-modes-and-mit.md)
