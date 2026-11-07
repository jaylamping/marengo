# Left upper arm print order

2026-11-07. **Not released yet: the recheck comes first.** Don't fit these parts to the robot until it passes.

**Decision (Joey):** the upper arm prints now, and the elbow rig test runs before the forearm is fitted, not before this print.

Settings are the test pass 1 process: P2S, 0.4 mm High Flow, Generic PETG, 0.20 mm, 6 walls, 100% infill, normal supports, outer 5 mm brim. See `../test-pass-1/SETTINGS_CHECKLIST.md`. Open each file as a project, don't rotate anything, and check the settings before printing. Each plate matches the P2 L04B project except `brim_type`, which is `outer_only` in place of `auto_brim`. No slicer warnings on any plate.

| Order | File | Print time | Filament | Down on the plate | Why |
| --- | --- | --- | --- | --- | --- |
| 1 | `fit_coupons.3mf` | 3 h 05 m | 83.8 g | Same way up as the parts | **Print this first.** Hoop ring, hub core, bearing seat ring and a carrier sector, cut from the parts. The sector has two Ø6.05 holes, the Ø7 washer seats and the Ø65.8 pilot |
| 2 | `carrier.3mf` | 1 h 12 m | 31.4 g | Inboard face | Same way up as the strength carrier in test pass 1 |
| 3 | `cover.3mf` | 48 m | 4.4 g | Standing on its end | Thin curved shell |
| 4 | `upper_arm.3mf` | 8 h 25 m | 263.7 g | Flat on the hub, X 131.99 | Layers across X, as in the FEA. The RS02 seat faces up, clean |
| 5 | `p2.3mf` | 5 h 54 m | 188.8 g | As P2 L04B, same 45 degree plate rotation | Only if you want a new P2, see below |

Total for plates 2 to 5: about 16 h 20 m and 488 g, including support and brim. Without P2, 10 h 25 m and 300 g.

**`seat_ring_only.3mf`, 33 m, 11.4 g:** the bearing seat ring on its own, with the support blocker. It's the one coupon still valid now the joint is going all-printed, so print this one instead of `fit_coupons.3mf` for now. A new coupon plate follows with the new joint.

## Fit coupons

Use the real parts, and write down what you get.

| Coupon | Check | Accept | If not |
| --- | --- | --- | --- |
| Hoop ring | RS02 slides into the bore and sits on the seat | By hand, no force. RS02 body is Ø65, bore Ø66 with Ø65.6 pads at the holes | Scrape the pads |
| Hoop ring | Ø5 sleeve in each Ø5.2 hole | Slides in by hand | Open by hand with a Ø5.2 drill held in fingers or a pin vise. **Never ream it hard: the wall to the bore is 1.1 mm** |
| Hub core | Ø6 sleeve in each Ø6.2 hole | Slides in by hand | Clear support with a Ø6.2 drill by hand |
| Seat ring | 6808 into the Ø52.2 seat. The coupon wall is thinner than the carrier's, so it reads slightly looser | Firm hand press, no rock | Too tight: change the CAD offset before printing the carrier. Too loose: use retaining compound |
| Carrier sector | Ø6 washers in the Ø6.05 holes | Two DIN 433 drop in by hand | Open with a Ø6 drill by hand |
| Carrier sector | Ø7 washer in the Ø7.05 land above them | One DIN 125 drops in by hand | Open with a Ø7 drill by hand |
| Carrier sector | Sector flat on a plate, two DIN 433 in a hole, straight edge across them | Steel stands 0.10 to 0.25 proud of the printed floor. **PETG proud of the steel is a fail**, because preload would go through the plastic | Under 0.10: scrape the floor with a Ø7 countersink turned by hand. Over 0.25: don't scrape, measure the two washers and tell me, because the carrier would float too far |
| Carrier sector | Ø65.8 pilot into the steel Ø66 ring | Slides in, small shake | Sand the pilot |

If a coupon misses by more than a scrape, tell me the numbers and I'll change the CAD before the long print.

The seat ring has the same support blocker as the carrier, so its bore prints as the carrier's does. A few low stubs of support stay near the bed in both: pull them out before pressing the bearing.

**The hoop coupon prints on its cut face.** It checks the bore, seat and hole fits. It doesn't check the 13.40 thickness or the sanding of the inboard face, because the real hoop face prints on support. Those checks happen on the real arm, in `ASSEMBLY_ORDER.md`.

The floor in the Ø6.05 hole can only land on a 0.2 mm layer. The hole is 0.85 tall in CAD and prints 0.8, so two 0.5 washers stand 0.2 proud before the floor droops. A 1.0 tall hole would start at zero and droop proud of the steel, so 0.8 is the safe choice and the band above suits it.

The carrier sector is cut from the next carrier revision, with the Ø7 washer seat. The `carrier.3mf` in this commit is the previous one and doesn't have that seat.

## P2

P2 L05 is P2 L04B with 2.5 mm off the inboard tip. It clears the M4 heads on the r 49 collar ring, so the inward roll limit can go from 2 to 5 degrees. **You don't have to reprint it.** You can file 2.5 mm off the tip of the P2 you have, flat, over Z 8 to 28, for the same clearance. A reprint uses the same process and orientation as L04B.

## Where support lands

Each is from the sliced G-code.

| Part | On support | Keep clear |
| --- | --- | --- |
| Carrier | Recess roof, retainer pockets, under the head pockets | **A support blocker keeps support out of the bearing seat.** The ledge prints unsupported, so check it is flat. The outboard face prints up, clean |
| Upper arm | **Hoop inboard face at X 151.79**, tube inside, port slot edges, **all six Ø6.2 hub holes** and the notch at the bottom of the bore | RS02 seat at X 165.19, hub face at Y 319.86 |
| Cover | Cable entry and mounting holes. **About 43% of its extrusion is support** | Seating rim, mostly |
| Fit coupons | As the parts | |

The hoop inboard face can't avoid support in this orientation. Flipping the arm moves support onto the RS02 seat instead, and adds 13 g and 23 minutes.

The bed contact on the upper arm was 15 mm². The new flat on the hub gives 290 mm², so no raft. The rest of the arm stands on support, so watch the first layers.

## After printing

- **Hoop and hub:** see the flatness and sleeve check in `ASSEMBLY_ORDER.md`, step 2.
- **Carrier:** clear the recess and pockets without marking the seat. Check the ledge is flat.
- **Upper arm:** take the tube support out through the cable window and port slot in small pieces. Clear each hub hole with a Ø6.2 drill by hand.

Parts list: `PARTS_LIST.md`. Assembly: `ASSEMBLY_ORDER.md`. Strength: `FEA_SUMMARY.md`. Hashes: `SHA256SUMS.txt`.
