# Torso frame rev A gate review (packet 2, 2026-11-06)

**Verdict.** Cutting the four posts is low-regret, but the build is not ready to start. Do not build the pack tray, the board carrier or the harness/cooling layout as specified. Order the gussets only after the end-anchor concept is chosen and the lower-box width is confirmed final.

The posts can be cut because their two lengths are fixed by things that will not change. The shoulder axis sits at Y 424.86. The existing pitch bracket bolts to the step side rail at Y 363.69, which pins Y 353.62 for the top of the lower posts. None of the findings below moves either of those.

The packaging claims do not hold. In `parts.json`, the pack, its tray rails and the board carrier are not attached to anything. Three cable paths run through solid aluminium, and two bracket sets still collide with frame parts.

**How I checked.** Python and Node needed approval in this session and weren't run, so I could not regenerate the files from `frame.py`. Every coordinate below was computed by hand from `frame.py` and then checked against values pulled out of `data/parts.json` with grep. Labels used below:
- **[V]**: verified from the coordinates.
- **[A]**: depends on an assumption I state.

Head sizes for M5 button-head screws use the ISO 7380 standard (9.5 mm across, 2.75 mm tall).

## Claims checked

| # | Claim | Result |
|---|---|---|
| 1 | Geometry and cut lengths | **[V]** 198.62 + 121.38 + 20 = 340. Clear widths 110 and 186, bay 156.8, axis 269.86 above the frame bottom, all correct. The rail lengths close. |
| 2 | Hardware counts | **[V]** 64 + 36 + 20 + 8 = 128 screws and 128 T-nuts; 32 brackets. README §7 item 5 says "28 × 2028 + 4 × 2020", which contradicts the 24 + 8 used everywhere else. The fastener table in §2 still says M5 × 16. |
| 3 | Pack position | **[V]** Coordinates and margins are correct (7.5 to the posts, 8 top and bottom, 25 at the rear). The pack is **not** under the shared rail: the shared rails sit at \|Z\| 50–70 and the pack at Z −45 to 34. Its ceiling is the shelf plate at Y 354.6. |
| 4 | Board position | **[V]** 34 + 3 + 0.5 = 37.5 and 70 − 61.5 = 8.5 are correct. The standoffs are modelled 2 mm tall, not 6: carrier Z 37.5–41.5, board starts at Z 43.5. The M3 screw heads are modelled at Z 61.5, 16 mm above the PCB. |
| 6 | No interference | **False.** See Q3. |
| 7 | Built frame | Photos are consistent with single 2020 rail and printed bottom corners. I could not see post lengths. |

## Findings

### Blockers

**B1. The pack, tray rails and board carrier touch no frame member.** **[V]**
- The lower posts occupy only \|Z\| 50–70.
- `packrail_±1` is X ±47.5–55, Z −50 to 40. Its only contact with any post is a line contact with the rear post edge at Z −50, and it stops 10 mm short of the front post.
- `pdb_carrier` is X ±50, Z 37.5–41.5. It is 5 mm from the post faces in X and 8.5 mm short of them in Z.
- "On the lower post inner faces" has no geometry behind it.
- Smallest fix: see Q1.

**B2. No cable path exists between the lower box and the upper bay.** **[V]**
- The top of the lower box is closed by the shelf plate (X ±55, Z ±50), the shelf rails (X ±55–75) and the shared rails (Z ±50–70).
- `harness_step` (X 33–45, Y 340–376, Z 50–60) runs through `rail_shared_1` for X 33–45, Y 353.62–373.62, Z 50–60. It also runs through the `br2` leg at Y 349.62–353.62.
- The "Z 50 to 60 gap between the shelf plate and the shared front rail" does not exist: the plate ends at Z 50 and the rail starts at Z 50.
- `harness_down_±1` (X ±47–55, Z 62–70, Y 160–370) runs through:
  - `rail_bot_1` at Y 160–175
  - both legs of `br0`/`br12` and of `br2`/`br14`
  - `rail_shared_1` at Y 353.62–370
- The arm pigtails, 12 V, USB, CAN and fan leads therefore have no route.
- Smallest fix: reprint the shelf plate with front-corner notches of about 30 × 20 mm at \|X\| 27–55, Z 30–50. This also fixes M1. The alternative is to route the trunks outside the frame's front face inside the shell.

### Major

