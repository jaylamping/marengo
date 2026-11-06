# Left arm URDF

Generated from the SolidWorks skeleton. Do not edit `left_arm.urdf` by hand. This is separate from
`assets/urdf/marengo.urdf`, which the bench still runs on.

To regenerate on Joey-PC:

1. Run `export_from_solidworks.py` in the layout folder and copy `cad_export.json` here.
2. Run `python generate_urdf.py`. It also runs the checks; `--check` runs them alone.

Conventions:

- Frame: REP-103, X forward, Y left, Z up. `torso_link` is on the pitch axis at the torso midplane.
- Joints: `left_shoulder_pitch`, `left_shoulder_roll`, `left_upper_arm_yaw`, `left_elbow_pitch`, `left_lower_arm_yaw`.
- Positive motion: pitch raises forward, roll abducts, yaw rotates outward, elbow flexes. The right arm uses the
  same meanings, which negates its roll and yaw axes.

Mass comes from SolidWorks geometry, scaled to `mass_overrides.json`. Put weighed values there; `null` uses solid PETG.
Each whole actuator is carried on its stator side, since rotor mass isn't known.

Limitations:

- The upper arm is in upper_arm_link and the fork in forearm_link. The hand is a placeholder estimate, not a design: 200 g, COM 90 mm past the RS00 output flange, 190 mm long, as a solid box. The forearm link also carries a 300 g placeholder pack, COM 140 mm from the elbow axis. That point sits inside the RS00, so the pack's real location is still open. Change `L_HAND`, the `HAND_` and the `PACK_` values in `generate_urdf.py`. The fork is not a layout component, so its entry in cad_export.json comes from the exported STL (volume within 0.1% of SolidWorks) and a SolidWorks re-export drops it: re-add it, or add the fork to the layout.
- Joint limits are provisional. Elbow stops at 133° (see ../ELBOW_COMPACTION.md). Inward roll at 15° is a hand calculation, not a sweep.
- `velocity` is the MIT protocol range, not rated speed.
