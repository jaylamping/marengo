# Rev B PDB review, lens 1: power path, protection, transients

Reviewer: Opus 5.5 (adversarial, power-logic lens). Repo: read-only, no edits.
Sources: gen/gen.py (the source of the netlist), REV_B_PARTS.md, REVB–REVB4 reviews, PRE_LAYOUT_AUDIT.md.
Sims: I copied tools/absorber_sim.py and tools/latch_sim.py to the scratchpad and removed only the Windows-only
`os.add_dll_directory` call. I ran them on Linux ngspice 42 (libngspice0). Results are in the "Checked and OK" section.

Earlier reviews already settled these, and I don't raise them again: the single-channel E-stop, the Q4/Q5 re-arm,
clamp capacity against 30 A of regen (the 5 A software cap), the BMS close surge into the absorber (10b gated on the
bench test), U9 as the only servo break, servo back-feed, the 90 A peak rule, the drive-capacitance ceiling, and the VBUS bleed.

---

## F1. MAJOR: A limb short trips the whole motor bus. The limb fuse is not what clears it.

**Assumption under test.** REV_B_CLEAN_SHEET rows 7 and 8 say "faults are the fuse's job". Trip order (Row 1
conditions, item 4) says "a limb fault blows its own fuse first".

**What the netlist does.** Every limb is fed from VBUS behind bank B (Q1/Q2, driven by U1 LM5069-1, RS1 = 0.5 mΩ).
No limb has its own electronic trip. The LM5069 power limit is active all the time, not just during start. With
the output shorted, VDS ≈ 54.6 V:

| U1 power limit (gen.py:58) | Current held in a short | Time before latch (timer 4.7 nF, 0.15–0.39 ms) | I²t through the limb fuse from this path |
| --- | --- | --- | --- |
| 480 W | 8.8 A | 0.39 ms | 0.03 A²s |
| 652 W | 11.9 A | 0.39 ms | 0.06 A²s |
| 810 W | 14.8 A | 0.39 ms | 0.09 A²s |

Before that, the pack current rises at 55–110 A/µs (0.5–1 µH). It reaches U1's breaker (160–260 A) or U8's RS1
comparator (158–242 A) within a few µs. So bank B, and often bank A too, opens and latches. Clearing it takes an
E-stop cycle and a reset.

A 0997020 (20 A MINI) needs about 380 A²s to melt (RATIONALE-log.md:285; please confirm on the Littelfuse 997 curve).
The pack path gives it about 0.1 A²s, which is more than 1000× too little.

The only thing that can blow the faulted fuse is the other drives' DC-link capacitance dumping backwards into the
short through the board's VBUS copper. That dump is I²t = C·V²/(2R). With about 30 mΩ of pigtail, fuse and shunt in
the path, it reaches 380 A²s only at 7.6 mF or more. That number is unmeasured (M11 in earlier rounds).

**Consequences**
- Any low-impedance limb fault drops every limb. With legs fitted, an arm short makes the robot fall.
- The fuse is often left intact. Reset then soft-starts U1 straight into the same short (power limit, timeout, latch
  again). The healthy limbs can't be brought back until someone finds the bad limb and unplugs it.
- Nothing on the board says which limb failed. The INA228s convert in about 1 ms, and the event is over in under 0.4 ms.
- The limb fuse only clears "soft" overloads: total bus current under U1's 97 A minimum limit, with VDS·I under 480 W.
  That is roughly a 30–75 A fault on one limb that doesn't pull the bus down. The clean-sheet premise is only half true.

**Fix (concrete)**
1. Correct the premise in REV_B_CLEAN_SHEET and REV_B_PARTS. A hard limb short trips the bus (U1 or U8). The fuse
   protects the pigtail against soft overloads and acts as backup for a failed-shorted bank.
2. Add a software and operator requirement. After a latch, read each limb INA228's CHARGE/ENERGY accumulators, or
   compare the last samples, to find the faulted limb. Pull that limb's fuse before pressing reset.
3. Before legs: either accept explicitly that "any limb short drops all limbs", or put back per-port electronic
   breakers (rev A's per-port LM5069 with a timer shorter than U1's) at least on the leg ports, so a limb fault
   stays local.
4. Bring-up: short one limb port behind its fuse, with drives connected, at reduced pack voltage. Record which
   protection acts and whether the fuse opens.

