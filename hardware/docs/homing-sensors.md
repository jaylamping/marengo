# Homing sensors — mechanical and electrical

Shoulder roll and future arm joints use **three Hall effect sensors per joint** plus **one magnet** on the rotating output.

## Layout

```text
        min_limit          home              max_limit
            |                |                    |
            v                v                    v
  ---- forbidden ---- usable travel ---- usable travel ---- forbidden
```

- **One magnet** on the rotating member (adjustable carrier).
- **Three fixed Hall sensors** on the housing face (home, min limit, max limit).
- Do **not** use two magnets unless polarity/field pattern is distinguishable.

## Mechanical requirements (shoulder roll revA+)

| Requirement | Notes |
|-------------|-------|
| Sensor protection | Recess or guard; cable strain relief at housing exit. |
| Adjustability | Home sensor bracket or magnet carrier slotted ±3–5° for calibration. |
| Magnet gap | Follow vendor spec; document min/max air gap in BOM. |
| Min/max placement | Inside software travel limits by ≥5° (configurable backoff). |
| Home placement | Safe neutral pose; not within 15° of either limit sensor. |
| Hardstops | Optional mechanical stops independent of Hall sensors; sensors are not load-bearing. |
| Repeatability target | ≤0.5° home repeatability after calibration. |

## Electrical (planned)

| Signal | Direction | Notes |
|--------|-----------|-------|
| Hall output | Active-high default | Configurable per input in `homing.yaml`. |
| Pull-up | 10 kΩ to 3.3 V | Pi GPIO; verify with selected sensor (open-drain vs push-pull). |
| Wiring | Shielded pair near motor phases | Route away from high-current CAN/motor leads. |
| Connector | Document in [electrical/wiring/connectors.md](../electrical/wiring/connectors.md) | Pinout TBD when harness finalized. |

## GPIO mapping (draft — update when wired)

| Joint | home | min_limit | max_limit |
|-------|------|-----------|-----------|
| `shoulder_roll` | GPIO 23 | GPIO 24 | GPIO 25 |

Assign unique GPIO per joint when multiple joints are homed; avoid shared inputs.

## Commissioning

1. Power off; verify all three sensors read inactive with magnet away.
2. Move joint by hand; confirm each sensor toggles once per revolution arc (no double-trigger overlap unless configured).
3. Run `motor-repl sensor-check shoulder_roll` (when GPIO wired).
4. Calibrate `home_offset_rad` after first Hall homing sequence.
5. Record calibration in host registry (see [homing.md](../../docs/homing.md)).

## CAD checklist

- [ ] Three sensor pads on shoulder roll housing face
- [ ] Magnet pocket on output flange with adjustment slot
- [ ] Wire channel to Pi HAT / breakout
- [ ] Clearance for bearing play and printed tolerance
- [ ] No simultaneous trigger of home + limit at nominal home pose
