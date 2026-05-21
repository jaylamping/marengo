# Upper torso — 2020 extrusion (rev A)

Cut list and vendor CAD for the **committed** upper torso cage (`125 × 150 × 380 mm` outer, `85 × 110 mm` inner clear). SSOT envelope: [`kinematics.md`](kinematics.md#torso-frame-committed). **Built frame** uses **340 mm** corner posts (see below).

## Purchased stock

| Field | Value |
|-------|--------|
| **Product** | [IXGNIJ 4× 500 mm T-Slot 2020 black](https://www.amazon.com/dp/B08Y8KCJW4) |
| **ASIN** | `B08Y8KCJW4` |
| **CAD / registry** | `vendor_2020_black_extrusion` → `hardware/cad/vendor/vendor_2020_black_extrusion.SLDPRT` |
| **Profile** | European standard 2020 **T-slot** — 20×20 mm, **6063-T5**, black anodized, **6 mm** slots |

## Cut list (mm) — as built in CAD

| Part ID (BOM) | SW instance pattern | Config | Qty | Length |
|---------------|---------------------|--------|-----|--------|
| `frame_2020_vertical_340` | `frame_2020_vertical_{back\|front}_{left\|right}_340` | L340 | 4 | **340** |
| `frame_2020_bottom_left_085` | `frame_2020_bottom_left_085` | L085 | 1 | **85** |
| `frame_2020_bottom_right_085` | `frame_2020_bottom_right_085` | L085 | 1 | **85** |
| `frame_2020_top_left_085` | `frame_2020_top_left_085` | L085 | 1 | **85** |
| `frame_2020_top_right_085` | `frame_2020_top_right_085` | L085 | 1 | **85** |
| `frame_2020_bottom_front_110` | `frame_2020_bottom_front_110` | L110 | 1 | **110** |
| `frame_2020_bottom_rear_110` | `frame_2020_bottom_rear_110` | L110 | 1 | **110** |
| `frame_2020_top_front_110` | `frame_2020_top_front_110` | L110 | 1 | **110** |
| `frame_2020_top_rear_110` | `frame_2020_top_rear_110` | L110 | 1 | **110** |

**Cut stock per cage:** 4×340 + 4×85 + 4×110 = **2140 mm** (one 4×500 mm pack still short; order extra stock).

**340 mm verticals:** Intentional rev-a choice — corner posts span the chest cage; bottom **40 mm** waist bay remains in layout (`waist_bay_top`). Layout ICE `corner_posts` may still show 380 mm as envelope reference.

## Assemblies

| File | Role |
|------|------|
| `hardware/cad/assemblies/marengo_torso_frame_asm_revA.SLDASM` | 12× extrusion + layout jig (hide layout when done) |
| `hardware/cad/assemblies/marengo_torso_asm_revA.SLDASM` | Layout + frame sub-asm + RS03 layout motors |
| `hardware/cad/parts/marengo_torso_layout_revA.SLDPRT` | URDF refs (`urdf_link_frame`, `joint_axis`, planes) |

## Vendor part configs

On `vendor_2020_black_extrusion.SLDPRT`:

| Config | Use |
|--------|-----|
| L500 | Raw stick reference |
| L340 | Vertical posts |
| L085 | Left/right rails (85 mm) |
| L110 | Front/rear rails (110 mm) |

## Brackets

2020 T-slot (not V-Slot): 2028 corners (e.g. PETOX **B0BFRB35Z8**), M5 T-nuts, optional BLCCLOY **B08C9Q2TGW** 3-way.

## URDF

All rails rigid under link **`torso_link`**. Origin: layout **`urdf_link_frame`**. Waist: **`joint_axis`** (Z). Frame is geometry only — no per-stick URDF links.