**M1. The step-tier brackets collide with frame parts.** **[V]**
- The 2020 step-side brackets `br25`, `br27`, `br29` and `br31` have legs at X ±73–93. They still run 2 mm into the shelf rails (X ±55–75) at X 73–75, Y 353.62–373.62, \|Z\| 46–50. The gap is 18 mm and the legs are 20 mm long; the gate-review fix only went from 28 to 20.
- The shelf-rail end brackets `br24`, `br26`, `br28` and `br30` (2028) collide with the existing shelf plate:
  - X 27–55 × Y 354.6–372.6 × \|Z\| 46–50
  - X 51–55 × \|Z\| 22–50
- "Shelf plate stays as built" cannot hold.
- Smallest fix: trim the 2020 legs to 17 mm or end-tap the step side rails, and notch the shelf plate as in B2.

**M2. The generator puts every bracket screw head inside the bracket leg.** **[V]**
- Example: `f0_1_h` is centred at X 52.75, which puts the head at X 50.5–55, inside the leg at X 51–55. The T-nut `t0_1` sits at X 52.5–58.5, across the leg/post interface.
- So the packet's clearance check misses every bracket head. With real heads on the free face of the leg:

| Bracket heads | What they hit |
|---|---|
| `br0`/`br12` | Logic-board envelope at X ±48.25–50, Y 190–193.75, Z 55.25–61.5 |
| `br2`/`br14` | Power-board envelope at X ±48.25–50, Y 334.87–340, Z 55.25–61.5 (the arm-port terminal corners) |
| `br2`/`br14` leg + head | The ring-lug / 10 AWG bend zone the board README reserves (Y 340–353.6, Z 43.5–60). Frame metal sits at X ±27–55, Y 346.9–353.6, Z 50–60, so the lugs must stay inside \|X\| < 27 or below Z 50. |

**M3. Electrical shorts to the frame.** See Q2. Live M4 lugs and arm pigtails end up 0–7 mm from frame-potential bracket heads and the shared rail, and the carrier isn't held. **[V]** for the geometry; **[A]** for the housing grounding.

**M4. Brake resistor.** **[V]** for the geometry.
- `brake_res` touches the PETG tray rail along Y 180 (0 mm) and sits 3 mm under the pack's bottom row.
- It clamps at 330 W, and firmware allows a 40 W average. A 100 mm piece of 2020 is not the heatsink area a 50 W clad resistor is rated on **[A]**.
- The documents give three locations: the −X side rail (README), the front bottom rail (board README and the `frame.py` comment), and a "TO-247 class" part in `parts.json`. The board README instead names an Arcol HS50.
- An HS50 is roughly 30 × 49 × 15 mm plus tags **[A]**, which does not fit the 25 × 50 × 8 envelope.
- Fix: put it on the bottom front rail, away from the PETG, and model the real part.

**M5. The step joint per-bolt load is understated.** See Q7.

**M6. Several screws are too long for the slot.** **[A: 2020 slot depth]**
- The generator's own T-nut bottom is 7.5 mm below the face.
- M5 × 20 through 12 mm of PETG (hardpoints) protrudes 8 mm.
- M5 × 12 through the 4 mm pitch-bracket ears protrudes 8 mm.
- In both cases the screw tip reaches the profile core before the head seats, so the part is never clamped. Use M5 × 18 and M5 × 10 respectively. Measure the slot first.

**M7. The cooling path does not exist.** See Q5.

**M8. The spine rail lip collides with the rear gussets.** **[V]**
- The lip (\|X\| ≤ 60, Z −82 to −70) overlaps the rear gussets at X ±55–60, Y 355–372, Z −73 to −70.
- It also hits the gusset screw head at (±62, 363.62), head extent X 57.75–60, Z −77.5 to −73.
- Fix: limit the lip to \|X\| ≤ 52.

**M9. The shoulder harness overlaps the Jetson.** **[V]**
- `harness_shoulder_±1` (X ±42–78, Y 447–457, Z −32 to −22) overlaps the Jetson at X ±42–51.5, Y 447–455.
- The README keep-out (Y 440–460) overlaps the Jetson too, which contradicts "2 mm above the Jetson top".

