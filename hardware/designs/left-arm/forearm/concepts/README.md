# Left forearm shape concepts

Step 1 of the forearm redesign: three shapes for Joey to choose from. These are **outer envelopes only** around the fixed interfaces. They have no walls, pockets, joints or FEA yet, so they are not a design.

- `concepts_compare.png`: all three side by side, iso, side and front, with the elbow at 0°.
- `concept_A.png`, `concept_B.png`, `concept_C.png`: each concept in iso, side and front at elbow 0°, iso and side at 90°, and a ghost side view. In the ghost view, the green cylinder is the 58 mm clear bore and the red section is the part of the requested bore that overlaps the RS00. The orange line is the cable route.
- Colours match the elbow diagrams. The printed parts are white, the actuators grey, and the test-weight stack is the adapter, an M8 stud and two 250 g pucks.

## Fixed in every concept

- Fork interface to the elbow RS02: the cheeks stay at X 131.7–139.7 and 197.19–207.19 with r 29 discs. The outboard cheek position is a parameter (`DX`, the second argument to the script) until the RS02 caliper check comes back.
- Between the cheeks the body stays outside r 47.5 about the elbow axis, the same as the current fork bridge. This clears the hoop (r 42.3) and the carrier (r 42.0).
- The RS00 stays where it is: output face at Y 89.9 (120 mm below the elbow), rear at Y 141.3.
- The yaw split is at the RS00 output face. Below it, every section is round about the yaw axis, so the turning distal tube looks seamless at any yaw angle.
- Length is 195 mm, elbow axis to wrist face at Y 14.86.
- Wall below the RS00 is at least 31.5 mm radius over the bore length, giving 2.5 mm, which is 6 walls, around a 58 mm bore.

## The three concepts

| Concept | Look | Mass (envelope estimate) | Print orientation | Strength path | Cables |
| --- | --- | --- | --- | --- | --- |
| **A Clean taper** | Smooth single body. Cheeks melt into a slightly flattened bulb that narrows to a round tube. Closest to the upper arm's plain tapered tube. | about 210 g | Fork vertical, cheeks up, like the current fork. The rounded front shelf needs tree supports. | Closed round section from the cheeks down. Good in twist. The cheek-to-tube neck is the weak spot, as today. | Inside the back wall beside the RS00. Only about 1.5 mm between the RS00 (r 28.5) and the wall, so a flat ribbon at best. **Tightest.** |
| **B Muscle belly** | Wide across the elbow axis just below the elbow (88 mm), then tapers to 64 mm, like a real proximal forearm. The strongest human taper. | about 215 g | Same as A. The belly sides overhang about 45° and are self-supporting. | Widest section where the bending moment and twist peak (gusset ends near Y 150). **Best chance of reaching SF 2.0 / layer 1.5.** | In the outboard belly, outside the RS00, with room for the RS00 fuse pocket and a service cover in the belly. **Most room.** |
| **C Ulna ridge** | A's taper plus a raised ridge down the back, rising into an olecranon spur behind the elbow. | about 220 g | Same, but the ridge and spur need supports on the back. | Ridge adds depth on the tension side in fore-aft bending. The spur is a bridge between the cheeks behind the RS02. | Inside the ridge, straight from the elbow loop. The spur doubles as the printed guide and guard for the rolling ribbon loop. |

Mass is an estimate: envelope area × 2.5 mm skin plus solid cheeks, PETG at 1.27 g/cm³. It is a comparison only, not a weight.

## Checks on the envelopes (`concept_checks.json`)

| Check | A | B | C |
| --- | --- | --- | --- |
| Gap to upper arm + carrier, elbow 0 / 60 / 90° | 0.8 mm | 0.8 mm | 0.8 mm |
| Gap at the 124° stop | 0.8 mm | 0.8 mm | **0.25 mm** |
| Overlap at 130° | 4.2 mm | 4.2 mm | 4.2 mm |
| RS00 room inside the envelope, min | 1.7 mm | 1.7 mm | 1.7 mm |
| Wall around the 58 mm bore, min | 2.6 mm | 2.6 mm | 2.6 mm |

