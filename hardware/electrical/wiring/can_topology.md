# CAN topology

Logical and physical CAN segments for Marengo. Authoritative parameters: [ADR 0001](../../docs/decisions/0001-can-and-motors.md).

## CAN1 — Robstride

| Item | Value |
|------|-------|
| Host | Raspberry Pi (`can0`) |
| Bitrate | 1 Mbit/s |
| Cable | Twisted pair (ISO 11898), keep stubs short |
| Termination | 120 Ω at bus ends only |
| Devices | `joint1` → ID 1, `joint2` → ID 2 (see `config/motors.yaml`) |

```
[Pi CAN HAT] ---- joint1 (ID 1) ---- joint2 (ID 2) ---- [120Ω term]
```

Bench: use `vcan0` in dev container (`just vcan`) — no physical transceiver required.

## CAN2 — Moteus

Reserved, not populated on v0 prototype. When added:

- Separate transceiver and harness from CAN1
- Document node IDs in a follow-on hardware ADR
