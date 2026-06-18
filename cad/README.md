# CAD

SolidWorks models, vendor imports, and CAD manifests for Marengo. Joint names, axes, and limits live in [hardware/docs/kinematics.md](../hardware/docs/kinematics.md). After export, runtime reads [assets/urdf/marengo.urdf](../assets/urdf/marengo.urdf).

Root assembly: `assemblies/marengo.SLDASM`.

## Layout

| Path | Purpose |
|------|---------|
| `assemblies/` | Root `marengo.SLDASM` and limb sub-assemblies |
| `parts/` | Marengo-authored `.SLDPRT` by subsystem |
| `vendor/` | Vendor STEP and imported `.SLDPRT` |
| `vendor/incoming/` | Drop downloads here before promoting to `vendor/` |
| `exports/` | Neutral STEP/STL from MCP `solidworks_export` (optional mirror of `assets/`) |
| `manifests/` | Conventions, vendor registry, design packages (MCP audits) |

Large binaries use Git LFS (root [.gitattributes](../.gitattributes)).

## Naming

- Authored part: `marengo_<subsystem>_<part>.SLDPRT` (example: `marengo_shoulder_pitch_mount_bracket_left.SLDPRT`)
- Authored assembly: `marengo_<subsystem>_asm.SLDASM` (example: `marengo_torso_asm.SLDASM`)
- Vendor import: `vendor_<mfg>_<mpn>_vendor.*` (STEP or `.SLDPRT` after import)

Regex targets: [manifests/cad-conventions.json](manifests/cad-conventions.json).

## Custom properties (authored parts)

Required on every Marengo-authored part and every vendor `.SLDPRT` in an assembly (MCP `marengo_design_review` checks the tree).

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

Instance and joint names must match [hardware/docs/kinematics.md](../hardware/docs/kinematics.md).

## Hardware instances

Place fasteners, inserts, and purchased hardware under a top-level feature folder named `Hardware` (see `manifests/cad-conventions.json`). Use real Toolbox or vendor STEP components, not cosmetic-only geometry, so BOM and `marengo_hardware_coverage` can count them.

## Vendor CAD and BOM

1. Add a row to [hardware/bom/master-bom.csv](../hardware/bom/master-bom.csv) with `part_id` aligned to [manifests/vendor-assets.json](manifests/vendor-assets.json) `id` when possible.
2. Stage vendor STEP under `vendor/incoming/`, then promote to `vendor/`.
3. Run MCP `marengo_vendor_registry_summary` and `marengo_hardware_coverage` before calling an assembly build-ready.

## Export to URDF (manual)

1. Model in SolidWorks under `assemblies/marengo.SLDASM`.
2. Run MCP `marengo_urdf_readiness` and `marengo_kinematics_consistency`.
3. Export with Brawner / sw2urdf to [assets/urdf/marengo.urdf](../assets/urdf/marengo.urdf).
4. Run MCP `marengo_urdf_export_postcheck`.
5. Copy meshes per [scripts/export-urdf.sh](../scripts/export-urdf.sh), then `./scripts/urdf-to-mjcf.sh` and `just check`.

## MCP session

Open [marengo.code-workspace](../marengo.code-workspace) (marengo + solidworks-mcp). Build MCP after worker changes: `npm run build` in `solidworks-mcp`. Allowed CAD root is `C:/code/marengo` only.
