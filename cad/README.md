# CAD (local only)

SolidWorks models and vendor imports are **not stored in git**. Keep your working tree under `cad/` on your machine (or a separate `marengo-cad` clone at the same path). Joint names, axes, and limits live in [hardware/docs/kinematics.md](../hardware/docs/kinematics.md). After export, runtime reads [assets/urdf/marengo.urdf](../assets/urdf/marengo.urdf).

Root assembly: `assemblies/marengo.SLDASM` (local).

## What stays in this repo

| Path | Purpose |
|------|---------|
| `manifests/` | MCP conventions, vendor registry, design packages (text JSON) |
| `vendor/incoming/README.md` | Vendor intake notes |

## Local layout (gitignored)

| Path | Purpose |
|------|---------|
| `assemblies/` | Root `marengo.SLDASM` and limb sub-assemblies |
| `parts/` | Marengo-authored `.SLDPRT` by subsystem |
| `vendor/` | Vendor STEP and imported `.SLDPRT` |
| `vendor/incoming/` | Drop downloads here before promoting to `vendor/` |
| `exports/` | Neutral STEP/STL from MCP `solidworks_export` |

## Setup

Clone marengo for software, then restore CAD locally (backup, separate repo, or `git lfs pull` from an archive):

```powershell
# Example: sibling clone used only for CAD binaries
git clone <marengo-cad-archive-url> C:\code\marengo-cad
# Symlink or copy assemblies/parts/vendor into C:\code\marengo\cad\
```

SolidWorks MCP allowed root remains `C:/code/marengo` — your local `cad/` tree must exist there for automation.

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

| Name | Type | Purpose |
|------|------|---------|
| `urdf_link_frame` | coordinate system | Link origin for URDF |
| `joint_axis` | axis | Revolute axis (right-hand rule) |
| `cable_exit` | plane or point | Harness routing |
| `mount_face` | plane | Actuator or bracket interface |

See [hardware/docs/kinematics.md](../hardware/docs/kinematics.md) for joint naming.

## Hardware folder

Place fasteners, inserts, and purchased hardware under a top-level feature folder named `Hardware` (see `manifests/cad-conventions.json`). Use real Toolbox or vendor STEP components, not cosmetic-only geometry, so BOM and `marengo_hardware_coverage` can count them.

## BOM alignment

1. Add a row to [hardware/bom/master-bom.csv](../hardware/bom/master-bom.csv) with `part_id` aligned to [manifests/vendor-assets.json](manifests/vendor-assets.json) `id` when possible.
2. Run `marengo_hardware_coverage` before major assembly saves.

## Workflow

1. Model in SolidWorks under `assemblies/marengo.SLDASM` (local).
2. Run `marengo_design_review` before saving assemblies.
3. Export URDF manually (Brawner) → `assets/urdf/marengo.urdf`.
4. Run `scripts/validate-urdf.sh` and update [hardware/docs/kinematics.md](../hardware/docs/kinematics.md) if joints changed.

Open [marengo.code-workspace](../marengo.code-workspace) (marengo + solidworks-mcp). Build MCP after worker changes: `npm run build` in `solidworks-mcp`. Allowed CAD root is `C:/code/marengo` only.
