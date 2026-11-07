# Power board rev B4: pre-layout audit

2026-11-07, schematic at main bd433155 (203 parts, 187 fitted). It covers four things:
- the orderable part numbers (MPNs) and the footprint land patterns;
- the thermal copper for the power FETs;
- the floorplan refresh;
- the issues found, ranked.

Routing hasn't started; Joey reviews placement first.

**Stock method.** DigiKey and Octopart block automated stock queries from Joey-PC, so every stock figure below comes from a dated distributor page found by search. A live check is one upload away: [bom-digikey-upload.csv](bom-digikey-upload.csv) lists the MPN and quantity for every fitted line, for DigiKey's BOM Manager. Run it before ordering.

## Blocking issues (fix or decide before layout)

| # | Part | Issue | Proposed fix |
| --- | --- | --- | --- |
| 1 | C38, C44, C45: Kyocera AVX APZ1012470M100R, 47 µF 100 V, 3 needed | **Supply.** DigiKey shows 0 in stock with a 24-week factory lead time. Octopart's inventory trend is down 100 % over three months | Find a 47 µF 100 V hybrid polymer in a 10 × 12.5 mm can with ESR ≤ 50 mΩ and a µs surge rating. Rerun absorber_sim at that part's ESR. Sourcing is a task for the Argon reader, plus a DigiKey check |
| 2 | C45 placement | **Doesn't fit on the front face.** Row 2 (bank B, latch) has no room for the two new cans. C44 fits a pocket under J26; C45 doesn't | Joey's call. Put C45 on the back face under bank B, where the absorber loop runs D7 → C → GND through vias. Or shrink bank A's block to widen the pocket |
| 3 | C10–C13: HHXF800GRA101MJH0G, 100 µF 80 V bus caps | **Part number not verified** (no datasheet or distributor listing found). The sheet note says the can is 10 × 16.5 mm, but the footprint is CP_Elec_10x14.3 | Pick a real 100 µF 80 V hybrid polymer and set the footprint from its datasheet. Bus caps need the Z 67 strip anyway |
| 4 | **Fixed:** R22: 19.1 MΩ 0402 (clamp hysteresis) | **Not a standard value in 0402**: the common thick-film series stop at 10 MΩ | Two 9.53 MΩ 0603 in series (0.1 % not needed) or a high-ohm series part. Same hysteresis |
| 5 | R53: 0R 2512 in the absorber ground leg | **Carries the full trip pulse**, up to about 900 A for about 10 µs, about 3 A²s. A generic 2512 jumper has no rating for that | Replace R53 with a direct copper connection, which drops the 10b option's footprint, or use a jumper with a published pulse rating. Joey decides with the 10b DNP decision: 10b only matters if the JK trips on close |
| 6 | Bank A FETs Q8, Q9 | **Outside the fan zone.** The top strip (Y 305–339) is above the upper fan zone (Y 264–304). See the thermal section | Give them extra copper (below), or swap bank A with the latch block so both banks sit under the fans |

## Fixed in the schematic (2026-11-07)

- R22 is now 2 × 9.53 MΩ 0603 in series (R22 + R64, via net CL_HY). Hysteresis is unchanged.
- Q3 value is now MMBTA92LT1G (onsemi, B1/E2/C3).
- U9 and U18 values are now TPS259541DSGR.
- C41 is now Panasonic EEH-ZA1E331P: 330 µF 25 V hybrid, 20 mΩ, 2.5 A ripple, 10 × 10.2 mm. The footprint moves from CP_Elec_8x10.5 to CP_Elec_10x10.5.
- R59 is now Vishay CRCW251222R0FKEGHP: 22 Ω 1 % 2512, pulse-withstanding (HP), 1.5 W.

## Footprint and land-pattern checks

