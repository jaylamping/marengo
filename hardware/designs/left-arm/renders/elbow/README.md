# Left elbow assembly diagrams

2026-11-07. Generated from the current meshes in arm coordinates by `elbow_diagram.py` (in this folder). The upper arm, carrier and cover are the released files at db71b23b. **The forearm is not released.** The cable loop and guard are **planned, not designed**.

- `ELBOW_ASSEMBLY.png`: iso, front (outboard side) and back (inboard side), elbow at 0 degrees.
- `ELBOW_EXPLODED.png`: exploded along the elbow axis (X), with the forearm parts dropped along Y, and exploded along the hub axis (Y), with the cover off along Z.

Colours: printed upper arm, carrier and cover white; actuators and screws grey; washers gold; inserts bronze; forearm fork and distal tube blue; fuse holders black; wiring orange. Orange dashes and the translucent orange shape are placeholders.

## Bill of materials in the diagrams

| Item | Qty | Notes |
| --- | --- | --- |
| Upper arm, printed PETG | 1 | `prints/upper-arm/upper_arm.3mf`, db71b23b |
| Carrier, printed PETG | 1 | `prints/upper-arm/carrier.3mf` |
| Cable-window cover, printed PLA | 1 | `prints/upper-arm/cover.3mf` |
| Elbow actuator, RobStride RS02 | 1 | Shoulder on the hoop seat at X 165.19, plug through the port slot |
| Upper-arm yaw actuator, RobStride RS02 | 1 | Hub end only, output on the hub at Y 319.86 |
| Forearm yaw actuator, RobStride RS00 | 1 | In the fork skirt. **Forearm not released** |
| Forearm fork and distal tube, printed | 1 each | **Not released** |
| 6808-2RS bearing, 40 × 52 × 7 | 1 | In the carrier seat, X 143.0 to 150.0, on the fork stub |
| M3 × 25 button head, cut to 24 | 6 | Hoop screws, carrier and hoop to the RS02 shoulder, 0.35 N·m, dry |
| DIN 6796 M3 conical spring washer | 6 | Under each hoop screw head, cone to the head |
| DIN 9021 M3 washer | 9 | 6 in the carrier seats under the hoop screws, 3 under the bearing retainers |
| M3 × 6 button head | 7 | 3 bearing retainers, 4 cover screws |
| M3 heat-set insert | 4 | Cover bosses |
| M4 × 30 socket head | 6 | Hub to the yaw RS02 output, 0.6 N·m, dry |
| DIN 125 M4 washer | 18 | Three under each hub screw |
| M4 × 14 button head | 6 | Fork to the elbow RS02 output, bought with the forearm |
| Inline mini-blade fuse holder | 2 | Hook and loop, in the pockets opposite the window |
| Fused trunk: XT30 power + CAN | 1 run | Shoulder, cable window, fuses, port slot, elbow RS02 plug. Path drawn approximately |
| Rolling ribbon loop and guard | | **Planned, not designed.** Drawn as an orange dashed path and a translucent draft outline |

No thread-locker anywhere during testing: see `../../prints/upper-arm/ASSEMBLY_ORDER.md`.

## What is a placeholder or approximate

- The cable loop and guard: shape and route are not designed.
- Trunk and lead routes: drawn to show the path, not to length.
- Fuse holders: boxes the size of the pocket.
- Screws: simple cylinders at the real positions and lengths.

## Open item: measure the real RS02 before the forearm is printed

The RS02 vendor 3D model and the vendor drawing disagree on where the mounting shoulder sits:

- The drawing (`../../../../docs/robstride/RS02_User_Manual_260713.pdf`, page 8) gives the Ø78.5 body as 28 mm long, with the Ø43 output disc 3.5 mm proud of its front. That makes the shoulder-to-output distance 31.5 mm, or 32.0 mm with the 0.5 mm step.
- The 3D model has the body 30.4 mm long, which makes it 33.9 mm.

The hoop seat (X 165.19) and the fork's outboard cheek (X 197.19) are 32.0 mm apart, so they follow the drawing. If the model is right, the RS02 output sits about 1.9 mm into the fork cheek. In these diagrams the RS02 is placed by the model on the hoop seat, so that overlap is drawn but hidden inside the fork.

**Measurement:** with calipers, measure from the **mounting shoulder** to the **output face**.

- Mounting shoulder: the flat ring with the nine M3 holes, where the big Ø78.5 body steps down to the smaller Ø65 back.
- Output face: the face of the raised Ø43 output disc with the six M4 holes.
- Use the depth rod, or put the caliper jaw across the output disc and measure down to the shoulder ring.

If it reads about **31.5 to 32.0 mm**, the fork is right as drawn. If it reads about **33.9 mm**, the fork cheek must move out about 2 mm before the forearm is printed. The upper arm does not change either way.

## Self-check

- Every BOM line above is labelled in at least one view. A part is labelled in a view only where at least 25 of its pixels are visible.
- Part-to-part overlap was checked by sampling 15,000 points per part. Clear: RS00 to fork and distal tube, fuse holders to arm, fork to carrier, distal tube to fork. The bearing and the yaw RS02 only touch their seats.
- Two overlaps remain, and both come from the RS02 model-versus-drawing difference above:
  - the elbow RS02 output disc runs 1.9 mm into the fork cheek;
  - its lower plug housing runs 1.1 mm into the hoop at the lower port relief.

  If the real RS02 matches the drawing, the whole motor sits about 2 mm further into the bore than the model shows, and both clear. The caliper measurement settles it.
