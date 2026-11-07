# Power system: production run (rev B5)

2026-11-07. Joey: "single board" means one production run of every board together, at JLC with factory assembly; the first
5 boards are prototypes, and a large review of the routed boards comes before ordering. This page lists the boards, the
shared stackup, how to order them, and what it costs at 5 and 10. Prices are estimates until the JLC quote: get the quote
during BOM mapping, before the review.

## 1. Boards in the run

| Board | Status | Size | Stackup | Fitted parts | Assembly |
| --- | --- | --- | --- | --- | --- |
| **Main power board** ([marengo_pdu_revb](../schematics/marengo_pdu_revb)) | schematic done, ERC clean, placed; routing next | 100 × 150 mm (carrier) | 4 layers, 2 oz on all four, 1.6 mm | 138 lines, 268 parts | JLC SMT, top side only; about 12 parts finished by hand (below) |
| **12 V buck** ([marengo_buck12](../schematics/marengo_buck12), LM5146, 12 A) | schematic done, ERC clean; layout not started | about 45 × 60 mm (estimate) | same as the main board | 33 lines, 43 parts | JLC SMT, top side only; J1 (Micro-Fit) by hand |

Not in the run:

| Board | Why |
| --- | --- |
| DVE adapter | Fallback only, if the buck fails bring-up. The Murata DVE plugs into the same J24 pigtail; its adapter can be a quick 2-layer board then. |
| Pack sense board | Dropped: the BMS is on the pack (JBD SP17S005, [pack plan section 3](../power/pack-build-plan.md)). |
| Logic board (rev A "LP") | Dropped in rev B: the latch, watchdog, meters and E-stop logic are on the main board, and the Pi connects straight to J21. |
| Arm junction boards | Ruled out (Joey, 2026-11-06): spliced trunk with inline fuses. |
| Rev A boards (marengo_hp / marengo_lp) | Superseded. |

So the run is **two designs: the main board and the buck.**

## 2. Common stackup