- The 0.8 mm gap at 0–124° is the cheek disc in the arm slot, the same as the current fork.
- **130° does not clear.** The front of the body has to cover the RS00's rear corner (Y 141, z 34.5), and that corner hits the upper arm past about 126°. The current fork has the same limit (1.9 mm at 130°). The stop stays at 124°. Reaching 130° means moving the RS00 down about 8 mm or chamfering its rear-corner cover.
- RS00 room of 1.7 mm is at that same rear corner. The detailed design needs 2.5 mm there, so the front shelf moves forward a little and the 124° gap will shrink.
- The front is cut back to z 10.5 between Y 146 and 172 in all three concepts, as on the current fork. This is the "cubital" hollow that lets the elbow fold.

## Hand pack bore: does not fit as briefed

The brief asks for a 58 mm bore about 100 mm long with its CoM 140 mm from the elbow axis. With the RS00 output at 120 mm and the wrist at 195 mm, only **72 mm** of tube lies below the RS00 (Y 17.4 to 89.9). The CoM of that space is about 156 mm from the axis. The requested bore would run 30 mm into the RS00 (red in the ghost views).

Options for Joey:

1. **Keep 195 mm:** 72 mm bore. That holds about 2/3 of the 12-servo pack, and the rest goes in the hand.
2. **Lengthen the forearm to about 223 mm:** 100 mm bore, CoM about 170 mm from the axis. More elbow and shoulder torque.
3. **Move the RS00 toward the wrist:** a bigger change, and it brings back the fold and the fork redesign.

This only changes the distal tube length. All three shapes work with any of these.

## Test weights (shown on every render)

The renders show a wrist adapter on the wrist flange (6 × M3 on Ø50; the RS00 output itself is 6 × M3 on Ø27, and Ø50 is its housing) with a captured M8 stud along the forearm axis. On the stud are two 250 g pucks (Ø60 × 30, PLA, filled and weighed) and an M8 nut. The stack CoM is about 40 mm past the wrist face, 235 mm from the elbow axis.

**First-pass torque estimate**, arm straight out horizontal. These use the URDF masses of the current forearm and hand links and vendor rated torques (RS02 6 / 17 N·m, RS03 20 / 60 N·m, RS00 5 / 14 N·m, see `../../../hand/RESEARCH.md`):

| Joint | With a 500 g stack | × 1.5 dynamic | Rated | Max stack at rated / 1.5 |
| --- | --- | --- | --- | --- |
| Elbow RS02 | 2.7 N·m | 4.0 N·m | 6 N·m | about 1.08 kg |
| Shoulder pitch RS03, full reach | 7.5 N·m | 11.3 N·m | 20 N·m | about 1.8 kg |
| RS00 yaw | about 0, stack on the axis | | 5 N·m | Output bearing moment about 1.0 N·m; the RS00 manual publishes no bearing rating |

The RS02 and RS03 margins still hold at full horizontal reach with 500 g. The number printed on the adapter will be the lower of the torque limit and the forearm FEA limit. **The current fork fails FEA already** (fore-aft SF 1.27, twist layer 0.9; see `../../ELBOW_COMPACTION.md`), so no stack goes on the arm until the new forearm passes.

## Next, after Joey picks

Detailed CAD in SolidWorks:

- printed 6808 inner-race retention;
- RS00 fuse pocket and service cover;
- cable route and rolling ribbon loop with a printed guard;
- FEA to SF 2.0 / layer 1.5 on fore-aft, twist, lateral, and the 500 g stack and pack cases, with a convergence check;
- print setup (6 walls, 50% with 100% modifiers, tree supports, Super Tack, white PETG), mesh gate, picture sheets, and a two-reviewer gate;
- test-weight adapter and pucks, and `../../testing/CALIBRATION_LOOP.md` mass/CoM bookkeeping.

## Regenerate

```
python forearm_concepts.py OUT_DIR [outboard_cheek_shift_mm] [concepts_to_rebuild]
```

Needs `elbow_diagram.py` and its meshes from the left-arm task folder, plus numpy, trimesh, scipy and scikit-image. It takes about 15 minutes for all three.
