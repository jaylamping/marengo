# Left arm: closed-loop calibration and test rig

Status: **approved by Joey 2026-11-07 06:25Z** (option A, Logitech BRIO, foam catch pad as the hard-stop fail-safe). Nothing built or wired yet. The control and analysis code belongs in the software project; this document specifies it only.

Goal (Joey, 2026-11-07): a 100 % closed feedback loop with a clearly defined calibration circuit and success criteria, so Claude can run calibration and testing of the left arm on its own for hours, and so we catch the hurdles that will show up once the arm starts moving.

Evidence labels used below: **vendor** (RobStride manual in [`hardware/docs/robstride/`](../../../docs/robstride/)), **repo** (existing marengo docs or config), **estimate** (my number, to be measured in stage C0), **assumption**.

## 1. Recommendation in one paragraph

Use the webcam Joey already owns, fixed on a rigid mount beside the bench, watching paper AprilTag markers on small clip-on plates on the torso frame and on each arm link. The camera says where each link really is in space; the RobStride drives already say where each motor thinks it is, plus torque, speed, temperature and faults. Comparing the two is the closed loop. Add **nothing permanent to the robot** for this. Total new cost is about $0 to $25 (tag plates, a mount, a lamp, a foam catch pad). Joey's "emitter on every joint, receiver in the head" idea is sound and has been done many times (section 2); the bench camera is the cheapest version of it, and the robot's future head camera can reuse the same tags and the same software later at no extra hardware cost.

## 2. Has this been done before?

Yes, in several forms. Joey's idea is a known one.

| Approach | How it works | Accuracy | Fit for marengo |
|---|---|---|---|
| Optical motion capture (Vicon, OptiTrack) | IR cameras around the room see reflective or LED markers on every link | sub-mm | The gold standard for robot calibration and gait labs. Thousands of dollars. Overkill. |
| Lighthouse trackers (HTC Vive / SteamVR) | Base stations sweep IR; photodiode trackers on each link time the sweep. This is closest to "emitter + receiver" | ~1–2 mm, a few hundred dollars per tracker | Bulky trackers (~90 g each) on a 2–3 kg arm change its mass. Not needed. |
| Camera + printed fiducials (AprilTag / ArUco / ChArUco) | One calibrated camera solves each tag's 6-DoF pose from the image | mm-level at 1 m with averaging (estimate) | Standard method for robot kinematic and hand-eye calibration. Costs paper. **Recommended.** |
| Head camera self-calibration | The robot looks at tags on its own hands and fixes its own model | similar to above | Done on research humanoids. Free later once marengo has a head camera. |
| Radio (UWB, BLE) | Time of flight between emitters and receivers | ~10 cm | Far too coarse for joints. |
| Magnetic trackers (Polhemus-style) | Coils measure field from a source | mm in clean air | Distorted badly by motors, steel and the aluminium frame. No. |
| One IMU per link | Accelerometer + gyro orientation | ~1° tilt, yaw drifts | Worse than the joint encoders we already have, and gives no position. No. |

## 3. What the loop has to catch

The drives already measure motor-side angle well (**vendor**: 14-bit absolute encoder, 2^14 counts per turn). What they cannot see is everything between the motor and the link's real position in space:

1. **Zero offset and sign** of each joint (the reference pose was set slightly wrong, or a joint runs backwards).
2. **Geometry errors**: printed parts, assembly and the CAD-generated URDF don't quite match (axis tilted, offset, link length off).
3. **Backlash** in the gearboxes and joints (different position depending on approach direction).
4. **Compliance and sag**: printed links and screw joints bend under gravity load.
5. **Slow change over time**: PETG creep, screws loosening, a crack starting, an output coupling slipping. This is the dangerous one, and it only shows up over hours.
6. **Collisions and snags**: cable caught, link touching the torso.

Encoder plus torque data alone catches 6 partly and 4 indirectly. The camera catches all six. That is the whole case for adding it.

