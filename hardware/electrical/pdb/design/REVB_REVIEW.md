# Power board rev B: schematic review

2026-11-07. Packet built from main at ee4967b2 (54 hashed files), run on Joey-PC. Three reviewers were asked, two gave usable reviews. **Result: not ready for layout.** Two blockers and 11 majors are accepted. Both blockers come from the second bank added in 11e71d78. Nothing has been fixed yet: Joey sees the findings first.

| Reviewer | Tool | Elapsed | Verdict | Note |
| --- | --- | --- | --- | --- |
| Fable high | Claude CLI, API key | 19.1 min | NO-GO | Python allowed for reruns |
| Wonder | Muse | 8.6 min | NO-GO | |
| Argon high | agy 1.5.2 | 31.4 min on the sixth try | **discarded** | Reviewed a design that doesn't exist, see below |

Hashes matched before the run and after every run. Each finding below was checked against the netlist or datasheets unless the row says otherwise. SMDJ58A, HB2-BS545, HS25, BAT54S and shunt datasheets weren't in the packet.

## Blockers (accepted)

| # | Finding | Raised by | How verified | Smallest fix |
| --- | --- | --- | --- | --- |
| B1 | **Close order is wrong, so the soft start can fail.** RUN releases U1 and U8 together. U8 can't close bank A until C28 (1 µF) reaches BST UVLO: about 26 ms typ, 52 ms worst (9.93 V at 190 µA). U1's gate rises at 100 to 220 V/s and passes the 2.5 V minimum threshold in as little as 11 ms. Bank B can be on when A closes, and the bus caps then charge through B with no dV/dt control. The sheet's "A closes first" is false. | Fable, Wonder | Netlist RUN to R5 and R27; TPS4810 and LM5069 tables; IPB017 p4 | Delay U1 UVLO by more than 60 ms (RC, with a diode for fast drop-out), or gate it on bank A being closed |
| B2 | **Bank A's hard turn-off can pass 100 V at the pack input.** U8 sinks 2 A through 10 Ω into 195 nC gates, so A opens in well under 1 µs with full load current. SMDJ58A is about 93.6 V at 32 A, so around 110 V or more at 60 A. U1 and U7 VIN, SENSE and OUT are 100 V abs max and don't avalanche. C1 alone gives about 60 V of rise at 60 A with 1 µH. | Fable, Wonder | Datasheet ratings; L di/dt and √(L/C) arithmetic. TVS figure and lead inductance are not from the packet | Slow A's gate pull-down to a 5 to 20 µs edge: stop time is already set by the latch's 100 µs. Then measure at the bench |

## Majors (accepted)

