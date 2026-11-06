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
Gate-source 1 MΩ (p45: driver sinks 30 µA). Vgs on = −7.5 to −9.7 V (p12). Peak in the 100 Ω: 298 W at 54.6 V. Energy: ½ × 1.1 mF ×
54.6² = 1.64 J per start, τ 0.11 s, 5τ = 0.55 s. **OTP:** enable PREDISCHARGE, timeout > 0.6 s.

**Naming, deviation 4.** The brief's "PACK+" means the stack; sheet 1's hierarchical PACK+ meant the post-FET port. Renamed sheet 1's
to VSYS so PACK_TOP = stack and VSYS = common port, with no PACK+ net left. PACK and LD sense VSYS through 5.1 k + 5.1 k (R18 to R21).
The 40 mV shunt offset between DSG_OUT and VSYS is negligible for LD. Pack + terminal J1 sits on PACK_TOP (BAT_P in the coordinator's naming), ahead of both banks, confirmed from the netlist: J1.1 shares a net with the CHG FET sources, and VSYS is the separate load-side net. The rename touched only the PACK/LD sense label; nothing bypasses the FETs.

**CP1, sheet 1 change.** 470 nF → 1 µF. p65: if gate load is large, raise CP1 (400 nF load → 2.2 µF, 11 V dips to 9 V). Ours: 8 × 211 nC
= 1.69 µC max, droop 1.7 V, start-up about 130 ms (60 ms at 470 nF, scaled).

**Charge input, deviation 5.** PLAN 3.7 said reverse-polarity protected; the brief said FET. Common port is the normal BQ76952 layout
(charger reaches the PACK pin, p44 to p45) and the charger wakes the AFE through LD. But with a reversed charger and a live pack the
blocker sees 54.6 + 54 ≈ 108 V: over the 100 V FET and over LM74700 CATHODE-ANODE 75 V abs max (SNOSD17G section 6.1). Rejected: LM74700 +
FET; P-FET with gate to GND (it conducts VSYS back onto the port). Chosen: D220, 200 V 15 A Schottky (V15PM20 class, verify), 0.7 V drop,
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
| OCD1 | 20 mV | 66.7 A | hardware comparator, up to 425 ms |
| OCD3 | current based | 60 A for 1 s | the agreed 60 A ceiling |
| OCC | 14 mV | 46.7 A | 425 ms |
| SCD | 60 mV | 200 A (170 to 230 A at ±15 %) | 15 to 450 µs in 15 µs steps |

SCD was 40 mV on paper. On 0.3 mΩ that would trip at 133 A and break the 150 A, 100 ms path rating, so SCD went up to 60 mV.

**Open.** External balance FETs (PLAN 3.1, 150 mA per cell) are not captured on this sheet: rev A wip has only the AFE's internal
balancing. They change the cell RC network, so revisit the 20 Ω. CSS4J
land on the BVR4026 library footprint is to be checked against the Bourns drawing. OTP owed: DFETOFF active high (0x02), CFETOFF and
HDQ as thermistor inputs.

## HP_power_board top sheet

**Board-to-board, deviation.** No stock 2×30 2 mm latched header, so two 2×15 ribbons (J101 power and logic, J102 CAN and analog),
all 60 pins used. Each signal keeps one global net across both boards. `check.py` asserts every HP and LP pin pair against
`CONN_A` and `CONN_B`.

Changes from README section 5: BOARD_PRESENT dropped (NTC_BOARD1 reads open when the HP board is missing; PF10 goes to PRECHG_EN),
FLT lines not routed. Added TS2_WAKE, PRECHG_EN, GND2 × 6, +3V3_HP, NTC_BOARD1, NTC_BRAKE, MID_SENSE. VSYS gets 6 pins because LP
draws up to 6.7 A at minimum pack, not README's 5 A.

Port order everywhere: 1 L arm, 2 R arm, 3 waist, 4 L leg, 5 R leg, 6 aux. CAN numbers match. J103 is CAN fault injection on CAN6.

**DFETOFF, deviation.** PLAN drives DFETOFF from the E-stop loop. That deadlocks: 12 V comes from VSYS after DSG, so the loop reads
open before 12 V exists; it would also drop compute on E-stop, against PLAN 3.1 and 3.2. Used instead: PACK_PRESENT_N (1 MΩ from
PACK_TOP, 3.3 V zener to VSS, pulled low by the pack sense harness when mated, 54 µA) diode-ORed by BAT54C with AFE_DFETOFF_SUP
into DFETOFF, 100 kΩ to VSS. Active high, config 0x02. The README's hardware loop link was not fitted: no pin left for it.

**Addresses.** Port INA228 straps A1,A0 give 0x40, 0x41, 0x44, 0x43, 0x47, 0x45 for ports 1 to 6. VSYS meter 0x4C, clamp 0x4D.

## estop_banks

**TPS48100-Q1 × 2, simple driver mode** (SLVSGC0 Fig 7-11 p21). No firmware in the path. Bank A VSYS to MID, bank B MID to VMOT,
3 × IPT015N10N5 each. Drains face VSYS so body diodes pass VMOT to VSYS: regen reaches the pack with banks open (PLAN 3.4).
VS, CS+, CS- at VSYS through 100 Ω; SCP_TEST, ISCP, CS_SEL, TMR grounded; CBST 1 µF; EN/UVLO and INP1 both on ESTOP_EN_x. Turn off 1.2 µs by
INP1, 6.8 µs by EN. Turn on 27 to 50 ms. Part is 100 V abs, 95 V operate (docs said 80 V). Gate 2.2 Ω per FET, 1 MΩ gate-source.

**VMOT precharge, deviation (reverses PLAN 3.4).** IPT015N10N5 SOA is 0.7 A at 54 V, 10 ms, so closing a bank onto 1.8 mF is out.
Q307 and 100 Ω PWR263S-35-1000F across bank B, gated by U302 G2 from PRECHG_EN. Close A, raise PRECHG_EN, wait for VMOT within 2 V
of VSYS, close B, timeout 0.5 s. EN low kills G2, so the loop gates precharge in hardware.

- Peak 54.6² / 100 = 29.8 W (the brief's 298 W was a tenfold slip). PWR263S-35-1000F passes; W21 3 W does not.
- Energy ½ × 1.8 mF × 54.6² = 2.68 J. τ = 100 Ω × 1.8 mF = 180 ms.

**Status, deviation.** TPS48100 has no VDS output. MID and VMOT dividers 1 MΩ / 56 kΩ (3.3 V = 62 V), 49.9 Ω to ADC, plus VSYS on LP. Bank A
short reads MID near VSYS with both open and VMOT low. FLT to test point only.

**Links.** ESTOP_EN_A/B and PRECHG_EN from LP over J101; MID_SENSE, VMOT_SENSE over J102; VMOT to vmot_bulk_clamp and every port.

**Open.** No KiCad footprint for TPS48100 DGX. Check U302 G2 drive against Q307 Qg.

## vmot_bulk_clamp

**Bulk, deviation.** 220 µF 80 V 10 × 12.5 does not exist; 80 V hybrids top out at 100 µF in 10 × 16.5. Fitted 18 × HHXF800GRA101MJH0G, 1.8 mF.
Rejected 10 × 63 V 180 µF: the caps would sit below the SMDJ58A's 64.4 V conduction point, so the TVS would not protect them. Footprint is CP_Elec_10x14.3 placeholder, land owed.

**Clamp, analog, self-powered from VMOT.** TPS7A4001 12.08 V (464 kΩ / 49.9 kΩ), LM4040A 2.5 V reference, TLV1805 comparator, UCC27517 driver,
IPT015N10N5, 20 mΩ shunt, MBRS3100T3G freewheel, INA228 0x4D. Divider 499 kΩ + 619 kΩ over 51.1 kΩ, hysteresis 19.1 MΩ. The series pair is there
because E96 pairs step 0.7 V.

- On 57.50 V, off 56.50 V (VOH 11.7 V). Spread 56.82 to 58.19 V full temperature, 56.99 to 58.02 V at 0 to 60 °C.
- Works with the supervisor dead. Budget 5 mA, 0.23 W in the LDO. Stuck on is 330 W, caught by resistor NTC and opening the E-stop.

**Regen, decision needed.** 10 Ω passes 5.75 A against a 30 A host regen limit. With the BMS open mid-brake the excess 24 A raises VMOT at 13.5 V/ms
on 1.8 mF: 2.5 V headroom lasts 185 µs, then drives trip on OV at 60 V. Kept 10 Ω and accepted the drive OV stop for now. That stop
drops torque on the braking limb, so it is not free. Alternative is about 2 Ω, 30 A, 1.7 kW peak. Joey to decide.

SMDJ58A stays as spike protection: it does not conduct under 64.4 V. Its 93.6 V clamp exceeds INA228's 85 V, so sense lines take 10 Ω on IN± and 100 Ω on VBUS.

**Open.** No datasheet in hand for SMDJ58A, BAV19W.

## port_channel (6 instances, legs DNP)

**LM5069-1 latched hot swap.** Per channel: 1 × IPB017N10N5LF (legs 2 ×), shunt, LM5069, INA228, INA241, fuse, load-side mini clamp.

| Value | 22 A channel | Calculation or source |
|---|---|---|
| Rs | 2.5 mΩ WSLP3921 | ILIM window 19.4 to 24.6 A, breaker 32 to 52 A |
| RPWR | 56.2 kΩ | 200 W limit |
| CT | 100 nF | 4.7 ms typ, 2.8 to 9.0 ms |
| Cdv/dt | 47 nF C0G | Cdv ≥ Cload × 12.9e-6, covers 3.6 mF at 1.7 A |
| UVLO | 191 kΩ / 18.7 kΩ | on 32.0 V, off 28.0 V |
| OVLO | 95.3 kΩ / 4.12 kΩ | off 60.3 V (57.9 to 62.7 V) |

**FET, deviation.** IPT015N10N5 has 0.8 A SOA at 50 V 10 ms; the docs' 10 A is wrong for it. IPB017N10N5LF (Linear FET, D2PAK-7) has 12 A.
Timer check at 9 ms, 58 V: 10.6 A × 0.64 (70 °C) / 1.3 = 5.2 A against 4.3 A worst case limit.

**Shunts, deviation.** WSLP3921 is 2-terminal, so Kelvin sense must come from the land; RS501 footprint is blank until that land exists. No 1.2 mΩ
part: legs use 1 mΩ (48.5 to 61.5 A, RPWR 23.7 kΩ) or 1.5 mΩ (32 to 41 A). Recommendation 1 mΩ.

**Fuse, deviation.** 20 A 0997020.WXN, not 15 A: a 15 A fuse opens at 20 A in 0.75 to 600 s, so it overlaps the 19.4 to 24.6 A trip. Fuse adds 1.47 W per
channel; the docs' 1.5 mΩ path ignored it. 997 MINI stops at 30 A: leg fuse needs another format, PN open.

**Telemetry, deviation.** INA228 cannot give 1 kHz on six devices (one register 0.85 ms at 400 kHz). 1 kHz current comes from INA241A1 (1.65 V + 10 × Vsense, ±58 A at 2.5 mΩ, sees regen) into
the supervisor ADC; INA228 at 100 Hz for V, P, E. ADCRANGE 0 (55 mV exceeds 40.96 mV). SHUNT_CAL 3750 at 60 A FS.

**Enable.** PORT_EN (100 kΩ down) drives Q503, which holds Q504 off. Supervisor dead: Q504 on, UVLO low, port off. Latch reset: drop PORT_EN, 148 ms, raise.

**Mini clamp.** TL431 + BSS123 cascode + DMP10H400SE + 150 Ω PWR263S-35-1500F, load side of the fuse, so a blown fuse with a backdriven arm is covered. On 59.19 V, off 58.20 V,
0.39 A, 23 W. On spread 58.33 to 59.73 V touches main clamp top only at extremes.

**CAN TVS, deviation.** NUP2105L VBR is 26 to 32 V: sustained 54 V kills it. SMAJ58CA per line. Clamp exceeds ISOW1044's 58 V and capacitance is unchecked. Scope the bus.

**GND2.** Bus reference ties to port return via 0 Ω. Isolation then separates logic from motor return; it does not block a CAN-to-VBUS fault.

**Deviations minor.** PGD unused (INA228 reads the state). Per-port Z1 omitted, 2 shared VMOT TVS cover it. REDCUBE press-fit M4 not found: THR 74651194R, rating unverified.

**Open.** Drive input capacitance unknown. MMBTA92 for dV/dt PNP unverified. 150 Ω 15 J pulse is off the graph: confirm with Bourns or use W22-150RJI. DMP10H400SE pin numbers inferred. Body 23 W needs thermal check on long regen.

## front_end_12v_24v

**Deviations.** Hold-up: README's "1 mF rides 10 ms at 90 W down to 20 V" gives only 5.0 ms from 37 V. 4 × EEV-FK1K331M, 1.32 mF:
½ × 1.32 mF × (46.8² − 20²) × 0.93 / 90 W = 12 ms; 6.6 ms from 37 V. These are 16 × 17 mm and break the 13 mm stack: board-area or height decision owed.
24 V is tapped upstream of the ideal diode, or 75 W more drains the bank. LM5050-1 replaces LM74700 (LM74700 is 65 V abs against PLAN's 75 V survive; LM5050-1 is 75 V operate, 100 V abs, but has no reverse-polarity function).

| Block | Values |
|---|---|
| LM5145 12 V 12.5 A | 300 kHz master, 6.8 µH XAL1510-682, CSD19534Q5A + BSC040N10NS5, RFB 33.2 kΩ / 2.37 kΩ, RILIM 348 Ω, PM 52° min, UVLO 30.3 / 18.2 V |
| LM5145 24 V 3.1 A | 300 kHz slave 180°, 33 µH, CSD19534Q5A × 2, RFB 80.6 kΩ / 2.80 kΩ = 23.83 V, RILIM 301 Ω, PM 62° min, UVLO 34.1 / 29.1 V |
| LMR38010 3.3 V AON | 400 kHz, 22 µH, 100 kΩ / 43.2 kΩ = 3.31 V |

**12 V budget, open.** Sum of maxima about 22 A on a 12.5 A converter (DC limit 14.2 A min). Relies on non-coincident load; PLAN needs an agreed budget or shedding.

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

**Deviations.** INA238 has no energy registers: PLAN 3.9's 2 % energy needs 100 Hz firmware integration. 1 % shunts use up the ±1 % budget: use 0.5 % or one-point calibration. TPS22965 has no current limit, and buck OCL 8.6 A exceeds its 6 A rating: polyfuse owed on 5 V and 3.3 V. It also cannot switch 7.4 V, so 7.4 V uses TPS26631. TPS2663 family UVLO defaults to 15.46 V, so every channel has a divider.

## supervisor_mcu

**STM32G0B1VET6 LQFP-100.** 68 signals against 60 I/O on LQFP-64. Pins checked 68 of 68 against ST data. ADC 12 channels at 20 kHz is 11 µs of a 50 µs scan. Only 3 I²C: I2C1 HP, I2C2 AFE, I2C3 to TCA9546A at 0x70 (A, B, Qwiic, spare); PLAN assumed 5.

Supplies: MCU on +3V3_AON, +3V3_HP through a ferrite, VREF +3V3_A through a ferrite. Air NTCs on ADC_SPARE1/2. ESTOP_B_STAT pin PC5 free.

**Open.** DS13560 itself not read (sibling DS13564 Rev 3 used). Sequencer rank limit, TC2030 pinout unverified.

## can_bridges_usb_hub

Two STM32G474 bridges, 3 channels each, on USB2514B-I/M2 (AEZG is not orderable). Hub ports: DN1 supervisor, DN2 bridge A, DN3 bridge B, DN4 service. CFG_SEL 00, NON_REM 11, no EEPROM. Reset is RC-released so the Pi keeps CAN if the supervisor dies. ABM8G 24 MHz 10 pF on all three; 18 pF parts fail G474 Gm margin.

**Deviations.** FDCAN1_RX lands on PB8-BOOT0 because USB takes PA11/12: option byte must ignore the pin. candleLight has no G474 support: budgetcan_fw, new board config and a 3-channel build test owed. Service USB-C is host side, so a laptop cannot reach the board: an upstream mux is needed if that is wanted. VBUS through TPS22965 + 0.5 A polyfuse (not TPS2051, unverified).

## isolated_can (6 instances)

ISOW1044 on RAW_5V: VDD must be 5 V, 6 ports draw up to 0.74 A. Bus pins ±58 V abs, ±45 V differential, so TVS is on the port. 120 Ω termination by jumper per port.

## io_fans_estop_loop

**E-stop.** +12 V, 680 Ω, NC contacts, opto A, opto B, 620 Ω, GND: 6.9 mA. Open loop, lost 12 V or lost 3V3 all give EN low. ISOM8710 to 74LVC1G14 to ESTOP_EN. SUP_OPEN 2N7002 only sinks, so firmware cannot close an open loop. Latency under 2 µs. Tether is a series contact with a shorting dummy.

**Deviation.** Not Cat 3: one series loop feeds both channels and a short across it defeats both. Dual-contact fix needs 2 more pins. Cat 1 costs five parts per channel, fitted DNP (289 ms, 232 to 356 ms).

Fans 12 V, 25 kHz open drain. TS2 wake: POWER_BTN pulls TS2 to VSS through 10 kΩ + BAT54. Shell cable is 6-pin; 4 cannot carry RGB + button.

**Open.** No KiCad part for ISOM8710. Fan and Qwiic pinouts unverified. JST and Littelfuse PDFs were blocked.

## pi_power_stub

Remote TPS56637 5.1 V (R6 75 kΩ, 5.6 µH), fed from OUT_12V_PI_STUB. Output through Micro-Fit to USB-C pigtail: a 2×3 GPIO housing would put 6 A through one GND pin. PG left open. GPIO-fed `usb_max_current_enable` behaviour needs a bench test.

## pack_sense_board

Fuse at the cell, one per tap. 1 A tap fuse: 0.1455 Ω, 63 V, 50 A interrupt, 1206 (63 V parts are not 0603 or 0805). 50 A interrupt is below pack prospective short current: rely on wire impedance or find a higher breaking part, open. 14 fuses; pins 3, 4, 13 on B0_F. Presence: pin 13 tied to the B0 return.