**JLC 4-layer, 1.6 mm, JLC3313 stackup, 2 oz outer and 2 oz inner.** JLC offers 2 oz inner copper only on 4-layer, 1.6 mm,
JLC3313, with 2 oz outer ([JLC copper weight FAQ](https://jlcpcb.com/help/article/jlcpcb-copper-weight)). That is exactly
what [LAYOUT_RULES_REVB.md](design/LAYOUT_RULES_REVB.md) asks for, so both boards use it:
- the main board needs it for 60 A;
- the LM5146 buck at 12 A benefits from it (TI's EVM is 4-layer, 2 oz) and gets a solid GND layer under the hot loop.

Rules that follow from 2 oz:
- Design for JLC's 2 oz minimum trace and space (larger than 1 oz; check the current JLC capability page when setting up DRC).
- The 0.15 mm gaps inside the U2 and U7 footprints need a check against that minimum; expect a DFM comment.
- Finish: ENIG (flat pads for the QFN, TSSOP and fine-pitch parts). Solder mask either colour. Lead-free.
- Same board thickness and copper on both, so one JLC engineering review covers both.

## 3. One order: two designs, or one panel?

| | Two designs in one order (recommended) | One panel: main + buck, tab-routed |
| --- | --- | --- |
| Fabrication | two PCB jobs, same stackup | one PCB job, about 100 × 215 mm panel |
| Assembly setup | two setups and two stencils (about $32 each) | one setup and one stencil |
| Extended-part loading fee | charged per job, so parts the boards share are paid twice (about 5 shared: SK810L, Micro-Fit, some MLCCs, about $15) | paid once |
| Re-spins | each board revs by itself. The buck's loop is only measured at bring-up, so it is the likelier re-spin | a buck fix re-orders the main board too, or splits the panel |
| Quantities | can differ (e.g. 5 main, 10 buck) | always equal |
| JLC handling | standard | customer panel with different designs: allowed, but extra review and a possible fee; confirm on the quote |
| Saving | | about $35–60 per order |

**Recommendation: two designs, one order, one shipment.** That's the "one production run". It costs about $35–60 more
than a panel, but each board can be revised alone, which matters most for the buck.

## 4. Shared BOM

Both boards draw from one parts list where they can, to keep JLC loading fees down. The plan is to map passives to JLC basic
parts wherever the value and rating allow:
- **Shared now:** SK810L-TP (both inputs), Micro-Fit 3.0 43045-1212 (J24 / buck J1), MMBT3904, 0603/0805 resistors and
  X7R MLCCs.
- **To unify in BOM mapping:**
  - the 2.2 µF 100 V and 1 µF 100 V MLCCs (one part number each, used on both boards);
  - the 100 nF 100 V soft-termination part;
  - BZT52 zeners.
- **Main-board extended parts** (about 45 unique: ICs, FETs, shunts, hybrid caps, connectors) set the loading fee, about
  $3 each per job.
- **Parts JLC may not stock** (check during mapping; else JLC global sourcing or consigned parts, which adds lead time):
  TPS48111-Q1, LM5069, IPB017N10N5, EEH-ZC1K470P, WSK2512 shunts, INA237, TPS3431, LM5146, XAL1010 and XAL7030 inductors,
  BSC070N10NS5 / BSC040N10NS5.

**Hand-finished on the main board** (about 1 h per board):
- J1 8 AWG pack pair;
- J8–J10 14 AWG limb pigtails;
- J3/J25 Mini-Fit and J5–J7, J22–J24 Micro-Fit headers;
- the F2 blade-fuse holder;
- cable ties through the tie holes.

JLC can place the through-hole headers for an extra fee, but the pigtails are hand work anyway.

## 5. Cost, 5 vs 10 (estimates; replace with the JLC quote)

| Item | Qty 5 | Qty 10 | Basis |
| --- | --- | --- | --- |
| Main PCB, 4 layers 2 oz all, 100 × 150, ENIG | $70–100 | $100–140 | third-party 2 oz 4-layer prices scaled by area |
| Buck PCB, same stackup, about 45 × 60 | $25–35 | $35–45 | |
| Assembly setup + stencil, two jobs | $65 | $65 | about $25 setup + $7 stencil per job |
| Extended-part loading, two jobs | about $180 | about $180 | about 45 main + 15 buck unique parts at $3 |
| Main board parts | 5 × $120 = $600 | 10 × $105 = $1050 | REVB5_CHANGES cost pass (about $100–105 after mapping at 5) |
| Buck parts | 5 × $15 = $75 | 10 × $13 = $130 | bom-buck12 |
| Placement (joints) | about $15 | about $30 | |
| **Total before shipping and tariff** | **about $1030–1080** | **about $1590–1640** | |
| Per main + buck set | about $210 | about $160 | |

Shipping (DHL) and any US import duty come on top. They are not estimated here: check them at checkout.

- The one-off costs (setup, stencils, loading fees, about $245) are the same at 5 or 10. That's why qty 10 is about $50 per
  set cheaper.
- Qty 10 also gets IC price breaks.
- JLC can assemble fewer boards than it fabricates (minimum 2). One option is to fabricate 5 and assemble 3, which saves
  about $270 of parts. The two bare boards can be assembled later by hand or in a second job.

**Recommendation: qty 5 of each, all assembled.** Joey called the first 5 prototypes, and the bring-up gates
(REV_B_PARTS steps 1–13) can still change parts. Order 10 after a prototype passes bring-up and the review findings are in.

## 6. Order checklist

1. Main board routed, DRC clean, buck board laid out and DRC clean.
2. BOM mapped to JLC part numbers (basic first), stock checked, consigned or global-sourced parts listed with lead times.
3. JLC quote for both designs (stackup JLC3313, 2/2 oz, ENIG). Update section 5.
4. Big review on the routed boards (Fable plus several Opus, per Joey). Fix everything blocking.
5. Gerbers, drill files, BOM and CPL exported from KiCad, rotation offsets checked in JLC's preview, and polarity of every
   diode, can and IC checked in the 3D preview.
6. Order, then buy the off-board parts (pack plan, REVB5_CHANGES off-board table) so they arrive with the boards.
