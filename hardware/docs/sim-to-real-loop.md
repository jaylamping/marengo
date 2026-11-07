# Sim-to-real loop: refinements and the hardware they need

**Status:** plan, 2026-11-07. Joey asked for it after agreeing the basic loop stays: model → sim, sim, sim → real test on the robot → repeat. This doc adds the refinements that shorten each lap and make sim believable, and specs the hardware parts. The software parts are written as a spec for the software project (Joey carries it over).
**Related:** [compute-plan.md](compute-plan.md) (section 8, learning direction), [CALIBRATION_LOOP.md](../designs/left-arm/testing/CALIBRATION_LOOP.md) (the camera loop this builds on), [right-arm calibration suite](../../docs/commissioning/right-arm-calibration-suite.md), [RobStride manuals](robstride/).

Evidence labels: **vendor** (RobStride manual), **repo** (existing marengo doc), **estimate** (my number, to be measured), **assumption**.

## 1. The loop, refined

```
          ┌──────────────────────────────────────────────────────────────────┐
          │                                                                  │
          v                                                                  │
  robot model (URDF/MJCF + measured actuator models + weighed masses)        │
          │                                                                  │
          v                                                                  │
  train / tune in sim, randomized around MEASURED values      (refinement 3) │
          │                                                                  │
          v                                                                  │
  cross-check in a second simulator                           (refinement 4) │
          │                                                                  │
          v                                                                  │
  hardware-in-the-loop: real Pi software, simulated drives    (refinement 5) │
          │                                                                  │
          v                                                                  │
  staged real tests: low torque → full, legs on the gantry    (refinement 6) │
          │  scored automatically by camera + telemetry       (refinement 7) │
          v                                                                  │
  everything logged → replay the same commands in sim → per-joint gap score  │
          │                                                   (refinement 2) │
          └── gap tells us which part of the model to fix ───────────────────┘
                 (actuator model, refinement 1; masses; geometry)
```

