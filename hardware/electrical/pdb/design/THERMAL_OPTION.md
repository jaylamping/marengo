# Power board rev B: thermal option (FET cooling)

2026-11-07. Joey (17:34Z): uncomfortable using the PCB as the heatsink; fine with heatsinks, bus bars and separately cooled
paths; will place the power hardware in the torso's upper cavity himself (the 24 mm envelope is no longer a constraint).
This page compares the options and recommends one. **Nothing in the schematic or layout has changed for it yet.**

## Recommendation

**Keep the D2PAK-7 IPB017N10N5LF FETs on the board, and take their heat out through the back of the board into one finned
aluminium heatsink (option D).** The board copper then only carries current, and the heat goes into a separate,
air-cooled heatsink.

Why not through-hole FETs:
- **Bank B is the soft start for the whole motor bus.** Its FETs run in linear mode at about 2 A and 54 V for 0.25–0.55 s
  (gen.py block 3: bus ceiling 9.4 mF at the 2.06 A hot DC SOA line).
- **Every TO-220 and TO-247 trench FET checked holds only 0.2–2 A at 50 V DC** (datasheet SOA graphs, table below).
  IXYS's linear "L2" parts can do it, but at 11–18 mΩ, $12–36 each and 44-week lead times. They would need a hybrid:
  an L2 soft-start FET in parallel with low-Rds trench FETs and a second gate drive, which adds new fault modes.
- The IPB017N10N5LF is Infineon's linear-mode ("LF") part. It is the one FET checked that covers all the duties: soft start,
  1.7 mΩ conduction and the port short-circuit pulse.

Option D, against the current board:
- **Same parts, same circuit and the same simulations.** It adds a heatsink, a thermal pad and four screws.
- About $20 per set.
- **Junction temperature at 60 A: about 75–80 °C** with fan air, against about 90–100 °C (estimate, unverified) from the
  board copper alone.

## The duties

| FET | Duty | What decides the part |
| --- | --- | --- |
| Bank A (2 in parallel) | switch, 30 A each continuous; opens on a bus short (trip cuts 350–890 A) | Rds(on), avalanche |
| Bank B (2 in parallel) | **soft start of the whole bus**: linear mode, about 2 A at up to 54 V for 0.25–0.55 s; then a switch at 30 A each | **DC SOA at 50 V** and Rds(on) |
| Ports (3 fitted) | a switch (the port turns on while the bus is still low and its output follows the bus); hard short 0.9–1.4 kA for under 2.4 µs (port_sim) | IDM / short-pulse SOA, Rds(on) |
| Q7 (12 V buck breaker) | LM5069 power-limited start into about 60 µF, 12 A | short SOA (1–10 ms) |
| Q6 (brake clamp) | about 6 A on/off into 10 Ω | anything |

## Through-hole FET survey (datasheets read 2026-11-07; SOA read off the graphs, ±20 %)

| Part (package) | Rds max 25 °C | ×100 °C | IDM | SOA at 50 V: 100 µs / 1 ms / 10 ms / DC | Fits |
| --- | --- | --- | --- | --- | --- |
| IRFB4110 (TO-220) | 4.5 mΩ | 1.5 | 670 A | 55 / 15 / 1–2 / 1 A | Q6; too lossy for the banks (6.1 W at 30 A) |
| IRFP4468 (TO-247) | 2.6 mΩ | 1.6 | 780 A | 47 / 6 / 1.4 / 0.3 A | bank A only |
| IPP023N10N5 (TO-220) | 2.3 mΩ | 1.5 | 480 A | 28 / 2.5 / 0.7 / 0.2 A | bank A only |
| TI CSD19536KCS (TO-220) | 2.7 mΩ | 1.45 | 400 A | 60 / 13 / 4 / 2 A | bank A, Q7 |
| onsemi FDP2D3N10C (TO-220) | 2.3 mΩ | 1.4 | 888 A | 60 / 5 / 3 / – A | bank A (2.9 W at 30 A), ports |
| IXYS IXTH110N10L2 (TO-247, linear) | 18 mΩ | 1.7 | 300 A | 220 / 73 / 30 / 12 A | bank B soft start only; 27 W at 30 A |
| IXYS IXTX200N10L2 (PLUS247, linear) | 11 mΩ | 1.7 | 500 A | 350 / 125 / 51 / 21 A | bank B soft start only; 17 W at 30 A, $29–36 |

