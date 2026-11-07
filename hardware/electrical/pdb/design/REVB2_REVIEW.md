# Power board rev B, fixed board: schematic review

2026-11-07. Packet built from main at 29a6598c (54 hashed files), run on Joey-PC at High effort with a 45 minute limit. **Result: NO-GO for layout.** Two blockers and 11 majors are accepted, plus one blocker and two majors from Opus's late run. Earlier close-order and wiring problems did not come back: every reviewer who checked FLT's static behaviour and the pin maps found them right. What's left is mainly protection that isn't proven, and numbers of mine that were wrong. Nothing here has been fixed yet.

| Reviewer | Tool | Effort | Minutes | Verdict | On-target check |
| --- | --- | --- | --- | --- | --- |
| Wonder | Muse | high | 15.8 | NO-GO | Pass, 47 of 50 values |
| Astra | Codex | high | 19.7 | NO-GO | Pass |
| Sol | Codex | high | 22.0 | NO-GO | Pass |
| Fable | Claude CLI, API | high | 23.9 | NO-GO | Pass. $10.44 from the JSON output |
| Argon | agy on Joey-PC | high | 17.8 | Discarded | **Fail**, 15 of 49 values. **Removed from the group by Joey** |
| DeepSeek V4.1 Flash (trial) | OpenCode Go | high | 45 + 23, timed out | None | No review. First run ended at 59 s on a denied shell call. The retry hit the Go quota and hung |
| GLM 5.3 (trial) | OpenCode Go | high | 45, timed out | None | No review: hit the Go quota at 08:39Z, then hung |
| Kimi K3.1 Pro (trial) | OpenCode Go | high | 45, timed out | None | No review: hit the Go quota at 08:42Z, then hung |
| Opus (agy) | Antigravity quota | high | 24.6 | NO-GO | Pass, 42 of 42 names. Late, folded in below |
| DeepSeek V4.1 Flash (trial, late) | OpenCode review agent, CheaperInference | high | 12.6 | NO-GO | Pass, 49 of 49 names, 16 of 18 values. Folded in below |
| GLM 5.3 (trial, late) | OpenCode review agent, CheaperInference | high | 24, cap | None | Stopped at the $2 spend cap after 40 steps, before writing a review |
| Kimi K3.1 Pro (trial, late) | OpenCode review agent, CheaperInference | high | 15 + 6 | None | The provider cut the stream both times ("The stream ended before completion") |

Hashes matched before the round and after each run. All pass ERC and check.py. Each finding was checked against the netlist or datasheets unless the row says otherwise.

## Blockers (accepted)

| # | Finding | Raised by | How verified | Smallest fix |
| --- | --- | --- | --- | --- |
| B1 | **Pack-side turn-off still isn't bounded.** My 150 Ω fix doesn't set di/dt. Crss is 25 pF at 50 V, so once VDS rises the gate loses control and di/dt follows the clamp voltage and lead inductance. U1's own breaker also trips with 230 mA of gate pull-down, in 1.2 µs. D1 clamps 93.6 V at 32 A and 120.9 V at 160 A, on a node of 100 V controllers. A controller killed this way may fail with its gate high, which is an E-stop failure. | Fable, Sol, Astra, Wonder | IPB017 p4, LM5069. TVS data from Littelfuse via Sol and Astra | Diode-fed absorber on BAT (47 to 100 µF, 100 V, with bleed), so the BMS never sees its inrush, plus a 5 kW class TVS. Measure lead inductance and scope a 60 A stop |
| B2 | **The brick's place isn't settled.** At 50 C the curve gives about 50 % with no heatsink at 20 LFM, and 75 % with one: 6.3 A and 9.4 A against 10.1 A. A heatsink plus 100 LFM is full output to 70 C. But the top reaches Z 66.8, and the brick's own +0.5 mm takes it past the unapproved Z 67. The body also runs 1.15 mm into the fan zone. | Wonder (blocker); Fable, Sol, Astra (major) | IQB p4 curve read by me, p8 tolerance. Sol read 55 and 75 %, Astra 60 and 80 % | Decide before placement: frame-plate brick on a pigtail, or get Z 67 approved with tolerance, and specify the airflow |

## Majors (accepted)

