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
- Extended arbitration ID: `(comm_type << 24) | (extra_data << 8) | device_id`.
- **MIT Mode 0** — five-tuple: `p_des`, `v_des`, `kp`, `kd`, `t_ff`; torque is encoded in the extended ID `extra_data` field.
- **Feedback:** `comm_type=2` status frames; decode position, velocity, torque, temperature, fault reports.
- **Lifecycle:** enable, disable, stop, `SetZero` per manual (parameter IDs may differ by model).
- Firmware `run_mode` (`0x7005`): `0=MIT`, `1=Position`, `2=Speed`, `3=Current`.

The Seeed wiki's simplified `0x200`/`0x300`/`0x400 + device_id` table is documentation drift. The Seeed SDK and vendor manuals use communication types plus parameter writes: Position writes `loc_ref` (`0x7016`), Speed writes `spd_ref` (`0x700A`), and Current writes `iq_ref` (`0x7006`).

## Rated limits (Seeed table — confirm in each PDF before production)

| Model | Peak torque | Max speed | KP max | KD max |
|-------|-------------|-----------|--------|--------|
| RS04 | 120 Nm | 15 rad/s | 5000 | 100 |
| RS03 | 60 Nm | 50 rad/s | 5000 | 100 |
| RS02 | 17 Nm | 44 rad/s | 500 | 5 |
| RS00 | 17 Nm | 50 rad/s | 500 | 5 |

## Marengo implementation

- `motor_type` in [`config/motors.yaml`](../../../config/motors.yaml): `rs00` | `rs02` | `rs03` | `rs04`.
- `crates/robstride`: vendor `encode_mit` / `decode_mit_feedback`, lifecycle frames, and parameter read/write helpers.
- **MIT production path:** Berthier → Davout → Robstride `OPERATION_CONTROL` (`comm_type=1`) every tick.
- **Bench diagnostics:** firmware Speed/Position/Current modes require explicit Davout methods and config gates; do not map Berthier control modes to firmware `run_mode`.

## Gate before bench gravity comp

1. Extended-frame roundtrip on all configured `device_id`s.
2. Per-joint **sign test**: small `+t_ff`, verify direction vs URDF axis.
3. `SetZero` / homing documented per motor before trusting `q` for `tau_g`.

## Related

- [0001-can-and-motors.md](0001-can-and-motors.md) — bus topology
- [docs/decisions/0004-control-modes-and-mit.md](../../../docs/decisions/0004-control-modes-and-mit.md)
