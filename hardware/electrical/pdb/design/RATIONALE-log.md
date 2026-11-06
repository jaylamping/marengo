# Power board rev A: schematic rationale log

Running log for the KiCad capture, one section per sheet: part chosen and why, what was rejected, datasheet pages and values,
calculations, links to neighbouring sheets, open items. Where a datasheet disagreed with PLAN or the design README, the datasheet
won and the deviation is listed under that sheet.

Datasheets: BQ76952 = TI SLUSE13B (Nov 2021). IPT015N10N5 = Infineon rev 2.4 (2023-05-04). LM74700-Q1 = TI SNOSD17G.

## bms_fets_precharge

**Main path.** PACK_TOP (stack side, sheet 1) → CHG bank → CD → DSG bank → DSG_OUT → RS201 → VSYS. 4 + 4 × IPT015N10N5: 100 V,
1.5 mΩ max at Vgs 10 V, Vgs ±20 V (p3, p4), Ciss 12 nF typ / 16 nF max, Qg 169 / 211 nC, Qgd 34 / 51 nC, Rg 1.4 Ω (p4), plateau 4.4 V.
Gate voltage: BQ76952 11 V mode needs FETs rated ≥ 15 V (p69): met. Voltage rule p69: 5 V DC / 10 V peak per cell = 65 / 130 V at
13S; 100 V meets DC, and peaks are held down by the two SMDJ58A clamps.

**Topology, deviation 1.** The brief said "common source between them". Fig 16-3 is common **drain**: Q2 pins 1,2,3 (sources) on
the stack, Q3 pins 1,2,3 on PACK+, drains joined at node CD, and each gate is referenced to its outer source through R31/D5 and
R32/D6. That is what the charge pump needs (CHG relative to BAT, DSG relative to LD, p44). Captured as common drain.

**Gate network per bank.** One 5.1 kΩ on CHG (Fig 16-3 R37), 10 MΩ gate-source, 16 V zener MMSZ5246B (D5). Each FET has its own
4.7 Ω at the gate to stop parallel FET ringing. 4.7 Ω replaces R34 (0 Ω).

**DSG turn-off, deviation 2.** Fig 16-3 turns off through 7.5 k + diode ∥ 10.2 k = 4.3 kΩ, tuned for a smaller FET load. p72 says
series gate resistance sets the speed: 100 Ω is fast and raises the spike at the top of stack, 1 k or 4.7 k is slower.
On 4 FETs:
- Ciss 48 nF typ. Miller charge 4 × 34 nC = 136 nC (204 max). At plateau about 5 V (150 A per FET) and 4.3 kΩ: 1.2 mA, 120 µs.
- SOA (p6 diagram 3, 25 °C case): at 54 V about 300 A for 10 µs, about 45 A for 100 µs. 120 µs at 150 A per FET **fails**.
- 220 Ω: τ = 10.6 µs, time to plateau 8 µs, Miller 136 nC / 23 mA = 6 µs (9 µs at max Qgd). 10 µs SOA holds 150 A per FET: **passes**.
- Temperature check at 10 µs: 54.6 V × 150 A = 8.2 kW × Zth 0.009 K/W (p6 diagram 4, read off log axes) = 74 K rise.
- Total for the "600 A prospective for 50 µs" gate question: about 40 µs on-state at 216 W for the bank, then about 10 µs of transition.
Drive current 11 V / 220 Ω = 50 mA, below TI's own 100 Ω scope case (p74). Turn-on stays at 10.2 kΩ per TI.
**Unverified:** chart readings are by eye; sharing between 4 FETs during turn-off is not modelled. Scope at bring-up.

**Stack TVS, addition.** p72 warns of an inductive spike at the top of stack. Energy at an assumed 2 µH and 600 A is 0.36 J. D206
SMDJ58A on PACK_TOP keeps the DSG FET out of avalanche. Not in Fig 16-3; added because the 4-FET bank turns off harder than TI's demo.

