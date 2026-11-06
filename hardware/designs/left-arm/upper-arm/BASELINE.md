# Upper arm baseline

2026-11-05. Rev A is the plain round tube. Rev B fixes its three failing joints. The load cases are fixed for every iteration.

## Result

**Rev B carries the fixed load case on the 3 mm mesh, but two M3 holes fall short once the mesh is refined.** Peak safety factor is 2.08 against a target of 2. It drops to 1.73 with local refinement at the bolt holes, and tension across layers reaches 1.36 to 2.5 against a target of 1.5. The rest of the part is well above target. The stress at those holes rises with mesh density, so it depends on the rigid bolt model and needs the bench test to settle.

| Item | Rev A | Rev B |
| --- | --- | --- |
| Total mass, solid PETG | 177 g | 247 g |
| Arm body | 137 g | 208 g |
| Worst peak stress, 3 mm mesh | 96.7 MPa | 21.7 MPa |
| Worst peak safety factor, 3 mm mesh | 0.47 | 2.08 |
| Max deflection, lateral | 8.33 mm | 1.27 mm |
| Max deflection, fore-aft | 5.36 mm | 1.03 mm |
| Torsional stiffness (estimate) | 17 N·m/° | 67 N·m/° |

## Rev B changes

- **Neck:** a round collar continues the tube around the top of the RS02, X 161.19 to 190 and down to Y 239, with 1.75 mm clearance to the flange. It stops 3 mm short of the output face to leave room for the fork. The web is wider at the top, and the collar root has a 1.5 mm fillet. The tube's lower corner is now square.
- **Top plate:** a tapered hub takes the M4 holes to 22 mm deep, with a hollow centre. It needs M4 × 30 or longer with limiters, fitted through the window.
- **Window:** 21 × 40 mm with 8 mm corner radii, in a 3.5 mm inner frame that ties roof to floor. The frame is 76° wide.
- **Carrier:** six Ø6.5 × 3 pockets clear the M3 heads. Rev A's flat carrier would have sat on them.

## Rev B results

Same load cases, material and fixed holes as rev A, on the hoop-flat print orientation. The M4 holes are now fixed over their full 22 mm depth, which matches a limiter. The tube has a safety factor of 4.4 to 5.9.

| Case | Peak stress, MPa | Peak location | Safety factor | Tube wall max / mean, MPa | Max deflection, mm | Across layers, MPa | Layer safety factor |
| --- | --- | --- | --- | --- | --- | --- | --- |
| LC1A | 20.9 | Lower cover insert | 2.15 | 10.3 / 2.0 | 1.03 | 16.5 at M3 hole | 1.36 |
| LC1B | 20.9 | Lower cover insert | 2.15 | | 1.03 | 12.2 at M3 hole | 1.84 |
| LC1C | 21.7 | Upper M3 hole | 2.08 | 7.6 / 1.6 | 1.27 | 10.6 at M3 hole | 2.13 |
| LC2 | 18.2 | Upper M3 hole | 2.47 | | 0.21 | 8.9 at M3 hole | 2.52 |

### Mesh convergence, LC1C, worst spot

| Mesh | Nodes | Peak, MPa | Safety factor | Across layers, MPa | Layer safety factor | Deflection, mm |
| --- | --- | --- | --- | --- | --- | --- |
| 3 mm | 75,862 | 21.7 | 2.08 | 10.6 | 2.13 | 1.27 |
| 2 mm | 220,091 | 22.7 | 1.98 | 17.6 | 1.28 | 1.28 |
| 3 mm, 0.8 mm at M3 holes | 103,951 | 26.0 | 1.73 | 16.2 | 1.39 | 1.28 |

Deflection has converged. Stress at the top two M3 holes has not, because the model pulls on a rigid bolt surface. Treat those peaks as an upper bound. Loading the flange seat instead gave 50 MPa at a sharp corner on an earlier geometry, which is the same kind of artefact in the other direction.

### Print orientation

Print with the hoop flat on the bed, tube axis horizontal. Upright printing gives a layer safety factor of about 1.0 on rev B. Printing with the rear face down gives 1.42 to 2.18. Hoop-flat needs supports inside the tube.

### Next

Use M3 steel limiters with flanged washers on the top two holes, then run the jig. Lightweighting should wait for that, and the hub and window frame hold most of the added mass.

---

# Rev A, plain tube

## Rev A geometry

| Item | Value |
| --- | --- |
| Tube | Round, 78.5 mm OD at the top, 69 mm at the bottom, 3 mm wall, no ribs or lattice |
| Top plate | 8 mm, 6 × M4 on the 24 mm bolt circle |
| Arm body mass | 137 g |
| Stub carrier mass | 36 g |
| Cable cover mass | 3.5 g |
| Total | 177 g, solid PETG at 1270 kg/m³ |

## Fixed load cases (all revisions)

