# Power board rev B: agreed parts list

2026-11-06 22:44Z. These are Joey's answers to the clean sheet ([REV_B_CLEAN_SHEET.md](REV_B_CLEAN_SHEET.md)). The rule is: a
part goes in only if leaving it out would hurt, and a cheap off-the-shelf part is preferred where one exists. Rev A stays in
the repo as a reference.

## Off the board (bought)

| What | Choice | Notes |
| ---- | ------ | ----- |
| BMS | **JK-BD6A17S8P** on the pack's rear face (battery thread pick, pack plan d42e88f section 3) | 80 A continuous / 150 A peak, 8–17S Li-ion, common port, 0.6 A active balance, 3 temperature sensors, Bluetooth, RS485 or CAN through an add-on module (order with it). 133 × 81 × 18 mm, fits 170 × 90 × 20. About $85 plus the module. **No precharge** (the board's soft-start bank covers all bulk capacitance) and **no hardware remote-off** (it can't be the E-stop's second break). Settings: OVP 4.22 V, UVP 2.8 V, DOC 80 A at the longest delay, COC 50 A for about 1 s, charge 0–45 °C. Charging uses an XT60 pigtail on the pack's P+/P−, so there is no charge port on the board; the host blocks arming while the BMS shows charge current. The robot's only ground is P−. |
| Pack fuse | Class T 60 A | On the pack, unchanged. |
| E-stop button | **HB2-BS545**, 22 mm mushroom, twist to release, 1NC + 1NO | 660 V AC 10 A, control-rated only, so it breaks the small loop and never the motor current. NC in the loop. NO to the Pi as "E-stop pressed". |
| CAN | Isolated USB-CAN adapters at the Pi (gs_usb / SocketCAN), one per bus used | The board doesn't touch CAN. |
| BMS telemetry | **Isolated** USB-RS485 adapter or isolated CAN, to the Pi | The BMS port sits on B−, the robot on P−. A non-isolated adapter ties them together, so load current can use the comm cable when the BMS opens. About $25 to $40 plus software. |
| Limb fuses | Littelfuse 997 58 V MINI blade fuses in inline holders in each pigtail | Same as the arm trunk. |
| Pi 5.1 V | 12 → 5 V 5 A buck module at the Pi | |
| Other rails | None (Joey: no other loads expected) | Add a small module at a load if one appears. |

## On the board (one board, existing 100 × 150 mm slot in front of the pack)

| Block | Parts | Why it stays |
| ----- | ----- | ------------ |
| Battery in | M4 terminals from the BMS output | |
| E-stop switch | Two FET banks in series, both from the E-stop latch. Bank B: hot-swap controller (soft start = precharge for all limbs). Bank A: second bank with its own TPS48100 driver, closes first, slow turn-off, own short-circuit trip. Reset latch (Joey: press reset at every power-up) | Safety, no software in the path. All bulk capacitance sits behind it, so the BMS never sees inrush. One shorted FET can't defeat the E-stop. |
| Braking clamp | Comparator, FET, Arcol HS50 10R J on the frame (Joey, 2026-11-07: bigger for redundancy; place it with the frame thread) | A full pack or a BMS trip during regen would push the bus past the drives' 60 V fault. |
| Bus capacitors | A few electrolytics at the clamp and switch | The drives have their own. Limb cap boards (Unitree style) get added if the bench shows ringing. |
| 12 V rail | Murata DVE0125V2NBC (eighth brick with baseplate, 36–75 V in, 12 V 25 A, 94.6 %, negative-logic on/off), picked by Joey 2026-11-07. $85 at Digi-Key (Joey checked), against $106 for the IQB150W8-12. Off the board on the top front rail (frame 79500b9), pigtail J24 to the board's top edge | 2× the current of the IQB, conduction-cooled into the frame, frees about 2,200 mm² of board. |
| Limb outputs (arms, waist, aux; legs later) | Terminal pair + shunt + one INA228 meter each (Joey: yes to meters) | Amps per limb at all times. No per-limb switching (Joey: OK to lose it). |
| 12 V meter | One INA228 | Compute power in telemetry. |
| SPARE_SW 12 V output (replaces the Motion 12 V row, 2026-11-07) | TPS259541 eFuse, 3.87 to 4.42 A limit, soft start, INA228, 2-pin output, enabled by the E-stop latch | Generic switched output, off with the motors. First use: 3 STS3215 neck servos and the jaw servo, with servo current capped in firmware. |
| SPARE_ON 12 V output (replaces the Head 12 V row, 2026-11-07) | TPS259541 eFuse, about 2 A limit, 2-pin output, unswitched | Generic always-on output that stays up through an E-stop. First use: head electronics. |
| Bank check meter | One INA228 reading MID, the node between the banks | Pi startup self-test. VBUS alone can't show one shorted bank. |
| Pi link | Header to the Pi: I2C plus E-stop pressed (low) | The Pi reads all meters and the button; no supervisor chip. |