| # | Finding | Raised by | How verified |
| --- | --- | --- | --- |
| M1 | Reset with bank B shorted closes A straight onto the bus. Only the BMS trip or Class T fuse then stops it. The sheet's "do not press reset" is an instruction, not protection. Fix options: U8's short-circuit comparator on RS1, or a hardware interlock on MID. | Fable, Wonder | Netlist. U8's comparator is grounded out, same as rev A gate 3 M11 |
| M2 | The 12 V budget doesn't survive derating. Expected load is 121 W. The brick is rated 100 % only to about 30 C at 20 LFM, falling to about 50 % at 70 C. Reviewers read about 75 % at 50 C, or 9.4 A against 10.1 A. It's outside the fan zones, with no heatsink. Needs forced air, a frame heatsink, or a smaller budget. | Fable (major), Wonder (blocker) | Spec notes 1 and 2 confirmed. The curve values are the reviewers' reading: the plot isn't in the extracted text. Kept as major: no failure until torso temperature is known |
| M3 | The E-stop control channel is single-point. A welded NC contact, a LOOP_OUT to LOOP_RET bridge, or RUN shorted to VCTL holds both banks on. The BMS has no remote-off. The banks only cover a failed FET or driver. Needs Joey's explicit acceptance, or a second break. | Wonder (blocker), Fable (noted) | Netlist. Kept as major: not fixable on this board, and rev A decided Cat 0 on the same basis |
| M4 | U1's power limit is about 652 W at 54.6 V, 480 to 810 W with tolerance, not 544 W. The sheet used 125k × Rs × P. The datasheet is 1.3E5 × Rs × (P − 1.18 mV × VDS / Rs). U7 is about 70 W, not 60 W. The start-into-short check needs redoing: TI's 1.3× margin is met only at the low end. | Fable. Wonder confirmed 544 W with the sheet's formula | Equation reproduces the datasheet's 14.9 kΩ example and the 25 mV spec point |
| M5 | BMS telemetry can bypass the low-side BMS. A non-isolated USB-RS485 adapter ties B− to P−, so load current can use the comm cable when the BMS opens. Specify isolation in the parts list. | Fable, Wonder | REV_B_PARTS has no isolation requirement. BMS port reference assumed |
| M6 | The clamp takes 5.7 A, 322 W, against 30 A of host regen. With the pack unable to accept charge, the drives fault at 60 V. Rev A accepted this, but rev B never restated it. HS25 is a 25 W part with no pulse or thermal check. | Fable, Wonder | Thresholds verified. Rev A rationale, "Regen, decided". Needs Joey's decision |
| M7 | A short on SPARE_ON can brown out compute. The polyfuse takes tenths of a second to seconds to trip while the brick is in overload. A second TPS259541 at about 2 A would fix it. | Wonder (major), Fable (minor) | 2 s at 8 A listed trip time. Scales to about 0.3 s at 20 A |
| M8 | Drive capacitance is still unmeasured, and the 9.4 mF ceiling relies on FET sharing in linear mode, which isn't analysed. | Wonder | Packet says unmeasured. Measure before layout |
| M9 | U7's timer depends on the brick's unpublished input capacitance. About 40 µF is the limit: above that it retries forever and compute never starts. | Fable (minor), Wonder (major) | Arithmetic checked with the corrected limit. Measure one brick |
| M10 | No thermal model. Two banks double the loss to 10.4 W at 60 A, about 2.6 W per FET at about 40 K/W on minimal copper, on a board with no spare area. | Wonder | IPB017 p3. Burst duty probably helps, unproven |
| M11 | The floorplan doesn't close: 100.5 % of the budget with legs, Z 67 unapproved, and bank A two blocks from bank B, adding a 60 A detour and loop inductance to B2. | Wonder (major), Fable (minor) | Floorplan |

## Minors (accepted, fix with the blockers)

| Finding | Raised by |
| --- | --- |
| U8 FLT floats. The datasheet says connect it to GND if unused. | Fable, Wonder |
| Self-test window: U17's input is 0.8 to 1.2 MΩ, so MID is 0.43 to 0.47 × pack, 15.5 V at 36 V, under my 16 V floor. A shorted reads only 2 V over the 34 V threshold at empty pack. VBUS bleed takes about 11 s on board caps, minutes at 9.4 mF. | Fable, Wonder |
| Pressed input: a harness short to the bus puts 0.8 W in R38 (1206) and 15 mA into the Pi rail. A broken wire reads released. 1 mA wetting is unproven for a 10 A silver contact. The Pi must debounce. | Fable, Wonder |
| J20 pins 3 and 4 are adjacent. Shorted with the button pressed, RUN sits near 2.2 V: B opens, but EN_A stays above U8's thresholds, so A stays closed. The stop still works through B. | Fable |
| U8 turn-off delay is 4.6 µs typ, 6.5 µs max through EN, not 1.2 µs. EN falls before INP1. | Wonder |
| C28 and C5 lose value under DC bias. Use larger cases or 2.2 µF and 10 µF. | Wonder; Fable on C5 |
| SPARE_SW: the budget's 4 A is above U9's 3.87 A minimum limit, so treat it as a ceiling. U9 doesn't block reverse current. D5 clamps above OUT's VIN + 0.3 V limit. A shorted U9 stays on through an E-stop. | Fable, Wonder |
| SPARE_ON polyfuse hold falls to about 1.4 A at 60 C. | Fable |
| Latch simulation predates the additions. Added load is under 2 %, but rerun it. | Wonder |
| Still unverified pads: IQB footprint mirror, U8 thermal pad and pin 1, shunt sense pads, Q6 gate. D6 must be the S variant. | Fable, Wonder |
| Added by me after checking Argon's leads: SPARE_SW and SPARE_ON use the same connector as each other and the compute outputs, so a swapped plug leaves servos live through an E-stop. Rev B also dropped rev A's series resistors on INA228 inputs (10 Ω IN±, 100 Ω VBUS). | Lead from Argon, checked against the netlist and rev A log |

