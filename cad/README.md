# CAD (local only)

SolidWorks models and vendor imports are **not stored in git**. Keep your working tree under `cad/` in the host checkout (`J:\code\marengo\cad`); see "Folder layout" below. Joint names, axes, and limits live in [hardware/docs/kinematics.md](../hardware/docs/kinematics.md). After export, runtime reads [assets/urdf/marengo.urdf](../assets/urdf/marengo.urdf).

Root assembly: `marengo.SLDASM` at the top of the local CAD tree.

## What stays in this repo

| Path | Purpose |
|------|---------|
| `README.md` | This file, including the layout rule below |
| `manifests/` | MCP conventions, folder layout (`cad-layout.json`), vendor registry, design packages (text JSON) |
| `vendor/incoming/README.md` | Vendor intake notes |

Everything else under `cad/` is gitignored. Near-final exports and renders that should be viewable anywhere are copied into `hardware/designs/<design>/` (STEP/STL/3MF via Git LFS, PNG in plain git).

## Folder layout (rule)

The single CAD home is **`J:\code\marengo\cad`** on Joey-PC. There is no other CAD root: no
`marengo-cad`, no `codex`/`task`/tool-named folders, no loose files. Folders are split by **body
region** (anatomy), with a few **function** folders beside them. Left/right goes in the file
name, not the folder.

```
cad/
  README.md, manifests/       tracked
  marengo.SLDASM              root robot assembly
  # body regions: native parts + that region's assemblies
  torso/  waist/  shoulder/  upper-arm/  elbow/  forearm/  hand/
  arm/                        whole-arm assemblies only (marengo_arm_left_asm, marengo_arm_right_asm)
  shell/                      body shell panels
  hip/  leg/  head/           created when that work starts
  # function folders
  vendor/                     robstride/<rs00|rs02|rs03|rs04>/, frame-hardware/, compute/, fasteners/, incoming/
  reference/                  read-only references (torso STEP, datum reports)
  exports/<region>/           STEP/STL exported from native CAD
  prints/<region>/            3MF projects, G-code, exact-settings checklist
  reviews/<region>/<YYYYMMDD>_<topic>/   review packets, views, briefs, reconciled findings
  drawings/<region>/          SLDDRW / PDF drawings
  scripts/                    CAD automation scripts
  # scratch: never referenced by a kept assembly
  _wip/<topic>/               in-progress studies, layout experiments, render copies
  _sim/<region>/              .CWR and other simulation results
  _archive/                   superseded versions, old copies, SolidWorks .checkpoints
```

Rules:

1. Every native part or assembly sits in exactly one region folder, `vendor/`, or a scratch folder. Only `marengo.SLDASM` sits at the top.
2. Names follow [manifests/cad-conventions.json](manifests/cad-conventions.json) (see Naming below).
3. WIP, simulation and checkpoint output goes only in `_wip/`, `_sim/`, `_archive/`. Date-stamped folders are allowed only there and in `reviews/`.
4. Move or rename native files **inside SolidWorks** (Pack and Go or SOLIDWORKS Explorer), never with a plain file move, so assembly references survive. Only one session drives SolidWorks at a time.
5. Run the checker before and after CAD work; it must report 0 errors:

```powershell
python J:\code\marengo\tools\cad\check_cad_layout.py J:\code\marengo\cad
```

The allowed folders, extensions and forbidden names live in [manifests/cad-layout.json](manifests/cad-layout.json). Add a new region there (and here) before creating its folder.

## Setup

Software and local CAD sources live together at **`J:\code\marengo`** on Windows.
Open that host checkout for software and SolidWorks
([ADR 0018](../docs/decisions/0018-windows-macos-software-home.md)).

If CAD binaries are restored from backup/LFS archive, put them under that same
tree's `cad/`. macOS uses a host checkout of the same remote repository; ignored SolidWorks files require their own backup and transfer policy.

SolidWorks MCP allowed root is `${workspaceFolder}` (the host workspace) — your
local `cad/` tree must exist there for automation.

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
cd J:\code\solidworks-mcp
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

Open [marengo.code-workspace](../marengo.code-workspace) from `J:\code\marengo` (marengo + solidworks-mcp). Build MCP after worker changes: `npm run build` in `J:\code\solidworks-mcp`. Allowed CAD root is `${workspaceFolder}`.