## Next

1. Done: JK-BD6A17S8P (see above).
2. Done: HB2-BS545 (see above).
3. Schematic done, ERC clean (`hardware/electrical/schematics/marengo_pdu_revb`). Short review, then layout.

## 12 V budget against the 25 A converter (DVE0125V2NBC)

| Load | Expected | Hard ceiling |
| ---- | -------- | ------------ |
| Jetson Orin Nano, 25 W | 2.1 A | 2.1 A |
| Pi buck, 25.5 W at about 90 % | 2.4 A | 2.4 A |
| 4 fans, about 0.15 A each (assumed) | 0.6 A | 0.6 A |
| SPARE_ON | 1.0 A | About 2.1 A |
| SPARE_SW | 4.0 A | 4.42 A |
| Total | **10.1 A, 121 W, 40 %** | **11.6 A, 46 %** |

The current fits with margin, and the heat now goes into the frame:
- **Loss:** at 93 to 94.6 %, the 140 W ceiling loses about 9 W.
- **Baseplate limit:** 110 °C with no derating (IPC 9592); thermal shutdown is 130 °C. From a 50 °C torso, the baseplate-to-air path through plate and rail must be 6 K/W or better. Use a thermal pad, and bolt the full plate face to the rail. Bench check: 140 W steady, baseplate under 100 °C.
- **Input UVLO:** fixed on the DVE at 32 to 36 V on, 30 to 34 V off. Near an empty pack under load sag, compute can drop before the BMS cuts out. **Software requirement: shut compute down cleanly at a pack voltage of 38 V or more.**

Off-board parts at the converter end, in the fuse and cap zone at the -X end of the rail (X -40 to -29):
- 33 µF 100 V aluminium low-ESR, plus 1 µF 100 V ceramic, across +VIN/-VIN at the pins. Murata tests with 33 µF at the pins, and the 150 mm lead is 75 to 150 nH.
- Strain relief.

The 20 A fuse required by Murata is F2, at the board end. That protects the pigtail from its source. U7 already limits at 9.7 to 12.3 A, so F2 only backs up a shorted Q7.

Pigtail: twisted pairs, 18 AWG.
- Input: 2 wires each way, about 4.2 A at 36 V.
- +12 V: 3 wires each way. At 12 A that's about 25 mV in the wire and about 80 mV in the Micro-Fit contacts, so about 0.1 V in all.

## 12 V converter: adapter and pigtail (rev B3 fixes)

The DVE needs parts at its own pins. The rev B3 review found its output had no capacitance there (M9), its input damping was unspecified (M7), and it had no reverse-polarity protection (B3). These parts go on a small adapter board soldered to the DVE pins, in the rail's fuse and cap zone (X −40 to −29):

| Part | Value | Why |
| --- | --- | --- |
| Input bulk | 47 µF 100 V aluminium, Panasonic EEE-TK2A470AQ (SMD, ESR about 0.42 Ω) | Its ESR damps the pigtail (75–150 nH) against the board's 4.4 µF, over-damped at about 0.4 Ω against Z0 of about 0.18 Ω |
| Input HF | 1 µF 100 V X7R 1210 | At the pins |
| Reverse crowbar | SK810L-TP, cathode on +VIN, anode on −VIN | A swapped pair clamps reverse at about 1 V. U7 limits it to 9.7–12.3 A and times out, so it retries at low duty. F2 never needs to blow |
| Output bulk | 100 µF 25 V hybrid polymer (Panasonic EEH-ZA1E101P class, confirm) | Murata asks for 100 µF minimum low-ESR at the output, and the DVE has no sense pins |
| Output HF | 10 µF 25 V X7R 1206 + 1 µF X7R | Murata's output test setup |
| Strain relief | Clamp on the plate | Pigtail |