## Can wait

| Finding | Raised by |
| --- | --- |
| The 60 A Class T fuse is below BMS DOC 80 A and U1's limit. It opens first on a sustained 100 A event. That may be intended. | Fable |
| U10 and RS10 are a removal candidate, since U6 already counts SPARE_SW. Saves about 1.9 cm². | Fable |
| No EMC filter on the brick. | Wonder |
| Leg shunts' pulse rating against pigtail fuse I²t, when legs arrive. | Wonder |
| Bus cap land for the 16.5 mm can, already on the sheet. | Fable, Wonder |
| Thermostat part isn't specified. | Fable |
| No BMS-open spin-down test on the list. Add it to the bench plan. | Wonder |

## Rejected

| Claim | Raised by | Why |
| --- | --- | --- |
| "VCTL lives as long as the bus or pack does" is false, so regen can outlive the clamp. | Wonder | U2 is on BAT, but VBUS reaches BAT through both banks' body diodes, so regen keeps VCTL up. |
| Move the thermostat in series with the resistor. | Wonder | In the loop it opens both banks and removes the pack. The real gap is that no thermostat is specified. |
| Self-test reads false "B shorted" with the BMS open. | Wonder | The Pi has no power with the BMS open. |
| "Size the TVS for a 5 to 7 µs edge." | Wonder | That's propagation delay. The current edge is still under 1 µs, see B2. |
| Brick spec calls for 1000 µF on the input. | Wonder | That's the output capacitive load table. The input parts are for EMC. |
| UVLO arms at 4.0 to 4.7 V, not 5.7 to 6.1 V. | Fable | The 21 µA is a sink while UVLO is low, so rising is 5 V + 21 µA × 33 k = 5.7 V. The sheet is right. |

## Argon: discarded

The review is 100 KB and says NO-GO, with 9 blockers, but it isn't about this board. None of these appear in the schematic, netlist or checks: R29 30.1 kΩ, IWRN, Q8 to Q16, BSS138, LM4040DBZ-3.0, a 33 MΩ resistor, a TPS259540 on MID, a 160 × 110 mm board, a 100 A continuous bus, or `check.py`'s quoted `vref = 3.3`. The ten questions it restates aren't the packet's questions. It read the real files (174 reads) and tried files that don't exist (`gen_revb.py`, `build.py`), and it used a subagent. I can't tell where the invented design came from. Nothing from it is accepted without its own check against the netlist. Do not cite it. Two of its leads were real enough to check, and they're listed under Minors.

## Usage

| Reviewer | Tokens | Cost |
| --- | --- | --- |
| Fable | 514 in, 79,063 out, 542,292 cache write, 1,155,251 cache read, from the session transcript | About $12 (estimate: cache write 1.25×, read 0.1×). Check the console for the billed figure |
| Wonder | No per-run figure | Subscription. Account total 10 % of window, 14 % of week |
| Argon, usable try | 1,221,257 in, 148,770 out, 46,740 thinking, 3,163,270 cache read | Subscription. Account total 96 % of week and 89 % of 5-hour Gemini limit left |
| Argon, try 5 | 365,101 in, 15,700 out, 1,126,717 cache read | Same |

Argon's tries 1 to 4 ended in under 30 s each, about 10 to 26 k tokens where reported.

## My errors

B1, B2 and the FLT finding are in the block I added and described as checked. "A closes first" was never tested against the charge-pump time. M4 was carried over from an equation I didn't re-derive. The self-test floor ignored the INA228's tolerance. The usage notes in hardware/docs/review-recipe.md predicted $3 to $4 for Fable, and this run was about three times that.
