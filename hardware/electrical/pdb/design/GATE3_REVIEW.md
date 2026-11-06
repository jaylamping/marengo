# Power board rev A: gate 3 schematic review

2026-11-06. Packet built from main at 525283a4 (62 hashed files) in a scratch worktree. Six independent reviewers, all from the Mac. **Result: not ready for layout.** 4 blockers and 18 majors are accepted. The schematic has not been changed for any of them. Each finding below was checked against the netlist, BOM, footprints or a datasheet, and how is stated. Raw reviews stay in the scratch packet, not in the repo.

| Reviewer | Tool | Elapsed | Note |
| --- | --- | --- | --- |
| Wonder | Muse | 4.2 min | |
| Sol | Codex | 10.7 min | fetched vendor datasheets during review |
| Astra | Codex | 11.8 min | fetched vendor datasheets during review |
| Opus high | Claude CLI, API key | 17.0 min | Python blocked by the recipe's tool list |
| Fable high | Claude CLI, API key | 18.5 min | Python blocked by the recipe's tool list |
| Argon high | agy | 36.4 min | stopped empty at 25.5 min on a denied command, exit 0; resumed 10.9 min with a wider read-only list |

Hashes checked before the run, after it, and after the Argon resume: all match.

## Blockers (accepted)

| # | Finding | Raised by | How verified |
| --- | --- | --- | --- |
| B1 | **VMOT precharge cannot run.** U302 EN/UVLO (pin 1) and INP1 (pin 3) are both on ESTOP_EN_B. G2 needs EN high, and EN high closes bank B, so no state has Q307 on and bank B open. Every arm hard-closes onto 1.8 mF. Separately, the 0.5 s timeout is below the 0.595 s needed to reach 2 V (0.71 s at +20 % C). `check.py` asserted the wrong wiring as correct. | Opus, Sol, Astra, Argon; Fable and Wonder saw only the ordering risk | Netlist; TPS4810-Q1 pin table; arithmetic |
| B2 | **D220 shorts VSYS to the charge port.** The 2-pin symbol lands K on pad 1 and A on pad 2 of TO-277A. On that package pads 1 and 2 are both anode leads and pad 3, the tab, is the cathode, so the part is bypassed and the cathode is unconnected. Port is live and has no reverse protection. Argon flagged pad 3 unconnected but read the pinout wrongly; I traced the rest. V15PM20 itself found no listing: pin data is from sibling V10PM15, same package. Part number also needs confirming. | Argon (partly), me | Netlist; KiCad footprint; Vishay SMPC diagram |
| B3 | **PWR263S resistors are open circuit as drawn.** Same class as B2: R315, R523, R623, R723, R1023 (and DNP R823, R923) use a 2-pin symbol on TO-263-2, whose pads 1 and 3 are the leads and pad 2 is the tab. The tab is stated as isolated from the resistor, so the second terminal lands on nothing. Precharge and all mini clamps are dead. Tab isolation comes from a mirrored Bourns excerpt, pin drawing not seen. | Me, from a pad audit after B2 | Script comparing every footprint's pads to netlist pins; only D220 and these parts have an unused lead |
| B4 | **Tap fuses cannot interrupt a harness short.** SF-1206FP100-2 is 50 A interrupt. B13 to B0 is about 146 A through about 0.37 Ω, at the cells. Needs 1.09 Ω or more in the loop, or a higher-breaking part. | Sol, Astra (blocker); Opus, Wonder (major); Fable (minor) | Bourns figure in log; arithmetic |

## Majors (accepted)

