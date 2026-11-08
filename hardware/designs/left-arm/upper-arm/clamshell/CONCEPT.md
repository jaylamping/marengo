# Upper arm clamshell: concept (not CAD yet)

2026-11-08. This is a concept for Joey to look at before any detailed CAD. It is cut straight from the printed db71b23b mesh (`prints/upper-arm/cad/upper_arm.stl`) with mesh booleans, so the shapes are rough. The masses are solid PETG at 1.27 g/cm³.

Images: `upper_arm_clamshell_concept.png` shows each part in its print orientation, from above and from below, with a support preview. `upper_arm_clamshell_exploded.png` shows the parts exploded. The numbers are in `clamshell_concept.json`. Regenerate with `clam_concept.py` and `support_check.py`, from the left-arm task folder on Joey-PC.

## Why it changes

The db71b23b print came out rough on the supported outer surfaces. The support in the closed cavity couldn't be cleared through the door, and the part weighs 202 g. Joey's rules from now on:

1. No support may touch a visible outer surface.
2. The arm is two clamshells fastened together, with no trap door.

## Split

| Part | Print orientation | Solid mass | Support area, total / on outer skin |
| --- | --- | --- | --- |
| Upper half (+Z) | Split face (Z 6) on the bed | 50.0 g | 2716 / 52 mm² |
| Lower half (−Z), with the seam rib and trunk channel | Split face on the bed | 40.4 g | 1992 / 0 mm² |
| RS02 seat ring, separate | Flat, elbow axis vertical | 33.9 g | 2044 / 27 mm² |
| Hub disc, separate, lightened | Flat, RS02 hub face on the bed | 56.9 g | 12 / 0 mm² |
| **Total** | | **181 g solid**, about **162 g printed** at the db71b23b ratio (202 g printed / 226 g solid), plus about 10 g of inserts and screws | |

For comparison, the one-piece db71b23b is 226 g solid. Its best orientation still puts 32–562 mm² of support on the outer skin and about 4400 mm² inside the closed tube.

- **Split on Z 6 (the XY plane through the elbow axis).** Bending stress runs along the arm (Y). A lengthwise seam lies parallel to it, so bending never pulls the seam open; the seam carries shear only.
  - The 5 kg sweep bends the arm about X, so Z 6 is close to the neutral plane.
  - Each half prints split face down. The outer skin then faces up with no support on it. The cavity is open to the bed, so any inside support can be reached and removed. Layers run along the arm, which is strong in bending.
- **Seat ring and hub disc are separate parts, each printed flat.** Split in half, the ring would print as an arch with support inside the RS02 bore, and that bore is the seat Joey says fits well. Printed flat it is round with no support. The hub disc printed flat keeps the 6 M4 holes on the RS02 hub accurate.
  - The db71b23b hub is a solid disc, 17 mm thick, which is about 100 g of the 226. The concept keeps a 6 mm plate, Ø10 bosses round the M4 holes and the tube rim. That is where most of the weight saving comes from.
- **Joints** (detailed CAD, not in the concept):
  - Halves: M3 screws along Z into Ruthex inserts, on a seam flange inside the wall, with a tongue and groove for shear.
  - Seat ring: captured between the halves, with its ears bolted through.
  - Hub disc: rabbeted into the tube end and bolted from inside before the halves close.
  - The bolted-joint FEA at the upper-arm load cases comes with the detailed CAD.
- **Trunk channel, internal: yes.** It is 10 × 4 mm, the 2 × 16 AWG pair laid flat beside the 26 AWG CAN pair. It runs on the seam, on the −X inner wall, in a rib that is also the seam flange for the inserts, so it costs about 5 g. It sits on the neutral plane of the sweep bending, so it doesn't cut the stressed fibres.
  - Route: the hub end, down the wall, then a turn into the RS02 port slot.
  - The mini-blade fuse holder and the splice sit in wall cradles beside it.
  - There are strain-relief tie bars at both ends.
  - Assembly: lay the cable in the lower half, then close the upper half over it.

## Seam orientation: Z split vs X split (2026-11-08)

