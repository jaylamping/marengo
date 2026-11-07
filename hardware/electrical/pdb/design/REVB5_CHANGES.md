# Power board rev B5: what changed after the Opus round 5 review

2026-11-07. Joey approved, at 15:49Z:
- every blocker, major and high finding, plus every non-minor recommendation, in [opus-review-r5/CHANGE_LIST.md](opus-review-r5/CHANGE_LIST.md);
- the recommended cost package;
- three additions he raised in the thread: per-limb breakers, a hardware heartbeat watchdog and a keyed battery connector;
- the rule that the board stays single-sided.

Schematic: [marengo_pdu_revb](../../schematics/marengo_pdu_revb) (gen/gen.py), plus the new buck board [marengo_buck12](../../schematics/marengo_buck12).

**Gates after every stage:**
- ERC: 0 errors.
- check.py: 125 assertions, 0 fails.
- latch_sim: 0 fails (now including the watchdog and the port-breaker load on RUN).
- absorber_sim: worst corner 91 V (unchanged).
- port_sim: 0 of 48 cases reach a bank threshold.

## Rating

The board is now rated **60 A continuous, under 90 A peak.** Two reviewers independently found that the U1 latch, the FET copper, the 60 A pack fuse and the terminals were all 60 A-class parts, so "leg-ready 80 A / 150 A" was never real.

Legs move to rev C. Their port footprints stay on the board, unfitted.

## Safety (in Joey's priority order)

| Item | What changed | Check |
| --- | --- | --- |
| Reverse polarity | J1/J2 M4 studs replaced by one soldered 8 AWG pair, BAT and GND side by side, ending in an XT90 that only mates the pack's XT90S one way. 8 AWG (Joey: the heaviest run in the robot). The AS150U was tried and doesn't fit the pack corner; XT90 is 45 A continuous, fine for arms and waist | check.py: BAT on one two-pole connector |
| Limb short drops the whole bus | Every port gets its own breaker: TPS48111-Q1 plus an IPB017. Short trip at 35–45 A within 1.6 µs, overload at 30 A for about 16 ms, latch-off cleared by an E-stop cycle, enabled from RUN. Each limb's + and return now leave on one soldered pair (no GND studs). The 20 A pigtail fuses stay as wire protection | **tools/port_sim.py:** 50 nH to 1 µH wiring, 0.5–1 µH pack lead, 0 or 50 A on the other limbs. RS1 peaks at 130 A or less, against U8 158 A and U1 160 A minimum: the faulted port always opens first |
| Crashed Pi leaves motors live | TPS3431 watchdog in series with the latch's seal-in NPN. The Pi toggles WD_FEED every 50–100 ms; with no falling edge for 270–355 ms the latch drops and stays off until reset. Powered from VCTL, not the Pi, so a dead Pi, a floating pin or a dead watchdog rail all drop the latch. It can only drop the latch, never hold it | **latch_sim:** missed heartbeat drops it, it stays off after WDO releases, and reset is blocked while the watchdog is low (all hFE corners) |

**Port FET stress.** A dead short right at the board pads puts about 1.1–1.4 kA through the port FET for under 2 µs (5–14 mJ). That's why the port uses the bank FET (1024 A pulse, 979 mJ avalanche). A short in the harness (200 nH or more) is 400–540 A.

