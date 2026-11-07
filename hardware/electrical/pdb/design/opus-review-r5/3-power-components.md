# Review 3: power and high-current component choice (PDB rev B4, pre-layout)

Scope: FETs, shunts, fuses, absorber and bus caps, TVS, current-carrying terminals and connectors, brake resistors and thermostat, the DVE0125V2NBC and its adapter parts, and the hot-swap and driver controllers. The repo was read only; nothing was edited.

Inputs: REV_B_PARTS.md, REV_B_CLEAN_SHEET.md, PLAN.md, PRE_LAYOUT_AUDIT.md, RATIONALE-log.md, bom-rev-B.csv, REVB_REVIEW through REVB4_REVIEW, the schematic sheet notes (parsed with tools/sx.py), and tools/absorber_sim.py.

Network note: the proxy blocked direct PDF downloads, so log-log charts could not be read. That covers the IPB017 SOA and Zth curves and the SK810L forward curve. Every value below is from datasheet text or a distributor page; anything not read from a source is marked **unverified**.

---

## Findings

### F1. HIGH: both Kyocera AVX APZ cap lines are obsolete, and the new bus-cap pick has no franchised stock

- **Parts:** C10–C13 APZ1012680M080R (bus, Joey's 2026-11-07 decision) and C38/C44/C45 APZ1012470M100R (absorber).
- **Evidence (Octopart, snapshot 2026-11-07):**
  - **APZ1012470M100R:** lifecycle **"Obsolete"**, LTB **2026-08-31**, intro 2022-01-26. Stock: Mouser 1,235 at $3.75 qty 1; DigiKey 6,799 (custom reel) at $4.21; RS 0. https://octopart.com/part/kyocera-avx/APZ1012470M100R
  - **APZ1012680M080R:** **"Obsolete"**, LTB **2026-08-31**. Stock: TTI 0, RS 0. The only stock is a broker, ICSuperman, with 1,000. No DigiKey or Mouser stock is listed. https://octopart.com/part/kyocera-avx/APZ1012680M080R
  - The audit's "DigiKey SG/FI 1,000 in stock" was undated. The audit's absorber alternate, **APC1012470M100R, returned no hits** on any distributor page or search. Treat it as unverified, possibly nonexistent, so there is **no verified second source**.
  - The ratings themselves are fine (APZ datasheet, https://www.mouser.com/datasheet/2/40/apz_series-2933544.pdf): 10.0 × 12.5 mm, ESR 50 mΩ max at 100 kHz, ripple 900 mA at 100 kHz/105 °C, DCL 940 µA (47/100) and 1,088 µA (68/80), 2,000 h at 105 °C.
- **Fix, bus caps:** go to **Panasonic EEH-ZC1K470P**: 47 µF 80 V hybrid, 36 mΩ, 4,000 h at 125 °C, 10 × 10.2 mm, an active Panasonic line. https://www.digikey.bg/en/products/detail/panasonic-industry/EEH-ZC1K470P/6035429
  - **Four cans** give 188 µF on the same 10 mm base (land CP_Elec_10x10.5). Regen overshoot scales roughly with 1/C: about 0.66 × 272/188 ≈ 0.95 V. That's my scaling, not a sim; it's still inside the 2.5 V between the clamp and the drive fault.
  - **Five cans** (235 µF) if there's room.
  - Ripple capacity rises too, because ESR drops from 50 to 36 mΩ.
- **Fix, absorber:** buy now, from the remaining Mouser/DigiKey stock, every can for every board you plan to build: 3 per board plus 2 spares.
  - Name a real fallback before layout. One candidate is **Würth 875585961005** (WCAP-HSAH hybrid, 33 µF 100 V, 10 × 12.4 mm, 1.36 A ripple, 4,000 h, in stock at RS). https://no.rs-online.com/web/p/polymer-capacitors/2759509
  - Its ESR is **unverified**, and at 33 µF it needs **4 cans to match 3 × 47 µF**, so rerun absorber_sim at its ESR.
- **Confidence:** high on lifecycle and stock (Octopart). Medium on whether DigiKey still has 68 µF regional stock.

### F2. MEDIUM-HIGH: the power parts don't support the stated "leg-ready" 80 A continuous / 150 A for 100 ms

PLAN §10.1 (Joey, 2026-11-06) set 80 A continuous and 150 A for 100 ms on the main path. The parts on the board give much less:

| Part | Limit | Source |
| --- | --- | --- |
| U1 LM5069-1 + RS1 0.5 mΩ | Current limit 97–123 A, fault timer 0.15–0.39 ms, then **latch**. 150 A for 100 ms is impossible | Sheet note block 1; REV_B_PARTS "keep bus under 90 A peaks" |
| Main FETs (4 in series-parallel) | Copper sized for 60 A (2.7 W/FET). At 80 A: 40 A/FET × 1.7 mΩ × 1.75 ≈ **4.8 W/FET**, which needs RthCA ≤ ~12 K/W, not 22 | IPB017N10N5LF Rev 2.3: 1.7 mΩ max, RthJC 0.4 K/W, 40 K/W on 6 cm² |
| Pack fuse | Class T **60 A** | REV_B_PARTS |
| J1/J2 BMS terminals, J15 GND stud with legs | Würth 74651194 rated **85 A at 20 °C**; ≥ 80 A at a 50 °C torso is over that | https://pdf.elecfans.com/WURTH/74651194R.html |
| Limb fuses | 997 MINI tops out at 30 A; PLAN wants 40 A leg fuses | Littelfuse 997 table |

- **The trap:** the obvious fix, RS1 → 0.3 mΩ (CSS4J-4026R-L300F), raises U1's breaker and U8's comparator threshold by 1.67×. That raises the current the banks interrupt (now 350–890 A) and pushes BAT past the 91 V worst case, onto 100 V parts. So leg current and the absorber design are coupled.
- **Fix:** don't change parts now. Rewrite the board's rating in REV_B_PARTS as **60 A continuous, < 90 A peak**, and record 80/150 A as a rev C item that needs a new absorber and copper. The leg footprints can stay.
- **Confidence:** high. This is all arithmetic on documented limits.

### F3. MEDIUM: limb fuses can't clear a hard limb short; U1 latches the whole bus first

- **Evidence:**
  - **997 20 A:** melting I²t **380 A²s**. Opening time is 80–250 ms at 350 % and 30–100 ms at 600 %. https://html.alldatasheet.com/html-pdf/200417/LITTELFUSE/0997020/600/1/0997020.html
  - **U1:** latches after 0.15–0.39 ms above 97 A, and its breaker (160–260 A) trips in about 1 µs. A 500 A, 10 µs event is about 2.5 A²s, less than 1 % of the fuse's melting I²t.
- **Consequence:**
  - The clean-sheet premise ("a limb fault blows its own fuse first", REV_B_CLEAN_SHEET row 1 condition 4) only holds for overloads of about 27–97 A.
  - A hard short latches the whole robot off. After reset it trips again, because the short is still there and the faulted limb never isolates.
  - It is safe, but it's not the fault-isolation behaviour the design assumes.
- **Fix:** no part change. Document the fuses as wire protection only, and add an operator step: after a latch, unplug limbs one at a time and use the INA228 history to find the bad one. Also specify the fuse rating for each limb pigtail; none is stated (REV_B_PARTS says "same as the arm trunk").
- **Confidence:** high.

### F4. MEDIUM: the brake-resistor software cap (50 W over 60 s) isn't backed by a thermal number; the thermostat will likely trip first

- **Evidence:** Arcol HS series datasheet (https://www.novotone.be/_site/datasheets/Divers/Arcol.pdf):
  - HS50 is 50 W only on a **535 cm² × 1 mm** heatsink, and **20 W without heatsink at 25 °C**.
  - Dissipation "derates linearly to zero at 200 °C", so free air at 50 °C is 20 × 150/175 ≈ **17 W**.
  - Thermal resistance is 3.0 °C/W on the standard heatsink.
  - Overload: "consult the factory" (no rating), which confirms the earlier reviews.
- **My estimate (unverified):** a 60 cm², 5 mm gusset in still torso air (h ≈ 10 W/m²K, both faces) is about 8 K/W plate to air. In parallel with the body's own free-air path, the plate reaches 120 °C (the 67L120 trip) from 50 °C at about **15–20 W per resistor**, 30–40 W total. The 50 W average cap allows 25 W each. A sustained-braking session at the cap would open the E-stop loop: safe, but a nuisance stop. The earlier "45–60 W on a ~60 cm² gusset" for the HS150 only works if the gusset conducts into the frame, and that's unmeasured.
- **Fix:** add a bench step next to step 8, which heats the thermostat with a heat gun: run the clamp at 50 W total for 20 min with the real gussets fitted, and log plate temperature. Set the software average cap at about 80 % of the power that holds the plate at 110 °C.
- **Sourcing:** Arcol is now sold under Ohmite at DigiKey (listed as "ohmite HS50-1R-J, ARCOL HS SERIES"). The REVB2 note's DigiKey number "HS5010RJ-ND" is the **10 Ω** part; order the 20 Ω. TE/CGS **THS5020RJ** is the usual same-format equivalent, with the THS50 range widely stocked at RS (e.g., https://uk.rs-online.com/web/p/chassis-mount-resistors/1644410). Check its body dimensions against the gusset holes before swapping (**unverified**).
- **Confidence:** medium on the datasheet facts, low-medium on the thermal estimate.

### F5. MEDIUM-LOW (unverified magnitude): the absorber result rests on D7's fixed 10 mΩ series resistance, and the code doesn't sweep it

- **Evidence:**
  - tools/absorber_sim.py prints "D7 RS 3 mOhm..10 mOhm", but `trip()` is always called with the default `rd=0.01` (lines 18, 26, 41). No corner varies it.
  - D7 is a single SK810L-TP carrying almost all of the 350–890 A interrupted current (D1 takes about 30 A).
  - SK810L datasheet text (MCC): VF 0.80 V max at 8 A, IFSM 200 A (8.3 ms half sine), RθJL 18 °C/W (via https://static.chipdip.ru/lib/733/DOC011733481.pdf).
  - On I²t the part is fine: about 860² × 28 µs / 3 ≈ 7 A²s per event, against IFSM's 200² × 4.15 ms ≈ 166 A²s.
  - The forward curve at hundreds of amps couldn't be read. Every 10 mΩ above the assumed value adds about 5–9 V to BAT at 500–860 A, on top of the 80–91 V worst case and against 100 V parts.
- **Fix:**
  - Add an `rd` sweep (10 / 20 / 30 mΩ) to the corner table. It's one line.
  - If 30 mΩ pushes BAT above about 95 V, fit a larger diode. Rev A's MBRB30H100CT (100 V, 30 A, 275 A surge per RATIONALE-log line 296) is one option; it's a D2PAK, so this is a footprint change before layout. Or put two SK810L in parallel with symmetric copper.
- **Confidence:** low-medium. It's a sensitivity gap, not a demonstrated failure.

### F6. LOW: RS9/RS10 are a "contact factory" value and over-specified; RS8 is over-specified

- **Evidence:** Bourns CSS4J-4026 datasheet (https://www.bourns.com/data/global/pdfs/css4j-4026.pdf):
  - The **-5L00x (5 mΩ) parts are "available upon request – contact factory"**.
  - Ratings at 70 °C terminal: 0.5 mΩ 10 W, 1 mΩ 8 W, 2 mΩ 6 W, 5 mΩ 4 W.
  - STOL is 5× rated power for 5 s.
- **Duty:**
  - RS9 sees at most 12 A for ms (0.72 W) and about 4 A running (80 mW).
  - RS10 sees at most 4.4 A (0.1 W).
  - RS8 sees about 12 A (0.29 W).
  - Each CSS4J land is about 10.6 × 7.3 mm, on a floorplan at 88–90 % of budget.
- **Fix:**
  - **RS8** → **Bourns CSS2H-2512K-2L00F** (2 mΩ, 2512, 5 W at 70 °C, DigiKey-listed: https://www.digikey.com/en/products/detail/bourns-inc/CSS2H-2512K-2L00F/6023752).
  - **RS9, RS10:** CSS2H has no 5 mΩ (datasheet https://www.farnell.com/datasheets/2058210.pdf lists 0.5–3 mΩ). Use a commodity 2512 5 mΩ 1 W metal strip with Kelvin routing; Vishay WSL2512R0050FEA is a common one (**MPN unverified**). Or move U7 and U12 to 3 mΩ CSS2H-2512K-3L00F, and recompute U7's limit and power-limit resistor and the INA228 SHUNT_CAL.
  - That frees about 3 × (77 − 20) ≈ 170 mm² and saves a few dollars.
- **Confidence:** high on availability status, medium on the savings.

### F7. LOW: the BOM's shunt power labels don't match either source

- **Evidence:**
  - BOM says RS1 "5W" and RS2–RS7 "6W".
  - The current Bourns datasheet says 10 W and 8 W.
  - DigiKey listings say **5 W** for CSS4J-4026R-L500F and **4 W** for CSS4J-4026R-1L00F (https://www.digikey.de/en/products/detail/bourns-inc/CSS4J-4026R-1L00F/6023761).
- **Impact:** with the most conservative figure (4 W), a limb shunt at its 60 A peak is 3.6 W, 90 % of rating. That's only OK as a transient, and it's inside STOL (5× for 5 s). RS1 at 60 A is 1.8 W, fine.
- **Fix:** relabel the BOM from the datasheet revision you order against, and use the lowest figure in the thermal notes.
- **Confidence:** high.

### F8. LOW: the 58 V fuse rating has about zero margin to the bus ceiling

- **Evidence:** the 997 is rated **58 VDC, 1,000 A interrupt** (alldatasheet link above). The clamp worst case is 57.45 V, plus about 0.66 V of regen overshoot, which is about 58.1 V on VBUS, where the limb fuses sit.
- **Impact:** a limb fuse that opens during heavy regen with a full pack interrupts at or slightly above its rating. Low probability, and the overrun is about 1 %.
- **Fix:** accept, and note it. For F2 (on BRK, ahead of the E-stop, at ≤ 54.6 V), an SMD option rated 72 V exists, Eaton **CB61F20A-TR1** (20 A, 72 V). It would replace the Keystone 3568 holder and its area, but its **breaking capacity is unverified**. https://www.digikey.ca/en/products/detail/eaton-electronics-division/CB61F20A-TR1/13618600
- **Confidence:** high on the numbers, low on the severity.

### F9. LOW: REDCUBE torque and board thickness

- **Evidence:** Würth 74651194R: 85 A at 20 °C, **tightening torque 1.2 N·m**, PCB thickness **1.6–2.0 mm** (https://pdf.elecfans.com/WURTH/74651194R.html).
- **Fix:** put 1.2 N·m in the harness notes; M4 ring lugs are often torqued higher, which strips a THR insert. Confirm the 4-layer 2 oz stackup is ordered at 1.6 mm, not 1.2 mm or thinner.
- **Confidence:** high.

---

## Checked and OK

- **IPB017N10N5LF (Q1, Q2, Q7, Q8, Q9).** Source: Infineon Rev 2.3, https://www.mouser.com/datasheet/3/70/1/Infineon_IPB017N10N5LF_DataSheet_v02_03_EN.pdf. Active part; RS shows a temporary stock-out with 1,654 due 2026-11-09 at £4.91.
  - **Ratings:** VDS 100 V. RDS(on) 1.7 mΩ max. RthJC 0.4 K/W. EAS 979 mJ at 100 A. ID pulse 1,024 A.
  - **Leakage:** IDSS 100 µA max at 125 °C, which matches the 200 µA/bank used in the self-test window.
  - **91 V BAT:** 9 % under VDS, and the part is avalanche-rated. OK.
  - **Q7:** over-specified for 4 A, but its SOA (12 A at 10 ms) is the reason, and it reuses a line item. Keep.
- **TPS48100-Q1 (U8):** 100 V abs max, 95 V operating, per the repo's gen/parts/tps48100-q1.json note on SLUSEW1 rev C. OK against 91 V. LM5069 is 100 V, per earlier reviews (not re-read).
- **5.0SMDJ58A (D1):** VR 58 V, VBR 64.4–71.2 V, VC 93.6 V at 53.5 A (10/1000 µs), https://pdf.elecfans.com/HAMLIN/5.0SMDJ58AS.html. The sim's model (BV 71.2, RS 0.55 Ω) is consistent. The regen return path can hold BAT near 57.45 V, just under VR 58 V: OK, but tight.
- **APZ ratings vs. sim:** 50 mΩ max and 0.9 A ripple match absorber_sim's ESR.
  - Absorber: ABS node ≤ about 88–91 V against 100 V rated.
  - Bus: 58 V against 80 V.
  - Life: 2,000 h at 105 °C, with near-zero ripple on the absorber, is decades at a 50 °C ambient.
  - Pulse energy into ESR is about 40 mJ per can per event. Trivial.
  - Lifecycle is not OK; see F1.
- **0997020.WXN (F2):** 20 A, 58 VDC, 1 kA interrupt, 3.21 mΩ, I²t 380 A²s. **Keystone 3568:** 30 A, 500 V (https://www.digikey.ca/en/products/detail/keystone-electronics/3568/2137306). The 997 has a voltage-rejection notch: it's meant to fit standard MINI holders, but check physical fit on the first article (**unverified**).
- **0466 (F1, F3–F5):** 125 V / 50 A breaking per the REVB4 review (not re-read). F3–F5 sit behind a DVE that limits at about 30 A, so they interrupt under 50 A. OK.
- **Murata DVE0125V2NBC** (DBE/DVE datasheet SDC_DBE.A07, https://static.chipdip.ru/lib/795/DOC011795410.pdf):
  - **Input:** 0–80 V continuous, **100 V for 100 ms**. This covers the 80–91 V µs BAT peaks. Reverse polarity "None, install external fuse", which the SK810L crowbar and F2 cover.
  - **DBE0125 table:** 20 A external input fuse, which matches F2.
  - **Output capacitance:** 12 V needs 100 µF min, 10,000 µF max, which the adapter's 100 µF polymer meets.
  - **Thermal and logic:** baseplate −40 to 110 °C with no derating. N = negative logic, B = baseplate, C = RoHS.
  - **UVLO:** start 32–36 V, shutdown 30–34 V, matching the 38 V software rule.
  - **Sizing:** 25 A against an 11.6 A ceiling is 2× over-specified. That was Joey's price/area call, and there's no clearly cheaper part with the 100 V transient rating. Keep.
- **EEE-TK2A470AQ:** 47 µF 100 V, 420 mΩ at 100 kHz, 2,000 h at 125 °C (DigiKey). Matches the damping note.
- **SK810L as the DVE reverse crowbar:** a 12 A limit for ≤ 17 ms, then low-duty retry, is far inside the 200 A IFSM. OK.
- **Sensata 6700 / 67L120:** 48 VDC max, 0.5 A at 48 VDC, low-level 1–20 mA at 5 VDC, 20k cycles (https://www.mouser.fi/airpax-sensata-6700-to-220-thermostat). The 17 mA, 12 V loop is inside these.
- **HS50 voltage and isolation:** element limit 1,250 V, proof 2,500 VAC. 57 V is trivial. Wirewound inductance is handled by D4 (rejected-as-settled in REVB4).
- **Bourns CSS4J RS1–RS5:** right parts for the Kelvin high-current duty. The audit already covers footprint and pad copper. STOL 5× for 5 s covers 60 A limb peaks.
- **Würth 74651194 at the current 60 A design:** 85 A at 20 °C. OK; see F2 for legs.
- **Micro-Fit J24:** 2 input pins at about 4.2 A and 3 output pins at about 4 A each with 18 AWG. Well inside typical Micro-Fit per-circuit ratings (not re-read).

## Couldn't verify (proxy blocked the PDFs; read these before layout sign-off)

1. **IPB017N10N5LF turn-off SOA** at the interrupt event: about 445 A per FET in bank B, VDS ramping to 80–90 V over about 1–3 µs. My rough energy estimate is 20–40 mJ per FET (unverified). Check it against Diagram 3 (1 µs and 10 µs lines at 80–100 V) and Diagram 4 (ZthJC at 1–10 µs), derated from Tc 25 °C to a hot case.
2. **SK810L forward voltage at 300–900 A** (feeds F5).
3. **APZ (or replacement) pulse-current capability:** about 300 A per can for tens of µs. Not rated in the datasheet.
4. **Eaton CB61F20A breaking capacity; TE THS50 body dimensions; Vishay WSL2512R0050FEA MPN.**

## Not re-raised (settled in earlier rounds)

- The HS50 burst and hot-spot caveat.
- D4 MBRS3100's 3 A rating.
- The absorber topology and 10b DNP.
- Q7 package size.
- The Class T 60 A vs BMS 80 A ordering ("can wait", REVB). It appears in F2 only as part of the leg-rating conflict.
