# Power and peripheral board: architecture plan (rev 0, planning)

2026-11-06. Thread "Plan the custom power and peripheral board". **Planning draft: nothing here is approved, no part is
ordered, no schematic exists yet.** It maps what the board has to do, how big each piece is, and which choices are Joey's.
Block diagram: [`POWER_BOARD_BLOCKS.png`](POWER_BOARD_BLOCKS.png). Torso coordinates as in
[`hardware/designs/torso-frame/README.md`](../../designs/torso-frame/README.md).

Evidence labels used below: **vendor manual** (RobStride PDFs in `hardware/docs/robstride/`), **decided** (Joey, with time),
**estimate** (my calculation, method stated), **verify** (from memory of a datasheet, check before ordering), **open**.

## 0. Ownership and interfaces

| Item | Owner |
| --- | --- |
| The board itself: circuit, parts, ratings, connectors, firmware requirements, cooling needs | this thread |
| Board placement, mounting carrier, cooling envelope, fans in the frame | "Finalize the internal torso frame" (handed over 07:11Z) |
| Arm trunk from the board's arm port outward (junction boards, drop fuses, joint jumpers) | "Plan the left arm wiring trunk" (`hardware/electrical/wiring/left-arm-trunk-plan.md`) |
| Bench harness for the shoulder actuators until this board exists | "Wire the RS03/RS02 shoulder actuators" |
| The pack: cells, nickel, pack fuse, case | `hardware/electrical/power/custom-21700-pack.md` |
| Supervisor and CAN-bridge firmware, host drivers | code project (this plan only states the requirements) |

**Envelope from the frame thread (main 91c4acd):** one assembly **100 × 150 × 24 mm** including components, at torso X ±50,
Y 190 to 340, Z 34 to 58, standing in the lower box flush with the frame front face, directly in front of the pack (pack Z −45
to +34). Intake fan 40 × 40 × 10 in the front grille at Y 250 to 290 blowing onto the board; two 40 mm exhausts in the D1 door
plane. If the plan below outgrows that envelope I must say so before routing (section 9 says it fits, tightly).

## 1. Fixed inputs

- **Pack (decided 06:13Z):** 13S2P Molicel P50B (P45B sold out, battery thread 07:29Z), 46.8 V nominal, **54.6 V full**, 39 V empty at 3.0 V/cell (2.9 V
  cutoff → 37.7 V), 9 to 10 Ah, 60 A MIDI fuse on the pack, XT90S out.
- **BMS on this board (decided 06:14Z):** "we can probably incorporate the bms onto there as well".
- **Drives (vendor manual):** RS00, RS02, RS03, RS04 all rated 48 V, operating **24 to 60 V**. At 54.6 V full there are only
  5.4 V of headroom for regen.
- **Fusing (decided 05:18Z):** "blow fuses in a cheap wiring trunk, not a $500 actuator". One fused port per limb trunk, one
  drop per motor in the trunk, no pass-through via a drive's second port.
- **Safety system (decided):** the pack fuse and the E-stop are the safety system, not the BMS. Charging is attended.
- **Loads:** Pi 5 + CAN, Jetson Orin Nano, maybe a third compute board, BNO085 IMU, 3 fans, future sensors; actuators below.

## 2. Power budget

### 2.1 Per actuator (bus side, at the pack)

Method (estimate): bus power ≈ output torque × speed ÷ 0.8 (drive + motor efficiency guess), at 46.8 V. "Peak" is peak torque
at half the no-load speed, the worst corner a real move reaches. "Rated" is rated torque at 100 rpm, the vendor's continuous
test point. Torque and speed figures are from the vendor manuals.

| Drive | Peak torque | No-load speed | Rated torque | Peak bus current | Rated bus current |
| --- | --- | --- | --- | --- | --- |
| RS00 | 14 N·m | 315 rpm | 5 N·m | 6 A | 1.4 A |
| RS02 | 17 N·m | 410 rpm | 6 N·m | 10 A | 1.7 A |
| RS03 | 60 N·m | 200 rpm | 20 N·m | 17 A | 5.6 A |
| RS04 | 120 N·m | 200 rpm | 40 N·m | 34 A | 11 A |

### 2.2 Per limb and whole robot

Leg layout from `config/motors_humanoid.yaml` (2 RS03, 2 RS04, 2 RS02 per leg; Joey owns two RS04 today).
"All-peak" assumes every joint at its peak corner at once, which does not happen; it is the ceiling for sizing a port's
trip point, not the pack.