| # | Refinement | What it buys | Owner |
|---|---|---|---|
| 1 | Measure every actuator type, fit sim to it, then a learned actuator model | Closes the biggest single sim-to-real gap (friction, backlash, latency, current-to-torque) | **Hardware** (rig, section 2) + software (fitting) |
| 2 | Real-to-sim replay with a per-joint gap score | Shows where sim is wrong instead of finding out from a failed test | Software |
| 3 | Domain randomization around measured values and their measured spread | Robust policies that don't go soft from over-randomizing | Software, using section 2's numbers |
| 4 | Second simulator cross-check | Catches policies that exploit one simulator's bugs | Software |
| 5 | Hardware-in-the-loop (HIL) | Catches timing, CAN and latency problems with nothing able to break | Software |
| 6 | Staged physical tests; legs on a support gantry | First leg tests without a fall; later the fall catcher | **Hardware** (gantry, section 3) |
| 7 | Automatic scoring | Lap time in minutes, unattended runs (the calibration loop's rules) | Software, camera rig already approved |
| — | Log everything, synced clocks | The day's data is the training set and the replay source | **Hardware** (storage, section 4) + software |

## 2. Actuator measurement plan (refinement 1)

### 2.1 Why

The drives report torque, but it's **computed from motor current** (iq × torque constant), not measured at the output (**vendor**: `iqf` 0x701A, torque constant listed per model). Gearbox friction, efficiency, backlash and compliance sit between that number and the real output torque, and they're exactly what sim gets wrong. The right-arm suite already found static friction about 3× the config value on the elbow and a forearm that fell at 2.46 rad/s against a planned 0.8 (**repo**). That's the gap this closes, once per actuator type, before legs make it dangerous.

### 2.2 What gets measured, per actuator type

| Parameter | Goes into sim as | Test |
|---|---|---|
| Coulomb + viscous friction vs speed, both directions | MJCF `frictionloss`, `damping` (or the actuator model) | T1 |
| Stiction (breakaway) | actuator model | T1 |
| Reflected rotor + gearbox inertia | MJCF `armature` | T1 |
| Real output torque vs reported torque, across the range | torque scale / efficiency | T2 |
| Backlash and torsional stiffness at the output | actuator model; calibration loop cross-check | T3 |
| Command-to-response latency (CAN + drive) | sim control delay | T4 |
| Thermal rise vs held torque | thermal limit model (sim + safety) | T5 |
| Rich excitation data | training set for a learned actuator model | T6 |

Vendor numbers to check against (**vendor**):

| Model | Rated / peak torque | Torque constant | Used for |
|---|---|---|---|
| RS00 | 5 / 14 N·m | 1.48 N·m/Arms | forearm yaw |
| RS02 | 6 / 17 N·m | 1.22 N·m/Arms | upper-arm yaw, elbow, ankles |
| RS03 | 20 / 60 N·m | 2.36 N·m/Arms | shoulder pitch/roll, waist, hip roll/yaw |
| RS04 | 40 / 120 N·m | 2.1 N·m/Arms | hip pitch, knees |

### 2.3 The tests

All tests run on one actuator alone on its own CAN bus, so the bus can carry a command and a reply at ~1 kHz (a lone drive on a 1 Mbit/s bus has ~10× the headroom of a 6-joint leg bus, compute-plan section 3). The drive's own active reporting bottoms out at 10 ms (**vendor**), so all high-rate data comes from the reply to each command, not active reporting.

- **T1 Free output (no load).** Velocity ramps and constant-speed passes from 0.05 rad/s to the speed cap, both directions; current-mode steps for acceleration. Friction curve from current at constant speed; inertia from current steps vs measured acceleration. Repeat cold and warm (after T5). About 15 min per unit.
- **T2 Torque truth.** A lever on the output with a **digital luggage scale** at the tip, the scale's other end tied to the stand: the drive pulls against the scale in steps from 10% to 100% of *rated* torque (never peak), holding 2 s at each. Real torque = scale force × lever length. Compare with reported torque. A second pass with weights hanging from the lever at known radius, swept slowly through gravity, gives the same check while moving.
- **T3 Backlash and stiffness.** Output locked to the stand with a printed clamp; command slow ± torque ramps to 30% of rated; the encoder change between the two directions is backlash, the slope is stiffness. Compared later with the camera's backlash measurement in calibration stage C3.
- **T4 Latency.** Small position and torque steps in MIT mode; latency = time from the command frame to the first reply showing a response, taken from the Pi's CAN timestamps.
- **T5 Thermal.** Hold 50% then 100% of rated torque against the T2 lever (stalled) until the temperature curve flattens or reaches 60 °C, whichever is first (the calibration loop's pause limit, **repo**). Gives the thermal time constant for sim and for software limits.
- **T6 Excitation for a learned actuator model.** Chirps and random multi-sine position targets in MIT mode with the weighted lever, at several Kp/Kd settings, logging command, position, velocity and current at the full rate. 20 to 30 minutes per type is enough to train the small network ANYbotics-style ([Hwangbo et al. 2019](https://arxiv.org/abs/1901.08652)).

**Which units:** the full set once per actuator type. Then a 5-minute T1 on every individual unit before it's installed, to catch an outlier. The two RS04s and the waist RS03 aren't installed yet, so they go first. The arm actuators are locked to the arms with no spares (Joey, 2026-11-07), so RS00/RS02/RS03 type tests run on a unit the next time it's off an arm, or in place on the arm with the link removed if that's easier.

**Safety:** torque cap at the test's stated maximum plus 10%, speed cap 1 rad/s except in T1's speed passes, E-stop within reach, nobody in the lever's swing. RS04 at rated torque is 40 N·m: the stand and lever are built for that (below).

### 2.4 The stand (print + fasteners, plus one $10 scale)

| Part | Make | Notes |
|---|---|---|
| Actuator cradle | Printed PETG, one per actuator size (RS00/02 share one if the flange allows) | Clamps the housing by its own mounting holes; bolts to a 2020 rail clamped to the bench |
| Output adapter | Printed PETG, 100% infill, per output flange | Bolts to the output flange with the actuator's own screw pattern |
| Lever | Short length of 2020 Joey already has, bolted to the adapter | 250 mm effective; holes at known radii for the scale hook and weights |
| Weights | Steel bolts/nuts/washers on an M8 rod, weighed on a kitchen scale | No custom weights |
| Output lock (T3) | Printed clamp from the lever to the rail | |
| **Digital luggage scale** (50 kg, 10 g resolution) | **Buy, ~$10–15**, unless Joey has one | The only external torque truth. At 250 mm, 50 kg = 122 N·m, so it covers RS04 rated (16 kg) with margin |

No bearings needed: the actuator's own output bearing carries the lever, and the lever loads are well inside the drive's rating.

### 2.5 Output of this plan

One file per actuator type in the repo with the fitted numbers and their spread across units, plus the raw logs (Git LFS). The software project turns it into MJCF parameters and the actuator model. When the robot's real behaviour drifts from the replayed sim (refinement 2), the first suspect is re-measured with the same tests.

## 3. Leg support gantry (refinement 6)

The robot is stationary and supported until legs exist, and fall safety is deferred until then (Joey's rule). This section is what "revisit when legs exist" becomes, written now so the pelvis design doesn't need a redesign to accept it.

### 3.1 What it has to do

| Mode | Use | Gantry carries |
|---|---|---|
| **Hang** | First leg motions, feet clear of the floor | 100% of the robot |
| **Assist** | First standing and weight shifting | An adjustable share (springs or a counterweight), feet on the floor |
| **Catch** | Walking tests | Nothing until a fall starts, then stops the robot before the knees or shell hit |

Robot mass target 38–42 kg (**repo**, status.md). Design load for the lift points and gantry: **2 kN** working (about 5× robot weight, covering a short catch drop), **estimate** to be checked against the drop height chosen.

### 3.2 Concept

- **Frame:** freestanding, so it doesn't need a ceiling anchor in the office. Two uprights and a top beam about 2 m high, with splayed feet. 2020 is too light at a 2 m span for a catch load; use 2040/4040 extrusion or a commodity steel option (a heavy-duty freestanding pull-up/power rack or an engine-hoist frame). Pick on price when legs are close. This is a test fixture, not robot structure, so buying the frame is fine; it can't be printed.
- **Line:** rated rope or webbing sling from the top beam through a small rated pulley to a cleat, so Joey sets the hang height and the catch slack by hand. Bungee or springs in series give assist mode.
- **Robot side:** a rated sling around, or a bolted bracket to, **two lift points on the pelvis 2020 frame** near the hip attach, above the combined centre of mass so the robot hangs upright. These lift points are the only part that touches the robot design: **the pelvis frame should include them from its first revision** (two M5/M6 tapped hardpoints or a through-bolt for a shackle), sized for the 2 kN case.
- **Later:** a treadmill under the gantry for walking tests, if wanted.

### 3.3 Buy list (when legs are close, not now)

Load-rated parts are a real need here because a 40 kg robot falling is exactly when printed or unrated parts let go: rated rope or sling, two rated carabiners or a shackle, one rated pulley, a cleat. About $50–100. The frame is $100–250 depending on extrusion vs a steel rack. No custom parts.

## 4. Logging and clocks (feeds refinements 2, 3, 7)

- **Storage:** an NVMe SSD on the Pi (official M.2 HAT+, plus a 256–512 GB SSD) and one in the Jetson dev kit's M.2 slot. Joint data at 200 Hz for 23 joints is small; camera video is what fills disks. Nightly offload to the PC. About $40–80 per board at current prices (**estimate**).
- **Clocks:** Pi and Jetson synced over their direct Ethernet link (chrony first; PTP if replay needs better than ~1 ms). Every log line carries the shared clock.
- **What to keep:** commands and drive replies, IMU, power-board current per limb, camera frames at holds or events, operator commands and the result. Same rule for bench and robot.

## 5. Spec for the software project

Joey to carry over; nothing here is decided for them.

1. **Actuator model:** take section 2's files; set MJCF `frictionloss`, `damping`, `armature` per joint; add a control delay; then train a small actuator network from T6 data and use it in sim training.
2. **Replay and gap score:** a tool that replays any logged real session's commands in sim and reports per-joint position, velocity and torque error over time.
3. **Randomization:** ranges taken from the measured spread across units and test runs, not guessed.
4. **Second simulator:** keep MuJoCo as primary (it's what `sim/` uses today); cross-check locomotion policies in one other (Isaac Lab on the 5080, or Genesis).
5. **Hardware-in-the-loop:** a SocketCAN `vcan` drive emulator that speaks the RobStride protocol and is driven by the simulator, so the real Pi binary, Davout included, runs unchanged against simulated drives.
6. **Automatic scoring:** reuse the calibration loop's camera and stop rules; every real test produces a pass/fail report.

## 6. Order and cost

| Step | When | Cost |
|---|---|---|
| Luggage scale + print the stand | Now (body work isn't blocked by it) | ~$10–15 + prints |
| T1–T6 on the two RS04s and the waist RS03 | Now, they're uninstalled | time |
| T1–T6 on RS00/RS02/RS03 types | Next time a unit is off an arm | time |
| NVMe on the Pi | With the next parts order | ~$40–80 |
| Pelvis lift points in the pelvis design | First pelvis revision | nothing extra |
| Gantry frame and rated line parts | When legs are close | ~$150–350 |

## 7. Joey's answers (2026-11-07)

1. Owns a kitchen scale, no luggage scale: **buy the digital luggage scale** (50 kg, 10 g resolution, ~$10–15) with the next order.
2. **Approved:** run T1–T6 on the uninstalled RS04s and the waist RS03 once the stand is printed.
3. Gantry type **waits for the new house**: garage workspace favours a steel rack or engine-hoist frame; indoors favours a lighter extrusion frame. The pelvis lift points are needed either way.