| # | Finding | Raised by | How verified |
| --- | --- | --- | --- |
| M1 | My "comparator covers reset with B shorted" is wrong. A closes hard, and current passes 200 A in about 4 µs at 1 µH while the comparator waits about 10 µs. It's a late breaker, not a limit, and the BMS may trip first. Removing C29 is the fastest setting. A hardware reset interlock would be the real fix. | Sol, Astra (blockers); Fable, Wonder | Datasheet timing. Kept as major: it needs a shorted B and a reset after a failed self-test |
| M2 | Accepting the clamp with a 5 A cap doesn't cover energy. HS25 is 25 W on its heatsink, so 322 W is 7.8 % duty at best. The cap can't bound passive regen in an E-stop, a fall or a BMS event. A Q6 short gives 298 W until a thermostat nobody has specified opens. | Sol, Astra (blockers); Wonder; Fable on the thermostat | HS25 data via Sol and Astra. Kept as major: it's off-board and doesn't hold layout. **Joey's acceptance needs revisiting with an energy limit** |
| M3 | U9 is the only break for the neck servos. If it shorts, they stay powered through an E-stop. | Sol (blocker), Astra; Wonder and Fable (minor) | Netlist. Needs a decision: a second switch, or accept |
| M4 | The NO input feeds the Pi rail. A bus short puts 15.5 mA through R44 (0.8 W in a 1206) straight into +3V3_PI, 20.7 mA with D6. My note counted only 5 mA, and a powered-off Pi can't absorb it. | Astra (blocker), Sol, Wonder; Fable (minor) | Netlist, arithmetic |
| M5 | The 1.5× SOA margin is wrong: about 1.05 to 1.1×. The sheet scaled the curve as √t, and TI uses t^0.346. 16 A at 0.82 ms and 70 C against 14.8 A. Fix: C3 to 4.7 nF. | Fable, Sol, Astra | Three independent readings agree. I didn't re-read the graph |
| M6 | U2's capacitors are at the minimum. C5 is 4.7 µF nominal in 0805 against a required 4.7 µF effective, and C1 is 1 µF. If VCTL goes unstable, the latch and clamp go with it. Fit 10 µF and a dedicated input cap. | Sol, Astra, Wonder; Fable (rev B round) | TPS7A4001 p11 |
| M7 | U2 has no reverse protection. OUT is limited to IN + 0.3 V, and BAT can collapse under C5. | Sol | TPS7A4001 p4 |
| M8 | The latch simulation is stale, and reset held at power-up may arm it: C6 couples VCTL's ramp into L_B, 0.24 to 0.88 V across R17 against about 0.55 V hot. | Fable (mechanism); Sol, Astra, Wonder (stale evidence) | Netlist. Mechanism not simulated. Rerun ngspice |
| M9 | Neither eFuse handles back-driven servos. OUT is limited to IN + 0.3 V with no reverse blocking, and regen can feed the compute rail. | Sol, Astra | TPS2595 p5 |
| M10 | MID thresholds ignore FET leakage: up to 100 µA per FET at 125 C against about 0.14 mA of divider current. Valid only cold, with defined supply and wait conditions. | Sol, Astra | IPB017 p4 |
| M11 | Carried over from the last round, still unmeasured: drive capacitance, brick input capacitance, FET thermal model. | All four | |

## Minors (accepted)

| Finding | Raised by |
| --- | --- |
| "RUN is the only shared signal" is now false: FLT also links U8 to U1. FLT shows BST is up, not that A conducted. If the FLT trace is open, close order is lost. Test it at bring-up. | Sol, Astra, Wonder |
| U8 lacks the recommended 22 nF from SRC to GND. Add one on MID. | Fable, Sol, Astra |
| My notes: close time is about 57 ms typ, 35 to 115 ms, not 26 ms. "A short stops in 5 µs" is wrong: the eFuse holds its limit until thermal shutdown, then retries. The 158 to 242 A band is interpolated. | Fable, Sol, Astra, Wonder |
| RS2 to RS7 part number: Bourns lists CSS4J-4026R-1L00, not 4026K. C10 to C13: maker lists HHXF800ARA101MJH0G. Check both before ordering. | Sol, Astra |
| R40 at 1 % can fall below the 487 Ω minimum. | Astra |
| 57.5 V isn't a guaranteed ceiling: comparator hysteresis is typical only, and reference drift adds. Fable and Wonder get about 57.4 V. | Sol, Astra |
| At 36 V, a brick overload can reach U7's 9.7 A minimum limit. | Fable |
| U9 and U18 FLT are unconnected, so the Pi can't see retries. | Wonder |
| Positive-opening and minimum load for the HB2-BS545 are assumed. Joey's acceptance rests on the first, so get the contact block's data. | Sol, Fable, Wonder |
| Bus caps' land is 14.3 mm for a 16.5 mm can. | All four |

## Joey's decisions, 2026-11-07 10:58Z

