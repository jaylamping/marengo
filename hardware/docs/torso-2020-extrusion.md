# Upper torso — 2020 extrusion (rev A)

Cut list and vendor CAD for the **committed** upper torso cage (`125 × 150 × 380 mm` outer, `85 × 110 mm` inner clear). SSOT: [`kinematics.md`](kinematics.md#torso-frame-committed).

## Purchased stock (locked)

| Field | Value |
|-------|--------|
| **Product** | [IXGNIJ 4× 500 mm T-Slot 2020 black](https://www.amazon.com/dp/B08Y8KCJW4) |
| **ASIN** | `B08Y8KCJW4` |
| **Profile** | **European standard 2020 T-slot** — 20 × 20 mm, **6063-T5**, black anodized |
| **Slots** | **4** grooves, **6 mm** slot width (per listing); M3 / M4 / M5 T-slot nuts |
| **Not** | OpenBuilds **V-Slot**, McMaster silver **47065T101**, plain square tube |

Registry id: `extrusion_2020_profile`.

### Stock vs torso cut list

| | mm |
|---|-----|
| **Cuts needed** (one cage) | **2300** (see below) |
| **One 4-pack** | 4 × 500 = **2000** → **short by 300 mm** |
| **Suggested order** | Second 4-pack of 500 mm, **or** same ASIN in **1000 mm** length (listing offers 150–2000 mm) |

## Cut list (mm)

Outer OD − 2×20 mm profile → inner clear **85 × 110**.

| Part ID | Qty | Cut length | Role |
|---------|-----|------------|------|
| `frame_2020_vertical_380` | 4 | **380** | Corner posts |
| `frame_2020_rail_x_085` | 4 | **85** | Front/back rails (+X) |
| `frame_2020_rail_y_110` | 4 | **110** | Left/right rails (+Y) |

Layout reference features: `corner_posts`, `top_rail_*`, `bottom_rail_*` in `marengo_torso_layout_revA.SLDPRT`.

## Brackets (compatible ecosystem)

Use **2020-series T-slot** hardware (not V-wheel plates):

- **Corner brackets:** 2028-style (e.g. Amazon PETOX **B0BFRB35Z8** — 30× 2028 corners + M5 nuts)
- **3-way corners:** BLCCLOY **B08C9Q2TGW** (listed with IXGNIJ)
- **Nuts:** M5 hammer / sliding T-nuts for 6 mm slot

## Vendor CAD workflow (GrabCAD SolidWorks)

**CAD source:** [GrabCAD — 2020 Aluminium Extrusions](https://grabcad.com/library/2020-aluminium-extrusions-1) (SolidWorks). Physical stock: IXGNIJ **B08Y8KCJW4**.

1. Copy the downloaded `.SLDPRT` (or extract from zip) to  
   `hardware/cad/vendor/incoming/grabcad_2020_aluminium_extrusions.SLDPRT`
2. Open in SolidWorks. Pick the **2020 T-slot** configuration (not 2040/4040, not V-slot). Use **500 mm** if offered, or trim longer stock.
3. **Verify** on real extrusion: outer **20 × 20 mm**, slot ~**6 mm**, **4** grooves.
4. **Save As** canonical copy:  
   `hardware/cad/vendor/vendor_ixgnij_2020_tslot_eu_black_vendor.SLDPRT`  
   (appearance: **black anodized aluminum**)
5. **Optional:** export STEP AP214 →  
   `hardware/cad/vendor/vendor_ixgnij_2020_tslot_eu_black_vendor.step`
6. Configurations **L380**, **L085**, **L110** for torso cuts; instance names include `part_id`.
7. `marengo_hardware_coverage` on frame assembly.

**Fallback** if profile mismatches stick: Misumi **HFS5-2020** STEP AP214 from PARTcommunity.

## Axes

**+X** forward, **+Y** left, **+Z** up; pelvis bottom = `urdf_link_frame`.

## Layout vs frame assembly

Keep `marengo_torso_layout_revA` for envelopes/URDF refs; put real extrusion in **`marengo_torso_frame_asm_revA`** (or equivalent) mated to layout planes.
