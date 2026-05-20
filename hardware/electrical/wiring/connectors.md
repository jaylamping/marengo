# Connectors

Pinouts and part numbers for field connectors. **Prototype bench** values — revise before field deployment.

## E-stop chain

| Connector | Part | Mating | Signals |
|-----------|------|--------|---------|
| E-stop in | Panel mount NC contact | — | `ESTOP+`, `ESTOP-` (series with motor enable relay) |
| To Pi GPIO | 2-pin JST-XH | — | `ESTOP_SENSE` (active-low when chain open) |

**Safe state:** E-stop pressed → chain open → motor power disabled → `ESTOP_SENSE` pulled to indicate fault.

| Pi GPIO (BCM) | Direction | Function |
|---------------|-----------|----------|
| 17 | Input, pull-up | `ESTOP_SENSE` (0 = asserted) |

## CAN1 — Robstride harness

| Connector | Part | Mating | Signals |
|-----------|------|--------|---------|
| Pi ↔ bus | JST-GH 4-pin or vendor HAT | — | `CAN_H`, `CAN_L`, `GND` |
| Motor | Robstride harness | per vendor | `CAN_H`, `CAN_L`, power (separate) |

## Power (motors)

| Connector | Part | Notes |
|-----------|------|-------|
| Motor power | XT30 (example) | Fused per leg; not switched by software enable |

Document final part numbers in the master BOM when the harness is built.