**M10. The spec contradicts itself on the gate decision.**
- README §6 still says the main battery is "not in the torso frame". It also says a torso pack means "the lower box is too small … the one decision that would reopen the frame".
- The locked 13S2P decision puts the pack in the torso, so this rule needs an explicit reconciliation.
- Other stale lines: §7 item 8 ("PDB behind the D2 door") and the fan table ("1 intake, Y 250–290").

### Minor

- **Upper box bracket and actuator clearances.** **[V]**
  - `br3`, `br9`, `br15` and `br21` overlap the pitch-bracket wing envelope by 1.62 mm (Y 400–401.62, \|Z\| 50–69).
  - The RS03 sits 0.64 mm from the top side rail, 1.74 mm from the step side rail and 0.5 mm from the posts in Z. Set the top rails' height from the pitch bracket's Y 485 bolt, not flush with the post tops. Cut the upper posts to 121.38 with +0.5/−0 tolerance.
- **Pack extraction clearance.** **[V]** The rear-bracket heads leave 0.75 mm in X (head at 48.25, pack at 47.5), 1.25 mm at the bottom and 1.87 mm at the top. A real case wider than the nominal 95 mm will snag.
- **Side harness band.** **[V]** The band (X 50–55, Z 40–50) has 0 mm clearance on all four sides: board edge, tray rail, the `br0` leg, and the sharp post corner.
- **Small modelling issues.**
  - `pi_stub` rests on top of the `br24` leg (0 mm) and floats 1 mm above the shelf plate.
  - There are two IMU envelopes: `imu_old` and `bno085`.
- **Frame mass.** Items 1–11 sum to 2154 g; adding item 34 and the fasteners gives about 2.5 kg, not 2.1.
- **Gusset DXF.** **[V]** It matches the spec: R15 at (98, 340.05), 9 × Ø5.5 holes, about 40 g, about 5000 mm². The hole at (62, 363.62) is 7 mm from the X 55 edge, below the usual 1.5d. My tear-out estimate is about 4.7 kN, so it is acceptable.

## Answers to the eight questions

**Q1. Pack retention.**
- At 17 g, the pack loads 2.2 × 17 × 9.81 ≈ 367 N.
- **What holds it in each direction today:**
  - **Down:** an undefined lip on rails that aren't attached to anything.
  - **Forward:** the 3 mm air gap, then the carrier and boards, then the open front of the lower box.
  - **Backward:** 25 mm of free travel, then a magnet-closed printed door. Magnets of this class hold tens of newtons **[A]**.
  - **Sideways:** the unattached rails.
  - **Up:** 9.6 mm of travel, crushing the 10 AWG pack leads against the shelf plate.
- **Not enough.** Smallest fix:
  1. Bolt each rail to the rear post's inner face with 2 × M5 drop-in T-nuts at Y about 220 and 310. That height is between the bracket legs at 175–203 and 325.6–353.6.
  2. Add a foot on the bottom side rail at \|Z\| 25–50.
  3. Give each rail a lip under the pack at Y 180–183, a front stop at Z 34 so the rail takes the forward load and the carrier doesn't, and a top hold-down lip at \|X\| 35–47.5.
  4. Add a screwed rear latch, for example 2 M4 thumbscrews into the rails. The door must not be the restraint.

**Q2. Electrical-mechanical safety.**
- **What can short to the frame:**
  - The power-board top-edge lugs (pack input and VMOT) and the arm-port terminals at the top corners, 0–7 mm from the `br2`/`br14` heads, the bracket leg and the shared rail.
  - Pigtails pinched in the zero-clearance side bands against the post edge.
  - Pack leads pinched under the shelf plate if it is metal.
  - The brake resistor leads.
- **Pack to boards:** 3 mm air + 0.5 mm Nomex + 4 mm PETG is adequate as a barrier, as long as the carrier is held.
- **Does the 1 MΩ ∥ 10 nF bond make it worse?** Not by itself. But:
  - A first B+ or VMOT fault to the frame is silent.
  - A second fault then shorts through the frame. If it is on the pack input, upstream of the BMS and E-stop FETs, only the pack fuse protects.
  - If the RS03 housings are internally tied to GND **[A, measure it]**, the frame is effectively GND and the 1 MΩ does nothing. Fault current then returns through each actuator's internal ground.
- **Fixes:**
  - Measure the RS03 housing-to-XT30 GND resistance.
  - Boot every M4 lug.
  - Cap the bracket heads near the board.
  - Add a frame-potential sense on the supervisor.
  - Note that the bond "at a carrier standoff" needs its own wire, because the carrier is PETG.