| Part | Footprint | Check |
| --- | --- | --- |
| RS1–RS10, Bourns CSS4J-4026 | R_Shunt_Isabellenhuette_BVR4026 | **Matches** Bourns' recommended pad layout (datasheet p2). Envelope 10.6 × 7.3 mm; current pads 2.55 × 5.6 mm with a 5.5 mm gap. Sense pads differ slightly: KiCad has 0.9 mm sense pads with a 0.8 mm gap, Bourns 0.8 mm pads with a 0.9 mm gap. Within tolerance. Bourns' L-shaped sense tab is missing, so route the sense traces from the inner edge of each sense pad |
| U8 TPS48100QDGXRQ1 | marengo_revb:TPS48100_DGX_VSSOP-19 | Custom footprint. Checked by four reviewers against the DGX0019A drawing: 19 pads, no pin 16 |
| Q1/Q2/Q7/Q8/Q9 IPB017N10N5LFATMA1 | TO-263-7_TabPin4 | Matches; the tab is pin 4 = drain |
| Q6/Q13 BSC040N10NS5ATMA1 | TDSON-8-1 | Matches. Pads 5–8 are drain: tie them together in copper (DRC note from the reviews) |
| U9/U18 TPS259541 | Texas_DSG0008A WSON-8 2 × 2 | Matches. **Order as TPS259541DSGR**; the BOM value has no reel suffix |
| U6/U10–U17 INA228AIDGSR | TSSOP-10_3x3mm_P0.5mm | DGS is VSSOP-10 3 × 3 mm, 0.5 mm pitch: same land |
| U1/U7 LM5069MMX-1/-2 | MSOP-10 | Matches. Order with the /NOPB suffix |
| U2 TPS7A4001DGNR | HVSSOP-8-1EP | Matches. The exposed pad goes to a GND pour with vias (0.47 W) |
| Q3 MMBTA92 | SOT-23 | The symbol is B1/E2/C3. **Order onsemi MMBTA92LT1G**, which has that pinout; other vendors differ |
| D1 5.0SMDJ58A, D4 MBRS3100T3G, D7 SK810L-TP | D_SMC | Match (SMC / DO-214AB) |
| D3/D9 SMAJ15A, D5/D19 B340A-13-F, D8 ES1D | D_SMA | Match |
| D20 SMBJ15A | D_SMB | Matches |
| C38/C44/C45 | CP_Elec_10x12.6 | Matches APZ's 10 × 12.5 mm can. Recheck if the part changes (issue 1) |
| C41 330 µF 25 V polymer | CP_Elec_8x10.5 | **No MPN yet.** Pick a 330 µF 25 V hybrid polymer and match its can |
| F1/F3/F4/F5 Littelfuse 0466 | Fuse_1206 | Matches. Order 0466.250NR, 0466005.NR, 0466002.NR |
| F2 0997020.WXN | Keystone 3568 holder | Matches |
| J1–J4, J8–J15, J25, J26 | Würth REDCUBE 74651194 (M4, THR) | Matches the KiCad library |
| J5–J7, J22–J24 | Molex Micro-Fit 43045-0212 / -1212 | Matches. J24 pin n faces pin n+6, checked by reviewers |
| J20/J21 | JST GH SM08B / SM05B-GHS-TB | Matches |

## Orderable part numbers (fitted)