**Q210 network.** Fig 16-3 Q4/R33/D7/R35/D8. Read from the figure it is an N-FET: source on PACK+, drain on the DSG gate node, gate
held at source by 10 MΩ and pulled toward PGND through 10 kΩ + 100 V diode only when PACK+ goes below PGND. It holds the DSG FET off
when an inductive load drags the output negative. BSS123 and BAV19W assumed (TI's figure gives no Q4 part in this image).

**Snubbers.** C201/C202 = Fig 16-3 C28/C29 across the FETs, two in series so one shorted cap cannot bridge the FETs. C204/C205 =
C30/C31 VSYS to GND. TI's E3 spark gap omitted: no external connector on the board.

**Predischarge, deviation 3.** README 2.1 and the brief said PCHG. p45: PCHG charges an undervoltage *battery*; PDSG, in
PREDISCHARGE mode, charges the *load* before DSG closes. VSYS bulk is the load, so PDSG (sheet 1 pin 39 now wired, pin 40 PCHG floating
as p75). Fig 16-1 has the PDSG FET **source on the mid node CD** and bypassing DSG only, so its body diode does not add a path past the
CHG FET. The brief's "PACK+ to VSYS" would have made one: VSYS → 100 Ω → body diode → stack, with CHG open. Followed Fig 16-1.
Gate-source 1 MΩ (p45: driver sinks 30 µA). Vgs on = −7.5 to −9.7 V (p12). Peak in the 100 Ω: 29.8 W at 54.6 V (gate 3: was misprinted 298 W, and the real load is 1.46 mF, 2.2 J, see Gate 3 fixes). Energy: ½ × 1.1 mF ×
54.6² = 1.64 J per start, τ 0.11 s, 5τ = 0.55 s. **OTP:** enable PREDISCHARGE, timeout > 0.6 s.

**Naming, deviation 4.** The brief's "PACK+" means the stack; sheet 1's hierarchical PACK+ meant the post-FET port. Renamed sheet 1's
to VSYS so PACK_TOP = stack and VSYS = common port, with no PACK+ net left. PACK and LD sense VSYS through 5.1 k + 5.1 k (R18 to R21).
The 40 mV shunt offset between DSG_OUT and VSYS is negligible for LD. Pack + terminal J1 sits on PACK_TOP (BAT_P in the coordinator's naming), ahead of both banks, confirmed from the netlist: J1.1 shares a net with the CHG FET sources, and VSYS is the separate load-side net. The rename touched only the PACK/LD sense label; nothing bypasses the FETs.

**CP1, sheet 1 change.** 470 nF → 1 µF. p65: if gate load is large, raise CP1 (400 nF load → 2.2 µF, 11 V dips to 9 V). Ours: 8 × 211 nC
= 1.69 µC max, droop 1.7 V, start-up about 130 ms (60 ms at 470 nF, scaled).

**Charge input, deviation 5.** PLAN 3.7 said reverse-polarity protected; the brief said FET. Common port is the normal BQ76952 layout
(charger reaches the PACK pin, p44 to p45) and the charger wakes the AFE through LD. But with a reversed charger and a live pack the
blocker sees 54.6 + 54 ≈ 108 V: over the 100 V FET and over LM74700 CATHODE-ANODE 75 V abs max (SNOSD17G section 6.1). Rejected: LM74700 +
FET; P-FET with gate to GND (it conducts VSYS back onto the port). Chosen: D220, Vishay V10PM15-M3/I 150 V 10 A Schottky (gate 3: the V15PM20 could not be confirmed), 0.6 V at 5 A,
about 3 W at 4 A. Side effects: the port is not live, and a port short does not draw on VSYS. Cost: pack end voltage is 0.7 V under the
charger, so use the 54.6 V charger; the supervisor stops at 4.15 V/cell (53.9 V). SMDJ58CA at the port, bidirectional for the reversed case.
**Recommendation stands unless Joey wants the lower loss of a 150 V FET with an isolated gate drive.**

**CHARGER_PRESENT.** BAV19W + 1 M / 62 k from CHG_IN (before D220, so VSYS can't fake it): 3.2 V at 54.6 V, logic high above about 34 V.
3.6 V zener clamp, 1 nF. Goes to LP via the board-to-board connector.

**Main path meter.** PLAN 3.9 needs an independent VSYS meter on its own shunt. RS201 0.25 mΩ between DSG_OUT and VSYS, U201 INA228
address 0x40, ±40.96 mV = ±164 A. Charger current joins after the shunt, so it does not pass through RS201: the cross-check against the
AFE must allow for that during charging. 3.3 V comes in as +3V3_HP, which README 5 does not list: taken from the spare pins.

**Links.** From sheet 1: PACK_TOP, CHG_GATE, DSG_GATE, PDSG. To sheet 1: VSYS. To E-stop banks and LP: VSYS. To LP: CHARGER_PRESENT,
I2C_HP. **Open:** bulk C206 placeholder; 3 W part choice for R209; M4 terminal part and footprint; RS201 footprint.

## pack_input_shunt_afe: changes made after first capture

**RS1 and RS201, deviation.** PLAN asks for 0.25 mΩ 4-terminal. No such part: CSS4J comes in 0.2, 0.3, 0.5, 1, 2 mΩ. Fitted 0.3 mΩ
CSS4J-4026 5 W on both. Rejected 0.2 mΩ: the AFE reads less, and its accuracy below 20 mV is already -2 / +2.65 mV (BQ76952 p22).

**Thresholds rescaled, shunt 0.3 mΩ:**

| Protection | Setting | Current | Note |
|---|---|---|---|
| OCD1 | 24 mV | 80 A (66.7 to 93.3 A) | hardware comparator, delay 100 ms |
| OCD3 | current based | 60 A for 1 s | the agreed 60 A ceiling |
| OCC | 12 mV | 40 A (33.3 to 48.8 A) | 425 ms |
| SCD | 60 mV | 200 A (170 to 230 A at ±15 %) | 30 µs, so bank A acts first |

SCD was 40 mV on paper. On 0.3 mΩ that would trip at 133 A and break the 150 A, 100 ms path rating, so SCD went up to 60 mV.

**Open.** CSS4J
land on the BVR4026 library footprint is to be checked against the Bourns drawing. OTP owed: DFETOFF active high with BOTHOFF (0x42), CFETOFF and
HDQ as thermistor inputs.

## HP_power_board top sheet

**Board-to-board, deviation.** No stock 2×30 2 mm latched header, so two 2×15 ribbons (J101 power and logic, J102 CAN and analog),
all 60 pins used. Each signal keeps one global net across both boards. `check.py` asserts every HP and LP pin pair against
`CONN_A` and `CONN_B`.

Changes from README section 5: BOARD_PRESENT dropped (NTC_BOARD1 reads open when the HP board is missing; PF10 goes to BANK_B_CLOSE, called PRECHG_EN before gate 3),
FLT lines not routed. Added TS2_WAKE, BANK_B_CLOSE, FLT_A (gate 3, in place of GND2 × 6, now GND), +3V3_HP, NTC_BOARD1, NTC_BRAKE, MID_SENSE. VSYS gets 6 pins because LP
draws up to 6.7 A at minimum pack, not README's 5 A.

Port order everywhere: 1 L arm, 2 R arm, 3 waist, 4 L leg, 5 R leg, 6 aux. CAN numbers match. J103 is CAN fault injection on CAN6.

**DFETOFF, deviation.** PLAN drives DFETOFF from the E-stop loop. That deadlocks: 12 V comes from VSYS after DSG, so the loop reads
open before 12 V exists; it would also drop compute on E-stop, against PLAN 3.1 and 3.2. Used instead: PACK_PRESENT_N (1 MΩ from
PACK_TOP, 3.3 V zener to VSS, pulled low by the pack sense harness when mated, 54 µA) diode-ORed by BAT54C with AFE_DFETOFF_SUP
into DFETOFF, 100 kΩ to VSS. Active high, config 0x42 (BOTHOFF, gate 3). The README's hardware loop link was not fitted: no pin left for it.

**Addresses.** Port INA228 straps A1,A0 give 0x40, 0x41, 0x44, 0x43, 0x47, 0x45 for ports 1 to 6. VSYS meter 0x4C, clamp 0x4D.

## estop_banks

**TPS48100-Q1 × 2, simple driver mode** (SLVSGC0 Fig 7-11 p21). No firmware in the path. Bank A VSYS to MID, bank B MID to VMOT,
3 × IPT015N10N5 each. Drains face VSYS so body diodes pass VMOT to VSYS: regen reaches the pack with banks open (PLAN 3.4).
VS, CS+, CS- at VSYS through 100 Ω; SCP_TEST, ISCP, CS_SEL, TMR grounded; CBST 1 µF; EN/UVLO and INP1 both on ESTOP_EN_x. Turn off 1.2 µs by
INP1, 6.8 µs by EN. Turn on 27 to 50 ms. Part is 100 V abs, 95 V operate (docs said 80 V). Gate 2.2 Ω per FET, 1 MΩ gate-source.

**VMOT precharge, deviation (reverses PLAN 3.4).** IPT015N10N5 SOA is 0.7 A at 54 V, 10 ms, so closing a bank onto 1.8 mF is out.
Q307 and 100 Ω PWR263S-35-1000F across bank B, gated by U302 G2. Gate 3 corrected the wiring: G2 follows INP2, so INP2 and EN sit on ESTOP_EN_B and INP1 takes BANK_B_CLOSE.
Loop closed and latch armed: precharge runs in hardware. Firmware closes B once VMOT is within 2 V of VSYS, timeout 0.85 s (0.595 s nominal, 0.71 s at +20 % C). EN low kills G1 and G2.

- Peak 54.6² / 100 = 29.8 W (the brief's 298 W was a tenfold slip). PWR263S-35-1000F passes; W21 3 W does not.
- Energy ½ × 1.8 mF × 54.6² = 2.68 J. τ = 100 Ω × 1.8 mF = 180 ms.

**Status, deviation.** TPS48100 has no VDS output. MID and VMOT dividers 1 MΩ / 56 kΩ (3.3 V = 62 V), 49.9 Ω to ADC, plus VSYS on LP. Bank A
short reads MID near VSYS with both open and VMOT low. FLT to test point only.

**Links.** ESTOP_EN_A/B and BANK_B_CLOSE from LP over J101; MID_SENSE, VMOT_SENSE over J102; VMOT to vmot_bulk_clamp and every port.

**Open.** No KiCad footprint for TPS48100 DGX. Check U302 G2 drive against Q307 Qg.

## vmot_bulk_clamp

**Bulk, deviation.** 220 µF 80 V 10 × 12.5 does not exist; 80 V hybrids top out at 100 µF in 10 × 16.5. Fitted 18 × HHXF800GRA101MJH0G, 1.8 mF.
Rejected 10 × 63 V 180 µF: the caps would sit below the SMDJ58A's 64.4 V conduction point, so the TVS would not protect them. Footprint is CP_Elec_10x14.3 placeholder, land owed.

**Clamp, analog, self-powered from VMOT.** TPS7A4001 12.08 V (464 kΩ / 49.9 kΩ), LM4040A 2.5 V reference, TLV1805 comparator, UCC27517 driver,
IPT015N10N5, 2 mΩ shunt, MBRB30H100CT freewheel, INA228 0x4D. Divider 499 kΩ + 619 kΩ over 51.1 kΩ, hysteresis 19.1 MΩ. The series pair is there
because E96 pairs step 0.7 V.

- Gate 3: on 56.77 V, off 55.77 V (VOH 11.7 V, top resistor 604 kΩ). Spread 56.08 to 57.46 V full temperature, off no lower than 55.1 V. See the window table in Gate 3 fixes.
- Works with the supervisor dead. Budget 5 mA, 0.23 W in the LDO. Stuck on is 330 W, caught by resistor NTC and opening the E-stop.

**Regen, decided (see follow-ups below).** 10 Ω passes 5.75 A against a 30 A host regen limit. With the BMS open mid-brake the excess 24 A raises VMOT at 13.5 V/ms
on 1.8 mF: 2.5 V headroom lasts 185 µs, then drives trip on OV at 60 V. Rev A keeps 10 Ω and accepts the drive OV stop, which
drops torque on the braking limb. The clamp stage is sized so the leg stage swaps in 2 Ω.

SMDJ58A stays as spike protection: it does not conduct under 64.4 V. Its 93.6 V clamp exceeds INA228's 85 V, so sense lines take 10 Ω on IN± and 100 Ω on VBUS.

**Open.** No datasheet in hand for SMDJ58A, BAV19W.

## port_channel (6 instances, legs DNP)

**LM5069-1 latched hot swap.** Per channel: 1 × IPB017N10N5LF (legs 2 ×), shunt, LM5069, INA228, INA241, fuse, load-side mini clamp.

| Value | Arm, waist, aux channel | Calculation or source |
|---|---|---|
| Rs | 3 mΩ WSLP3921 | ILIM window 16.2 to 20.5 A, breaker 26.7 to 43.3 A |
| RPWR | 69.8 kΩ | 200 W limit |
| CT | 100 nF | 4.7 ms typ, 2.8 to 9.0 ms |
| Cdv/dt | 47 nF C0G | Cdv ≥ Cload × 12.9e-6, covers 3.6 mF at 1.7 A |
| UVLO | 191 kΩ / 18.7 kΩ | on 32.0 V, off 28.0 V |
| OVLO | 95.3 kΩ / 4.12 kΩ | off 60.3 V (57.9 to 62.7 V) |

**FET, deviation.** IPT015N10N5 has 0.8 A SOA at 50 V 10 ms; the docs' 10 A is wrong for it. IPB017N10N5LF (Linear FET, D2PAK-7) has 12 A.
Timer check at 9 ms, 58 V: 10.6 A × 0.64 (70 °C) / 1.3 = 5.2 A against 4.3 A worst case limit (200 W, +24 %).

**Shunts, deviation.** WSLP3921 is 2-terminal, so Kelvin sense must come from the land; RS501 footprint is blank until that land exists. No 1.2 mΩ
part: legs use 1 mΩ (48.5 to 61.5 A, RPWR 23.7 kΩ) or 1.5 mΩ (32 to 41 A). Recommendation 1 mΩ.

**Fuse, deviation.** 20 A 0997020.WXN, not 15 A: a 15 A fuse opens at 20 A in 0.75 to 600 s, so it overlaps the 19.4 to 24.6 A trip. The docs' 1.5 mΩ path ignored the fuse. 997 MINI stops at 30 A: leg fuse needs another format, PN open.

**Telemetry, deviation.** INA228 cannot give 1 kHz on six devices (one register 0.85 ms at 400 kHz). 1 kHz current comes from INA241A1 (1.65 V + 10 × Vsense, ±48 A at 3 mΩ, sees regen) into
the supervisor ADC; INA228 at 100 Hz for V, P, E. ADCRANGE 0 (61.5 mV exceeds 40.96 mV). SHUNT_CAL 4500 at the 60 A LSB.

**Enable.** PORT_EN (100 kΩ down) drives Q503, which holds Q504 off. Supervisor dead: Q504 on, UVLO low, port off. Latch reset: drop PORT_EN, 148 ms, raise.

**Mini clamp.** TL431 + BSS123 cascode + DMP10H400SE + 150 Ω PWR263S-35-1500F, load side of the fuse, so a blown fuse with a backdriven arm is covered. On 59.19 V, off 58.20 V,
0.39 A, 23 W. On spread 58.33 to 59.73 V touches main clamp top only at extremes.

**CAN TVS, deviation.** NUP2105L VBR is 26 to 32 V: sustained 54 V kills it. SMAJ58CA per line, to GND. It is an ESD part here, not a ±58 V clamp. Capacitance unchecked. Scope the bus.

**GND2, corrected.** There is no GND2 any more. The earlier claim here, that isolation separates logic from motor return, was false: the drives reference CAN to their power GND, the RS03 housings are at GND, and R503 bonded GND2 to GND anyway. The board is non-isolated. See Gate 3 fixes.

**Deviations minor.** PGD unused (INA228 reads the state). Per-port Z1 omitted, 2 shared VMOT TVS cover it. REDCUBE press-fit M4 not found: THR 74651194R, rating unverified.

**Open.** Drive input capacitance unknown. MMBTA92 for dV/dt PNP unverified. 150 Ω 15 J pulse is off the graph: confirm with Bourns or use W22-150RJI. DMP10H400SE pin numbers inferred. Body 23 W needs thermal check on long regen.

## front_end_12v_24v

**Deviations.** Hold-up: README's "1 mF rides 10 ms at 90 W down to 20 V" gives only 5.0 ms from 37 V. Fitted 20 × 68 µF, see follow-ups below.
24 V is tapped upstream of the ideal diode, or 75 W more drains the bank. LM5050-1 replaces LM74700 (LM74700 is 65 V abs against PLAN's 75 V survive; LM5050-1 is 75 V operate, 100 V abs, but has no reverse-polarity function).

| Block | Values |
|---|---|
| LM5145 12 V 12.5 A | 300 kHz master, 6.8 µH XAL1510-682, CSD19534Q5A + BSC040N10NS5, RFB 33.2 kΩ / 2.37 kΩ, RILIM 348 Ω, PM 52° min, UVLO 30.3 / 18.2 V |
| LM5145 24 V 3.1 A | 300 kHz slave 180°, 33 µH, CSD19534Q5A × 2, RFB 80.6 kΩ / 2.80 kΩ = 23.83 V, RILIM 301 Ω, PM 62° min, UVLO 34.1 / 29.1 V |
| LMR38010 3.3 V AON | 400 kHz, 22 µH, 100 kΩ / 43.2 kΩ = 3.31 V |

**12 V budget.** Sum of maxima about 22 A on a 12.5 A converter (DC limit 14.2 A min). Handled by load shedding, see follow-ups below.

## pol_rails

| Rail | Part | Values |
|---|---|---|
| 7.4 V | TPS56637 | R6 113 kΩ, 5.6 µH (90.9 kΩ = 6.05 V, 130 kΩ = 8.40 V) |
| 5 V | TPS56637 | R6 73.2 kΩ, 3.3 µH |
| 3.3 V | TPS62933 | 31.6 kΩ / 10 kΩ = 3.33 V, 4.7 µH |
| Spare | TPS62933 | stuffed 5 V, 6.8 µH |
| 5.6 V prereg, new | TPS62933F | 60.4 kΩ / 10 kΩ, 6.8 µH |
| Clean | TPS7A2050, TPS7A2033 | 3.3 V through 4.7 Ω |

**Deviations.** TPS7A20 is 6 V max and needs VOUT + 0.3 V, so "after the bucks" cannot work: 5.6 V prereg added. Spare "3.3 to 24 V" is impossible from 12 V; about 10.5 V max (Dmax 0.93).

**Open.** No footprint for TPS56637 RPA. Ceramic bias derating estimated. Bode is an averaged model.

## rail_switches_meters

Metered, switched outputs. Bus A: 24 V 0x40, Jetson 0x41, third 0x42, aux 0x43, Pi stub 0x44. Bus B: 7.4 V 0x40, 5 V 0x41, 3.3 V 0x42, spare 0x43, clean 0x44.

TPS26631 settings (RILIM, IOL): 24 V 4.99 kΩ 3.61 A; Jetson and third 6.04 kΩ 2.98 A; aux 4.53 kΩ 3.97 A; Pi stub 4.99 kΩ 3.61 A; 7.4 V 3.01 kΩ 5.98 A; spare 4.99 kΩ 3.61 A. UV and OV from each three-resistor chain: 20.2 / 27.4 V, 9.66 / 14.7 V, 5.04 / 9.3 V, 4.55 / 6.1 V.

**Deviations.** INA238 has no energy registers: PLAN 3.9's 2 % energy needs 100 Hz firmware integration. 1 % shunts use up the ±1 % budget: use 0.5 % or one-point calibration. TPS22965 has no current limit, and buck OCL 8.6 A exceeds its 6 A rating: polyfuse after each switch. It also cannot switch 7.4 V, so 7.4 V uses TPS26631. TPS2663 family UVLO defaults to 15.46 V, so every channel has a divider.

## supervisor_mcu

**STM32G0B1VET6 LQFP-100.** 68 signals against 60 I/O on LQFP-64. Pins checked 68 of 68 against ST data. ADC 12 channels at 20 kHz is 11 µs of a 50 µs scan. Only 3 I²C: I2C1 HP, I2C2 AFE, I2C3 to TCA9546A at 0x70 (A, B, Qwiic, spare); PLAN assumed 5.

Supplies: MCU on +3V3_AON, +3V3_HP through a ferrite, VREF +3V3_A through a ferrite. Air NTCs on ADC_SPARE1/2. ESTOP_B_STAT pin PC5 free.

**Open.** DS13560 itself not read (sibling DS13564 Rev 3 used). Sequencer rank limit, TC2030 pinout unverified.

## can_bridges_usb_hub

Two STM32G474 bridges, 3 channels each, on USB2514B-I/M2 (AEZG is not orderable). Hub ports: DN1 supervisor, DN2 bridge A, DN3 bridge B, DN4 service. CFG_SEL 00, NON_REM 11, no EEPROM. Reset is RC-released so the Pi keeps CAN if the supervisor dies. ABM8G 24 MHz 10 pF on all three; 18 pF parts fail G474 Gm margin.

**Deviations.** FDCAN1_RX lands on PB8-BOOT0 because USB takes PA11/12: option byte must ignore the pin. candleLight has no G474 support: budgetcan_fw, new board config and a 3-channel build test owed. Service USB-C is host side, so a laptop cannot reach the board: an upstream mux is needed if that is wanted. VBUS through TPS22965 + 0.5 A polyfuse (not TPS2051, unverified).

## can_phy (6 instances, was isolated_can)

Gate 3 replaced ISOW1044 with TCAN1462V-Q1 on RAW_5V (VIO 3.3 V). Bus pins ±58 V abs, ±45 V differential. 0.42 A for six in dominant, against 0.74 A before. Termination and TVS at the port connector on HP.

## io_fans_estop_loop

**E-stop.** +12 V, 680 Ω, NC contacts, opto A, opto B, 620 Ω, GND: 6.9 mA. Open loop, lost 12 V or lost 3V3 all give EN low. ISOM8710 to 74LVC1G14 to ESTOP_EN. SUP_OPEN 2N7002 only sinks, so firmware cannot close an open loop. Latency under 2 µs. Tether is a series contact with a shorting dummy.

**Deviation.** Not Cat 3: one series loop feeds both channels and a short across it defeats both. Dual-contact fix needs 2 more pins. Decided: stays Cat 0, with Cat 1's five parts per channel DNP (289 ms, 232 to 356 ms).

Fans 12 V, 25 kHz open drain. TS2 wake: POWER_BTN pulls TS2 to VSS through 10 kΩ + BAT54. Shell cable is 6-pin; 4 cannot carry RGB + button.

**Open.** No KiCad part for ISOM8710. Fan and Qwiic pinouts unverified. JST and Littelfuse PDFs were blocked.

## pi_power_stub

Remote TPS56637 5.1 V (R6 75 kΩ, 5.6 µH), fed from OUT_12V_PI_STUB. Output through Micro-Fit to USB-C pigtail: a 2×3 GPIO housing would put 6 A through one GND pin. PG left open. GPIO-fed `usb_max_current_enable` behaviour needs a bench test.

## pack_sense_board

Fuse at the cell, one per tap. 1 A tap fuse: 0.1455 Ω, 63 V, 50 A interrupt, 1206 (63 V parts are not 0603 or 0805). 50 A interrupt is below pack prospective short current: rely on wire impedance or find a higher breaking part, open. 14 fuses; pins 3, 4, 13 on B0_F. Presence: pin 13 tied to the B0 return.

## Follow-ups, 2026-11-06: decisions applied

Recorded in [EXPLAINER.md](EXPLAINER.md). Sheets changed: pack_input_shunt_afe, vmot_bulk_clamp, port_channel, front_end_12v_24v, rail_switches_meters, supervisor_mcu.

**External balance (closes the biggest gap).** SLUAA81A Fig 2-2 p4, N-channel, on pack_input_shunt_afe, 13 channels: Rbal from tap n to the drain, source on tap n-1, gate through Rg 1 kΩ to pin VC(n-1), 5.1 V zener gate to source. Internal FET current through the lower input resistor makes Vgs.

- Tap resistors: 20 Ω cannot work. Vgs at 3.9 V = 3.9 × 20 / (20 + 20 + 28) = 1.15 V. Changed to 100 Ω, the note's advised value and the datasheet p9 maximum. VC16 stays 20 Ω (SLUAA81A 2.5), or top cell reads low by 35 µA × R × cells: 2.8 mV at 20 Ω and 4 cells.
- Gate drive: 3.9 × 100 / (100 + 100 + RCB). RCB 15 / 28 / 46 Ω (datasheet 7.20): 1.82 / 1.71 / 1.59 V. Top cell is higher at 2.63 V.
- FET: SSM3K56FS, 20 V, ±8 V gate, 840 mΩ max at 1.5 V, so it is rated below the 1.59 V worst case. AO3420-class parts are rated only to 1.8 V: rejected. Loss 0.15² × 0.84 = 19 mW.
- Rbal 27 Ω 1 W 2512. External current with about 0.9 Ω in series (FET plus two tap fuses and wire): 140 mA at 3.9 V, 150 mA at 4.2 V. Internal path adds 17 to 19 mA, well under 100 mA abs (top cell 28 mA). Total about 157 to 169 mA, DC while switched on. Measurement pauses cut the average; CB_SLOW helps. Rbal 0.61 W at 4.2 V.
- Heat: Cell Balance Max Cells 4 gives about 2.5 W. Autonomous mode allows up to 7 non-adjacent cells, 4.3 W: raise only after a thermal check. Cell Balance Min Cell V 3900 mV.
- Accuracy: AFE stops balancing on the measured and adjacent cells, so the 1 A tap fuse drop (about 25 mV each) is not read. RC 100 Ω × 220 nF = 22 µs against a 3 ms window: no settling issue (SLUAA81A 2.5).
- Open: FET figures are from a datasheet excerpt, original PDF not read. SOT-416 land and pin order G, S, D to confirm against Toshiba. Zener leakage at 1.7 V unchecked.

**Arm, waist, aux trip (agreed with the arm trunk thread).** XT30 contacts and enclosed 16 AWG are treated as 15 A continuous, so the old 22 A limit no longer protected them. Software trip is the real overload limit. Aux shares the same sheet and connector, so it takes the same hardware, with a 10 A software default to keep PLAN's aux limit; its 7.5 A fuse becomes the shared 20 A. Leg numbers unchanged.

- Shunt: 3 mΩ WSLP39213L000FEA, not the requested 3.3 mΩ. WSLP3921 has 2, 2.5, 3, 4 mΩ. 3.3 mΩ would also give 48.5 / 3.3 = 14.7 A minimum, under the 15 A software limit, so a low part would current-limit at normal peak. 4 mΩ is 12.1 to 15.4 A: fails too.
- Limit: VCL 48.5 / 55 / 61.5 mV (LM5069 p5-6) gives 16.2 / 18.3 / 20.5 A. Breaker VCB 80 / 105 / 130 mV gives 26.7 / 35 / 43.3 A.
- RPWR: 1.30e5 × 3 mΩ × (200 − 1.18 mV × 58 V / 3 mΩ) = 69.1 kΩ, fitted 69.8 kΩ, 201.8 W. PLIM minimum 96.7 W is met.
- SOA: unchanged. 3.48 A at 58 V, 4.3 A at +24 %, against 5.2 A allowed at 9 ms and 70 °C. At the top of the limit, 20.5 A meets 248 W at 12 V: well inside the same 9 ms curve by extrapolation, not a printed line.
- Fuse: stays 20 A 58 V, backstop for a shorted FET only. Hardware maximum 20.5 A is 102.5 % of rating, so it is not strictly above tolerance, but the 997 needs 110 % for 100 h to matter and 135 % to open, and 20.5 A lasts at most 9 ms: 3.8 A²s against 380 A²s. Coordination holds. Blade fuse hot rerating is unverified; at 15 A it runs at 75 %.
- Dissipation: shunt 0.68 W at 15 A, 1.0 W at 18.3 A, 1.26 W at 20.5 A, rated 5 W. At 15 A: fuse 0.72 W cold, FET about 0.65 W, about 2 W port path.
- Software trip: INA241 gives 30 mV/A, so 1 A is 37 ADC counts. Firmware filter owed, adjustable 1 to 15 A, default 15 A. Between the software limit and 16.2 to 20.5 A only firmware protects the connector: a hung supervisor needs its watchdog to drop PORT_EN.
- PLAN's port table and hand note updated to match.

**Regen clamp sized for the 2 Ω leg stage.** 10 Ω fitted. 57.5 V / 2 Ω = 28.75 A, 1.65 kW; 30 A at 60 V.

| Part | Was | Now | Basis |
|---|---|---|---|
| FET | IPT015N10N5 | same | 29² × 1.5 mΩ × 1.7 hot = 2.1 W at 100 % duty. Edges about 100 ns inside the 10 µs SOA (about 300 A at 54 V, sheet 2). Stays in saturation, so no linear SOA question |
| Shunt | 20 mΩ, 17 W at 29 A: fails | 2 mΩ CSS4J-4026K-2L00F 6 W | 1.7 W, 58 mV. At 10 Ω: 11.5 mV, 66 mW. INA228 ADCRANGE 0, SHUNT_CAL 2000 at 40 A |
| Freewheel | MBRS3100T3G 3 A: fails | MBRB30H100CT-E3/81 | 100 V, 30 A total, 275 A surge. Carries lead inductance current for L/R, about 5 µs per 10 µH |
| Terminals, wiring | M4, 74651194R | same | M4 terminals are on the VMOT and CL_D paths. Pour and lead for 30 A owed in layout; REDCUBE rating unverified |

Energy basis: 1.7 kW for the time the drives keep regenerating with the BMS open. At 1 s that is 1.7 kJ into the external resistor and about 2 J in the FET, so the resistor sets the limit. The leg stage must pick 2 Ω by its pulse curve; no continuous rating near 1.7 kW is needed. Regen duration is an assumption until legs exist.

**Hold-up.** 20 × TDK B40950B0686M100, 68 µF 80 V hybrid, 10 × 10.8 mm, 33 mΩ, 10000 h at 105 °C. Fits the 13 mm board gap with 2.2 mm clear. Total 1.36 mF: 12.6 ms from 46.8 V, 6.8 ms from 37 V, at 90 W and 0.93. At -20 %: 10 ms and 5.5 ms. Rejected B40950A0686M000: 12.8 mm tall. Cost is about 21 cm² of land against 4 cans' 10 cm², for layout to fit. Dimensions from DigiKey listings; TDK datasheet not read. Footprint CP_Elec_10x10.5, listed land 10.3 × 10.3 mm matches its class.

**Load shed.** Reworked at gate 3, see Gate 3 fixes M16. Table on supervisor_mcu. Thresholds are starting values, firmware owed. No 12 V total meter exists: estimate is the sum of meters. Fans and the E-stop loop sit on +12V directly, so step 3 drops neither.

**Output limits.** TPS26631 per rail section above. Added after the TPS22965s: MF-MSMF250/16X-2 2.5 A on 5 V (1.8 A budget) and MF-MSMF110/16-2 1.1 A on 3.3 V (0.6 A budget), both ahead of the meter so the meter reads through them. Hold is at 23 °C: hot derating to check, 5 V margin is the thin one. Distributor data, Bourns datasheet not read. MF-MSMF200-2 rejected: last time buy, 8 V.

**Leg.** 1 mΩ shunt approved, RPWR 23.7 kΩ. Fuse format stays open: F on an ATO holder footprint, DNP, beside F501, position only. E-stop stays Cat 0.

**Footprints.** New project library `marengo.pretty`, loaded through `fp-lib-table`. TI lands from the datasheet drawings: TPS48100 DGX0019A (SLUSEW1C p35, no pad 16), TPS56637 RPA0010A (SLVSEG1A p29), ISOM8710 DFF0005A (SLLSFO4E p34, HV option, keeps the >5 mm rating). WSLP3921_Kelvin: Vishay current lands (doc 30176 p2), sense pads 2 and 3 sized by me as copper tabs with a net tie, position accuracy unmeasured. Vishay's Kelvin part is WSLP2726, unused. Wire_Pads_2_NTC: 1 mm drill, 103AT lead size unchecked. All parse and pass a DRC on a test board; none fabricated.

**C206.** Placeholder replaced by the same 100 µF 80 V hybrid as VMOT bulk.

## Gate 3 fixes (2026-11-06)

All 4 blockers and 18 majors from [GATE3_REVIEW.md](GATE3_REVIEW.md) are in. Part facts below come from manufacturer documents read this session, except where marked from memory.

### Three defaults, Joey can veto each

**Default 1, ports.** Keep the 15 A wiring. Add an independent hardware watchdog, TPS3431S (U2031). It starts itself at power-up: SET1 high, CWD open gives 1.6 s (1.36 to 1.84 s). No firmware can start or stop it. WDO and ENOUT on WD_N, which clears both E-stop latches. Latch A's Q̄ drives the OE̅ pins of SN74LVC244A U2032, which sits between PORT_REQ1-6 and PORT_EN1-6, so every port enable tri-states and its 100 kΩ on HP pulls it off. Both E-stop enables drop with the latches. Compute rails are untouched. Firmware kicks WD_KICK from the main loop only, after the port and pack checks ran. PE1 reads WD_N through 10 kΩ, so firmware cannot hold it up. WDO is a 200 ms pulse, not a latch: with a hung supervisor, a reset press re-arms for up to 1.84 s. A shorted port FET still ends at the blade fuse; that was accepted.

**Default 2, CAN.** Replace isolated transceivers with non-isolated TCAN1462V-Q1 (VIO on pin 5, SOIC-8, TCAN1462VDRQ1 listed in SLLSFF2B). Reason: the motors reference CAN to their power GND and the RS03 housings are at GND, so isolation bought little. Pins ±58 V to GND, ±45 V differential.

**Default 3, E-stop.** Stay Cat 0. Add a hardware re-close latch. Move +12 V off the E-stop connectors. A reset needs a press of a separate NO button after the loop is restored; a held button does nothing. A press is needed on every power-up too, because power-up clears the flops: veto this if you want power-on arming.

### Correction: the isolation claim

The log said isolation "separates logic from motor return". That was false. R503 bonded GND2 to GND on the same board, and the drives and frame bond CAN reference to power GND. Nothing on this board is isolated now. CAN_REF goes to GND through a 60 V 0.14 A PTC (MF-MSMF014/60X), so a lost motor return cannot pull drive current through the signal wire.

### CAN short at 54.6 V

Termination is split at the port connector: 56 Ω + 56 Ω PWR263S-35-56R0F, TERM_M 4.7 nF to GND, MF-MSMF010/60X (0.70 to 15 Ω), jumper to CANL. Total 113 to 127 Ω.

| Fault | Result |
|---|---|
| H or L alone to 54.6 V | The other line follows through the termination. Dominant current about 115 mA (specified only to 40 V) gives about 14 V across the termination, inside ±45 V. Pins inside ±58 V unless V_PORT is above 58 V (regen, mini clamp up to 59.4 V: double fault, accepted) |
| H at 54.6 V, L hard to GND | 0.43 to 0.48 A, 24 to 26.4 W, up to 13.2 W each. The PTC's guarantee is 0.5 A in 1.5 s at 23 °C, and this is below that point, so clearing time is not guaranteed: about 2 s expected, bench test owed. Part rating is 35 W at 25 °C case with tab on GND copper, 3.5 W free air. Differential exceeds ±45 V, so the transceiver may need replacing |

The old 0402 took 24.8 W at the same fault. The SMAJ58CA stays off at 54.6 V and is for ESD only.

### Blockers

| # | Fix |
|---|---|
| B1 | U302 EN and INP2 on ESTOP_EN_B, INP1 on BANK_B_CLOSE with 100 kΩ down. Text and check.py corrected |
| B2 | D220 V10PM15-M3/I, 3-pin symbol, TO-277A. Vishay doc 87432: pads 1, 2 anode, tab cathode. Reversed charger 109 V is under 150 V. TVS surge plus reversal is a double fault |
| B3 | Custom PWR263S symbol: terminals on pads 1, 3, tab pad 2 no-connect. Bourns PWR263S-35: "electrically isolated from the backplate", 5.08 mm pitch. R209, precharge, mini clamp, termination. Pulse chart about 10 J at 100 ms against 2.68 J and 2.2 J. 35 W at 25 °C case; 3.5 W at ambient on a small FR4 test board, so copper matters: tab pad on GND (isolated part) |
| B4 | F2201 to F2214: Schurter 3-148-693 USE 2410, 1 A, 200 A at 86 VDC (τ under 0.05 ms), 79 mΩ, against 146 A prospective. Balance error 24 mV at 150 mA. Footprint marengo:Fuse_2410 from Schurter data sheet 21.07.2026 p.2: 2.50 x 2.80 mm pads, 3.00 mm gap |

### Majors

| # | Fix |
|---|---|
| M1 | U1104 and its input caps on VSYS_OR, so 3V3_AON rides the hold-up bank |
| M2 | BUCK12_OFF 100 kΩ to GND: default on, a hung supervisor leaves 12 V up. BUCK24_OFF stays pulled up |
| M3 | EN_INV pulled from VMOT through 1 MΩ with a 12 V zener. Q3 sinks it through a new BSS123 gated by VCLAMP12, 1 MΩ down. Losing VCLAMP12 or PORT_EN leaves Q2 on and the port off |
| M4 | Default 1 |
| M5 | R209 PWR263S, tab on GND. Load 1.46 mF, 2.2 J, tau 0.146 s, peak 29.8 W. OTP Predischarge Timeout 100 (1.0 s), Stop Delta 2500 mV. Pass 2 reversed Timeout 0: Stop Delta needs under 25 mA, so a loaded VSYS would never exit. Firmware holds BUCK12_OFF high at boot until DSG is on. A VSYS short: about 30 J in R209, then SCD. The pulse chart stops at 0.1 s, so bench it |
| M6 | DFETOFF 0x42 |
| M7 | J2.4 is B0, a Kelvin tab net, 10 kΩ to BAT-. J2.3 is VSS (NTC and presence return). Cell 1 balance source B0. VC0 below VSS by the return drop, about 90 mV at 60 A with 1.5 mΩ assumed, inside the limit. D15 B140 (Vf from memory) clamps VC0 near -0.3 V in a short (-0.3 V at 200 A, R14 limits current). Deviation from datasheet p70, which ties VC0 to VSS. Cell 1 Interconnect 0 |
| M8 | J2201.3 NTC_RET, J2201.13 PRES, J2202 1 × 16 with pins 1, 2 looped in the tap plug, beside B0 and away from B13. Either plug out opens DSG and CHG |
| M9 | Q211 MMBT2907A local turn-off, 220 Ω in series with the base, 10 kΩ base-emitter. Diodes Inc gives 600 mA, 800 mA peak, 200 mA peak base. 844 nC max in about 2 µs. FET sharing to scope |
| M10 | OCD1 24 mV, OCC 12 mV, table above. V(OC_ACC) -2/+2.65 mV under 20 mV, ±4 mV 20 to 56 mV, ±5 mV 56 to 100 mV |
| M11 | U301 uses SCP: CS on RS201 (Kelvin, high side), ISCP 10.5 kΩ = 31.6 to 49.5 mV = 105 to 165 A on 0.3 mΩ. That sits above OCD1 max 93 A and below SCD min 170 A. TMR 100 kΩ + 680 pF, about 10 µs, latch-off. Bank A opens, compute stays up. Reset by E-stop cycle or a SUP_OPEN_A pulse, so firmware bounds retries. FLT_A to MCU PE15, read with ESTOP_ARMED because FLT also asserts on UVLO. Bank B closes at 0.5 V or less to stay under the trip: 0.84 s, 1.01 s at +20 %, firmware timeout 1.2 s, which opens the loop |
| M12 | Default 3. Loop GH4s are signal, GND, signal, GND, so pin-to-pin shorts land on GND and open the loop. Wireless receiver power has its own GH2 with a 0.5 A polyfuse. 10 kΩ + 100 nF on CLR rides through contact chatter, adding about 1.4 ms to a stop. Residuals: a cable short across one contact pair still defeats the series loop, and so does WL_12V shorted to LOOP_TH at the receiver: keep its power and contact wires in separate cables. One series loop feeds both channels. Not Cat 3 |
| M13 | NC thermostat on the brake plate, in the E-stop loop (off-board, part and placement owed from the frame thread, about 100 °C class). Stuck-on 322 W into the HS25 opens the banks in hardware |
| M14 | Clamp top 604 kΩ, window below |
| M15 | 3 × 100 µF 80 V hybrid PACK_TOP to GND. Surge table below |
| M16 | Firmware must configure U1321's alert limit at boot; its default never trips. Load-step droop against 12.5 A is a bench check. U1321 INA238 on a 2 mΩ shunt between L12_OUT and +12V. ALERT latched at 12.0 A, about 150 µs conversion, U1322 and three 2N7002 sink EN_12V_THIRD, AUX, SPARE through the supervisor's 1 kΩ series resistors. Firmware steps: fans above 10.5 A, third board above 11.0 A, 500 ms. Pi stub and Jetson only above 12.4 A for 1 s after hardware shed, and after telling the host. PG_12V falling alone logs only |
| M17 | Default 2 |
| M18 | Passes, no change, below |

Brake resistor is now Arcol HS25 10 Ω 25 W on all sheets (frame main 04983bb). 56.77² / 10 = 322 W while on.

### VMOT window (M14)

| Level | Value |
|---|---|
| Pack full | 54.6 V |
| Clamp off, min | 55.1 V |
| Clamp on, nominal | 56.77 V (off 55.77 V) |
| Clamp on, max | 57.45 V, under the 58 V fuse rating |
| OVLO, min | 57.9 V (nominal 60.3 V, max 62.7 V) |
| Mini clamp on | 59.42 V, off 58.43 V |
| Drive OV fault | 60 V |

### Stack surge (M15)

Pass 2 replaced the energy estimate: the pack stays connected behind the inductance after the FETs open, so a stored-energy sum understates the peak. Limits: caps 80 V, INA228 85 V, FET 100 V, TPS7A4001 100 V, LM5069 100 V, LM5145 105 V abs max, U1 CHG VSS + 85 V.

Time-step model: pack EMF 54.6 V, 2 µH, 90 mΩ (PLAN pack resistance 90 to 110 mΩ, which is also what limits the fault to 600 A), I0 600 A, C from 54.6 V, ESR 6 mΩ, D206 from 64.4 V at 0.91 Ω.

| C | 90 mΩ | 110 mΩ, 496 A |
|---|---|---|
| 4 × 100 µF at -20 % | 77.5 V | 71.8 V |
| 4 × 100 µF nominal | 74.0 V | 69.1 V |

At 3 µH and 320 µF it is 85 V, over the caps and at INA228's limit: measure the inductance. D206 conducts in this event. CHG_G sits about 11 V above the stack through 5.1 kΩ, so U1's CHG limit is close. Leakage up to 320 µA in ship mode before ageing. Plug-in inrush into 400 µF at the XT90 is ESR-limited: check the connector on the bench. The pack's Class T fuse covers a shorted part on PACK_TOP.

### M18, start-up SOA

Checked against Infineon IPB017N10N5LF Rev 2.3, Diagram 3 (Tc 25 °C), read from the curves. C = 3.6 mF, 5.37 J. Corrected in pass 2: Tj uses the 0.4 K/W maximum.

| Case | Peak | Against SOA |
|---|---|---|
| T = 117 ms | 1.68 A, 92 W at 54.6 V | DC line about 4 A at 54 V, 2.06 A at Tc 85 °C. 1.2× margin. Tj rise 92 W × 0.4 K/W = 37 K |
| T = 257 ms | 0.76 A, 42 W | Passes |
| Fault limit 200 W | 3.66 A for up to 9 ms | Datasheet 10.2 A at 56 V, 10 ms, about 5.3 A at 85 °C. Passes |

No timer or dV/dt change at 3.6 mF. Inrush is C × dV/dt, so more drive capacitance raises the current: at T = 117 ms the hot DC limit of 2.06 A is reached at 4.4 mF. 3.6 mF is therefore a ceiling for this setting, and the real drive capacitance needs measuring.

### Minors picked up

C14 1 µF 100 V 1206. D1 BAV19W. U401 thermal pad on pad 9. U201 address text 0x4C. INA228 range ±136.5 A. Mini clamp text 59.42 V. RS1 6.75 W at 150 A, brief only. SSM3K56FS text: 1.5 V is rated at 50 mA only, so Rds at 150 mA and 2.35 V is a bench check. J101/J102: GND is now about 0.55 A per conductor; VSYS 1.1 A per pin needs its Milli-Grid rating checked.

### Tools

check.py now audits every footprint with 3 or more pads, pad against symbol pin, both ways (247 parts). Run on the pre-fix netlist it fails 10 times, on D220, all the PWR263S parts and U401. On this netlist it passes.
