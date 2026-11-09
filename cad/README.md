# CAD

The **latest version of every CAD file** lives in git under `cad/`: SolidWorks parts, assemblies
and drawings, STEP/STL exports, print 3MF/G-code, vendor models, references and review packets.
Binary CAD goes through **Git LFS** (patterns in the repo's `.gitattributes`); PNG, PDF, text
and scripts are plain git. The working tree is the host checkout `J:\code\marengo\cad` on
Joey-PC; see "Folder layout" below. Joint names, axes, and limits live in
[hardware/docs/kinematics.md](../hardware/docs/kinematics.md). After export, runtime reads
[assets/urdf/marengo.urdf](../assets/urdf/marengo.urdf).

Root assembly: `cad/marengo.SLDASM` at the top of the CAD tree.

## What goes in git

**Git holds the design of record. Joey-PC holds everything that led to it.**

| Versioned (tracked; binaries via LFS) | Ignored (local to Joey-PC) |
|---------------------------------------|----------------------------|
| The one current file per part/assembly: the file the robot assembly or a print uses today, in its region folder | Variants, studies, test and render copies: `_wip/<topic>/` |
| The STEP/STL export of a current part that something consumes (a print, a review, a URDF, another tool) or that is released, in `exports/<region>/`, with its source recorded in `manifests/exports.json` | FEA/solver results: `_sim/<region>/`, `*.CWR`, `*.STE` |
| The exact 3MF of anything actually printed, plus its G-code and settings checklist (`prints/<region>/`) | Pre-git old copies, duplicates, raw review logs: `_archive/` |
| Review packets at a gate: markdown and key renders (`reviews/<region>/<YYYYMMDD>_<topic>/`) | SolidWorks `.checkpoints/`, `~$` lock files, `Backup of *` / `Backup (n) of *` copies |
| Vendor models, references, drawings, CAD scripts and macros, manifests | Anything 100 MB or larger; the writer lock `cad/.cad-writer.lock` |

### Process

1. **Take the writer lock** before opening SolidWorks to change anything in `cad/`:
   `python tools/cad/cad_lock.py acquire <thread-or-name>`, and set `CAD_WRITER=<same name>` in
   that shell. Release it after your commit (`... release <name>`). Many sessions share the
   one checkout on Joey-PC; the pre-commit hook refuses a commit touching CAD unless
   `CAD_WRITER` holds the lock. Hold it too while switching branches, resetting or pulling in
   that checkout. A lock untouched for 4 hours is stale: make sure that session has stopped and
   closed its SolidWorks, then `acquire <name> --take-stale`.
2. **Work in `_wip/`.** New ideas, variants and experiments start there and never get committed.
3. **Promote when chosen.** A part moves into its region folder (with SolidWorks Pack and Go)
   when it is picked: it passes its gate, or an assembly or print starts using it. Stage it,
   then run `python tools/cad/check_sw_refs.py --all` (Windows; reads references without
   opening files, so it covers every part, assembly and drawing in git, parents included;
   it refuses if a scanned file's working copy differs from its staged copy): every reference
   must resolve to a committed or staged file in `cad/` outside the scratch folders, or the
   committed assembly would be broken for everyone else. Rebuild the top assembly (Ctrl+Q)
   once before the commit.
4. **Export what is consumed.** When a print, review, URDF or another tool uses the part, export
   its STEP/STL to `exports/<region>/` and record it:
   `python tools/cad/record_export.py exports/<region>/<file> <region>/<part>.SLDPRT --config <name> --settings "<STEP AP214, STL 0.01 mm/5 deg, ...>"`.
   The layout checker warns when the source changed after the export (stale) and fails when
   the export changed without a new record.
5. **Overwrite in place.** Region, exports, prints and drawings folders hold only the current
   version; git history is the version trail. `_v2`, `_revC`, `_final`, `(2)`, `Copy of` files
   there are errors.
6. **Commit at a gate, not every save.** Commit native files at the end of a design pass or a
   review gate, and squash interim saves before pushing: every pushed version is stored in
   LFS forever. The pre-push hook (`tools/cad/lfs_budget.py`, limits in
   `manifests/lfs-budget.json`) fails a push that adds more than 300 MB of new LFS data, takes
   the month past 1 GB, or takes storage past 9 GB (GitHub Free: 10 GiB storage, 10 GiB
   download a month). With Joey's OK for that push, set `CAD_BUDGET_OK=1` (the first `cad/`
   commit will need it). `python tools/cad/lfs_budget.py report` shows the totals. The script
   counts what the local history can see and dates objects by commit, so it is a lower bound:
   objects from deleted branches or rewritten history are still billed. Check the real number
   on GitHub (Settings, Billing, Git LFS) at the start of each month.
7. **Checks.** The hooks run them; by hand:
   `python tools/check_file_sizes.py --staged`, `python tools/cad/check_cad_layout.py cad`
   (the folder on disk) and `python tools/cad/check_cad_layout.py --repo` (what git tracks).
   No file may be 100 MB or more (LFS included); simplify or split it, or keep it in `_wip/`.
   `scripts/check.sh` and CI run the size check and `--repo` on every PR.
8. **Prints:** commit the exact 3MF that was printed, its G-code, and the settings checklist,
   overwriting the previous print of that part. The G-code is the true record of the print;
   Bambu Studio has changed saved 3MF settings on open before.

### Hooks

Install once per clone (Joey-PC and the Mac), and again after `git lfs install`:

```
python tools/cad/install_hooks.py
```

pre-commit checks staged sizes and, when CAD is staged, the writer lock and the `--repo`
layout of the index (the bytes being committed, including that CAD files are LFS pointers and
exports match `manifests/exports.json`). pre-push checks pushed sizes and the LFS budget for
all refs of the push together, then runs `git lfs pre-push` (and fails if Git LFS is missing).
Another existing hook is kept as `<hook>.local` and still runs. CI repeats the size and layout checks, so a
`--no-verify` commit still gets caught at the PR.

### One home for CAD files (Joey, 2026-11-08)

STEP, STL, 3MF, G-code and SolidWorks files live only under `cad/`. `hardware/designs/<design>/`
keeps the docs, renders, FEA reports and review notes and links to the `cad/` paths; it holds
no CAD file copies. `check_cad_layout.py --repo` fails on a CAD file tracked anywhere else.
Files that have not moved yet are listed in `manifests/layout-exceptions.txt` (reported as
HELD); each owning thread moves its files, repoints its scripts and deletes its line. Once the
list is empty the old `hardware/...` LFS patterns in `.gitattributes` go too.

### Reviews and renders

A packet is `reviews/<region>/<YYYYMMDD>_<topic>/` and holds markdown, key renders and small
data (md, txt, png, jpg, svg, pdf, csv, json). Limits: a render under 2 MB (about 1600 px),
no file over 10 MB, a packet under 15 MB and 10 images. The packet's markdown names the exact
design it reviewed with a `Pinned commit: <full 40-character SHA>` line, so links to
`https://github.com/jaylamping/marengo/blob/<sha>/cad/...` stay valid after the files are
overwritten. Raw logs, prompts, hash lists and bulky data stay in `_archive/reviews/`.

### Clones, CI and bandwidth

Every LFS download counts against the 10 GiB monthly bandwidth. CI checks out without LFS
content, and cloud sessions see pointer files; every check here reads sizes and hashes from
the pointers. A normal `git clone` on a machine with Git LFS installed DOES download all LFS
content, so clone with `GIT_LFS_SKIP_SMUDGE=1 git clone ...` unless you need the CAD, then fetch
only what you need, e.g. `git lfs pull --include "cad/shoulder/**"`
(`git lfs pull --include "cad/**"` fetches all of it).

### Backup

`_wip/`, `_sim/` and `_archive/` exist only on Joey-PC. Git is not their backup: keep them in
Joey-PC's own backup, or promote a `_wip/` design before it would hurt to lose it.

## Folder layout (rule)

The single CAD home is **`J:\code\marengo\cad`** on Joey-PC. There is no other CAD root: no
`marengo-cad`, no `codex`/`task`/tool-named folders, no loose files. Folders are split by **body
region** (anatomy), with a few **function** folders beside them. Left/right goes in the file
name, not the folder.

```
cad/
  README.md, manifests/       docs and rules
  marengo.SLDASM              root robot assembly
  # body regions: native parts + that region's assemblies
  torso/  waist/  shoulder/  upper-arm/  elbow/  forearm/  hand/
  arm/                        whole-arm assemblies only (marengo_arm_left_asm, marengo_arm_right_asm)
  shell/                      body shell panels
  hip/  leg/  head/  neck/    created when that work starts
  fixtures/                   test stands, jigs and rigs (not part of the robot)
  electronics/                board models (PDB etc.) for fit checks
  # function folders
  vendor/                     robstride/<rs00|rs02|rs03|rs04>/, frame-hardware/, compute/, fasteners/, incoming/
  reference/                  read-only references (torso STEP, datum reports)
  exports/<region>/           STEP/STL exported from native CAD
  prints/<region>/            3MF projects, G-code, exact-settings checklist
  reviews/<region>/<YYYYMMDD>_<topic>/   review packets, views, briefs, reconciled findings
  drawings/<region>/          SLDDRW / PDF drawings
  scripts/                    CAD automation scripts
  # scratch (gitignored): never referenced by a kept assembly
  _wip/<topic>/               in-progress studies, layout experiments, render copies
  _sim/<region>/              .CWR and other simulation results
  _archive/                   superseded versions, old copies, SolidWorks .checkpoints
```

Rules:

1. Every native part or assembly sits in exactly one region folder, `vendor/`, or a scratch folder. Only `marengo.SLDASM` sits at the top.
   Region folders hold only SLDPRT/SLDASM (and notes in `.md`); `arm/` holds only whole-arm assemblies; drawings go in `drawings/<region>/`, exports in `exports/<region>/`.
2. Names follow [manifests/cad-conventions.json](manifests/cad-conventions.json) (see Naming below).
3. WIP, simulation and checkpoint output goes only in `_wip/`, `_sim/`, `_archive/`. Date-stamped folders are allowed only there and in `reviews/`.
4. Move or rename native files **inside SolidWorks** (Pack and Go or SOLIDWORKS Explorer), never with a plain file move, so assembly references survive. Only one session drives SolidWorks at a time.
5. Run the checker before and after CAD work; it must report 0 errors (held paths are listed, not failing):

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

1. Model in SolidWorks under `cad/marengo.SLDASM`.
2. Run `marengo_design_review` before saving assemblies.
3. Export URDF manually (Brawner) → `assets/urdf/marengo.urdf`.
4. Run `scripts/validate-urdf.sh` and update [hardware/docs/kinematics.md](../hardware/docs/kinematics.md) if joints changed.

Open [marengo.code-workspace](../marengo.code-workspace) from `J:\code\marengo` (marengo + solidworks-mcp). Build MCP after worker changes: `npm run build` in `J:\code\solidworks-mcp`. Allowed CAD root is `${workspaceFolder}`.
