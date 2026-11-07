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
  left = +X), Y 190..340 (+Y up, the top edge at Y 340). They match the torso only while the board sits at its floorplan
  position. Joey places the board in the upper cavity, so the frame thread applies one rigid transform to the whole package.
- **Z = 0 is the board's bottom face.** Components are on +Z (top face at Z 1.6) and the heatsink is on −Z (gap pad −0.5..0,
  base −6..−0.5, fins to −25).
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
| Limb pigtails J8, J10, J12 (14 AWG, 3.6 mm) | pads at X +45, Y 206–248 | +Z, then +X out of the side edge | R11 | `KO_limb_pigtails_right` |
| Limb pigtails J9, J11, J13 | pads at X −45, Y 206–248 | +Z, then −X | R11 | `KO_limb_pigtails_left` |
| Brake J3, J25 (Mini-Fit Jr, 18 AWG, 2.3 mm) | mated plug top, Z about 23.6, at X 29 and 37 | +Z, then +Y | R8 and up (nested) | `KO_brake_plugs_J3_J25` |
| 12 V converter J24 (Micro-Fit 2 × 6, 18 AWG, 1.9 mm) | plug top, Z about 17.6, X −37.5..−22.5 | +Z, then +Y | R6 and up | `KO_12V_converter_plug_J24` |
| 12 V outputs J5, J6, J7, J22, J23 (Micro-Fit) | plug top, Z about 17.6, Y about 194–197 | +Z, then −Y | 4 × OD and up | `KO_bottom_plugs_J5_J7_J21_J22_J23` |
| Pi link J21 (GH 7, 26 AWG, 1.0 mm) | plug back at Y about 185, Z 3.8 | −Y (side entry) | none on the board | same |
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
| L arm, R arm, waist | 14 AWG pigtail → inline 58 V MINI blade fuse holder (20 A, Littelfuse 997 class) → **Amass XT30(2+2)-F** on the pigtail (live side), joint at the frame step corner (arm-trunk agreement) | arm trunk XT30(2+2)-M | fuse holder about 40 × 20 × 15 mm (inline 58 V part still to choose); XT30(2+2) mated about 15 × 8 × 32 mm. The 2 signal pins carry the trunk's CAN from the Pi's USB-CAN adapter; this board carries no CAN |
| L leg, R leg (rev C) | 14 AWG pigtail → fuse → **Amass XT60** + separate JST GH CAN at the leg | — | XT60 mated about 16 × 8 × 32 mm |
| Aux | as arms | — | unfitted |
| Brake ×2 | Mini-Fit Jr 5557-02R plug | HS50 tabs (solder or ring) | plug about 10 × 10 mm, 22 mm above the board |
| 12 V converter | Micro-Fit 3.0 43025-1200 | the same plug on the buck board | plug about 21 × 10 mm |
| 12 V outputs | Micro-Fit 3.0 43025-0200/-0400 | loads | plug about 7 × 10 mm |
| Pi link, E-stop | JST GH GHR-07V-S / GHR-08V-S | Pi adapter; E-stop and reset buttons | plug about 10–11 × 4.25 × 6.5 mm |

Open item for the arm trunk: XT30 solder cups take 16 AWG at most, but the pigtails are 14 AWG. Either the pigtail steps down to
16 AWG at the fuse holder (fuse 20 A, wire ≥ 20 A at short length), or the joint moves to an XT60. **To be settled with the arm
trunk thread before cutting wire.**
