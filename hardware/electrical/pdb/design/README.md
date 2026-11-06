# Power board rev A: design sketch

2026-11-06. First design pass after the architecture plan ([`../PLAN.md`](../PLAN.md)). **Sketch, not a schematic.** It picks a
part for every block, does the calculations that size them, fixes the sheet structure and the HP-to-LP signal list, and
places everything to scale ([`FLOORPLAN.png`](FLOORPLAN.png)). Every part number is **verify** until checked against its
current datasheet and stock at JLCPCB or Digi-Key during schematic capture. Tool: KiCad 10 (project stub in
[`../../schematics/marengo_pdu/`](../../schematics/marengo_pdu/)).

Decided inputs: leg-ready ratings, 80 A continuous / 150 A for 100 ms on the main path (Joey 07:28Z); BMS on the board,
high-side; regen to the pack first, clamp as backup (Joey 08:58Z); HP + LP boards plus a Pi power stub, no XT breakouts.

## 1. Boards in rev A

| Board | Size | Stack-up | Content |
| --- | --- | --- | --- |
| HP (high power) | 100 × 95 mm | 4 layers, 2 oz outer and inner (verify the fab's heavy-copper option and price) | pack in, pack sense connector, BMS AFE, shunt, BMS FET banks, charge port, E-stop banks, VMOT bulk, regen clamp, 6 port channels, port fuses and connectors, fast current amps |
| LP (low power) | 100 × 53 mm | 6 layers, 1 oz | supervisor, 2 CAN bridges, USB hub, 6 isolated CAN, 12 V and 24 V front ends, point-of-load rails, rail switches and meters, I/O |
| Pi power stub | 30 × 20 mm | 2 layers | 12 V → 5.1 V 6 A buck at the Pi |
| Pack sense | 40 × 25 mm | 2 layers | 14 tap fuses, 3 NTC, 2 × 10 Micro-Fit (battery thread spec) |

## 1a. No plugs on the boards: everything leaves on pigtails (frame check 09:16Z)

The frame thread measured the plug room around the carrier (main 733b9d0): 13.6 mm above the HP top edge before the shared
front rail and shelf plate, 8.5 mm in front, the side edges blocked except a 10 mm band at Z 40 to 50, and 35 mm below the LP
bottom edge. No XT plug fits on the HP board. So rev A puts **no high-current connector on either board**:

- **High-current exits are ring-lug pigtails on low-profile threaded board terminals** (Würth REDCUBE-class M4 press-fit or SMT
  terminals, about 5 to 8 mm tall, 80 A class, verify). Serviceable with a screwdriver, no solder joint carrying the current,
  and the cable leaves flat along the board.
- **Pack power and pack sense mate at the back, at the D2 door,** not at the board. The HP top edge carries a 10 AWG pair and the
  sense harness over the pack's top edge (8.6 mm headroom, fine for cable) to the pack's rear top corner, where they meet the
  pack's XT90S and the battery thread's sense board (pack rear face, X 5 to 45, Y 318 to 343). Unplug at the door, slide the
  pack out: no reaching into the torso, and no shelf cutout needed. Battery thread (09:18Z): the pack's XT90S pigtail is about 60 mm of
  10 AWG from the fuse, and both pack-side connectors sit in a printed holder on the case; board-side leads reach that holder with
  about 30 mm of slack.
- **Arm ports:** pigtails from the HP top corners run up the front corners with the trunk; the arm trunk's first XT30(2+2) joint
  becomes an inline connector on that run. Agreed with the trunk thread (09:18Z): about 250 mm of 16 AWG or heavier plus the
  CAN pair, XT30(2+2) **female on the pigtail** (live side), joint at the frame step corner around Y 360 to 380, 80 to 100 mm
  short of the pitch RS03's top-rear flat, reachable through a removable shell panel, 30 mm of slack at the joint.
- **Waist, aux, leg ports and the charge input:** pigtails leave the HP side edges in the open Z 40 to 50 band and drop past
  the LP board to inline connectors below Y 190 (35 mm of room), or straight to the shell charge port. The leg pigtails are
  fitted when the legs exist.
- **LP bottom edge** carries the small connectors (fans, rail outputs, USB, E-stop loop, sensors), mating downward.
- The floorplan's connector boxes now read as terminal pads; the layout gate re-draws them as such.
- Cable paths reserved by the frame thread (main e1be5af, item 27): over-the-pack run X −35 to 30, Y 345 to 353.6; front-corner
  trunks X ±47 to 55, Z 62 to 70; side drops |X| 50 to 55, Z 40 to 50, Y 160 to 300; LV up-run X 33 to 45, Z 50 to 60, Y 340 to 376
  (12 V, USB, CAN, fan leads to the upper bay). **Layout constraint:** HP top-edge ring lugs and the 10 AWG bend stay inside
  Y 340 to 353.6, Z 43.5 to 60, so the terminals sit flat with the lug pointing up and the cable turning rearward over the pack.

## 2. High-power board

### 2.1 Main path parts

| Block | Part (verify) | Qty | Why |
| --- | --- | --- | --- |
| Pack input | 2 × M4 threaded board terminals + 10 AWG ring-lug pigtail to an XT90 at the pack's rear top corner (section 1a) | 1 | the board's rear face sits 3.5 mm off the pack lid, so nothing mounts there |
| Pack sense | low-profile locking board header (JST GH or Micro-Fit vertical, to fit 13.6 mm) + harness to the pack sense board's 2 × 10 at the rear | 1 | 14 taps, 3 NTC, presence loop, spare |
| BMS AFE | TI BQ76952 on the HP board, next to the shunt and FET banks, so the gate drive and Kelvin sense stay short | 1 | 13 cells on VC0 to VC13, unused VC inputs tied per datasheet; 3 pack NTCs plus 1 board NTC; DFETOFF from the E-stop loop and the presence loop; I2C to the LP supervisor through the board-to-board connector |
| Charge port | M4 terminals + pigtail to the shell charge port (XT60), reverse-polarity FET, presence divider | 1 | 4 A, CV 53.9 V; joins PACK+ after the BMS FETs (common port) |
| Pack shunt (low side, for the AFE) | 0.25 mΩ 4-terminal, 5 W, 3921 size (Vishay WSLP3921 or Bourns CSS4J-4026 class) | 1 | 80 A → 20 mV, 1.6 W |
| BMS CHG bank, DSG bank (back-to-back) | Infineon IPT015N10N5 (100 V, 1.5 mΩ, TOLL) | 4 + 4 | see 2.2 |
| BMS precharge FET + resistor | small 100 V P-FET + 100 Ω 3 W | 1 | AFE PCHG pin, charges VSYS bulk before DSG closes |
| E-stop bank A, bank B | IPT015N10N5 | 3 + 3 | single direction, body diodes VMOT → VSYS (PLAN 3.4) |
| E-stop gate drivers | TI TPS4810-Q1 class 100 V smart high-side driver, one per bank, enable from the loop | 2 | hardware path, short-circuit protection built in |
| VMOT bulk | 8 × 220 µF 80 V polymer-hybrid, Ø10 × 12.5 mm, laid flat if needed | 8 | about 1.8 mF, under the 24 mm height |
| VMOT TVS | SMDJ58A class | 2 | fast edges only |
| Regen clamp | comparator + 100 V FET (IPT015N10N5) self-powered from VMOT; external 10 Ω 50 W aluminium-clad resistor (Arcol HS50 class) on the front bottom rail | 1 | see 2.3 |

### 2.2 FET bank sizing

- IPT015N10N5: about 1.5 mΩ at 25 °C, about 2.4 mΩ hot (verify). Four in parallel → 0.6 mΩ hot per bank.
- Conduction loss at 80 A continuous: BMS (two banks in series) 2 × 0.6 mΩ × 80² = 7.7 W; E-stop A + B with 3 each
  (0.8 mΩ hot) = 2 × 0.8 mΩ × 80² = 10 W. **At the typical 10 to 30 A it is 0.2 to 1.6 W in total.** 80 A continuous is a
  rating, not a use case; at that level the board needs the fan and the copper pour to spread about 20 W. Thermal check in the
  layout gate.
- Short circuit: the pack can push about 54.6 V / 0.09 Ω ≈ 600 A prospective. AFE SCD at 40 mV on 0.25 mΩ = **160 A, delay set
  to the minimum (about 15 µs, verify)**. In practice the Class T pack fuse, the port eFuses (faster, lower) and the FET pulse
  rating all share this; the schematic gate must show the FET SOA covers 600 A for about 50 µs.
- AFE thresholds on 0.25 mΩ, corrected against the BQ76952 datasheet SLUSE13B (p22: OCD/OCC in 2 mV steps, OCC delay max
  425 ms): **OCD1 16 mV = 64 A** (hardware comparator, ≤ 425 ms), **OCD3 60 A for 1 s** (current-based, firmware level inside the
  AFE) as the agreed 60 A ceiling, **OCC 12 mV = 48 A at 425 ms** (stays under the P50B's 50 A pack charge limit), **SCD 40 mV =
  160 A** (valid step).
- Cell wiring per datasheet Table 16-3 (p75): cells 1, 2 and 16 must be real, so cells 1 to 12 on VC0 to VC12, cell 13 on
  VC15 to VC16, VC13 to VC15 shorted to VC12 as the table shows. Thermistors: TS1, TS3, CFETOFF and HDQ as thermistor inputs,
  TS2 left free for wake from SHUTDOWN (DFETOFF stays on the E-stop loop).

### 2.3 Regen clamp sizing

- Threshold 57.5 V on, 56.5 V off (1 V hysteresis), self-powered from VMOT.
- Resistor 10 Ω: 57.5² / 10 = 330 W while on, 5.75 A, which matches the host's 5 A braking cap when the pack is full (PLAN
  3.4). The 50 W clad part takes about 5× overload for a few seconds; the supervisor logs on-time and the resistor's NTC and
  raises a fault if the average passes 40 W.
- Arm drop for scale: about 15 J, under 0.1 s of clamp on-time.

### 2.4 Port channels

| Item | Arms, waist, aux (4) | Legs (2, unpopulated in rev A) |
| --- | --- | --- |
| Hot-swap controller | TI LM5069 (9 to 80 V, circuit breaker, dV/dt soft start) | LM5069 |
| FET | 1 × IPT015N10N5 | 2 × IPT015N10N5 |
| Sense resistor (LM5069, shared with the meter and fast amp) | 2.5 mΩ, 3 W (1.0 W at 20 A) → 22 A limit | 1.2 mΩ, 5 W (1.9 W at 40 A) → 46 A limit |
| Meter | TI INA228 (85 V, 20-bit, energy registers) on the same shunt | INA228 |
| Fast current | TI INA241 (−5 to 110 V common mode) on the same shunt → LP ADC | INA241 |
| Fuse | 15 A blade, **58 V DC rated** (Littelfuse 997 MINI 58 V class) in a 58 V holder | 40 A, 58 V |
| Load-side clamp | 59 V mini clamp, about 20 W (footprint, may stay unpopulated) | same |
| CAN TVS | NUP2105L class at the connector | same |
| Connector | M4 terminals + CAN header on the board; pigtail to an inline XT30(2+2) | M4 terminals + CAN header; pigtail to inline XT60 + JST GH |

The LM5069 current limit is set by a 55 mV (typ, verify) sense threshold: 2.5 mΩ gives 22 A for the 20 A ports, 1.2 mΩ gives
46 A for the 40 A legs. The circuit-breaker timer is set so the electronic trip always beats the blade fuse.

## 3. Low-power board

| Block | Part (verify) | Notes |
| --- | --- | --- |
| Supervisor MCU | ST STM32G0B1 (USB, plenty of I2C, low power) | separate from the CAN bridges |
| CAN bridges | 2 × ST STM32G474 (3 FDCAN each), candleLight/gs_usb firmware | 6 buses |
| USB hub | Microchip USB2514B (4 port) | supervisor + 2 bridges + service port; one USB-C up to the Pi |
| Isolated CAN | 6 × TI ISOW1044 (integrated isolated supply) | bus pins rated for a 58 V fault (verify) |
| 12 V intermediate, 150 W | TI LM5145 (6 to 75 V sync buck controller) + 100 V FETs | from VSYS through the ideal diode |
| 24 V, 75 W | second LM5145 | from VSYS |
| Ideal diode + hold-up | TI LM74700-Q1 + 1 mF at VSYS (rides 10 ms at 90 W down to 20 V) | |
| 7.4 V servo (6 to 8.4 V), 5 A | TI TPS56637 (4.5 to 28 V in, 6 A) | adjustable by resistor |
| 5 V peripheral, 4 A | TPS56637 | |
| 3.3 V peripheral, 2 A | TI TPS62933 | |
| 5 V / 3.3 V clean | TI TPS7A20 LDOs | under 5 mV ripple |
| Spare rail slot, 3 A | TPS62933 footprint, output set by resistor | |
| 3.3 V always-on | TI LMR38010 (4.2 to 80 V in, 1 A) | from VSYS, ahead of everything |
| Rail switches | TI TPS2663 (60 V, 6 A eFuse) per 12 V and 24 V output; TPS22965 class for 5 V and 3.3 V | each switchable by the supervisor |
| Rail meters | INA238 per output (10) | 2 I2C buses, 5 meters each |
| Fan drivers | 3 × 4-pin PWM headers, tach to the supervisor | |
| E-stop loop | 12 V loop supply, opto or comparator inputs, 2 independent enable outputs to the HP gate drivers | no firmware in the enable path |

## 4. Schematic sheet structure (KiCad hierarchy)

```
marengo_pdu (root)
├── HP_power_board
│   ├── pack_input_shunt_afe   (BQ76952 lives on HP)
│   ├── bms_fets_precharge
│   ├── estop_banks
│   ├── vmot_bulk_clamp
│   └── port_channel ×6        (one sheet, instanced; legs DNP)
├── LP_logic_board
│   ├── supervisor_mcu
│   ├── can_bridges_usb_hub
│   ├── isolated_can ×6        (instanced)
│   ├── front_end_12v_24v
│   ├── pol_rails
│   ├── rail_switches_meters
│   └── io_fans_estop_loop
├── pi_power_stub
└── pack_sense_board
```

## 5. HP-to-LP board-to-board connector (draft)

One locking 2 × 30 header, 2.0 mm pitch, along the HP bottom edge and the LP top edge. No motor current crosses it.

| Pins | Signal |
| --- | --- |
| 6 | VSYS (to the LP front ends, up to 5 A total) |
| 6 | GND |
| 2 | E-stop enable A, enable B (loop logic on LP → gate drivers on HP) |
| 2 | E-stop bank A and B status (drain-source sense) |
| 2 | VMOT sense, clamp status |
| 6 | port enables |
| 6 | fast current outputs from the INA241s |
| 3 | AFE I2C (SDA, SCL) and AFE ALERT |
| 2 | HP meter I2C (port INA228s, HP NTCs) |
| 2 | AFE DFETOFF (loop) and RST_SHUT |
| 12 | CAN_H / CAN_L for the six ports, from the isolated transceivers on LP to the port connectors on HP |
| 2 | charger present, board present |
| 9 | spare and extra GND between CAN pairs |

## 6. Next

1. Joey installs KiCad 10 on Joey-PC.
2. Schematic capture sheet by sheet, starting with the HP main path and the AFE (the safety-critical core), with the calculation
   notes above carried into the schematic as text.
3. Gate 3 review of the schematic (review recipe), then layout.
