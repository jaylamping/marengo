# Torso PETG actuator brackets — rev A

Printed mounts for the **three RS03 layout motors** in `marengo_torso_asm_revA`.

**Rev-a shoulder decision:** shoulder **pitch** RS03 units live **inside** the 2020 cage (not on outer Y faces). Shoulder **roll** RS03 mounts on the arm sub-assembly (distal). Packaging and brackets only on the torso for pitch.

| Instance | Joint | Axis | Mount region |
|----------|-------|------|----------------|
| `actuator_rs03_waist_yaw` | `waist_yaw` | **+Z** | Bottom **40 mm** waist bay (`waist_bay_top`) |
| `actuator_rs03_left_shoulder_pitch` | `left_shoulder_pitch` | **+Y** | **Inside** cage, **+Y** half, at `shoulder_plane` |
| `actuator_rs03_right_shoulder_pitch` | `right_shoulder_pitch` | **+Y** | **Inside** cage, **−Y** half, at `shoulder_plane` |

SSOT frame: [`kinematics.md`](kinematics.md#torso-frame-committed). Handoff: [`torso-design-handoff.md`](torso-design-handoff.md).

---

## Why inside (vs outer poke)

| Outer poke (superseded) | Inside cage (current) |
|-------------------------|------------------------|
| ~230 mm shoulder line (150 + 80 poke) | **150 mm** frame OD preserved |
| External PETG wing on vertical rail | Bracket on **inner rail faces** / corner |
| Fights harness + shell taper | Shares **85 × 110 mm** chest with compute |

**Kinematics:** torso → `left/right_shoulder_pitch` (**+Y**); arm sub-asm adds `left/right_shoulder_roll` (**+X**) distal to pitch.

---

## RS03 envelope vs inner cage

Measured bbox on `vendor_robstride_rs03_vendor.SLDPRT` (mm):

| Axis | Motor | Inner clear |
|------|-------|-------------|
| **Y** (width) | **~99** | **110** → ~11 mm total margin |
| **X** (depth) | **~57** (if body depth along X) | **85** → OK |
| **Z** (height) | **~57** | shoulder band at `shoulder_plane` |

**Required orientation at shoulder (pitch motor):** pitch axis **+Y**; pack body so **99 mm spans Y** and **57 mm spans X** (not 99 mm into the 85 mm depth). Verify in a test assembly before printing brackets.

CAD action: **move** the two shoulder layout instances in `marengo_torso_asm_revA` from outer fixed poses into the cage; remate to layout refs `shoulder_mount_left` / `shoulder_mount_right` (add on layout part).

---

## Authored parts

| File | Qty | `process` / `material` |
|------|-----|------------------------|
| `hardware/cad/parts/marengo_shoulder_pitch_mount_bracket_left_revA.SLDPRT` | 1 | `print` / `PETG` |
| `hardware/cad/parts/marengo_shoulder_pitch_mount_bracket_right_revA.SLDPRT` | 1 | `print` / `PETG` |
| `hardware/cad/parts/marengo_waist_yaw_adapter_revA.SLDPRT` | 1 | `print` / `PETG` |

### Reference geometry (each part)

| Name | Shoulder (internal) | Waist adapter |
|------|---------------------|---------------|
| `urdf_link_frame` | Rigid to `torso_link` | Same |
| `joint_axis` | **+Y** (pitch) | **+Z** |
| `mount_face` | **Inner** vertical rail (+Y rail for left, −Y for right) or inner corner block | Bottom rail seat |
| `shaft_axis` | Motor output (+Y) | Motor output (+Z) |
| `cable_exit` | Toward chest center (down/back) | Up into torso |

Layout part anchors to add:

- `shoulder_mount_left` / `shoulder_mount_right` — coord sys at shoulder plane, motor center, +Y axis (pitch)
- `waist_adapter_mount` — bottom of waist bay

---

## 1. Internal shoulder pitch mount bracket (L / R)

**Purpose:** Capture RS03 against **inner face** of the side vertical (`frame_2020_vertical_*_340`) or a short PETG bridge from vertical + front/rear rail; keep motor mass inside **110 mm** Y span.

**Interfaces**

1. **Frame:** M5 + T-nut on **inner slot** of side vertical (and optionally one rail at shoulder height).
2. **Motor:** RS03 flange; pitch output **+Y** toward arm (arm sub-asm mates at pitch output; shell cutout aligned to shaft).

**Form factor (starting)**

| | mm |
|---|-----|
| Bracket depth (into chest X) | **≤ 60** (target **57** + clearance) |
| Bracket span in Y | **≤ 50** per side (half of 99 mm motor, centered in each half) |
| Wall thickness | **4–6** mm PETG, ribs to vertical |

**Modeling order**

1. Move layout motor inside cage in `marengo_torso_asm_revA` (unfix → mate to `shoulder_mount_*`).
2. New part `marengo_shoulder_pitch_mount_bracket_left_revA.SLDPRT`.
3. Sketch `mount_face` against inner rail; extrude saddle for RS03.
4. Clash-check vs `vendor_robstride_rs03_vendor` and future Pi envelope.
5. Mirror for right.

---

## 2. Waist yaw adapter (unchanged intent)

Plate in **≥ 40 mm** waist bay; **Z** axis; bolts to bottom 2020 rectangle. See previous revision notes — **57 mm** motor height vs **40 mm** bay may still require pocket or pelvis-side extension (*open in CAD*).

---

## Assembly checklist

- [ ] Shoulder RS03 instances physically **inside** cage in SW
- [ ] `Hardware` folder: 3 bracket instances
- [ ] `marengo_kinematics_consistency` finds `waist_yaw`, `left_shoulder_pitch`, `right_shoulder_pitch` in torso asm
- [ ] Shell/URDF: arm attachment at **shaft**, not outer frame corner

---

## Tradeoffs to watch

1. **Compute volume** — two RS03 + harness in **85 × 110** leaves less room for Pi/PDB; stack vertically or offset Z above motors.
2. **Shoulder roll RS03** (arm sub-asm) — mounts on pitch output; verify stack height vs shell cutout and yaw motor clearance.
3. **Service** — waist split / panel removal must clear internal shoulder brackets.
