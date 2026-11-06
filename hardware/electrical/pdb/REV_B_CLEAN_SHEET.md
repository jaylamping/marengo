# Power board rev B: clean sheet (for Joey to mark up)

2026-11-06. Joey: "I don't want to include a component unless we think it would be a detriment to exclude it, and even in
that case, if there is a cheap off the shelf solution we can employ instead, please bring that to my attention." Target:
all main power stuff on one board of about 140 × 140 mm or less.

Rev A (27 sheets, about 1,290 parts) stays in the repo as a reference only. Nothing below is decided. Mark each row
**keep / swap / drop**. Off-the-shelf part names are starting points to check, not verified picks.

## The list

| # | Block | Hurts to leave out? | Cheap off-the-shelf option | Proposal |
| - | ----- | ------------------- | -------------------------- | -------- |
| 1 | **BMS** (cell monitor, balancing, charge and discharge switches) | Yes: unprotected Li-ion cells | JK/JBD-class 13S smart BMS, Li-ion setting, common port (charge and regen go through the same leads), 100–200 A, active balance, CAN/RS485 telemetry, about $60–100. JK sells 8–20S 100 A models with CAN/RS485 | **Swap.** This removes about 40 % of rev A. Catch: these switch the negative side, so the robot's ground must come only from the BMS output (nothing may touch pack B−). Lives on or in the pack, so it's the battery thread's call. Rev A's high-side choice was about CAN ground, and that doesn't apply if nothing references B−. |
| 2 | **Pack fuse** | Yes | Already on the pack (Class T 60 A) | Keep, on the pack. Nothing on the board. |
| 3 | **E-stop switch** (cuts motor power, no software in the path) | Yes: safety | (a) 48 V 100 A DC contactor (e.g. Littelfuse DCNLR100 class) plus precharge resistor and relay, off the board. (b) One on-board switch bank driven by a hot-swap controller, whose built-in soft start doubles as the precharge | **(b), one bank.** About 10 cm², no box to mount, precharge for free. Rev A's second series bank (in case a transistor fails shorted) becomes optional. (a) works too if the frame has room for the contactor. |
| 4 | **E-stop latch** (your "reset press at every power-up") | Yes: your decision | With (a), a standard start/stop seal-in circuit does it with no electronics | Keep: a few logic parts with (b), none with (a). |
| 5 | **Braking clamp** (dumps regen to a resistor when the battery can't take it) | Yes: with a full pack, regen pushes the bus past the drives' 60 V fault | No good off-the-shelf part at 48–60 V that I know of | Keep, minimal: comparator, transistor, the HS25 resistor on the frame. About 4 cm². |
| 6 | **Motor bus capacitors** | Some: the clamp and switch need a few nearby; the drives have their own | Plain electrolytics | Keep 3–4 on the board. **Your Unitree-style limb cap boards:** add if the bench shows the bus ringing. Deferred, not dropped. |
| 7 | **Per-limb fuse** | Yes: your rule, "blow fuses in a cheap trunk, not a $500 actuator" | 58 V MINI blade fuse (Littelfuse 997) in an inline holder in each pigtail | **Swap to inline holders.** Zero board area, same as the arm trunk. |
| 8 | **Per-limb soft start and electronic trip** (6 hot-swap channels in rev A) | No, if every limb is connected before the bus comes up: the E-stop soft start (row 3) charges all limbs together. Faults are the fuse's job | n/a | **Drop.** Lose: hot-plugging a limb onto a live bus, and switching one limb off by itself. Motors can still be disabled over CAN. |
| 9 | **Per-limb current meter** | Yes: you asked to see amps per limb at all times | Breakout boards exist, but their shunts are too small for 20 A | Keep: shunt plus one INA228-class meter per limb (one, not two), about 1.5 cm² each. Read by the Pi over I2C. |
| 10 | **Pack voltage, current, cells, temperature** | Yes | The smart BMS in row 1 reports all of it | Comes from the BMS, nothing on the board. |
| 11 | **CAN buses** (rev A: 6 buses, 2 custom bridge chips, USB hub, surge parts) | No: CAN can go straight from the Pi to the limbs | Isolated USB-CAN adapters, gs_usb/SocketCAN on Linux (e.g. CANable 2.0 Pro isolated, about $50–60, or a 2-channel isolated USB-CAN) | **Swap.** The board doesn't touch CAN at all. One or two adapters for arms and waist. |
| 12 | **Supervisor microcontroller** | No: the Pi can read the meters, run fans and log | n/a | **Drop.** No firmware to write. |
| 13 | **12 V rail** (Jetson, fans, 3rd board, cameras) | Yes | 36–72 V in, 12 V about 12.5 A, e.g. Mean Well SD-150C-12 / RSD-150C-12. These are boxes (check size), or a compact potted 48→12 V module | **Your pick:** a box on the frame (no board area), or one buck converter on the board (about 15 cm²). |
| 14 | **Pi 5.1 V** | Yes | 12→5 V 5 A buck module at the Pi | Swap to a module at the Pi, as the rev A Pi stub already did. |
| 15 | **Other rails** (24 V, 7.4 V servo, 3.3 V, clean 5/3.3 V, spare) | Only if a load needs them | Small buck modules at the load | **Drop from the board.** Add a module where a real load appears. What did we want 24 V for? |
| 16 | **Rail switches and meters** (9 switches, 11 meters) | No | n/a | **Drop.** Keep one meter on the 12 V branch so compute power shows up in telemetry. |
| 17 | **Computer hold-up** (1.36 mF) | No: dips come from pack sag, which the converter's input range rides through. A BMS trip lasts longer than any hold-up | n/a | **Drop.** |
| 18 | **Charge port electronics** | No | The charger plugs into the BMS charge path with a keyed XT60 | **Drop.** |
| 19 | **Leg channels** | Not yet | n/a | **Defer.** Leg outputs are just two more terminals plus meters; a 100–200 A smart BMS already covers leg current. |

## What would be left on the board

Battery in (from the BMS), one E-stop switch bank with soft start and the latch, braking clamp, a few bus capacitors, six
limb outputs (terminal pair plus shunt and meter each), an I2C header to the Pi, and the E-stop loop connector. The 12 V
buck goes here too if you want it on the board. That's well under 100 × 100 mm, so it fits the existing 100 × 150 mm
slot in front of the pack with room to spare.

Note: the frame slot is 100 mm wide between the posts, so a 140 × 140 mm board won't fit there. The list above doesn't
need it.

## Things to decide first

1. BMS: off-the-shelf smart BMS on the pack (row 1), or custom on the board? Biggest single call; it reverses the 06:14Z
   "BMS on the board" decision.
2. E-stop: one on-board switch bank (3b), or a contactor (3a)?
3. 12 V: converter box on the frame, or a buck on the board?
4. The 24 V rail: needed for anything?