| Branch | Drives | All-peak | Sum of rated | Board port proposal |
| --- | --- | --- | --- | --- |
| Left arm | 2 RS03, 2 RS02, RS00 | 60 A | 16 A | 20 A electronic trip, **15 A fast fuse** (trunk thread's number) |
| Right arm | same | 60 A | 16 A | same |
| Waist | RS03 | 17 A | 5.6 A | 20 A trip, 15 A fuse |
| Left leg (future) | 2 RS03, 2 RS04, 2 RS02 | 120 A | 37 A | 40 A trip, 40 A fuse |
| Right leg (future) | same | 120 A | 37 A | same |
| Aux 48 V (gripper, head) | | | | 10 A trip, 7.5 A fuse |
| Compute and peripherals | Pi 5, Jetson, third board, fans, sensors | about 90 W | about 40 W typical | internal, see 2.3 |

Robot draw (estimate): arms plus waist at work, 100 to 300 W average with 1 to 1.5 kW spikes; with legs walking, 400 to
800 W average with 2 to 3 kW spikes. **The pack is the limit, not the board:** 60 A fuse × 46.8 V = 2.8 kW, and a 13S2P pack
has about 90 mΩ total resistance (battery thread, P50B at 12.8 mΩ DCIR; measured value to come from cell grading), so 60 A
sags it about 5.5 V and heats it about 320 W while it lasts. P50B cells are good for 120 A continuous at 2P, so the 60 A fuse is
the limit. Fine for arms and waist. For legs the pack (and its fuse) needs a second look
(13S3P, or a pelvis pack in parallel); the board below is sized so that it is not the thing that has to change.

### 2.3 Low-voltage rails (Joey 07:29Z: "many different voltage rails, and steady too")

**Two-stage regulation, so every rail is steady no matter what the motors do.** VSYS swings from 37 V (empty, sagging) to
58 V (full, regen clamp). Stage 1 is one stiff 12 V intermediate bus with hold-up; stage 2 is a point-of-load converter per
rail, fed from 12 V, which never sees a motor transient. Noise-sensitive loads get an extra LDO after their buck.

```
VSYS ─[ideal diode]─┬─[hold-up ≈1 mF, rides 10 ms dropouts at 90 W]─[48→12 V, 150 W]─► 12 V bus ─┬─ point-of-load rails
                    └─[48→24 V, 75 W]─► 24 V                                                    └─ 12 V outputs
```

| Rail | Rating | Loads | Source | Steadiness target |
| --- | --- | --- | --- | --- |
| 24 V | 3 A | aux actuators, LED strips, solenoids, small pumps or grippers, 24 V sensors | VSYS buck | ±3% |
| 12 V | 12 A total (the intermediate bus) | Jetson Orin Nano (9 to 20 V input per NVIDIA forum, so 12 V is fine; 7 to 25 W), third compute board, fans, cameras | VSYS, 150 W module | ±2% static, ±3% on a 50% load step |
| 7.4 V servo (adjustable 6 to 8.4 V) | 5 A | hobby or smart servos for the head, hands, grippers | 12 V buck | ±3% |
| 5.1 V compute | 6 A | Pi 5 through its GPIO 5 V pins (set `usb_max_current_enable=1`, verify) | 12 V buck | ±2%, under 30 mV ripple |
| 5 V peripheral | 4 A | USB devices, cameras, LED strips, CAN-isolator supplies, hub | 12 V buck | ±3% |
| 3.3 V peripheral | 2 A | I2C/SPI sensors, encoders, small boards | 12 V buck | ±2% |
| 5 V and 3.3 V clean | 0.3 A each | IMU, microphones, analog sensors, ADC references | low-noise LDOs after the 5 V and 3.3 V bucks | under 5 mV ripple |
| Spare rail slot | 3 A, set by resistor from 3.3 to 24 V | whatever the third board or a new sensor needs | 12 V buck, standard footprint | ±2% |
| 3.3 V always-on | 0.1 A | supervisor MCU, AFE logic, power button | VSYS, low-Iq | stays up when "off"; ship mode drops it to µA |

Every rail output (and each compute board separately) gets its own **load switch and current/voltage meter** (section 3.9),
a polyfuse or electronic limit, and a status LED. Every converter is rated for a 75 V input so a regen or clamp event cannot
kill it. Compute and peripheral total: about 150 W worst case, 3.2 A at 46.8 V; about 50 W typical.

## 3. Architecture

One sentence: **pack → on-board high-side BMS → system bus → (always: compute rails) and (only with E-stop released:
two series motor switches → motor bus with regen clamp → one switched, fused, metered port per limb).**

```
 PACK (cells + 60 A MIDI fuse + 2 NTC)
   │ XT90S              │ 20-pin balance/NTC/sense (Micro-Fit 2x10)
   ▼                    ▼
 [shunt, low side]  [BMS AFE: BQ76952-class, 13S]──I2C──[Supervisor MCU]
   │                    │ drives                          │
   ▼                    ▼                                 │
 [CHG/DSG back-to-back high-side FETs] ◄──────────────────┤ (DFETOFF also opened by E-stop loop)
   │
   ▼ VSYS (37 to 54.6 V, regen to 58 V)  ◄── charge port (54.6 V CC/CV, attended)
   ├──[ideal diode + hold-up]──[12 V 100 W]──[5.1 V 30 W]──[5 V aux]──► Pi, Jetson, 3rd board, fans, sensors
   │
   └──[E-stop switch A]──[E-stop switch B]──► VMOT ──[bulk caps]──[regen clamp → brake resistor on the frame]
            ▲ hardware gate path, no firmware     │
       NC E-stop loop (body button, tether,       ├─[port: hot-swap switch + meter + blade fuse]─► L arm   (VBUS, GND, CAN1)
        wireless receiver contact, all series)    ├─[ "" ]─► R arm   (CAN2)
                                                  ├─[ "" ]─► waist   (CAN3)
                                                  ├─[ "" ]─► L leg   (CAN4)   future, populated later
                                                  ├─[ "" ]─► R leg   (CAN5)   future
                                                  └─[ "" ]─► aux 48 V (CAN6)

 Logic section: supervisor MCU · 2 × CAN-bridge MCU (3 CAN each) · USB hub · 6 isolated CAN transceivers · one USB cable to the Pi
```

### 3.1 BMS on the board (Joey's call, how to do it well)

- **High-side switching**, so pack negative stays system ground at all times. This matters here more than on an e-bike: every
  CAN transceiver, the Pi and every drive share that ground, and a low-side BMS that trips while a charger or the bench supply
  is connected lifts the whole electronics ground by up to the pack voltage. The cheap JK/JBD boards are low-side; that alone
  is the reason to put the BMS on our board.
- **AFE: TI BQ76952 class** (3 to 16 cells, integrated high-side NFET charge pump, coulomb counter, I2C, 3 thermistor
  inputs, precharge and predischarge FET control; verify the current part and its errata). It runs autonomously: OV, UV,
  over-current in discharge and charge, short circuit in discharge (µs), over-temperature, charge only between 0 and 45 °C
  (battery thread), random cell attach tolerated as long as cells land on ascending VC pins (datasheet 16.3), cell open-wire
  detection (datasheet 12.11, detection time still to read), all in hardware with no MCU in the
  loop. The supervisor MCU only configures it and reads it.
- **Settings** (agreed with the battery thread 07:26Z): charge ends at **4.15 V/cell** (charger CV 53.9 V, and the supervisor
  stops charge when any cell reaches 4.15 V); AFE cell **OV trip 4.22 to 4.25 V with about 1 s delay**, so braking into a full
  pack (terminal voltage rises about 0.15 V/cell at 20 A regen) does not trip it; **UV 3.0 V soft** (host warns, then disarms
  motors) and **2.8 V hard** AFE trip; 60 A discharge OC (matches the pack fuse; raise only with a new pack); 60 °C over-temp;
  short-circuit trip about 150 A in 200 µs (verify against FET SOA). The regen clamp at 57.5 V (4.42 V/cell) sits above the OV
  trip and only acts when the pack can't take the energy.
- **Connection order:** mate the 20-pin balance connector first and the XT90 last; unplug the XT90 first. Check the BQ76952
  datasheet on random cell attach and the cell-input series resistors it needs (verify). The AFE's open-wire check must fault
  safe if the 20-pin comes off with the XT90 still in (verify the BQ76952 open-wire feature and its detection time).
- **Balancing:** passive, external bleed FETs at about 150 mA per cell, during charge above 3.9 V. Matched cells and attended
  charging make passive enough; the pack doc's 2 A active balancer is not needed.
- **Charge FET stays on while running**, so braking energy goes back into the pack. The regen clamp (3.4) covers the moments
  the pack can't take it (full, cold, or CHG FET open on a fault).
- **The BMS is not the E-stop.** The E-stop loop also pulls the AFE's DFETOFF input, but only as a backup to the motor
  switches; opening DSG would also drop compute.

Consequences Joey should know:

1. **The pack has no BMS of its own,** so it charges in the robot (charge port on the board, attended) or on the bench
   connected to the board. A spare pack for swap-and-charge would need its own BMS. Pack removal is for service.
2. **Buy one cheap 13S JK/JBD BMS anyway** (the $45 to 80 line in the pack doc). The pack has to be graded, charged and
   discharge-tested before this board exists, and that BMS is the bring-up tool and the fallback if board rev A has a problem.
3. **Pack sense board (this thread builds it, battery thread asked 07:29Z):** a 40 × 25 mm PCB on the pack carrying 14 tap
   fuses (0.5 to 1 A, at least 63 V DC), 2 NTC inputs (10k B3435, which is the 103AT curve the BQ76952 defaults to; verify)
   and the 2 × 10 Micro-Fit header (battery thread 9ad5303; frame placed it at X 5 to 45, Y 318 to 343, Z −55 to −45, main ad7fb6c), tap order chosen so adjacent pins are one cell apart, 2 pins for a connector-present loop.
   The AFE input resistors stay on the main board. Ordered with the main boards.
4. **Pack connector change:** the 14 balance leads plus 2 pack NTCs go to one locking 20-pin connector (Molex Micro-Fit 3.0
   2 × 10, 600 V class, verify: 14 taps, 3 NTCs, 1 spare; 2 pins form the connector-present loop that opens DSG directly) instead of a JST balance plug, each lead fused at the pack end as the pack doc already says.

### 3.2 Power-on sequence and states

| State | What is on | How you get there |
| --- | --- | --- |
| Ship | nothing (AFE in shutdown, µA) | long-press, or plug in the charger to wake |
| Off | 3.3 V always-on, supervisor asleep | short-press power button wakes |
| Standby | VSYS, compute rails; motor bus off | power button; compute boots |
| Motors armed | E-stop switches closed, VMOT precharged, ports enabled one at a time | E-stop loop closed **and** host "arm" request over USB **and** no charger present **and** no fault |
| E-stop | motor switches open in hardware; compute stays up | any break in the loop |
| Fault | as E-stop, plus the faulted port latched off | port over-current, VMOT over-voltage, BMS fault, over-temperature |

Re-arming after an E-stop needs the loop closed and a fresh host request (matches ADR 0001: hardware released and operator
homing before `READY`).

### 3.3 E-stop: two independent switches, no firmware in the path

- **Loop:** a 12 V, few-mA NC series loop through every E-stop contact: body button, tether plug, and a contact on a
  wireless E-stop receiver (recommended once the robot moves on its own). The loop current itself enables the gate drivers of
  two series FET banks on the motor bus, A and B, through separate drivers (TPS4810-Q1-class 80 V high-side drivers, verify).
  Loop open → both banks off within about 10 µs. The supervisor reads the loop and both banks' states, can open them, and
  cannot close them while the loop is open.
- **Why two banks:** a MOSFET usually fails short. One shorted bank leaves the other; the supervisor sees the disagreement and
  refuses to re-arm. The pack fuse is the third layer.
- **Do not put a mechanical E-stop button in series with the 48 V motor current on the robot.** Mushroom heads are rated for
  control circuits; breaking 20 to 60 A DC at 54 V draws an arc they are not built for. The bench harness does this today at
  24 V; once the battery arrives, the button only ever switches the loop. (Flagged to the arm trunk thread.)
- **Stop category:** Cat 0 (power cut, joints go limp) is the default and what the robot does today. A Cat 1 option (supervisor
  tells the host and the drives to damp first, hardware cuts after a fixed 300 ms RC timer no matter what) costs two parts and
  can be fitted by a jumper. RobStride manuals have a "post-power-off anti-backdrive protection" section (4.2.5 in the RS03
  manual) that may give passive damping when unpowered: **verify** on a drive before relying on it.

### 3.4 Motor bus: bulk capacitance, precharge, regen clamp

- **Precharge:** each port switch is a hot-swap controller (LM5069-class, 9 to 80 V, verify) with a dV/dt soft start and a
  circuit breaker. The E-stop banks close onto the board's own bulk capacitance only, then ports start one at a time, so the
  drives' input capacitors (unknown, **measure**) never see an inrush spike. No precharge resistor or contactor.
- **Regen clamp:** an active shunt regulator on VMOT: comparator at 57.5 V switches a FET into an external brake resistor
  (aluminium-clad, 50 W, 10 Ω class, bolted to a 2020 rail so the frame is the heatsink; off-board, two-wire connector). Plus
  a TVS for fast edges. A TVS alone cannot do this job: a 58 V-standoff TVS clamps at about 90 V.
- **Bulk:** about 2 mF of low-ESR polymer or low-profile electrolytics on VMOT. Standard 63 V cans are 25 mm tall and break the
  24 mm envelope, so they lie down or are replaced by arrays of shorter ones.

### 3.5 Limb ports

Each port: hot-swap switch (soft start, current limit, breaker) → **INA238-class 85 V current/voltage monitor** (verify) →
blade fuse in a holder (the backstop if the FET fails short) → connector carrying **VBUS, GND, CAN_H, CAN_L**, exactly the four
wires the arm trunk thread specified. The supervisor can switch each limb separately (bring up one arm at a time, power-cycle a
faulted limb without dropping the rest) and logs per-limb current, which is the measurement the trunk thread needs to settle
its fuse values.

| Port | Connector (proposal) | Trip / fuse |
| --- | --- | --- |
| L arm, R arm, waist, aux | AMASS XT30(2+2) female on the board, the same family as the RS02 (20 A at 16 AWG per the handoff) | 20 A / 15 A |
| L leg, R leg | XT60 + separate locking 4-pin signal connector (JST GH), or a 2+2 XT60 if AMASS makes one (verify) | 40 A / 40 A |

Leg ports are footprints on rev A; populate when the legs exist.

### 3.6 CAN: on the board, isolated, one bus per limb

- **Today:** a Waveshare 2-CH MCP2515 HAT on the Pi (2 RX frames per controller; Davout paces TX around it). The full body
  needs five buses (both arms, waist, both legs) plus a spare.
- **Proposal:** two STM32G4-class MCUs on the board (three FDCAN controllers each, run as CAN 2.0 at 1 Mbit/s) running
  candleLight/gs_usb-compatible firmware behind a small USB hub. Linux sees them as native SocketCAN `can0` to `can5` with deep
  hardware FIFOs, so the 2-frame limit goes away, and one USB cable replaces the HAT. **This touches the code project** (bus
  names, Davout pacing, `config/motors*.yaml`, the bus-plan ADR that ADR 0001 says is still owed), so it is a proposal to them,
  not a decision made here.
- **Isolated transceivers** (ISOW1044-class, integrated isolated supply, verify) on every bus. The trunk thread's real "$500
  actuator" fear is a crushed cable putting 54 V on CAN; with junction boards ruled out (08:35Z) the drives' transceivers rely on the
  trunk's discrete TVS at the splices, and isolation here protects the Pi, the Jetson and the board. Pick a transceiver whose bus
  pins survive a sustained 58 V fault (ISOW1044 class claims ±58 V; verify), and add a port-side CAN TVS on the board. The bus side of each transceiver references that limb's trunk GND at the port.
- **Termination:** 120 Ω at the board end of each bus, switchable (solid-state or jumper), far end on the trunk's last junction
  board as the trunk thread planned.
- The supervisor MCU appears on the same USB hub as a CDC serial device for telemetry and the arm/disarm handshake. If USB drops,
  the drives' own CAN timeout (600, about 30 ms, ADR 0001) stops them; the supervisor also opens the motor switches if it loses
  the host heartbeat.

### 3.7 Peripherals and I/O

| Function | Detail |
| --- | --- |
| Power to compute | Pi 5 (5.1 V, locking 2-pin to the GPIO 5 V pins or a USB-C pigtail), Jetson (12 or 19 V, barrel pigtail), third board (12 V), all switchable by the supervisor so a hung Jetson can be power-cycled |
| Fans | 3 × 4-pin PWM with tach (12 V), closed-loop on board and pack temperature; fan stop is a fault |
| Temperatures | 2 pack NTCs and 1 board NTC on the AFE; 3 more board NTCs (FET banks, DC-DC) on the supervisor |
| User I/O | power button + RGB status LED on a 4-pin cable to the shell; buzzer for low battery and faults; charge-port LED |
| E-stop | loop in/out connector (4-pin: loop, sense, 12 V, GND), tether jack, wireless receiver header |
| Sensors | I2C (Qwiic-style, 3.3 V) and one spare isolated CAN for future pelvis IMU, foot sensors, head |
| Charge port | XT60 on the shell or the D2 door to the board; reverse-polarity protected; charger presence blocks arming |
| Debug | SWD for each MCU, test points on every rail, a USB-C service port |

The BNO085 IMU stays on the Pi shelf (rigid, close to the host); the board does not need to carry it.

### 3.8 Test mode and fault injection (trunk thread ask, 07:24Z)

So fuse selectivity and CAN fault tests can run without risking real actuators:

- **Bench test mode** (supervisor command + a physical jumper, both needed): ports can be enabled with no host heartbeat, the
  trip point of each port is programmable from 1 A upward, and every trip is logged with peak current and time to open.
- **Dummy-load port:** the aux port doubles as the test port. A plug-in load box (power resistors plus a MOSFET pulse load on
  a small board of its own) drives step and short-circuit currents into a trunk segment, so drop fuses can be checked to blow
  before the port trips.
- **CAN fault-injection header** on one bus (the spare CAN6): jumper positions to short CAN_H to CAN_L, CAN_H or CAN_L to GND,
  and CAN_H to VBUS through a 100 ohm current-limit resistor, plus a removable terminator. This checks the trunk junction boards'
  surge protection and the isolated transceivers before a real trunk sees it. Only usable in test mode with motors disarmed.
- Test points on VSYS, VMOT and every port, sized for a scope probe ground spring.

### 3.9 Observability and telemetry (Joey 07:28Z: "know at all times how many amps/volts we are pulling and where")

**Every branch on the board has its own meter.** The board measures the pack, every limb, every rail and every compute
board directly. Per-motor numbers come from the drives' own CAN feedback, checked against the limb meter. Nothing on the
power tree goes unmeasured.

| Where | What is measured | Part (verify) | Rate |
| --- | --- | --- | --- |
| Each of 13 cells, the stack | voltage per cell, stack voltage, cell imbalance | BQ76952 AFE | 4 to 20 Hz |
| Pack | current (coulomb counter), state of charge, charge/energy in and out, cycle count, 2 pack NTCs, FET states, every protection flag | BQ76952 | 4 Hz, flags instantly |
| Main path (VSYS) | voltage, current, power, energy and charge accumulators (independent of the AFE, so the two cross-check) | INA228 on its own shunt | 1 kHz |
| VMOT | voltage, regen-clamp on-time, clamp energy, brake-resistor NTC | supervisor ADC + INA228 | 1 kHz |
| Each limb port (6) | voltage, current, power, energy; trip count, peak current, time to open | INA228 per port (85 V, 20-bit, energy registers) | 1 kHz |
| Each limb port, fast | current waveform for faults | INA241-class high-side amp into the MCU ADC | 20 kHz into a ring buffer |
| Each rail and compute output (10) | voltage, current, power, energy | INA228/INA238 class per output | 100 Hz |
| E-stop | loop state; each FET bank's gate and drain-source voltage, so a shorted FET is detected | supervisor GPIO/ADC | 1 kHz, edges instantly |
| Temperatures | 2 pack, 2 FET banks, DC-DC, logic, intake air, exhaust air, brake resistor | NTCs on the AFE and supervisor | 10 Hz |
| Fans | RPM and PWM duty per fan | supervisor | 1 Hz |
| Per motor | bus voltage, phase current, torque, speed, temperature as each drive reports them over CAN | RobStride feedback frames (check which parameters the drives expose, e.g. VBUS) | at the control rate |

**Where the data goes.** The supervisor streams a 100 Hz telemetry frame over USB to the Pi: min, max and average of every
channel in each 10 ms window, so short spikes are not lost. It also sends events immediately: port trip, E-stop edge, BMS
fault, clamp activation, over-temperature. On any event it freezes a **black box**: 200 ms before and 200 ms after, at
20 kHz, for every port's fast current plus VSYS and VMOT. The Pi pulls it and stores it with the CAN log, so every trip comes
with its own waveform. The Consul dashboard already has a dummy `PowerBoardMetrics` card (`consul/src/data/power-metrics.ts`,
"replace with shunt telemetry from custom board later"). The channel list above is what the code project would wire into it.

**Derived pack values** (battery thread 07:30Z; supervisor firmware or host): cell spread (max minus min), balance-on time per
cell, a running pack DCIR estimate from load steps (ΔV/ΔI) to track ageing, cycle count and lifetime Ah/Wh throughput, peak
current and peak pack temperature per session, and a timestamped fault log (OV, UV, OC, OT, open-wire) in the supervisor's
non-volatile memory so it survives a pack swap. A third pack NTC by the pack fuse: the BQ76952 has spare multi-function pins
that can read thermistors (verify), but the 2 × 9 connector is full, so it costs a 2 × 10 Micro-Fit. Recommended, cheap.

**Per-motor current.** The board measures each limb exactly. Per motor it relies on the drives' own reports, and the
supervisor reconciles the sum against the limb meter (the residual is wiring loss or error). If Joey wants measured current
per motor as well, the trunk junction boards can carry a sense chip, which needs a small MCU on CAN at each one. Joey ruled out junction
boards (08:35Z, trunk thread), so per-motor current comes from the drives' own CAN reports, reconciled on the host against the
measured limb total; there are no per-motor sense nodes.

**Accuracy target:** ±1% current above 1 A, ±0.2% voltage, energy within 2% over a session. Calibrated against a bench meter
at bring-up, with the calibration stored in the supervisor.

## 4. Grounding

- Pack negative is the single system ground; the low-side BMS shunt sits between the cell stack and that node, everything else
  references the node after it. High-side switching everywhere means no switch ever lifts ground.
- One star point on the board where pack, motor ports, compute rails and charger returns meet. Motor return current never flows
  through compute or USB grounds.
- Each limb's CAN references its own trunk GND through the isolated transceiver, so motor-current ground shifts along a 1.5 m
  trunk do not appear as CAN common-mode on the host side.
- Aluminium frame: bond to system ground at one point through 1 MΩ ∥ 10 nF (ESD bleed without a second current path), unless
  the frame thread prefers it fully floating. **Open**, frame thread.

## 5. Heat and cooling

| Source | Typical (arms working, 10 A bus, 60 W compute) | Worst sustained (40 A bus, 90 W compute) |
| --- | --- | --- |
| BMS FETs + shunt (≈0.7 mΩ path) | 0.1 W | 1.1 W |
| E-stop banks A + B (≈0.4 mΩ) | 0.04 W | 0.6 W |
| Port switches, fuses, connectors (≈1.5 mΩ per path) | 0.2 W | 2.5 W |
| PCB copper and pack leads | 0.3 W | 4 W |
| 12 V and 5 V converters (93% assumed) | 4.5 W | 7 W |
| Logic, transceivers, isolated supplies | 2 W | 2.5 W |
| **Total** | **about 7 W** | **about 18 W**, with 80 A peaks adding about 15 W for seconds |

Estimate. With the 40 mm intake fan at about 8 CFM, 18 W raises the air about 4 °C across the board. Heat is not the hard part;
**hot spots** are: the FET banks get 2 oz outer copper, thermal vias to an inner plane and a clip-on spreader, and the DC-DC
section sits directly in the fan's jet (frame grille Y 250 to 290 → DC-DC at board Y 250 to 290). The board's rear face
(towards the pack) carries no tall parts and gets a 0.5 mm insulating sheet; **ask: 3 mm air gap between pack lid and board
rear face** (frame thread). The brake resistor heat goes into the frame, not the board.

## 6. Physical layout (draft, for the frame thread)

Board stands vertically, component side forward (+Z) into the fan, 100 wide (X) by 150 tall (Y).

| Band (torso Y) | Contents | Edge connectors |
| --- | --- | --- |
| 310 to 340 (top) | Pack input XT90 and 20-pin balance connector on the rear face top edge; arm and waist ports | top edge: L arm, R arm (up the front corners to the shoulders), USB to the Pi, fan leads, power button cable |
| 250 to 310 | BMS FETs, E-stop banks, VMOT bulk, DC-DC modules in the fan jet | |
| 205 to 250 | Logic section: supervisor, CAN bridges, USB hub, isolated transceivers, AFE | side edge: E-stop loop, charge port, sensors |
| 190 to 205 (bottom) | Port switches for waist, legs, aux | bottom edge: waist and leg ports down past Y 155, brake resistor |

**Two PCBs, one assembly:** a thick-copper power board (4 layers, 2 oz, about 100 × 95) and a fine-pitch logic board (6
layers, about 100 × 55) bolted coplanar on the same carrier and joined by a board-to-board connector. A mistake in either is
a cheap re-spin, and the expensive copper is not wasted on logic. Same 100 × 150 × 24 envelope. **Mounting proposal for the
generator:** 6 × M3 through-holes on a 92 × 142 rectangle (corners at X ±46, Y 194 and 336) plus two at X ±46, Y 265, 6 mm
keep-out pads, onto standoffs on the frame thread's printed carrier.

## 6a. Board family (Joey 07:31Z: "multiple boards... low power, high power, XT30/60/90 breakouts?")

Recommendation: **split by current and by job, not by connector.** Every extra board-to-board joint that carries motor current
is a new failure point and a contact resistance that heats, so high current stays on one board; everything else splits freely.

| Board | Job | Where | Size | Status |
| --- | --- | --- | --- | --- |
| **HP: high-power board** | pack in, BMS FETs and shunt, E-stop banks A and B, VMOT bulk, regen clamp, six switched and metered limb ports with fuses and connectors | lower box, upper part of the carrier (Y 245 to 340) | about 100 × 95, 4 layers, 2 oz or heavier | rev A |
| **LP: low-power board** | AFE, supervisor, CAN bridges and isolated transceivers, USB hub, 12 V intermediate and all LV rails, rail switches and meters, fans, temperatures, E-stop loop logic, I/O | lower box, lower part (Y 190 to 243) | about 100 × 55, 6 layers | rev A |
| **Pi power stub** | 12 V → 5.1 V at 6 A right at the Pi, so the Pi's rail is regulated at the load and not 300 mm of wire away | Pi shelf, upper bay | about 30 × 20 | rev A, tiny |
| **Pack sense board** | tap fuses, NTCs, 2 × 10 Micro-Fit | on the pack | 40 × 25 | rev A (battery thread asked) |
| ~~Junction boards~~ | ruled out by Joey 08:35Z; trunk is spliced with inline fuses and plug-in joint jumpers | | | trunk thread |
| **Test load box** | dummy load and pulse load for fuse and trip testing | bench only | | after rev A |

HP and LP join through one locking board-to-board connector (VSYS at a few amps for the rails, I2C/SPI to the port meters,
gate enables, E-stop lines); no motor current crosses it. Either board can be re-spun alone.

**No separate XT30/60/90 breakout boards on rev A** (the drone-style board in Joey's photo is in effect the port edge of the HP
board). A breakout would put two extra high-current joints in every limb path for no electrical gain, and the trunks all start
at the HP board's edges anyway. What a breakout would buy is replaceable connectors, since XT plugs wear; the HP board gets the
same thing more cheaply by using board-mount connectors on screw-down footprints that can be swapped with a soldering iron, plus
short pigtails for the most-cycled ports (pack, charge). Revisit if a limb's connector ever has to move away from the board.

## 7. Parts direction (none ordered; every part number is verify)

| Block | Direction |
| --- | --- |
| BMS AFE | TI BQ76952 (13S in a 16S part), external balance FETs |
| Main FETs (BMS, E-stop, ports) | 80 to 100 V N-channel in TOLL or D2PAK-7, ≤ 1.5 mΩ, parallel per bank as current needs |
| Shunts | 0.25 to 0.5 mΩ 4-terminal, 5 W |
| Port controllers | LM5069 (or newer 80 V hot-swap with breaker) + INA238 |
| E-stop drivers | TPS4810-Q1 class 80 V smart high-side drivers, one per bank |
| 12 V converter | 36 to 75 V in, 12 V 100 W: an eighth-brick module (proven, thermal data published) or LM5145-class controller; must tolerate 60 V transients |
| 5.1 V converter | 60 V-in synchronous buck, 6 A, or a 12 V → 5 V buck off the 12 V rail (simpler, a little less efficient) |
| Supervisor MCU | STM32G0/G4 or RP2350; separate from the CAN bridges so a bridge crash cannot touch power control |
| CAN bridges | 2 × STM32G4 (3 FDCAN each), USB FS, behind a USB2514-class hub |
| CAN transceivers | ISOW1044-class isolated CAN FD with integrated power |
| Connectors | XT90 (pack), Micro-Fit 2 × 10 (balance + sense), XT30(2+2) (arm, waist, aux), XT60 + GH (legs), JST GH/XH for low-current |
| Fab | KiCad (the existing empty `../schematics/marengo_pdu/` project), PCB and SMT assembly at JLCPCB or PCBWay; through-hole power connectors hand-soldered |

Board-thread budget also carries (Joey, via the battery thread 08:10Z): the JK 13S bench BMS (about $60) and the trimmable
4 A 13S charger (about $50), about $110. Board-side Micro-Fit leads are pre-crimped, so no crimper is needed.

Rough cost for rev A (estimate): $150 to 250 for 5 power PCBs and 5 logic PCBs assembled (2 sets populated), $100 to 150 in
parts not stocked by the assembler, and a current-limited 0 to 60 V lab supply (about $100) for bring-up, since the 24 V
bench supply has no current limit.

## 8. How it gets built, and the review gates

1. **Now (no board needed):** pack parts and the cheap JK/JBD BMS on order; pack built, graded and tested on that BMS; arms keep
   the 24 V bench supply and bench harness.
2. **Joey's decisions** in section 10, then the CAN-host proposal to the code project.
3. **Schematic** in KiCad, with a calculation sheet (FET SOA at short circuit, clamp energy, thermal) → **review gate 3**
   ("before anything load-bearing or powered") with the reviewer roster from `hardware/docs/review-recipe.md`.
4. **Layout** → gate 3 again (layout packet: Gerbers, 3D STEP into the frame model, thermal check).
5. **Bring-up** on the current-limited lab supply: rails, then AFE on a dummy cell stack (resistor ladder), then E-stop
   timing on a scope, then one port into a resistive load, then one RS02, then an arm. Pack last.
6. **Validation:** short-circuit test per port and on VMOT (into a fuse-protected load), E-stop open time under 40 A, regen
   clamp with an arm dropped from horizontal, 30-minute thermal soak at 40 A in the closed torso, BMS OV/UV/OT trips with an
   adjustable cell simulator.

## 9. Does it fit 100 × 150 × 24?

Estimate of placed area including spacing: BMS and FETs 3,000 mm², E-stop banks 1,500, six ports 5,400, converters 2,000,
logic and CAN 2,500, connectors and bulk 3,500, the extra rails and their switches 2,500, telemetry meters 1,500: about
**22,000 mm²** against 30,000 mm² of two-sided area (15,000 a side).
It fits with high-current parts on the front and small parts on both faces. The tightest constraints are height (bulk caps,
XT90, converter modules all approach 24 mm) and the top edge, which carries the most connectors. If the leg ports are dropped
from rev A, area falls by about 1,800 mm². No change to the frame envelope requested yet.

## 10. Joey's decisions (my recommendation first)

1. **Size the board for the legs now?** **DECIDED: yes (Joey 2026-11-06 07:28Z).** 80 A continuous, 150 A for 100 ms on the main path, leg port
   footprints unpopulated. Arms-and-waist-only (40 A) is a smaller board but means a re-spin when the legs arrive. The pack
   stays the real limit either way.
2. **CAN on the board** (two 3-channel bridges, isolated, one USB cable) instead of more Pi HATs. Recommend yes; needs the
   code project's agreement.
3. **E-stop:** two FET banks switched by the loop (recommended) vs a mechanical contactor (about 60 × 60 × 70, does not fit the
   envelope). Plus: Cat 0 now with a Cat 1 jumper option, and a wireless E-stop receiver input from rev A.
4. **Pack charges in the robot** through the board (consequence of the on-board BMS); one cheap JK/JBD BMS for pack bring-up.
5. **Board split:** HP + LP boards in one assembly, a Pi power stub, no XT breakout boards (section 6a). Recommend.

## 11. Open items

| Item | Who |
| --- | --- |
| Jetson Orin Nano input range (12 V vs 19 V rail) | this thread, datasheet |
| RobStride drive input capacitance and inrush; whether the drives terminate CAN; anti-backdrive behaviour unpowered | measure on a drive (bench) |
| Measured arm bus current to set port trip and trunk fuse values | shoulder wiring + arm trunk threads, after the board logs it; bench meanwhile |
| ~~Pack-to-board air gap, carrier and standoffs, frame bonding, brake-resistor location~~ closed by the frame thread (main 0a31dda): 0.5 mm Nomex over the pack lid at Z 34, carrier rear face Z 37.5, component face Z 61.5; power PCB Y 245 to 340, logic PCB Y 190 to 243; 6 × M3 standoffs at X ±46, Y 194/265/336; brake resistor 25 × 50 × 8 on the front bottom rail inner face, X ±25, Y 153 to 178, Z 52 to 60; frame bonded at one carrier standoff via 1 MΩ ∥ 10 nF | done |
| E-stop button and charge-port location on the shell; power button and LED location | shell manufacturing thread |
| Bus-plan ADR (5 or 6 buses, interface names, IDs) | code project |
| Third compute board identity and power | Joey |
| Leg-era pack (13S3P or a pelvis pack in parallel; fuse and BMS limits) | at the hip/leg stage |

## Revision

| Rev | Date | Change |
| --- | --- | --- |
| 0.7 | 2026-11-06 | Junction boards ruled out (Joey 08:35Z): per-motor current from drive reports; port-side CAN TVS, 58 V-tolerant transceivers |
| 0.6 | 2026-11-06 | Board family (6a): HP, LP, Pi power stub, pack sense board; no XT breakouts; per-motor sensing on junction boards |
| 0.5 | 2026-11-06 | Rails reworked into two-stage regulation with 9 rails (Joey 07:29Z); telemetry section 3.9 (Joey 07:28Z); P50B cells, pack sense board, charge temperature window from the battery thread |
| 0.4 | 2026-11-06 | Joey chose leg-ready ratings: 80 A continuous, 150 A for 100 ms main path, leg ports as unpopulated footprints |
| 0.3 | 2026-11-06 | BMS thresholds and connection order agreed with the battery thread; pack resistance estimate updated |
| 0.2 | 2026-11-06 | Test mode, dummy-load port, CAN fault-injection header (3.8) |
| 0.1 | 2026-11-06 | Frame thread placed the assembly (main 0a31dda); open item closed |
| 0 | 2026-11-06 | First architecture plan. Takes over the board from the torso-frame thread (which keeps placement, mounting, cooling envelope) |
