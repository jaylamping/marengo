# Left arm: software prep for testing

**Status:** brief for the software project, rev 2, 2026-11-08. Written by the hardware project. Joey pastes this page's link
into the software project as its starting brief.

**The plan (Joey, 2026-11-07/08):** the left arm comes up on a **fresh, lean stack** that follows the
[software tenets](../../../SOFTWARE_TENETS.md). The existing right-arm software is not extended. If the left arm passes its
tests on the new stack, the old code is deleted. A six-reviewer panel judged every crate, bin and tool against the tenets; the
reconciled verdicts are in [review/sw-foundation-20261108/RECONCILIATION.md](../../../../review/sw-foundation-20261108/RECONCILIATION.md).
The CAN layer choice comes from a cross-industry deep dive:
[can-stack-for-marengo.md](../../../docs/research/can-stack-for-marengo.md).

Hardware state, joint table and IDs: [hardware/STATUS.md](../../../STATUS.md).

## Start here

1. **Read the tenets.** They are the yardstick for every item below.
2. **Write the skeleton YAML** (item 1). No hardware needed; everything reads it.
3. **Lift the hard-won knowledge** out of the old code into notes and test vectors (item 2), before anything is deleted.
4. **Build the new CAN layer and its simulated drive together** (items 3 and 4), on `vcan` first.
5. **Buy one USB-CAN adapter**, confirm it, and bring the left drives up on it loose on the bench (item 5).
6. Then the actuator test stand tooling (item 7), and the camera loop (item 9).

## When the hardware is ready

| Hardware | State on 2026-11-08 | Software needed by then |
|---|---|---|
| Left shoulder and upper-arm drives (RS03 ×2, RS02), loose on the bench, 24 V supply | **Ready now** (drives owned, bench harness planned) | Items 1 to 6 |
| USB-CAN adapter for `can1` | Not bought. Buy **one** isolated STM32F0 board running mainline candleLight (Openlight CANable Pro class, ~$60), not a "CANable 2.0 Pro" G431 clone | Item 5 |
| Actuator test stand, RS03 waist unit first | Designed for all four sizes, waiting for Joey's review and print; luggage scale to buy | Item 7 |
| Upper arm (printed) | Printing | Item 8 before it moves under power |
| Elbow dummy-output rig | 6808 bearings arrive Mon 2026-11-09; rig is hand-loaded | **None.** No drive is used |
| Forearm and RS00 | Designed, not printed | Item 8, after the elbow rig passes |
| Camera rig (Logitech BRIO, AprilTag plates) | Plan approved; camera owned; plates not printed | Item 9 |
| Arm on the robot torso | Waits for Joey's rebuilt torso frame | Items 8 and 9 on the real geometry |
| Power board (rev B5) | Being routed; review, then fab | Item 10. The bench uses the 24 V supply and a physical E-stop until then |

## 1. The skeleton YAML (tenets 1 and 2)

One file describes the whole robot: limbs (left arm, right arm, left leg, right leg, waist), and in each, every joint with its
actuator type, bus, device ID, direction, zero, soft and hard limits, and torque, current and speed caps. Left arm: `can1`,
IDs 11 to 15, joint names `left_shoulder_pitch`, `left_shoulder_roll`, `left_upper_arm_yaw`, `left_elbow_pitch`,
`left_lower_arm_yaw`. Right arm stays `can0`, IDs 1 to 5. Unbuilt limbs are listed and simply show offline.

- The runtime never writes it. Tools that teach limits produce a proposed diff for a commit.
- It replaces `motors.yaml`, `control.yaml`, `robot.yaml`, `homing.yaml` and the humanoid templates. URDF and MJCF are for sim
  and dynamics only.
- Directions for the left joints are unknown until the sign check; leave them as placeholders.

## 2. Lift the hard-won knowledge first