Pigtail J24, Molex Micro-Fit 3.0, 43025-1200 plug, 18 AWG, twisted in pairs, about 150 mm. Pin n faces pin n+6 across the rows:

| J24 pin | Net | DVE pin | Faces |
| --- | --- | --- | --- |
| 1, 2 | +VIN (BRK_F, after F2) | 1 | 7, 8: −VOUT (GND) |
| 3, 4 | −VIN (GND) | 4 | 9: −VOUT; 10: +VOUT |
| 5 | ON/OFF, low = on | 2 | 11: +VOUT. A short there turns the converter off |
| 6 | −VIN (GND) | 4 | 12: +VOUT |
| 7, 8, 9 | −VOUT (GND) | 5 | |
| 10, 11, 12 | +VOUT (+12V_RAW) | 16 | |

No short across the rows puts +VIN on +VOUT. Before first power, check polarity at the adapter with a meter. F2 is a Littelfuse 0997020.WXN: MINI blade, 20 A, 58 V DC, 1 kA breaking. It sits in the Keystone 3568 holder at the board end.

## Bring-up measurements that gate layout sign-off

1. **Pack-lead inductance** and loop resistance, BAT to P−, with the real pack, BMS and leads. tools/absorber_sim.py assumes 0.5–1 µH and 0.1 Ω, giving BAT 81–83 V on a trip. Rerun it with the measured values. Over 90 V means more absorber.
2. **BMS close into the absorber.** Simulated 190–230 A for a few µs. Close the JK onto a bare board 20 times: it must not trip.
3. **Scope a 60 A E-stop** and a bus short behind a current-limited source, on BAT and MID.
4. **Converter baseplate** at 140 W steady: under 100 °C.

## Accepted risks and software requirements (Joey, 2026-11-07)

| Item | Decision | Requirement |
| ---- | -------- | ----------- |
| E-stop control path is one channel. A loop short or RUN shorted to VCTL holds both banks on. | Accepted. The HB2-BS545 NC contact is positive-opening, so a welded contact is covered by the button's mechanism. | None on the board. Keep the loop harness protected. |
| Clamp takes 5.7 A, 322 W, against 30 A of host regen. | Accepted. | **Software: cap braking at about 5 A when the pack is nearly full**, or whenever the BMS isn't accepting charge. |
| Reset can't be blocked by the Pi. | Bank A's short-circuit trip now covers a shorted bank B. | **Software: run the MID and VBUS self-test before reset and tell the operator not to press reset on a failure.** |
| Pressed input is information only. | | **Software: debounce it in software.** |
| Self-test reads (rev B3 review M6) | | **Software: read MID only with VBUS under 3 V, banks open, at least 5 s after the last stop.** Healthy MID is 0.38 to 0.55 × pack, from BMS telemetry. MID within 2 V of pack means A is shorted; MID under 3 V means B is shorted. Any other reading is a fail. |
| Servo back-feed into +12V (rev B3 review M8) | U9 and U18 don't block reverse current; C41 and the compute loads take it | **Software: keep the neck and jaw servo current cap, and never back-drive them faster than their rated speed.** |
| U9 is the only break for the neck and jaw servos (rev B2 M3) | Accepted by Joey: the neck never moves fast | None |
| A Q4 or Q5 C–E short makes the latch re-arm on button release without a reset press (rev B3 M5) | Accepted by Joey, 2026-11-07, alongside the single-channel E-stop path. Stop still works | **Bring-up and periodic test: press, release, and confirm the motors stay off until reset.** |
| Reset needs MID healthy and VBUS under about 6 V (rev B3 M1, Joey's go 2026-11-07) | After a stop, reset is blocked until the drives' caps bleed down | **Software/operator: wait for VBUS under 3 V before pressing reset**; the Pi shows it from U11 |
