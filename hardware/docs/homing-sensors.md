# Homing sensors — mechanical and electrical

Shoulder pitch, shoulder roll, and future arm joints use **three Hall effect sensors per joint** plus **one magnet** on the rotating output.

## Layout

```text
        min_limit          home              max_limit
            |                |                    |
            v                v                    v
  ---- forbidden ---- usable travel ---- usable travel ---- forbidden
```

- **One magnet** on the rotating member (adjustable carrier).
- **Three fixed Hall sensors** on the stationary actuator/bracket or housing face (home, min limit, max limit).
- Do **not** use two magnets unless polarity/field pattern is distinguishable.
- For the shoulder pitch joint, prefer the Hall sensors fixed to the actuator face/bracket and the magnet fixed to the moving shoulder roll housing. This keeps the sensor harness stationary.

## Mechanical requirements (shoulder pitch/roll revA+)

| Requirement | Notes |
|-------------|-------|
| Sensor protection | Recess or guard; cable strain relief at housing exit. |
| Adjustability | Home sensor bracket or magnet carrier slotted ±3–5° for calibration. |
| Magnet gap | Start at 1-3 mm and tune empirically; document min/max air gap in BOM after testing. |
| Min/max placement | Trigger before mechanical hardstops with enough margin to stop; start with >=5 deg backoff and tune in config. |
| Home placement | Safe neutral pose; not within 15° of either limit sensor. |
| Hardstops | Optional mechanical stops independent of Hall sensors; sensors are not load-bearing. |
| Repeatability target | ≤0.5° home repeatability after calibration. |

## Selected sensor approach

Initial shoulder pitch homing uses low-cost `A3144` / `3144` / `OH3144` / `AH3144E` digital Hall switches.

Use these sensors as **digital references**, not continuous angle sensors:

- **Home**: startup reference used to establish the actuator's trusted zero.
- **Min limit**: negative travel sanity limit and recovery cue.
- **Max limit**: positive travel sanity limit and recovery cue.

The actuator remains the source of continuous position after homing; `home_offset_rad` maps the Hall home event to the semantic joint zero.

If buying a cleaner replacement later, prefer an **omnipolar, non-latching digital Hall switch module** or a **push-pull digital Hall switch**. Omnipolar sensors reduce magnet-pole mounting mistakes. Non-latching behavior is required so the signal clears when the magnet leaves. Avoid adding an absolute magnetic encoder unless the magnet can be mounted coaxially on the pitch axis.

## A3144 mounting notes

The A3144 package senses magnetic field mainly through the flat/marked face, perpendicular to that face.

With the flat/marked face toward you and legs down:

```text
left pin   VCC
middle pin GND
right pin  OUT
```

Bench-test each batch before final mounting:

1. Power the sensor.
2. Pull `OUT` high using the MCP23017 internal pull-up or an external 4.7-10 kΩ resistor.
3. Measure `OUT` to `GND`.
4. Bring a small neodymium magnet toward the flat face, then flip the magnet.
5. Mark the sensor face and magnet pole that make `OUT` switch active.

Typical A3144 behavior is active-low with the correct pole near the marked face:

```text
no magnet       -> output high
magnet detected -> output low
```

## Electrical

| Signal | Direction | Notes |
|--------|-----------|-------|
| Hall output | Active-low for A3144 | Configurable per input in `homing.yaml`. |
| Pull-up | MCP23017 internal pull-up preferred | External 4.7-10 kΩ to the MCP23017 I/O rail if wires are long/noisy. |
| Wiring | Shielded pair near motor phases | Route away from high-current CAN/motor leads. |
| Connector | Document in [electrical/wiring/connectors.md](../electrical/wiring/connectors.md) | Pinout TBD when harness finalized. |

Current bench expansion board: Waveshare MCP23017 IO Expansion Board.

- The board supports 3.3 V / 5 V operation and exposes 16 digital I/Os over I2C.
- A3144 outputs are open-collector/open-drain style, so they are compatible with MCP23017 inputs.
- Configure each MCP23017 pin as an input and enable its internal pull-up.
- Pull up to the MCP23017 I/O rail, not an unrelated rail. For a 3.3 V expander setup, the Hall output pull-up must be 3.3 V.
- Sensor `VCC` may be 5 V while `OUT` is pulled up to 3.3 V, as long as grounds are common and the selected sensor behaves open-collector.
- The PH2.0 6-pin connector is for I2C/power/interrupts; Hall sensors wire to the 16 GPIO pads/pins.

## GPIO mapping (draft — update when wired)

| Joint | home | min_limit | max_limit |
|-------|------|-----------|-----------|
| `shoulder_pitch` | TBD | TBD | TBD |
| `shoulder_roll` | GPIO 23 | GPIO 24 | GPIO 25 |

Assign unique GPIO per joint when multiple joints are homed; avoid shared inputs.

## Commissioning

1. Power off; verify all three sensors read inactive with magnet away.
2. Move joint by hand; confirm each sensor toggles once per revolution arc (no double-trigger overlap unless configured).
3. Run `motor-repl sensor-check <joint>` (when GPIO wired).
4. Calibrate `home_offset_rad` after first Hall homing sequence.
5. Record calibration in host registry (see [homing.md](../../docs/homing.md)).

For initial homing:

1. Move slowly toward the expected home edge.
2. Stop on the first active home transition.
3. Back off until the home sensor clears.
4. Re-approach slowly from the same direction.
5. Store that actuator position as the home reference and apply `home_offset_rad`.

This backoff/re-approach sequence reduces hysteresis and backlash error. Min/max sensors should stop motion and only allow recovery motion away from the triggered limit.

## CAD checklist

- [ ] Three sensor pads on actuator/bracket or stationary housing face
- [ ] Magnet pocket on moving shoulder roll housing/output flange with adjustment slot
- [ ] Wire channel to Pi HAT / breakout
- [ ] Clearance for bearing play and printed tolerance
- [ ] No simultaneous trigger of home + limit at nominal home pose
