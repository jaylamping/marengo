# Left upper arm print order

2026-11-07. **Not released yet: the pre-print review comes first.** Don't fit these parts to the robot until it passes.

Settings are the test pass 1 process: P2S, 0.4 mm High Flow, Generic PETG, 0.20 mm, 6 walls, 100% infill, normal supports, outer 5 mm brim. See `../test-pass-1/SETTINGS_CHECKLIST.md`. Open each file as a project, don't rotate anything, and check the settings before printing. Each plate matches the P2 L04B project except the brim. No slicer warnings on any plate.

| Order | File | Print time | Filament | Down on the plate | Why |
| --- | --- | --- | --- | --- | --- |
| 1 | `carrier.3mf` | 1 h 12 m | 31.0 g | Inboard face | Same way up as the strength carrier in test pass 1 |
| 2 | `cover.3mf` | 48 m | 4.4 g | Standing on its end | Thin curved shell, so no support on the seating rim |
| 3 | `upper_arm.3mf` | 8 h 24 m | 264.6 g | Inboard side, hoop flat | Layers across X, as in the FEA. The RS02 seat at X 165 faces up, clean |
| 4 | `p2.3mf` | 5 h 54 m | 188.8 g | As P2 L04B, same 45 degree plate rotation | Only if you want a new P2, see below |

Total: about 15 h 18 m and 489 g, including support and brim. Without P2, 9 h 24 m and 300 g.

## P2

P2 L05 is P2 L04B with 2.5 mm off the inboard tip. It clears the M4 heads on the r 49 collar ring, so the inward roll limit can go from 2 to 5 degrees. **You don't have to reprint it.** You can file 2.5 mm off the tip of the P2 you have, flat, over Z 8 to 28, and get the same clearance. The reprint uses the same process and orientation as L04B, 6 minutes longer in the slicer and 0.2 g heavier.

## Where support lands

Each is from the sliced G-code.

| Part | On support | Keep clear |
| --- | --- | --- |
| Carrier | Recess roof, retainer pockets, under the head pockets | **A support blocker keeps support out of the bearing seat.** The Ø49.5 to 52 ledge prints unsupported, so check it is flat. The outboard ring face and the ribbon slot print up, clean |
| Upper arm | **Hoop inboard face at X 151.7**, tube inside, port slot edges | Motor bore, RS02 seat at X 165, hub face at Y 320, cover recess |
| Cover | Almost none | Seating rim |

The hoop inboard face can't avoid support in this orientation. Flipping the arm moves support onto the RS02 seat instead, and adds 13 g and 23 minutes. Sand the hoop face flat around the six sleeve holes before fitting the steel ring.

## After printing

- **Carrier:** clear the recess and pockets. Check the race slides into the seat.
- **Upper arm:** take the tube support out through the cable window and port slot in small pieces. Check the bore with the RS02 before fitting anything.
- Parts list: `PARTS_LIST.md`. Assembly: `ASSEMBLY_ORDER.md`. Strength: `FEA_SUMMARY.md`. Hashes: `SHA256SUMS.txt`.
