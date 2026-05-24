# CAN topology

Logical and physical CAN segments for Marengo. Authoritative parameters: [ADR 0001](../../docs/decisions/0001-can-and-motors.md).

## Robstride limb buses

| Item | Value |
|------|-------|
| Host | Raspberry Pi SocketCAN (`can0`, `can1`, `can2`, etc.) |
| Bitrate | 1 Mbit/s |
| Cable | Twisted pair (ISO 11898), keep stubs short |
| Termination | 120 Ω at bus ends only |
| Devices | Static addresses from `config/motors.yaml` (`can_interface`, `device_id`) |

```
[Pi CAN HAT can0] ---- left limb devices ---- [120Ω term]
[Pi CAN HAT can1] ---- right limb devices ---- [120Ω term]
```

Test harness: `just vcan` creates `vcan0`/`vcan1` as virtual stand-ins only.

## CAN2 — Moteus

Reserved, not populated on v0 prototype. When added:

- Separate transceiver and harness from CAN1
- Document node IDs in a follow-on hardware ADR
