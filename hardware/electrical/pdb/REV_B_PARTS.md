# Power board rev B: agreed parts list

2026-11-06 22:44Z. These are Joey's answers to the clean sheet ([REV_B_CLEAN_SHEET.md](REV_B_CLEAN_SHEET.md)). The rule is: a
part goes in only if leaving it out would hurt, and a cheap off-the-shelf part is preferred where one exists. Rev A stays in
the repo as a reference.

## Off the board (bought)

| What | Choice | Notes |
| ---- | ------ | ----- |
| BMS | **JK-BD6A17S8P** on the pack's rear face (battery thread pick, pack plan d42e88f section 3) | 80 A continuous / 150 A peak, 8–17S Li-ion, common port, 0.6 A active balance, 3 temperature sensors, Bluetooth, RS485 or CAN through an add-on module (order with it). 133 × 81 × 18 mm, fits 170 × 90 × 20. About $85 plus the module. **No precharge** (the board's soft-start bank covers all bulk capacitance) and **no hardware remote-off** (it can't be the E-stop's second break). Settings: OVP 4.22 V, UVP 2.8 V, DOC 80 A at the longest delay, COC 50 A for about 1 s, charge 0–45 °C. Charging uses an XT60 pigtail on the pack's P+/P−, so there is no charge port on the board; the host blocks arming while the BMS shows charge current. The robot's only ground is P−. |
| Pack fuse | Class T 60 A | On the pack, unchanged. |
| E-stop button | **Joey's own large E-stop switch** | Its rating decides the wiring: a control-rated button breaks a small loop that drives the on-board switch; a DC-rated disconnect could break the power itself. Model to confirm. |
| CAN | Isolated USB-CAN adapters at the Pi (gs_usb / SocketCAN), one per bus used | The board doesn't touch CAN. |
| BMS telemetry | USB-RS485 adapter or CAN, to the Pi | About $10 plus software. |
| Limb fuses | Littelfuse 997 58 V MINI blade fuses in inline holders in each pigtail | Same as the arm trunk. |
| Pi 5.1 V | 12 → 5 V 5 A buck module at the Pi | |
| Other rails | None (Joey: no other loads expected) | Add a small module at a load if one appears. |

## On the board (one board, existing 100 × 150 mm slot in front of the pack)

| Block | Parts | Why it stays |
| ----- | ----- | ------------ |
| Battery in | M4 terminals from the BMS output | |
| E-stop switch | One FET bank with a hot-swap controller (soft start = precharge for all limbs and the 12 V input), driven by the E-stop loop; reset latch (Joey: press reset at every power-up) | Safety, no software in the path. All bulk capacitance sits behind it, so the BMS never sees inrush. |
| Braking clamp | Comparator, FET, HS25 10 Ω on the frame | A full pack or a BMS trip during regen would push the bus past the drives' 60 V fault. |
| Bus capacitors | A few electrolytics at the clamp and switch | The drives have their own. Limb cap boards (Unitree style) get added if the bench shows ringing. |
| 12 V rail | Board-mount 150 W brick, e.g. Mean Well IQB150W8-12 (quarter brick, 9–75 V in, 12 V 12.5 A) (Joey deferred to me) | No converter to design; small. |
| Limb outputs (arms, waist, aux; legs later) | Terminal pair + shunt + one INA228 meter each (Joey: yes to meters) | Amps per limb at all times. No per-limb switching (Joey: OK to lose it). |
| 12 V meter | One INA228 | Compute power in telemetry. |
| Pi link | I2C header to the Pi | The Pi reads all meters; no supervisor chip. |

## Next

1. Done: JK-BD6A17S8P (see above).
2. Joey confirms the E-stop switch model or rating.
3. New small schematic on the Mac, ERC, short review, then layout.
