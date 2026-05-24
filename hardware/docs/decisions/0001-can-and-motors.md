# Hardware ADR 0001: CAN and motors

**Status:** Accepted (prototype values — update when hardware is commissioned)  
**Date:** 2025-05-19

## Context

Marengo uses Robstride RS-series actuators on SocketCAN. Software reads joint→bus mapping from [`config/motors.yaml`](../../../config/motors.yaml) (4-DOF arm bring-up — active config) or [`config/motors_humanoid.yaml`](../../../config/motors_humanoid.yaml) (23-DOF humanoid template). Joint names and motor types are defined in [`kinematics.md`](../kinematics.md).

## Decision

### Robstride limb buses

| Parameter | Value |
|-----------|-------|
| Interface | `can0`, `can1`, `can2`, etc. on Raspberry Pi (Pi 5 + CAN HATs) |
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

**Dual shoulder pitch bring-up** — [`config/bringup/shoulder_pitch_dual/`](../../../config/bringup/shoulder_pitch_dual/):

| Joint | `can_interface` | `device_id` | Motor |
|-------|-----------------|-------------|-------|
| `left_shoulder_pitch` | can0 | 12 | RS03 |
| `right_shoulder_pitch` | can1 | 2 | RS03 |

**Left 4-DOF arm (Milestone B)** — [`config/bringup/arm_4dof_left/`](../../../config/bringup/arm_4dof_left/) / [`config/motors.yaml`](../../../config/motors.yaml):

| Joint | `device_id` | Motor |
|-------|-------------|-------|
| `shoulder_roll` | 11 | RS03 |
| `shoulder_pitch` | 12 | RS03 |
| `upper_arm_yaw` | 13 | RS02 |
| `elbow` | 14 | RS02 |

All on `can0`. Right arm (future, `can1`): roll **1**, pitch **2**, yaw **3**, elbow **4**.

**Legacy 4-DOF template (superseded):** IDs 1–4 on single bus — do not use on commissioned hardware.

**Humanoid (template)** — [`config/motors_humanoid.yaml`](../../../config/motors_humanoid.yaml): IDs 1–23 for full body layout.

Reassign IDs in firmware before changing motor YAML.

### CAN2 — Moteus (future)

Reserved for auxiliary axes (gripper, head). Not wired in v0 prototype. Do not mix Robstride and Moteus on the same bus segment.

### E-stop

| Signal | Behavior |
|--------|----------|
| Hardware E-stop (normally closed chain) | Opens motor power stage / enable relay; drives coast or brake per vendor doc |
| Safe state when E-stop asserted | **No torque** — software must read estop input and remain in `OPERATIONAL_MODE_DISABLED` |
| Software reset | Requires hardware E-stop released **and** operator homing sequence before `READY` |

Pin mapping: see [connectors.md](../../electrical/wiring/connectors.md) (prototype bench harness).

### Firmware

Document per-joint Robstride firmware strings in `config/motors.yaml` (`firmware_version`). Re-flash via vendor tool before changing control gains in software.

## Consequences

- [`can_topology.md`](../../electrical/wiring/can_topology.md) and [`connectors.md`](../../electrical/wiring/connectors.md) must stay aligned with this ADR.
- CI uses virtual CAN interfaces as test stand-ins only (no motors).

## References

- [wiring/can_topology.md](../../electrical/wiring/can_topology.md)
- [wiring/connectors.md](../../electrical/wiring/connectors.md)
- [docs/safety.md](../../../docs/safety.md)
