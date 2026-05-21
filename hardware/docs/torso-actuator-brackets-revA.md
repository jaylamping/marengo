# Torso PETG actuator brackets — rev A

Printed mounts for the **three RS03 layout motors** already in `marengo_torso_asm_revA`:

| Instance | Joint | Axis | Mount region |
|----------|-------|------|----------------|
| `actuator_rs03_waist_yaw` | `waist_yaw` | **+Z** | Bottom **40 mm** waist bay (`waist_bay_top` layout plane) |
| `actuator_rs03_left_shoulder_roll` | `left_shoulder_roll` | **+X** | Outer **+Y** face of 2020 cage, `shoulder_plane` |
| `actuator_rs03_right_shoulder_roll` | `right_shoulder_roll` | **+X** | Outer **−Y** face of 2020 cage, `shoulder_plane` |

SSOT frame: [`kinematics.md`](kinematics.md#torso-frame-committed). Handoff context: [`torso-design-handoff.md`](torso-design-handoff.md).

---

## RS03 envelope (measured in CAD)

From `vendor_robstride_rs03_vendor.SLDPRT` (SolidWorks bbox, mm):

| | mm |
|---|-----|
| Footprint (X × Y) | **~99 × 99** |
| Height (Z) | **~57** |
| Mass | **~120 g** |

Layout motors are **fixed** for now; brackets must clear the committed **85 × 110 mm** inner chest volume (controller housing stays inside; motor body may poke **~40 mm** per side in Y — *draft*, verify after first print).

---

## Authored parts (create in SolidWorks)

| File | Qty | `process` / `material` |
|------|-----|------------------------|
| `hardware/cad/parts/marengo_shoulder_roll_mount_bracket_left_revA.SLDPRT` | 1 | `print` / `PETG` |
| `hardware/cad/parts/marengo_shoulder_roll_mount_bracket_right_revA.SLDPRT` | 1 | `print` / `PETG` |
| `hardware/cad/parts/marengo_waist_yaw_adapter_revA.SLDPRT` | 1 | `print` / `PETG` |

Mirror: right bracket is the **Y-mirror** of left (chiral; do not reuse one STL without mirroring in slicer).

### Required reference geometry (each part)

Per [`cad-standards.md`](cad-standards.md):

| Name | Shoulder brackets | Waist adapter |
|------|-------------------|---------------|
| `urdf_link_frame` | Parent link = `torso_link` (bracket rigid to torso) | Same |
| `joint_axis` | **+X** (colocated with motor `joint_axis`) | **+Z** (waist_yaw) |
| `mount_face` | Face bolted to **2020 rail** (T-slot side) | Top face to **bottom torso rail** |
| `shaft_axis` | Motor output / bracket bore | Motor output |
| `bolt_circle` | RS03 + T-slot M5 pattern (draft OK) | RS03 + rail M5 pattern |

Add layout anchors on `marengo_torso_layout_revA` (planes or coord systems):

- `shoulder_mount_left` — origin on **+Y** outer rail, `shoulder_plane`, motor centerline
- `shoulder_mount_right` — mirror on **−Y**
- `waist_adapter_mount` — center of waist bay, bottom rail interface

---

## 1. Shoulder roll mount bracket (L / R)

**Purpose:** Tie one RS03 to the **outer vertical 2020 post** at shoulder height; roll axis **+X** (motor shaft horizontal, left–right).

**Interfaces**

1. **Frame:** 2020 T-slot on `frame_2020_vertical_*_340` — use **M5 × 10** (or 12) + **M5 T-nut** in slot; minimum **2** screws, prefer **3** for print compliance.
2. **Motor:** Robstride RS03 mounting face (use vendor STEP holes as `Convert Entities` master).

**Rough form factor (starting envelope)**

| | mm |
|---|-----|
| Rail contact patch | **40 × 20** along extrusion slot direction |
| Motor pad | **100 × 100** (matches RS03 flange) |
| Thickness (normal to rail) | **8–12** PETG walls, **3–4** perimeter shells on P2S |
| Max protrusion into chest | Stay **inside** 85 mm half-width minus harness — target **≤ 15 mm** past inner rail face until poke validated |

**Modeling order (SolidWorks)**

1. New part → save as `marengo_shoulder_roll_mount_bracket_left_revA.SLDPRT`.
2. Custom properties: `process=print`, `material=PETG`, `revision=A`.
3. Sketch on plane normal to **+X**: outline motor flange + rib to rail.
4. Extrude; cut T-slot cleats or through-holes for T-nuts.
5. Insert `vendor_robstride_rs03_vendor` in a **temporary assembly** to verify clash with `marengo_torso_frame_asm_revA`.
6. Mirror body features (or derive) for `..._right_revA.SLDPRT`.
7. Insert into `marengo_torso_asm_revA` under folder **`Hardware`**; mate `mount_face` to layout `shoulder_mount_left` / `right`; mate motor to bracket (fixed in bracket, revolute in URDF at `joint_axis`).

---

## 2. Waist yaw adapter plate

**Purpose:** Mount `waist_yaw` RS03 in the **≥ 40 mm** waist bay; transfer load to **bottom 2020 rectangle** (110 × 85 mm rails) before pelvis adapter exists.

**Interfaces**

1. **Frame:** bottom front/rear **110 mm** rails + left/right **85 mm** rails (four rail seats or a single plate spanning the bay).
2. **Motor:** RS03 on top or center of plate; **Z** = waist rotation axis.
3. **Future:** pelvis ring bolts to **bottom** of this plate (not modeled in rev A).

**Rough form factor**

| | mm |
|---|-----|
| Plate outline | **≤ 105 × 80** (fits inside 110 × 85 inner rail span) |
| Plate thickness | **6–8** mm PETG |
| Motor standoff | Clear **40 mm** bay height — motor height **57 mm** may extend **below** bottom rails (*flag*: may need to lower motor into pelvis void or thin plate + pocket).

**Modeling order**

1. New part → `marengo_waist_yaw_adapter_revA.SLDPRT`.
2. Sketch plate on `mount_face` (bottom); extrude upward.
3. Add RS03 pocket / standoffs; `joint_axis` along **Z** through motor center.
4. Mate in torso asm: plate bottom coplanar with layout `pelvis_top` or bottom rail plane; motor on plate.

---

## Assembly insertion checklist

In `marengo_torso_asm_revA`:

- [ ] Folder **`Hardware`** contains three bracket instances
- [ ] Instance names include subsystem: `marengo_shoulder_roll_mount_bracket_left`, etc.
- [ ] Brackets **fixed** to frame; motors fixed to brackets (or motor floated with mate to `joint_axis` — pick one strategy and keep for URDF export)
- [ ] Re-run `marengo_hardware_coverage` (printed parts are not vendor assets; BOM rows below)
- [ ] Re-run `marengo_kinematics_consistency` — `waist_yaw`, `left_shoulder_roll`, `right_shoulder_roll` still found

---

## Print notes (Bambu P2S)

- Bed **256 mm** — all three parts fit individually; print shoulders as **pair** only if mirrored layout fits diagonally.
- PETG: **0.2 mm** layer, **3–4** walls, **15–20%** gyroid or honeycomb; orient **mount_face** on plate for best Z strength on waist adapter.
- Tap or heat-set inserts only where repeated service is needed (waist plate); shoulders can use T-nuts through bracket into extrusion.

---

## Open questions (resolve in CAD)

1. Does waist RS03 fit in **40 mm** bay without violating pelvis interface? If not, increase bay in layout (*requires user sign-off* on committed 380 mm frame).
2. Exact Robstride bolt pattern — confirm from vendor part holes vs datasheet.
3. Cable exit from RS03 toward chest — add `cable_exit` plane on bracket once routing is known.