The two options:
- **Z split:** the plane through the elbow axis, Z 6. The seam lines run along the ±X sides of the tube.
- **X split:** the plane across the elbow axis, like the forearm. Taken at X 165.2 so the plate ears land on the bed. The seam lines run along the top and bottom of the tube.

Shear flow assumes a thin round tube, R 38 mm, at 5 kg. That gives V = 59 N, including the forearm. The yaw RS02 peak torque is 17 N·m.

| | Z split | X split at X 165.2 |
| --- | --- | --- |
| Pitch-sweep shear flow at the seam (V/πR at the neutral plane, 0 at the extreme fibres) | **0.49 N/mm** (the maximum) | **about 0** |
| Lateral-case shear flow at the seam | about 0 | 0.49 N/mm |
| Twist shear flow (17 N·m / 2A), the same everywhere | 1.87 N/mm | 1.87 N/mm |
| Worst seam shear flow | **2.4 N/mm** | 2.4 N/mm (lateral + twist), plus the load transfer below |
| Load transfer between the halves | The seat ring is captured by both halves, so each takes its own share | The ring ears are all in the −X half. About 9.6 N·m (half the 19 N·m at Y 250) must cross the seam within about 40 mm of the plate root, roughly **4 N/mm locally** |
| Support on the outer skin | 0 on the lower half, about 50 mm² on the upper half. That is two concept artefacts: the channel-end stub and tessellation slivers at the hub cut. Both come out in the CAD | 0 on both halves |
| Trunk channel | On the seam, on the neutral plane of the pitch sweep, so the rib doubles as the seam flange | The seam lines are the top and bottom extreme fibres of the pitch bending. The channel can't share the seam, so it needs its own rib inside one half, and the split doesn't capture it |
| Print | Both halves cut face down. Layers run along the arm | The same, and the plate ears sit flat on the bed |

**Decision: keep the Z split.**
- Its seam shear flow is the higher of the two for pitch, but only 2.4 N/mm at worst.
- It keeps the elbow load path symmetric: the ring is captured by both halves.
- It puts the channel and the seam flange on the neutral plane, as Joey asked.
- The X split's one advantage is a clean outer skin, and the Z split gets that too once the two concept artefacts are cleaned up in CAD.

**Seam sizing for the Z split.** With SF 2, the design shear flow is 4.8 N/mm. Over the 50 mm of tube seam per side, that is 240 N per side.
- Ruthex M3 inserts at 25 mm pitch, 3 per side: about 80 N per insert in shear, well under the pull-out and shear capacity of an M3 insert in PETG (several hundred N).
- Tongue and groove, 2 × 2 mm: the shear plane is 2 mm²/mm. At the 22.5 MPa layer allowable that is 45 N/mm, about 9× the design flow. The tongue locates the halves and takes the shear, so the screws mostly clamp.
- The seam FEA with the bolted joint, at the upper-arm load cases, comes with the detailed CAD.

## RS02 tab pocket (Joey's fit report on db71b23b)

The pocket in the Ø66 back bore clears the RS02's lower plug housing:
- Housing (vendor model): X 159.5–165.0, out to r 36.1, 164–190°.
- Pocket: floor at r 36.9–38.0, 161–191°.

In db71b23b the pocket stopped at X 163.49. That left a lip 1.70 mm long, at r 33–37, between the pocket and the seat face (X 165.19). The vendor housing overlaps that lip by 0.84 mm axially, and Joey had to remove about 1 mm to get the RS02 in.

The fix in the clamshell rev is to run the pocket through to the seat face over 161–191°, so the lip is gone. That lip is not seat contact, because the plug housing occupies that space. The shoulder still bears on the pocket floor to r 41 there, and on the full ring everywhere else. The seat face loses about 4% of its area.

The pocket roof gets a chamfer so it prints without support in the flat ring orientation. The other RS02 reliefs (the 9-hole pattern reliefs at 152–161° and 191–200°, and the port slot) get the same treatment in the detailed CAD.

## Open before detailed CAD

- Joey to approve the split plane, the separate ring and hub, and the internal trunk channel.
- Mesh gate and slice check: add the outer-skin support rule using `support_check.py`, judged against the assembled arm, so faces that face a neighbouring part count as hidden.