The old code holds real bench lessons. Copy them into the new codec's docs and test vectors before deleting anything. The list,
with file paths, is in [RECONCILIATION.md, "Hard-won knowledge"](../../../../review/sw-foundation-20261108/RECONCILIATION.md#hard-won-knowledge-to-lift-before-deleting-anything):
ID layout and identity frames, the MIT scale table (RS03 ±20 rad/s measured), the TX allowlist, Set Zero timing, enable and stop
order, active reporting persisting across processes, the socket options, and the drive model in
`davout/tests/physical_firmware/` for the simulator.

**Open fact to settle:** RS00 MIT scales. Code uses ±50 rad/s / ±17 N·m, every RS00 manual says ±33 / ±14. Capture a loose RS00
before any forearm motion.

## 3. The new CAN layer (tenet 5)

Per [can-stack-for-marengo.md](../../../docs/research/can-stack-for-marengo.md):

- **One realtime process** owns every bus. One blocking thread per bus, SCHED_FIFO, pinned, memory locked. Each tick: send every
  command, collect replies until about 60 % of the period, publish a snapshot with each joint's age. Parameter traffic only in
  the slack. **250 Hz default**; 300 Hz only if measured.
- **Rust:** `socketcan` 4.0 (blocking socket, netlink, candump-log), `rtrb` for realtime to non-realtime handoff,
  `thread-priority`. No async runtime in the loop.
- **Codec:** our own, no I/O inside, a typed frame table transcribed from the [RobStride manuals](../../../docs/robstride/),
  per-model scales from the skeleton, test vectors from the manuals, cross-checked against the `motorcom` crate. Stay in the
  drive's default private protocol with active reporting off.
- **Linux:** bitrate and sample point set explicitly, error frames on, no automatic bus-off restart while moving, short TX queue.
  PREEMPT_RT only if the Pi 5 measurement misses its budget.
- **Only the realtime process transmits.** The one exception is the independent `disable` run by systemd after the process
  exits.
- **Tools it needs:** a drive scan (lists every ID answering on a bus) and a one-drive-at-a-time set-ID. The left drives may hold
  old IDs (12 on `can1`, 11 to 14 on `can0`).

## 4. Simulated drives on `vcan`

`vcan0` to `vcan4` plus one sim-drive process that speaks the RobStride protocol through the same codec, driven by MuJoCo, with
fault injection built in (silent drive, fault bits, VBUS out of range, reboot, dropped and delayed frames). The realtime binary
runs unchanged against it. There are no spare actuators, so every stop rule is proven here first.

## 5. Bring-up on the bench

- Buy one adapter; confirm `dmesg` shows `gs_usb` (not slcan) and `ip -details link show can1` lists it.
- Bench harness per [left-shoulder-bench-harness.md](../../../electrical/wiring/left-shoulder-bench-harness.md): 120 Ω at each end,
  about 60 Ω across CAN_H/CAN_L with power off, one ground tie.
- Scan, set IDs one drive at a time, then a CAN soak (error frames, overruns, bus load). Then measure round-trip latency and
  per-tick jitter on the Pi 5 at 250 Hz. Only then buy adapters for the other buses.

## 6. Safety: the three layers (tenet 3)

The panel found the **drive layer doesn't exist** in the old code. The new stack builds it first.

| Layer | What | Rule |
|---|---|---|
| Drive | `canTimeout` 0x7028 | Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above. Re-check after any drive power cycle or reset report |
| Drive | `limit_torque` 0x700B, `limit_cur` 0x7018, `limit_spd` 0x7017 | Per joint from the skeleton; never above the weakest printed part on that joint |
| Drive | Damper 0x702A (RS03 also 0x2026) | Read only; must read 0. Never write 1 |
| Drive | VBUS 0x701C, fault bits | 22 to 26 V on the bench supply; no faults present |
| Host | Silence | Stop the limb after 3 missed replies (well inside 50 ms); never hold drives energised after a drive goes silent |
| Host | Faults | Stop on any fault bit; never auto-clear and retry |
| Host | Limits | Soft limits 2 to 3° inside hard stops supervised, ≥ 5° unattended; total commanded torque bounded, backed by the drive's own limit |
| Host | Temperature | Pause at 60 °C, stop at 75 °C |
| Host | Hang | systemd `WatchdogSec` fed from the control thread; later the power board heartbeat (item 10) |
| Host | Bus-off | Stop the whole robot; no automatic restart while moving |

## 7. Actuator test stand tooling (T1 to T6)

Runner, logger and fitting scripts for [sim-to-real-loop.md](../../../docs/sim-to-real-loop.md) section 2.3: one drive alone on
its bus, command and reply at about 1 kHz, logging command, position, velocity, current (`iqf` 0x701A), temperature and kernel
timestamps; T1 friction and inertia, T2 reported vs real torque (operator types the luggage-scale reading), T3 backlash, T4
latency, T5 thermal, T6 excitation. Output: one file per actuator type, ready for MJCF `frictionloss`, `damping`, `armature`.

Limits: stalled holds stop at the **stall** rating (RS03 13 N·m, RS02 6 N·m; check RS00 and RS04), T5 stops at 60 °C, torque cap
= test maximum + 10 %, speed cap 1 rad/s except T1's speed passes. Order: RS03 (waist unit), RS02, RS00, RS04.

## 8. Limits for the arm under power

| Rule | Value | Source |
|---|---|---|
| Elbow torque cap | ≤ 17 N·m (printed stop design load), lower if the rig fails low | [ELBOW_DUMMY_OUTPUT_TEST.md](ELBOW_DUMMY_OUTPUT_TEST.md) |
| Elbow soft limits | 2 to 3° inside the 0° and 124° stops, supervised | same |
| Unattended soft limits | ≥ 5° inside every hard stop | [CALIBRATION_LOOP.md](CALIBRATION_LOOP.md) §9 |
| Never | run into a hard stop, hold a stall, exceed rated torque | same |

Operator rule, not software: keep side pushes at the hand under about 40 N until the rig's test 7 has run.

## 9. Camera calibration loop

Spec: [CALIBRATION_LOOP.md section 11](CALIBRATION_LOOP.md#11-spec-for-the-software-project). Order inside it: camera service
and stage C0 (arm unpowered), then the stage runner with section 9's stop rules tested on `vcan`, then C1 and C2 attended with
Joey. Reuses item 6's preflight.

## 10. Power board interface (later)

Needed before the arm runs from the power board and battery. Requirements:
[STATUS.md, power board section](../../../STATUS.md#power-board-and-battery-what-software-must-do) and
[REV_B_PARTS.md](../../../electrical/pdb/REV_B_PARTS.md#accepted-risks-and-software-requirements-joey-2026-11-07-rev-b5-rows-marked):
heartbeat toggled from the control loop every 50 to 100 ms, port-fault reporting, INA237 meters on one I²C controller (limb shunts
2 mΩ), self-test, brake clamp caps (30 W average over 60 s until the heat test, 3 kJ per burst), brown-out budget, E-stop input
debounce. Can be built against the sim now.

## Observability (tenet 8), all items

Every frame in and out goes to **MCAP** from the realtime process (raw, decoded, injected faults), with kernel timestamps;
`candump -L` runs per bus as an independent black box; health metrics (missed replies, latency, bus errors, temperatures) go to
Prometheus/Grafana. This replaces `marengo-store`, `marengo-log-cli` and the gateway log routes. A wider observability stack
choice (Grafana, Sentry, OpenTelemetry, Foxglove, Rerun) is still to be researched.

## Rules that hold for every item

- **No spare actuators.** Nothing overloads, overheats or stalls a real drive. Destructive tests use printed dummies.
- **Stationary and supported** until legs exist. The arm may drop onto the foam pad when a run stops hard; that's accepted.
- **The bench supply has no current limit.** 3 A fuse for first power-up, 10 A after.
- **No pointless tests** (tenet 6). A test guards a real failure: a bug we hit, a safety rule, a protocol contract.
- Hardware facts change: check [STATUS.md](../../../STATUS.md) before acting on a number here.

## Before the old stack is deleted

Move the right arm into the skeleton as a limb on `can0`, lift item 2's knowledge, and keep the `gravity-fit` and
`firmware-timing` tools and the commissioning docs. Then delete per the
[reconciled table](../../../../review/sw-foundation-20261108/RECONCILIATION.md#verdict-per-component-majority-of-six).