**Confidence:** high on the mechanism (LM5069 power limit and fault timer, U1/U8 thresholds from the sheet). Medium
on the fuse I²t figure.

---

## F2. MAJOR: Reverse polarity at the board's battery terminals depends on a TVS acting as a crowbar and on the JK's SCP.

**Evidence.** J1 (BAT) and J2 (GND) are identical Würth REDCUBE M4 studs (gen.py:40–41) taking ring lugs, so a lug
swap at assembly or service isn't prevented. Nothing on the main path blocks reverse current:
- Both banks have their drains toward the pack (gen.py:46–47, 160–161). Their body diodes conduct from VBUS to BAT,
  which is exactly the direction a reversed pack drives current.
- D1 (5.0SMDJ58A, unidirectional, BAT–GND) becomes the crowbar. Its datasheet gives VF 5.0 V max at 100 A, and IFSM
  300 A for an 8.3 ms half sine, about 374 A²s.
  - Prospective current is (54.6 − ~6 V) / 0.05–0.1 Ω, so about 490–970 A.
  - At that current D1 lasts about 0.4–1.6 ms. That is good enough only if the JK's short-circuit trip acts faster.
    I could not verify the JK-BD6A17S8P SCP threshold or delay.

**What sees the reverse voltage while D1 conducts** (BAT about −5 to −7 V)
- U1 VIN (SH_P) and U7 VIN (SH9_P) sit straight on BAT. LM5069 VIN abs min is −0.3 V, and the substrate diodes have
  no current limit apart from the pack itself, so expect damage.
- VBUS is about BAT + 2 body-diode drops, so about −4 to −5 V. That lands on:
  - C10–C13 (polymer, reverse-biased);
  - U11–U14 INA228 VBUS pins, which connect directly to V_Lx with no series resistor (abs min −0.3 V);
  - every drive.
- If D1 fails open, the full −54.6 V reaches all of these parts.
- What survives:
  - U2: blocked by D8 ES1D.
  - U8: TPS4810 VS is rated −65 V.
  - The DVE path, via its SK810L crowbar: F2 or the JK opens.

**Fix**
1. Key the terminals mechanically. Use different stud sizes (for example M5 for BAT, M4 for GND), or one polarized
   two-pole terminal. Or make the board-to-XT90S lead a fixed, made-once assembly that never comes off the studs in service.
2. Add to bring-up and service: check polarity at the lugs with a meter before closing the BMS.
3. Ask JK for the SCP threshold and delay (or measure them). Confirm the delay is short enough for D1 at about
   500–1000 A. If it isn't, the "crowbar" is a fuse that opens and leaves everything reverse-biased.

**Confidence:** medium. The topology is certain. The damage level depends on the JK SCP timing and the LM5069
substrate behaviour, which I didn't measure.

---

## F3. MINOR: The clamp-energy software cap can't be implemented as written.

**Evidence.** REV_B_PARTS (accepted risks) says: "Software tracks resistor energy as clamp on-time × V²/10 Ω from the
bus voltage." No clamp signal reaches the Pi. J21 carries only +3V3_PI, SDA, SCL, GND and ESTOP_PI_N (gen.py:117),
and CL_OUT/CL_G go nowhere else.
- The clamp is a hysteretic switch between about 55.9 and 56.6 V. With 272 µF and a net 5 A, one cycle takes about 40 µs.
  The INA228's roughly 1 ms conversion averages that away, so duty can't be recovered.
- A stuck-on Q6 (298 W) leaves VBUS at pack voltage, so VBUS can't show it at all. Only the thermostat catches that case.

**Fix.** Either:
- (a) Specify a conservative estimator: count any VBUS sample at 55.5 V or above as 322 W. Add a stuck-Q6 detector:
  BMS pack current − Σ limb currents − 12 V input current > 3 A while VBUS < 55 V means fault, so do a safe stop.
- (b) Wire CL_OUT through 100k to a Pi GPIO, with a 3.3 V clamp. J21 would go to 6 pins (2 parts).

**Confidence:** high.

---

## F4. MINOR: One thermostat for two parallel resistors leaves a latent gap.

**Evidence.** The 67L120 is on the +X plate only (REV_B_PARTS). The two HS50s share VBUS and CL_D, so in normal
running and with a stuck Q6 they share power evenly. That part is fine. The gap: if the +X resistor or its lead goes
open, nothing detects it. There's no clamp current sense, and the resistance check only happens "before first power".
After that, a stuck-on Q6 puts 149 W into the −X HS50 (rated 50 W on a 535 cm² plate) with no thermal trip.

