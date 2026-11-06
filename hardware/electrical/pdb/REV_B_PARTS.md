# Power board rev B: agreed parts list

2026-11-06 22:44Z. These are Joey's answers to the clean sheet ([REV_B_CLEAN_SHEET.md](REV_B_CLEAN_SHEET.md)). The rule is: a
part goes in only if leaving it out would hurt, and a cheap off-the-shelf part is preferred where one exists. Rev A stays in
the repo as a reference.

## Off the board (bought)

| What | Choice | Notes |
| ---- | ------ | ----- |
| BMS | Off-the-shelf **JK smart BMS** (Joey: "whichever has the most features and is the best") | JK has active balancing, a Bluetooth app, CAN/RS485, common port, and a heater output on some models. Needs 13S Li-ion, 60–100 A, and must fit 170 × 90 × 20 mm on the pack's rear face. The JK BD6A20S10P (162 × 102 × 20) is 12 mm too wide, so the exact model is still to be found (battery thread). The battery thread's five conditions apply (clean sheet, row 1). |
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

1. Battery thread picks the JK model that fits, and confirms precharge and remote-off.
2. Joey confirms the E-stop switch model or rating.
3. New small schematic on the Mac, ERC, short review, then layout.