## 4. Options

| Option | New hardware | Cost | What it adds | Verdict |
|---|---|---|---|---|
| **A. One fixed webcam + AprilTags (bench only)** | Joey's webcam, mount, tag plates, lamp | ~$0–25 | Real link pose for all six error types | **Recommended** |
| B. Two cameras (stereo, or front + side) | second webcam | ~$30–60 | Better depth accuracy, no occlusion | Only if A's depth error fails the C0 gate or the arm hides its own tags too often |
| C. Per-link IMUs on the robot | 5 IMUs + wiring through every joint | ~$30 + harness work | Tilt only, drifts in yaw | Rejected: worse than encoders, adds wiring through joints |
| D. Lighthouse trackers | 2 base stations + trackers | $300+ | ~1 mm, fast | Rejected: cost and tracker mass |
| E. Permanent head camera | comes with the head | later | Lifetime self-check | Not now. Reuse A's tags and software when the head exists |

## 5. The rig (option A)

**Camera.** Joey's **Logitech BRIO** (4K webcam, USB 3). Settings for every run: 4K (3840 × 2160) stills at the holds, 1080p for live monitoring between holds; field of view set to 78° in Logitech's tool; **autofocus off, auto exposure off, auto white balance off**, all set over UVC (`v4l2-ctl` on Linux) and read back in preflight, because any of them drifting breaks the lens calibration. RightLight / HDR off. Use the USB 3 port on the host.

Resolution check (**estimate**, measured in C0): at 1 m with the 78° field of view the image is about 1.6 m wide, so 4K gives about 0.42 mm per pixel and a 50 mm tag spans about 120 px. That is comfortably enough for the C0 noise gate when averaging over a 1–2 s hold. 4K detection on a Pi 5 will be slow (roughly a frame or two per second), which is fine because measurements are taken at static holds.

**Mount.** Rigid, not on the same surface as the torso stand if that surface flexes. Camera about 0.8–1.2 m from the shoulder, looking at the arm's front quarter (about 30–45° off the front) and slightly from above, so the whole reachable space of the test circuit is in frame. A printed clamp on a piece of 2020 bolted to the bench is enough. Exact position is decided once the model and field of view are known.

**Reference tag on the frame.** Two tags on a plate bolted to the aluminium torso frame (not the shell). Every link pose is measured relative to this frame tag, so a bumped camera does not corrupt results; it only triggers a re-check.

**Link tags.** AprilTag family `tag36h11`, laser-printed on matte paper, glued to flat printed PETG plates (about 50 × 50 mm, a few grams each). At least one plate per moving link (upper arm, elbow fork / forearm, wrist flange), two per link where the link rotates out of view. Plates clip or screw to existing hardpoints on the links, never to the shell, and their pose on the link is found by the calibration itself (no precise mounting needed). Tag size: 50 mm black square (the BRIO resolves this well at 1 m); go to 40 mm only where a link face is too small.

**Camera calibration target.** A ChArUco board printed on paper and glued flat to something rigid (foamcore or a printed plate), used once per camera setup to measure the lens.

**Lighting.** One fixed LED lamp. No direct window light (sun moving over hours changes the images).

**Catch pad.** Foam under the arm's fall path. When the drives lose power or drop to reset the arm falls under gravity (the vendor's unpowered anti-backdrive damping slows it but does not hold it; **vendor** section 4.2.5). Unattended runs must assume this can happen at any moment.

**Host.** The camera plugs into the same computer that runs CAN (the bench Pi 5, **repo**), so frames and drive telemetry share one clock. AprilTag detection at 1080p on a Pi 5 at a few frames per second is enough because measurements are taken at static holds (**estimate**; verified in C0).

**Power.** Bench supply only (24 V, no current limit, **repo**), with the trunk fuse from the left-arm harness plan and the hardware E-stop in the supply line. The battery pack is not used for unattended runs.