**Fix.** Put a second 67L120 in series in the NC loop on the −X plate. It's about $10 and needs no board parts. Add
the 20 Ω / 10 Ω resistance check to periodic maintenance, not only first power.

**Confidence:** medium. It needs a latent fault plus a second fault, but the fix is nearly free.

---

## F5. MINOR: The clamp threshold on the sheet doesn't match the divider.

**Evidence.** The section 3 heading (gen.py:88) and the notes say 56.8 V on / 55.8 V off. From the netlist:
- divider 499k + 604k over 51.1k;
- hysteresis 2 × 9.53 MΩ from CL_OUT;
- LM4040 2.50 V, VCTL 12.1 V.

That gives **56.61 V rising and 55.91 V falling** (typical, with no comparator offset and no reference tolerance).
REVB2_REVIEW already flagged "the note's 56.8 V needs a recheck", and it hasn't been updated. This isn't a hazard:
the off threshold stays above the 54.86 V OVP pack, and 57.5 V stays a safe ceiling. But the absorber, the bus-cap
margin and the software estimator (F3) should use the real numbers.

**Fix.** Correct the notes. Put the worst-case band (offset and reference) in check.py.

**Confidence:** high.

---

## F6. IMPROVEMENT: Hot-plugging limbs and inserting fuses live

Per-limb soft start was dropped (clean sheet row 8). That makes fitting a limb fuse or mating a limb XT30 on a live
bus a hard close onto the drive's DC link from 272 µF plus the other drives. The resistive bound is 1.8–3.2 kA
(30 down to 17 mΩ), limited in practice by wire inductance. Results: contact arcing, possible input-ceramic ringing
toward 2× VBUS on the plugged drive, and a possible bus-wide U1/U8 trip.

**Fix.** Make it an operator rule in REV_B_PARTS: "Fit or remove limb fuses and connectors only with VBUS under 3 V
(E-stop pressed and bled down)."

**Confidence:** medium (drive input capacitance is unknown).

---

## Checked and OK

- **Absorber sim, reproduced on Linux ngspice 42.** Three cans at the worst corners (0.05 Ω, 0.5–1 µH, D1 at 71.2 V,
  C −20 %, trip 8.8–17.5 µs) give BAT 79.8–90.8 V, cutting 443–889 A. At 0.1 Ω loop the result is 68–75 V. This
  matches REV_B_PARTS. The sim's 54.6 V EMF is conservative against the 54.0 V (4.15 V/cell) pack. The BMS close
  into one can gives 194–227 A, and the 2 Ω option 25 A, as stated.
- **latch_sim:** 0 failures, all hFE and interlock cases, including a shorted B with a charged bus (blocked).
- **E-stop during regen:** the body diodes of both banks keep returning regen to the pack. VCTL (from BAT) and the
  clamp stay powered, and nothing interrupts the inductive lead current, so there's no new spike.
- **BMS opening, or the pack fuse or XT90 opening, during regen:** BAT is held via the body diodes, so U2, the latch
  and the clamp stay alive. Above 5.7 A the drives fault at 60 V. That is already accepted (REVB M6). On the board
  side, a low-side BMS opening shows up as a dip, not a spike; the inductive kick appears across the JK's FETs.
- **Stuck-on Q6, or a CL_D harness chafed to frame or GND:** CL_D is common to both resistors, so both heat equally
  (298 W total) and the +X thermostat sees it. Once the banks open, VBUS can't be fed from the pack (body diodes point
  toward the pack), so the clamp current stops. One HS50 open on its own doesn't raise the other's power (parallel, same V).
- **Bus short behind the banks:** U1's breaker or U8's comparator, with the absorber bounding BAT. U8's VS/CS/SRC are
  rated −65 to 100 V, so 91 V is within rating.
- **Shorts on BAT ahead of the banks:** only the JK SCP and Class T can clear these. That is by design; no board
  part could interrupt them.
- **Brick pigtail short:** U7 power limit (70 W, about 1.3 A at 54.6 V) with auto-retry. F2 is backup for a shorted
  Q7. A reversed DVE pair goes into the SK810L crowbar.
- **Clamp off threshold (55.9 V) against the pack:** above the 54.86 V OVP pack and above a 54.6 V charger CV, so
  the clamp never runs on a resting or charging pack.
