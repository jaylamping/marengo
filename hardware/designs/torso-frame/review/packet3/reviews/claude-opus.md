**Verdict: NO-GO for the sides and hooks as modelled.** Four changes are needed first, each about an hour of CAD or slicing: close the strap slot at the rear, widen the snap-tongue slit, state the print orientation and supports, and define the thumbscrew stop. The carrier could print now, but its screws need changing before assembly. The arithmetic below is hand calculation; there is still no FEA and nothing printed.

## What I verified and what I assumed

**Verified** by probing the STL and STEP geometry with rays and re-reading the build scripts:
- The strap slot cuts the full web thickness at Y 345.5–348.9 from Z −51 to 5.2. At X −49.75 the web material ends at Y 345.5 for every Z from −45 to 4, and is continuous only from Z 6 forward.
- The tongue slit is 0.8 mm (Z −7.8 to −7). The bar bridges 21.7 mm (X 30 to 51.7). The bar is 12 × 3 mm where it passes through the web.
- Only 0.8 mm of material separates the detent pocket (top at Y 344.7) from the slot floor (Y 345.5).
- In the carrier, the standoff counterbore floor is at Z 39.5. The countersink goes from r 1.7 at Z 39.89 to r 3.1 at Z 41.5, which is an 82° included angle.
- Masses, the 34 % window area, the 4.5 mm thumbscrew reach, the 0.2 mm hook float each side, and the 1.9 mm room above the tongue all check out.
- Tongue: strain = 3·1.6·1.1/(2·17.8²) = 0.83 % and force = 3·2000·2.46·1.1/17.8³ = 2.9 N. Both match the README.

**Assumed:**
- Printed PETG: E 2 GPa, in-layer strength about 40 MPa.
- Tip-over demand of about 30–70 g: roughly 4.5 m/s stopped in about 30 mm of shell or pad crush, peak about twice the mean.
- Strength of the M4 insert in the pack case is unknown; I used 300–800 N per insert.
- A real 2028 bracket has a diagonal web, which `parts.json` does not model.
- The thumbscrew part is undefined.

## Q1: Tip-over load paths (2.2 kg pack = 21.6 N per g)

| Direction | What carries it | Capacity | Approx. g |
|---|---|---|---|
| ±X (side fall) | Struck web bending over about 168 mm. The weakest section is at the Ø62 window (Y 301): the front 16.5 mm of web plus the lip gives I ≈ 470 mm⁴ and section modulus ≈ 72 mm³, so the limiting moment is about 2.9 N·m and F = 8M/L ≈ 140 N. The far web adds about 150–250 N through its thumbscrew pulling on the boss, but only if that insert does not pull out of the pack case. | ~300–400 N | **~15 (fails first)** |
| +Y (pack lifting) | The thumbscrews take this first (0.25 mm radial clearance in the hole, against 0.5 mm under the strap), so it lands on the pack inserts. The hooks are a backup: bar section modulus 18 mm³ gives about 0.72 N·m, so about 34 N on the right hook (lever about 21 mm) and 63 N on the left (about 11.5 mm). | Inserts, then ~100 N from the hooks | Inserts unknown; hooks ~5 |
| −Z (rearward, back fall) | Only the two M4 thumbscrews into the pack-case inserts. The feet slip on friction (all M5 slots run along Z), then bear after 2.25 mm. | 600–1,600 N (assumed) | 28–75 (unknown) |
| −Y (down) | Ledge cantilever is fine (about 2 kN per side). The real limit is the bottom tongue, 43 × 4 mm with section modulus 115 mm³, levering about the rail edge at \|X\| 55 with an 11 mm arm: about 420 N per side, plus some load hung from the top foot. | ≥840 N | ~40 |
| +Z (forward) | The lip is a 6 mm cantilever with a root section of 162 × 3.5. | ~4 kN per side | >150 |

**The pack is not captured in a side fall or a back fall.**
- **Sideways:** the side opening between the posts (100 × 178.6) is bigger than the pack's 79 × 162 face. Once a web fails at about 15 g, only the shell stops the pack.
- **Rearward:** only the two thumbscrews and inserts hold it, and the frame README says the D2 door is not a restraint.

With roughly 30–70 g of demand, the side case contradicts the requirement that the pack must not come loose.

## Findings

### Blockers

**B1. The strap slot is open to the rear edge for no remaining reason.**
- Evidence: `build_cradle.py` cuts the slot from Z −51 to 5.2. That detaches the top edge of the web over 55 of its 87.5 mm, and the rear M5 on the top foot (Z −30) then connects to the web only through the 4.7 mm strip. The README's own fitting method inserts the hook along X, so the rear opening is a leftover from the abandoned rear slide-in.
- Smallest fix: cut only Z −15.5 to 5.5. The whole hook lies within Z −15 to 5, so it still passes. The slot ends then hold the hook in Z, so the detent pin and pocket, with their 0.8 mm ligament, can be deleted.