- **M3 accepted.** U9 stays the only break for the neck and jaw servos. Joey: "the neck shouldn't ever be moving too fast so i think the risks are acceptable at this time."
- **Fit:** merge the six limb "−" terminals into two shared GND studs, Fable's idea. It needs a current check on the studs.
- **Open:** the brick on a frame plate (B2), and a bigger braking resistor (M2). Those areas stay parametric until he answers.
- GLM 5.3 and Kimi K3.1 Pro are out of the reviewer group for now. DeepSeek V4.1 Flash stays as a trial on CheaperInference.

## Braking resistor: HS50 (Joey, 2026-11-07, "for redundancy")

Arcol publishes no pulse or short-time overload rating for the HS range; its datasheet says to ask (sales@arcolresistors.com). So I sized it on thermal mass. I assumed 0.9 J/g·K, half the mass taking heat in a burst, and 100 K of rise from a 50 °C ambient, well under the 200 °C hot spot. These are estimates, not ratings.

| Part | On std heatsink | No heatsink | Mass | Burst estimate | At 322 W | Body (L × W × H) | Holes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| HS25 (was) | 25 W | 9 W | 14 g | 0.6 kJ | 2 s | 51.0 × 28.0 × 14.8 | 18.3 × 19.8, Ø3.2 |
| **HS50** | 50 W | 14 W | 32 g | 1.4 kJ | 4.5 s | 72.5 × 28.0 × 14.8 | 39.7 × 21.4, Ø3.2 |
| HS75 | 75 W | 24 W | 85 g | 3.8 kJ | 12 s | 72.0 × 47.5 × 24.1 | 29.0 × 37.0, Ø4.4 |

- **The worst burst I can name** is a fall once legs are on: 40 kg dropping 1 m is 400 J, if all of it came back as regen, which overstates it. The arm stage is tens of joules.
- **Why HS50:** the HS25 covers that fall with about 1.5× margin, and the HS50 with about 3.5×. So the HS50 is the smallest part that gives the redundancy Joey asked for.
- **What none of them cover:** a stuck-on Q6 (298 W continuous). Every HS size up to the HS150 needs the NC thermostat in the loop for that, so it stays.
- **The 10 Ω stays.** The clamp math is unchanged.
- **Fit:** it does **not** fit where the HS25 went. The front bottom rail slot is |X| ≤ 25.5, so 51 mm, and the HS50 is 72.5 mm long. Its 28.0 mm width (datasheet A max) fills the Y 154 to 182 slot with no margin, the same as the HS25. In that orientation it would need |X| ≤ 36.3, which runs into the rule to keep the LP bottom-edge connectors at |X| > 36. The frame thread has to place it.
- **Price:** about $2.30 to $4 each in snapshots (Newark $2.26, Octopart $2.71, Findchips $4.06 at 10). Digi-Key is HS5010RJ-ND with a 21-week lead time. Check live stock.

## Fix round 2 (2026-11-07)

On the sheet, block 10 holds the new parts, so earlier refs keep their numbers. ERC is clean, check.py passes 80 of 80, and the courtyard total is 8,832 mm²: 98 % of the one-sided budget with legs, down from 100.6 %.

| Item | Fix |
| --- | --- |
| B1 | Absorber on BAT: MBRS3100 into 47 µF 100 V polymer (ESR 30 mΩ or less), 100k bleed. A 200 A, 1 µH lead adds about 13 V, so about 70 V peak. D1 is now 5.0SMDJ58A. The BMS sees 47 µF (0.07 J) when it closes: check it at the bench |
| B3 | UVLO and EN_A take RUN through their own BAV19W. U8 FLT reaches UVLO through a BAV19W, so a UVLO fault can't reach FLT. R5 is 22k, so UVLO is 3.8 V at RUN 7 V |
| M12 | J20 is 8 pins: GND between loop return and reset source |
| M13 | 33k from MID to U17 VBUS. Calibrate the 3 % gain |
| M14, M6, M7 | U2 is fed through F1 (250 mA, 63 V, 1206) with its own 1 µF. VCTL is clamped by SMAJ15A, has 10 µF 1206, and a BAV19W from OUT to IN |
| M4 | The NO pull-up goes through a BAT46W, and the Pi resistor is 100k: a bus short leaves 0.5 mA in the clamp |
| M5 | Timer 4.7 nF: 0.15 to 0.39 ms, about 1.4× SOA margin at t^0.346. Still read the real curve at layout |
| M9 | 330 µF on +12V takes spare-output shorts and servo back-feed, along with the compute loads |
| M10 | MID divider is 47k + 47k: hot leakage moves MID by under 5 V, so the self-test bands (0.4 to 0.6 × pack healthy) hold |
| minor | 22 nF from MID to GND for U8 |
| Fit | Six limb "−" terminals replaced by two GND studs. Check the stud and lug current |
| Open | M8: rerun the latch sim at power-up with reset held. M1: no hardware reset interlock yet. M11: measurements. Brick placement (B2) waits for Joey's converter pick. The floorplan PNG predates this round |

