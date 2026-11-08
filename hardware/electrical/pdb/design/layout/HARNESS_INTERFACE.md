# Power board rev B5: harness interface

This is the hand-off from the power board to the torso harness (the frame thread's `harness.py`). It covers what leaves the
board, where it leaves, and what each run ends in. In-torso routing is the frame thread's job once Joey places the board.

| File | Content |
| --- | --- |
| [marengo_pdu_revb_package.step](marengo_pdu_revb_package.step) (LFS) | board with parts, option-D heatsink, `KO_*` keep-outs, `WIRE_*` stubs |
| [harness_wires.csv](harness_wires.csv) | wire list: one row per conductor group (net, gauge, OD, from/to, twist) |
| [harness_exits.csv](harness_exits.csv) | one row per `WIRE_*` body: exit point, exit direction, bend radius, direction after the bend |
| [tools/package_step.py](../../../schematics/marengo_pdu_revb/tools/package_step.py) | builds the STEP and `harness_exits.csv` from the board (`tools/conn_export.py` gives the connector data) |

## 1. Package frame (local origin and orientation)

- **X, Y are the board floorplan coordinates.** These are the same numbers as the torso frame docs: board X −50..50 (robot's
  left = +X), Y 170..340 (+Y up, the top edge at Y 340; 100 × 170 mm since option A, 2026-11-07). They match the torso only while the board sits at its floorplan
  position. Joey places the board in the upper cavity, so the frame thread applies one rigid transform to the whole package.
- **Z = 0 is the board's bottom face.** Components are on +Z (top face at Z 1.6) and the heatsink is on −Z (option D, chosen:
  thermal pad −0.5..0, 120 × 69 mm footprint at X −34.5..34.5, Y 196.5–316.5, base to −5.1, fins to −36.5). Air runs along Y;
  `KO_heatsink_air_inlet` / `_outlet` keep 20 mm free at both ends of the fins.
- **Units:** mm. All parts are on the top side; the bottom carries only the heatsink.

## 2. Exits

Every exit point and direction is in [harness_exits.csv](harness_exits.csv), with one row per `WIRE_*` body. The fields are:
- the start point (x, y, z);
- the initial direction (dx, dy, dz);
- the bend radius;
- the direction after the bend.

Each `WIRE_*` body runs 80–120 mm. Rows that bend the same way use concentric bends so they nest. Summary:

| Group | Exit | Leaves toward | Bend | Keep-out |
| --- | --- | --- | --- | --- |
| Pack pair J1 (8 AWG, 7.0 mm) | solder pads at (1, 335) and (15, 335), up from Z 1.6 | +Z, then +Y over the top edge | R15 | `KO_pack_8AWG_leads_R15_bend` |
| Limb pigtails J8, J10, J12 (arms, waist, aux: 16 AWG, 3.0 mm; legs: 14 AWG, 3.6 mm) | pads at X +45, Y 206–248 | +Z, then +X out of the side edge | 3 × OD (R9 / R11) | `KO_limb_pigtails_right` |
| Limb pigtails J9, J11, J13 | pads at X −45, Y 206–248 | +Z, then −X | 3 × OD | `KO_limb_pigtails_left` |
| Brake J3, J25 (Mini-Fit Jr, 18 AWG, 2.3 mm) | mated plug top, Z about 23.6, at X 29 and 37 | +Z, then +Y | R8 and up (nested) | `KO_brake_plugs_J3_J25` |
| 12 V converter J24 (Micro-Fit 2 × 6, 18 AWG, 1.9 mm) | plug top, Z about 17.6, X −37.5..−22.5 | +Z, then +Y | R6 and up | `KO_12V_converter_plug_J24` |
| 12 V outputs J5, J6, J7, J22, J23 (Micro-Fit) | plug top, Z about 17.6, Y about 194–197 | +Z, then −Y | 4 × OD and up | `KO_bottom_plugs_J5_J7_J21_J22_J23` |
| Pi link J21 (GH 8, 26 AWG, 1.0 mm) | plug back at Y about 185, Z 3.8 | −Y (side entry) | none on the board | same |
| E-stop loop J20 (GH 8, 26 AWG, 1.0 mm) | plug back at X about −48.5, Z 3.8 | −X (side entry) | none on the board | `KO_estop_plug_J20` |

Rev B5 change in this hand-off: J20 was turned 180° so its plug enters from the −X edge. It used to face into the board.

Mated heights (Micro-Fit about 16 mm, Mini-Fit Jr about 22 mm above the top face) are approximate. They come from the Molex
series, not from a drawing, and should be checked against the 43025 and 5557 drawings before the frame clearances are frozen.

## 3. Wire list

The full list is in [harness_wires.csv](harness_wires.csv): 21 rows with net, gauge, OD including the jacket, from, to,
length, twist and status. It also covers the off-board thermostat pair and the fan leads.

## 4. Connectors at the far ends

All dimensions marked "about" are approximate. Take the drawing before freezing the frame clearances.

