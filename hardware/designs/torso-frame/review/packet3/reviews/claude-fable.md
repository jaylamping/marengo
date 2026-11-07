**Verdict: NO-GO** for the first test set as packaged. Three items would waste the print or block assembly (hook slit fuses in the bridge, standoff screws collide, side-part orientation has an island). Each is a one-line fix in a script or slicer; after those, the cradle is printable as a fit-check, not as the retention design.

## What I verified vs assumed

Verified from the STEP/STL meshes and scripts (point containment, slice intersections, bounds): side volume 55,434 mm³ (README 55,370, tessellation), strap 4,653, hooks 2,927 and 1,726, carrier 38,279 (48.6 g). Windows at Z −41 to 21, boss at Z −5, slot Y 345.5 to 348.9 open to Z −51, strip above the slot 4.72 mm, detent pocket 0.8 deep, pads with Ø4.2 holes 5.5 deep, M5 slots at Z ±12 and −30/+20, window share 34.1 %. Hook fit: seated with no overlap; outward travel stopped by the barb at 0.3 mm; lift stopped by the slot ceiling at 0.5 mm. Tongue hand calc: ε = 3δt/2L² = 0.83 %, F = 3EIδ/L³ = 2.9 N. Carrier countersink measured at 82° from the mesh. Standoff screw stack: 4 + 4.4 mm into a 6 mm standoff.

Assumed: PETG E 2 GPa, yield ~50 MPa in-plane, ~30 MPa across layers; tip-over deceleration ~30 g where the spine pads crush, 50 to 100 g sideways/forward with no crush zone (pack at ~1.07 m, v ≈ 4.7 m/s, s = 35 mm gives 32 g; s = 10 mm gives 110 g); pack case material and insert pull-out unknown; thumbscrews brass or A2.

## Findings

**Blockers (test print wasted or cannot be assembled)**

1. **Hook slit fuses.** The tongue is separated from the bar by a 0.8 mm slit (`Tongue_slit`, Z −7.8 to −7). In the stated orientation that slit is the air under a 21.7 mm PETG bridge at 7.2 mm height. PETG bridges sag more than 0.8 mm over that span, so the bar welds to the tongue and the snap cannot flex; the fit test tells you nothing. Fix: widen the slit to ≥ 1.8 mm (bar Z −6 to 5), keep the orientation.
2. **Standoff screws collide.** M3 × 6 from the rear through 2 mm of plate enters the standoff 4 mm; M3 × 6 through a 1.6 mm board enters 4.4 mm. 8.4 mm in a 6 mm F-F standoff: the board screws stop 2.4 mm proud. Fix: M3 × 5 rear, M3 × 4 front (3.0 + 2.4 = 5.4), or 1 mm counterbore and M3 × 5 both ends.
3. **Side orientation.** Standing on the rear edge (Z −50) puts the part on a 3.5 × 178 mm sliver, and because the slot runs to the rear edge, the strip plus top foot is a separate island for the first 55 mm of height on a 3.5 × 4.7 mm footprint (foot underside overhanging at 6 mm). It will detach or need heavy support. Fix: print front edge down (Z 37.5 face is coplanar across web, lip, feet, pads); the slot then opens at the top; support only the ledge, tongue and boss undersides. Layer plane is unchanged.

**Major**

4. **Open-rear slot is vestigial and cuts the web off from the top foot** (Q2). Hooks now enter along X, so the slot needs only Z −16 to 5.2. As built, the top-rear 55 mm of the web is a free edge and the top foot's Z −30 bolt hangs on a 32 mm root at Z 5.2. The 4.7 mm strip itself is not the problem; the disconnect is. Fix: `Strap_slot` z0 from −51 to −16. This also stops the hook sliding rearward if the detent lifts (item 13).
5. **Thumbscrew clamp loop runs through bent PETG** (Q3). The pack slides freely (ledge friction ~7 N), so the first screw pulls the pack 0.5 mm to its web with no bow; the second must bow its web ~1.0 mm, not 0.5. Middle band between windows (34 × 3.5 × 62 span) needs ~100 to 200 N per mm, so hand torque (0.3 N·m ≈ 400 N) closes it and the rest clamps the pack case. Stress ~20 MPa sustained in a 3.5 mm web at pack temperature: creep relaxes the bow, preload goes, screws back out, and they are the only rearward retention. Fix: clamp with one screw and make the other a locating pin (head backed off), torque limit, thread-lock; add a secondary rear stop (clip across the web rear edges above the BMS) so a loose screw does not let 2.2 kg exit through D2.
6. **Side fall exceeds the web.** One web takes the whole lateral load (the opposite thumbscrew head just lifts off its boss). Fixed-fixed 178 mm span, end moment WL/12 = 320·n N·mm; section at the window posts (9 + 16.5 mm) Z = 52 mm³ gives 6.2·n MPa, full section 1.8·n MPa. Yield at 8 to 28 g, realistic 8 to 15 g at the posts or the slotted top-rear corner. At ~8 mm lateral travel the pack leaves the opposite 8 mm ledge. Pack stays inside the shell but not in the cradle. Fix for rev B: web 5 mm outward (|X| 48 to 53, nothing is there), windows narrowed to leave ≥ 20 mm posts. Accept for a fit print.
7. **M5 button heads on 4 mm PETG.** BUILD.md's 4 N·m gives ~4 kN preload on a 47 mm² head annulus, 85 MPa, well past PETG. Heads embed, preload drops, feet then float in the 10 mm slots (±2.25 mm travel) and Z position is friction only. Fix: Ø15 washers, 1 to 1.5 N·m.
8. **Carrier countersinks.** Mesh shows 82° cones (build123d default), README says 90°. A 90° head bears only on the rim. The rim at X 54.85 is 0.15 mm from the plate edge, the Ø3.4 hole 1.55 mm; torqueing a flat head there splits the corner. Fix: `counter_sink_angle=90`, and move the four screws and pad inserts inboard to |X| 50.5.
9. **Thermal unquantified.** 36 W over 0.071 m² with no lower-box exhaust means the pack runs to the BMS limit (~60 °C). PETG at 60 °C has roughly half modulus and 10× creep; the exposures are item 5's clamp loop and the carrier (4 mm plate between pack at 3 mm and board at 6 mm) sagging under 250 g. No melt risk. Fix: thermocouple on a web and the carrier rear during a 36 W soak; ASA or PC for the carrier if > 55 °C.
10. **Strap does not stop lifting as claimed.** Bar has 0.4 mm slot clearance over a 3.5 mm web, so the hook rocks 2.5° (barb contact) to 6.5° (ceiling contact) before resisting: 2 to 5 mm of inner-end lift on the 43 mm right hook. Once the barb engages, lever ratio 43/4.5 puts ~10× the lift force into the 1.6 × 7.2 mm tongue in compression; ~500 N tongue capacity gives 5 to 7 g for both hooks. Lift is in fact carried by the thumbscrews bearing in the Ø4.5 web holes (0.25 mm play). The strap is a backup only; say so, and do not credit it in the load case.

