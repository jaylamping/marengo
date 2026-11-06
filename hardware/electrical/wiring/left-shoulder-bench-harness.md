# Left shoulder bench harness (RS03 pitch, RS03 roll, RS02 upper-arm yaw)

**Status:** bench plan, rev 0 (2026-11-05). Not a final routed harness. No schematic, gauge or fuse value here is approved for the robot; they are bench choices to be confirmed against the parts Joey actually has.

**Scope:** power and CAN for the first left-arm actuators so the redesigned printed shoulder (P1 L01, P2 L04B, PF03) can move under power. The torso left shoulder pitch RS03 is included: P1 sits between pitch and roll, so a powered P1 test needs both.

Evidence labels used below: **repo** (in this repository), **handoff** (2026-11-05 arm handoff), **assumption** (not yet checked), **to measure** (Joey checks on the bench).

---

## 1. Sequencing

Wire and commission the actuators now, loose on the bench. Mount them on the printed parts and move them under power only after the gates in section 6.

| Step | Needs P2 fit check? | Why |
|------|---------------------|-----|
| Build bench harness, power up, scan IDs, set IDs, CAN soak | No | Actuators are clamped to the bench, not to printed parts |
| Single-joint unloaded jog, direction and zero check | No | Same |
| Loose fit of P2 + connector fit with real XT30 plug | Yes (that is the fit check) | L04B connector relief is only a CAD clearance until a real plug goes in |
| Mount P1/P2 on actuators, clamp, powered motion | Yes, plus fastening closed | Washers, limiters and under-head spacers are not owned (handoff); no torque spec; L04B is cleared only for one loose-fit print |

Final harness lengths wait on the routing studies (L05 to L11 unresolved, handoff). The bench harness uses generous service loops so it survives being moved onto the printed parts later.

## 2. Topology

**Trunk with drops** (Joey, 2026-11-05): one harness runs from the torso down the arm, inside it where the parts allow, with a short pigtail at each actuator. Connectors differ by motor (Joey, 2026-11-05): the RS02 has XT30(2+2) (power + CAN in one plug); the RS03 has a regular 2-pin XT30 for power and a separate GH1.25 2-pin connector for CAN (vendor manual, [../../docs/robstride/](../../docs/robstride/README.md)). Each motor has two ports; the harness uses one. No daisy-chaining through the motors' second ports.

```
 24 V bench PSU (current-limited)            Pi 5 + Waveshare 2-CH CAN HAT can1
   + ──[NC E-stop]──[10 A trunk fuse]──┐        H  L  GND ── to PSU − (single ground tie)
   − ──────────────────────────────┐   │        │  │
                                   ▼   ▼        ▼  ▼
   torso ═════ trunk: +, −, CAN_H, CAN_L (CAN pair twisted) ═════════════► down the arm
                     │                    │                     │
                 drop ≤ 15 cm         drop ≤ 15 cm          drop ≤ 15 cm
                     │                    │                     │
            XT30-F + CAN plug     XT30-F + CAN plug       XT30(2+2)-F   [120 Ω] trunk end
               M11 pitch RS03        M12 roll RS03         M13 upper yaw RS02
```

- **Bus:** `can1`. `can0` keeps the right arm (ids 1 to 5) so the two arms never share one MCP2515 and its 2-frame receive buffer (repo, ADR 0001). `can1` is already wired to the HAT and unused (repo, pi-commissioning.md).
- **IDs (proposal):** left arm block is 11 to 20 (repo, ADR 0001). Pitch **11**, roll **12**, upper-arm yaw **13**. An earlier left pitch used **12** on can1 and an earlier left 4-DOF used **11 to 14** on can0 (repo), so the drives may already hold old IDs: scan first, set IDs in firmware one motor at a time, then set zero.
- **CAN on a trunk:** at 1 Mbit/s stubs should stay short (ISO 11898, roughly 0.3 m or less), so keep each drop at 15 cm or less and the pair twisted to the splice. That keeps the bus a proper line with the terminator at the far end of the trunk.
- **Power on a trunk:** the trunk carries all three motors' current, so it is sized for the sum and fused once at the source. Per-motor fusing is dropped for the bench (simpler, fewer parts in the arm). Add drop fuses later only if a fault case needs them.
- **Extendable:** later actuators (elbow RS02, lower-arm yaw RS00) become more drops on the same trunk, and the terminator moves to the new end.
- **Crossing joints:** the trunk passes the roll and upper-yaw joints, so it bends and twists with them. This is the open routing problem from the handoff (L05 to L11: slack at pitch, turn at yaw). On the bench, leave visible service loops at each joint and note how much each one uses through the range; that measurement feeds the fixed-length harness.
- **Unused second port:** it is probably paralleled with the used one inside the drive, so treat its pins as live at bus voltage (**to measure** with a multimeter once powered). Cap or tape it so it cannot short against tools, screws or the printed parts.

## 3. Termination and ground

- 120 Ω at each end only: the HAT end and after M13 (repo, ADR 0001). The RobStride manuals do not say the drives carry a terminator (they mention one only on the vendor USB-CAN module), so measure before leaving the resistor out.
- Check the HAT's can1 termination jumper/switch is ON (**to measure**). Fit a 120 Ω 1/4 W resistor across H/L at the far end, heat-shrunk.
- Power off, all three motors plugged in: measure CAN_H to CAN_L. Expect about **60 Ω**. About 120 Ω means one terminator is missing; about 40 Ω means a motor or the HAT adds a third.
- Tie Pi/HAT GND to PSU negative at one point only. No ground loop through a second path.

