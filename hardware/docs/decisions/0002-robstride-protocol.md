# ADR 0006 (hardware): Robstride RS00–RS04 MIT protocol

**Status:** Accepted  
**Date:** 2025-05-19

> File name `0002-robstride-protocol.md` under `hardware/docs/decisions/` — companion to repo-root ADR 0004/0005.

## Primary sources

| Model | Manual |
|-------|--------|
| RS00 | [RS00 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS00/RS00User%20Manual260428.pdf) |
| RS02 | [RS02 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS02/RS02User%20Manual260428.pdf) |
| RS03 | [RS03 User Manual 260713](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS03/RS03User%20Manual260713.pdf) |
| RS04 | [RS04 User Manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS04/RS04User%20Manual260428.pdf) |

**Secondary:** [Seeed RobStride control](https://wiki.seeedstudio.com/robstride_control/), [crates.io `robstride`](https://docs.rs/robstride/latest/robstride/) (`robstride00`–`04`).

## Protocol family (shared)

- **CAN 2.0B extended** frames (29-bit arbitration).
- Extended arbitration ID: `(comm_type << 24) | (extra_data << 8) | device_id`.
- **MIT Mode 0** — five-tuple: `p_des`, `v_des`, `kp`, `kd`, `t_ff`; torque is encoded in the extended ID `extra_data` field.
- **Feedback:** `comm_type=2` status frames; decode position, velocity, torque, temperature, fault reports.
- **Active Reporting:** `comm_type=24` free-drive sensing frames (manual §4.1.11) — see appendix below. Not a motor-command path.
- **Lifecycle:** enable, disable, stop, `SetZero` per manual (parameter IDs may differ by model).
- Firmware `run_mode` (`0x7005`): `0=MIT`, `1=Position`, `2=Speed`, `3=Current`.

The Seeed wiki's simplified `0x200`/`0x300`/`0x400 + device_id` table is documentation drift. The Seeed SDK and vendor manuals use communication types plus parameter writes: Position writes `loc_ref` (`0x7016`), Speed writes `spd_ref` (`0x700A`), and Current writes `iq_ref` (`0x7006`).

## Rated limits (Seeed table — confirm in each PDF before production)

| Model | Peak torque | Max speed | KP max | KD max |
|-------|-------------|-----------|--------|--------|
| RS04 | 120 Nm | 15 rad/s | 5000 | 100 |
| RS03 | 60 Nm | 20 rad/s | 5000 | 100 |
| RS02 | 17 Nm | 44 rad/s | 500 | 5 |
| RS00 | 17 Nm | 50 rad/s | 500 | 5 |

RS03 "Max speed" is the MIT velocity field range: Seeed listed 50, but the RS03 manual (§4.1.2, all revisions) and bench capture `cd-20261003T145133Z` give ±20 rad/s; Marengo uses 20 since 2026-10-03. RS00 is disputed (manuals: 33 rad/s, 14 Nm). See [robstride-mit-ranges.md](../../../docs/commissioning/firmware/robstride-mit-ranges.md).

## Marengo implementation

- `motor_type` in [`config/motors.yaml`](../../../config/motors.yaml): `rs00` | `rs02` | `rs03` | `rs04`.
- `crates/robstride`: vendor MIT encoding, report-owned status decoding, lifecycle frames, and parameter read/write helpers. It stays in raw motor/CAN coordinates and does not apply joint sign or gearing.
- `crates/davout`: safety gateway and joint↔motor coordinate boundary. It reads each motor row's `direction` and `gear_ratio`, filters commands in joint space, converts approved commands to motor space before calling robstride, and converts feedback back to joint space before Berthier reads it.
- **MIT production path:** Berthier joint-space commands → Davout safety + direction/gear transform → Robstride `OPERATION_CONTROL` (`comm_type=1`) every tick.
- **Bench diagnostics:** firmware Speed/Position/Current modes require explicit Davout methods and config gates; do not map Berthier control modes to firmware `run_mode`.

### Software validation update (2026-10-03)

MIT encoders return typed errors for nonfinite and finite out-of-range fields;
addressed batches validate every command and route before any transmit. Firmware
writes validate register kind and reject nonfinite floats, negative gains,
negative speed/torque caps, and writes to `ZeroSta` or `AddOffset`. Raw type-22
save frames are refused at the CAN bus boundary. A type-4 fault-clear encoder
uses `Byte[0]=1`, but no `MotorBus` fault-clear operation is wired; ordinary
Disable remains a stop request, not a fault clear.

The RS03 260713 manual's parameter table (PDF pages 49–50, printed pages 48–49)
specifies `run_mode` 0x7005 as u8, `EPScan_time` 0x7026 as u16,
`CAN_TIMEOUT` 0x7028 as u32, and supported target/gain/limit registers as
float. All use little-endian parameter payloads. The [current RS02 260713
manual](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS02/RS02User%20Manual260713.pdf)
agrees on timing register kinds. Older vendor documentation uses conflicting
register IDs/kinds, so this static schema requires model/firmware
identification before commissioning use.

This verification repairs software admission and the stale RS03 source link.
The historical wire scales above and installed-drive policy are unchanged;
per-model firmware limit/timeout readback and physical acceptance remain M02 in
the implementation ledger. This does not certify other models or installed
firmware against the newer RS03 manual.

### Coordinate ownership

- **Joint space:** URDF axes, armee-dynamics gravity torques, Berthier control modes, Chappe `RobotState`, and Davout limit checks.
- **Motor space:** Robstride MIT fields and SocketCAN frames.
- **Boundary:** Davout only. Do not duplicate `direction` / `gear_ratio` handling in Berthier, robstride, bins, or generated telemetry. This keeps safety limits and sign tests tied to the same approved command path.

For `scale = direction * gear_ratio`, Davout applies:

- position / velocity command: `motor = joint * scale`
- torque feedforward command: `motor = joint / scale`
- feedback position / velocity: `joint = motor / scale`
- feedback torque: `joint = motor * scale`
- MIT `kp` / `kd`: `motor_gain = joint_gain / scale^2`

## Gate before bench gravity comp

1. Extended-frame roundtrip on all configured `device_id`s.
2. Per-joint **sign test**: small `+t_ff`, verify direction vs URDF axis.
3. `SetZero` / homing documented per motor before trusting `q` for `tau_g`.

## Appendix: Active Reporting (comm type 24)

Vendor free-drive sensing (manual §4.1.11; [Seeed RobStride control](https://wiki.seeedstudio.com/robstride_control/)). Marengo uses it only for **diagnostics / limp sensing**, never as a Berthier→Davout command bypass.

| Aspect | Contract |
|--------|----------|
| Encode | `robstride::encode_active_reporting` / `encode_default_active_reporting` — payload `01..06 F_CMD` (`F_CMD` `00`=off, `01`=on); arbitration uses `CommunicationType::ActiveReporting` (=24) |
| Decode | The bounded receive/report path decodes **OperationStatus** (`comm_type=2`) and **ActiveReporting** (`comm_type=24`) with the same MIT scale tables; status flags and drive mode remain separate evidence |
| Bus | `MotorBus::enable_active_reporting_at` / `disable_active_reporting_at`; `recv_feedback_report` retains type-24 status observations |
| When on | Davout sync: desired iff not ACTIVE and (global diagnostics or any unexpired lease). **Never** while operational mode is `ACTIVE` (MIT OperationControl + status own that path) |
| Leases | Consul Enhanced-logging → gateway → Chappe → Davout (`crates/davout/src/active_reporting.rs`); TTL backstop; see [docs/safety.md](../../../docs/safety.md) |

Unit coverage: `lifecycle.rs` §4.1.11 encode tests; `mit.rs` type-24 decode roundtrip; Davout lease/TTL/ACTIVE-exclusion tests.

## Appendix: vCAN test harness (SocketCAN)

Marengo’s ignored SocketCAN integration tests open a **`vcan*`** interface and send/receive on the **same socket** (`crates/robstride/src/bus.rs`, `configure_vcan_loopback`). On real `can0`/`can1` hardware this is not applied.

| Setting | vcan only | Purpose |
|---------|-----------|---------|
| `SOCK_RAW` loopback | `set_loopback(true)` | TX frames are visible to the same socket’s RX queue |
| `CAN_RAW_RECV_OWN_MSGS` | `set_recv_own_msgs(true)` | Driver delivers self-sent frames for send→recv roundtrip tests |

**Production path unchanged:** live bench CAN does not enable loopback; protocol encoding and MIT lifecycle frames are identical. See [Seeed RobStride control](https://wiki.seeedstudio.com/robstride_control/) for vendor MIT semantics.

## Related

- [0001-can-and-motors.md](0001-can-and-motors.md) — bus topology
- [docs/decisions/0004-control-modes-and-mit.md](../../../docs/decisions/0004-control-modes-and-mit.md)
