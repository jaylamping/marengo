# Power board: Opus review + cost-down — proposed change list (2026-11-07)

Six Opus reviewers ran on main `8e7d2df8`: power path, control logic, power parts, small-signal parts, layout, and cost-down. Their full reports sit next to this file (1-…6-…md). Below, the findings are merged and ordered the way you asked: electrical logic, then components, then layout. Each item says whether it holds whatever we do on cost, or depends on the package you pick.

## Recommended package: "Stationary-stage moderate + own 12 V buck"

| | Now | Recommended |
|---|---|---|
| Board (parts + PCB share) | ~$230–250 | **~$160–180** |
| Whole power system (excl. cells) | ~$575–595 | **~$380–430** |

Estimates are ±25 %.

What it does:
- **Honest rating:** state the board as **60 A continuous / under 90 A peak**. Two reviewers independently found that the parts can't do the "leg-ready 80 A / 150 A" anyway: the U1 latch, the FET copper, the 60 A pack fuse and the 85 A terminals are all 60 A-class parts. Legs get a rev C board, which is cheap at JLC, and bring-up will likely force one anyway. The leg footprints stay.
- **Own 12 A synchronous buck** (LTC7803 or LM5146), about $25–30 instead of ~$100 for the Murata and its adapter. The real 12 V load is 9.1 A expected and 11.6 A at most, so the 25 A brick was twice what's needed. The Murata stays as a fallback on the same J24 pigtail.
- **INA228 → INA237:** same 85 V family and pinout, −$16. The Pi integrates energy instead of the chip.
- **Brake-resistor leads to Micro-Fit headers:** they carry 2.8 A, so the screw terminals were overkill. −$12, and it frees area.
- **Limb outputs and GND to solder-in pigtails with strain relief**, instead of 8 REDCUBE terminals. −$21, frees area, and removes the heavy through-hole joints that can't be hand-soldered.
- **Commodity passives from LCSC** with the PCB order: −$15–20.
- **Aux port left unfitted** like the legs: −$12.
- **Cheaper authorized sources** for the HS50s, the thermostat and the Class T fuse: −$15 or more.
- **Kept on purpose:** both E-stop banks, the latch, the clamp, the absorber, the thermostats, the Class T fuse, a 4-layer board (layout needs a solid ground layer for the 60 A return), and both FETs per bank (thermal headroom).
- **Rejected:** USB-C PD (the Jetson is on a barrel jack and nothing else needs PD), an off-the-shelf contactor (costs more and is less safe), dropping bank A, and a cheap MIDI pack fuse.

## 1. Electrical logic (holds for either package)

1. **MAJOR — a crashed Pi leaves the motors live.** Rev B has no hardware watchdog in the E-stop path. Fix: require the drives' CAN timeout on every RobStride (read back before enable), and add a Pi heartbeat that holds the latch, about 3 parts. Software requirement plus small hardware.
2. **MAJOR — a limb short drops the whole bus, not its own fuse.** U1 latches in under 0.4 ms, long before the 20 A fuse melts, and reset closes straight back into the short. That's acceptable while the robot is stationary. Fix now: software uses the INA readings to name the faulted limb and blocks re-arm until it's cleared. Rev C, for legs, needs per-port breakers.
3. **MAJOR — reverse polarity at the battery.** J1 and J2 are identical studs. Fix: a keyed, polarized battery connector (e.g. Amass AS150 or XT90 pigtail), which is also cheaper than studs.
4. **MAJOR — compute brown-out under load.** At 38 V rest, 48–90 A of motor current sags the converter input to 30–34 V, below its cut-off. Fix: raise the shutdown threshold so it's load-aware (software), and size the own buck's UVLO below the worst sag.
5. **MAJOR — the brake energy cap can't be computed.** No clamp signal reaches the Pi. Fix: feed CL_OUT through a divider and RC into U17's spare input, about 3 parts.
6. **Minor — second thermostat** on the −X resistor (+$6). Otherwise an open +X resistor leaves the −X one unwatched.
7. **Minor:**
   - Clamp threshold is actually 56.6/55.9 V; correct the docs.
   - Self-test window has no margin at 36 V; pick one window.
   - Add a fault decode table.
   - Add an operator rule: fit limb fuses with VBUS under 3 V.

## 2. Components

1. **HIGH — both APZ capacitor lines are obsolete** (last-time buy 2026-08-31).
   - **Bus caps:** switch to Panasonic EEH-ZC1K470P (47 µF 80 V, active), 5–6 of them.
   - **Absorber caps:** buy all APZ1012470M100R plus spares now. Mouser and DigiKey have thousands in stock. Qualify the EEH-ZC 100 V-class or another active part as the long-term alternate.
2. **MAJOR — U18 enable pin overvoltage** during servo back-feed (about 7.5 V on a 7 V absolute-max pin). Fix: change R42 from 68k to 33k.
3. **D18's part number doesn't exist.** Use Nexperia BZT52H-C68.
4. **Brake cap needs a bench number.** The HS50 is a 20 W part without a heatsink. Set the software cap from a heat test, not from 50 W.
5. **Minor:**
   - Soft-termination MLCCs on BAT (C1/C2/C20).
   - Key J22/J23 differently.
   - Specify C8/C42.
   - Up-size R48/R10/R58.
   - Q7 → BSC040N10NS5, which is already on the BOM.
   - Sweep D7's resistance in absorber_sim.
   - The CSS4J shunts are "contact factory"; use 2512 parts on the 4 A paths.

## 3. Layout (re-check after the package is applied)

1. **BLOCKER — C45 can't go on the back face.** It's 13 mm tall against 6 mm of room to the carrier. Area freed by the terminal and meter cuts brings it back to the front.
2. **BLOCKER — the FET copper spec doesn't fit.** At the honest 60 A rating with both FETs per bank it's 2.7 W per FET. The copper must be re-planned on all 4 layers (2 oz everywhere, solid GND on layer 2), possibly with a small clip-on heatsink per bank.
3. **Majors:**
   - Put BAT+ and BAT− side by side on the top edge, where the pack cable arrives (lower inductance).
   - Pair each limb's + and − together.
   - Remove the bank B → A → B detour.
   - Absorber loop budget ≤ 10 nH.
   - Guard ring or slot around the E-stop/UVLO node: 0.44 MΩ of leakage from any 54 V copper enables bank B by itself.
   - Open an exhaust path past the terminal columns.
4. **Assembly route:** decide on JLC assembly for the SMT side. The leadless TPS259541 parts and the 50 × 0402 parts need a stencil and hot plate otherwise. Recommended: JLC assembly.
