# Kinematics

Single source of truth for joint names, axes, limits, link dimensions, and actuator assignment.

| Model | URDF | Status |
|-------|------|--------|
| **Humanoid (target)** | [`assets/urdf/marengo.urdf`](../../assets/urdf/marengo.urdf) | CAD in progress — URDF placeholder until Brawner export |
| **Arm (4 DOF, bring-up)** | [`assets/urdf/arm_4dof.urdf`](../../assets/urdf/arm_4dof.urdf) | **Current bench slice** — same arm chain as humanoid; not a separate robot |

Runtime config: [`config/robot_humanoid.yaml`](../../config/robot_humanoid.yaml) + [`config/motors_humanoid.yaml`](../../config/motors_humanoid.yaml) for the full body; [`config/robot.yaml`](../../config/robot.yaml) is the active bring-up config for the wired 4-DOF arm until remaining joints are commissioned. Milestones: [`docs/roadmap.md`](../../docs/roadmap.md).

---

## Humanoid mechanical reference

Design follows **Unitree G1** upper-body proportions and **Unitree R1** lower-body slimness, scaled to **1524 mm (5 ft)**. No wheels — biped only.

**Design status:** only the [**upper torso 2020 frame**](#torso-frame-committed) is committed for rev-a CAD. Pelvis, shell, battery bays, shoulder poke, and leg lengths below are **draft** — may change during layout; joint names, axes, and actuator map stay authoritative.

| Parameter | Unitree G1 | Marengo (draft unless noted) |
|-----------|------------|------------------------------|
| Standing height | 1320 mm | **1524 mm** |
| Body width | 450 mm | *draft* — shell TBD |
| Chest depth | 200 mm | *draft* — shell TBD |
| Thigh + shank (link length) | 600 mm | **660 mm** |
| Shoulder → hand reach | ~450 mm | **~480 mm** |
| Target mass (with battery) | ~35 kg | **38–42 kg** |
| Total DOF (v1, no hands) | 23 | **23** |

### Torso frame (committed)

Upper body structure is a single **2020 aluminum cage** (`marengo_upper_torso_frame` in CAD). Depth = **+X forward**, width = **±Y**, height = **+Z** from pelvis bottom origin.

| | Depth (X) | Width (Y) | Height (Z) |
|---|-----------|-----------|------------|
| **Outer (2020 OD)** | **125** | **150** | **380** |
| **Inner clear (between rails)** | **85** | **110** | **~300** above waist bay |

| Feature | mm | Notes |
|---------|-----|-------|
| **Waist bay** (bottom of frame) | **≥ 40** | Reserved for `waist_yaw` RS03 + adapter; no shelves |
| **Chest zone** (above waist bay) | **~340** outer | Pi, PDB, harness; M18 bay *draft* |
| **Bottom interface** | — | Bottom rail → waist adapter → pelvis ring (*pelvis draft*) |
| **Top interface** | Z = **495** from pelvis bottom | Shoulder RS03 mounts on outer Y faces (*poke draft*) |

Vertical: **380 mm** = waist joint plane → shoulder plane when pelvis height remains **115 mm** (draft).

### Link dimensions (mm)

Floor → crown must sum to **1524 mm**. Values marked *draft* are CAD targets until vendor STEP and layout freeze.

| Segment | Length | Floor → landmark |
|---------|--------|------------------|
| Foot (sole → ankle axis) | 80 *draft* | ankle @ 80 |
| Shank (ankle → knee) | 325 *draft* | knee @ 405 |
| Thigh (knee → hip) | 335 *draft* | hip @ 740 |
| Pelvis block | 115 *draft* | waist @ 855 |
| Torso (waist → shoulder) | **380** | shoulder @ **1235** |
| Neck | 65 *draft* | |
| Head (chin → crown) | 224 *draft* | crown @ **1524** |

| Part | Size (mm) | Status | Notes |
|------|-----------|--------|-------|
| **Upper torso frame** | **125 × 150 × 380** outer; **85 × 110** inner clear | **committed** | 2020 cage; see [torso frame](#torso-frame-committed) |
| Pelvis ring | ~215 × 260 × 115 outer *draft* | draft | Wider than torso; main pack bay TBD |
| Main battery envelope | ~185 × 165 × 90 *draft* | draft | Pelvis; max Wh TBD |
| M18 HD12 aux bay | ~168 × 98 × 112 *draft* | draft | Upper back cantilever |
| Shoulder outer width | ~230 *draft* | draft | 150 frame + RS03 poke (~40 mm/side); verify STEP |
| Hip span (joint centers) | ~250 *draft* | draft | R1-class; splayed mounts |
| Foot (each) | 250 × 115 × 80 *draft* | draft | Toe-heavy for CoM |
| Head | 175 × 160 × 224 *draft* | draft | Skipped in v1 CAD |
| Upper arm (per side) | 215 *draft* | draft | |
| Forearm (per side) | 195 *draft* | draft | |
| Hand / gripper stub | 90 *draft* | draft | v1 — no dexterous hand |

### Leg kinematic tree (each side)

Pelvis → **hip yaw** → **hip roll** → **hip pitch** → thigh → **knee** → shank → **ankle pitch** → **ankle roll** → foot.

Yaw is the outboard joint at the pelvis (G1/H1-style). Document any CAD reversal in the assembly drawing; joint names and axes here are authoritative.

---

## Actuator assignment (humanoid)

Peak torque ratings from [ADR 0002](decisions/0002-robstride-protocol.md) (confirm in vendor PDF before production).

| Model | Peak τ | Count | Joints |
|-------|--------|-------|--------|
| **RS04** | 120 Nm | **4** | `left_hip_pitch`, `right_hip_pitch`, `left_knee`, `right_knee` |
| **RS03** | 60 Nm | **9** | `left/right_hip_roll`, `left/right_hip_yaw`, `waist_yaw`, `left/right_shoulder_roll`, `left/right_shoulder_pitch` |
| **RS02** | 17 Nm | **8** | `left/right_ankle_pitch`, `left/right_ankle_roll`, `left/right_upper_arm_yaw`, `left/right_elbow` |
|| **RS00** | 17 Nm | **2** | `left/right_wrist` |

**Leg rationale:** RS04 on **inner hip pitch** (primary stance/swing load) and **knee** (single-support peaks, G1-class ~90 Nm knee). RS03 on **outer hip** roll/yaw. RS02 on ankles.

**Arm rationale:** RS03 shoulders (same as bring-up arm); RS02 for yaw, elbow; RS00 for wrists (5 DOF per arm, G1-aligned).

---

## Humanoid joint table

Mirror limits left/right unless noted. Effort column = motor peak τ (Nm). Angles in radians unless noted.

### Waist (1 DOF)

| Joint | Actuator | Parent → Child | Axis | Lower | Upper | Effort | Notes |
|-------|----------|----------------|------|-------|-------|--------|-------|
| `waist_yaw` | RS03 | pelvis → torso | Z | -2.71 | 2.71 | 60 | G1: ±155° |

### Left leg (6 DOF)

| Joint | Actuator | Parent → Child | Axis | Lower | Upper | Effort | Notes |
|-------|----------|----------------|------|-------|-------|--------|-------|
| `left_hip_yaw` | RS03 | pelvis → left_hip_yaw_link | Z | -2.76 | 2.76 | 60 | G1: Y ±158° |
| `left_hip_roll` | RS03 | yaw → left_hip_roll_link | X | -0.52 | 2.97 | 60 | G1: R −30°..+170° |
| `left_hip_pitch` | RS04 | roll → left_thigh | Y | -2.69 | 2.69 | 120 | **Inner hip**; G1: P ±154° |
| `left_knee` | RS04 | thigh → shank | Y | 0.0 | 2.88 | 120 | G1: 0–165°; no hyperextension |
| `left_ankle_pitch` | RS02 | shank → left_foot | Y | -1.0 | 0.8 | 17 | Tune after foot CAD |
| `left_ankle_roll` | RS02 | pitch → left_foot | X | -0.5 | 0.5 | 17 | `kd` max **5** |

### Right leg (6 DOF)

Same limits as left; roll/pitch signs follow right-hand URDF convention.

| Joint | Actuator | Parent → Child | Axis | Lower | Upper | Effort |
|-------|----------|----------------|------|-------|-------|--------|
| `right_hip_yaw` | RS03 | pelvis → right_hip_yaw_link | Z | -2.76 | 2.76 | 60 |
| `right_hip_roll` | RS03 | yaw → right_hip_roll_link | X | -2.97 | 0.52 | 60 |
| `right_hip_pitch` | RS04 | roll → right_thigh | Y | -2.69 | 2.69 | 120 |
| `right_knee` | RS04 | thigh → shank | Y | 0.0 | 2.88 | 120 |
| `right_ankle_pitch` | RS02 | shank → right_foot | Y | -1.0 | 0.8 | 17 |
| `right_ankle_roll` | RS02 | pitch → right_foot | X | -0.5 | 0.5 | 17 |

### Left arm (5 DOF)

| Joint | Actuator | Parent → Child | Axis | Lower | Upper | Effort | Notes |
|-------|----------|----------------|------|-------|-------|--------|-------|
| `left_shoulder_roll` | RS03 | torso → left_shoulder_roll_link | X | -1.57 | 1.57 | 60 | |
| `left_shoulder_pitch` | RS03 | roll → left_shoulder_pitch_link | Y | -1.2 | 1.2 | 60 | **Upright hazard** when q > ~0.5 rad |
| `left_upper_arm_yaw` | RS02 | pitch → left_upper_arm | Z | -1.57 | 1.57 | 17 | |
| `left_elbow` | RS02 | upper_arm → left_forearm | Y | 0.0 | 2.5 | 17 | **Upright hazard** — verify G-comp sign |
| `left_wrist` | RS00 | forearm → left_hand | Y | -1.6 | 1.6 | 17 | G1 wrist pitch band ~±92.5° |

### Right arm (5 DOF)

| Joint | Actuator | Parent → Child | Axis | Lower | Upper | Effort |
|-------|----------|----------------|------|-------|-------|--------|
| `right_shoulder_roll` | RS03 | torso → right_shoulder_roll_link | X | -1.57 | 1.57 | 60 |
| `right_shoulder_pitch` | RS03 | roll → right_shoulder_pitch_link | Y | -1.2 | 1.2 | 60 |
| `right_upper_arm_yaw` | RS02 | pitch → right_upper_arm | Z | -1.57 | 1.57 | 17 |
| `right_elbow` | RS02 | upper_arm → right_forearm | Y | 0.0 | 2.5 | 17 |
| `right_wrist` | RS00 | forearm → right_hand | Y | -1.6 | 1.6 | 17 |

Masses and inertials in URDF are **estimates** until CAD export; re-run MuJoCo cross-check after export ([ADR 0005](../../docs/decisions/0005-dynamics-library.md)).

---

## 4-DOF arm bring-up (joint table)

Subset of humanoid arm kinematics (shoulder roll/pitch, upper-arm yaw, elbow); currently wired per [`config/robot.yaml`](../../config/robot.yaml).

| Joint | Actuator | Parent → Child | Axis (joint) | Lower (rad) | Upper (rad) | Effort (Nm) | Notes |
|-------|----------|----------------|--------------|-------------|-------------|-------------|-------|
| `shoulder_roll` | RS03 | base → shoulder_roll_link | Z | -1.57 | 1.57 | 60 | High shoulder torque |
| `shoulder_pitch` | RS03 | roll → shoulder_pitch_link | Z (after fixed rpy) | -1.2 | 1.2 | 60 | **Upright hazard** when q > ~0.5 rad |
| `upper_arm_yaw` | RS02 | pitch → upper_arm_link | Z | -1.57 | 1.57 | 17 | |
| `elbow` | RS02 | upper_arm → forearm | Z | 0.0 | 2.5 | 17 | **Upright hazard** — verify G-comp sign |

---

## Upright / elevated poses

Documented incident (arm bring-up): arm elevated, control stopped, arm fell without gravity feedforward. Applies to **shoulder_pitch** and **elbow** on the bring-up arm and full humanoid arms.

Before bring-up tests with elevated shoulder/elbow:

1. Run per-joint `torque_ff` sign test.
2. Use **GravityComp** only until impedance is tuned.
3. Apply `danger_zones` in `config/control.yaml` for shoulder_pitch + downward velocity.

Leg bring-up: start with **hip pitch** and **knee** RS04 sign tests under load before closing the ankle loop.

---

## Frames

| Frame | Use |
|-------|-----|
| `base_link` / pelvis | Biped origin; pelvis `urdf_link_frame` on CAD pelvis (*pelvis draft*) |
| `torso_link` | Upper torso 2020 frame; shoulder roll RS03 inside cage at shoulder plane (rev-a layout) |
| `left_foot` / `right_foot` | Sole contact; Z = floor in standing neutral |
| `left_hand` / `right_hand` | Tool frame placeholder (TCP when gripper defined) |
| `forearm_link` | Arm bring-up tool frame (full humanoid: `left_hand` / `right_hand`) |

Named CAD geometry: see [cad-standards.md](cad-standards.md).