**Q3. Interference.** All items are in B2, M1, M2, M4, M8, M9 and the minor list above. I found no other overlaps among the frame members, gussets, hardpoints, fans, Pi or Jetson. The Pi's clearances are 2.0 mm to H3 rear and 2.38 mm to the shared rail.

**Q4. Serviceability.**
- **Pack:** remove the M3 lock screw, open D2, unplug the XT90S and the sense connector at Z −45 to −55, slide the pack out. Nothing else needs to come off, but it has the 0.75–1.9 mm head clearances above. The unplugged board-side 10 AWG lead lies loose on the pack and can be pinched on reinsertion; add a clip to park it.
- **Boards:**
  1. Remove the pack.
  2. Remove the front shell panel and unplug the fans in it.
  3. Unclip the front-corner trunks.
  4. Remove about 15 M4 lugs from the front.
  5. Undo the carrier fixings (not yet designed).
  6. Lift out forward between the bracket legs: the opening is 102 wide over Y 203–325, and the carrier is 100 × 150.
- So the boards are removable, but only with the shell, the pack and every pigtail off.

**Q5. Cooling.**
- **Not plausible as drawn.** The lower box is sealed:
  - top: shelf plate and rails
  - bottom: the 150 × 140 waist adapter plate
  - back: the pack, behind a solid carrier
- The intakes blow into an 8.5 mm gap. The only exits are the two 5 × 10 mm side bands, which are already full of harness, leading into the lower shell's V1 side grilles. No path reaches the upper-rear exhausts.
- **Hot spots:** the brake resistor during clamping (PETG and cells within 0–3 mm); for the board, the BMS FETs and shunt at Y 294–318 plus the lugs in the dead-end pocket at Y 340–353.6 under the shelf plate.
- Fix: treat the V1 side grilles as the lower-box exhaust, and treat the upper bay as a separate zone.

**Q6. Build and shopping list.** What Joey would find missing on the bench:
- **Drop-in T-nuts:** 48 of the 50 are already committed. The tray rails, carrier, Jetson shelf and clips need about 20 more, and they go into post slots already blocked by the `br0`/`br2` nuts.
- **Fans:** the list says 3; the model has 4.
- **Not listed:** M3 T-nuts for the resistor, M3 standoffs, the Nomex sheet.
- **Pitch bracket:** M6 hardware if the bolts turn out to be M6.
- **Screw lengths:** M5 × 18 instead of × 20 (M6 above).
- **Shelf plate:** a reprinted or notched plate.
- **2028 brackets:** "16 used" doesn't match the photos (printed bottom corners), so count what is on hand.
- **Stock:** if the built frame really used 2200 mm with no shelf side rails, item 5 is new stock too. One 4 × 500 pack still covers it.
- **Existing mounts:** the photos show existing T-nut screws on the post inner faces; remove them before cutting.

**Q7. Step joint with the pack.**
- The pack adds about 367 N, about 7% shear on the 5.3 kN lower-body load. With an arm of about 90 mm, against much longer arms for the legs, it adds only about 1–2% moment. So the pack itself makes it only marginally worse.
- The stated item is worse than written:
  - 860 N per bolt is 4.3 kN over 5 bolts. Each member group in the plate has only 3 bolts, and the load must pass through each group, so 4.3 / 3 ≈ 1.43 kN per bolt against the 1 kN of friction assumed.
  - Post uplift on the tension side runs along the vertical slots: both the gusset post bolts and the single bracket per post are friction-only.
  - M6 could make the clamp zero.
- **Order the gussets first?** Only if the anchor is a tapped post end bolted through the shared rail. That leaves the DXF unchanged and the plates are cheap. If the anchor needs a plate feature, wait.

**Q8. What would make cutting and ordering now a waste.**
- **Gussets:** wasted if the lower-box width changes, because the X 65 hole line and the X 55 edge follow the posts. The spec's own §6 rule says a torso pack reopens that width. Also wasted if the anchor needs a plate feature.
- **Posts:** only at risk if a built post isn't 340, or if the upper posts are cut more than 0.6 mm short with the top rails then set flush.
- **The real waste** would be printing the tray rails, carrier and harness clips to this envelope set, since B1, B2 and M1–M4 all change them.