**Minor**

11. M3 × 5.7 inserts (frame README) in 5.5 mm holes stand 0.2 mm proud, so the carrier rocks on inserts instead of sitting on pads and lips. Hole depth 6.5.
12. Pad wall is 1.65 mm around the Ø4.2 pilot (7.5 wide pad). Make pads 9 to 10 wide; the cable band is at Z 42 to 50, above the pads.
13. Detent engagement at the float extreme is 0.4 mm, equal to the slot's vertical clearance, so a lifted and dragged hook can ride out and slide rearward until the lip meets br17. Closing the slot (item 4) fixes it.
14. Barb top (Y 350) interferes 0.38 mm with the top foot underside (Y 349.62) along 23.5 mm of the insertion path; `check_hook_fit.py` sweeps parts.json boxes, which lack the foot. Benign, the ramp takes it, but the check missed it.
15. Hook click at 2.9 N and 0.2 mm barb gap are within printer tolerance scatter; expect some hooks to rattle or not click. Not a blocker.

## Answers to the eight questions

1. Load case (per side, 2.2 kg, n g): forward, lip 3.1 MPa at 30 g, fine to >100 g; feet in Z by bolt friction then slot-end bearing (4 MPa at 30 g). Rearward: thumbscrews only, M4 shank shear 1.6 kN brass, web hole bearing 7 MPa at 30 g, so ~70 g if tightened; zero if loose. Sideways: web yields at 8 to 15 g (item 6), first failure in the design. Downward: ledge 6 MPa at 30 g. Upward: thumbscrews in bearing, strap at 5 to 7 g (item 10). Order of failure: web in a side fall, then pack insert pull-out (unknown case material), then strap tongue. Captured in rear, front and down to ~30 g; not captured sideways above ~10 g.
2. Strip: adequate for the hook prying (bearing ~30 MPa in-layer, worst case) but irrelevant, because for Z < 5.2 the web is not joined to it. Close the slot to Z −16.
3. Bowing: 0/1.0 mm, not 0.5/0.5; the clamp preload lives in bent PETG and creeps out. One clamp screw plus a pin, torque limit, secondary rear stop.
4. Hooks fit (verified at 0.1 mm equivalent), click at ~3 N, 0.83 % strain, release as described. They will not print as described: the slit fuses (item 1).
5. Carrier: in-plane bending ~17 MPa at 30 g, first mode ~30 to 50 Hz, acceptable. Pads clear the cable band (Z 29.5 to 37.5 vs 42 to 50); carrier clears bracket heads (Z 41.5 vs 55). Missed: screw collision, 82° countersink at a 0.15 mm edge, proud inserts.
6. Thermal: item 9.
7. Print: items 1, 3; carrier flat with 100 % infill is fine (annular counterbore bridges 1.55 mm).
8. Contradictions: insert 5.7 vs hole 5.5; countersink 90° text vs 82° model; pdb_sheet "must narrow to 42" while parts.json already has ±42; harness reserve X −35 to 30 (frame README) vs X −5 to 25 (cradle README, parts.json); packrail_* entries are the cradle webs under the old name; "M4 × 16 with shoulder" leaves thread engagement ambiguous (4.5 mm only if 16 is thread); PACKET's bracket-head band Y 175 to 203 is the bracket leg, heads are Y 187 to 196 and 333 to 341; side volume 55,370 vs 55,434 in the STL (tessellation).

**NO-GO** for printing until the slit, the standoff screws and the side orientation are fixed; then GO for a fit-check set only.