| Line | MPN to order | Stock seen (date, source) |
| --- | --- | --- |
| U8 | TPS48100QDGXRQ1 | In stock at DigiKey (3,656 on one regional page, undated); 18-week factory lead time |
| C38/C44/C45 | APZ1012470M100R ×3 | **0 at DigiKey, 24 weeks** (issue 1) |
| Brake thermostat (off board) | 67L120P (fallback 67L120) | **67L120 at 0 on DigiKey France, 2026-11-07** |
| U1/U7 | LM5069MMX-1/NOPB, LM5069MMX-2/NOPB | Not checked: use the upload |
| U2, U3, U4 | TPS7A4001DGNR, TLV1805DBVR, LM4040AIM3-2.5/NOPB | Not checked |
| U6, U10–U14, U17 | INA228AIDGSR ×7 | Not checked |
| U9, U18 | TPS259541DSGR ×2 | Not checked |
| Q1, Q2, Q7, Q8, Q9 | IPB017N10N5LFATMA1 ×5 | Not checked |
| Q6 | BSC040N10NS5ATMA1 | Not checked |
| Q3 / Q4 / Q5, Q10, Q11, Q14 / Q12 | MMBTA92LT1G / MMBT3906LT1G / MMBT3904LT1G ×4 / 2N7002LT1G | Commodity |
| Diodes | 5.0SMDJ58A, BAV19W-7-F ×3, SMAJ15A ×2, MBRS3100T3G, B340A-13-F ×2, BAT54S-7-F, SK810L-TP, ES1D-13-F, BZT52C12-7-F ×2, BAT46W-7-F ×2, BZT52C10-7-F, BZT52C5V1-7-F, BZT52C68-7-F, SMBJ15A | Commodity |
| Shunts | CSS4J-4026R-L500F, CSS4J-4026R-1L00F ×4, CSS4J-4026K-2L00F, CSS4J-4026K-5L00F ×2 | Not checked |
| Fuses | 0466.250NR, 0466005.NR ×2, 0466002.NR, 0997020.WXN + Keystone 3568 | Not checked |
| Connectors | Würth 74651194 ×13, Molex 43045-0212 ×5, 43045-1212, JST SM08B-GHS-TB, SM05B-GHS-TB, Keystone 5015 ×6 | Not checked |
| Precision resistors | RT1206BRD07499KL, RT1206BRD07604KL, RT0402BRD0751K1L (Yageo RT, 0.1 %) | Commodity |
| R59 (U2 feed, 22 Ω pulse) | A pulse-rated 2512 (anti-surge thick film). Pick one with a published single-pulse rating of at least 0.3 J | **MPN to pick** |
| Other R and C | Spec only (value, tolerance, voltage, dielectric, size from the BOM). Match in BOM Manager: Yageo RC/RT for resistors, X7R/C0G MLCC at ≥ 2× working voltage | Commodity |

## FET thermal copper

Conduction loss at a 60 A bus, using the hot Rds(on) (1.7 mΩ max × about 1.75 at Tj 110 °C):

| Part | Current | Loss |
| --- | --- | --- |
| Q8, Q9 (bank A), each | 30 A | 2.7 W |
| Q1, Q2 (bank B), each | 30 A | 2.7 W |
| Q7 (converter path) | 3.9 A | 0.04 W |
| Q6 (clamp, while on) | 5.7 A | 0.23 W |
| RS1 (0.5 mΩ) | 60 A | 1.8 W |
| RS2–RS5, per limb | 19 A typical, 60 A peak | 0.36 W typical, 3.6 W peak |

**Budget.** Tj ≤ 110 °C from a 50 °C torso gives 60 K. With RthJC 0.4 K/W, each main FET needs RthCA ≤ 22 K/W. The datasheet gives 40 K/W for 6 cm² of single-layer 70 µm copper in still air. That's a 157 °C junction, so the datasheet footprint alone isn't enough.

**Layout requirements:**
- **Stackup:** 4 layers, 2 oz (70 µm) outer, ≥ 1 oz inner. The 60 A nets (BAT, SW_D, MID, VBUS) pour on all four layers.
- **Copper per FET:** at least 10 cm² of drain-net copper on the top layer per FET. Bank A's shared SW_D pour is ≥ 20 cm²; bank B's tabs sit on MID, ≥ 20 cm². Mirror each pour on the bottom layer.
- **Thermal vias:** 0.3 mm drill on a 1.0 mm pitch. At least 30 per FET tab, under and around the tab, filled or tented on the bottom.
- **Airflow:** bank B sits in the upper fan zone. **Bank A doesn't (issue 6).** Either move bank A under the fans, or give it ≥ 15 cm² per FET and confirm by thermocouple at bring-up.
- **Bench check (added to the bring-up list):** at 60 A for 10 min, each FET case stays under 95 °C.
- **Shunts:** RS1 and the limb shunts are Kelvin parts with no thermal pad. Give their current pads ≥ 4 cm² each on 2 oz.

