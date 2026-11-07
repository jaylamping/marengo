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
| BMS telemetry | USB-RS485 adapter or CAN, to the Pi | About $10 plus software. |
| Limb fuses | Littelfuse 997 58 V MINI blade fuses in inline holders in each pigtail | Same as the arm trunk. |
| Pi 5.1 V | 12 → 5 V 5 A buck module at the Pi | |
| Other rails | None (Joey: no other loads expected) | Add a small module at a load if one appears. |

## On the board (one board, existing 100 × 150 mm slot in front of the pack)

| Block | Parts | Why it stays |
| ----- | ----- | ------------ |
| Battery in | M4 terminals from the BMS output | |
| E-stop switch | Two FET banks in series, both from the E-stop latch. Bank B: hot-swap controller (soft start = precharge for all limbs). Bank A: second bank with its own TPS48100 driver. Reset latch (Joey: press reset at every power-up) | Safety, no software in the path. All bulk capacitance sits behind it, so the BMS never sees inrush. One shorted FET can't defeat the E-stop. |
| Braking clamp | Comparator, FET, HS25 10 Ω on the frame | A full pack or a BMS trip during regen would push the bus past the drives' 60 V fault. |
| Bus capacitors | A few electrolytics at the clamp and switch | The drives have their own. Limb cap boards (Unitree style) get added if the bench shows ringing. |
| 12 V rail | Board-mount 150 W brick, e.g. Mean Well IQB150W8-12 (quarter brick, 9–75 V in, 12 V 12.5 A) (Joey deferred to me) | No converter to design; small. |
| Limb outputs (arms, waist, aux; legs later) | Terminal pair + shunt + one INA228 meter each (Joey: yes to meters) | Amps per limb at all times. No per-limb switching (Joey: OK to lose it). |
| 12 V meter | One INA228 | Compute power in telemetry. |
| SPARE_SW 12 V output (replaces the Motion 12 V row, 2026-11-07) | TPS259541 eFuse, 3.87 to 4.42 A limit, soft start, INA228, 2-pin output, enabled by the E-stop latch | Generic switched output, off with the motors. First use: 3 STS3215 neck servos and the jaw servo, with servo current capped in firmware. |
| SPARE_ON 12 V output (replaces the Head 12 V row, 2026-11-07) | 2 A polyfuse (1812L200/16DR, 3.5 A trip), 2-pin output, unswitched | Generic always-on output that stays up through an E-stop. First use: head electronics. |
| Bank check meter | One INA228 reading MID, the node between the banks | Pi startup self-test. VBUS alone can't show one shorted bank. |
| Pi link | Header to the Pi: I2C plus E-stop pressed (low) | The Pi reads all meters and the button; no supervisor chip. |

## Next

1. Done: JK-BD6A17S8P (see above).
2. Done: HB2-BS545 (see above).
3. Schematic done, ERC clean (`hardware/electrical/schematics/marengo_pdu_revb`). Short review, then layout.

## 12 V budget against the 12.5 A brick (IQB150W8-12)

| Load | Expected | Hard ceiling |
| ---- | -------- | ------------ |
| Jetson Orin Nano, 25 W | 2.1 A | 2.1 A |
| Pi buck, 25.5 W at about 90 % | 2.4 A | 2.4 A |
| 4 fans, about 0.15 A each (assumed) | 0.6 A | 0.6 A |
| SPARE_ON | 1.0 A | 2.0 A hold, 3.5 A trip |
| SPARE_SW | 4.0 A | 4.42 A |
| Total | **10.1 A, 121 W, 81 %** | **11.5 A at polyfuse hold, 13.0 A at its trip** |

It fits in normal use. Two things to watch: the brick's free-air derating starts near 30 C inside a warm torso (15 W of its own heat at 121 W), and the ceilings add up past 12.5 A only if every output sits at its limit at once.
