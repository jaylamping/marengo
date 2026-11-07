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

## Accepted risks and software requirements (Joey, 2026-11-07)

| Item | Decision | Requirement |
| ---- | -------- | ----------- |
| E-stop control path is one channel. A loop short or RUN shorted to VCTL holds both banks on. | Accepted. The HB2-BS545 NC contact is positive-opening, so a welded contact is covered by the button's mechanism. | None on the board. Keep the loop harness protected. |
| Clamp takes 5.7 A, 322 W, against 30 A of host regen. | Accepted. | **Software: cap braking at about 5 A when the pack is nearly full**, or whenever the BMS isn't accepting charge. |
| Reset can't be blocked by the Pi. | Bank A's short-circuit trip now covers a shorted bank B. | **Software: run the MID and VBUS self-test before reset and tell the operator not to press reset on a failure.** |
| Pressed input is information only. | | **Software: debounce it in software.** |
