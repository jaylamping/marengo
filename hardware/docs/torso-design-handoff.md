# Marengo torso design — agent handoff

Brief for continuing CAD/layout work. **SSOT for numbers:** [`kinematics.md`](kinematics.md). **Only the upper torso 2020 frame is committed**; everything else is draft unless noted.

---

## Robot goals (context)

- **5 ft (1524 mm)** standing biped, no wheels.
- **G1** upper-body proportions; **R1** lower-body slimness (narrow hips, shrouded legs).
- **Not** hourglass / feminine taper — athletic straight sides; smooth printed PETG shell taper (shell is *draft*, not skeleton).
- **23 DOF** v1, no dexterous hands. Head skipped in v1 CAD.
- **Actuators:** RS04 hip pitch + knees; RS03 outer hips, waist, shoulders; RS02 ankles + arm chain.
- **Materials:** 2020 aluminum internal frame; PETG/ABS panels/shell; pelvis v1 PETG (*draft*).
- **Power (*draft*):** large fixed **48 V** pack in pelvis (max Wh); **Milwaukee M18 HD12** aux bay upper back (18 V adjunct, not motor bus).
- **Comms:** OTA-first; removable front/back panels (screws); G1-style waist split for service.
- **Printer:** Bambu P2S — **256 mm** bed; panels >256 mm must split.

---

## Design evolution (what we tried)

1. Full-length **spine ladder** (2× 2020) + short wide chest box → dropped.
2. Full-height narrow cage → too constraining.
3. **Current:** single **upper torso 2020 box** + separate **pelvis ring** (wider, *draft*).

---

## Committed: upper torso 2020 frame

Single aluminum cage, CAD name target: `marengo_upper_torso_frame`.

| | Depth (X) | Width (Y) | Height (Z) |
|---|-----------|-----------|------------|
| **Outer (2020 OD)** | **125** | **150** | **380** |
| **Inner clear** | **85** | **110** | **~300** above waist bay |

| Feature | mm | Notes |
|---------|-----|-------|
| **Waist bay** (bottom of 380) | **≥ 40** | `waist_yaw` RS03 + adapter; **no shelves** |
| **Chest zone** (above waist bay) | **~340** outer | Pi, PDB, harness |
| **Bottom interface** | — | Bottom rail → waist adapter → pelvis (*pelvis draft*) |
| **Top** | Z **495** from pelvis bottom | Shoulder RS03 on outer Y faces |

**Axes:** origin = pelvis bottom center (`urdf_link_frame`); **+X** forward, **+Y** left, **+Z** up.

**Vertical stack (when pelvis stays 115 mm *draft*):** pelvis 115 + torso 380 = **495 mm** to shoulder plane; full robot crown stack in `kinematics.md`.

---

## Draft: RS03 mounting (shoulders + waist)

- **Shoulder roll RS03:** mount on **outer Y face** of torso frame; axis **+X**.
- **Controller housing** stays **inside** 85×110 clear; **motor body pokes out** in Y (~**40 mm**/side *draft*) → ~**230 mm** outer shoulder width (*draft*: 150 + 80).
- **Waist RS03:** in **40 mm waist bay**; pelvis does **not** bolt directly to box — **adapter plate** between pelvis ring and torso bottom rail.
- Import **`vendor_robstride_rs03_vendor.*`** before freezing brackets (module ~106×106×56 mm).

---

## Draft: pelvis & power

| Item | Draft size | Notes |
|------|------------|-------|
| Pelvis ring outer | ~**215 × 260 × 115** | Wider than torso; hip mounts splayed ~35° |
| Pelvis inner | ~**175 × 220** | Sized for main pack |
| Main battery envelope | **185 × 165 × 90** | 13S class, max Wh, fixed in pelvis |
| M18 HD12 bay | **168 × 98 × 112** | Upper back; not inside 85×110 spine |

---

## Draft: shell & layout

- Shell wraps frame with gap for harness; **shoulder line ~230 mm** (*draft*); waist taper in **PETG only** (frame stays 125×150).
- **Hip span ~250 mm** (*draft*); legs 6-DOF G1 tree; shrouded actuators.
- Arm segments *draft*: upper 215, forearm 195, hand stub 90; reach ~480 mm.

---

## CAD files & tools

| Path | Status |
|------|--------|
| `hardware/cad/parts/marengo_torso_layout_revA.SLDPRT` | Layout skeleton — mostly empty |
| `hardware/cad/assemblies/marengo_torso_asm_revA.SLDASM` | Sub-asm; layout part inserted |
| `hardware/docs/kinematics.md` | SSOT — joint names, limits, committed frame |
| `hardware/docs/cad-standards.md` | Naming, URDF ref geometry |
| SolidWorks MCP | Audit tools; worker path fixed in `solidworks-mcp` |

**Layout part next steps:** model committed box + waist bay + draft shoulder/hip points; named refs per `cad-standards.md` (`urdf_link_frame`, `joint_axis`, `cable_exit`, etc.).

**PETG actuator brackets (rev A):** [`torso-actuator-brackets-revA.md`](torso-actuator-brackets-revA.md) — shoulder roll mounts (L/R) + waist yaw adapter for the three RS03 layout instances.

**2020 extrusion:** [`torso-2020-extrusion.md`](torso-2020-extrusion.md) — `vendor_2020_black_extrusion`, `marengo_torso_frame_asm_revA` (12× named cuts, **340 mm** verticals), inserted in `marengo_torso_asm_revA`.

---

## Concept images (non-CAD)

Under repo `assets/` (paths may vary by machine):

- `marengo-torso-final-dimensions.png` — orthographic draft dims
- `marengo-torso-final-dimensions-iso.png` — isometric cutaway
- Earlier skeleton/torso concepts — **superseded** by committed frame above

---

## Do not change without user sign-off

- **125 × 150 × 380 mm** outer torso frame
- **85 × 110 mm** inner clear
- **≥ 40 mm** waist bay at bottom of frame

## Safe to iterate

- Pelvis dimensions, battery bay, M18 placement, shoulder poke mm, shell taper, hip splay, head, panel splits.

---

## Active bench (unchanged)

4-DOF arm bring-up: `config/robot.yaml`, `assets/urdf/arm_4dof.urdf`. Full humanoid templates: `config/robot_humanoid.yaml`, `config/motors_humanoid.yaml`.
