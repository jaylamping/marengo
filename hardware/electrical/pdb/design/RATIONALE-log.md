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
The 40 mV shunt offset between DSG_OUT and VSYS is negligible for LD.

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
