# Rev B PDB review, lens 2: control logic, sensing, compute rails, software interface

Reviewer: Opus (adversarial, control-logic lens). Schematic: main 8e7d2df8 (gen/gen.py → netlist).
Method: no kicad-cli or ngspice in this container. I rebuilt the netlist by running gen/gen.py against a mock of
gen/kit.py that logs every (ref, pin) → net. That gave 204 symbols and the same DNP set as the audit; the scratch copy
is at `scratchpad/nl/nl.json`. All numbers below are my own Python unless they cite a datasheet. Datasheets I fetched:
TI TPS4810-Q1 (SLUSEW1) and TI INA228. I checked the repo's software docs (AGENTS.md, docs/safety.md) for the host side
of each interface.

I did not re-raise settled items: single-channel loop, Q4/Q5 C–E short, U9 the only servo break, the SDA straps,
D12/D17, the U2 feed, the absorber, the J24 pinout, and the Q12 negative-logic drive.

---

## Findings

### M-1 (major): A hung Pi leaves the motors live. Nothing on the board or in the written requirements makes it stop

- **Evidence.**
  - Netlist: the Pi has no output into the board. J21 carries only +3V3_PI, I2C_SDA, I2C_SCL, GND and ESTOP_PI_N, and
    ESTOP_PI_N is an input to the Pi. RUN, EN_A, UVLO, SP_EN, L_B and LOOP_RET have no Pi-driven member.
  - Rev A had a hardware watchdog that dropped both banks (EXPLAINER.md §5). Rev B dropped the supervisor (clean sheet
    row 12), and no replacement was recorded.
  - The system brief lists "a watchdog". None exists in rev B.
  - Host side, docs/safety.md "No drive-side or independent watchdog": *"Marengo never writes CanTimeout (0x7028) … a
    killed or hung marengo-pi leaves each drive on its last MIT frame (including τ_g feed-forward)… SIGKILL, panic and a
    hung loop are not [handled]."*
  - Only the five right-arm drives happen to hold CanTimeout = 600, left there by PR #254, and it is unverified whether
    that survives a power cycle.
  - With the last MIT frame holding kp = 0 and v_des ≠ 0, a joint keeps turning until it hits a limit. That is
    "moves when it shouldn't".
- **Also from my calculation (see M-2):** compute can brown out under motor load while the motor bus stays up. That
  produces the same hung-host state with no software fault at all.
- **Fix (pick one or both):**
  1. *Zero board parts:* add a REV_B_PARTS software and commissioning requirement. Every drive gets CanTimeout written
     and saved (≤ 50 ms, for example). marengo-pi reads it back (it already reads 0x7028 in protocol-inspect) and
     refuses Enable if any drive reads 0 or above the limit. Re-verify after every drive power cycle.
  2. *Hardware, about 5 parts:* a heartbeat-gated element in the loop.
     - Build: a Pi GPIO square wave → AC-coupled charge pump → an NPN/PNP pass element in series with LOOP_RET, or an
       NPN that robs Q5's base (L_B).
     - Behaviour: it only conducts while the GPIO toggles, so a stuck-high, stuck-low or floating pin drops the latch,
       and reset is still needed.
     - Constraint: it must not be able to hold the latch on (keep the rule "software can drop, never hold").
- **Confidence:** high on the board facts and the repo docs. Medium on drive behaviour beyond the documented
  last-frame hold.

### M-2 (major): The "shut compute down at ≥ 38 V" rule doesn't stop a load-sag brownout, and when the banks are open it rests on one telemetry link

- **Evidence.**
  - DVE input UVLO turns off at 30–34 V (REV_B_PARTS).
  - Pack DC resistance, cells only: 13 × 12.8 mΩ / 2 = 83 mΩ. The 12.8 mΩ DC figure is from the rev B4 review. The
    BMS FETs, fuse and leads add to it.
  - At a 38 V rest voltage:
    - 48 A brings the input to 34.0 V;
    - 60 A brings it to 33.0 V;
    - 90 A (the allowed bus peak) brings it to 30.5 V.
  - Compute can therefore drop at or above the software threshold while VBUS stays live. Combined with M-1, the drives
    then run on their last command.
  - To keep a 90 A peak above 34 V, the pack must rest at ≥ 41.5 V (cells only).
  - Measurement path: no INA228 reads BAT. U6 VBUS is on +12V (U6.8), limb meters read V_Lx behind bank B, and U17
    reads MID. With the banks open, MID is 0.35–0.61 × pack (my corners), so pack voltage while E-stopped comes only
    from BMS telemetry over the isolated USB adapter.
  - A single weak cell trips the JK UVP (2.8 V/cell) at any pack voltage. The clean sheet's "3.0 V/cell soft cutoff by
    the Pi" (row 1, condition 4) never made it into the REV_B_PARTS requirements table.