## 4. Wire, connectors, lengths

| Run | Wire | Bench length | Notes |
|-----|------|--------------|-------|
| PSU to E-stop to trunk start | 14 AWG silicone, red/black | 1.0 m | E-stop breaks + (repo: physical NC E-stop is the only E-stop authority) |
| Trunk power, torso to past M13 | 16 AWG silicone pair | about 1.0 m | Sum of three motors at bench torque caps; XT30(2+2) rated 20 A at 16 AWG (handoff) |
| Trunk CAN, HAT to past M13 | 22 to 24 AWG twisted pair, ~1 twist per 25 mm | about 1.3 m | Runs alongside the power pair |
| Drops to M11, M12, M13 | 16 AWG pair + twisted CAN pair | 15 cm or less each | Spliced into the trunk, heat-shrunk, strain-relieved |
| Service loops | — | at pitch output, roll output, upper-yaw | Sized generously; measure what each joint uses |

Lengths are bench **assumptions**, sized long. Record the actual cut lengths here once built; they are the starting point for the fixed-length harness (work package 4).

**Mating connectors:** RS02: AMASS XT30(2+2)-F.G.B (90° exit) (handoff, header XT30PB(2+2)-M.G.B). RS03: AMASS XT30UW-F for power (board end XT30APW-M) plus a GH1.25-T 2-pin housing for CAN (board end GH1.25-2PWT) (vendor manual). Pinouts and diagrams are in [../../docs/robstride/README.md](../../docs/robstride/README.md); confirm them against the drive in hand before soldering.

Plug and unplug only with the E-stop open. The drives have input capacitance and XT30 contacts arc on hot-plug at 24 V (assumption, typical for these drives).

## 5. Parts list

| Qty | Part | Note |
|-----|------|------|
| 1 + 2 spare | AMASS XT30(2+2)-F.G.B female | RS02 drop |
| 2 + 2 spare | AMASS XT30UW-F | RS03 power drops |
| 2 + 4 spare | GH1.25 2-pin housing + crimp contacts (or pre-crimped GH1.25 2-pin leads) | RS03 CAN drops; pre-crimped leads avoid needing a GH crimper |
| 1 + spare | 10 A ATO fuse + inline holder | Trunk, at the source |
| — | Solder sleeves or inline splices for the drops | One power + one CAN splice per drop |
| 2 m each | 14 AWG silicone, red + black | Trunk |
| 2 m each | 16 AWG silicone, red + black | Trunk power and drops |
| 3 m | 22 to 24 AWG twisted pair (or 2-core shielded) | CAN |
| 2 | 120 Ω 1/4 W resistors | One spare |
| 1 | NC mushroom E-stop | Reuse the right-arm one if the left branch goes through it |
| — | Heat-shrink, cable ties, labels (M11/M12/M13, CAN1) | Label both ends of every lead |
| — | Bench clamps or a scrap board with M4/M3 screws | Holds loose actuators for unloaded tests |

The fuse value is for the 24 V bench with the low torque caps used on the right arm (5 Nm RS03, 3 Nm RS02, repo motors.yaml). It protects the wire and connectors, not the motor. Revisit for the 48 V pack.

Not wiring but needed before section 6: the shoulder fastener kit (M4 washers OD9/ID4.3, M3 washers, limiters, 3 mm under-head spacers, M3x12), none of which is owned yet (handoff).

## 6. First bench test

Run with Joey present. One motor at a time until step 5.

1. **Before power.** Continuity on each lead. No short between + and −. CAN_H to CAN_L about 60 Ω (section 3). CAN_H and CAN_L to GND open.
2. **Supply.** Set 24 V, current limit about 2 A. Close E-stop with only M11 plugged in. Expect a short inrush then a small idle draw; note it.
3. **Bus up.** `sudo ./scripts/can-up.sh can1` (or `ip link set can1 type can bitrate 1000000 && ip link set can1 up`). `ip -details link show can1` shows `clock 8000000` and state ERROR-ACTIVE.
4. **Scan and ID.** Read M11's ID and firmware. Set ID 11 if needed, then set zero. Repeat for M12 (12) and M13 (13) one at a time. Record firmware strings for `firmware_version`.
5. **All three on the bus.** 10 minute idle soak with feedback polling. Pass = no error frames, no RX overflow (both latch a transport fault, repo safety.md), TX/RX error counters stay 0.
6. **Unloaded jog.** Each actuator clamped to the bench, nothing attached. One joint at a time, low torque cap, slow: confirm it moves, confirm which way +q goes and write it down, check case temperature by hand after a minute.
7. **Stop.** Do not mount the printed parts under power yet.

**Gates before powered motion with printed parts:**

- P2 L04B loose fit passed, with a real XT30(2+2)-F plugged into the RS02 inside the connector relief.
- Fastener kit in hand and stacks closed (handoff work package 3).
- Unpowered hand rotation through the intended range with the harness attached: no rub, no cable pull, no pinch.
- A left-arm bench config on can1 (software side; `config/motors.yaml` is right-arm only and the humanoid template puts the left arm on can3 with ids 14+, which does not match this plan).
- First powered motion supported by hand, torque caps at or below the right arm's, one joint at a time.

## Open questions

- Is the current right-arm supply 24 V, and what is its current limit? (Playbook baselines are quoted at 24 V.)
- Does the left branch share the right arm's E-stop?