**B2. The 0.8 mm tongue slit sits directly under the 21.7 mm bridge in the specified hook print orientation.**
- Evidence: the bar's first bridge layer, at Z −7, prints 4 layers above the tongue's top face. Normal PETG bridge sag will likely fuse the bridge to the tongue, which kills the snap, and testing the snap is the main purpose of this print.
- Smallest fix: widen the slit to at least 1.5–2 mm (tongue Z −15 to −8.5).

**B3. Pack capture in side and back falls (from Q1).**
- This does not stop a fit-check print, but it does stop robot use. It needs either:
  - a positive stop that does not rely on the pack-case inserts (a rear bar, or a stiffener behind the web at about \|X\| 52–55), or
  - an explicit decision that the shell is the capture, together with a check of the shell's strength.

### Major

**M1. The sides' print orientation needs supports, and the README does not say so.**
- Rear edge down (Z −50) puts only the 3.5 × 175 mm web edge on the bed. These features then start in mid-air:
  - the top foot, a 23.5 mm overhang starting at Z −44
  - the ledge, 8 mm at Z −45
  - the bottom strip, at Z −44
  - the tongue, 20 mm at Z −21.5
  - the underside of the boss
  - the pads, 4 mm outboard
- Fix: print **front face down (Z 37.5)**. The web, lip, top foot, strip and pads all start on the bed, and only the tongue and boss need support. Layers still lie in the C plane, so strength is unchanged.

**M2. The carrier's standoff screws collide inside the 6 mm standoff.**
- The rear M3 × 6 runs from the counterbore floor at Z 39.5 to Z 45.5. The board-side M3 × 6 runs from Z 49.1 down to 43.1. They overlap by 2.4 mm.
- `parts.json` models only the board-side screw, which is why this was not caught.
- Fix: rear M3 × 4 and front M3 × 4 (or × 5), keeping the total engagement under 6 mm.

**M3. The thumbscrew is undefined, and the stop is self-contradictory.**
- "Head seats on boss" and "with shoulder" describe two different length stops. An off-the-shelf M4 shoulder screw (ISO 7379) has a Ø5 shoulder, which does not pass the Ø4.5 hole.
- The Ø4.5 hole leaves only ±0.25 mm for the insert position on a hand-built pack, and the pack is not built yet.
- Fix: use a stop that bears on the pack face with shoulder length 11.5 mm (boss face at \|X\| 59 to pack face at 47.5), enlarge the hole to about 6 mm (the Ø18 head covers it), and add a nylon patch or wave washer against back-off.

**M4. The thumbscrews cannot be reached through D2.**
- From the rear, the gap between the web (\|X\| 51.5) and the post or br17 (\|X\| 51–55) is 3.5 mm, and the heads sit at Z −5. Every pack swap means removing both side shells, which contradicts "slides out through D2".

**M5. The M5 feet are clamped on PETG.**
- BUILD.md says to snug every M5 to about 4 N·m, including the cradle. A button head bearing on about 35 mm² beside a 5.5 mm slot would see roughly 130 MPa, which crushes PETG.
- The slots run along Z (adjustment the T-nut already gives) and so throw away positive Z bearing, while offering no X adjustment for the 0.5 mm pack clearance.
- Fix: DIN 9021 washers, about 1.5 N·m, round holes or slots along X.

**M6. The 0.8 mm ligament above the detent pocket.**
- Under uplift the pin (0.31 mm radial play) loads this ligament before the bar reaches the slot ceiling (0.4 mm gap). B1's fix removes it.

**M7. Thermal margin depends on an exhaust that does not exist yet.**
- The pack's surface area is about 0.071 m². With natural convection (h ≈ 5 W/m²K), 20 W gives a steady rise of about 56 K and 36 W about 100 K. With forced air (h 15–25), the rise is about 11–19 K.
- The webs cover 66 % of the pack's sides, and the carrier blocks Z 37.5–41.5 of the side-exhaust window.
- PETG at 50–60 °C carries these low stresses (under 2 MPa) fine, but preloaded joints creep: the feet, the thumbscrew clamp and the inserts.

### Minor