**Nothing permanent on the robot.** The tag plates come off after testing.

## 6. What gets measured

| Source | Signals | Rate |
|---|---|---|
| Camera | 6-DoF pose of every visible tag relative to the frame tag; detection quality (reprojection error, tag decision margin) | each hold, averaged over 1–2 s of frames |
| Drives (type-2 feedback, parameter reads) | position, velocity, torque, temperature, fault bits, VBUS (0x701C) | 50–100 Hz during motion (**repo** practice) |
| Host | commanded trajectory, timestamps, stage and pose IDs, ambient temperature if a cheap sensor is on hand | per command |
| Model | forward kinematics of the current URDF at the measured joint angles | per hold |

The core measured quantity is the **residual**: camera pose of each link minus the pose the URDF predicts from the encoder angles.

## 7. Calibration circuit

Stages run in order. Each one must pass its gate before the next starts. The existing right-arm gravity and friction suite ([`docs/commissioning/right-arm-calibration-suite.md`](../../../../docs/commissioning/right-arm-calibration-suite.md)) is reused as stage C6; the camera adds what that suite could not measure.

| Stage | What runs | Measures | Attended? |
|---|---|---|---|
| **C0 Rig check** | ChArUco calibration; arm held still (unpowered on the catch pad, or held) for 60 s | lens model, camera noise per tag | Joey sets up once |
| **C1 Sign and zero** | each joint alone: small move (±5°) from the reference pose | joint direction, zero offset vs camera | Attended (first motion) |
| **C2 Axis identification** | each joint alone, slow sweep over its safe window, others held; 2 sweeps each direction | each joint's real axis direction and location (tag traces a circle; fit it) | Attended first run |
| **C3 Accuracy grid** | 30–50 poses spread over the safe workspace, each approached from two directions | camera vs URDF residual at every link; backlash per joint | Unattended after one clean attended run |
| **C4 Repeatability** | 3 reference poses, each revisited 20× from different start poses | spread of the arrived pose | Unattended |
| **C5 Power-cycle reference** | power drives off and on, re-reference, return to the 3 reference poses | whether the reference workflow restores the same physical pose | Attended (involves the E-stop / supply) |
| **C6 Gravity and friction** | existing suite phases 0–7 | link mass x COM, friction; camera adds sag (deflection under load) | per that suite |
| **C7 Tracking** | compound waypoint moves at 25 %, 50 %, 75 % of the speed cap | following error (encoders); end-point error at the stops (camera) | Unattended after attended pass |
| **C8 Soak** | loop C3 (reduced grid) + C4 for hours with rest periods | drift of residuals, compliance, temperatures over time | Unattended |

What each stage feeds back:

- **C1** gives zero offsets and sign fixes (config).
- **C2** gives joint origin and axis corrections to the CAD-generated URDF ([`../urdf/left_arm.urdf`](../urdf/left_arm.urdf)), and flags any axis far enough off that it means a CAD, print or assembly error rather than a calibration value.
- **C3** fits the remaining small geometry errors and gives backlash per joint.
- **C6** gives gravity and friction parameters and a compliance baseline (mm of sag per N·m) per link.
- **C8** watches everything above for change. Change is the early warning for creep, loosening and cracks.

## 8. Success criteria (initial gates)

These are starting values. Camera-dependent numbers are **estimates** until C0 measures the real noise; after the first full pass each gate is re-set from data (gate = the larger of the target and 3 x the measured noise) and recorded in the run log.

