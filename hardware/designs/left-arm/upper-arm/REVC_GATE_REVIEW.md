# Rev C gate review, elbow pass 2

2026-11-06. Packet: `review/left-arm-revc2`, PR #272, `hardware/designs/left-arm/upper-arm/review/revc2/`. Argon is still to run on the Mac, so this summary may change.

**Result: not passed. Do not print the full upper arm or forearm.** All four reviewers reached that independently. Print small fit and stop pieces first, once the items below are fixed.

| Reviewer | Run time | Verdict |
| --- | --- | --- |
| Muse Wonder 1.0 | 7.9 min | Fail |
| Astra, high | 18.1 min | Hold |
| Sol, high | 18.7 min | Blocker |
| Fable, high, on the subscription, as no API key is set on Joey-PC | 25.2 min | Do not print yet |
| Argon, high | Pending, on the Mac | |

Packet hashes held before and after the run. Run started 07:29:53Z, in parallel.

## Accepted

I checked each of these against the packet files. "Raised by" names who found it.

### Blockers

1. **One hoop M3 cannot be tightened with the fork on** (Fable; Sol and Astra flagged access generally). My check on the pass 2 fork STL and STEP holes, with the driver projected along X at every degree from 0 to 133: the hole at θ 63.9 degrees is clear only at exactly 133 with a Ø3.5 tool and never with Ø5.5. The other five are clear at some angle. The three retainers at r 29 are never reachable with the fork on, so they must go in before it. Fix: move the access, by a relief in the cheek or by clocking the RS02, then write out and prove an assembly order.
2. **The carrier floor can drag on the inner race** (Sol, Astra, Fable). The floor bore is Ø42 at X 150 to 151.2, and the race face sits at X 150. EZO gives a Ø49 to 50 housing abutment for a 6808 2RS. Fix: choose the bearing, then open the bore to clear the inner ring and seal.
3. **The stop is not sized, and the carrier has no strength evidence** (all four). Contact 3.5 × 6.5 mm at r 38.25. At 17 N·m that is 444 N and 19.5 MPa average. At the 0 degree stop the shoulder can apply more: 60 N·m gives 120 N at the hand and 34 N·m at the elbow, so 39 MPa (Fable, arithmetic checked). The FEA covers the arm body and fork only, not the carrier or the finger root. There is no fall case. Fix: stop and carrier FEA, a fall energy case, and a stop test on a dummy output.
4. **"Printed parts fail first" is not shown** (all four). The 47% figure is right but covers only a force on the arm axis. A side moment still gives 385 N, up 28% from the wide layout, and axial load still goes through the output. No RobStride ratings exist. Fix: get or measure a limit, or Joey accepts the risk on record. See the decision below.

### Majors

5. **Layer SF 1.76 and fork SF 2.26 are not demonstrated margins** (all four). The baseline shows the same hole edge going from layer SF 2.13 to 1.28 with mesh alone. Pass 2 ran only the 3 mm mesh, and the fork has no convergence study. The reviewers disagree on whether the 1.33 to 1.76 move is physical; only a matched refined run settles it. The fork's layer SF barely moved, 1.60 to 1.61. Earlier statements that 1.76 clears the target should read: clears it at 3 mm mesh, unconverged.
6. **Fits are not specified** (all four): stub, seat, float, and only 0.2 mm from shoulder to bearing. Fix: a seat and stub coupon, then put measured offsets in the model.
7. **Screw clamp paths** (Astra, Fable): 3 mm of carrier PETG sits in the M3 clamp path, outside the sleeves, and the fork M4s clamp 6 mm of PETG.
8. **The interference sweep leaves out the parts most likely to clash** (Sol, Astra, Muse): screws, washers and the 6808. Clearances have no tolerance or deflection allowance. The hand is missing, and turning RS00 whole is not a conservative stand-in for it (Astra); the record used that word wrongly.
9. **Fork M4 heads** (all four): socket heads reach X 209.19, so 77.5 mm overall. Specify ISO 7380 or deepen the recess.
10. **Print risks** (Fable, Sol, Muse): +X support faces land on fitted surfaces, and the carrier has four bridges with its outboard face up. Fable suggests outboard face down. Compare -X as well (Sol, Astra). The arm and carrier have no slice.
11. **Scope** (all four): the forearm junction opening, cable route and length decision can all make a full print obsolete.

### Minors

- The URDF upper limit equals the hard stop. The controller needs a soft limit 2 to 3 degrees inside it (Fable).
- Retainer washers clamp flush, with the pocket floor on the race plane; the washers catch about 1.5 mm of race (Fable, Muse).
- Stub ends 0.1 mm short of the race face (Fable).
- Ø6 counterbore for a Ø5.7 button head is tight when printed (Sol, Fable).
- `p2_gusset.py` defaulted to 30.5 mm while the part is 36.5 (Sol, Astra): fixed locally. Scripts carry fixed paths, and the CAD reorg has since moved the exports.
- `jumper_band.py` stopped short of the band edges, and the r 42.5 to 50 band is too thin for Ø8 (Sol, Astra). The posterior route replaces this band.

## Rejected

- **M4 engagement under a 7 mm minimum, called a blocker by Muse and a major by Fable.** The RS02 drawing reads 6-M4 7 MIN, and the manual says screw depth must not exceed the casing thread depth. So 7 mm is the least thread available, and 6.0 to 6.14 mm is within it. The packet's phrase "7 mm minimum depth" caused this and is my error, carried from earlier records. Sol and Astra read it correctly. Fable's M4 × 16, at 8 mm, could exceed the thread. Still open: measured depth on a real RS02 and thread strength.
- **Contact pressure compared with layer tension** (Muse). The 22.5 MPa allowable is for tension across layers, and the stop face is in compression. The stop stays a blocker on the other grounds.
- **A 30 × 18 board fits the 21 × 40 window only diagonally** (Muse). 18 is under 21 and 30 is under 40.
- **The retainer stack must fit in the 0.8 mm gap** (Muse). It sits in a 2.5 mm pocket, with a 2.45 mm stack (Fable).
- **±30 to 40% on all four SF tables** (Muse). That applies one hole's sensitivity to every result. Finding 5 covers what is supported.

## Closed since the packet

- Fork added to the URDF: `forearm_link` 0.481 kg, total 2.751 kg (Sol, Astra, Muse raised it).
- Fork re-sliced with the P1 and P2 process: 7 h 40 min, 264 g with supports. The packet's slice used an assumed process.
- Elbow jumper band replaced by the posterior route, still to check.

## Next

No CAD work until SolidWorks is free.

1. Fix the floor bore, hoop access, fork head recess and retainer pocket.
2. Write the assembly order.
3. Run carrier and stop FEA, and refined LC1A and fork twist.
4. Add screws, washers and the 6808 to the interference sweep.
5. Print layer coupons, then the carrier alone with a real 6808, then a fork cheek and stub sector on a dummy output, including stop hits and a run to failure.
6. Print full parts only after that, with lengths and the wiring openings frozen.

**Decision for Joey:** the compact layout saves 3 mm of width but raises the side-moment couple on the RS02 output bearing by 28%, with no rating to judge it. Keep it and accept that risk, measure a limit first, or go back to a wider span.