The model is the arm body alone, with no cover or carrier. The six M4 holes are fixed. Loads act at the elbow axis through a rigid link to the six M3 holes.

| Case | Load at the elbow axis | Source |
| --- | --- | --- |
| LC1A | 300 N fore-aft, plus 17 N·m about the elbow axis | 60 N·m at the shoulder over 215 mm is 279 N. 2× gravity on 1.07 kg adds 21 N. 17 N·m is the RS02 elbow peak |
| LC1B | LC1A reversed | Same stress magnitude, different layer tension |
| LC1C | 300 N lateral, plus 17 N·m about the elbow axis | Same force in the shoulder roll direction |
| LC2 | 17 N·m about the arm axis | Upper-arm yaw RS02 peak |

Material: E 1900 MPa, Poisson 0.38, strength 45 MPa. Tension across layers is limited to 22.5 MPa, which is a 0.5 knockdown. These are assumed values, not coupon data.

## Rev A results

Mesh: 3 mm, high quality, 54,708 nodes. There was no convergence study, and peaks at sharp corners and fixed holes are partly mesh singularities.

| Case | Peak stress, MPa | Peak location | Safety factor | Tube wall max / mean, MPa | Max deflection, mm |
| --- | --- | --- | --- | --- | --- |
| LC1A / LC1B | 53.6 | M4 hole in the top plate | 0.84 | 18.7 / 3.0 | 5.36 |
| LC1A / LC1B, away from fixed holes | 52.9 | Cable window corner | 0.85 | | |
| LC1C | 96.7 | Corner where the neck meets the tube | 0.47 | 14.1 / 2.9 | 8.33 |
| LC2 | 29.9 | Neck to tube | 1.50 | | 0.83 |

Max deflection is at the end of the elbow hoop, 152 mm below the top plate.

Torsional stiffness is about 17 N·m per degree (0.99° at 17 N·m). I derived this from the peak displacement at 48 mm from the arm axis, so it is an estimate.

### Tension across layers

Allowable is 22.5 MPa.

| Case | Tube upright on the bed (layers across the arm) | Hoop flat on the bed |
| --- | --- | --- |
| LC1A | 51.1 MPa, safety factor 0.44 | 44.6 MPa, 0.50 |
| LC1B | 57.0 MPa, 0.39 | Not run |
| LC1C | 51.6 MPa, 0.44 | 57.0 MPa, 0.39 |
| LC2 | 27.7 MPa, 0.81 | Not run |

Neither print orientation fixes this, because the highest stresses are local to the joints.

## What rev A showed

1. **Neck.** The tube is carried by one 13.5 mm web on the inboard side. This is the worst point, in lateral load. It needs fillets and a wider path into the tube.
2. **Top plate.** The whole arm hangs from a 24 mm bolt circle through 8 mm of PETG. It needs a thicker hub or a metal backing ring.
3. **Window.** Corners need radii, and the cover is not counted as carrying load.
4. **Wall.** Only after those three is there a case for thinning it. A hand check gives 1.9 MPa nominal bending at the top of the tube.

## Bench jig

Two printed parts, in `jig/`, both modelled in arm coordinates.

| Part | Size | Use |
| --- | --- | --- |
| `jig_upper_arm_root_plate.step` | 170 × 60 × 12 mm | Bolts to the arm's six M4 holes with long M4 bolts and nuts. Clamp to a bench, or 4 × M5 to rails 40 mm apart |
| `jig_upper_arm_load_bar.step` | 260 mm long, 18 mm thick, with an 80 mm disc | Bolts to the arm's six M3 holes with nuts on the far side. Ø6.5 hanger holes at the centre and ±100 mm |

Set-up: arm horizontal, cantilevered from the bench, with the root plate held firm. Measure the plate as well, so its movement can be subtracted. Apply each load, wait 30 seconds, and read the deflection. Remove the load and check the arm returns to zero.

| Test | Hanger | Read at | Rev A at 10 kg | Rev B at 10 kg |
| --- | --- | --- | --- | --- |
| Bending, fore-aft | Centre hole, cover side up | End of the elbow hoop | 1.04 mm | 0.20 mm |
| Bending, lateral | M6 eyebolt in the centre hole, arm rolled 90° | End of the elbow hoop | 2.75 mm | 0.40 mm |
| Torsion | +100 mm hole | Far bar end, 130 mm from the axis | 1.29 mm (0.57°) | 0.33 mm (0.15°) |

Predictions are linear, so check 2.5, 5, 7.5 and 10 kg. Rev B moves little, so use a dial indicator rather than a ruler, and watch for root plate movement. Rev B's local safety factor at 10 kg is 5.3 in the lateral test, so it can go to 25 kg if the rig is safe. PETG creeps, so long holds will read high.

For rev B the load bar disc is Ø80 to clear the collar, and the M4 bolts need to be M4 × 40.