| Gate | Pass if | Why this number |
|---|---|---|
| C0 lens calibration | reprojection RMS ≤ 0.5 px | standard for a usable calibration |
| C0 static noise | per-hold averaged tag position σ ≤ 0.5 mm, rotation σ ≤ 0.1° | must be well under the gates below |
| C1 sign | 100 % correct | no motion stage runs with an unverified sign |
| C1 zero offset | ≤ 0.5° after correction | matches the existing homing repeatability target (**repo**, `hardware/docs/homing-sensors.md`) |
| C2 axis | direction within 1° and location within 2 mm of CAD after correction; any raw error > 3° or > 5 mm is flagged to Joey as a physical problem | bigger than that is a build error, not calibration |
| C3 wrist-flange position | ≤ 5 mm RMS, ≤ 10 mm max over the grid after correction | a printed arm at ~0.4 m reach; tighten once real backlash is known |
| C3 backlash | recorded per joint; > 0.5° flagged | RobStride publishes no backlash spec, so the first measurement is the baseline |
| C4 repeatability | wrist-flange spread ≤ 2 mm (3σ) | |
| C5 power cycle | same physical pose within C4's spread | proves the reference workflow; if it fails, the reference method changes before any unattended run that power-cycles |
| C6 gravity / friction | existing suite gates (residual ≤ 0.10 N·m etc.) | **repo** |
| C7 tracking | following error and stop error within limits set from the attended pass; no overshoot past a soft limit | |
| C8 drift | grid residual change ≤ 1 mm per hour and no step change > 2 mm; compliance within 20 % of baseline; no joint temperature trend that would reach the pause limit | a step change means something slipped or cracked |

## 9. Safety limits for unattended runs

The motors are irreplaceable (no spares). Every rule below exists to protect them first, then the printed parts, then the frame.

**Before any unattended run**

1. The same stage has passed once attended, with Joey present, with the same code, config and hardware. Any change to any of those resets this.
2. Joey has given a forbidden-pose list (torso, stand, cables) and Claude has checked every planned pose and path against the URDF collision model with a margin of at least 20 mm.
3. The arm is on the catch pad path, the workspace is clear, the hardware E-stop is in the supply line, and the trunk fuse is fitted.

**Limits set inside the drives** (so they hold even if the host crashes)

| Limit | Drive parameter (**vendor**) | Setting |
|---|---|---|
| Torque cap | `limit_torque` 0x700B | per joint: 1.5 x the largest gravity torque in that stage's window plus friction, and never above the weakest printed part's rating on that joint (e.g. the elbow stop wear part is rated 17 N·m, **repo**). Starting caps come from the existing `tau_ff_max_nm` values in `config/control.yaml` and are written into each stage plan. |
| Current cap | `limit_cur` 0x7018 | consistent with the torque cap |
| Speed cap | `limit_spd` 0x7017 | ≤ 1.0 rad/s for C1–C6, raised only in C7 |
| Host-loss timeout | `canTimeout` 0x7028 (20000 = 1 s) | about 0.1–0.2 s: if the host stops sending, the drive drops to reset |

The anti-backdrive damper stays at its default (on). It is never disabled (**vendor** 4.2.5; project memory).

**Limits in the host (stop rules)**

| Trigger | Action |
|---|---|
| Any drive fault bit | Stop the run. Never auto-clear a fault and retry. Wait for review. |
| Joint temperature ≥ 60 °C | Pause (hold at rest pose, torque off if the pose allows) until ≤ 45 °C. Drive protection is at 135–145 °C (**vendor**); we stay far below it. |
| Joint temperature ≥ 75 °C | Stop the run. |
| VBUS outside 22–26 V on the 24 V supply | Stop. |
| Camera loses the frame tag, a link tag is missing at a hold where it should be visible, or frames go stale > 1 s | Stop motion; hold. Unattended runs never continue blind. |
| Camera vs model residual jumps > 10 mm at a hold, or torque at a hold is > 30 % above the model | Treat as a collision or slip: stop. |
| Command outside soft limits (≥ 5° inside every hard stop) | Refused before sending. |
| Drive feedback missing > 50 ms | Stop. |