## Floorplan

[FLOORPLAN_REVB.png](FLOORPLAN_REVB.png) is refreshed for rev B4:
- J25/J26 (HS50 B, −X gusset) sit on the top edge, +X half, inboard of J3/J4. The clamp sits under them.
- The rev B4 parts are added.
- The MID meter group (U17, its zener, the CS RC placeholder) moves to the bottom free zone.
- Courtyards about 7,900 mm², 88 % of the one-sided 60 % budget. area.py says 90 % with its stricter count.
- **About 8 cm² free remains.** C45 is the open placement question (issue 2).

## Options for Joey (2026-11-07): absorber, bus caps, thermostat

### Absorber caps (100 V rating required; nothing below 100 V)

Peak BAT is from absorber_sim at the worst corners: 0.05 Ω pulse loop, 0.5–1 µH, D1 at 71.2 V, C −20 %, trip at 8.8–17.5 µs. Specs are from the Kyocera AVX APZ and APC datasheets.

| Option | Parts | ESR, ripple | Peak BAT | Stock / price | Fits the floorplan? |
| --- | --- | --- | --- | --- | --- |
| A (now) | 3 × APZ1012470M100R, 47 µF, 10 × 13 mm | 50 mΩ, 0.9 A | 79.8–90.8 V | US DigiKey 0; other DigiKey regions showed 4,475 to 9,296 (undated snapshots). About $3.60 | As now (C45 is the open question) |
| B | 3 × **APC1012470M100R**, 47 µF 100 V, same 1012 can, 15,000 h | **35 mΩ**, 2.3 A | 79.7–91.3 V | Not found on distributor pages yet. Ask Mouser/DigiKey | Drop-in footprint |
| C | 4 × APC1012470M100R | 35 mΩ | **75.6–85.7 V** | As B | One more can, 149 mm²: no room without the back face |
| D | 6 × APZ0811220M100R, 22 µF 100 V, 8 × 11 mm | 50 mΩ, 0.6 A | 80.8–93.2 V | Not checked | Three more cans: no |
| ✗ | 100 V aluminium electrolytic (about 0.3 Ω) | | 109–131 V | | Fails: ESR dominates. Not usable even with film or ceramic in parallel, because the ceramic's effective µF at 55 V is too small to carry a 500 A pulse |

**Recommendation:** keep A. Order from a DigiKey region or Mouser that shows stock, and buy spares. B is the second source in the same footprint once its stock is confirmed.

### Bus caps C10–C13 (HHXF800GRA101MJH0G is unverified)

VBUS peaks at about 58 V: 57.5 V clamp worst case plus about 0.7 V of regen overshoot. An 80 V part has 27 % margin, so 80 V is acceptable here. 100 V isn't needed on the bus, which sits behind the banks and the absorber.

| Option | Parts | Bulk | Area vs now | Regen overshoot (30 A step, about 6 µs clamp response) |
| --- | --- | --- | --- | --- |
| 1 | 4 × APZ1012680M080R, 68 µF 80 V, 50 mΩ, 10 × 13 mm | 272 µF (−32 %) | **Same** (10 × 12.6 land, about 149 mm² each) | about 0.66 V (was 0.45 V) |
| 2 | 6 × APZ1012680M080R | 408 µF | +298 mm²: the board goes from about 90 % to about 92 % of its area budget | about 0.44 V |
| 3 | 8 × Panasonic EEH-ZC1K470P, 47 µF 80 V, 36 mΩ, 10 × 10.2 mm | 376 µF | +590 mm² | about 0.48 V |

APZ1012680M080R: DigiKey SG/FI showed 1,000 in stock (undated), about $2.57 each, with a 24-week factory lead time.

**Recommendation: option 1.** The drives carry their own DC-link bulk, so the board's bulk only sets local decoupling and clamp overshoot. 0.66 V is far inside the 2.5 V between the 57.5 V clamp and the drives' 60 V fault. U1's start (power limit, timer) gets easier with less C. If you'd rather keep 400 µF, take option 2 and put two cans on the back face.

