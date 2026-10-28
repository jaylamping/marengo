# Hardware ADR 0001: CAN and motors

**Status:** Accepted (prototype values; bus plan for the full body still open, see Device IDs)  
**Updated:** 2026-10-28  
**Date:** 2025-05-19

## Context

Marengo uses Robstride RS-series actuators on SocketCAN. Software reads joint→bus mapping from [`config/motors.yaml`](../../../config/motors.yaml) (live right 5-DOF arm — the master config) or [`config/motors_humanoid.yaml`](../../../config/motors_humanoid.yaml) (23-DOF humanoid template). Joint names and motor types are defined in [`kinematics.md`](../kinematics.md).

## Decision

### Robstride limb buses

| Parameter | Value |
|-----------|-------|
| Interface | `can0`, `can1` on a Raspberry Pi 5 with a Waveshare 2-CH CAN HAT (MCP2515, SPI). More channels needed for the full body |
| Bitrate | **1 Mbit/s** |
| Termination | 120 Ω at each end of the daisy chain (max two terminators powered) |
| Frame format | Extended 29-bit CAN |
| Protocol | Robstride RS communication-type frames (see `crates/robstride`) |

Each configured motor is addressed by (`can_interface`, `device_id`). Device IDs may repeat on different CAN interfaces; duplicate addresses on the same interface are invalid.

### Device IDs

**Commissioned ID blocks (prototype — May 2026):**

| Subsystem | ID range |
|-----------|----------|
| Right arm | 1–10 |
| Left arm | 11–20 |
| Right leg | 21–30 |
| Left leg | 31–40 |
| Waist + aux | 41+ |

**Live right arm** (master [`config/motors.yaml`](../../../config/motors.yaml), all on `can0`):

| Joint | `device_id` | Motor | Firmware |
|-------|-------------|-------|----------|
| `right_shoulder_pitch` | 1 | RS03 | 0.3.1.42 |
| `right_shoulder_roll` | 2 | RS03 | 0.3.1.42 |
| `right_upper_arm_yaw` | 3 | RS02 | 0.2.3.34 |
| `right_elbow_pitch` | 4 | RS02 | 0.2.3.34 |
| `right_lower_arm_yaw` | 5 | RS00 | 0.0.3.32 |

**History:** earlier bring-ups used a dual shoulder-pitch slice (`left_shoulder_pitch` on can1 id 12, `right_shoulder_pitch` on can0 id 1) and a left 4-DOF arm (ids 11–14 on can0). Their `config/bringup/` trees were retired; the matching URDFs are archived under [`assets/urdf/archive/`](../../../assets/urdf/archive/). A legacy 4-DOF template used ids 1–4 on one bus; do not reuse it.

**Humanoid template** ([`config/motors_humanoid.yaml`](../../../config/motors_humanoid.yaml)) currently numbers the 23 joints 1–23 across five buses (`can0` left leg, `can1` right leg, `can2` waist, `can3` left arm, `can4` right arm). **This conflicts with the ID blocks above and with the live right arm** (template: right arm on `can4`, ids 19–23; hardware: `can0`, ids 1–5). Five buses also need more CAN channels than the current 2-channel HAT. Settle the full-body bus plan (channel count, interface hardware, ID scheme) in a follow-on hardware ADR before roadmap M5/M6, then make the template match.

Reassign IDs in firmware before changing motor YAML.

### CAN2 — Moteus (future)

Reserved for auxiliary axes (gripper, head). Not wired, and no Moteus driver exists in the workspace. Do not mix Robstride and Moteus on the same bus segment.

### E-stop

| Signal | Behavior |
|--------|----------|
| Hardware E-stop (normally closed chain) | Opens motor power stage / enable relay; drives coast or brake per vendor doc |
| Safe state when E-stop asserted | **No torque** — software must read estop input and remain in `OPERATIONAL_MODE_DISABLED` |
| Software reset | Requires hardware E-stop released **and** operator homing sequence before `READY` |

Pin mapping: see [connectors.md](../../electrical/wiring/connectors.md) (prototype bench harness).

**Current state (2026-10-28):** the E-stop GPIO input is **not wired**, so software cannot read it. The physical NC E-stop in the motor power path is the authority. `marengo-pi.service` runs `motor-repl disable` on every exit. The drives on the right arm carry a drive-side CAN timeout of 600 (~30 ms) written during PR #254, so a host stall longer than that stops them drive-side ([docs/safety.md](../../../docs/safety.md)).

### MCP2515 receive limit

The bench MCP2515 holds only 2 RX frames. Davout paces TX bursts 2 ms apart and staggers Enables (one target per interface per period); fault, E-stop and shutdown stops are never paced. Any CAN error frame, including an RX overflow, latches a transport fault ([docs/safety.md](../../../docs/safety.md)).

### Firmware

Document per-joint Robstride firmware strings in `config/motors.yaml` (`firmware_version`). Re-flash via vendor tool before changing control gains in software.

## Consequences

- [`can_topology.md`](../../electrical/wiring/can_topology.md) and [`connectors.md`](../../electrical/wiring/connectors.md) must stay aligned with this ADR.
- CI uses virtual CAN interfaces as test stand-ins only (no motors).

## References

- [wiring/can_topology.md](../../electrical/wiring/can_topology.md)
- [wiring/connectors.md](../../electrical/wiring/connectors.md)
- [docs/safety.md](../../../docs/safety.md)
