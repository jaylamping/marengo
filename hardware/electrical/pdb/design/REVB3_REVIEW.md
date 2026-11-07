# Power board rev B3: schematic review, round 3

2026-11-07. The packet was built from main at 02035d4f (57 hashed files). Every run used High effort with a 45 minute limit on Joey-PC. **Result: NO-GO for layout.** I accept three blockers and nine majors. The rev B2 fixes held where reviewers checked them:
- The RUN diodes contain a pin fault.
- C42 stops a held reset arming the latch.
- Q12's negative-logic drive is right.
- The converter move took care of the brick's heat and fit problems.

What failed was my own arithmetic and one diode put in backwards.

| Reviewer | Tool | Effort | Minutes | Verdict | On-target check |
| --- | --- | --- | --- | --- | --- |
| Fable | Claude CLI, API | high | 30.0 | NO-GO | Pass, 65 of 65 names, 9 of 9 values. $13.91 |
| Muse (Wonder) | Muse | high | 14.9 | NO-GO | Pass, 97 of 97, 14 of 16 |
| Sol | Codex | high | 23.2 | NO-GO | Pass, 70 of 70 |
| Astra | Codex | high | 32.7 | NO-GO | Pass, 76 of 76 |
| Opus | agy | high | 6.3 | None | Antigravity quota ran out mid-run ("resets in 2h36m"). Fell back to the API per Joey's routing: API, 28.6 min, NO-GO, pass 63 of 63 names. $6.66 |
| DeepSeek V4.1 Flash (trial) | OpenCode review agent, CheaperInference | high | 12 + 6.5 | NO-GO | Names pass (128 of 128, 39 of 51 values), but most of its blockers are false. Try 1 hit OpenCode's default output cap ("length"); fixed in config, retried once |

Hashes matched before and after every run. ERC was clean and check.py passed 83 of 83 at build.

## Blockers (accepted)

| # | Finding | Raised by | How verified | Smallest fix |
| --- | --- | --- | --- | --- |
| B1 | **D12 is backwards for the fault it was added for.** Its anode is on UVLO and its cathode on FLT_A. A U1 pin 2–3 bridge drives UVLO to the pack, and D12 conducts that straight into U8 FLT (20 V abs max). U8 can then die in an unknown state, so "one fault loses one bank at most" is false. My note claimed the opposite. | Sol, Astra (blockers); Muse, Fable | Netlist: D12 pin 1 FLT_A, pin 2 UVLO | Replace D12 with 10k from FLT_A to UVLO and a 15 V zener from FLT_A to GND. FLT low still holds UVLO near 1.6 V (22k from RUN_B, 33k ∥ 10k). A bridged UVLO puts 3.9 mA into the zener, and FLT sees 15 V |
| B2 | **The absorber is sized wrong twice.** First, a battery still pushing during current decay gives an overshoot of I·√(L/C), not an energy balance. At 200 A, 1 µH and 47 µF that's +29 V, not +7 V (Sol, Astra). Second, the board's own trips interrupt far more than 200 A in a bus short: about 500 A for B's breaker and about 800 A for A's comparator with its TMR delay and the 150 Ω pull-down (Fable). At 500 A the peak is over 100 V on U1, U8, Q8/Q9, D7 and C38. D7 and C38 also have no qualified part. | Fable, Sol, Astra, Muse | Arithmetic re-done: 54.6 + 200 × 0.146 + ESR is about 90 V; 500 A is about 128 V | (a) Remove C29, so U8's comparator is at its fastest; R37 alone still latches. (b) R35 150 → 22 Ω: the absorber now takes the commutation, so slowing A buys nothing. (c) Two C38 in parallel, with an MPN. (d) D7 to a part with a documented µs surge rating, same SMC footprint. (e) **Measure pack-lead inductance before layout.** At 0.15 µH (150 mm twisted) Fable's 530 A case lands at 78 V |
| B3 | **The converter interface isn't defined well enough to build safely.** (Opus also raised the J24 re-pin as a major.) F2 is "part TBD": a 32 V MINI blade at 54.6 V would arc. J24 puts +VIN opposite +VOUT. The DVE has no reverse-polarity protection, and a row-mirrored harness would feed it backwards indefinitely through U7's limit. | Muse (blocker); Fable (minor) | Datasheet: "Input Reverse Polarity: None" | Pick a 58 V-rated 20 A fuse by MPN. Re-pin J24 so +VOUT faces −VIN. Draw the harness by pad position against both Molex drawings. Add a first-article polarity check |

## Majors (accepted)

