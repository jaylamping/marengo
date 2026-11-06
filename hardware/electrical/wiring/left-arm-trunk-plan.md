# Left arm wiring trunk: design plan

**Status:** planning draft, rev 0 (2026-11-06). **Joey 2026-11-06 08:35Z: junction boards (3.2 and the per-motor telemetry) are ruled out.** **08:38Z: Joey chose option B (spliced trunk, inline drop fuses).** Build spec, wiring diagram and cut list: [left-arm-trunk/README.md](left-arm-trunk/README.md); sections below still describe the board where they have not been rewritten yet. Nothing here is approved or built. It sets the architecture so the arm, frame and power-board work can reserve space and interfaces. The bench harness for the first three motors stays as described in [left-shoulder-bench-harness.md](left-shoulder-bench-harness.md); this document is what replaces it on the robot.

Evidence labels: **repo** (in this repository), **vendor** (RobStride manual, [../../docs/robstride/](../../docs/robstride/README.md)), **assumption** (not yet checked), **to measure**, **proposal** (a choice for Joey to confirm).

---

## 1. Goals

1. A fault costs a fuse or a cheap harness segment, never an actuator (Joey, 2026-11-06).
2. One power port per motor off a fused trunk. Nothing passes through a drive (Joey, 2026-11-06).
3. Every part that wears out (the cable that bends at a joint) is a short, plug-in, replaceable piece.
4. Serviceable: any segment can come out without unbuilding a link.
5. Fits inside the links the arm thread is designing (Ø9 or 5 × 11 mm channel, repo upper-arm README).
6. The right arm is the same harness mirrored.

## 2. What the trunk has to carry

| Item | Value | Source |
|------|-------|--------|
| Bus | 13S pack, 46.8 V nominal, 54.6 V full | repo, custom-21700-pack.md |
| Drive limits | 24 to 60 V operating, 48 V rated | vendor |
| Headroom to the 60 V ceiling | 5.4 V at full charge | derived |
| Motors, shoulder to wrist | M11 pitch RS03, M12 roll RS03, M13 upper-arm yaw RS02, M14 elbow RS02, M15 lower-arm yaw RS00 | repo memory, IDs 14 and 15 are a **proposal** following ADR 0001's 11 to 20 block |
| CAN | `can1`, CAN 2.0, 1 Mbit/s | repo |
| Future | wrist and hand (no design yet) | — |

Bus current is not measured yet. Phase current (RS03 43 Apk, RS02 23 Apk, vendor) is much higher than bus current; bus current is mechanical power over bus voltage plus losses. A first estimate for the whole arm is a few amps typical and roughly 10 to 15 A in short peaks (**assumption**). Measuring it on the bench at 24 V and then at 48 V is the first job in section 9; fuse values and wire gauges below are provisional until then.

## 3. Architecture (proposal)

Five ideas, each one aimed at a specific way the arm can be hurt.

### 3.1 One trunk per arm, fused at the power board

The power board (owned by the "Plan the custom power and peripheral board" thread, plan at [../pdb/PLAN.md](../pdb/PLAN.md), main 0b6a6ef) gives each arm its own port: an XT30(2+2) female carrying VBUS, GND, CAN_H, CAN_L. Behind it sit a hot-swap switch with soft start (the precharge) and a 20 A electronic trip, a current meter, then a 15 A blade fuse as the backstop. Left and right arms never share a port. CAN1 (left arm) leaves the board through an isolated transceiver whose bus side references the trunk GND at the port, with a switchable 120 Ω at the board end. The port meter gives the measured arm current that sets the fuse values in section 4.

**E-stop on the robot:** the button switches only a low-current loop that gates the board's motor FET banks. It is never in series with the 48 V trunk current (a mushroom head is not rated to break 20 A+ DC at 54 V). The bench harness's series E-stop is fine at 24 V only.

### 3.2 Junction boards instead of splices