**Reverse polarity, battery-side branches checked.**
- Nothing on the main path blocks a reversed pack: both banks' body diodes conduct and D1 becomes the crowbar.
- U2's feed is blocked by D8. U8 VS is rated −65 V. The absorber is isolated by D7 (and its new twin). The buck input has its own SK810L crowbar behind U7.
- U1 and U7 VIN, the BAT MLCCs, the INA VBUS pins and the drives would see about −5 V while D1 conducts.
- **The Class T fuse does not clear before D1 fails.** At 500–1000 A it is in its time-current region (tens of ms), and D1's surge rating is about 374 A²s. A TVS normally fails short, which keeps it crowbarring until the BMS's short-circuit trip (JBD 200–800 µs, Daly 200–500 µs, against D1's 0.4–1.6 ms: typical units clear in time, worst case does not) or the Class T opens.
- So the keyed plug plus the bring-up polarity check carry this, as approved.
- **Back-to-back banks (bank A flipped to block reverse) were evaluated and left out.** With the banks open, regen could no longer return to the pack and would strand on the bus, which changes how the E-stop behaves. That makes it Joey's decision, not a clean fix.

## Cost package

| Item | What changed |
| --- | --- |
| 12 V converter | **Own 12 A buck on a separate small board:** LM5146, BSC070/BSC040, XAL1010 6.8 µH, 400 kHz, starting from TI's 48 V → 12 V application circuit 2. It bolts to the -X rail where the DVE sat and plugs into the same J24 pigtail. **The DVE0125V2NBC and its adapter stay the drop-in fallback.** Kept off the main board on purpose: its 4–6 W goes into the frame rail instead of a board that already carries 11 W of FET heat; it keeps switching noise away from the meters and the latch; and the main board has no pour budget left (layout review finding 2). The input runs through an LC filter: tools/absorber_sim.py now follows the bank-trip spike through U7, F2, the pigtail and the filter, and buck VIN peaks at 86 V (100 V parts). UVLO is 30.2 V on and 27.2 V off, below the 30.5 V worst sag |
| Meters | INA228 → INA237: same pinout, addresses and 85 V rating. 16-bit, no energy accumulator, so the Pi integrates |
| Brake leads | M4 terminals → one Mini-Fit Jr 2-pin header per resistor (J3, J25). A different connector family from every 12 V output, so no 12 V load can mate a 57 V brake header |
| Limb outputs and returns | Soldered 14 AWG pigtail pairs with strain-relief holes (KiCad SolderWire-2.5sqmm Relief) |
| Aux port | Unfitted, like the legs |
| Shunts | CSS4J "contact factory" 5 mΩ parts and the 2 mΩ → Vishay WSK2512 4-terminal; limb shunts 1.5 mΩ WSK2512 (sized for the breaker) |
| Bus caps | 4 × APZ 68 µF (obsolete) → 5 × Panasonic EEH-ZC1K470P, 47 µF 80 V, 235 µF, active line |
| Sourcing | LCSC for commodity parts with the PCB order; Sager, Heilind and Arrow/TTI for the off-board parts (see the cost section) |
| Kept on purpose | Both E-stop banks, both FETs per bank, the latch, the clamp, the absorber, the Class T fuse, 4 layers |

## Other majors, highs and recommendations

| Finding | Fix |
| --- | --- |
| Compute brown-out under motor load (M-2) | Buck UVLO 27.2 V off. Software: load-aware current budget and shutdown trigger (REV_B_PARTS) |
| Brake-energy cap can't be computed (M-3) | CL_OUT through 100k/1k and 4.7 µF into U17 IN+: 120 mV at 100 % duty. The Pi reads the duty |
| One thermostat for two resistors (F4) | Second 67L120 on the −X plate, in series in the loop (off-board) |
| APZ caps obsolete (HIGH) | Bus caps changed (above). **Absorber, Joey 16:46Z: the APZ is dropped.** Now 8 × EEH-ZC1K470P (47 µF 80 V hybrid, the same part as the bus caps: one line item, 13 per board, with Chemi-Con and Würth 80 V hybrids as second sources, JLC-placeable). absorber_sim, all corners: **75.0 V worst** with 8 cans (6 cans 79.1 V was too thin; APZ baseline 91.3 V): 6 % under the 80 V rating, about 18 % under the surge rating (Panasonic's usual 1.15 ×, to confirm on the ZC spec sheet). A pulse loop under 0.05 Ohm raises it (0.03 Ohm: about 83 V), so bring-up step 1 is a gate. A faster bank A (TPS48111) is being evaluated to get under 72 V. Rejected: 100 V MLCCs (120 × 1210 still 92.5 V once DC bias is counted), 100 V aluminium (8 × 12.5 mm cans to reach 75.5 V, ESR unverified), and a TVS-only absorber (hundreds of amps is far beyond any TVS's clamp). Cost about $9 against $11; area about +5 cm², placed in a 2 × 4 block right of the pack keep-out. The BMS close current rises to 325–363 A (from about 300 A): bench step 2 still decides 10b |
| U18 EN overvoltage on back-feed | R42 68k → 33k: EN is 4.96 V at 20 V |
| D18 order code doesn't exist | Nexperia BZT52H-C68,115 (SOD-123F) |
| Brake cap needs a bench number | New bring-up step: 50 W total for 20 min with the real gussets; the software cap is set at 80 % of the power that holds the plate at 110 °C |
| Two self-test windows | One window, applied to gain-corrected MID: 0.33–0.62 × pack, pack at 40 V or more |
| No fault decode table | Added to REV_B_PARTS |
| D7's resistance unswept | absorber_sim sweeps 10/20/30 mΩ: 30 mΩ reaches 94 V. A second SK810L in parallel keeps the worst corner at 91 V |
| Clamp threshold text | 56.6–56.8 V on, 55.8–55.9 V off (typ) |
| Minor, done anyway because they were cheap | Soft-termination MLCCs on BAT; SPARE_SW keyed on a 2 × 2 header; voltage and dielectric on C7, C8, C37, C42 (C42 now 1 µF 50 V 0805); R48 and R58 → 0805, R10 → 0603; 1 µF 100 V at Q6/D4; GND test points and pads on GATE, GATE_A, UVLO, FLT_A; the Pi header re-pinned with GND between SDA and SCL |
| Minor, not done | Q7 → BSC040N10NS5 (its SOA at U7's power limit can't be read from the PDF here); U2 thermal land enlargement (layout task); 0.1 % note fixes in old review files |

## Layout

The layout findings are written into [LAYOUT_RULES_REVB.md](LAYOUT_RULES_REVB.md), and tools/place.py follows them:
- single-sided;
- 2 oz on all four layers, L2 solid GND;
- BAT pair at the top edge;
- flow order RS1 → bank A → bank B → VBUS;
- absorber cluster of 25 mm or less and 10 nH or less;
- ESTOP_SAFE net class with a guard;
- limb pairs together;
- an exhaust lane past the outputs.

## Assembly: JLC SMT assembly (flagged for Joey)

The board has leadless parts:
- TPS259541 WSON 2 × 2;
- TPS3431 VSON;
- the TPS7A4001 PowerPAD;
- about 60 parts in 0402.

It also has D2PAK-7s on 20 cm² pours with 30 vias each. An iron can't do those. **Plan: JLC (or PCBWay) assembles the SMT side; the Micro-Fit, Mini-Fit, GH and fuse holder, plus the pigtails, are hand-soldered.**
- JLC charges a setup fee and about $3 per unique "extended" part per order, so the 5-board order carries roughly $100–150 of assembly fees.
- TPS48100 and TPS48111 may need JLC global sourcing or consignment.
- The alternative is a stencil plus a hot plate at home: about $10, but it's a real risk on the 2 × 2 mm WSON parts.

## Cost (estimates, ±25 %; run the carts before ordering)

**Lean number first.** One board, LCSC/JLC prices at a 5-board order, no assembly fee: **about $120 of parts (about $100-105 after the second cost pass's BOM mapping) plus about $20 of PCB.** With JLC SMT assembly, about $175–185 per board. The buck board adds about $20.

| Main board group | Estimate |
| --- | --- |
| 8 × IPB017N10N5LF (4 main path, Q7, 3 ports) | $34 |
| Controllers: TPS48100, 3 × TPS48111, 2 × LM5069, TPS7A4001, TLV1805, LM4040, 2 × TPS259541, TPS3431 | $25 |
| 6 × INA237 | $9 |
| Shunts: CSS4J 0.5 mΩ, 6 × WSK2512 | $12 |
| Caps: 13 × EEH-ZC (8 absorber, 5 bus), EEH-ZA 330 µF, MLCCs | $24 |
| Diodes, small transistors | $5 |
| Connectors, fuse holder and fuses, test points | $17 |
| Small resistors | $3 |
| **Board parts** | **about $130** |

| Off-board | Estimate |
| --- | --- |
| Buck board parts + 2-layer PCB share | $20 (replaces DVE + adapter, $100) |
| 2 × HS50 20R (Arrow/TTI $6.11 each) | $12 |
| 2 × 67L120 thermostats (Heilind $5.57 each) | $11 |
| 3 × limb fuses + holders (aux and legs unfitted) | $13 |
| Class T 60 A + holder (fuse from Sager) | $40 |
| XT90 pair, 8 AWG, Mini-Fit / Micro-Fit housings and crimps, GH leads, wire | $29 |
| Smart 13S 60 A BMS (JBD; Daly alternate) + UART/RS485 lead (was JK + module, $100) | $50 |
| HB2-BS545 E-stop | $15 |
| **Off-board** | **about $190** |

**Whole power system, excluding cells: about $345 lean (hand assembly), about $375 with JLC SMT assembly** (with the JBD BMS; $50 more with the JK). That's down from about $575–595 at rev B4. The protection added in this round is in that number: three port breakers about $25 and the watchdog about $2. The Class T fuse and holder are now the biggest single off-board line.

## Second cost pass (Joey 16:53Z: target 100 dollars of main-board parts, no weaker protection)

**Lean total now: about 120 dollars of main-board parts at a 5-board LCSC/JLC order** (estimates; LCSC prices can't be read automatically, run the cart). Not under 100 without touching protection.

| Candidate | Saving per board | Verdict |
| --- | --- | --- |
| Test points: Keystone loops to bare pads (3 GND loops kept) | about 2 | **Applied** (13653e87) |
| Shunts: Vishay WSK2512 to a commodity 4-terminal 2512 from LCSC (same land) | about 5 | Do at BOM mapping; the land doesn't change |
| JLC basic parts: move resistor and MLCC values onto JLC's basic list (no 3-dollar extended fee per unique part) | about 10-20 at qty 5 (about 50 unique extended lines today) | Do at BOM mapping; values stay in their tolerance |
| Port FETs IPB017 (4.30) to a BSC040-class TDSON (about 1.30) | about 9 | **Not applied.** port_sim puts 1.1 kA through the FET for a short at the board pads; BSC040's pulse rating is well under that. That's the protection getting weaker |
| Drop the per-port INA237s and read the TPS48111 IMON pins with one ADC | 0 or worse | **Not applied.** The shunt stays (the breaker needs it), an 8-channel ADC costs about what three INA237s do, and the port-output voltage that names a tripped port goes away |
| Bank A FETs to a non-linear-rated 1.7-2 mOhm D2PAK-7 (bank A never runs linear) | about 2 | For the big review: needs the SOA read for the interrupt event |
| Q7 IPB017 to a TDSON part | about 3 | Not applied: its SOA at U7's 70 W limit can't be read here |
| TPS48100 to a cheaper driver | - | Not applied: it carries the close-order interlock (FLT) and bank A's short-circuit trip |
| Order 10 boards instead of 5 | about 10-15 of parts per board, and the PCB and fee share halves | Joey's call (see PRODUCTION_RUN.md) |

With the shunt and basic-part mapping the board lands at **about 100-105 dollars of parts**. Under 100 needs a 10-board order or a port-FET downgrade.

**Faster bank A (TPS48111 in place of the TPS48100), evaluated, not adopted.** It would cut the interrupted current: absorber_sim with the bank opening in about 4.5 us gives 67.7 V worst with 8 cans (70.4 V with 6), against 73.7-78 V at today's 8.8-13 us. But the TPS48111's FLT_I only reports over-current; it doesn't assert while its charge pump is still coming up. Today U8's FLT holds U1 off until bank A can actually close ("A closes first by logic, not by timing"). Adopting the TPS48111 would need a new close-order interlock. That's an E-stop logic change, so it goes to the big review with the numbers above, and the board keeps the TPS48100 and 8 cans.

## Decisions that are Joey's

1. **Pack connector: decided.** XT90S / XT90 on 8 AWG (the AS150U doesn't fit the pack corner). **Open for legs (rev C):** pack-corner redesign for an AS150.
2. **Back-to-back banks: decided, no** (Joey, 16:45Z). Banks stay as they are; the keyed plug carries reverse polarity.
3. **Assembly route: decided, JLC SMT assembly** (Joey, 16:45Z), with a big review (Fable plus several Opus) on the routed board before ordering, and the first 5 boards treated as prototypes.
4. **APZ: dropped** (Joey, 16:46Z). The absorber is 8 × EEH-ZC1K470P, the bus-cap part.

## Not resolved here

- **Buck control loop.** TI's compensation for this L and C. The pigtail plus the 330 µF on the main board resonate near 20 kHz, so measure the loop at bring-up (a 0R injection resistor is fitted for it).
- **Unverified part numbers:**
  - WSK2512 1.5 mΩ / 2 mΩ / 5 mΩ codes (the footprint is right; confirm the exact MPN and value in stock);
  - TPS48111AQDGXRQ1 orderable suffix;
  - XAL7030-222MEB.
- **BMS on the unit (JBD 13S 60 A):** short-circuit delay ≥ 200 µs (absorber close), DOC settable to 70 A or more (else the 80 A version), size against the rear-face envelope.
- **Drive capacitance per limb.** It sets the port start current; 20 mF total draws about 20 A, under the 30 A overload level.
