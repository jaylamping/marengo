# Upper arm baseline

2026-11-05. Round tube v1, before any lightweighting. Keep these load cases unchanged for every iteration.

## Result

**The tube is not the problem. The joints at each end are.** At the full load case the tube wall peaks at 14–19 MPa and averages 3 MPa, so it has a safety factor of 2.4–3.2 against the knocked-down strength. The neck below the tube, the 8 mm top plate around the M4 bolts, and the cable window corners all exceed that strength. This version is not safe at the fixed loads, and there is little to gain from thinning the wall until those three areas are fixed.

## Geometry

| Item | Value |
| --- | --- |
| Tube | Round, 78.5 mm OD at the top, 69 mm at the bottom, 3 mm wall, no ribs or lattice |
| Top plate | 8 mm, 6 × M4 on the 24 mm bolt circle |
| Arm body mass | 137 g |
| Stub carrier mass | 36 g |
| Cable cover mass | 3.5 g |
| Total | 177 g, solid PETG at 1270 kg/m³ |

## Fixed load cases

The model is the arm body alone, with no cover or carrier. The six M4 holes are fixed. Loads act at the elbow axis through a rigid link to the six M3 holes.

| Case | Load at the elbow axis | Source |
| --- | --- | --- |
| LC1A | 300 N fore-aft, plus 17 N·m about the elbow axis | 60 N·m at the shoulder over 215 mm is 279 N. 2× gravity on 1.07 kg adds 21 N. 17 N·m is the RS02 elbow peak |
| LC1B | LC1A reversed | Same stress magnitude, different layer tension |
| LC1C | 300 N lateral, plus 17 N·m about the elbow axis | Same force in the shoulder roll direction |
| LC2 | 17 N·m about the arm axis | Upper-arm yaw RS02 peak |

Material: E 1900 MPa, Poisson 0.38, strength 45 MPa. Tension across layers is limited to 22.5 MPa, which is a 0.5 knockdown. These are assumed values, not coupon data.

## Results

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

## What this says about the next step

1. **Neck.** The tube is carried by one 13.5 mm web on the inboard side. This is the worst point, in lateral load. It needs fillets and a wider path into the tube.
2. **Top plate.** The whole arm hangs from a 24 mm bolt circle through 8 mm of PETG. It needs a thicker hub or a metal backing ring.
3. **Window.** Corners need radii, and the cover is not counted as carrying load.
4. **Wall.** Only after those three is there a case for thinning it. A hand check gives 1.9 MPa nominal bending at the top of the tube.

## Bench jig

Two printed parts, in `jig/`, both modelled in arm coordinates.

| Part | Size | Use |
| --- | --- | --- |
| `jig_upper_arm_root_plate.step` | 170 × 60 × 12 mm | Bolts to the arm's six M4 holes with M4 × 30 and nuts. Clamp to a bench, or 4 × M5 to rails 40 mm apart |
| `jig_upper_arm_load_bar.step` | 260 mm long, 18 mm thick, with an 84.5 mm disc | Bolts to the arm's six M3 holes with nuts on the far side. Ø6.5 hanger holes at the centre and ±100 mm |

Set-up: arm horizontal, cantilevered from the bench, with the root plate held firm. Measure the plate as well, so its movement can be subtracted. Apply each load, wait 30 seconds, and read the deflection. Remove the load and check the arm returns to zero.

| Test | Hanger | Read at | Predicted at 10 kg |
| --- | --- | --- | --- |
| Bending, fore-aft | Centre hole, cover side up | End of the elbow hoop | 1.04 mm |
| Bending, lateral | M6 eyebolt in the centre hole, arm rolled 90° | End of the elbow hoop | 2.75 mm |
| Torsion | +100 mm hole | Far bar end, 130 mm from the axis | 1.29 mm (0.57°) |

Predictions are linear, so check 2.5, 5, 7.5 and 10 kg. Stop at 10 kg: the lateral test has a local safety factor of only 1.9 there. PETG creeps, so long holds will read high.
