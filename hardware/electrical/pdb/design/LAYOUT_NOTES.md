# Power board rev A: layout step 1 notes

2026-11-06. **The captured schematic does not fit the two boards.** Board setup is done. Placement stopped at a fit test, so nothing here is a finished placement, and routing should not start.

Boards are built by `tools/pcb_place.py` from the netlist, into `schematics/marengo_pdu/pcb/`. Board x = robot X + 150, board y = 400 - robot Y. Component face towards the viewer, so robot left is on the right, as in `FLOORPLAN.png`.

Renders: [HP front](layout/hp_fit_front.png), [HP rear](layout/hp_fit_rear.png), [LP front](layout/lp_fit_front.png), [LP rear](layout/lp_fit_rear.png). Parts beside the board are the ones that did not fit.

## Fit test

Main path parts and terminals are hand-placed. Everything else goes to the nearest free spot, courtyard touching courtyard, on both faces, with no room for routing or vias. That is the best case, so an unplaced part is a hard no-fit. Parts through-holed or over 5 mm stay on the front. Rear parts use the 6 mm gap to the carrier.

Area available is 19,000 mm² on HP and 10,600 mm² on LP, two faces.

| Case | HP courtyard | HP fill | HP unplaced | LP courtyard | LP fill | LP unplaced |
|---|---|---|---|---|---|---|
| As captured | 24,662 mm² | 130 % | 318 of 649 | 11,242 mm² | 106 % | 317 of 607 |
| DNP parts removed (legs and spare footprints) | 18,787 mm² | 99 % | 142 of 517 | 11,202 mm² | 106 % | 308 of 599 |
| Also all 10 mm caps removed | 15,372 mm² | 81 % | 125 of 494 | 8,232 mm² | 78 % | 262 of 579 |

A routable power board is near 60 % fill, my working figure rather than a calculation. That is about 17,800 mm² across both boards against 30,000 mm² populated, so about 12,000 mm² must go, or the area must grow.

Largest populated items:

| Item | Qty | Area |
|---|---|---|
| 100 µF 80 V bulk | 23 | 3,415 mm² |
| 68 µF hold-up | 20 | 2,970 mm² |
| PWR263S resistors (clamp, CAN termination, precharge) | 14 | 2,670 mm² |
| M4 REDCUBE terminals | 14 | 2,430 mm² |
| Micro-Fit rail outputs | 11 | 896 mm² |

The floorplan's boxes were estimates from before capture. Gate 3 checked the circuit, not the area.

## Levers

1. **Take the leg channels off the board.** Already counted in the second row. Legs then need a later board. The main path keeps its leg-ready rating.
2. **Move the 46 caps to their own board or to the frame.** Saves 6,385 mm² and fixes the height problem below. Needs a location from the frame thread. Row 3 shows this alone is not enough.
3. **Get more height.** Frame front face is Z 70, ceiling is Z 61.5, and the board looks out between the posts from Y 175 to 353.6. Up to Z 67 would stand the 17 mm caps, and taller cans mean fewer. Frame thread must confirm, including shell, airflow and trunks.
4. **Get more board.** LP might extend down towards Y 176, about 1,400 mm² a face, with its rear 0.5 mm into the brake resistor's Z range and plug room lost. Unchecked.
5. **Smaller PWR263S parts.** Clamp and termination power were sized in gate 3, so this needs recalculation.

Recommendation: 1 and 2, then rerun the fit, then 5 if it is still over 70 %. All of these change the schematic, so gate 3 needs a pass on the changes.

## Constraints

| Constraint | State |
|---|---|
| HP 100 × 95, 4 layers 2 oz. LP 100 × 53, 6 layers 1 oz | Met, separate files. Layer count differs, so no shared file |
| 60 V clearance | 0.6 mm outer and 0.2 mm inner between HV classes and other nets, in `.kicad_dru`. IPC-2221 B2 and B1, 51 to 100 V |
| Current widths | HV_Port 6.1 mm, HV_Main 16 mm, Rail12 6.4 mm, Rail6 2.3 mm. IPC-2221 at 20 °C rise. 80 A needs about 42 mm on one outer 2 oz layer, or 108 mm inner, so the main path is pours on several layers |
| Corner keep-outs for parts over 5 mm | Met in the fit test |
| Arm M4 terminals inboard of \|X\| 44 | Met: edge at \|X\| 42.4, centres ±35.8 and ±22.6 |
| Pack lug envelope | Met: J1 and J3 at X ±9.4, Y 333, inside the X -35 to 30 run |
| **Ceiling 12.4 mm** | **Not met.** The 23 HHXF 100 µF are 16.5 to 17 mm tall (log, and DigiKey's listing for sibling HHXF800ARA101MJH0G). About 15 mm for the 3568 mini fuse holders with a fuse in: inferred from a distributor size, not a drawing. Vertical Micro-Fit and 2 × 15 ribbon headers need their plug and cable height checked |
| **LP connectors at \|X\| > 36** | **Not met.** That leaves two 14 mm edge strips, against 11 Micro-Fit at 8.2 mm wide and 3 fan headers at 11.2 mm, before the JST connectors |
| **Side pigtail bands** | **Not met.** Charge, waist and R leg all leave at -X, aux, L leg and brake at +X. Section 1b fits one arm bundle in a band, and no arm plus leg together |
| B0 to VSS return under 1 mΩ | **At risk off the board.** RS1 sits beside J3, so board copper is small. My estimate of 150 to 250 mm of 10 AWG at 3.28 mΩ/m is 0.5 to 0.8 mΩ, before the XT90 contact. Measure it, or take B0 at the board |
| Kelvin shunts | RS1 placed. RS201 and RS401 landed on the rear, away from their FETs: not final |
| Hold-up area, clamp heatsinking | Not resolved. Q401, R209, R318 and F501 did not fit |

DRC with errors only: no courtyard overlaps and no shorts. Clearance errors (HP 99, LP 1) and LP hole clearance (8) remain, and are expected from touching courtyards.

## Standoffs, robot coordinates

M3, 3.2 mm hole. Keep the frame's six, add two so LP is not cantilevered 50 mm from its bottom edge.

| Board | X | Y | State |
|---|---|---|---|
| HP | ±46 | 336 | Frame's |
| HP | ±46 | 265 | Frame's |
| LP | ±46 | 194 | Frame's |
| LP | ±46 | 239 | **New**, carrier change |

All are at the edge, away from the centre main path. A centre post under the terminals is worth checking once placement is real. Rings are on no net: the frame bond point waits on the RS03 housing check.

## Not done

Pi stub and pack sense boards. Schematic sync is by script, since these boards have no schematic of their own: rerun `pcb_place.py` after any netlist change. Output carries new UUIDs each run.
