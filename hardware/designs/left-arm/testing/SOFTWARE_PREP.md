# Left arm: software prep for testing

**Status:** brief for the software project, 2026-11-07. Written by the hardware project from the test plans in this repo.
Nothing here is built. Joey pastes this page's link into the software project as its starting brief.

**What this is:** the software work the left arm's hardware tests need, ranked by when the hardware needs it. For each
item: what to build, what it needs from hardware, what it unblocks, and when the hardware is ready. The hardware state
and the full joint/CAN table live in [hardware/STATUS.md](../../../STATUS.md); this page doesn't repeat them.

**Good news first:** most of the stack already exists from the right arm (`marengo-pi`, Davout, the robstride driver,
`motor-repl`, `marengo-pi-mcp`, the [limb playbook](../../../../docs/commissioning/limb-playbook.md) and the
[right-arm calibration suite](../../../../docs/commissioning/right-arm-calibration-suite.md)). The left arm mostly needs
configuration, a stricter drive preflight, and new test tooling, not a new runtime.

## Start here

1. **Config and names** (item 1). An evening of work, no hardware needed, and everything else reads it.
2. **`can1` bring-up and drive preflight** (items 2 and 3). The drives can be commissioned loose on the bench today; this
   is the first session with Joey at the bench.
3. **Simulated drives on `vcan`** (item 4), in parallel. It lets items 2, 3, 5 and 8 be tested before any arm part is
   powered, and it's the only safe way to test stop rules.
4. **Actuator test stand tooling** (item 5). The stand is designed; it's the next powered hardware.
5. **Camera calibration loop** (item 7). The biggest piece; start once 1 to 4 are moving.

Items 6, 8 and 9 follow the hardware as it arrives.

## When the hardware is ready

| Hardware | State on 2026-11-07 | Software needed by then |
|---|---|---|
| Left shoulder and upper-arm drives (RS03 ×2, RS02), loose on the bench, 24 V supply, `can1` | **Ready now** (drives owned, bench harness planned) | Items 1, 2, 3 |
| Actuator test stand, RS03 waist unit first | Designed and sliced for all four sizes, waiting for Joey's review and print; luggage scale to buy | Item 5 |
| Upper arm (printed) | Printing now | Item 6 (soft limits and caps) before it moves under power |
| Elbow dummy-output rig | 6808 bearings arrive Mon 2026-11-09; rig is hand-loaded | **None.** The rig uses weights and a torque wrench, no drive |
| Forearm and RS00 | Designed (sleeve on two 6808s, chosen 2026-11-07), not printed | Item 6, after the elbow rig passes |
| Camera rig (Logitech BRIO, AprilTag plates) | Plan approved; camera owned; nothing printed or mounted | Item 7 |
| Arm on the robot torso | Waits for Joey's rebuilt torso frame to be assembled and validated | Items 6 and 7 on the real geometry |
| Power board (rev B5) | Being routed; review, then a fab order | Item 9. The bench uses the 24 V supply and a physical E-stop until then |

## 1. Left arm config and joint names

**Build:** add the five left joints to `config/motors.yaml`, `config/control.yaml`, `config/robot.yaml` and
`config/homing.yaml`, using the hardware names: `left_shoulder_pitch`, `left_shoulder_roll`, `left_upper_arm_yaw`,
`left_elbow_pitch`, `left_lower_arm_yaw`. Bus `can1`, device IDs 11 to 15 (14 and 15 are proposals). Add `left_arm`
parameters to the limb playbook (`gcomp_poses`, `wave_pose`, `torque_only_tau_cmd`).

