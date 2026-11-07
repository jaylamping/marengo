# Torso harness solids

`harness.py` sweeps each wire as a solid round section along a filleted polyline (bend radius per wire class) and writes
`harness.step` (one solid per conductor, labelled) and `harness.json` (lengths). Torso coordinates as in `../generator/frame.py`.
Import `harness.step` into the torso assembly at the origin.

Status 2026-11-07: frame-side segments only (brake leads from both HS50s, 12 V brick in/out). Every board-bound run stops at a stub
until Joey places the power board in the torso; the board ends, the 8 AWG pack pair, the limb pigtails and the signal cables come from
the power board thread's `hardware/electrical/pdb/design/layout/HARNESS_INTERFACE.md`. Bounding-box clash check against
`../generator/parts.json`: clear.
