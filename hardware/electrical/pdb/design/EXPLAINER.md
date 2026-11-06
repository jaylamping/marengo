# Power board rev A schematic, explained (high level)

2026-11-06. A plain-language tour of the rev A schematic ([PDF](schematic-rev-A-wip.pdf), [BOM](bom-rev-A.csv)). The detail behind
every choice (part, rejected options, datasheet page, calculation) is in [RATIONALE-log.md](RATIONALE-log.md). **Status:
schematic captured, 27 pages, about 1,180 parts, KiCad's electrical rules check clean. Nothing is built, laid out or tested;
every value is calculated from datasheets.** It still has to pass the gate-3 review before any PCB layout.

## The one rule behind the whole design

**Safety works with no software running.** The battery chip, the E-stop and the braking clamp all act in hardware. Firmware
can turn things *off*, report and log, but it can never hold something *on* that the hardware has turned off. If the
supervisor chip dies, the motors go off and the computers stay up.

## The power path, from the battery to a motor

1. **Battery to the board.** The pack lands on two M4 screw terminals. Every amp passes a tiny 0.3 mΩ resistor (the shunt), and
   the voltage across it tells the battery chip how much current flows.
2. **Battery chip (TI BQ76952).** It measures all 13 cells, the current and 3 pack temperatures, and controls two banks of
   four large transistors (charge side and discharge side). Overcharge, overdischarge, overcurrent, short circuit or
   overheating: it opens the banks on its own in microseconds to milliseconds. *Why this chip:* it is TI's 16-cell monitor that
   drives high-side switches, so the robot's ground never shifts when it trips, which keeps CAN and the Pi safe.
3. **VSYS, the system bus.** After the battery chip there is one bus that feeds two things: the computers (always) and the
   motors (only through the E-stop).
4. **E-stop: two banks of three transistors in series.** The E-stop loop switches them through dedicated gate drivers (TI
   TPS48100) with no firmware in the way. Two in series, because a failed transistor usually fails shorted. A small precharge
   path fills the motor bus capacitors gently first, because slamming the banks onto them would destroy the transistors.
5. **VMOT, the motor bus.** 18 capacitors smooth it. A braking clamp, powered from the motor bus itself, dumps energy into a
   10 Ω resistor on the frame if the voltage reaches 57.5 V (the drives fault at 60 V). Braking energy normally goes back into
   the battery through the same path; the clamp only works when the battery can't take it.
6. **Six limb outputs** (left arm, right arm, waist, aux, and two leg outputs left empty until the legs exist). Each one has a
   hot-swap controller (TI LM5069) that soft-starts the limb and cuts it in microseconds on overcurrent, a current meter for
   your telemetry, a 58 V-rated blade fuse as the last resort, a small clamp for a backdriven arm with a blown fuse, and its own
   isolated CAN bus. Each limb leaves the board on a short pigtail, because there is no room for plugs inside the frame.

## The low-power side

- **Rails.** VSYS feeds a 12 V and a 24 V converter; the 12 V then feeds the 7.4 V servo rail, 5 V, 3.3 V and extra-quiet 5 V and
  3.3 V for sensors. Every output is switched and metered. 1.3 mF of capacitors keep the computers alive through short dips.
- **Brains.** A supervisor chip (ST STM32G0B1) reads every meter, temperature and fan, runs the power-on sequence and talks to
  the Pi. Two CAN bridge chips (ST STM32G474, three CAN channels each) give the Pi six CAN buses over one USB cable. They are
  separate from the supervisor so a crash in one cannot touch the other.
- **Small remote boards.** The pack sense board fuses every cell wire right at the cell. The Pi stub makes the Pi's 5.1 V next
  to the Pi so it stays steady.

## Where the datasheets changed our plan (the important ones)

- **Port transistor:** the planned part could only survive a fraction of an amp while soft-starting at 50 V, so the ports use
  Infineon's linear-mode version (IPB017N10N5LF), which is built for that.
- **Motor bus precharge added:** closing the E-stop banks straight onto the capacitors would exceed the transistors' safe area.
- **Pack shunt is 0.3 mΩ** (0.25 mΩ doesn't exist), so the battery chip's trip points shift to about 67 A discharge, 47 A
  charge or braking, 200 A short circuit.
- **Arm fuses are 20 A, not 15 A,** so the fuse never fights the faster electronic trip.
- **CAN surge protection** uses a 58 V part on every bus, since the usual CAN protector would die if a crushed cable put 54 V on
  CAN.
- **Telemetry:** the fast 1 kHz current comes from a dedicated amplifier per limb; the precision meters update at 100 Hz.

## Choices made along the way (my calls, open to your veto)

1. **Braking resistor stays 10 Ω for now.** It's right for the arms. Heavy leg braking with a full battery would want about 2 Ω,
   so the clamp's transistor is sized for that and only the external resistor changes at the leg stage.
2. **Hold-up capacitors** are too tall for the 13 mm gap between the boards; they will be swapped for more, lower parts in layout.
3. **12 V budget:** if every 12 V load peaked at once it would be about 22 A against a 12.5 A converter, so the supervisor gets a
   load-shedding order (fans and the third board first, the Pi and Jetson last).
4. **Leg outputs** get a 1 mΩ shunt; the leg fuse type is decided at the leg stage (the blade fuse range stops at 30 A).
5. **E-stop stays "cut the power" (Cat 0)**; the parts for a controlled stop are on the board, unfitted.

## What is still owed before layout

- The 150 mA cell-balancing switches (biggest gap; rev A wip only has the battery chip's internal balancing).
- A handful of footprints and datasheets (listed in the rationale log), and bench measurements we can only get on real hardware
  (drive input capacitance, timing on a scope).
- Gate-3 review of the schematic with the reviewer roster, then PCB layout.