| # | Finding | Raised by | How verified |
| --- | --- | --- | --- |
| M1 | Hold-up does not hold. U1104 (3V3_AON) VIN and EN are on VSYS ahead of Q1101, so the supervisor dies and compute enables pull low with the bank full. | Fable, Sol, Astra, Wonder, Argon | Netlist |
| M2 | R1111 pulls BUCK12_OFF up from 3V3_AON, so any supervisor reset, including the watchdog the design relies on, stops 12 V, compute and the E-stop loop. | Opus, Fable, Argon | Netlist |
| M3 | Port enable fails on. R510 and siblings pull EN_INV from VCLAMP12. Lose U401 and Q503 drops, UVLO releases, and all ports ignore PORT_EN with the clamp dead too. | Opus, Fable (minor) | Netlist; log's enable logic |
| M4 | A hung supervisor leaves 16 to 20.5 A on wiring treated as 15 A, indefinitely. The log's "at most 9 ms" holds only at the limit. A hardware-started watchdog plus PORT_EN's reset default may close the hung case; a shorted Q501 stays open. | Sol, Astra, Wonder (blocker); Opus | Arithmetic; netlist |
| M5 | BMS predischarge timeout closes DSG on expiry, voltage or not. Set timeout 0 with Stop Delta. R209 is a generic 2512; VSYS load is about 1.46 mF via Q1101's body diode, 2.2 J, not 1.1 mF. | Sol, Astra (blocker); Fable, Opus, Wonder, Argon (R209) | TRM section on Predischarge Timeout; netlist; BOM |
| M6 | DFETOFF 0x02 leaves CHG on with the sense harness out. Needs BOTHOFF, 0x42. | Sol (blocker) | TRM Table 6-5, OPT[4]. One byte, so not a blocker |
| M7 | Pack return runs through the B0 tap. J2.3/4 join BAT- with J3.1, in parallel with the 10 AWG return, through 1 A fuse F2201. Estimated 0.75 to 1.9 A at 60 A, and cell 1 reads wrong by the return drop. TRM offers Cell 1 Interconnect compensation. | Opus, Argon | Netlist; TRM 4.1.3. Return resistance assumed |
| M8 | Presence does not cover the cell header. J2.13 returns through B0_F on the sense board, never through J2202 or F2201. | Argon | Netlist |
| M9 | DSG turn-off of 10 µs is not supported. TI describes a pulsed discharge toward VSS gated by LD, and gives only 2 µs typ, 20 µs max at 20 nF. Ours is 48 nF. The 150 A per FET SOA case needs a recheck, and sharing is unmodelled. | Fable, Sol, Astra, Muse, Argon | BQ76952 section 16.5 and FET driver table |
| M10 | OCD and OCC tolerance was misread. At 20 mV it is ±4 mV, so OCD1 is 53 to 80 A. OCC is 40 to 55.5 A, over a 50 A limit. | Sol, Astra; Opus, Wonder in part | BQ76952 V(OC_ACC) table |
| M11 | TPS48100 short protection is grounded out, so a VMOT short only clears by BMS SCD or fuse, and compute goes with it. | Fable, Opus, Wonder, Sol | Netlist; log |
| M12 | E-stop has no re-close latch, one loop feeds both optos, and +12 V sits on the same connectors (J2001.1, J2003.1), so one short holds it closed. Cat 0 was decided; the shared +12 V and auto re-close were not. | Opus, Sol, Astra, Wonder | Netlist |
| M13 | Stuck-on clamp: 298 to 330 W into a 50 W part with only firmware to stop it. Needs a thermal cutout. Fable's blocker lowered: opening the E-stop clears it unless the supervisor is also hung. | Fable, Sol, Astra, Opus, Argon | Log; arithmetic |
| M14 | Voltage windows overlap. OVLO minimum 57.9 V is below clamp-on top 58.19 V. The 58 V port fuse sits below the mini clamp. | Fable, Opus, Wonder, Sol | Log values; arithmetic |
| M15 | No surge table. SMDJ58A conducts from 64.4 V and clamps at 93.6 V, above 60 V drives, 80 V caps and INA228's 85 V. DSG turn-off also reaches 100 V at about 41 A in the TVS, so "keeps the FET out of avalanche" is unproven. | Wonder, Sol, Opus, Fable (minor) | Sol's cited ratings; log |
| M16 | 12 V can brown out before shedding acts, 100 Hz against tens of µs. Summing amps across rails is invalid, there is no total meter, step 4 at 13 A exceeds 12.5 A, and PG_12V falling drops compute. | All six | Log; arithmetic |
| M17 | CAN short: ±58 V is pin to GND2, but differential is ±45 V. With R1602 fitted, 54.6 V gives 24.8 W in an 0402. R503 bonds GND2 to GND, so the log's "separates logic from motor return" is false. | Sol, Astra, Opus, Wonder, Argon | ISOW1044 abs max; netlist; BOM |
| M18 | Port start-up SOA is unchecked: 5.37 J in the FET over 117 to 257 ms, not the 9 ms case. Depends on unmeasured drive capacitance. | Sol, Astra | Arithmetic from LM5069 values |

Also accepted, lower cost: J101 and J102 carry VSYS on 6 pins and GND on 6, 1.1 A per conductor at 37 V, ribbon rating unverified (Fable). SSM3K56FS's 840 mΩ applies at 50 mA only, not 150 mA, so the 19 mW is unsupported and the top-cell worst case is 2.35 V, not 2.63 V (Sol, Astra; datasheet read).