- **Fix (requirements, plus an optional zero-part change):**
  - **Load-aware current budget:** the total drive current limit is set from (V_rest − 35 V) / R_pack_measured, and it
    tapers as the pack falls.
  - **Shutdown trigger:** shut compute down on min cell ≤ 3.0 V *or* filtered pack ≤ 38 V *or* BMS telemetry lost for
    more than N s.
  - **Bring-up:** add a DC pack-resistance measurement (bring-up step 1 measures only pulse R).
  - **Optional:** move U6 pin 8 (VBUS) from +12V to BRK_IN through about 10 kΩ, so one meter always reads the pack. The
    12 V rail power then becomes software I × 12 V, and you lose 12 V voltage telemetry (it's used for the back-feed
    check), so this is a trade.
- **Confidence:** high on the arithmetic. The real pack R is unmeasured, which is why the bring-up step is in the fix.

### M-3 (major): The brake-energy cap ("clamp on-time × V²/10 Ω") can't be implemented: nothing tells the Pi when the clamp is on

- **Evidence.**
  - Netlist: CL_OUT and CL_G connect only to U3, R22, R23 and Q6. No net reaches J21 or any meter. The sheet says the
    rev A status FET was removed (block 3 note).
  - VBUS meters average over ms-scale conversions, while the clamp cycles hysteretically between 55.8 and 56.8 V. With
    272 µF and 1 V of hysteresis it switches in about 100 µs at a few amps net. So VBUS readings can't give duty.
  - This software cap (50 W per 60 s, 3 kJ per burst) is the accepted basis for 2 × HS50 (REV_B_PARTS accepted risks).
- **Fix (3 parts, no new I²C address):**
  - U17's shunt inputs are unused: U17.9 and U17.10 are on GND. Feed CL_OUT through a divider and RC (for example
    100 kΩ / about 1 kΩ to give about 120 mV at 12 V, with 1 µF) into U17 IN+, with IN− to GND.
  - U17's shunt reading is then clamp duty × full scale, and its hardware CHARGE accumulator integrates duty with no
    polling.
  - Energy = ∫ duty × VBUS² / 10 Ω.
  - Alternative: CL_OUT through 100k/33k to a Pi GPIO, but Linux can't time µs-scale edges reliably.
- **Confidence:** high that the signal is missing. The fix values are a sketch: size the RC for about 10 ms.

### m-1 (minor): The self-test window has no margin at the low end, and the requirements table carries two windows

- **Evidence.**
  - The MID node, with banks open, includes:
    - 47k/47k divider, ±1 %;
    - U17 load of 33k + 0.8–1.2 MΩ (INA228 datasheet: VBUS input impedance 0.8/1/1.2 MΩ);
    - the interlock load (MID − V_Z) / 440 kΩ, with V_Z 9.4–10.6 V;
    - U8 SRC leakage of 1.4 µA max (TPS4810 datasheet: "SRC pin leakage … 0.85 / 1.4 µA");
    - FET leakage of 0–200 µA per bank.
  - Results:

    | Pack | True MID / pack | Raw U17 reading / pack |
    | --- | --- | --- |
    | 36 V | 0.349–0.607 | 0.335–0.591 |
    | 40 V | 0.360–0.594 | not recorded |
    | 54.6 V | 0.386–0.561 | 0.371–0.546 |

  - If software applies the "calibrate the 3 % gain" correction, a hot A-leak at a low pack reads above 0.60, which is a
    false fail. That's fail-safe, but it is a nuisance.
  - The A-shorted rule ("within 2 V of pack") fails with the raw reading at full pack: 54.6 × 0.96 = 52.4 V. That too is
    fail-safe, because it falls into "other → fail", but it hides the diagnosis.
  - REV_B_PARTS line 116 still says "0.38 to 0.55" in the M6 row, and the B4 row says "0.33 to 0.60 (replaces)".
