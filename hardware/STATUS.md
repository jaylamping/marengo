# Hardware status (for the software project)

**Last updated:** 2026-11-07 · Maintained by the marengo HARDWARE project. This page is the bridge between the two
projects: it says what physically exists, what is coming, and what the software side needs to know or build. It links to the
detailed docs rather than repeating them. If a linked doc disagrees with this page, the linked doc wins; please flag it.

Legend: **built** = physically exists · **printing/printed** · **designed** = CAD/spec done, not built · **planned** · **parked**.

## At a glance

| Workstream | State | What it means for software |
|---|---|---|
| Right arm (5 DOF) | **built**, live on the bench | Runs on `assets/urdf/marengo.urdf` + `config/robot.yaml`, `can0` IDs 1–5. Unchanged by hardware this week |
| Left arm (5 DOF) | upper arm **printing**; elbow and forearm **designed**; shoulder bench harness planned | New URDF (below); new joint names; `can1` IDs 11–15 |
| Torso frame (2020 aluminium) | **designed**, being rebuilt by Joey to the new spec, not yet validated | Robot frame and mount points; no software change |
| Power board (rev B5) | **designed**, routing in progress, review before fab order | Heartbeat watchdog, E-stop status, per-limb meters on I2C, brake caps: hard software requirements (below) |
| Battery (13S2P 21700) | **designed**, build plan done | BMS telemetry over isolated RS485/UART; brown-out and charge interlocks in software |
| Compute | Pi 5 (control/CAN) + Jetson Orin Nano 8 GB (perception) confirmed; buy nothing yet | Next buy is isolated USB-CAN adapters, one per bus (SocketCAN) |
| Actuator test stand | **designed** and sliced for RS03, RS02, RS00, RS04 | Test data feeds actuator models for sim (spec below) |
| Calibration rig (camera loop) | **approved** plan | Camera + AprilTag + telemetry loop; software spec in section 11 of the plan |
| Body shell | Rev B baseline **designed**; fit kit printable; rest waits for the torso | None |
| Hand | research only | Reserved CAN ID 16 on `can1` |
| Head, neck, waist yaw | **parked** (lower priority than the body) | Neck plan: 3 × 12 V STS3215 bus servos. Waist: one RS03 |

## Left arm: joints, URDF, CAN

- **URDF:** [`hardware/designs/left-arm/urdf/left_arm.urdf`](designs/left-arm/urdf/left_arm.urdf), generated from the
  SolidWorks skeleton by `generate_urdf.py` (do not hand-edit). Weighed masses override CAD via `mass_overrides.json`. Conventions,
  limitations and placeholders: [urdf/README.md](designs/left-arm/urdf/README.md). Frame REP-103 (X forward, Y left, Z up).
  It is separate from the bench `assets/urdf/marengo.urdf`.
- **Joints (shoulder to hand), provisional limits from the URDF:**

  | Joint | Actuator | CAN (`can1`) | Limits (rad) | Effort (N·m) |
  |---|---|---|---|---|
  | `left_shoulder_pitch` | RS03 | 11 | −0.87 … 3.14 | 60 |
  | `left_shoulder_roll` | RS03 | 12 | −0.26 … 3.14 | 60 |
  | `left_upper_arm_yaw` | RS02 | 13 | −1.57 … 1.57 | 17 |
  | `left_elbow_pitch` | RS02 | 14 (proposal) | 0.0 … 2.164 (124°, printed stop) | 17 |
  | `left_lower_arm_yaw` | RS00 | 15 (proposal) | −1.05 … 1.05 | 14 |
  | (hand, future) | — | 16 (reserved) | — | — |

  Positive motion: pitch raises forward, roll abducts, yaw rotates outward, elbow flexes. The right arm uses the same meanings
  (which negates its roll and yaw axes). `velocity` in the URDF is the MIT protocol range, not rated speed.
  CAN plan and IDs: [left-shoulder-bench-harness.md](electrical/wiring/left-shoulder-bench-harness.md),
  [left-arm-trunk/README.md](electrical/wiring/left-arm-trunk/README.md). `can0` stays right arm only, so the two arms never share one MCP2515.
- **Known mismatches in software-owned files (please reconcile on your side):**
  - `config/robot.yaml` `limbs.left_arm` and the left table in [docs/kinematics.md](docs/kinematics.md) still use
    `left_elbow` / `left_wrist`. Hardware names are `left_elbow_pitch` / `left_lower_arm_yaw` (matching the right arm).
  - `config/motors_humanoid.yaml` puts the left arm on `can3` with device IDs 14–18; hardware plans `can1` with 11–15.
  - The left RS00 joint is a forearm yaw (like the built right arm), not a ±1.6 rad wrist pitch.
- **Forearm:** RS00 sits just below the elbow and is carried on a sleeve with two 6808 bearings, so the RS00 takes only twist
  (chosen for now, 2026-11-07). Hand and forearm electronics pack are still placeholder masses in the URDF.
- **Before the forearm is powered:** elbow dummy-output rig test,
  [testing/ELBOW_DUMMY_OUTPUT_TEST.md](designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md). Until then, keep side loads at the
  hand under about 40 N.