A small printed circuit board at each motor drop, mounted in the link on the upstream side of the joint it serves:

```
 trunk in ──► [ junction board ] ──► trunk out (to next joint jumper)
                 │  fuse (drop)
                 │  TVS on CAN_H/CAN_L
                 │  optional 120 Ω (jumper, fitted on the last board only)
                 ▼
           drop to motor (≤ 10 cm), one port
```

Why a board rather than spliced wires with inline blade fuse holders:

- The CAN bus passes straight across the board, so the only stub is the short drop to the motor. That keeps the bus a clean line.
- The drop fuse is on the board, so it is reachable and swappable without cutting heat-shrink.
- A TVS on each CAN line, plus the fuse, puts the protection in the cheap part. If a crushed cable shorts bus voltage onto CAN, the clamp and the fuse take it instead of the CAN transceiver in every drive on the bus (see 3.5).
- Every board is the same part. Five per arm plus spares, so they are cheap to order and swap.
- The termination resistor lives on the last board, on a jumper, so adding the wrist later means moving one jumper.

**Per-motor telemetry (proposal, follows Joey's "know at all times how many amps/volts we are pulling and where"):** each junction board carries a current/voltage sense chip (INA-class, shunt after the drop fuse) and a small MCU with a CAN controller (STM32G0/G4-class). It reports drop current, bus voltage, board temperature and fuse state (voltage after the fuse, so a blown fuse is named rather than inferred) at a low rate (10 to 50 Hz) to keep bus load small; the power board's 1 kHz limb meter stays the fast measurement (pdb PLAN.md section 3.9, 79d1e88). Node IDs: the left-arm block 11 to 20 has room, so junction boards take **16 to 20** (JB-11 → 16 … JB-15 → 20), framed so they cannot collide with RobStride's extended-ID protocol. That framing and the bus-load budget go to the code project. The sense and MCU parts are optional fits: the board works as a plain fuse and clamp board without them.

Rough size: 30 × 18 × 8 mm with connectors (**assumption**, to check against the link cavities with the arm thread). Splices with inline ATM holders stay as the fallback if a board cannot fit somewhere.

### 3.3 Joint jumpers: the wear part is a plug-in cable

The trunk is not one long cable. It is static segments inside links, joined by short **jumpers** that cross each joint:

```
power board ═ T0 ═ [JB-11]─M11 ~J1~ [JB-12]─M12 ~J2~ [JB-13]─M13 ~J3~ [JB-14]─M14 ~J4~ [JB-15 +120Ω]─M15 ~J5~ (wrist, later)
  ═ static segment    ~ dynamic jumper (crosses a joint)    ─ drop to one motor port
```

| Piece | Lives in | Crosses | Static or dynamic |
|-------|----------|---------|-------------------|
| T0 | torso, power board to the shoulder | — | static |
| JB-11 | torso, at the pitch RS03 top-rear flat | — | static, feeds M11 |
| J1 | pitch jumper | pitch, −50° to 180° (230°) | dynamic |
| JB-12 | P1, at the roll RS03 housing | — | static, feeds M12 |
| J2 | roll jumper | roll, −15° to 180° (195°) | dynamic |
| JB-13 | P2, at the upper-arm yaw RS02 housing | — | static, feeds M13 |
| J3 | yaw jumper | upper-arm yaw, ±90° (twist) | dynamic |
| JB-14 | upper arm, near the elbow RS02 upper port | — | static, feeds M14 |
| J4 | elbow jumper | elbow, 0° to 133° (main d6cae06); band X 121.7 to 131.2 inboard of the fork cheek, 28.2 mm shell margin, sweep clean | dynamic |
| JB-15 | forearm, at the RS00 | — | static, feeds M15, carries the terminator |
| J5 (later) | wrist jumper | lower-arm yaw, ±60° (twist) | dynamic |

Joint ranges are the provisional URDF limits (repo, `hardware/designs/left-arm/urdf/left_arm.urdf`).

Each actuator's housing sits on the link before the joint it drives, so every drop is static and only the jumpers move. A jumper that fatigues after a few thousand cycles is a five-minute swap, not a rebuild. Jumpers use flex-rated wire (section 5); static segments can use ordinary silicone.

### 3.4 Same four conductors the whole way

VBUS, GND, CAN_H, CAN_L. Nothing else runs down the arm. The wrist and hand, when they exist, take their logic power from a small buck converter on the last junction board, so no 5 V or 12 V run has to cross four joints.

### 3.5 What actually kills a drive, and where it gets stopped

| Threat | What it does | Where it is stopped |
|--------|--------------|---------------------|
| Short in a cable or drop | Melts wire, burns connector | Drop fuse on the junction board, then trunk fuse |
| Bus voltage onto CAN (crushed cable, XT30(2+2) plugged in skewed, stray screw) | Kills the CAN transceiver in every drive on that bus | TVS + series protection on each junction board; CAN and power pairs kept apart in the jumpers; keyed connectors |
| Regen overvoltage (arm back-driven or braking hard at full charge) | Pushes the bus over 60 V; only 5.4 V headroom at 54.6 V | Active regen clamp at 57.5 V into a brake resistor on a frame rail (power board PLAN.md) |
| Hot-plug inrush and arcing | Pits contacts, spikes the bus | Hot-swap soft start on the power board port; plug only with the E-stop open |
| Reverse polarity | Instant drive damage | Keyed connectors everywhere; no bare pigtails on the robot |
| Ground offset between Pi and drives | CAN errors, transceiver stress | Single star ground on the power board; isolated CAN transceiver referenced to trunk GND |

No fuse protects a drive from its own internal failure. The fuses protect the wiring, connectors and the other drives.

## 4. Fusing (provisional)

| Fuse | Value | Type |
|------|-------|------|
| Arm port, on the power board | 20 A electronic trip, then 15 A blade backstop | power board PLAN.md |
| Drop, RS03 (M11, M12) | 7.5 A | fast-blow, on junction board |
| Drop, RS02 (M13, M14) | 5 A | same |
| Drop, RS00 (M15) | 3 A | same |

The trunk fuse has to stay intact when one drop fuse blows (selectivity). A 2:1 ratio in current rating is the usual starting rule; the RS03 drops sit at 2:1. This gets confirmed with a bench short test on a dummy load (section 9), never on a real actuator.

Fuse format on the board is open: mini-blade (ATM) is the most familiar and easy to replace by hand but large; a small PCB fuse in a clip holder is smaller. **Proposal:** pick after the first board layout shows how much room the links have.

## 5. Wire and connectors (provisional)

| Run | Power | CAN | Notes |
|-----|-------|-----|-------|
| T0 and static segments | 16 AWG silicone | 24 AWG twisted pair | As in the bench harness spec |
| Jumpers J1 to J3 (shoulder and upper arm) | 16 AWG, ultra-fine strand, flex-rated (PTFE or TPE jacket, not silicone, which tears) | 24 to 26 AWG twisted, flex-rated | Loose wires, not a bundle, through tight bends |
| Jumpers J4, J5 (elbow, wrist) | 18 AWG flex-rated | same | Only RS00 and the future hand are downstream (**assumption** until current is measured) |

Bend radii and torsion lengths are already in the bench harness doc (static ≥ 5 × OD, dynamic ≥ 10 × OD, 150 mm free length per ±90° of twist).

**Connectors (proposal, part numbers to verify):**

- Drop to motor: whatever the drive takes. RS02 and RS00: AMASS XT30(2+2)-F; RS03: XT30UW-F plus GH1.25 2-pin (vendor).
- Junction board to jumpers: one 4-way keyed connector per side. Candidate: the same XT30(2+2) family, which carries 20 A power and the CAN pair in one plug (vendor). Check that a PCB-mount male and an inline female exist in the size needed. Fallback: XT30 for power plus a latching 4-pin (JST-GH or Molex Pico-Lock) for CAN.
- Every connector latches or is retained, so arm vibration cannot back it out. An Oct 4 loss of motor ID 5 on the right arm was blamed on a loose harness at the elbow (repo memory).

## 6. The hard part: crossing pitch and roll

The pitch and roll RS03s have no through-bore (vendor), and their ranges are large: 230° at pitch, 195° at roll. Yaw (±90°) and elbow (135°) already have candidate routes in the upper-arm README; the shoulder has none.

Options for J1 and J2 (rendered in [options/TRUNK_OPTIONS_SHOULDER_CROSSING.png](options/TRUNK_OPTIONS_SHOULDER_CROSSING.png); the drop architectures are in [options/TRUNK_OPTIONS_TOPOLOGY.png](options/TRUNK_OPTIONS_TOPOLOGY.png)):

| Option | How | Good | Bad |
|--------|-----|------|-----|
| A. Spiral coil around the housing | Jumper coils 2 to 3 turns in a ring around the RS03 (r 55 to 75); inner end on a sleeve that turns with the link, outer end fixed. The joint tightens or loosens the coil. | Round flex wire works: coil radius ~50 mm is above the 30 mm dynamic minimum; fits inside the r78 shoulder cavity | ~14 mm ring plus a rotating sleeve and cover; most design work |
| B. Flat-flex clock-spring cassette | Flat ribbon wound in a thin cassette, steering-wheel style | Smallest (~8 mm ring, r 52 to 62), longest flex life | Flat cable for 15 A at 60 V is special or custom; harder to repair |
| C. Free service loop | Slack loop beside the joint | Simplest; fine for the bench | Snags, pinches or rubs over 230° (L05 to L11 never closed it) |
| D. Rotary cable carrier | Drag-chain links guide a U-turn arc | Off the shelf, proven | U-turn needs ~2× bend radius, so the arc reaches r ~110+, outside the cavity and shell; heavy |

A rolling-loop groove with round wire (the "wrap groove" in rev 0 of this plan) is dropped: its U-turn needs a gap of twice the 30 mm bend radius, about 60 mm. That is why A coils the wire instead.

**Space check (arm thread, 2026-11-06):** neither r 52 to 78 band is free today. At pitch, the RS03 sits inside the torso on its bracket (X ±78.4 to ±113, bolted to the step and top side rails and the upper posts). The only free annulus is outboard of X ±135, where P1 and the r78 shell cavity are, and a 14 mm ring there would push the arm out 14 mm. At roll, P1 L01 clamps the housing on the r49 screw ring and fills much of r 52 to 78. CAD will measure the real free annuli; until then, A is unproven, and B (thinner) or a shrunk A are the likelier fits.

**Proposal (pending that measurement):** A for both pitch and roll, modelled with the arm thread once SolidWorks is free, with B as the upgrade if A is too big. Either way the jumper stays a plug-in part.

### Elbow crossing (J4), with the arm thread

The inboard band (X 121.7 to 131.2) works but sticks out 16.8 mm past the forearm and undoes the elbow slimming, so it is the fallback. Lead route: posterior, across the back of the elbow. The RS02 body and Ø84 carrier put the wires at r 45 to 55 from the axis, so the path grows about 120 to 130 mm from 0 to 133°. That length is stored as a **guided rolling loop** in the upper-arm tube (U-turn ~50 to 55 mm across, ~70 mm axial travel, legs ~3 × 8 mm, smooth lining or rails), not a free S-loop, which kinks when pushed back. The wires wrap onto a guide surface on the posterior carrier during flexion, like a pulley. JB-14 sits at the shoulder end of the tube, clear of the loop. Second fallback: small clock-spring between the RS02 end face and the fork cheek. CAD check pending (free posterior radius, tube travel).

## 7. Interfaces this needs from other threads

| Thread | What we need |
|--------|--------------|
| Plan the custom power and peripheral board | Agreed (0b6a6ef): one XT30(2+2) port per arm, soft start, 20 A e-trip, meter, 15 A backstop, 57.5 V regen clamp, star ground, isolated CAN1 with board-end 120 Ω |
| Finalize the internal torso frame | A place for T0 from the board to the shoulder and JB-11 at the pitch top-rear flat |
| Design the left arm from scratch | Pockets for JB-12 to JB-15 (about 30 × 18 × 8 mm each) on the upstream side of each joint; clamp points at both ends of each jumper; the wrap groove at pitch and roll; access to each junction board's fuse |
| Wire the RS03/RS02 shoulder actuators | Bench harness keeps its splice-and-inline-fuse build for first power-up; its measured lengths, service-loop use and bus current feed this plan |
| Body shell | No shell over the shoulder actuators (standing rule), so the wrap groove is visible there; style it rather than hide it |

## 8. Software touchpoints (flag only, code project owns them)

- `config/motors.yaml` is right-arm only; the humanoid template puts the left arm on `can3` with IDs 14+, which does not match `can1` with 11 to 15.
- **Junction-board telemetry frame (proposal, with the power board thread; code project owns the final spec):** standard 11-bit IDs so they cannot collide with RobStride's 29-bit extended frames. One 8-byte frame per node at 50 Hz: int16 drop current (mA), uint16 bus voltage after the fuse (10 mV), int16 temperature (0.1 °C), uint8 flags (bit0 fuse open, bit1 sense fault, bit2 over-temperature), uint8 rolling sequence counter (host spots dropped frames and stale nodes). Plus a 1 Hz status frame with firmware version and uptime. Timestamps come from the CAN bridge, not the node. Reconciliation against the 1 kHz limb meter runs on the host.
- A drop fuse blowing looks to software like one motor dropping off the bus. The controller should treat that as a fault and hold the rest of the arm safe, not keep commanding.

## 9. Validation plan

Following the standing method: build simple, test for a baseline, then iterate. No destructive test ever uses a real actuator; a dummy load board (resistor bank plus a CAN node) stands in.

1. **Measure bus current** on the bench (24 V now, 48 V when the pack exists) for each motor through representative moves, with a clamp meter or shunt logger. Sets fuse values and gauges.
2. **Fuse selectivity**: short a drop on the dummy load, confirm only the drop fuse opens. Uses the power board's test mode (PLAN.md section 3.8, 254e2f9): aux port as test port with the plug-in load box, adjustable trip from 1 A, and every trip logged with peak current and time to open.
3. **CAN fault injection** on the board's spare bus (CAN6) fault header, motors disarmed: H-L short, CAN_H or CAN_L to GND, VBUS on CAN_H through 100 Ω. Put a junction board and a dummy CAN node on that bus, and confirm the clamp holds and the bus recovers.
4. **Jumper flex life** on a printed jig that cycles each joint's motion, with a continuity monitor on every conductor. Target to be set; a starting point is 100k cycles at full range.
5. **Connector retention** under vibration with the arm moving.
6. **Thermal**: trunk and drops at the measured peak current for a few minutes, temperature by hand or thermocouple.
7. Adversarial review at gate 3 (before anything powered) with the review recipe.

## 10. Open questions for Joey

1. Junction boards (small custom PCB at each drop) or splices with inline fuse holders?
2. OK with the jumpers as separate plug-in wear parts at every joint?
3. Pitch and roll crossing: start with the wrap groove (A)?
4. One E-stop for the whole robot (it gates all motor FET banks on the power board), or a separate stop per arm?
5. When the E-stop cuts arm power the arm goes limp and falls. Is that acceptable for now, or should the stop be a controlled one (software damping first, power cut after)?
