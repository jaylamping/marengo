# Rev B4 PDB: component choice review (small-signal, logic, regulators, passives, cost)

Reviewer lens: component selection only (high-current parts are out of scope). Repo state: main as checked out 2026-11-07, read-only.

**Method.** I rebuilt the netlist by running `gen/gen.py` with its `nic()` and `Grid.two()` calls instrumented (KiCad isn't installed here). Symbols came from the `lib_symbols` embedded in `marengo_pdu_revb.kicad_sch`. Output went to the scratchpad and the repo is unchanged (`git status` is clean). The result has 204 entries, and refs and values match `bom-rev-B.csv`. Datasheet values come from TI datasheets fetched over the web, the repo's `gen/parts/*.json` abs-max extracts, and the distributor and datasheet pages cited in each finding. Net voltages used:
- BAT, SH_P, SW_D, ABS, BRK_IN: up to 91 V transient (absorber_sim worst corner).
- MID: up to 91 V transient (B opens first), 54.6 V steady.
- VBUS: up to 58 V or so.
- VCTL: 12.1 V, 19.2 V with a shorted U2.
- +12V: up to the SMBJ15A clamp.

Severity scale: **Major** (fix before layout), **Medium** (fix before ordering or assembly), **Minor**, **Nit**.

---

## Findings

### 1. Major: U18 EN/UVLO is driven past its 7 V absolute maximum during servo back-feed

- **Evidence.**
  - SPON_EN = +12V × 68k/(100k+68k) = 0.405 × V(+12V), through R41 100k and R42 68k.
  - TPS2595x datasheet (https://www.ti.com/lit/ds/symlink/tps2595.pdf), §7.1: EN/UVLO abs max is −0.3 to **7 V**. The recommended VEN is 0 to **6 V**. IN abs max is **20 V**.
  - EN reaches 6 V at +12V = 14.8 V and 7 V at +12V = 17.3 V.
  - D20 is the only bound on +12V during back-feed. SMBJ15A VBR is **16.7 V min, 18.5 V max**, and VC is 24.4 V max at Ipp (Littelfuse SMBJ15A; element14 page https://my.element14.com/littelfuse/smbj15a/diode-tvs-15v-600w-5-uni-smb/dp/1827610).
  - So a D20 at the top of its tolerance lets EN reach about **7.5 V before D20 even conducts**. Bring-up step 7 accepts +12V up to 16 V, where EN is 6.5 V, already outside the recommended range.
  - Side effect: U18's TPS259541 output clamp is 13.3/13.7/14.3 V. Any back-feed above that makes U18 clamp SPARE_ON and dissipate (V(+12V) − 13.55 V) × I(head) in a 2 × 2 mm WSON. Expect thermal cycling or brownout of the head electronics during hard back-drive.
  - U9's EN is fed from RUN (about 7.7 V × 0.405 = 3.1 V), so it isn't affected.
- **Fix.**
  - R42 68k → 33k. EN at 12 V is then 2.98 V, and at 20 V it is 4.96 V, inside 6 V.
  - The UVLO trip moves to 1.13–1.27 V × 133/33 = 4.6–5.1 V rising, still far under 12 V. TI EN thresholds: rising 1.13/1.2/1.27 V, falling 1.03/1.1/1.17 V.
  - Alternative: a BZT52C5V1 from SPON_EN to GND. That's one more part.
  - Also note in bring-up step 7 that U18 clamps SPARE_ON above about 13.3 V.
- **Confidence:** High (datasheet numbers plus arithmetic).

### 2. Medium: the assembly method is undefined, and the package mix assumes reflow

- **Evidence.**
  - PRE_LAYOUT_AUDIT.md cost section says: "Bare boards only: no assembly, and the THR terminals are hand-soldered." PLAN.md line 457 says "PCB and SMT assembly at JLCPCB or PCBWay".
  - Fitted parts that are hard or impossible with an iron:
    - U9 and U18 TPS259541 **DSG WSON-8, 2 × 2 mm, 0.5 mm pitch, no leads, exposed pad that "must be connected to GND"** (TPS2595 pin table).
    - U2 TPS7A4001 DGN with a PowerPAD.
    - Q6 TDSON-8.
    - 50 fitted 0402 parts: 37 R and 13 C.
    - 5 D2PAK-7 and 13 Würth REDCUBE **THR** (through-hole *reflow*) terminals on a 4-layer, 2 oz board with 20 cm² pours and 30 thermal vias per tab. An iron can't bring those joints up to temperature without a preheater.
  - Leaded fine-pitch parts are hand-solderable by drag soldering: 9 × INA228 VSSOP-10 0.5 mm, U8 VSSOP-19, 2 × LM5069 MSOP-10.
- **Fix: pick one route and write it in the audit.**
  - (a) Stencil plus hot plate or reflow for the whole board. Order the stencil with the JLC/PCBWay boards for about $10.
  - (b) Fab-assembled SMT, consigning TPS48100-Q1 and the APZ cans if they aren't in the assembler's library. Hand-solder only the Micro-Fit, GH and fuse holder.
  - If the build stays fully manual, change 0402 to 0603 as the default. That's about +75 mm² against roughly 8 cm² free, and it also lifts the 50 V working-voltage ceiling (finding 7).
- **Confidence:** High.

### 3. Medium: the D18 order code doesn't exist ("BZT52C68-7-F")

- **Evidence.**
  - The audit's diode line orders "BZT52C68-7-F", a Diodes Inc code.
  - Diodes Inc's BZT52C series ends at **39 V**: datasheet title "BZT52C2V0 – BZT52C39" (https://www.radiolocman.com/datasheet/data.html?di=114565).
  - 68 V SOD-123-class parts that do exist: Nexperia BZT52H-C68 (SOD123F), Panjit BZT52-C68, Diotec BZT52C68.
  - Nexperia BZT52H series datasheet (https://www.farnell.com/datasheets/2331103.pdf), BZT52H-C68: **VZ 64.0–72.0 V at 2 mA**, IR ≤ 0.05 µA at 47.6 V, Ptot 375 mW.
  - With that part, U17's VBUS pin sees at most about 72 V, against the INA228's 85 V abs max. At MID 91 V the zener carries about 0.6 mA through R47.
- **Fix.** Order Nexperia BZT52H-C68,115, which lands on the SOD-123 footprint. Update the audit line.
- **Confidence:** High that -7-F doesn't exist in Diodes' C68. Medium that no other vendor uses that exact suffix.

### 4. Medium: unprotected MLCCs directly across BAT: flex-crack short ahead of every protection

- **Evidence.**
  - Netlist: C1 (1 µF 100 V 1210, BAT–GND), C2 (100 nF 100 V 0805, SH_P–GND) and C20 (100 nF 100 V 0805, SH9_P = BAT–GND) sit upstream of RS1, the banks, U7 and F1.
  - Their only protection is the pack's 60 A Class T fuse and the BMS DOC (80 A).
  - A flex-cracked MLCC usually fails as a resistive short of a few amps. That never trips either one, and it heats the board.
  - These parts sit beside the M4 J1/J2 terminals, which are torqued.
  - C27 is behind R29 (100 Ω) and C39 is behind F1, so those two are protected.
- **Fix.** Use soft-termination or open-mode parts for C1, C2 and C20. Examples: TDK CGA/C-series "soft termination", Murata GCJ, KEMET FT-CAP. Same footprints. Keep them at least 5 mm from J1/J2 and any screw hole, and orient them parallel to the likely bend axis.
- **Confidence:** Medium (failure mechanism; no field data for this board).

### 5. Medium: SPARE_SW and SPARE_ON still use identical, swappable connectors

- **Evidence.**
  - BOM: J22 and J23 are both Molex 43045-0212, as are J5–J7.
  - REVB_REVIEW.md "Should fix" raised it: "a swapped plug leaves servos live through an E-stop". No outcome is recorded in later rounds.
  - That defeats the "off with the motors" purpose of SPARE_SW. Swapping SPARE_ON with a compute output also moves loads between fuse ratings.
- **Fix.** Make J22 (SPARE_SW) mechanically unique, for example a 43045-0412 (2 × 2) with two pins unused, or a Micro-Fit with a different position count. One part-number change, no circuit change.
- **Confidence:** High on the fact. Severity depends on how the harness is managed.

### 6. Medium: timing and decoupling MLCCs lack a dielectric or voltage spec

- **Evidence.**
  - BOM rows with no voltage, or no dielectric: C8 "100nF" 0402 on **VCTL**, C7 "10nF" on L_B, C37 "1nF", C18/C21–C24/C30/C34 "100nF", C42 "1uF 25V" (no dielectric), and C43 (DNP).
  - The audit rule is "≥ 2× working voltage".
  - VCTL is 12.1 V, and 19.2 V under the shorted-U2 case the design claims to survive. A 0402 100 nF bought "by value" is often 10 V or 16 V.
  - C42 (RST_SRC, 4.7k pull-up) sets the held-reset immunity verified by latch_sim at a nominal 1 µF (`CRST RST_SRC 0 1u`). As a 25 V 0603 at a 12 V bias, its effective value is well under 1 µF, and an X5R or Y5V part loses more again. The sim never ran a reduced CRST.
- **Fix.**
  - C8 and C43: 25 V X7R minimum (50 V preferred).
  - C42: 1 µF 50 V X7R 0805, or keep 0603 and rerun latch_sim at CRST = 0.4 µF.
  - C7 and C37: X7R.
  - Put voltage and dielectric in every BOM value string. This matters most if parts are matched automatically in BOM Manager.
- **Confidence:** High on the missing specs. Medium on the C42 impact (the sim wasn't rerun here).

### 7. Minor: R48 (0603) sees about 80 V across it on a MID transient

- **Evidence.**
  - R48 220k 0603 runs from MID_Z (MID − 10 V zener) to MID_B (clamped at Q11's VBE).
  - Steady at a 54.6 V pack: about 44 V. At MID 91 V (B opens first): about 80 V.
  - Yageo RT/RC 0603: 75 V max working, 150 V overload. RT datasheet: https://pdf.elecfans.com/p/5242229.html. 0402 is 50 V and 1206 is 200 V.
  - It's inside the overload rating for µs, but over the working voltage.
- **Fix.** R48 → 0805 or 1206. No other 0402 or 0603 part exceeds its working voltage. I checked all 47 small resistors against their net pairs (list below).
- **Confidence:** High.

### 8. Minor: continuous dissipation in small resistors

| Ref | Case | Condition | Power | Rating (70 °C) |
| --- | --- | --- | --- | --- |
| R10 4.7k 0402 | VCTL to RST_SRC | Q10 or Q14 on, i.e. almost always | 12.1²/4.7k = **31 mW** | 62.5 mW, 50 % |
| R58 47k 0603 | VB_Z to VB_B | Q14 base drive whenever VBUS is up | (58 − 5.1 − 0.7)²/47k = **58 mW** | 100 mW, 58 % |
| R15 1.2k 0603 | RUN | latch on | 7.7²/1.2k = 49 mW | 100 mW, 49 % |

In a 50 °C torso, near FETs that dissipate 2.7 W each, local ambient can pass the 70 °C derating knee.

- **Fix.** R10 → 0603, R58 → 0805.
- **Confidence:** High.

### 9. Minor: C14/C15 DC-bias derating weakens the converter-input damping analysis

- **Evidence.**
  - The M7 fix claims EEE-TK2A470AQ (about 0.42 Ω ESR) over-damps the pigtail (75–150 nH) "against the board's 4.4 µF". That figure is 2 × 2.2 µF 100 V X7R 1210 at nominal.
  - At 54.6 V DC, a 100 V X7R 1210 keeps well under half its nominal value (exact figure depends on the part; Murata/TDK curves).
  - ζ = (R/2)·√(C/L): about 1.14 at 4.4 µF and 150 nH, and about 0.7 if the effective C is about 1.6 µF. That's mildly under-damped, and the resonance moves to about 325 kHz.
- **Fix.** Pick the MPNs, read the bias curve at 55 V, and rerun the damping numbers. If needed, change to 2 × 4.7 µF 100 V 1210.
- **Confidence:** Medium. The derated value is an estimate until a part is chosen.

### 10. Minor: U2 thermal pad land smaller than TI's DGN0008B

- **Evidence.**
  - The footprint is `HVSSOP-8-1EP_3x3mm_P0.65mm_EP1.57x1.89mm`, whose KiCad description cites tpa6110a2.
  - The TPS7A4001 datasheet (https://www.ti.com/lit/ds/symlink/tps7a4001.pdf) uses outline **DGN0008B**. Its land and stencil example shows a **1.88 × 1.98 mm** exposed-pad land; outline numbers 1.62–1.88 by 1.78–1.98. Signal pads are 1.4 × 0.45 mm at 0.65 mm pitch.
  - The pinout matches (1 OUT, 2 FB, 4 GND, 5 EN, 8 IN; pad = GND).
  - U2 dissipates about 0.5–0.6 W at RθJA 66.7 °C/W. A smaller land is solderable, but it gives away some of the thermal path.
- **Fix.** Enlarge the EP pad to 1.88 × 1.98 in a project copy of the footprint, and put 4–6 vias in it.
- **Confidence:** Medium. The datasheet drawing text was ambiguous about tolerances.

### 11. Minor: LM4040 SOT-23 substitutes

- **Evidence.**
  - TI LM4040 SOT-23: pin 1 cathode, pin 2 anode, pin 3 "must float or connect to anode" (https://www.ti.com/lit/ds/symlink/lm4040-n.pdf). The netlist matches: 1 VREF_CL, 2 GND, 3 NC.
  - The Diodes/Zetex LM4040 SOT23 notes say "die attached to pin 3 … contact between pin 2 and pin 3", and "Pin 3 must be left floating or connected to pin 2" (https://www.digikey.at/en/htmldatasheets/production/120592/0/0/1/lm4040b25fta). With pin 3 left NC on this board, that's harmless.
  - TL431-family and other "2.5 V reference" SOT-23 parts that an assembler might substitute have different pinouts.
- **Fix.** Order TI LM4040AIM3-2.5/NOPB by exact MPN, and mark it "no substitute" if JLC or BOM-Manager matching is used. Keep pin 3 NC. Same applies to TLV1805, whose IN− 3 / IN+ 4 pin order is non-standard.
- **Confidence:** Medium.

### 12. Minor (cost): INA238 is a drop-in for the seven INA228s

- **Evidence.**
  - INA238 (https://www.ti.com/lit/ds/symlink/ina238.pdf) has the same DGS VSSOP-10 package and the same pins: 1 A1, 2 A0, 3 ALERT, 4 SDA, 5 SCL, 6 VS, 7 GND, 8 VBUS, 9 IN−, 10 IN+.
  - It has the same 85 V common-mode and VBUS abs max, and the same 16-address A0/A1 map (0x40–0x4F).
  - It's 16-bit and has no energy or charge accumulators. Nothing in REV_B_PARTS uses those: the clamp-energy cap is computed from bus voltage and clamp on-time.
  - At 1 mΩ, 16 bits is still finer than 10 mA per LSB at ±163 mV range.
- **Fix.** Price INA238AIDGSR against INA228 in the DigiKey upload. If it's cheaper, switch all seven fitted parts (and the 2 DNP); software needs register-map changes. Optional.
- **Confidence:** High on compatibility. The saving needs a live price.

### 13. Nit: BOM text

- F5 reads "2A 32V 0466002". Per the Littelfuse 466 table (https://static.chipdip.ru/lib/353/DOC012353712.pdf), the 2.00 A rating is in the **63 V** group (50 A @ 63 V). The 2.5–5 A ratings are the 32 V group.
- F1 "250mA 125V" is correct: 125 V, 50 A @ 125 V, 0.691 Ω cold, melting I²t 0.0022 A²s.
- The audit asks for 0466 order codes "0466.250NR" and so on. Keep the "HF" (halogen-free) variants in mind if NR stock is short.

### 14. Optional simplification: SPARE_ON eFuse (U18 + R41/R42/R43/C35/C36) → one fuse

SPARE_ON feeds the head electronics, and J5–J7 already use plain 0466 fuses on the same rail. A 0466002 (2 A) in place of U18 and its five passives:
- removes one of the two DSG parts that are hard to hand-solder (finding 2);
- removes finding 1 entirely.

What you lose: auto-retry, and a 5 µs trip (a fuse blows instead). Rev B M7 replaced a polyfuse because of hold derating; a fast fuse has no such derating at 1 A. This is Joey's call under the clean-sheet rule.

---

## Checked and OK

**Absolute maximum vs nets**
- **U8 TPS48100-Q1.**
  - VS, CS± and SRC are rated −65 to 100 V (parts JSON, SLUSEW1) against 91 V on BAT. VS also has a 100 Ω/100 nF filter.
  - EN/UVLO: −1 to 70 V against about 6.4 V.
  - FLT: 20 V against 12.7 V max (BZT52C12).
  - BST–SRC: 19 V.
  - ISCP/TMR: 5.5 V.
  - CBST: 2.2 µF 25 V 0805 still exceeds the eq-4 minimum (about 0.42 µF for 2 × 211 nC) after DC-bias loss.
- **U1/U7 LM5069** (https://www.ti.com/lit/ds/symlink/lm5069.pdf).
  - VIN, SENSE, OUT, PGD, GATE and UVLO are all rated −0.3 to 100 V. U7's UVLO tied to VIN is fine.
  - OVLO is rated 7 V, and both are tied to GND.
  - PGD off-leakage is 5 µA max at 80 V. Against 470k from BRK_IN that still leaves at least 45 µA in D15 at a 36 V pack.
- **U2 TPS7A4001.**
  - IN and EN are rated 105 V, with EN–IN ≤ 0.3 V, so EN tied to IN is fine.
  - FB divider: 464k/49.9k gives 12.08 V, with 23.5 µA in the divider.
  - COUT: 10 µF 50 V X7R 1210 against the >4.7 µF requirement, fine at 12 V.
  - CIN: 1 µF 100 V meets the minimum.
- **U3 TLV1805.**
  - VS is rated 42 V against VCTL ≤ 19.2 V.
  - SHDN is tied to V−.
  - IN− is pin 3 and IN+ pin 4, matching the netlist (VREF on IN−, sense on IN+), and the hysteresis polarity is correct.
- **INA228** (https://www.ti.com/lit/ds/symlink/ina228.pdf).
  - Common-mode and VBUS are rated −0.3 to 85 V, and the differential ±40 V.
  - Limb meters sit at VBUS ≤ 58 V. U17 is behind 33k plus 68 V. U6 and U10 are on 12 V.
  - IN± bias is ≤ 2.5 nA, so it's negligible.
  - No meter is strapped to SDA. The pinout matches DGS.
- **Small transistors.**
  - Q3 MMBTA92: VEB reverse is ≤ 1 V, because CDV tracks GATE through D2.
  - Q4/Q5/Q10/Q11/Q14 (40 V) all sit on nets ≤ VCTL.
  - Q12 2N7002: gate clamped ≤ 12.7 V, drain behind 220 Ω.
  - SOT-23 pinouts are standard: B-E-C, and G-S-D for the 2N7002. MMBTA92LT1G is already settled.
- **Diodes.**
  - BAV19W (100 V) on D2/D10/D11.
  - BAT46W (100 V) on D13/D17.
  - BAT54S orientation is correct: GND, node, 3V3.
  - SMAJ15A on VCTL and LOOP_RET.
- **TPS259541.**
  - The DSG pinout matches TI: 1 dVdt, 2 EN, 3/4 IN, 5 OUT, 6 FLT, 7 ILM, 8 GND, pad to GND.
  - R40 499 Ω gives about 4.05 A, and R43 1.02 kΩ about 2.0 A (I_LIM ≈ 2000/R + 0.04). Both are inside 487–5000 Ω.
  - CdVdt 3.3 nF matches TI's recommended 3300 pF.
  - U9's EN from RUN is fine.

**Voltage ratings**
- **Resistor working voltage** (Yageo RT: 0402 50 V, 0603 75 V, 1206 200 V). Every 0402 resistor's worst pair is under 50 V: R29, R30, R47, R5, R13 and the rest. R47 sees about 27 V transient.
  - 1206 parts on high-voltage nets: R19 (about 25 V), R20 (about 30 V), R33/R34 (up to 91 V), R46 (91 V), R51 (79 V), R44/R45 (harness short 54 V). All fine.
  - R22/R64 9.53 MΩ 0603 see ≤ 12 V. The part exists: Yageo RC0603FR-079M53L (https://www.digikey.ch/en/products/detail/yageo/RC0603FR-079M53L/5918431).
  - R21 51.1k 0.1 % is inside RT0402's range, and R19/R20 inside RT1206's (≤ 1.5 MΩ).
- **MLCC voltage on BAT-side nets.** All are 100 V: C1, C2, C20, C27, C39, C14, C15, C40 and C4. Peak 91 V is 91 % of rating for µs, which is acceptable for MLCC. No MLCC sits on VBUS.
  - +12V-side MLCCs are 25 V against D20's clamp, ≤ 24.4 V at full Ipp.
  - C41 hybrid 25 V has surge headroom.
- **Fuses.**
  - F1 0466.250: 125 V, 50 A breaking.
  - F3/F4 0466005: 32 V, 50 A. A 12 V short is bounded by the DVE at about 30 A. C41's discharge I²t (about 0.6 A²s at 300 A peak, τ ≈ 13 µs) is below the 5 A part's 2.5 A²s melting I²t, so the fuse clears on DVE current, inside 50 A.

**Leakage on high-impedance nodes**
- Clamp divider: Thevenin about 48.8 kΩ at CL_SENSE. 1 GΩ of surface leakage from 57 V shifts the threshold by about 0.06 V.
- R22+R64 (19 MΩ): a 100 MΩ flux path raises hysteresis by about 15 %. Clean the flux off; no part change needed.
- U17 path: D18 leakage of 0.05 µA × 33k is 1.6 mV.
- BRK_PG: PGD leakage is covered above.
- ESTOP_PI_N: with a no-pull GPIO it reads about 3.0 V open.

**Footprints**
- INA228 on KiCad `TSSOP-10_3x3mm_P0.5mm`: 1.45 × 0.3 mm pads at ±2.15 mm, close to TI DGS0010A (1.45 × 0.3 at about ±2.2). Fine.
- TPS259541 on `Texas_DSG0008A`: 0.6 × 0.25 mm pads, 0.9 × 1.6 mm EP. Matches DSG0008A.
- LM5069 on MSOP-10, TLV1805 on SOT-23-6, LM4040 on SOT-23: fine.

**Ratings and parts already justified**
- C4 100 nF C0G 1210 (CDV ≤ 67 V), C19 220 nF C0G, C3 4.7 nF C0G timer.
- LM4040A bias: 1.06 mA, inside the 60 µA–15 mA range.
- U2 load and heat: about 0.5–0.6 W, about +38 °C.