## The fit decision

None of the four reviewers chose moving parts to the back face, which is the plan Joey approved. Fable argues against it: 6 mm standoffs, the pack behind, no airflow. Sol and Wonder prefer smaller RS9 and RS10 shunts; Astra and Fable prefer dropping U10 and RS10. Fable's other idea is the best one I've seen: the limb "−" terminals are all GND, so merging six into two shared studs frees about 690 mm², enough to cover B1's absorber too. B1 adds 150 to 300 mm² at the battery entry, so this needs deciding again. It needs a current check on the shared studs.

## Follow-up: Opus through agy (late)

Opus ran on the Antigravity quota, finishing at 09:33Z: 24.6 min, 903k input, 138k output and 8.8M cache-read tokens. The hashes matched after. It repeats B1's spike, B2, M2, M3, M6 and the RS part-number minor. These are new; I checked the first two in the netlist:

| # | Finding | How verified | Smallest fix |
| --- | --- | --- | --- |
| B3 | **One solder bridge defeats both banks.** U1 pin 2 (VIN, on SH_P) sits next to pin 3 (UVLO) at 0.5 mm pitch. If they bridge, U1 turns on whatever RUN does, and the pack drives RUN backwards through R5 (33k) into R15 (1.2k) and the dividers. RUN sits at about 1.7 V with the E-stop pressed, so EN_A is about 1.6 V, above TPS4810's 1.08 to 1.19 V turn-off. A stays on too. The bridge also puts pack voltage on U8 FLT (20 V abs max). So "RUN is the only shared signal" is true on the schematic, not in a fault. | Netlist: R5 RUN–UVLO, R15 RUN–GND, R27/R28 10k/100k, R38/R39 100k/68k. LM5069 p3 pinout. My own node arithmetic agrees | Feed R5 and R27 from RUN through separate diodes. Keep FLT off a node the pack can reach: a series resistor and zener at FLT, or a cascode. Coat U1 pins 2–3 |
| M12 | **J20 pin 3 (LOOP_RET) is next to pin 4 (RST_SRC, VCTL through 4.7k).** A short there, or a Q4 or Q5 C–E short, holds RUN at about 2.1 V with the E-stop pressed. B opens but A stays enabled, and the board re-arms on release without reset. Positive opening doesn't cover it. check.py only checks pins 1–3. | Netlist J20 pin map. Opus's arithmetic, not re-run | Re-pin J20 with GND between pins 3 and 4 |
| M13 | U17 VBUS (pin 8) sits on MID with no series resistor. The B1 spike above 85 V can kill it and carry pack voltage toward +3V3_PI. | INA228 p3 | 20 to 47k in series, recalibrate |
| minor | +12V has only 2 × 22 µF bulk, so a spare-output short can dip compute. If anyone fits the brick's suggested 220 µF input cap, U7's timer can't charge it and the brick never starts. Brick OVP up to 170 % is about 20.4 V, over TPS2595's 20 V IN. | Opus. The OVP figure isn't checked | Add 220 to 470 µF on +12V. Note the BRK_IN limit. Check the OVP figure |

