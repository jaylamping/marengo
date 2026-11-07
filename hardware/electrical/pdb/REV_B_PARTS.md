# Power board rev B: agreed parts list

2026-11-06 22:44Z, updated for rev B5 on 2026-11-07 (Opus round 5, Joey's go at 15:49Z: [design/REVB5_CHANGES.md](design/REVB5_CHANGES.md)).

These are Joey's answers to the clean sheet ([REV_B_CLEAN_SHEET.md](REV_B_CLEAN_SHEET.md)). The rule: a part goes in only if leaving it out would hurt, and a cheap off-the-shelf part is preferred where one exists. Rev A stays in the repo as a reference.

**Rating: 60 A continuous, under 90 A peak.** Legs come on a rev C board; their port footprints stay unfitted on this one.

## Off the board (bought)

| What | Choice | Notes |
| ---- | ------ | ----- |
| BMS | **Cheap smart 13S 60 A common-port BMS: JBD (Jiabaida) lead, Daly alternate** on the pack's rear face (rev B5: the JK-BD6A17S8P at about $100 was dropped on cost; pack plan section 3) | 60 A, Bluetooth/UART (RS485 option), programmable cell and current thresholds. Short circuit about 300–360 A, delay 200–800 µs (JBD; Daly 200–500 µs): the absorber close pulse is above 180 A for only 36–52 µs. About $40–55. Verify size against the 170 × 90 × 20 envelope. **No precharge** (the board's soft-start bank covers all bulk capacitance) and **no hardware remote-off** (it can't be the E-stop's second break). Settings: OVP 4.22 V, UVP 2.8 V, DOC as high as the app allows (at least 70 A, else buy the 80 A version) at the longest delay, COC 30 A for about 1 s, SCP delay ≥ 200 µs, charge 0–45 °C. Charging uses an XT60 pigtail on the pack's P+/P−, so there is no charge port on the board. Software refuses drive Enable while BMS telemetry shows charge current, or while telemetry is stale (the Pi can't block reset). The robot's only ground is P−. |
| Pack connector | **XT90S (anti-spark) on the pack pigtail, XT90 on the board's soldered 8 AWG pair** (rev B5: 8 AWG fine-strand silicone, the heaviest run in the robot) | Keyed, so a plug-in can't be reversed. Amass XT90: 45 A continuous, enough for arms and waist. The AS150U (70–80 A) doesn't fit the pack corner (frame thread, 2026-11-07). **Open for legs (rev C): pack-corner redesign, e.g. a recessed vertical AS150 pocket.** Board side: the pair bends over the top edge at R15, 7.0 mm max OD at the carrier edge, flat, no sleeve |
| Pack fuse | Class T 60 A, on the pack | Buy the fuse from Sager. It does not clear a reversed pack before D1 fails (REVB5_CHANGES): the keyed plug and the polarity check carry that |
| E-stop button | **HB2-BS545**, 22 mm mushroom, twist to release, 1NC + 1NO | 660 V AC 10 A, control-rated only, so it breaks the small loop and never the motor current. NC in the loop. NO to the Pi as "E-stop pressed". |
| CAN | Isolated USB-CAN adapters at the Pi (gs_usb / SocketCAN), one per bus used | The board doesn't touch CAN. |
| BMS telemetry | **Isolated** USB-RS485 adapter or isolated CAN, to the Pi | The BMS port sits on B−, the robot on P−. A non-isolated adapter ties them together, so load current can use the comm cable when the BMS opens. About $25 to $40 plus software. |
| Brake resistor thermostats | **Two Sensata Airpax 67L120**, one on each resistor's plate, wired in series in the NC loop (rev B5) | 6700 series, TO-220, open on rise ("L"), 120 ±5 °C, resets at 85 °C or above. Contacts 0.5 A at 48 VDC; the loop carries about 17 mA. $5.57 at Heilind (2,035 in stock) on 2026-11-07. Clamp each bracket to its plate strip under the HS50 body with thermal compound. Two, so an open resistor or lead can't leave the other one unwatched |
| Brake resistors | 2 × Arcol HS50 20R J (Ohmite at DigiKey; $6.11 at Arrow/TTI). Order the 20 Ω part (HS5010RJ is 10 Ω) | One per gusset, leads to the Mini-Fit Jr headers J3 (+X) and J25 (−X) |
| Limb fuses | Littelfuse 997 58 V MINI blade fuses in inline holders in each pigtail: 20 A | Wire protection and backup for a shorted port FET. The port breaker clears faults first. **Operator rule: fit or remove limb fuses and connectors only with VBUS under 3 V** |
| 12 V converter | **Own 12 A buck board** (LM5146, [marengo_buck12](../schematics/marengo_buck12)) on the -X rail, on the J24 pigtail. **Fallback: Murata DVE0125V2NBC + adapter** on the same pigtail | See "12 V converter" below |
| Pi 5.1 V | 12 → 5 V 5 A buck module at the Pi | |
| Other rails | None (Joey: no other loads expected) | Add a small module at a load if one appears. USB-C PD was evaluated and rejected: the Jetson takes a barrel jack and nothing else needs PD |