### Thermostat (in-stock alternatives to the 67L120P)

| Part | Opens / closes | Mount | Stock seen |
| --- | --- | --- | --- |
| Sensata 67L120 (metal bracket) | 120 ±5 °C / ≥ 85 °C | TO-220, M3 | 48 at DigiKey US per one aggregator (undated); 0 on DigiKey FR/ES |
| Sensata 67L115P / 67L115 | 115 ±5 °C / ≥ 80 °C | TO-220, M3 | Not checked. Same datasheet, 5 °C earlier trip, also acceptable |
| RS PRO 222-8237 | 120 °C / 105 °C, auto reset | Flange, tab terminals | 63 at RS UK (undated). Read its datasheet before choosing: the ratings aren't confirmed yet |

## Decisions applied (Joey, 2026-11-07 15:12Z)

1. **Absorber:** keep APZ1012470M100R. Order from a DigiKey region that shows stock, and buy 2 spares. The alternate is APC1012470M100R: same can, 35 mΩ, 15,000 h.
2. **Bus caps:** C10–C13 are 4 × APZ1012680M080R, 68 µF 80 V, on the existing 10 × 12.6 land. That's 272 µF in total.
3. **Thermostat:** Sensata 67L120 (metal bracket) is primary, 67L120P the alternate.
4. **Placement:**
   - C45 goes on the back face under bank B.
   - R53 becomes direct copper: a NetTie-2 footprint, at least 5 mm wide in layout. Cut it only to fit the 10b option.
   - Bank A moves into the fan zone beside bank B. The latch block moves up to the top strip, with about 0.8 cm² spilling beside C44.

ERC clean, check.py 92/92, latch_sim 54/54. The floorplan is updated.

## Rough cost estimate (2026-11-07)

These are estimated distributor prices at qty 1 from typical catalogue pricing, with a ±25 % band. They aren't quotes. Run bom-digikey-upload.csv for real prices.

**Board parts, one board: about $215**

| Group | Estimate |
| --- | --- |
| Power FETs, 5 × IPB017N10N5LF | $28 |
| Controllers: TPS48100, 2 × LM5069, TPS7A4001, TLV1805, LM4040, 2 × TPS259541 | $24 |
| 7 × INA228 | $39 |
| 8 × CSS4J shunts | $14 |
| Absorber 3 × APZ 47 µF 100 V, bus 4 × APZ 68 µF 80 V, EEH-ZA1E331P | $23 |
| 13 × Würth REDCUBE M4 terminals | $46 |
| Micro-Fit, JST GH, test points | $13 |
| Diodes, small transistors, fuses + holder | $13 |
| MLCCs, resistors (including the 0.1 % and pulse parts) | $18 |

**PCB:** 4 layers, 2 oz outer, 100 × 150 mm, qty 5. JLCPCB about $60–120 including shipping; PCBWay about $100–180. That's about $15–35 per board. Bare boards only: no assembly, and the THR terminals are hand-soldered.

**Off-board power parts: about $230, or about $345 with the BMS and E-stop button**

| Part | Estimate |
| --- | --- |
| Murata DVE0125V2NBC | $85 |
| Converter adapter parts (EEE-TK2A470AQ, SK810L, caps) plus a small adapter PCB | $15 |
| 2 × Arcol HS50 20R J | $20 |
| Sensata 67L120 thermostat | $10 |
| Limb fuses: 6 × Littelfuse 997 MINI + inline holders | $25 |
| Class T 60 A pack fuse + holder | $40 |
| Mating connectors and crimps: Micro-Fit plugs, JST GH leads, ring lugs | $25 |
| Wire: 18 AWG silicone, twisted pairs, loop wiring | $10 |
| JK BMS JK-BD6A17S8P | about $100 |
| HB2-BS545 E-stop button | about $15 |

**Total for one powered board set: about $500–600** (one board's parts, one board's share of the PCB order, the off-board parts, the BMS and the E-stop). Cells and the pack aren't included.
