# Power board rev B4 and power system: cost-down review

2026-11-07. Reviewer: cost-reduction pass (Opus). The repo is read-only; nothing was edited.

**Inputs read:**
- `pdb/design/PRE_LAYOUT_AUDIT.md` (cost section), `bom-rev-B.csv`, `bom-digikey-upload.csv`
- `pdb/REV_B_PARTS.md`, `REV_B_CLEAN_SHEET.md`, `PLAN.md`
- `pdb/design/README.md`, `RATIONALE-log.md` (rev B sections)
- `power/pack-build-plan.md`, `wiring/left-arm-trunk/BUY_LIST.md`
- the schematic text notes for the shunt, meter and converter roles

**Joey's guidance during the review (15:19–15:22Z):**
- Don't cheap out to save a dollar, and don't use sketchy parts.
- Go after the structural cost drivers: premium or over-specified parts, too many terminals and meters, leg-rated parts not needed yet, and an expensive brick.
- Evaluate an on-board 12 V buck.
- Evaluate USB-C PD rails.
- The Jetson stays on a barrel jack.

**Price rules.** Every $ figure carries one of three labels:
- a cited distributor price (URL, seen 2026-11-07);
- **(repo)**: the audit's own ±25 % estimate;
- **(est)**: my estimate, with the method stated.

Octopart and LCSC snapshots aren't quotes. Run the DigiKey/LCSC carts before ordering.

---

## 1. Where the money actually is

The baseline is the audit's estimate:
- board parts about $215;
- PCB $15–35 per board;
- off-board about $345 with the BMS and E-stop;
- about $575–595 in all.

Prices checked today against the audit's estimate:

| Item | Audit (repo) | Checked today | Comment |
| --- | --- | --- | --- |
| Würth REDCUBE 74651194R M4 terminal | $46 for 13 | **$3.44 each** at DigiKey/Mouser ([Octopart](https://octopart.com/part/wurth-elektronik/74651194R)) | Only **12** are fitted (J12/J13 are DNP). The upload CSV says 13, so it is $41 really. **Largest board line, and 4 of the 12 carry 2.8 A brake-resistor leads.** |
| INA228AIDGSR | $39 for 7 (≈ $5.57) | LCSC $3.93 ([LCSC C2887910](https://www.lcsc.com/product-detail/C2887910.html)), TME $3.95 ([Findchips](https://findchips.com/search/INA228AIDGSR)) | DigiKey qty-1 not readable. The audit's $5.57 is plausible for DigiKey qty 1 |
| INA237AIDGSR (85 V, 16-bit, same VSSOP-10 family) | n/a | **DigiKey $3.23 qty 1, $2.42 qty 10** ([Octopart](https://octopart.com/part/texas-instruments/INA237AIDGSR)); LCSC from $1.07 ([LCSC C2864837](https://www.lcsc.com/product-detail/C2864837.html)) | The cheap 85 V option |
| INA226AIDGSR | n/a | LCSC $0.69 ([LCSC C49851](https://www.lcsc.com/product-image/C49851.html)) | **Not usable**: 36 V bus/common-mode limit, below the 54.6 V bus. INA219 (26 V) is out for the same reason |
| Adafruit INA228 module | n/a | $14.95 ([Adafruit 5832](https://adafruit.com/product/5832)) | Costs more than the chip, carries a 15 mΩ shunt (10 A max), and is the wrong form factor. No |
| IPB017N10N5LFATMA1 | $28 for 5 | LCSC $5.04 qty 1, $4.30 qty 10 ([LCSC C3289286](https://www.lcsc.com/product-image/C3289286.html)) | Q7 is a 180 A FET on a 4 A path |
| BSC040N10NS5 | n/a | Newark $3.22 qty 1 ([Octopart](https://octopart.com/search?q=BSC040N10NS5ATMA1)) | Already on the BOM (Q6) |
| TPS48100QDGXRQ1 | in the $24 | $4.50 DigiKey/Mouser ([Octopart](https://octopart.com/part/texas-instruments/TPS48100QDGXRQ1)) | Safety core, keep |
| APZ1012470M100R | $3.60 | Mouser $3.75 (1,235 in stock), **DigiKey $4.21, 6,799 in stock** ([Octopart](https://octopart.com/search?q=APZ1012470M100R)) | The audit's "0 at DigiKey" blocker looks resolved on this snapshot: recheck |
| Murata DVE0125V2NBC | $85 (DigiKey, Joey) | **Sager $70.99 (146 in stock)**, Mouser $82.80, Newark $99.39 ([Octopart](https://octopart.com/part/murata-power-solutions/DVE0125V2NBC)) | 25 A brick for an 11.6 A ceiling load |
| HS50 20R J (2×) | $20 | $6.11 each at Arrow/TTI ([Octopart](https://octopart.com/search?q=HS50+20R+J)) | The listing shows the HS50 20R J under Ohmite's name. Confirm the brand on the page before ordering |
| Sensata 67L120 | $10 | **$5.57 at Heilind (2,035 in stock) and ICC (678 in stock)** ([Octopart](https://octopart.com/search?q=67L120)) | Also fixes the audit's 0-stock flag. The 67L120P shows 0 at Heilind/ICC |
| Class T 60 A (JLLN060.T) | $40 with holder | Fuse alone $25.76 (Sager) to $43.69 ([Octopart](https://octopart.com/part/littelfuse/JLLN060.T)) | Holder price not checked |

**Structural cost drivers, in order:**
1. **The 12 V brick plus its adapter, about $100.** It is 25 A for a load the docs put at 9.1 A expected and 11.6 A ceiling.
2. **Terminals, about $41.**
3. **Meters plus shunts, about $53.**
4. **The leg-ready main path:** 2 FETs per bank, and a 4-layer 2 oz PCB.
5. **Qty-1 DigiKey pricing on commodity parts, about $31.**

The safety core is about $90–100 of parts:
- two FET banks with TPS48100 and LM5069;
- latch, clamp, absorber and bus caps.

That core is not where the fat is.

---

## 2. Ideas, ranked by $ saved per unit of risk

Risk scale: 0 = none, 1 = negligible (sourcing or BOM swap), 2 = small (a recheck or a feature lost), 3 = real design risk, 4–5 = touches the safety function.

| Rank | Idea | Saving | Risk | $/risk | What is lost or added | Effort |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **Buy from the cheaper authorized source:** DVE from Sager, 67L120 from Heilind, HS50 from Arrow/TTI, Class T from Sager | **≈ $26** (DVE −$14, thermostat −$4.4, HS50 −$7.8). Class T likely −$5 to −$14 vs repo | 0 | ∞ | Nothing | Minutes |
| 2 | **Brake-resistor leads (J3/J4/J25/J26) move from 4 REDCUBE terminals to 2 Micro-Fit 2-pin headers** (43045-0212, already on the BOM, $0.40–0.73: [Octopart](https://octopart.com/search?q=43045-0212)) | **≈ $12** board, plus ring lugs off-board, plus about 4 footprints of area | 1 | 12 | Nothing real. The leads carry 2.84 A peak per resistor, and Micro-Fit is a positive-latch connector, so it holds better than a screw under vibration | Footprint swap; crimp 18 AWG |
| 3 | **Commodity parts from LCSC with the PCB order** (MLCC, R, diodes, small BJTs, 1206 fuses, JST/Micro-Fit) | **≈ $15–20 (est)**: $31 (repo) at DigiKey qty-1 rates to about $10–14 at LCSC basic-part rates, including the 10–100 MOQ on passives | 1 | 15–20 | Nothing. Stay with known brands (Yageo, Samsung, Murata, Diodes, onsemi) | BOM mapping, an hour or two |
| 4 | **INA228 → INA237** on every meter: same 85 V family, VSSOP-10, same pinout and I²C interface | **≈ $16** for 7 ($5.57 → $3.23) | 1 | 16 | Lost: the 20-bit ADC and the hardware ENERGY/CHARGE accumulators (the Pi integrates instead), and some offset accuracy. At 1 mΩ, 16 bits gives 1.25–5 mA LSB, fine for limb amps. Verify INA237's offset and gain error in its datasheet; tens of mA at 1 mΩ is fine for telemetry | Driver register widths change. Schematic: value swap only |
| 5 | **Defer the aux port**: J11 terminal, RS5, U13 → DNP like the legs, plus one fewer pigtail fuse and holder | **≈ $12** ($3.44 + $1.75 + $3.23 + about $4 fuse/holder, est) | 1 | 12 | Nothing today: no aux load is named in the docs. Populate later, with no re-spin | DNP flag |
| 6 | **Defer leg current: fit 1 FET per bank** (Q2 and Q9 DNP, footprints kept) | **≈ $10–11** | 2 | 5 | **Main path becomes 30 A continuous / 60 A short bursts** instead of 60 A: one hot FET is 2.7 W at 30 A, the same per-FET loss as today's pair at 60 A. Recheck U1's start SOA (it should already assume one FET, since paralleled FETs don't share in linear mode) and U8's 158–242 A trip against one FET's pulse SOA. Lower the software bus cap to about 30 A continuous. The legs repopulate 2 parts | DNP + an SOA recheck |
| 7 | **Q7 (converter hot-swap) IPB017N10N5LF → BSC040N10NS5** (already on the BOM) | ≈ $2.4 | 1 | 2.4 | Recheck LM5069-2 start SOA at its power limit against the TDSON SOA. Pennies, but it is the clearest over-specified premium part on the board | Value/footprint swap. Skip if layout is frozen |
| 8 | **Delta E48SC12020NRFA instead of the DVE.** Same 1/8-brick class: 36–75 V, **100 V for 100 ms transient**, regulated ±48 mV, 94.5 %, negative logic by default ([E48SC datasheet](https://www.mouser.com/datasheet/2/632/DS_E48SC12030-611308.pdf)) | **≈ $29 vs Sager DVE, $43 vs DigiKey DVE**: $41.53 at Master/Onlinecomponents, **0 stock there**; brokers $66–72 ([Octopart](https://octopart.com/search?q=E48SC12020NRFA)) | 2 | 15–20 | Same function, 20 A (1.7× the ceiling). **Stock risk.** The adapter needs a re-pin check: sense/trim pins, and no baseplate unless the -H option. Not the E48SC12010NRFA (10 A, $39.80 DigiKey): below the 11.6 A ceiling | Adapter footprint check |
| 9 | **Limb + outputs and the two GND studs: solder-in pigtails instead of REDCUBE** (J8–J10, J14, J15). Keep J1/J2 for the 10 AWG battery leads | **≈ $21** board + about $5 ring lugs (est) | 2 | 13 | Serviceability: swapping a pigtail means desoldering 16 AWG from a 2 oz pour, so you need a big iron. Needs a strain-relief tie-down hole beside each exit, and an adhesive or heat-shrink boot against vibration fatigue. XT30/XT60 PCB connectors (XT60PW-M $0.61, [LCSC C98732](https://www.lcsc.com/product-image/C98732.html)) would be cheaper still, but rev A's frame check found no plug room at the board edges, so pigtails it is | Footprint swap; layout not started |
| 10 | **Drop the 12 V meter (U14, RS8) and the SPARE_SW meter (RS10)** | ≈ $10 at INA237 prices ($6.5 + $3.5 shunts) | 2 | 5 | No board-level 12 V power telemetry. Compute power comes from Jetson tegrastats and the Pi. The STS3215 servos report their own load over their bus, and U9 still limits in hardware. Against Joey's "one meter on 12 V" (clean sheet row 16): his call | DNP |
| 11 | **Own 12 A synchronous buck instead of the DVE + adapter** (section 3) | **≈ $70** ($100 → about $25–30, est) | 3 | 23 | Design and bring-up risk: layout, compensation, transient survival. Fallback: the DVE/Delta on the same J24 pigtail | 1–2 weekends + 1 PCB spin + 1 bring-up day |
| 12 | **2-layer 2 oz PCB instead of 4-layer 2 oz.** Only together with idea 6 (30 A main path) | **≈ $30–60 per order (est)**: the repo's 4L estimate is $60–120 per order; a 2L 2 oz 100 × 150 ×5 should be about half (JLC doesn't publish 2 oz prices: [JLC copper weight](https://jlcpcb.com/help/article/jlcpcb-copper-weight)). About $6–12 per board share | 3 | 10–20 | The audit's thermal plan (60 A, pours on all 4 layers, 30 vias per FET) no longer applies; you re-do it for 30 A. No inner GND plane under the INA/latch analog and the absorber loop: use a solid bottom pour. **Legs then need a re-spin**, but at JLC prices a re-spin is cheap, and bring-up will likely force a rev C anyway | Layout rule change before routing starts |
| 13 | Pack fuse: Littelfuse MIDI 58 V 60 A (142.5631.5602, $5.15 at Newark: [Octopart](https://octopart.com/search?q=142.5631.5602)) instead of Class T | ≈ $25–30 | **4** | 7 | **Not recommended.** About 1 kA interrupting rating against an estimated ~550–600 A prospective pack fault (54.6 V / ~90–100 mΩ). Thin margin on the last-line protection, and a lower-resistance pack would exceed it. Class T is 20 kA | |
| 14 | Remove bank A (single E-stop bank) | ≈ $25 | **5** | 5 | **Rejected**: one shorted FET would defeat the E-stop | |
| 15 | Off-the-shelf architecture (section 5) | **Negative** | 5 | — | **Rejected** | |
| 16 | USB-C PD rails (section 4) | **Negative today** | 2 | — | **Rejected for now** | |

**Dropping "redundant" protection.** I went through every layer. None is truly stacked, because each covers a different fault:
- limb pigtail fuse: limb short;
- U1 breaker: bus fault and inrush;
- bank A trip: a shorted bank B;
- JK DOC: pack over-current;
- Class T: pack short;
- D1 TVS and absorber: different energies;
- thermostat: hardware backstop for the software clamp-energy cap;
- F2 behind U7: shorted Q7, about $3.

There is no real money here. Leave it.

---

## 3. 12 V rail: own buck vs bricks (Joey's 15:21Z request)

### Real load (from REV_B_PARTS.md, 12 V budget table)

| Load | Expected | Ceiling |
| --- | --- | --- |
| Jetson Orin Nano dev kit, 25 W. Barrel jack, **9–20 V** per NVIDIA staff ([forum](https://forums.developer.nvidia.com/t/jetson-orin-nano-input-voltage/298625)) | 2.1 A | 2.1 A |
| Pi 5 buck, 25.5 W | 2.4 A | 2.4 A |
| 4 fans | 0.6 A | 0.6 A |
| SPARE_ON (head) | 1.0 A | 2.1 A |
| SPARE_SW (neck + jaw STS3215, 12 V version, firmware cap) | 3.0 A | 4.42 A (U9 limit) |
| **Total** | **9.1 A, 109 W** | **11.6 A, 140 W** |

**So a 12 A continuous / about 15 A current-limit design is right.** 25 A is 2× oversized.

### Options

| Option | Price | Fit | Verdict |
| --- | --- | --- | --- |
| Murata DVE0125V2NBC (25 A, 36–75 V) + adapter board | $70.99 Sager / $82.80 Mouser / $85 DigiKey + adapter $15 (repo) | As designed | Works, oversized |
| Delta E48SC12020NRFA (20 A, 36–75 V, 100 V/100 ms) | $41.53 (0 authorized stock), brokers $66–72 | Same 1/8-brick class, adapter re-check | Good if stock appears |
| Delta E48SC12010NRFA (10 A) | $39.80 DigiKey, 119 in stock ([Octopart](https://octopart.com/search?q=E48SH12012)) | Fits | **Too small** for the 11.6 A ceiling |
| Mean Well IQB150W8-12 | $91–106 ([Octopart](https://octopart.com/search?q=IQB150W8-12)) | Quarter brick | No saving |
| Mean Well SD-150C-12 (36–72 V, 12.5 A) | $52.30 ([Jameco](https://www.jameco.com/z/SD-150C-12-MEAN-WELL-DC-DC-Power-Supply-Single-Output-12-Volt-12-5A-150W_2107080.html)) | 199 × 110 × 50 mm, 0.86 kg | **Doesn't fit the torso.** It is the "heavy iron" Joey doesn't want |
| Mean Well RSD-150C-12 / RSD-200C-12 | not priced | 33.6–62.4 V input | Input max is below the 80–91 V BAT peaks. No |
| Generic Amazon "48→12 V 20 A golf-cart" bucks | not priced | Potted, no transient rating | Sketchy: no |
| **Own buck, 12 A, on a small separate board** | **≈ $25–30 (est)** | Replaces the DVE on the rail and on J24 | **Recommended for the aggressive package** |

### Own buck: controller choice

The 80–91 V BAT transient decides the controller voltage class.

| Controller | Vin | Control | Price | Notes |
| --- | --- | --- | --- | --- |
| TI LM5145 | 75 V | VM | — | **Out**: below the BAT peak |
| TI LM5143 | 65 V dual | CM | — | **Out**: below the BAT peak |
| ADI LT8390 | 60 V buck-boost | — | — | **Out**: below the BAT peak, and the wrong topology |
| TI LM5146 | 5.5–100 V | Voltage mode + line feed-forward; **external Type III compensation** | $2.4–3.4 at stocking brokers ([Octopart](https://octopart.com/search?q=LM5146RGYR)). 7.5 V gate drive, 100 kHz–1 MHz ([TI](https://www.ti.com/product/LM5146)) | WEBENCH and TI quick-start calculator |
| ADI LTC7803 | 100 V class | Current mode, Type II compensation | $6.19–6.76 ([Octopart](https://octopart.com/search?q=LTC7803EMSE%23PBF)) | **Easier loop**; LTspice model. Verify its abs-max in the datasheet |
| ADI LTC7801 | 150 V | Current mode | $14.35 at DigiKey/ADI ([Octopart](https://octopart.com/search?q=LTC7801EUFD%23PBF)) | Real margin over 91 V with no input filter |

**Recommendation:** LTC7803, or LM5146 if you prefer TI's tools. Either way, put an input LC (about 1–2.2 µH, 6 A, plus a 47 µF 100 V hybrid plus ceramics) ahead of it, keep U7 and F2 in front, and extend `tools/absorber_sim.py` to the converter input node to confirm the peak stays ≤ 85 V. Take LTC7801 (+$8) if you'd rather not depend on the filter. Use 100 V FETs either way.

### BOM (qty 1, 48 → 12 V, 12 A, about 250–300 kHz)

| Part | Price |
| --- | --- |
| Controller | $3–7 (above) |
| 2 × 100 V FETs, BSC040N10NS5 class, or a low-Qgd part on the high side | $2–6.5 ($3.22 Newark; brokers $0.6–1.4) |
| 10 µH shielded, Isat ≥ 15–18 A, ≤ 10 mm tall (fits the 12.4 mm ceiling) | $2–6. SRP1770TA-100M $1.91 ([Octopart](https://octopart.com/search?q=SRP1770TA-100M)); verify Isat, else a Coilcraft XAL1510-103-class part (est) |
| Input LC + 47 µF 100 V APZ ($3.75) + 4 × 2.2 µF 100 V 1210 | ≈ $6 (est) |
| 2 × EEH-ZA1E331P ($0.59–1.91: [Octopart](https://octopart.com/search?q=EEH-ZA1E331P)) + 4 × 22 µF 25 V | ≈ $3–5 (est) |
| About 25 small parts, plus a small inverter so that J24 pin 5 keeps "low = on" | ≈ $2 (est) |
| 2-layer 2 oz board, about 45 × 30 mm, ×5 | ≈ $5–10 per order (est) |
| **Total** | **≈ $20–30 parts + PCB**, against $100 for DVE + adapter |

### Area, efficiency and heat

- **Area:** about 30 × 45 mm, about 13 cm² one-sided (est). Main-board fill is already 88–90 %, and the DVE went off-board to free about 2,200 mm². So put it on its own board on the rail where the DVE sits, with a thermal pad to the rail. That keeps the J24 pigtail and the DVE as a drop-in fallback.
- **Efficiency:** about 94–96 % at 48 → 12 V, 10 A (est, typical of these controllers' 12 V EVMs; not verified). That's 5–6 W at 109 W and 6–9 W at the 140 W ceiling, about the same as the DVE (about 9 W at ceiling, repo).

### Design risk for a hobbyist

- **Hot loop:** input ceramics within a few mm of the FET pair, a small switch-node copper area, and a solid GND under it. This matters most: at 55 V, a sloppy loop rings the switch node toward the FET rating.
- **Compensation:** Type III (LM5146) is the fiddly one; Type II current mode (LTC7803) is easy with LTpowerCAD/LTspice. Check the load step 1 → 10 A on a scope.
- **Current limit:** inductor DCR sensing vs a shunt.
- **UVLO:** set about 34 V on, consistent with the "shut compute at 38 V" software rule.
- **Start-up:** Jetson inrush at start-up.
- **EMI:** keep it away from the INA/CAN analog. A separate board helps.

**Bring-up:**
- current-limited 36–58 V bench supply;
- electronic load (the DL24P-class load in the pack plan);
- scope on SW and Vout;
- thermocouple on the FETs and inductor.

**Schedule:** expect one re-spin ($5–20).

**Verdict:** worth it. It is about $70 off, and the fallback stays one plug away.

---

## 4. USB-C PD rails (Joey's 15:22Z idea): loses today

| Load | Can it take PD? |
| --- | --- |
| Jetson Orin Nano dev kit | No: barrel jack, 9–20 V (above). Stays on 12 V |
| Pi 5 | Only via a **non-standard 5.1 V / 5 A** PDO. Standard PD tops out at 3 A at 5 V, and 5 A needs an e-marked cable. Without 5 A the Pi limits its USB ports. The planned 12 → 5 V buck into the 5 V pins is cheaper and has no cable issue |
| Cameras (Logitech BRIO class) | Bus-powered from the host's USB port. They need no supply of their own |
| Fans | 12 V DC |
| STS3215 neck/jaw servos | 12 V bus power, not USB |
| Head electronics | 12 V on SPARE_ON |

That leaves **zero PD loads today**.

**Chips:**
- Injoinic IP6538 is about $0.65 ([LCSC C727912](https://www.lcsc.com/product-detail/C727912.html)), but **8.2–32 V in** and 45 W shared across 2 ports.
- Infineon CCG7D is **4–24 V** (40 V tolerant), sold in 2,000-unit reels at €2.98 ([Farnell](https://ie.farnell.com/infineon/cypd7271-68lqxqt/usb-type-c-pd-ctrl-40-to-105deg/dp/4228540)), and needs firmware.
- TI TPS25750 is a PD controller only and needs its own power path plus a buck.

All of them still need the 48 V → 12–20 V buck first, so PD adds cost on top: about $3–6 per port for chip, inductor, caps and receptacle (est).

**Mechanics:**
- USB-C holds by friction only (roughly 8–20 N withdrawal per the spec), which is poor in a moving robot.
- Screw-locking Type-C exists, but its cables are niche and expensive.
- Hobby use needs no certification, but non-compliant PD can fail to negotiate.

**Where PD would win:** if real PD-native loads appear (USB-C displays, a USB-C hub, phones). Even then, add a small off-the-shelf 12 V→PD module at that load rather than designing it into the board.

---

## 5. Is an off-the-shelf architecture cheaper? No

Candidate building blocks:

| Block | Price | Problem |
| --- | --- | --- |
| Flipsky anti-spark switch 200 A, 90 V max | $32 sale / $68 regular ([Flipsky](https://flipsky.net/products/anti-spark-switch-aluminum-pcb-case-v1-0-200a)) | **A latching push button.** It has no NC-loop input and no protection against a shorted FET, so it can't be the E-stop |
| ODrive regen clamp | $89, sold out ([ODrive](https://shop.odriverobotics.com/products/odrive-regeneration-clamp)) | 12–58 V operating: the top of that range sits on the 57.5 V clamp target |
| Applied Motion / Omega RC-050 clamp | €345 ([Farnell](https://ie.farnell.com/omega/omrc-050/regeneration-clamp-80vdc/dp/3909193)) | Price |
| Four Adafruit INA228 meters | $60 | Price |
| DC contactor with precharge | not priced | 60 × 60 × 70 mm: it doesn't fit the envelope (PLAN §10) |

Adding up the blocks:
- **Total:** ≥ $180–250 before the contactor.
- **Fit:** it doesn't fit the envelope.
- **Function lost:** the fast short-circuit trip, the dual-bank E-stop and the MID self-test.

The custom safety core is about $90–100 of parts. **Keep the custom board.** Of the modules, the only one worth buying is the 12 V converter, and section 3 says even that can be built cheaper.

---

## 6. Two packages

### Moderate: architecture unchanged, 4-layer 2 oz kept, leg footprints kept

Ideas: 1, 2, 3, 4, 5, 6, plus 8 if Delta stock appears.

| Line | Baseline (repo) | Moderate |
| --- | --- | --- |
| Power FETs | $28 (5) | $16.8 (3: Q1, Q8, Q7) |
| Controllers | $24 | $24 |
| Meters | $39 (7 × INA228) | $19.4 (6 × INA237; aux deferred) |
| Shunts | $14 | $12.3 (RS5 DNP) |
| Absorber, bus, C41 caps | $23 | $23 |
| M4 terminals | $46 | $25.6 (7 REDCUBE + 2 Micro-Fit for the brake leads) |
| Micro-Fit, GH, test points | $13 | $13 |
| Diodes, BJTs, fuses | $13 | ≈ $6 (LCSC, est) |
| MLCC and resistors | $18 | ≈ $8 (LCSC, est) |
| **Board parts** | **$215** | **≈ $150** |
| PCB share (4L 2 oz) | $15–35 | $15–35 |
| 12 V brick + adapter | $85 + $15 | $71 (DVE, Sager) + $15. **$41.5 + $15 with Delta** |
| HS50 ×2, thermostat | $20 + $10 | $12.2 + $5.6 |
| Limb fuses + holders | $25 (6) | ≈ $12.5 (3: arms, waist; est pro rata) |
| Class T + holder | $40 | $40 (shop Sager for the fuse) |
| Connectors/crimps, wire | $25 + $10 | ≈ $22 + $10 |
| JK BMS, E-stop | $100 + $15 | $100 + $15 |
| **Off-board incl. BMS + E-stop** | **$345** | **≈ $303 (DVE) / ≈ $274 (Delta)** |
| **System** | **≈ $575–595** | **≈ $440–490** |

What changes:
- The main path drops to 30 A continuous (arms and waist).
- The INA driver needs a rewrite for INA237.
- The aux port waits.

Re-adding legs means fitting Q2/Q9, RS6/RS7, U15/U16 and J12/J13, the DNP parts already on the board. No re-spin.

### Aggressive: simplify for the stationary stage, with a planned rev C for the legs

Ideas: 1–7, 9, 10, 11, 12.

| Line | Aggressive |
| --- | --- |
| Power FETs | $14.4 (Q1, Q8 IPB017; Q7 BSC040) |
| Controllers | $24 |
| Meters | $12.9 (4 × INA237: L arm, R arm, waist, MID. U11 still provides VBUS) |
| Shunts | $8.8 (RS1–RS4, RS9) |
| Caps | $23 |
| Terminals | $8.4 (J1/J2 REDCUBE + 2 Micro-Fit; limbs and returns soldered pigtails) |
| Micro-Fit, GH, test points | $13 |
| Diodes etc., passives | ≈ $14 (LCSC, est) |
| **Board parts** | **≈ $120** |
| PCB share (2L 2 oz) | ≈ $6–12 (est) |
| 12 V | Own 12 A buck on a small board ≈ $25–30 (est), replacing DVE + adapter ($100) |
| HS50, thermostat | $12.2 + $5.6 |
| Limb fuses | ≈ $12.5 |
| Class T | $40 (kept: safety) |
| Connectors/crimps, wire | ≈ $17 + $10 |
| JK BMS, E-stop | $100 + $15 |
| **Off-board incl. BMS + E-stop** | **≈ $240** |
| **System** | **≈ $365–385** (±25 %) |

What is lost:
- the 60 A leg-ready main path: legs need a rev C board;
- board-level 12 V and servo power telemetry;
- screwdriver-swappable limb leads.

What is added:
- the converter design and bring-up risk, with the brick as fallback on J24.

The E-stop (two banks, latch, MID self-test), the clamp, the absorber, the thermostat and the Class T fuse are untouched in both packages.

---

## 7. Side findings (fix whichever package)

1. **`bom-digikey-upload.csv` is incomplete:**
   - it is missing APZ1012680M080R ×4 (C10–C13), EEH-ZA1E331P (C41) and CRCW251222R0FKEGHP (R59), so the live check will under-price;
   - it lists 74651194 ×13, but only 12 are fitted;
   - distributors list the MPN as **74651194R**.
2. **Sensata 67L120** is in stock at Heilind (2,035) and ICC (678) at $5.57. That clears the audit's thermostat stock flag.
3. **APZ1012470M100R** shows 6,799 at DigiKey on the Octopart snapshot. Recheck the audit's blocking issue 1.
4. **The JK BMS is now the largest single off-board line ($100).** A 60 A JK for the arm-only stage would save a little. But the pack plan already expects a leg-era pack and BMS redo, so leave it to the battery thread. I did not price it.