- **Fix:**
  - Delete the stale 0.38–0.55 text.
  - State that the window is applied to the gain-corrected MID (÷ Rin / (Rin + 33k), with Rin calibrated per board).
  - Widen the window to 0.33–0.62, or require pack ≥ 40 V for the test.
  - Apply the A-shorted rule to the corrected value.
- **Confidence:** high.

### m-2 (minor): "Keep total bus current under 90 A peaks" can't be met by measurement

- **Evidence.**
  - U1 latches off above 97 A for 147–383 µs (REV_B_PARTS).
  - INA228 limb meters convert over 50 µs to 4 ms with averaging, are read over I²C at host rate, and nothing meters
    RS1. Software can't see or react to sub-ms peaks.
  - A trip is a Cat 0 drop of every limb mid-motion, because LM5069-1 latches.
- **Fix:** turn the requirement into drive configuration. Set each drive's current and power limit parameters so the
  sum of worst-case bus currents of everything that can move at once is ≤ 90 A, and verify by readback at startup the
  same way as CanTimeout.
- **Confidence:** high.

### m-3 (minor): "The host blocks arming while the BMS shows charge current" is impossible on rev B

- **Evidence.**
  - REV_B_PARTS BMS row says the host blocks arming during charge.
  - The same file's accepted-risks table says "Reset can't be blocked by the Pi", and the netlist confirms the Pi has no
    path to RST_SRC, RUN or UVLO.
- **Fix:** reword it to "software refuses drive Enable over CAN while BMS telemetry shows charge current, or while
  telemetry is stale". Optionally tell the operator not to press reset while the charger is on.
- **Confidence:** high.

### m-4 (minor): The Pi can't tell which of several "bus off" states it is in, and there's no decode table

- **Evidence.**
  - The board reports only ESTOP_PI_N, VBUS (via V_Lx) and MID. Several states look alike.
  - Thermostat open, loop harness open, U8 SCP latched, and VCTL loss all read: not pressed, MID ≈ 0.47 P, VBUS ≈ 0.
  - U1 latched off (LM5069-1, cleared only by E-stop cycle then reset) reads: MID ≈ P, VBUS ≈ 0.
- **Fix (zero parts):** add a software state table to REV_B_PARTS:
  - pressed → E-stop;
  - not pressed, MID ≈ P, VBUS low → bank B faulted, so cycle E-stop then reset;
  - not pressed, MID ≈ 0.47 P, VBUS low → loop open (thermostat or harness) or U8 tripped: check the thermostat and
    wait for cooling.

  An optional 2-resistor RUN divider to a Pi GPIO (RUN about 7 V → about 3 V, behind a clamp) would split "latch off"
  from "U8 tripped".
- **Confidence:** high.

### m-5 (minor): The Pi ground shares J21's single GH pin with the Pi buck return, and the INA228's VIL is only 0.4 V

- **Evidence.**
  - INA228 datasheet: SDA/SCL VIL max 0.4 V, VIH min 1.2 V.
  - The Pi's ground reaches the board twice: J21.4 (JST GH, 28–30 AWG) and the Pi buck's return through J6.2
    (non-isolated buck, so input and output grounds are common).
  - Part of up to 2.4 A of buck input current returns through the GH pin. The Pi's VOL plus that ground offset eats into
    a 0.4 V low threshold.
- **Fix:**
  - Run SCL next to a GND pin. The order is currently 3V3, SDA, SCL, GND, so SDA sits beside 3V3: re-pin to SDA, GND,
    SCL, or twist each line with GND.
  - Keep the J21 harness short (≤ 300 mm) and run I²C at 100–400 kHz.
  - Scope the low level at an INA228 with the motors and the Jetson loaded at bring-up.
- **Confidence:** medium. The current split depends on harness lengths not yet defined.

### m-6 (minor): The FLT state of U8 while it is disabled or waking isn't in the datasheet, so close-order rests on U1's insertion delay

- **Evidence.**
  - TPS4810 datasheet: FLT "asserts low during short circuit fault, charge pump UVLO, input UVLO…". Shutdown disables
    "all the protection features". When FLT first asserts after EN rises is not specified.
  - RUN reaches U1 UVLO (3.8 V) through D10/R5 at the same instant as EN_A, and the UVLO node has no capacitor.
  - If FLT lags EN by more than nothing, UVLO briefly passes 2.45 V. LM5069's insertion delay (TIMER 4.7 nF, ms-scale)
    should mask that, but nobody has written this down or tested it.
