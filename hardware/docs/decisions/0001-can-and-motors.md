# Hardware ADR 0001: CAN and motors

**Status:** Accepted (prototype values — update when hardware is commissioned)  
**Date:** 2025-05-19

## Context

Marengo bench bring-up uses Robstride RS-series actuators on SocketCAN. Software reads joint→bus mapping from [`config/motors.yaml`](../../../config/motors.yaml).

## Decision

### CAN1 — Robstride (primary leg/arm bus)

| Parameter | Value |
|-----------|-------|
| Interface | `can0` on Raspberry Pi (Pi 5 + CAN HAT) |
| Bitrate | **1 Mbit/s** |
| Termination | 120 Ω at each end of the daisy chain (max two terminators powered) |
| Frame format | Standard 11-bit CAN |
| Protocol | Robstride RS motion frames (see `crates/robstride`) |

### Device IDs (prototype v0)

| Joint | `device_id` | Notes |
|-------|-------------|-------|
| `joint1` | 1 | Bench link 1 |
| `joint2` | 2 | Bench link 2 |

Reassign IDs in firmware before changing `config/motors.yaml`.

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
- CI uses `vcan0` at 1 Mbit/s for encode/decode tests only (no motors).

## References

- [wiring/can_topology.md](../../electrical/wiring/can_topology.md)
- [wiring/connectors.md](../../electrical/wiring/connectors.md)
- [docs/safety.md](../../../docs/safety.md)