The two IXYS datasheets are marked "Advance Technical Information".

Datasheets:
[IRFB4110](https://www.infineon.com/assets/row/public/documents/24/49/infineon-irfb4110-datasheet-en.pdf),
[IRFP4468](https://www.infineon.com/assets/row/public/documents/24/49/infineon-irfp4468-datasheet-en.pdf),
[IPP023N10N5](https://web.eecs.utk.edu/~dcostine/ECE482/components/parts/PartsKit/datasheets/IPP023N10N5.pdf),
[CSD19536KCS](https://www.ti.com/lit/ds/symlink/csd19536kcs.pdf), [CSD19535KCS](https://www.ti.com/lit/ds/symlink/csd19535kcs.pdf),
[FDP2D3N10C](https://static.chipdip.ru/lib/865/DOC034865752.pdf),
[IXTH110N10L2](https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7125/PdfFile135152.pdf),
[IXTX200N10L2](https://static.chipdip.ru/lib/436/DOC011436468.pdf).

The trench parts' SOA lines fall steeply at 50 V (thermal-instability derating). That is why none of them can hold the
soft start. No through-hole part has an IDM above 888 A, against the port short's 0.9–1.4 kA peak. In a real FET the peak
is limited by the FET's own saturation current. port_sim models the FET as a resistor, so this needs a check on any
part, including the current one (open item below).

## Options

| | 0. Current: D2PAK-7 on 2 oz copper | A. Top-side-cooled SMD (TOLT) + finned heatsink on top | B. TO-220s upright with clip-on heatsinks | C. TO-220s on one bar heatsink + bus bars | **D. D2PAK-7 + finned heatsink under the board** |
| --- | --- | --- | --- | --- | --- |
| Covers the bank B soft start | yes (LF part) | **no verified part**: Infineon's 100 V TOLT parts (IPTC014/015/019N10NM5) are conduction parts with no linear-mode claim | **no** (trench) unless hybrid with IXYS L2 | **no** unless hybrid | **yes** |
| Tj at 60 A, fan air | about 90–100 °C (estimate; needs a coupon or FEM) | about 70 °C (bank A/ports only) | about 50–60 °C | about 65 °C | **about 75–80 °C** (sum below) |
| Heat leaves through | board copper and the air over it | heatsink on top (through an insulating pad: every tab is a different drain) | one heatsink per FET (each one live at drain voltage) | one bar, insulated pads and shoulder washers | one heatsink under the FET zone, through via arrays and an insulating gap pad |
| Inductance | lowest | lowest | +10–15 nH per FET | +10–15 nH per FET + bus bar (about 30 nH) | same as current |
| absorber_sim / port_sim | see below | same as 0 | bank drain into avalanche at 117–127 V; port peak 1.1 kA, 24 mJ | same as B | same as 0 |
| Board copper | 2 oz outer for current and heat | 2 oz outer | 1 oz possible only if every 60 A path is on bars | 1 oz possible with bars | **2 oz outer, 1 oz inner** (for current only) |
| Hand assembly | hot plate | hot plate; flat FET tops are critical | iron for the FETs | iron for the FETs, plus bar work | hot plate (unchanged); heatsink screwed on afterwards |
| Extra cost, 2 sets | – | about $60 (TOLT parts + heatsink) | about $40 (clip-ons; FETs about the same) | about $80 (bar, insulators, bus bars, IXYS L2 hybrid more) | **about $40** |
| Size | board only | +15–25 mm on top | +38 mm on top, wide | bar along one edge, about 150 × 40 × 30 mm | **+25 mm under the board** |

**Hot plate for option C:** still needed. The board keeps WSON, VSON and QFN parts (TPS259541, TPS3431, LM5146 on the buck),
and paste plus a hot plate is the only sane way to fit those. So through-hole FETs don't remove the hot plate.

## Option D in numbers

FET heat at 60 A (Rds at Tj 100 °C, about 1.75 × the 25 °C max):
- banks: 4 × 30² × 1.7 mΩ × 1.75 = 4 × 2.7 W;
- ports: about 2.7 W each at 30 A, about 0.3 W at a typical 10 A;
- worst total under the heatsink: about 16 W.

Path from junction to air for one bank FET:

| Step | K/W | Rise at 2.7 W |
| --- | --- | --- |
| Junction to tab (IPB017N10N5LF) | 0.4 | 1 K |
| Solder + 2 oz pad | 0.1 | 0.3 K |
| 50 × 0.3 mm vias through 1.6 mm (unfilled, 25 µm plating, about 176 K/W each) | 3.5 | 9.5 K |
| Bottom pad 15 × 15 mm into a 0.5 mm insulating gap pad at 3 W/mK | 0.75 | 2 K |
| **Junction to heatsink** | **about 4.8** | **about 13 K** |

The heatsink takes all 16 W: about 1.5 K/W with fan air → +24 K. Ambient inside the torso is taken as 40 °C. So
**Tj ≈ 40 + 24 + 13 ≈ 77 °C**. The board copper still spreads and sheds some heat in parallel, so this is the pessimistic
side. Without fan air, the heatsink (about 3 K/W natural) gives about 101 °C, still under 125 °C.

Layout changes for D (when adopted):
- 50 vias per tab, at the pad edge and under the tab. Tent them on the bottom (the gap pad is insulating). Keep them out of
  the paste aperture so they don't wick solder on the hot plate.
- Bare bottom copper (solder mask off, ENIG) under each FET, 15 × 15 mm per tab.
- Four M3 holes for the heatsink: nylon screws, or steel screws with insulating shoulder washers.
- No THT pins inside the heatsink footprint (checked: none at the current placement).

## Simulation effects

**1. Absorber loop and bank drain (applies to every option).** Earlier absorber_sim runs left out the absorber loop's own
inductance. With a real FET current fall (50–200 ns) and 8 cans, all corners:

| Absorber loop | Copper absorber → bank drain | Ceramics at the bank drain | BAT | Bank drain |
| --- | --- | --- | --- | --- |
| 2 nH | 2 nH | none | 78–88 V | 108–115 V |
| 5 nH | 2 nH | none | 88–102 V | 111–120 V |
| 10 nH | 2 nH | none | 102–111 V | 111–122 V |
| 2 nH | 2 nH | 4.4 µF | **74 V** | 82–85 V |
| 5 nH | 2 nH | 4.4 µF | 82 V | 90–92 V |
| 10 nH | 2 nH | 4.4 µF | 93 V | 99–102 V |
| 2–10 nH | 30 nH (bus bar, options B/C) | 4.4–8.8 µF | 74–76 V | 106–119 V (avalanche) |

What this means:
- The layout rule "absorber loop ≤ 10 nH" was too loose. **The absorber loop has to be about 2 nH**: D7/D21 and the cans
  in one tight cluster over solid GND. **Add about 4.4 µF of 100 V X7R (2 × 2.2 µF 1210) right at each bank A drain to GND.**
- With a bus bar between the absorber and the banks (options B and C), BAT is protected but the bank FETs avalanche on every
  bus-short trip. That needs a TVS clamp at the bank drain. It is one more reason for D.
- To do: fold this into tools/absorber_sim.py, rerun at the extracted layout values, and update the layout rules.

**2. Port short (port_sim).** Extra drain inductance between the bus caps and the port FET:

| Extra inductance | Port peak | FET energy | Bank trips |
| --- | --- | --- | --- |
| 10 nH (current) | 1.38 kA | 13.6 mJ | 0 of 48 |
| 35 nH (TO-220 legs + short bar) | 1.10 kA | 24 mJ | 0 of 48 |
| 60 nH | 0.92 kA | 27 mJ | 0 of 48 |

Lead inductance lowers the peak but raises the energy the FET takes, and coordination still holds. None of the IRF
datasheets checked states the lead inductance (LD/LS). The 35 nH figure assumes about 7.5 nH per TO-220 leg plus the bar.

**3. latch_sim:** not affected. It doesn't depend on the FET package.

## Cost, 2 sets (option D)

| Item | Each | 2 sets |
| --- | --- | --- |
| Finned aluminium heatsink, about 100 × 70 × 25 mm (extrusion cut to length) | $8–15 | $16–30 |
| Insulating gap pad, 3 W/mK, 0.5 mm, about 100 × 70 mm | $4–8 | $8–16 |
| 4 × M3 nylon screws + spacers | $2 | $4 |
| ENIG under the FET pads | already in the board quote | – |
| **Total** | | **about $30–50** |

The FETs, board and stencils don't change. The two 2.2 µF drain capacitors (option-independent) add about $0.50 per board.

## Package spec for Joey (option D)

| Item | Value |
| --- | --- |
| Board | 100 × 150 × 1.6 mm, all parts on the front |
| Tallest front-side parts | about 12.5 mm (Mini-Fit Jr headers J3/J25), 10.5 mm (cans), 9.6 mm (Micro-Fit headers) |
| Heatsink | under the back, about 70 × 105 mm footprint centred on X 0, Y 200–305 (board coordinates), 25 mm tall including the 0.5 mm pad and fins |
| Overall | about **150 × 100 × 40 mm** (12.5 front + 1.6 board + 25 heatsink) |
| Mounting | the board's 6 × M3 holes at X ±46, Y 194 / 265 / 336; the heatsink hangs off the board on its own 4 × M3 (insulated); nothing is bolted to the frame through the heatsink |
| Airflow | along the fins, parallel to the board's long side (Y). **Minimum about 1 m/s over the fins.** One 40 mm intake (5–8 CFM) ducted across a 70 × 20 mm fin section gives about 2–3 m/s |
| Fins' clear space | 20 mm free beyond the fin tips; the air inlet and outlet at the heatsink's short ends kept open |
| Cable exits | top edge (Y 340): pack 8 AWG pair (X 1 and 15), brake Mini-Fit pairs (X 29, 37, 18 AWG), 12 V converter pigtail J24 (X −41 to −19, 18 AWG). Left and right edges (X ±45): limb pigtails, 14 AWG pairs at Y 209–249. Bottom edge (Y 195): Pi GH 7-pin (X −31, 26–28 AWG), Micro-Fit 12 V outputs (X −19 to 18, 18–20 AWG) |
| Keep-outs | pack lead bend: X −5..25, Y 315–340 above the front face (8 AWG at R15, 7 mm OD). Back side outside the heatsink: THT pins up to 2.5 mm proud. 6 mm around each M3 hole |

**STEP for placement:** [layout/marengo_pdu_revb_package.step](layout/marengo_pdu_revb_package.step) (Git LFS, 42 MB; rebuilt by
tools/package_step.py). It's an assembly with:
- the board with every KiCad 3D model, plus approximate blocks for the Micro-Fit and Mini-Fit headers, which have no
  model;
- the 70 × 105 × 25 mm finned heatsink and its gap pad;
- translucent red keep-out bodies (`KO_*`) for the pack lead bend, the plugs and cable exits on every edge, and the
  heatsink's air inlet and outlet.

Coordinates are board/torso X and Y, with Z 0 at the board's bottom face.

## Open items

- **Port short peak vs IDM.** port_sim treats the FET as a resistor; a real FET limits at its saturation current. Check
  the IPB017N10N5LF's short-pulse SOA at 50 V and 1–2 µs against port_sim's peak, with the FET's transfer curve in the model.
- **Bank B bus ceiling (9.4 mF) vs the drive interface requirement (20 mF).** These don't agree. Measure the drive
  capacitance per limb (already a bring-up open item) before the board is ordered.
- **Copper:** with D, the current-only copper calc passes on 2 oz outer + 1 oz inner (L1 and L4 each carry 30 A in 10.5 mm at
  20 °C rise per IPC-2221; the two 1 oz inner GND planes return 60 A across about 100 mm). That is JLC's standard stackup,
  not the JLC3313 2/2 oz one.