- **Fix:** add a bring-up scope test: RUN, U1 UVLO, FLT_A and GATE at reset. UVLO must never stay above 2.45 V longer
  than the insertion delay before FLT_A goes low. Optionally put 10–47 nF on UVLO (with R6 33k that's a sub-ms RC) to
  remove the race outright.
- **Confidence:** medium.

### i-1 (improvement): No compute power-off after a clean shutdown

U7 (LM5069-2, auto-retry) has no enable. After a clean shutdown at the low-pack threshold, the Jetson, the halted Pi,
fans and SPARE_ON head electronics keep drawing from a nearly empty pack until the JK UVP opens. Options:
- If the JK add-on supports a discharge-FET-off command over RS485/CAN (verify it), the last step of shutdown sends it.
- Or add a latched off circuit on BRK_PG. It must latch, because a plain GPIO pull-down would release when the Pi dies
  and restart the converter.

### i-2 (improvement): The I²C address space is full once the legs are fitted

The nine meters (0x40, 41, 43, 44, 45, 47, 4C, 4D, 4F) use all nine non-SDA strap combinations. Any other 0x40–0x4F
device on that bus, such as a PCA9685 at 0x40, collides, and a tenth meter forces an SDA strap (the hold-time issue
again). Dedicate one Pi I²C controller to the PDB and record it as a software or interface requirement.

### i-3 (improvement): A doc error in the VBUS source note

REV_B_PARTS says "U11 reads V_L1, after the left-arm pigtail fuse". The fuse is inline in the pigtail, so V_L1 (the
board terminal, RS2 pin 4) is *ahead of* the fuse and equals VBUS minus the shunt drop. "Highest limb meter" still
works, but the stated reason is wrong.

### i-4 (improvement): ESTOP_PI_N reads wrong during boot

ESTOP_PI_N sits behind 103 kΩ, so the Pi 5's default boot pulls set its level until the runtime sets no-pull. That's
already a requirement. Also name the GPIO, and treat the pin as invalid until configured.

---

## Checked and OK

- **INA228 addresses.** The A1/A0 pins (1/2) per instance give L arm 0x40, R arm 0x41, aux 0x43, L leg 0x44, R leg
  0x45, SPARE_SW 0x47 (U10: VS, SCL), waist 0x4C, 12 V 0x4D (U6: SCL, VS), MID 0x4F (U17: SCL, SCL). They're unique,
  none use SDA, and they match REV_B_PARTS.
- **INA228 inputs with the Pi off.** The datasheet allows 0–85 V on the inputs "regardless of whether the device has
  power". Power-on default is continuous conversion with ADCRANGE 0 (±163.84 mV). Software must still program
  SHUNT_CAL, and ADCRANGE = 1 for U10.
- **Clamp threshold.**
  - Divider 499k + 604k / 51.1k, 2 × 9.53 MΩ hysteresis, LM4040 2.5 V, TLV1805 14 mV internal hysteresis.
  - I get 56.6–56.8 V on and about 55.8–55.9 V off, which matches the sheet's 56.8/55.8 once the internal hysteresis is
    included. This closes the rev B2 "56.6 vs 56.8" question.
  - TLV1805 SHDN on GND is the enabled state.
- **Latch drive levels.**
  - RUN is about 7.4 V. EN_A = (RUN − 0.7) × 100/110 ≈ 6 V, against EN 1.03 V and INP 2.02 V, and below the 70 V abs
    max.
  - UVLO is 3.8 V with FLT high-Z and about 1.94 V with FLT low (BAT46W + 10k), against LM5069's 2.45 V minimum.
  - SP_EN = 7 × 68/168 ≈ 2.8 V.
- **BMS auto-recovery or a pack brown-out doesn't re-arm the motors.** Power-up with reset open stays off (latch_sim
  cases), so a JK reconnect needs a fresh reset press.
- **Converter ON/OFF.** U7 PGD open (good) → Q12 on → BRK_ON to −Vin through 220 Ω → on. PGD low, a broken wire or no
  BRK_IN → off.
- **ESTOP_PI_N.** Levels are about 3.2 V released and 0 V pressed, with the BAT54S clamp the right way round (pin 1
  GND, 2 +3V3_PI, 3 node). A bus short leaves about 0.5 mA in the clamp.
- **No Pi-driven net can hold the banks or SPARE_SW on.** The "software can drop, never hold" rule holds, trivially,
  because software can do neither.
- **Self-test classifier fails safe.** Every misclassification lands in "other → fail".