| # | Finding | Raised by | Fix |
| --- | --- | --- | --- |
| M1 | The reset interlock uses an absolute threshold (MID over about 11 V). With B shorted and the bus still holding charge above 11 V, reset is allowed and A hard-closes. | Fable, Sol, Astra | Decision for Joey: (a) also require VBUS low (about 3 more parts), or (b) remove Q10/Q11/D14/R48–R50 and rely on procedure plus the software self-test. I recommend (a): the button is hardware, so software can't stop a press |
| M2 | U7's timer can expire before the converter-end capacitance (about 48 µF plus the DVE's unpublished internal capacitance) is charged at the low power-limit corner. Each retry then starts from zero. | Fable, Muse, Sol, Astra | C19 47 → 100 nF (3.1 to 8 ms) |
| M3 | 47 µF straight on BAT, through the diode, breaks the "nothing bulky ahead of the soft start" rule. When the BMS closes through 1 µH, the surge into C38 can reach 240 to 370 A for about 20 µs. Whether the JK BMS's short-circuit trip ignores that is unknown. | Fable, Sol, Astra | Bench it on the real BMS, or ask JK. Doubling C38 (B2) makes the surge worse, so decide both together |
| M4 | The shorted-U2 race (F1 against D9 against the 40 V parts) isn't proven. C5 is rated 25 V against D9's 24.4 V clamp, before dynamic resistance. | Muse (blocker), Sol, Astra | C5 → 50 V. Close the race with 0466 I²t and SMAJ15A curves, or test it on a sacrificial board |
| M5 | A Q4 or Q5 C–E short makes the latch re-arm on release with no reset. Stop still works. | Sol, Muse (major); Fable (minor) | Accept explicitly with the single-channel risk, or add a second seal-in element. Joey's call |
| M6 | The MID healthy window and the self-test need explicit preconditions: bus discharged, the interlock's load, the 1 MΩ VBUS input, leakage. MID is about 0.465 × pack, not 0.48 to 0.50. | Sol, Astra, Fable | Software self-test spec: VBUS under 3 V before reading MID; widen the band to 0.38–0.55 × pack |
| M7 | Input-filter damping at the converter isn't specified. The pigtail resonance is 208 to 294 kHz, inside the DVE's switching band. The 33 µF aluminium's ESR is the only damping. | Sol, Astra, Fable | Specify the 33 µF by MPN with an ESR range, or raise C14/C15 to 10 µF to move the resonance below 150 kHz |
| M9 | **The DVE has no output capacitance at its pins.** Murata asks for 100 µF to 10,000 µF low-ESR, and a 1 µF + 10 µF pair across the output pins. All of ours sits on the board, behind about 100 to 150 nH of pigtail, and the DVE has no sense pins. Expect ringing on load steps. | Opus | Add 10 µF + 1 µF X7R and 100 µF or more low-ESR polymer at the converter's output pins, in the rail's fuse and cap zone |
| M8 | Servo back-feed into C41 isn't bounded, and U9/U18 don't block reverse current. | Sol, Astra | Bound it in software (neck servo current cap is already required), or add a reverse-blocking diode on SPARE_SW. Low energy; keep as a requirement |

## Minors (accepted)

| Finding | Raised by |
| --- | --- |
| R15 1.2k 0402 runs at 72 % of its rating; R9 runs at 215 mW with LOOP_OUT shorted; R48 has 44 V across an 0402. Move to 0603 and 0.5 W | Fable |
| RST_IN is a 1 MΩ node AC-coupled to the trigger: R11 → 10k cuts harness pickup about 100× | Fable |
| U8's comparator spread is 134 to 313 A with the ISCP bias tolerance, not 158 to 242 A. U2's current limit is 51 to 200 mA, not 309 mA (that only slows VCTL, so the sim stays conservative) | Fable |
| U1's power limit is disputed: Fable gets 544 W typ (413 to 675 W), and earlier rounds got 652 W. Check against TI's design calculator | Fable |
| HS50 burst figure: the winding hits its hot spot before the housing absorbs 1.4 kJ. A stuck-on Q6 more likely opens the resistor first, which is a safe failure | Fable |
| RS2 to RS7: Bourns lists CSS4J-4026**R**-1L00F, not -4026K (carried over from B2) | Astra |
| Pi-off back-power through D6 at 0.5 mA in a harness fault | Astra |
| The 57.5 V clamp worst case rests on typical comparator hysteresis | Astra |

## Rejected

| Claim | Raised by | Why |
| --- | --- | --- |
| All INA228s share one I²C address | DeepSeek | False: the A0/A1 straps are in the netlist and check.py checks all nine addresses are unique |
| The converter soft start is reversed | DeepSeek | False: RS9 runs BAT → BRK_D, Q7's drain is on BRK_D and its source on BRK_IN |
| D6 nets are swapped | DeepSeek | False: D6 1 GND, 2 +3V3_PI, 3 ESTOP_PI_N |
| The interlock doesn't work at a 36 V pack | DeepSeek | Misread a zener-plus-divider as a plain divider. Q11 still gets about 27 µA at MID 16.6 V |
| A stuck-on Q6 shorts the pack | DeepSeek | The HS50 is in series: 5.5 A |
| Order the positive-logic DVE (P variant) and drop Q12/R51/D15 | Fable | Valid simplification, but a broken ON/OFF wire then means converter on. Kept the N variant |

## Usage

| Reviewer | Tokens | Cost |
| --- | --- | --- |
| Fable | 898 in, 123k out (106k thinking), 544k cache write, 3.7M cache read | $13.91, from `total_cost_usd` |
| Opus (agy, quota out) | 537k in, 32k out, 3.8M cache read | Antigravity quota (5-hour limit hit) |
| Opus (API) | 98 in, 159k out, 359k cache write, 8.4M cache read | $6.66, from `total_cost_usd` |
| DeepSeek | Two tries | $0.056 + $0.076, estimated at CheaperInference list prices |
| Muse, Sol, Astra | No per-run figure | Subscription |

## My errors

- **D12:** I put it in backwards and wrote a note claiming the opposite.
- **Absorber:** I sized it with an energy balance that leaves out the battery, and for a current the board's own trips don't respect.
- **Two numbers:** I carried 309 mA for U2's current limit and 0.48 to 0.50 for MID.
