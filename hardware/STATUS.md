# Hardware status (for the software project)

**Last updated:** 2026-11-08 · Maintained by the hardware project. The robot is **Roark**. Its new software lives in
[jaylamping/roark](https://github.com/jaylamping/roark) ([PLAN.md](https://github.com/jaylamping/roark/blob/main/PLAN.md),
[TENETS.md](https://github.com/jaylamping/roark/blob/main/TENETS.md)). This repo is the hardware repo and the read-only
reference for the legacy software.

This page says what physically exists, what is coming, and what software needs to know or build. It links to the detailed docs
rather than repeating them. If a linked doc disagrees with this page, the linked doc wins; please flag it.

Legend: **built** = physically exists · **printed** · **designed** = CAD/spec done, not built · **planned** · **parked**.

## Whole robot at a glance

Target: full humanoid, 23 joints on 5 CAN buses (both arms, both legs, waist). The left arm is the current build step, not the
scope.

| Workstream | State | What it means for software |
|---|---|---|
| Right arm (5 DOF) | **built**, on the bench | `can0`, IDs 1–5. Runs on the legacy stack today; moves to Roark after milestone 1 |
| Left arm (5 DOF) | Upper arm being **redesigned** as a clamshell after the first print was rejected; forearm **designed**, not released; actuators in hand | `can1`, IDs 11–15, joint table below |
| Torso frame (2020 aluminium) | **designed**; Joey rebuilding it to the new spec, not yet validated | Robot frame and mount points. Pi bay ceiling 33 mm |
| Power board (rev B5) | **designed**; layout and routing in progress; full review before the fab order | Pi link J21: I2C meters, E-stop status, heartbeat watchdog, port fault, power-fail warning. Hard software requirements below |
| Battery (13S2P 21700) | **designed**; build plan done | BMS telemetry over an isolated adapter; charge, brown-out and shutdown interlocks in software |
| Compute | Pi 5 4 GB (control, CAN, safety) + Jetson Orin Nano 8 GB (perception) confirmed; nothing bought | Pi runs from NVMe; one isolated candleLight USB-CAN adapter per bus, first one for `can1` |
| Actuator test stand | **designed** and sliced for RS03, RS02, RS00, RS04 | Test data feeds the actuator models in sim. Order RS03 (waist unit) → RS02 → RS00 → RS04 |
| Calibration rig (camera loop) | **approved** plan | BRIO + AprilTags + drive telemetry; software spec in section 11 of the plan |
| Body shell | Rev B baseline **designed**; fit kit printable; the rest waits for the torso | None |
| Hand | research only | Reserved CAN ID 16 on `can1` |
| Legs (2 × RS04 in stock) | planned | Two buses later. Pelvis keeps two lift points for a leg gantry |
| Head, neck, waist yaw | **parked** | Neck plan: 3 × 12 V STS3215 bus servos. Waist: one RS03 |

**CAN bus map so far:** `can0` right arm 1–5 · `can1` left arm 11–15 (+16 hand) · legs and waist: not assigned yet. Each bus
gets its own USB-CAN adapter, so no two limbs share a bus.

## Left arm (milestone 1)

- **URDF:** [`designs/left-arm/urdf/left_arm.urdf`](designs/left-arm/urdf/left_arm.urdf), generated from the SolidWorks skeleton
  by `generate_urdf.py` (do not hand-edit). Weighed masses override CAD via `mass_overrides.json`. Conventions, limitations and
  placeholder masses: [urdf/README.md](designs/left-arm/urdf/README.md). Frame REP-103 (X forward, Y left, Z up). Not yet
  regenerated for the clamshell upper arm or forearm draft 4, so masses there are provisional.
- **Joints, shoulder to hand** (provisional limits from the URDF):

  | Joint | Actuator | CAN `can1` | Limits (rad) | Effort (N·m) |
  |---|---|---|---|---|
  | `left_shoulder_pitch` | RS03 | 11 | −0.87 … 3.14 | 60 |
  | `left_shoulder_roll` | RS03 | 12 | −0.26 … 3.14 | 60 |
  | `left_upper_arm_yaw` | RS02 | 13 | −1.57 … 1.57 | 17 |
  | `left_elbow_pitch` | RS02 | 14 | 0.0 … 2.164 (124°, printed stop) | 17 |
  | `left_lower_arm_yaw` | RS00 | 15 | −1.05 … 1.05 (URDF); mechanical stop ±170° on the wrist flange | 14 |
  | (hand, future) | — | 16 (reserved) | — | — |

  Positive motion: pitch raises forward, roll abducts, yaw rotates outward, elbow flexes. The right arm uses the same meanings
  (which negates its roll and yaw axes). `velocity` in the URDF is the MIT protocol range, not rated speed. The drives may still
  hold old IDs (12 on `can1`, 11–14 on `can0`), so scan and set IDs one drive at a time. Bus and ID plan:
  [left-shoulder-bench-harness.md](electrical/wiring/left-shoulder-bench-harness.md),
  [left-arm-trunk/README.md](electrical/wiring/left-arm-trunk/README.md).
- **Don't copy names or IDs from the legacy files.** `config/robot.yaml` and the left table in
  [docs/kinematics.md](docs/kinematics.md) still say `left_elbow` / `left_wrist`, and `config/motors_humanoid.yaml` puts the
  left arm on `can3` with IDs 14–18. The table above is current. The left RS00 joint is a forearm yaw, like the built right arm.
- **Build state:**
  - Upper arm: the first all-printed part (2026-11-07) was rejected on 2026-11-08 (202 g, support scars, support trapped
    inside). Clamshell draft 2 passes the mesh check, support check and seam FEA, and waits for the two-reviewer gate before
    printing: [upper-arm/clamshell/cad/README.md](designs/left-arm/upper-arm/clamshell/cad/README.md).
  - Forearm draft 4: the RS00 rides on two 6808 bearings so it carries only twist. About 779 g with the RS00, CoM 102 mm below
    the elbow axis (CAD estimate). Not released: [forearm/README.md](designs/left-arm/forearm/README.md).
  - Elbow cable crossing decided 2026-11-08: a free back loop in loom, no guard.
  - 6808 bearings arrive 2026-11-09.
- **Before the forearm is powered:** the elbow dummy-output rig test,
  [testing/ELBOW_DUMMY_OUTPUT_TEST.md](designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md). Keep side loads at the hand under
  about 40 N until it passes.
- Measured masses: [MEASURED_MASSES.md](designs/left-arm/MEASURED_MASSES.md). Bench supply: 24 V, 1200 W.

## Power board and battery: what software must do

Full list with reasons: **[REV_B_PARTS.md, "Accepted risks and software requirements"](electrical/pdb/REV_B_PARTS.md)** and the
fault table under it. Rev B5, not yet built. Headlines:

- **Pi link J21** (JST GH 8, everything at 3.3 V,
  [pinout](electrical/pdb/design/layout/HARNESS_INTERFACE.md)): 1 +3V3_PI, 2 I2C_SDA, 3 GND, 4 I2C_SCL, 5 `ESTOP_PI_N`,
  6 `WD_FEED`, 7 `PORT_FLT_N`, 8 `PWR_FAIL_N`.
- **Heartbeat watchdog (TPS3431):** toggle `WD_FEED` from the control loop itself, not a PWM peripheral, every 50–100 ms. No
  falling edge for 270–355 ms drops the motor latch until a manual reset. Stopping the toggle is a valid way to drop the motors.
- **Drive CAN timeout:** write `CanTimeout` ≤ 50 ms on every drive and read it back before every enable; refuse enable if any
  drive reads 0 or above the limit. Re-verify after every drive power cycle.
- **Drive current limits:** set so everything that can move at once sums to ≤ 90 A; read back at startup.
- **E-stop:** hardware only, no software in the path. `ESTOP_PI_N` low = pressed (info only: debounce, no pull). `PORT_FLT_N`
  low = a limb port latched off; name the port from the meters and tell the operator. Reset is a physical button the Pi can't
  block, so run the self-test first and tell the operator not to reset on failure.
- **Power-fail warning (new 2026-11-08):** `PWR_FAIL_N` goes low when the converter input falls under 31.4 V (re-arms at
  34.4 V). GPIO input with a pull-up, edge-triggered. On assert: safe-stop the motors, stop the heartbeat, `sync`, remount
  read-only, shut down. The software low-battery rules below act first; this is the hardware backstop. There is no hold-up, so a
  sudden total power loss gives no warning: use ext4 with journaling.
- **Meters (INA237, one dedicated Pi I2C bus):** L arm 0x40, R arm 0x41, aux 0x43, L leg 0x44, R leg 0x45, SPARE_SW 0x47,
  waist 0x4C, 12 V 0x4D, MID 0x4F. Shunts: limbs 2 mΩ, 12 V 2 mΩ, SPARE_SW 5 mΩ. 16-bit, no energy accumulator, so the Pi
  integrates energy.
- **Brake resistors (2 × Arcol HS50 20 Ω, thermostats in the E-stop loop):** cap braking at about 5 A when the pack is nearly
  full or the BMS isn't accepting charge. Cap clamp energy at 30 W average over a rolling 60 s (until the bench heat test sets the
  real number) and 3 kJ per burst; clamp duty is read from U17. At either limit, stop braking and make a safe stop.
- **Battery / BMS (JBD SP17S005, 13S, ~46.8 V nominal, 54.6 V full):** telemetry over an isolated USB-RS485/UART adapter.
  Refuse drive enable while the BMS shows charge current or telemetry is stale. Shut compute down at min cell ≤ 3.0 V, filtered
  pack ≤ 38 V, or BMS telemetry lost > 5 s. Drive current budget = (V_rest − 35 V) / R_pack_measured.
- **12 V servo back-feed:** keep the neck/jaw servo current cap; never back-drive them faster than rated.
- Background: [pdb/README.md](electrical/pdb/README.md), [REVB5_CHANGES.md](electrical/pdb/design/REVB5_CHANGES.md),
  [PRODUCTION_RUN.md](electrical/pdb/PRODUCTION_RUN.md), [pack-build-plan.md](electrical/power/pack-build-plan.md).

## Compute and CAN

- **Pi 5 setup** ([compute-plan.md](docs/compute-plan.md)): boot and run from NVMe on the official M.2 HAT+ (SD card for
  recovery only), official Active Cooler, `PSU_MAX_CURRENT=5000` in the EEPROM (the 5 V supply is a plain ≥ 6 A buck with no
  USB-PD; without this the USB ports cap at 600 mA and the CAN adapters can brown out). The CAN HAT goes away.
- **CAN adapters:** one isolated classic-CAN adapter per bus running mainline candleLight firmware (STM32F0, CANable Pro class,
  shows up as `gs_usb`). G431 "CANable 2.0 Pro" clones are not supported. Buy one for `can1`, measure latency and jitter, then
  buy the rest. Full reasoning and the suggested Rust stack: [research/can-stack-for-marengo.md](docs/research/can-stack-for-marengo.md).
- **Jetson:** cameras, speech, small models. Large models run on the desktop PC or in the cloud.

## Specs written for the software project

These are hardware-side asks; Roark's [PLAN.md](https://github.com/jaylamping/roark/blob/main/PLAN.md) decides what gets built
and in what order.

| Spec | Where |
|---|---|
| Calibration loop: camera service, `circuit` runner with stop rules, fit tools, drive parameter preflight, MCP access, gesture scoring | [CALIBRATION_LOOP.md section 11](designs/left-arm/testing/CALIBRATION_LOOP.md#11-spec-for-the-software-project) |
| Sim-to-real: actuator models, replay and gap score, measured randomization, second simulator, `vcan` RobStride drive emulator, automatic scoring | [sim-to-real-loop.md section 5](docs/sim-to-real-loop.md#5-spec-for-the-software-project) |
| CAN layer: adapters, Linux setup, realtime threads, codec, drive watchdog, logging | [research/can-stack-for-marengo.md](docs/research/can-stack-for-marengo.md) |
| Power board software requirements and Pi interface | [REV_B_PARTS.md](electrical/pdb/REV_B_PARTS.md) (section above) |

## Other references

- RobStride manuals (read before assuming drive behaviour): [docs/robstride/](docs/robstride/README.md).
- Actuator test stand: [designs/actuator-test-stand/README.md](designs/actuator-test-stand/README.md).
- Torso frame: [designs/torso-frame/README.md](designs/torso-frame/README.md). Body shell:
  [designs/body-shell/manufacturing/README.md](designs/body-shell/manufacturing/README.md).
- Waist yaw study: [designs/waist-yaw/README.md](designs/waist-yaw/README.md). Neck: [designs/neck/](designs/neck/).
- CAD: the latest version of every CAD file lives under [`cad/`](../cad/README.md) in Git LFS.

## Change log

Newest first. One line per milestone that matters to software.

- 2026-11-08: Robot named Roark; software moved to jaylamping/roark. Upper arm print rejected, clamshell redesign at the review
  gate. Forearm draft 4. Elbow back loop. Power board `PWR_FAIL_N` (J21 pin 8), Pi 5 V supply ≥ 6 A, limb shunts 2 mΩ. Pi setup
  requirements (NVMe, cooler, `PSU_MAX_CURRENT`). CAN adapter pick (candleLight STM32F0) and CAN stack report. Left arm IDs 14
  and 15 confirmed in the plan.
- 2026-11-07: Page created. Left arm URDF and joint names, `can1` ID plan, power board rev B5 software requirements, calibration
  and sim-to-real specs, compute plan.

---

**Hardware threads:** update this page in the same commit when a milestone lands (a part built or tested, a joint name, limit,
CAN ID or URDF change, a new software requirement, a spec for software). Bump "Last updated" and add a change-log line.
