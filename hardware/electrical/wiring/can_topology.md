# CAN topology

Logical and physical CAN segments for Marengo. Authoritative parameters: [hardware ADR 0001](../../docs/decisions/0001-can-and-motors.md).

## Robstride limb buses

| Item | Value |
|------|-------|
| Host | Raspberry Pi SocketCAN (`can0`, `can1`, `can2`, etc.) |
| Bitrate | 1 Mbit/s |
| Cable | Twisted pair (ISO 11898), keep stubs short |
| Termination | 120 Ω at bus ends only |
| Devices | Static addresses from `config/motors.yaml` (`can_interface`, `device_id`) |

Current bench (2026-10-28):

```
[Pi 5 Waveshare 2-CH CAN HAT can0] ---- right arm ids 1-5 (RS03, RS03, RS02, RS02, RS00) ---- [120Ω term]
[Pi 5 Waveshare 2-CH CAN HAT can1] ---- (unused; previously left shoulder pitch id 12)
```

The full-body bus plan is open: `config/motors_humanoid.yaml` assumes five buses (`can0`–`can4`), which needs more channels than the 2-CH HAT, and its ID numbering disagrees with the ID blocks in [hardware ADR 0001](../../docs/decisions/0001-can-and-motors.md). The MCP2515 holds only 2 RX frames, so per-bus traffic and TX pacing matter.

Test harness: `just vcan` creates `vcan0`/`vcan1` as virtual stand-ins only.

## Moteus (future)

Reserved, not populated on v0 prototype. When added:

- Separate transceiver and harness from the Robstride buses
- Document node IDs in a follow-on hardware ADR