| Run | Board end | Far end, part number | Mated envelope |
| --- | --- | --- | --- |
| Pack | 8 AWG soldered pair → **Amass XT90 (board half, solder cups take 8 AWG)** | **Pack-side XT90S half in a case-wall pocket** (battery main ec2513d): X +10..+45, Y 323..345, Z −45..−60 in torso coordinates, **mating face toward −X**, leads inside the case | about 21 × 11 mm section, about 55 mm mated length along −X/+X. The 8 AWG pair has to arrive along +X into the pocket face, so it needs space for an R15 turn before the plug (8 AWG plus the plug back is about 30 mm straight) |
| L arm, R arm, waist (aux the same) | **16 AWG** silicone pigtail, as in the arm trunk plan → inline 58 V MINI blade fuse holder (20 A, Littelfuse 997 class) → **Amass XT30(2+2)-F** on the pigtail (live side), joint at the frame step corner (arm-trunk agreement) | arm trunk XT30(2+2)-M | fuse holder about 40 × 20 × 15 mm (inline 58 V part still to choose); XT30(2+2) mated about 15 × 8 × 32 mm. The 2 signal pins carry the trunk's CAN from the Pi's USB-CAN adapter; this board carries no CAN |
| L leg, R leg (rev C) | 14 AWG pigtail (kept heavier for the leg ports, rev C) → fuse → **Amass XT60** + separate JST GH CAN at the leg | — | XT60 mated about 16 × 8 × 32 mm |
| Aux | as arms | — | unfitted |
| Brake ×2 | Mini-Fit Jr 5557-02R plug | HS50 tabs (solder or ring) | plug about 10 × 10 mm, 22 mm above the board |
| 12 V converter | Micro-Fit 3.0 43025-1200 | the same plug on the buck board | plug about 21 × 10 mm |
| 12 V outputs | Micro-Fit 3.0 43025-0200/-0400 | loads | plug about 7 × 10 mm |
| Pi link, E-stop | JST GH GHR-08V-S (both) | Pi adapter; E-stop and reset buttons | plug about 11 × 4.25 × 6.5 mm |

**Arm, waist and aux pigtails are 16 AWG** (coordinator 18:55Z, matching
[wiring/left-arm-trunk](../../../wiring/left-arm-trunk/README.md): 16 AWG trunk power on XT30, GH1.25 CAN). The XT30 cups
take 16 AWG, and the board's SolderWire-2.5sqmm pads (2.4 mm hole, 3.6 mm relief) take it unchanged. Check, for a twisted
pair of 16 AWG silicone (13.2 mΩ/m, 3.0 mm OD), in still air with about 12 W/m²K from the surface and a 1.3 factor for the pair:

| Case | Current | Temperature rise | Verdict |
| --- | --- | --- | --- |
| Expected arm load (drop fuses 7.5 + 7.5 + 5 + 5 + 3 A; real draw is a few amps) | 5 A | about 4 K | fine |
| Heavy arm load | 10 A | about 16 K | fine |
| Software trip limit (sustained) | 15 A | about 39 K → about 80 °C at 40 °C torso air | fine for 200 °C silicone; XT30 contacts are the limit, as the trunk plan says |
| Fuse rating, software failed | 20 A | about 79 K → about 120 °C | survivable (silicone 200 °C), not a running point; the 20 A fuse clears a few × 20 A within seconds |
| Port overload trip (TPS48111, 30 A, about 16 ms) | 30 A | 0.04 K (adiabatic) | negligible |
| Short at the pigtail end (1.1–1.4 kA, under 2 µs) | — | 0.01 K | negligible |

**Pi link J21 pinout** (JST GH 8, 26 AWG; rev B5 added pin 8 for the power-fail warning). Every Pi input here is
open drain or driven at 3.3 V from the Pi's own rail; nothing above 3.3 V reaches the cable.

| Pin | Net | Direction | Pi side |
| --- | --- | --- | --- |
| 1 | +3V3_PI | Pi → board | Pi 3.3 V: meters' supply, I²C and NO pull-ups |
| 2 | I2C_SDA | both | dedicated Pi I²C controller, Pi pull-ups |
| 3 | GND | | |
| 4 | I2C_SCL | Pi → board | Pi pull-ups |
| 5 | ESTOP_PI_N | board → Pi | GPIO input, no pull: low = E-stop pressed |
| 6 | WD_FEED | Pi → board | GPIO output, heartbeat toggled from the control loop |
| 7 | PORT_FLT_N | board → Pi | GPIO input, 10k pull-up on the board: low = a port latched off |
| 8 | PWR_FAIL_N | board → Pi | GPIO input **with a pull-up to the Pi's 3.3 V** (10k, or the internal pull-up): low = converter input under 31.4 V (re-arms at 34.4 V) |

The CAN pair for the XT30(2+2) signal pins doesn't come from this board. It comes from the Pi's USB-CAN adapter and joins at
the pigtail's XT30(2+2)-F.