- The countersink is modelled at 82° (the build123d default); the README says 90°, which is the angle of metric countersunk heads. Set `counter_sink_angle=90`.
- The insert size is inconsistent: "M3 × 5" in the code, M3 × 5.7 in the frame shopping list, and a 5.5 mm deep hole. The walls in X are only 1.65 mm, and a bulge would land on the \|X\| 48 face that the pack slides along. Specify the insert.
- The pads reach \|X\| 55.5, about 0.5 mm from the diagonal of the br1/br11 brackets (only a box check was run). Trim the pads to \|X\| ≤ 55.
- The bar bottom (Y 345.5) is line-on-line with the slot floor, so it has zero clearance below.
- The scripts read `../generator/parts.json` and `scripts/*.stl`, so none of the checks can be rerun from this packet's layout.

## Q2: The 4.7 mm strip
The strip and top-foot L is adequate for the barb and bar loads. The real problem is B1: below the slot, the rear 63 % of the web has no top support. That cuts the web's lateral capacity and leaves the rear top-foot M5 doing almost nothing. The fix is B1.

## Q3: Thumbscrew bowing
The two screws' pulls cancel through the pack, so the preload bends each web inward. The web is very compliant: about 6 N/mm as a beam, so 0.5 mm takes only about 5–30 N at under 2 MPa and nothing breaks. After the gap closes, the remaining preload clamps 11 mm of PETG, which creeps, so the screw loosens. An off-centre pack bows one web up to 1 mm.

A spacer is not possible, because it would sit in the pack's sliding gap. A torque limit does not stop the creep. The fix is a redesigned stop that bears on the pack face, as in M3.

## Q4: Snap hooks
- **Fit:** the clearances check (0.2 / 0.2 / 0.4 mm, and 0 below the bar). Passing under the top foot from \|X\| 75 to 51.5, the barb has to be pre-deflected 0.38 mm. The ramp handles that.
- **Hold:** outward is strong (1.1 mm engagement on a 90° catch, and compression keeps it locked). Rearward relies on the 0.6 mm pin. Upward is only about 5 g.
- **Release:** feasible as described, but two-handed: a blade from the rear while pulling from the side.
- **Print:** fails per B2.
- **Tongue arithmetic:** correct.

## Q5: Board carrier
- **Strength:** at 50 g, about 150 N (board 250 g plus carrier 49 g). The edge strip between the window and the edge (21 × 4 mm) reaches about 0.8–1.6 N·m against roughly 2.2 N·m allowable, so the margin is thin but it passes. The inserts are adequate.
- **Vibration:** first mode estimated at 60–100 Hz.
- **Countersunk heads in PETG** tend to creep and loosen; use thread-lock in the metal inserts.
- **Missed interference:** the screw collision in M2.
- **Clearances:** clear of the cable band (heads at Z 41.5, below 42). A thicker ISO 10642 head would sit about 0.26 mm proud, still below 42. Bracket heads clear the board-top screws by 4.2 mm.
- **Assembly:** all six rear standoff screws end up behind the lip or web, as the author noted.

## Q6: Thermal
See M7. Measure with a thermocouple run with the shell closed before relying on any PETG clamp.

## Q7: Print orientation and settings
- Sides: see M1.
- Hooks: see B2.
- Carrier: printing it flat, rear face down, is fine.
- A fit test needs:
  - a frame with shelf rails at Y 353.62 (rev A has not been cut)
  - a dummy 95 × 162 × 79 block with inserts, because the pack does not exist yet

## Q8: Contradictions between files
- **Pack lead path:** the frame README says X −35 to 30, while the cradle README and `parts.json` say X −5 to 25.
- **Stale carrier description:** the frame README says the carrier is "screwed to the front ends of the tray rails" and gives "tray rails \|X\| 47.5–55". It is actually on cradle pads, with webs at \|X\| 48–51.5.
- **Board count:** the frame README still describes two PCBs (100 × 95 and 100 × 55); rev B is one 100 × 150 board.
- **Fastener counts:**
  - The 96 T-nuts and 76 M5 × 10 omit the cradle's 8 M5 and 8 drop-in T-nuts.
  - The shopping list lacks the 4 M3 × 8 countersunk, 12 M3 × 6, 6 standoffs and 2 pack-side M4 inserts.
- **Countersink angle:** 90° in the README, 82° in the geometry.
- **Insert size:** M3 × 5 in the code, M3 × 5.7 in the frame shopping list.
- **Thumbscrew stop:** "head seats on boss" against "with shoulder".
- **M5 torque:** BUILD.md's 4 N·m is applied to the PETG feet.
- **Mass totals:** the gyroid set total is given as 129 g; the parts sum to 128 g. Trivial.

**Verdict: NO-GO** — fix B1, B2, M1 and M3 before printing the sides and hooks; the carrier can print now if its screws change (M2).