**Fix on the software side:** `robot.yaml` still lists `left_elbow` / `left_wrist`; `motors_humanoid.yaml` puts the left arm
on `can3` with other IDs. Details in [STATUS.md, known mismatches](../../../STATUS.md#left-arm-joints-urdf-can). Hardware has
settled the "does the left arm mirror the right" question (status.md open decision 2): yes, same five joints and the same
positive directions as the right arm.

**Needs from hardware:** done. Joint list, IDs and limits are in STATUS.md and [left_arm.urdf](../urdf/left_arm.urdf).
`direction` signs are unknown until the sign check (item 3), so leave them as placeholders.

**Unblocks:** everything else.

## 2. `can1` bring-up tooling

**Build:** make sure `motor-repl`, `marengo-candump` and the Pi MCP tools work on `can1` by name, not only `can0`. Add or check:

- an ID scan that lists every drive answering on a bus, with its firmware version;
- a one-drive-at-a-time "set device ID" flow. The left drives may still hold old IDs: an earlier left pitch used 12 on
  `can1` and an earlier left 4-DOF used 11 to 14 on `can0` ([hardware ADR 0001](../../../docs/decisions/0001-can-and-motors.md));
- a short CAN soak report (error frames, RX overruns, bus load) per bus.

**Watch out:** `can1` is the second channel of the same MCP2515 HAT, so the open RX overrun **cause B** (status.md) applies
here too. The hardware plan's answer is an isolated USB-CAN adapter (gs_usb, SocketCAN) per bus
([compute-plan.md](../../../docs/compute-plan.md) section 3), not bought yet. Code should treat the bus as a name from config
so the left arm can move from the HAT to an adapter with no code change. If the soak shows cause B on `can1`, that's the
evidence to buy the first adapter.

**Needs from hardware:** the bench harness ([left-shoulder-bench-harness.md](../../../electrical/wiring/left-shoulder-bench-harness.md)):
120 Ω at the HAT and after the last drive, about 60 Ω across CAN_H/CAN_L with power off, single ground tie. Joey builds it.

**Unblocks:** item 3, then all powered tests.

## 3. Drive preflight, then sign and zero

**Build:** a preflight that runs before every enable, on every drive, and refuses to enable on any mismatch:

| Check | Parameter (vendor manuals in [hardware/docs/robstride/](../../../docs/robstride/)) | Rule |
|---|---|---|
| Host-loss timeout | `canTimeout` 0x7028 (20000 = 1 s) | Written to **50 ms or less** and read back. Refuse enable if 0 or above the limit. Re-check after every drive power cycle. The right arm still carries 600 (~30 ms) from PR #254 with no owning policy (status.md M02/M03) |
| Torque cap | `limit_torque` 0x700B | Per joint, from the stage plan; never above the weakest printed part on that joint |
| Current cap | `limit_cur` 0x7018 | Consistent with the torque cap |
| Speed cap | `limit_spd` 0x7017 | ≤ 1.0 rad/s for first motion |
| Anti-backdrive damper | 0x702A (RS03 also 0x2026) | **Read only.** Must read 0 (on). Software never writes 1 |
| Bus voltage | VBUS 0x701C | 22 to 26 V on the 24 V bench supply |
| Faults | fault bits | None present. Never auto-clear a fault and retry |

Then run limb playbook chapters 1 to 3 (reference, limits confirm, sign) for `left_arm`, first on loose drives, then on the
printed parts.

**Needs from hardware:** loose drives clamped to the bench (ready now). On the printed parts: the upper arm print, the fastener
kit, and the shoulder on Joey's rebuilt torso.

**Unblocks:** any powered motion, and the calibration loop's preflight (item 7 reuses this).

## 4. Simulated RobStride drives on `vcan`

**Build:** a SocketCAN `vcan` drive emulator that speaks the RobStride CAN 2.0 protocol (type-2 feedback, parameter reads and
writes, enable/disable, faults), driven by MuJoCo, so the real Pi binary with Davout runs unchanged against simulated drives
([sim-to-real-loop.md](../../../docs/sim-to-real-loop.md) section 5, item 5). The robstride crate already has `vcan` tests to build on.

**Why now:** the left arm has no spare actuators, and the stop rules (item 7), the preflight (item 3) and the power board
interface (item 9) all need fault injection that is unsafe on real drives: a drive that stops answering, a fault bit, VBUS out
of range, a camera that loses its tag.

**Needs from hardware:** nothing new. Use `left_arm.urdf` for the model (item 8).

**Unblocks:** testing items 3, 5, 7 and 9 before the hardware exists.

## 5. Actuator test stand tooling (T1 to T6)

**Build** the runner, logger and fitting scripts for [sim-to-real-loop.md](../../../docs/sim-to-real-loop.md) section 2.3:

- one drive alone on its own bus, command and reply at about 1 kHz, logging command, position, velocity, current (`iqf`
  0x701A), temperature and Pi CAN timestamps. All high-rate data comes from the reply to each command, since the drive's own
  active reporting bottoms out at 10 ms;
- T1 friction, stiction and inertia; T2 reported vs real torque (operator types the luggage-scale reading); T3 backlash and
  stiffness; T4 latency; T5 thermal; T6 chirp and multi-sine excitation;
- the output: one file per actuator type with fitted values and their spread, ready to become MJCF `frictionloss`, `damping`
  and `armature`.

**Hard limits for the runner** (no spares): stalled holds stop at the **stall** rating, not peak (RS03 13 N·m, RS02 6 N·m;
check RS00 and RS04 in their manuals); T5 stops at 60 °C; torque cap = the test's maximum + 10 %; speed cap 1 rad/s except T1's
speed passes. T2 lever angle: arm = r cos θ from the drive's encoder, referenced to a level lever
([stand README](../../actuator-test-stand/README.md)).

**Order (Joey):** RS03 (the spare waist unit) first, then RS02, RS00, RS04.

**Needs from hardware:** stand printed and assembled, luggage scale bought. Joey's review of the stand is the next step.

**Unblocks:** a measured actuator model for sim, and the friction numbers the right-arm suite found to be about 3× off.

## 6. Limits and caps for the arm under power

**Build:** config values and Davout checks for the first powered moves on the printed arm.

| Rule | Value | Source |
|---|---|---|
| Elbow torque cap | ≤ 17 N·m (the printed stop's design load); lower it if the rig test fails low | [ELBOW_DUMMY_OUTPUT_TEST.md](ELBOW_DUMMY_OUTPUT_TEST.md) |
| Elbow soft limits | 2 to 3° inside the 0° and 124° hard stops for supervised bench use | same |
| Soft limits for unattended runs | ≥ 5° inside every hard stop | [CALIBRATION_LOOP.md](CALIBRATION_LOOP.md) section 9 |
| Starting torque caps | `tau_ff_max_nm` in `config/control.yaml` (RS03 5, RS02 3, RS00 3 N·m) | same |
| Shoulder through the elbow stop | Covered by the torque cap and soft limits only; no mechanical protection this pass | ELBOW_DUMMY_OUTPUT_TEST.md |
| Never | run a joint into a hard stop, hold a stall, exceed rated torque | CALIBRATION_LOOP.md |

Also close status.md **CS10** before the arm moves on printed parts: bench caps bound feed-forward only, and PD torque can exceed
them. A printed stop rated 17 N·m needs a bound on total torque, not just feed-forward.

**Operator rule, not software:** keep side pushes at the hand under about 40 N until the rig's test 7 has run.

**Needs from hardware:** the elbow rig results (tests 5 to 7), then the forearm print. The URDF limits are provisional
(inward roll 15° is a hand calculation).

**Unblocks:** powered elbow and forearm moves, then the calibration loop on the full arm.

## 7. Camera calibration loop

The full spec is [CALIBRATION_LOOP.md section 11](CALIBRATION_LOOP.md#11-spec-for-the-software-project): camera service
(BRIO, fixed exposure and focus, AprilTag `tag36h11` + ChArUco, poses relative to a frame tag, timestamps on the CAN clock),
the `circuit` runner with section 9's stop rules inside the host loop, analysis tools (axis circle fit, geometry least squares,
backlash, repeatability, drift), the drive preflight (item 3), MCP access for Claude, and gesture scoring.

**Suggested order inside it:** camera service and stage C0 first (works with the arm unpowered); then the runner with stop
rules, tested on `vcan` (item 4); then C1 and C2 attended with Joey.

**Needs from hardware:** tag plates and a camera mount (prints, not made yet), the foam catch pad, and where the robot and
camera stand (open until the rig is set up). Host is assumed to be the bench Pi 5 with the BRIO on it.

**Unblocks:** unattended calibration and testing, and gesture iteration (the wave).

## 8. URDF from CAD

**Build:** a path to load [left_arm.urdf](../urdf/left_arm.urdf) alongside, and later into, `assets/urdf/marengo.urdf` and the
MJCF. Don't hand-edit it: it is regenerated from SolidWorks by `generate_urdf.py`, and weighed masses override CAD through
`mass_overrides.json` ([urdf/README.md](../urdf/README.md)).

**Known gaps:** the hand (200 g) and forearm pack (300 g) are placeholder masses; joint limits are provisional; the right arm's
link COMs were fitted while its shoulder rubbed, so don't copy them across. Gravity parameters for the left arm come from the
calibration suite (C6), not from the right arm.

**Needs from hardware:** a re-export after the forearm is final and the parts are weighed. Hardware will regenerate it.

**Unblocks:** gravity compensation, collision checks for the calibration loop, the `vcan` sim.

## 9. Power board interface on the Pi (later)

Not needed for the bench, which runs on the 24 V supply with a physical E-stop. Needed before the arm runs from the power board
and the battery. The requirements are listed in
[STATUS.md, power board section](../../../STATUS.md#power-board-and-battery-what-software-must-do) and in full in
[REV_B_PARTS.md, accepted risks and software requirements](../../../electrical/pdb/REV_B_PARTS.md#accepted-risks-and-software-requirements-joey-2026-11-07-rev-b5-rows-marked):
the heartbeat toggled from the control loop every 50 to 100 ms, `canTimeout` ≤ 50 ms (item 3), port-fault reporting, INA237
meters on one Pi I²C controller, the startup self-test, the brake clamp caps (30 W average over 60 s until the heat test, 3 kJ
per burst), the brown-out current budget and the E-stop input debounce.

**Note:** REV_B_PARTS.md gives the limb shunt as 2 mΩ in the parts table and 1.5 mΩ in the meter row. The power board thread
is resolving it; read the value from that doc when you write the meter scaling.

**Can start now** against the spec and the `vcan` sim (item 4). **Hardware ready:** after the board's review and fab order.

## Rules that hold for every item

- **No spare actuators.** Nothing overloads, overheats or stalls a real drive. Destructive tests use printed dummies.
- **Stationary and supported** until legs exist. The arm may drop onto the foam pad when a run stops hard; that's accepted.
- **The bench supply has no current limit.** Joey fits a 3 A fuse for first power-up and 10 A after that.
- **Never write damper = 1, never auto-clear a fault, never write a new zero outside the reference workflow.**
- Hardware facts change: check [STATUS.md](../../../STATUS.md) before acting on a number here.

## Open questions for Joey

1. Left arm on HAT `can1` for now, or buy the first USB-CAN adapter up front? Recommendation: start on `can1`, buy the adapter
   only if the soak shows cause B (item 2).
2. Device IDs 14 (elbow) and 15 (forearm yaw) are proposals; confirm when those drives are wired.