**How a stop happens.** First a controlled move to the nearest rest pose at low speed if every drive is healthy, then disable. If anything is unhealthy, disable at once and let the arm fall onto the catch pad. The hardware E-stop is the backstop when Joey is present; unattended, the drive timeout and the drive-level caps are the backstop.

**What unattended runs never do**: run a joint into a hard stop, hold a stall, exceed rated torque, run a new stage, change limits, flash firmware, write a new zero to a drive outside the reference workflow, or power-cycle drives (C5 stays attended).

**Duty cycle.** C8 soak alternates about 20 minutes of motion with 10 minutes at rest. The temperature rules above override this.

## 10. How the autonomous loop runs

```text
plan stage -> preflight -> run -> measure -> analyse -> gate
     ^                                                  |
     |        pass: apply allowed corrections,          |
     |        re-run as a check, next stage             |
     +------------- fail: stop, report ------------------+
```

1. **Plan**: Claude writes the stage plan (poses, speeds, caps, expected torques) as a file. Every pose is checked against soft limits, the forbidden-pose list and the collision model before anything moves.
2. **Preflight**: drive parameters read back and compared with the plan (caps, timeout, damper); camera sees the frame tag and every link tag at the start pose; temperatures cold; no faults.
3. **Run**: the host executes the plan. Claude does not stream commands live; the plan runs on the host so a dropped network link cannot leave the arm mid-move.
4. **Measure and analyse**: residuals, fits, backlash, drift, all saved with images of each hold.
5. **Gate**: compare with section 8.
6. **Corrections Claude may apply on its own** (then re-run the stage as a no-refit check): zero offsets ≤ 2°, friction parameters, URDF joint origin corrections ≤ 2 mm / 1°, link COM shifts within the existing suite's limit. Everything else (bigger corrections, limit changes, a failed gate twice in a row, any physical anomaly) stops and waits for Joey.
7. **Report**: one short summary per stage with the gate table, the worst residuals and a few annotated camera frames. A failed run report starts with what failed and what Claude thinks caused it.

Each session is saved in one folder: plan, drive telemetry, tag detections, selected frames and the results summary. Calibration records go next to the existing ones in `docs/commissioning/calibrations/` (software project decides the exact format).

### Gesture iteration (e.g. a human wave)

Joey (2026-11-07): commanded gestures like "a standard human wave" have never come out cleanly. Once C1–C7 pass, the same rig iterates gestures: the target is written as a measurable path (hand height and forward position, elbow angle, wave arc in degrees, frequency, number of cycles), the arm performs it at C7's safe speeds, the camera records the real hand and elbow path, and Claude scores it against the target and adjusts the joint trajectory, repeating until the score passes. A reference clip of a person waving, filmed by the same BRIO, can supply the target path. Same safety rules as C7; a new gesture's first run is attended.

## 11. Spec for the software project

Not built here; listed so the software project can pick it up.

