# Left upper arm, rev B

2026-11-05. Not reviewed and not released for printing or load. The CAD stays on Joey-PC.

The arm carries the yaw RS02 output at the top and holds the elbow RS02 housing at the bottom. Strength results and the load test are in [BASELINE.md](BASELINE.md).

- Section: round on the arm axis, 78.5 mm OD at the top tapering to 69 mm, with a 3 mm wall. A round collar continues over the top of the RS02.
- Top: tapered hub with six M4 holes, 22 mm deep. It needs M4 × 30 or longer with limiters.
- Window: rear, 21 × 40 mm with 8 mm corners, in a reinforced frame. The curved cover is held by 4 M3 screws.
- Elbow support: bolted carrier with a stub for a 6808 bearing (40 × 52 × 7 mm), hollow, with pockets for the RS02 M3 heads.
- Stops: elbow lugs on the carrier for 0° and 135°. They need the forearm fork to act against.
- RS02 fit: CAD interference is zero. The collar stops at X 190, 3 mm short of the output face, so the fork must stay outboard of that.

Mass is 247 g as solid PETG: arm 208 g, carrier 35 g, cover 3 g. Print with the hoop flat on the bed.

Fold clearance at 135° is 6.7 mm to the RS00, or 3.7 mm once the forearm has a 3 mm shell, so no flat is needed on the tube.

## Cable route

The RS02 is not hollow, so the 14 mm centre bore in the top is not a cable path. It is only a lightening hole. Rev A's README was wrong on this.

**The RS02 lower port is enclosed. Only the upper port is usable.**

| Item | Size | Harness spec | Status |
| --- | --- | --- | --- |
| Trunk entry, rear cover | 13 × 11 mm with 4 mm corners | Ø9 or 5 × 11 mm | Passes |
| Tube cavity | Ø58 to Ø70 | Connector pockets | Passes. XT30(2+2)-F fits |
| Upper RS02 port slot | 20 × 24 mm into the cavity | | Passes |
| Trunk exit, tube floor, inboard | 9.4 × 14 mm with 3 mm corners | Ø9 or 5 × 11 mm | Passes. XT30 fits narrow side first |

The upper arm has no RS03, so the XT30UW plus GH1.25 pocket does not apply here.

| Crossing | Route | Free length | Bend radius | Status |
| --- | --- | --- | --- | --- |
| Yaw, ±90° | Clamp on P2, half turn around the RS02 outside the housing, in through the cover entry, clamp at a lower cover boss | About 185 mm against 150 mm | About 42 mm | Static passes (35 mm). Dynamic 70 mm does not fit, so split into loose wires (30 mm) |
| Elbow, 0 to 135° | Out through the floor, around the stub between the carrier and fork, X 134 to 143 | About 105 mm at radius 45 mm, against about 113 mm pro rata | About 45 mm | Provisional. Needs slack in the fork design, and loose wires for the same reason |

The 10 mm band for the elbow crossing is shared with the stop lugs, so the fork design has to confirm it. Neither clamp point exists yet: a P2 clamp and a tie point on the lower boss are still to be added. Free lengths are estimates from the geometry, not a routed harness.

## Open items

- Two M3 holes fall short of the safety targets on refined meshes. See BASELINE.md.
- The shoulder roll stop is not here: it belongs on P1 and P2, at the roll joint. See `../envelope/README.md`.
- No bearing retainer or forearm fork yet.
- Rev B has had no adversarial review.