Rejected: the brick footprint "insert diagonal" (closed by my own check below), and "a 97 A limit trips a 100 A class bus" (the design is 60 A, so this is my prompt's wording again, though a demand peak over 97 A for 0.3 ms is worth measuring). Opus also adds an area option: drop U10 and RS10, and move Q7 to a 5×6 package, which gives about 97.7 % with legs, if Q7's start-into-short SOA holds.

## Follow-up: DeepSeek V4.1 Flash (late trial)

DeepSeek ran on CheaperInference with the read-only review agent: 12.6 min, about $0.06, hashes held. Most of it repeats the accepted list (M2 clamp energy, B2 brick, M4 NO input, M6 C5, M3 U9, SPARE_SW at its limit, the 57 ms close time) and Opus's M12. New and real:

| # | Finding | How verified | Smallest fix |
| --- | --- | --- | --- |
| M14 | U2 (TPS7A4001) is a single part between the pack and VCTL. An input-to-output short puts up to 54.6 V on VCTL, and U3, Q4 and Q5 are 40 V parts. What they fail into isn't known, so the latch could fail on. | Netlist VCTL members. Failure mode not analysed | Clamp VCTL (15 V zener or TVS sized to blow a fuse) or add a resistor ahead of U2 |
| minor | The brick needs an external input capacitor ("external input capacitor required", IQB p2 note). U7's 1.5 to 3.8 ms timer at 70 W can't charge much: it fits about 50 µF. Opus raised the same with 220 µF. DeepSeek's 1000 µF is the output-load column, not the input | IQB text, Opus's arithmetic | Get the required value from the maker, then size C19 and the power limit for it |
| minor | SPARE_ON (U18) has no output capacitor | Netlist: SPARE_ON has only J23 and U18 | Add 1 µF |
| minor | INA228s strapped to SDA or SCL need the datasheet's hold-time care | INA228 address table | Check the Pi's I2C timing, or move those addresses |
| minor | A harness short to the bus overloads R9 (1.3 W in 0.25 W) and R10 (0.38 W in 0402) | Arithmetic | Larger parts, or accept: the harness short already stops the robot |
| minor | No protection per 12 V output: a fan short hiccups the brick and drops compute | Netlist J5 to J7 | Fuse the fan output at least |

Wrong: the clamp "loses power 2.4 ms after a BMS trip" (with a low-side BMS everything floats together, and regen holds VBUS and VCTL up), the A SOA "~228 A at 20 µs" (scaled from one 10 ms point), and 300 °C body diodes in sustained regen (the banks are on in normal running). It also called FLT on the UVLO node "valid" and missed Opus's B3. Its clamp number, 56.46 V, ignores the 19.1M hysteresis resistor; I get about 56.6 V on. The note's 56.8 V needs a recheck.

## Closed by my own check

| Item | Result |
| --- | --- |
| Brick footprint mirror (Wonder B3, Fable m6) | Correct. Pins and inserts match the p8 top view. Not a finding |

## Rejected

| Claim | Raised by | Why |
| --- | --- | --- |
| Add a backup RC delay on U1 UVLO. | Wonder | A bring-up test covers an open trace, and the discharge path is the problem already rejected in the fix |
| BRK_PG needs a pull-up. | Wonder | R.C open means on, and PGD only sinks |
| 6 to 10 µs gate arithmetic passes. | Wonder | Superseded by B1 |
| 100 A continuous isn't demonstrated. | Astra | My prompt's "100 A class" wording. The design is 60 A |

## Argon: discarded and removed

The review is 102 KB with five blockers, but 34 of its 49 cited values don't match the netlist, and it describes an SMCJ58A on MID, CS_SEL on pin 16, and parts that aren't on the board. Its questions aren't the packet's. This run used no subagent, read 298 files itself, and was told to cite only what it saw, so subagents aren't the cause. Two off-target reviews in two rounds meet the removal rule. Joey removed it at 09:07Z and waived the effort trial.

## Usage

| Reviewer | Tokens | Cost |
| --- | --- | --- |
| Fable | 930 in, 99,692 out, 352,719 cache write, 4,134,541 cache read | $10.44, from `total_cost_usd` |
| Argon | 1,029,951 in, 92,311 out, 13,180 thinking, 2,214,745 cache read | Subscription |
| Wonder, Sol, Astra | No per-run figure | Subscription |
| DeepSeek, late | 376k in, 114k out and reasoning, 4.6M cache read, from the JSON | About $0.06 at CheaperInference list prices. The dashboard is the bill |
| GLM, late | 1.28M in, 86k out, 5.8M cache read. Little of the input was cached | $2.02, stopped at the cap |
| Kimi, late | Two tries | $0.83 + $0.25 |
| OpenCode trials | Go console: 77 requests, 3.9M tokens. Kimi 1.7M cache read, 883k cache write, 42k out. GLM 293k in, 712k cache read, 98k out | **$3.59 on the Go console**: Kimi $2.56, GLM $1.03, DeepSeek under $0.01. That used all of the 5-hour window, 40 % of the week and 20 % of the month. OpenCode's own figure was about $0.10 |

## Open models: none delivered

The Go plan's quota ran out mid-round. GLM and Kimi hit "Go usage limit exceeded" at 08:39 and 08:42Z, and DeepSeek's retry hit it on its first request. `opencode run` doesn't exit on that error: it hangs with no output until the timeout. Permissions weren't the cause. They score as time-limit misses. The recipe now probes first, kills a run on a quota error, and uses a read-only agent with no subagents, one model at a time. They rerun as a late trial once the quota resets or Extra Usage is on.

## My errors

M1, M5 and the notes above are mine: I claimed protection and margins I hadn't proven, and B1's fix used a gate-charge estimate where the physics doesn't apply. On the brick, I took 9.4 A as a no-heatsink figure when it's with a heatsink, and I gave a heatsink without the airflow that actually fixes it.