## On the board (one board, existing 100 × 150 mm slot in front of the pack)

| Block | Parts | Why it stays |
| ----- | ----- | ------------ |
| Battery in | One soldered 8 AWG pair, BAT and GND side by side on the top edge, to an XT90 | Keyed; lower pack-loop inductance than the old split studs |
| E-stop switch | Two FET banks in series, both from the E-stop latch. Bank B: hot-swap controller (soft start = precharge for all limbs). Bank A: second bank with its own TPS48100 driver, closes first, slow turn-off, own short-circuit trip. Reset latch (Joey: press reset at every power-up) | Safety, no software in the path. All bulk capacitance sits behind it, so the BMS never sees inrush. One shorted FET can't defeat the E-stop. |
| Heartbeat watchdog (rev B5) | TPS3431 in series with the latch's seal-in NPN, on its own 3.3 V rail from VCTL | A hung, crashed or unpowered Pi drops the motors within 270–355 ms; it can only drop the latch, never hold it |
| Braking clamp | Comparator, FET, **two Arcol HS50 20R J in parallel** (10 Ω total), one on each gusset (+X and −X), on the 5 mm countersunk aluminium plates with thermal compound | A full pack or a BMS trip during regen would push the bus past the drives' 60 V fault. Clamp duty goes to the Pi through U17's shunt input (rev B5) |
| Bus capacitors | 5 × Panasonic EEH-ZC1K470P, 47 µF 80 V hybrid, 235 µF | The drives have their own. Active part line (the APZ 68 µF is obsolete) |
| Limb ports (arms, waist; aux and legs unfitted) | Per port: 1.5 mΩ Kelvin shunt, **TPS48111-Q1 breaker + IPB017 FET** (rev B5), INA237 meter, soldered + / return pigtail pair | A limb fault trips only its own port: 35–45 A in 1.6 µs, or 30 A for about 16 ms, latch-off until an E-stop cycle (tools/port_sim.py) |
| 12 V meter | One INA237 | Compute power in telemetry. |
| SPARE_SW 12 V output | TPS259541 eFuse, 3.87 to 4.42 A limit, soft start, INA237, 2 × 2 Micro-Fit (keyed apart from the 2-pin outputs), enabled by the E-stop latch | Generic switched output, off with the motors. First use: 3 STS3215 neck servos and the jaw servo, with servo current capped in firmware. |
| SPARE_ON 12 V output | TPS259541 eFuse, about 2 A limit, 2-pin output, unswitched | Generic always-on output that stays up through an E-stop. First use: head electronics. |
| Bank check meter | One INA237 reading MID, the node between the banks; its shunt input reads clamp duty | Pi startup self-test. VBUS alone can't show one shorted bank. |
| Pi link | JST GH 7: 3V3, SDA, GND, SCL, E-stop pressed (low), heartbeat out, port fault (low) | The Pi reads all meters and the button, feeds the watchdog and sees port trips; no supervisor chip. |

## 12 V converter (rev B5: own buck, DVE fallback)

Budget:

| Load | Expected | Hard ceiling |
| ---- | -------- | ------------ |
| Jetson Orin Nano, 25 W (barrel jack, 9–20 V) | 2.1 A | 2.1 A |
| Pi buck, 25.5 W at about 90 % | 2.4 A | 2.4 A |
| 4 fans, about 0.15 A each (assumed) | 0.6 A | 0.6 A |
| SPARE_ON | 1.0 A | About 2.1 A |
| SPARE_SW | 3.0 A (servo cap; U9's minimum limit is 3.87 A) | 4.42 A |
| Total | **9.1 A, 109 W** | **11.6 A, 140 W** |

**Buck board** (separate small board on the -X rail, bolted with a thermal pad where the DVE sat):
- **Controller and power train:** LM5146 at 400 kHz, Q1 BSC070N10NS5, Q2 BSC040N10NS5, Coilcraft XAL1010-682 (6.8 µH, 8.9 mΩ, 21.8 A Isat), 5 × 22 µF out.
- **Input:** SK810L reverse crowbar, 1 µF, XAL7030 2.2 µH filter, 47 µF aluminium (EEE-TK2A470AQ) damping, 5 × 2.2 µF hot-loop ceramics.
- **Valley current limit** at about 12–13 A hot. **UVLO** 30.2 V on, 27.2 V off: below the 30.5 V worst sag at 90 A from a 38 V pack.
- **Loss and efficiency:** about 4 W at 12 A (about 97 %).
- **Why separate:** heat goes to the frame, switching noise stays away from the meters and the latch, and the main board has no pour budget left.
- **J24 pinout is unchanged.** Pin 5 low = on (U7 PGD good). With pin 5 open the board stays off, so the DVE fallback plugs into the same pigtail.

Main-board side, unchanged:
- **F2** is a 20 A MINI blade fuse (0997020.WXN) in a Keystone 3568 holder at the board end.
- **U7** limits at 9.7–12.3 A and soft-starts the converter input.
- **Pigtail:** Molex Micro-Fit 3.0 43025-1200 plugs at both ends, 18 AWG, twisted in pairs, about 150 mm. Pin n faces pin n+6 across the rows:

| J24 pin | Net | Faces |
| --- | --- | --- |
| 1, 2 | +VIN (BRK_F, after F2) | 7, 8: −VOUT (GND) |
| 3, 4 | −VIN (GND) | 9: −VOUT; 10: +VOUT |
| 5 | ON/OFF, low = on | 11: +VOUT. A short there turns the converter off |
| 6 | −VIN (GND) | 12: +VOUT |
| 7, 8, 9 | −VOUT (GND) | |
| 10, 11, 12 | +VOUT (+12V_RAW) | |

**DVE fallback.** Murata DVE0125V2NBC: 25 A, input UVLO fixed at 32–36 V on and 30–34 V off. It needs its adapter board:
- 47 µF 100 V EEE-TK2A470AQ plus 1 µF on the input;
- SK810L reverse crowbar;
- 100 µF 25 V hybrid plus 10 µF and 1 µF on the output;
- a strain-relief clamp.

Buy it from Sager ($70.99) if it's needed.

## Bring-up measurements that gate layout sign-off

1. **GATE: pack-lead inductance and pulse loop resistance.** If the pulse loop measures under 0.05 Ω, rerun tools/absorber_sim.py with the measured values before powering motors (0.03 Ω puts 8 cans at about 83 V). BAT to P−, with the real pack, BMS and leads. Measure resistance with a pulse (µs scale): the cells' AC resistance is half their DC value. Also measure the **DC pack resistance** for the software current budget. tools/absorber_sim.py worst corners give BAT 80–91 V with three cans and two D7 diodes; rerun it with the measured values. Over 90 V means more absorber or a faster trip.
2. **BMS close into the absorber.** About 345–395 A peak with the eight 47 µF 80 V cans, above 180 A for 36–52 µs (rev B5; absorber_sim close case). The JBD's short-circuit delay (200 µs minimum) should ride through it. Close the BMS onto a bare board 20 times: it must not trip. **Decided (Joey, 2026-11-07): 10b stays DNP, gated on this test.**
3. **Polarity at the plug** (rev B5). Before the first close, meter the XT90 on the pack harness and on the board pair: BAT on the + contact. Repeat after any harness work.
4. **Port short** (rev B5). At reduced pack voltage, behind a current-limited source, with drives on the other limbs: short one limb port behind its fuse. The port must latch off (PORT_FLT_N low, its output low), the banks must stay closed, and an E-stop cycle plus reset must clear it. Scope RS1 and the port shunt.
5. **Drive bus bleed.** After a stop, time VBUS from 54 V to 3 V with all drives connected. Over 30 s: fit the DNP VBUS bleeder (R63) at a value chosen from the measured capacitance.
6. **Converter start.** Scope U7's start into the buck board at 36 V and 54.6 V: U7 must reach PGD inside its timer, then the buck starts. **Measure the buck loop** with an injection resistor in place of its 0R (crossover near 40 kHz, phase margin at least 45°). Load-step 1 → 10 A. 140 W steady: FETs and inductor under 100 °C.
7. **Servo back-feed.** Back-drive the neck and jaw servos by hand as fast as they go, with U9 on and then off, and scope +12V and SPARE_SW. +12V must stay under 16 V. The SMBJ15A (D20) clamps above 16.7 V; if it conducts, cap back-drive speed in software. U18 clamps SPARE_ON above about 13.3 V.
8. **Brake resistor thermostats.** With the board running and the motors enabled, heat each thermostat with a heat gun. The loop must open at 115–125 °C: RUN drops, both banks open, and the latch stays off. After it cools below 85 °C, the motors must stay off until reset is pressed.
9. **Brake heat test** (rev B5). Run the clamp at 50 W total for 20 min with the real gussets fitted, and log both plate temperatures. Set the software average cap at 80 % of the power that holds the hotter plate at 110 °C.
10. **Watchdog** (rev B5). With the motors enabled, stop the heartbeat: RUN must fall within 270–355 ms and stay off until reset. With the Pi off, reset must not hold the latch. Repeat as a periodic test.
11. **Close order.** Scope RUN, U1 UVLO, FLT_A and GATE at reset (pads on the board). UVLO must not stay above 2.45 V longer than U1's insertion delay before FLT_A goes low.
12. **I²C levels.** Scope the low level at an INA237 with the motors and the Jetson loaded: under 0.4 V.
13. **Scope a 60 A E-stop** and a bus short behind a current-limited source, on BAT and MID.

Brake resistor harness (frame main 72a3889, mirrored HS50s on the ±X gussets):
- Each resistor gets its own twisted pair of **18 AWG** silicone wire (2.84 A peak per resistor, a few seconds at a time) into a Mini-Fit Jr 2-pin plug.
- +X pair: about 50 mm to J3. −X pair: about 200 mm to J25, both on the top edge, running above the board.
- Each thermostat on its plate strip with 1 × M3, then a twisted 24 AWG pair; the two are in series in the NC loop.
- Before first power and at each service, check that both resistors measure 20 Ω each, and 10 Ω across the pair.

## Accepted risks and software requirements (Joey, 2026-11-07; rev B5 rows marked)

| Item | Decision | Requirement |
| ---- | -------- | ----------- |
| E-stop control path is one channel. A loop short or RUN shorted to VCTL holds both banks on. | Accepted. The HB2-BS545 NC contact is positive-opening, so a welded contact is covered by the button's mechanism. | None on the board. Keep the loop harness protected. |
| **Heartbeat (rev B5)** | The watchdog drops the latch without a heartbeat | **Software: toggle WD_FEED (J21 pin 6) from the control loop itself, not a PWM peripheral, every 50–100 ms. A falling edge at least every 270 ms.** Stop toggling to drop the motors on purpose |
| **Drive timeout (rev B5)** | Covers a hung Pi between heartbeat edges and any drive that misses a command | **Software: write CanTimeout on every drive (50 ms or less) and read it back before every enable; refuse enable if any drive reads 0 or above the limit. Re-verify after every drive power cycle** |
| **Port faults (rev B5)** | A port latches off on its own | **Software: when PORT_FLT_N (J21 pin 7) goes low, name the port whose output meter reads low (U11 L arm, U13 waist; U12 reads the R arm port input, so the R arm is the one left when the other outputs are up), log it, and tell the operator to fix it before the E-stop cycle and reset that clears it** |
| Clamp takes 5.7 A, 322 W, against 30 A of host regen: 2.84 A and 161 W per resistor | Accepted with 2 × HS50 20R + two thermostats | **Software: cap braking at about 5 A when the pack is nearly full**, or whenever the BMS isn't accepting charge. **Cap clamp energy, total for both resistors: average from the bench heat test (until then 30 W) over a rolling 60 s window, and 3 kJ per burst.** Energy = clamp duty × VBUS² / 10 Ω, with duty = U17 shunt voltage / 120 mV (rev B5). At either limit, stop braking and make a safe stop |
| **Compute brown-out (rev B5)** | Buck UVLO 27.2 V off, under the worst sag | **Software: total drive current budget = (V_rest − 35 V) / R_pack_measured, tapering as the pack falls. Shut compute down on min cell at or under 3.0 V, filtered pack at or under 38 V, or BMS telemetry lost for more than 5 s** |
| Reset can't be blocked by the Pi. | Bank A's short-circuit trip covers a shorted bank B. | **Software: run the self-test before reset and tell the operator not to press reset on a failure.** |
| Pressed input is information only. | | **Software: debounce it.** Configure that GPIO with no pull, and treat it as invalid until configured |
| **Self-test (one window, rev B5)** | Hot FET leakage moves MID by up to about 4 V | **Software: test only with VBUS under 3 V (U12), banks open, at least 5 s after the last stop, and pack at 40 V or more from BMS telemetry. Correct MID for U17's input divider (÷ Rin / (Rin + 33k), Rin calibrated per board). Healthy: corrected MID 0.33 to 0.62 × pack. Within 2 V of pack: A shorted. Under 3 V: B shorted. Anything else fails.** |
| Servo back-feed into +12V | U9 and U18 don't block reverse current; C41 and the compute loads take it | **Software: keep the neck and jaw servo current cap, and never back-drive them faster than their rated speed.** |
| U9 is the only break for the neck and jaw servos | Accepted by Joey: the neck never moves fast | None |
| A Q4 or Q5 C–E short makes the latch re-arm on button release without a reset press | Accepted by Joey, alongside the single-channel E-stop path | **Bring-up and periodic test: press, release, and confirm the motors stay off until reset.** |
| Reset needs MID healthy and VBUS under 6–8.5 V | After a stop, reset is blocked until the drives' caps bleed down | **Software/operator: wait for VBUS under 3 V (U12) before pressing reset.** |
| Meters (rev B5: INA237) | Same addresses; 16-bit, no energy accumulator | **Software: L arm 0x40, R arm 0x41, aux 0x43, L leg 0x44, R leg 0x45, SPARE_SW 0x47, waist 0x4C, 12 V 0x4D, MID 0x4F. INA237 register map. Shunts: limbs 1.5 mΩ, 12 V 2 mΩ, SPARE_SW 5 mΩ (ADCRANGE 1). Dedicate one Pi I²C controller to the board** |
| U1 latches off after a bus peak over 97 A lasting 147–383 µs | Software can't see sub-ms peaks | **Drives: set each drive's current and power limits so the sum for everything that can move at once stays at or under 90 A; read them back at startup like CanTimeout** |
| Bus capacitance on the drives | U1 starts the bus at 100–220 V/s; a port starts its limb at about 1 kV/s | **Drive interface requirement: total drive bus capacitance at most 20 mF, and no drive load until the bus is above 45 V** |
| Neck and jaw servos | STS3215 comes in 7.4 V and 12 V versions | **Buy the 12 V version.** |

## Fault decode (rev B5)

| ESTOP_PI_N | PORT_FLT_N | MID (U17, corrected) | VBUS (U12) | Port outputs | Meaning | Action |
| --- | --- | --- | --- | --- | --- | --- |
| low | any | any | any | any | E-stop pressed | Release, then reset |
| high | low | ≈ pack | up | one low | That port tripped (short or overload) | Find the fault on that limb, then E-stop cycle and reset |
| high | high | ≈ pack | low | low | Bank B tripped (U1 latched) | E-stop cycle, then reset; check for a bus fault first |
| high | high | ≈ 0.47 × pack | low | low | Loop open (a thermostat or the harness), watchdog trip, or U8 tripped | The Pi knows if it stopped the heartbeat. Otherwise check both thermostats (wait for under 85 °C) and the loop harness, then reset |
| high | high | under 3 V | low | low | Bank B shorted | Don't reset. Repair |
| high | high | within 2 V of pack | low | low | Bank A shorted | Don't reset. Repair |