## Minor

C14 sits on BAT at 54.6 V in a voltage-less 1 µF 0603 BOM row (Opus, Argon; needs its own row, 100 V). PRECHG_EN has no HP pull-down. D1's value reads "100V, verify". U401's exposed pad does not map (symbol EP, footprint 9). Stale text: 298 W should be 29.8 W; ±164 A should be ±136.5 A on 0.3 mΩ; RS201 0.25 mΩ; U201 0x40 against a 0x4C strap; mini clamp is 59.42 V on with TL431 2.495 V, not 59.19 V (Sol, recomputed). Fans and J2001 pin 1 are unfused. RS1 sees 6.75 W at 150 A against 5 W.

## Rejected

| Claim | By | Why |
| --- | --- | --- |
| TLV1805 inputs swapped | Opus (blocker) | Datasheet: IN- pin 3, IN+ pin 4, "reversed positions" noted. SHDN is active high, tied low is enabled |
| ISOM8710 polarity may be wrong | Opus (blocker) | Output low at IF 6 mA, high at IF 0, so the inverter gives EN high with the loop closed |
| E-stop takes 1.81 s, Cat 1 parts fitted | Argon (blocker) | BOM marks C2005, C2006, D2004, D2005, R2007, R2008, R2013, R2014 DNP. ISOM8710 is CMOS output, not open collector |
| TPS62933 EN and RT must not float | Argon (blocker) | Datasheet: floating EN enables, RT floating is a defined frequency |
| TPS56637 pin 5 is VCC, MODE must be set | Argon (blocker) | Datasheet: pin 5 NC, keep floating. MODE floating is FCCM |
| USB2514B OCS_N need pull-ups | Argon (blocker) | Datasheet: internal pull-ups, type IPU |
| STM32G0 symbol scrambled, VDDIO2 missing | Argon (blocker) | 100 of 100 pins match ST pin data; pin 64 is VDDIO2, drawn as VDD on +3V3_AON |
| Argon's minor on footprints BOURNS_PWR263S and others | Argon | Those files do not exist in `marengo.pretty` |
| Jetson and third-board eFuses below 40 W | Argon | PLAN puts Jetson at 7 to 25 W |
| D220 sees 15 A, Argon's thermal case | Argon | Charger is 4 A. The pad fault is B2 |
| Clamp is 57.2 or 57.34 V on, 0.7 V hysteresis | Wonder, Fable | Recomputed with TLV1805 ±7 mV: 57.50 and 56.50 V, as Opus, Sol and Astra found |
| Hold-up ends at 30.3 V | Wonder | That is turn-on. Falling is 18.2 V; 20 V endpoint stands |
| TPS4810 SRC above VS | Fable | Abs max is to GND only, ±100 V range |
| VC16 live before BAT | Fable | VC16 is limited to VSS + 85 V, no BAT-relative limit |
| TPS48100 part may not exist | Fable | SLVSGC0 names it |
| Q209 pinout could defeat the BMS; SSM3K56FS pin order | Wonder | Datasheets: DMP10H400SE G, D, S, tab D matches; SSM3K56FS G, S, D matches |
| LM5145 is 75 V abs max | Opus | 75 V operating, 105 V abs max |
| Fit 2 Ω now | Wonder | Rev A's 10 Ω with drive OV stop was decided; see below |

## Can wait

- 2 Ω has no margin: 30 A × 2 Ω is exactly 60 V, needing about 1.85 Ω or less, which then exceeds D403's 30 A. Goes with the leg stage. Behaviour of a drive after its OV stop while back-driven is unknown and should be bench-tested with one drive before legs.
- Leg channels still carry arm values; DNP.
- OTP table, firmware watchdog and shed requirements: need review before power-on, not before layout, apart from M5, M6 and M10 values.
- G474 BOOT0 on PB8: RXD high boots ROM until the option byte is set, already noted in the log. G0 BOOT0 floating depends on option bytes, unverified.
- SMAJ58CA bus capacitance, mini clamp pulse rating, polyfuse hot derating: measure or look up.
- R103 fault injection, OCD1 delay under 100 ms, and Argon's remaining minors and Q1 to Q8 detail were read but not individually verified.

## For the next pass

- `check.py` tests connectivity against what the generator intended, so it passed B1. It needs pad-to-function checks on every 2-pin symbol on a 3-pad part.
- Recipe: Fable and Opus cannot run scripts with the current tool list, and agy's list needs `sed`, `awk` and similar, or it can exit 0 empty after 25 minutes.
