# Marengo CAD standards

SolidWorks models live under [`hardware/cad/`](../cad/). Manifests in [`hardware/manifests/`](../manifests/) drive MCP audits. After manual export, runtime reads [`assets/urdf/marengo.urdf`](../../assets/urdf/marengo.urdf).

## Folder layout

| Path | Purpose |
|------|---------|
| `hardware/cad/assemblies/` | Root `marengo.SLDASM` and limb sub-assemblies |
| `hardware/cad/parts/` | Marengo-authored `.SLDPRT` by subsystem |
| `hardware/cad/vendor/` | Immutable vendor STEP and imported `.SLDPRT` |
| `hardware/cad/exports/` | Neutral STEP/STL from MCP `solidworks_export` (optional mirror of `assets/`) |

## Naming

- **Authored part:** `marengo_<subsystem>_<part>.SLDPRT` (example: `marengo_shoulder_pitch_mount_bracket_left.SLDPRT`)
- **Authored assembly:** `marengo_<subsystem>_asm.SLDASM` (example: `marengo_torso_asm.SLDASM`)
- **Vendor import:** `vendor_<mfg>_<mpn>_vendor.*` (STEP or `.SLDPRT` after import)

Regex targets are in [`cad-conventions.json`](../manifests/cad-conventions.json).

## Custom properties (authored parts)

Required on every Marengo-authored part **and** every vendor `.SLDPRT` referenced in an assembly (MCP `marengo_design_review` checks the tree).

| Property | Authored example | Vendor import example |
|----------|------------------|------------------------|
| `process` | `cnc`, `print`, `layout` | `purchase` |
| `material` | `6061-T6`, `PETG` | `6063-T5`, `aluminum`, `RS03 actuator (vendor)` |
| `revision` | `A` | `vendor` |
| `owner` | handle or name | same as authored parts |

Batch-apply vendor properties (SolidWorks must be running):

```powershell
cd C:\code\solidworks-mcp
npm run build
npm run set:vendor-props
npm run promote:corner-bracket   # incoming → vendor_2028_corner_bracket_vendor + rewire torso asm
```

## Named reference geometry (URDF handoff)

Required on links and actuated joints before Brawner export:

| Name | Use |
|------|-----|
| `urdf_link_frame` | Link origin / URDF `<origin>` |
| `joint_axis` | Revolute/prismatic axis |
| `mount_face` | Interface plane |
| `shaft_axis` | Bearing / actuator bore |
| `bolt_circle` | Fastener pattern |
| `cable_exit` | Harness routing |
| `tool_access` | End-effector clearance |

Instance and joint names must match [`kinematics.md`](kinematics.md).

## Hardware instances

Place fasteners, inserts, and purchased hardware under a top-level feature folder named **`Hardware`** (see `cad-conventions.json`). Use real Toolbox or vendor STEP components — not cosmetic-only geometry — so BOM and `marengo_hardware_coverage` can count them.

## Vendor CAD and BOM

1. Add a row to [`master-bom.csv`](../bom/master-bom.csv) with `part_id` aligned to [`vendor-assets.json`](../manifests/vendor-assets.json) `id` when possible.
2. Stage vendor STEP under `hardware/cad/vendor/`.
3. Run MCP `marengo_vendor_registry_summary` and `marengo_hardware_coverage` before calling an assembly build-ready.

## Export to URDF (manual)

1. Model in SolidWorks under `hardware/cad/assemblies/marengo.SLDASM`.
2. Run MCP `marengo_urdf_readiness` and `marengo_kinematics_consistency`.
3. Export with **Brawner** / sw2urdf to [`assets/urdf/marengo.urdf`](../../assets/urdf/marengo.urdf).
4. Run MCP `marengo_urdf_export_postcheck`.
5. Copy meshes per [`scripts/export-urdf.sh`](../../scripts/export-urdf.sh), then `./scripts/urdf-to-mjcf.sh` and `just check`.

## MCP session

Open the multi-root workspace [`marengo.code-workspace`](../../marengo.code-workspace) (marengo + solidworks-mcp). Build MCP after worker changes: `npm run build` in `solidworks-mcp`. Allowed CAD root is **`C:/code/marengo`** only.