- Camera service on the bench host: fixed exposure and focus, AprilTag + ChArUco detection, per-hold averaged poses relative to the frame tag, timestamps on the CAN clock.
- `circuit` runner: executes a stage plan file on the host with the stop rules of section 9 inside the loop (not in Claude's session).
- Analysis tools: circle fit for axis identification (C2), least-squares geometry fit for C3, backlash and repeatability stats, drift tracking for C8.
- Drive preflight: read back and verify 0x700B, 0x7018, 0x7017, 0x7028 and the damper parameter before every run.
- MCP access for Claude: start / stop a stage, fetch results and frames.
- Gesture scoring: compare the camera-measured hand and elbow path with a target path (section 10, gesture iteration).

## 12. Bill of materials

| Item | Status | Cost |
|---|---|---|
| Webcam | Joey owns a Logitech BRIO | $0 |
| Camera mount (printed clamp + 2020 offcut) | print | ~$1 |
| Tag plates, ~6–8 (PETG) + paper tags | print | ~$1 |
| ChArUco board on foamcore or a printed plate | print | ~$2 |
| LED desk lamp | likely owned | $0–15 |
| Foam catch pad | likely owned | $0–10 |

## 13. Decisions and remaining questions

Decided (Joey, 2026-11-07 06:25Z): option A; camera is his Logitech BRIO; the arm dropping onto a foam pad is the accepted fail-safe when a run stops hard.

Still open (settled when the rig is set up, no need to decide now):
1. Where the robot stands during testing, and the rigid spot about 1 m away for the camera.
2. Bench host for the left arm: the Pi 5 on can1 with the BRIO on it is assumed.

## 14. Test weights: mass and CoM bookkeeping

Added 2026-11-07 with forearm B and its 6808 support pair (`../forearm/README.md`). The test weights stand in for the hand during gravity-compensation and calibration runs.

**Hardware.** A PETG wrist adapter bolts to the wrist flange: 6 × M3 × 10 into the flange's heat-set inserts on Ø50, with a Ø34.8 pilot boss in the flange's Ø35 recess. An M8 × 100 fully threaded bolt sits head-down in the adapter's hex pocket as a stud along the forearm axis. Printed PLA pucks slide on, 250 g (30.0 mm tall) and 100 g (15.0 mm tall) when filled. Each has a sealed cavity and a 3-screw lid. An M8 nut and washer lock the stack. CAD is in `cad/parts/test_weights/`.

**Datum.** The adapter's bottom face is the stack datum. It is 12.0 mm below the wrist flange face, and the flange face is 195.0 mm from the elbow axis along the forearm, so the datum is **207.0 mm from the elbow axis**. Pucks stack down from the datum with no gaps.

**Bookkeeping, per puck.** Weigh each filled puck with its lid and screws, and write the mass on the label. Fill each cavity completely, with steel nuts, BBs or sand, and pack foam into any gap, so the contents can't shift. Then the puck's CoM is at its mid-height to within about ±3 mm for a 250 g puck. Puck *i* in the stack has its CoM at

  d_i = 207.0 + (sum of the heights of the pucks above it) + h_i / 2   (mm from the elbow axis)

Add the fixed parts:

| Item | Mass | CoM from the elbow axis |
| --- | --- | --- |
| Wrist adapter | weigh it, about 38 g at 100% PETG | about 201 mm |
| M8 × 100 bolt | weigh it, about 40 g | 207 + (100 − 5.6) / 2 − 5.6 ≈ 248 mm |
| M8 nut and washer | about 7 g | at the bottom of the stack |

The stack's mass is m_s = Σ m_i, and its CoM distance is d_s = Σ m_i d_i / m_s. Log m_s and d_s with every run.

**Forearm itself (CAD, 100% infill; weigh the printed parts and update this).** 779 g, including the RS00 (310 g), the RS00 ring, the 6808 pair, the coupling, the race clamp and the screws. Its CoM is **102.3 mm below the elbow axis**, within 1 mm of the forearm axis. No hand is fitted during these tests.

**Limits.** Allowable torque is the rated torque divided by a 1.5 dynamic factor, with the arm straight out horizontal:

| Joint | Allowable | Self load | Maximum stack m_s at d_s |
| --- | --- | --- | --- |
| Elbow RS02 (6 N·m rated) | 4.0 N·m | forearm 0.78 N·m | m_s ≤ (0.4077 − 0.0797) / d_s, so **1.40 kg at d_s = 0.235 m** |
| Shoulder pitch RS03 (20 N·m rated), full reach | 13.3 N·m | 4.0 N·m | m_s ≤ (1.3589 − 0.4114) / (0.215 + d_s), so 2.1 kg at d_s = 0.235 m |

The elbow governs. The adapter is engraved **MAX 1.4 kg**. That holds for stacks with d_s up to 235 mm, which is two 250 g pucks or lighter. For a longer stack, use the formula. Peak torque is never the limit here. The forearm passes the 5 kg worst case, so the forearm structure is not the limit either.