- Measured masses: [MEASURED_MASSES.md](designs/left-arm/MEASURED_MASSES.md). Upper-arm print package:
  [prints/upper-arm/](designs/left-arm/prints/upper-arm/).

## Power board and battery: what software must do

Full list with reasons: **[REV_B_PARTS.md, "Accepted risks and software requirements"](electrical/pdb/REV_B_PARTS.md)** and the
fault table under it. Headlines (rev B5, not yet built):

- **Heartbeat watchdog (TPS3431):** toggle `WD_FEED` (J21 pin 6) from the control loop itself, not a PWM peripheral, every
  50–100 ms. No falling edge for 270–355 ms drops the motor latch until a manual reset. Stopping the toggle is a valid way to drop
  the motors on purpose.
- **Drive CAN timeout:** write `CanTimeout` ≤ 50 ms on every drive and read it back before every enable; refuse enable if any
  drive reads 0 or above the limit. Re-verify after every drive power cycle.
- **Drive current limits:** set so everything that can move at once sums to ≤ 90 A; read back at startup.
- **E-stop:** hardware only, no software in the path. Software reads `ESTOP_PI_N` (pressed, info only: debounce, no pull),
  `PORT_FLT_N` (J21 pin 7, a limb port latched off), and the meters to tell the operator which fault happened. Reset is a
  physical button the Pi cannot block, so run the self-test first and tell the operator not to reset on failure.
- **Meters (INA237, one dedicated Pi I2C bus):** L arm 0x40, R arm 0x41, aux 0x43, L leg 0x44, R leg 0x45, SPARE_SW 0x47,
  waist 0x4C, 12 V 0x4D, MID 0x4F. 16-bit, no energy accumulator, so the Pi integrates energy.
- **Brake resistors (2 × Arcol HS50 20 Ω, thermostats in the E-stop loop):** cap braking at about 5 A when the pack is nearly
  full or the BMS isn't accepting charge. Cap clamp energy at 30 W average over a rolling 60 s (until the bench heat test sets
  the real number) and 3 kJ per burst; clamp duty is read from U17. At either limit, stop braking and make a safe stop.
- **Battery / BMS (JBD SP17S005, 13S, ~46.8 V nominal, 54.6 V full):** telemetry over an isolated USB-RS485/UART adapter.
  Refuse drive enable while the BMS shows charge current or telemetry is stale. Shut compute down at min cell ≤ 3.0 V, filtered
  pack ≤ 38 V, or BMS telemetry lost > 5 s. Drive current budget = (V_rest − 35 V) / R_pack_measured.
- **12 V servo back-feed:** keep the neck/jaw servo current cap; never back-drive them faster than rated.
- Background: [pdb/README.md](electrical/pdb/README.md), [pdb/design/REVB5_CHANGES.md](electrical/pdb/design/REVB5_CHANGES.md),
  [pack-build-plan.md](electrical/power/pack-build-plan.md).

## Specs written for the software project

These are hardware-side asks; nothing in them is decided for software.

**What to build next, in priority order:** [roark/PLAN.md](https://github.com/jaylamping/roark/blob/main/PLAN.md) (the robot software plan in the new Roark repo, milestone 1 = left arm online, ranked). The specs below are the detail behind it.

| Spec | Where |
|---|---|
| Calibration loop: camera service, `circuit` runner with stop rules, fit/analysis tools, drive parameter preflight, MCP access, gesture scoring | [CALIBRATION_LOOP.md section 11](designs/left-arm/testing/CALIBRATION_LOOP.md#11-spec-for-the-software-project) |
| Sim-to-real: actuator models (frictionloss/damping/armature, delay, actuator net), replay + gap score, randomization from measured spread, second simulator, `vcan` RobStride drive emulator for HIL, automatic scoring | [sim-to-real-loop.md section 5](docs/sim-to-real-loop.md#5-spec-for-the-software-project) |
| Compute split and CAN plan (one isolated USB-CAN adapter per bus, ~5 limb buses for the full body) | [compute-plan.md](docs/compute-plan.md) |
| Power board software requirements | [REV_B_PARTS.md](electrical/pdb/REV_B_PARTS.md) (section above) |

## Other references

- Joint SSOT for the whole humanoid: [docs/kinematics.md](docs/kinematics.md) (left arm rows out of date, see mismatches above).
- RobStride manuals (read these before assuming drive behaviour): [docs/robstride/](docs/robstride/README.md).
- Actuator test stand: [designs/actuator-test-stand/README.md](designs/actuator-test-stand/README.md). Test order RS03 → RS02 → RS00 → RS04.
- Torso frame: [designs/torso-frame/README.md](designs/torso-frame/README.md). Body shell: [designs/body-shell/manufacturing/README.md](designs/body-shell/manufacturing/README.md).
- Waist yaw study: [designs/waist-yaw/README.md](designs/waist-yaw/README.md). Neck: [designs/neck/](designs/neck/).

## Change log

Newest first. One line per milestone that matters to software.

- 2026-11-07: Page created. Left arm URDF and joint names, `can1` ID plan, power board rev B5 software requirements, calibration
  and sim-to-real specs, compute plan.

---

**Hardware threads:** update this page in the same commit when a milestone lands (a part built or tested, a joint name, limit,
CAN ID or URDF change, a new software requirement, a spec for software). Bump "Last updated" and add a change-log line.
