# Left arm swept envelope

2026-11-05, for the upper torso shell. Coordinates are CAD: mm, X lateral, Y up, Z forward. Robot Y is CAD X.

`left_arm_envelope.stl` is the volume the arm can reach. It is a 5 mm voxel surface, so faces are stepped, and it is STL only with no STEP version.

| Item | Value |
| --- | --- |
| Pitch | -50 to 180° |
| Roll | -15 to 180° |
| Upper-arm yaw | ±90° |
| Elbow | 0 to 135° |
| Bounds | X -175 to 675, Y -80 to 930, Z -505 to 505 |
| Volume | 0.48 m³ |

The envelope ignores collisions, so it passes through the torso. The hand reaches across the chest in some poses, and those poses need software limits.

**Provisional parts:** the forearm is a proxy (Ø63 mm shell plus a Ø60 mm hand stub), because neither is designed yet. Lower-arm yaw is not swept, since the proxy is round.

## Current shell

The tables below are for the first shell concept. For the wider frame (+38 mm per side, shell `fa63d48`) use [TORSO_WIDE_VARIANT.md](TORSO_WIDE_VARIANT.md), and `shell_gaps.json` for the full results.

The P2 bracket sweeps out to 88 mm from the pitch axis, so a 62 mm socket does not clear it when the arm is raised. The shell needs a socket of about 93 mm radius.

## Gap to the shell, first concept

Wall used: X 108 down to 170 mm below the shoulder, tapering to X 82 at the frame bottom, Z -88 to 110, flat and stopping at shoulder height. This is the widest value given, so the result is conservative at the shoulder, and it ignores any corner rounding of the chest.

| Inward roll | Tube only | Whole upper arm, including stub carrier | P2 to X 97.5 plane |
| --- | --- | --- | --- |
| 15° | **-13.1 mm** | **-29.6 mm** | -4.9 mm |
| 10° | 0.2 mm | -10.4 mm | -1.6 mm |
| 8.25° | 5.0 mm | | |
| 7.5° | | | 0.2 mm |
| 5.75° | | 5.2 mm | |
| 5° | 13.0 mm | 8.0 mm | 2.2 mm |
| 1.5° | | | 4.9 mm |
| 0° | 23.3 mm | 23.3 mm | 6.2 mm |

**15° inward roll is not achievable with this shell.** The tube is 13 mm into the chest side across pitch -30 to 40°.

For 5 mm clearance the limits are:

- Tube alone: 8.25° inward
- Whole upper arm: 5.75° inward, worst at pitch 35° where the carrier meets the front corner of the chest
- P2 bracket against a flange at X 97.5: 1.5° inward

P2 is the tightest. It is the reused L04B part, and its inner edge sits about 50 mm from the pitch axis, right over the bolt ring. The options are a roll stop at about 5° inward with 2.2 mm at P2, or a trim of P2 to keep a 5° stop with 5 mm. The URDF still has 15° (-0.26 rad). I have not changed it, because this needs a decision. The elbow fork is not designed and will reduce the whole-arm figure.

Pitch alone cannot cause contact with these planes, because it does not change CAD X.

## Pitch RS03 outer-ring interface

Source: RS03 manual section 1.1, the vendor STEP and the bracket CAD. Each RS03 has two sets of M4 threads on the 98 mm circle, at the same eight positions (23, 67, 113, 157, 203, 247, 293, 337°).

| Set | Depth | Used now by | Access |
| --- | --- | --- | --- |
| Outboard face, 8 × M4 | 8 mm (manual) | **Nothing. Free for the shell** | From outboard. Turn the arm or remove P1 |
| Inboard side, 8 × M4 | About 8.5 mm (STEP, no manual figure) | Torso bracket, M4 SHCS through it. The vendor model shows factory cap screws in four of these positions | From inside the cage |

Do not use the 6 × M4 × 6 or the 3 × Ø4 pin holes on the Ø30.36 circle. They are on the rotating output and carry P1.

**Housing face and stack:**

| X | Feature |
| --- | --- |
| 94.5 | Housing outboard face, ring of M4 holes |
| 97.0 | Output face (Ø70 stub, 2.5 mm proud) |
| 97.5 | Back of a 3 mm flange |
| 101.75 | Nearest P1 surface in the bolt ring zone (r 44 to 54 mm) |

**Screws for a shell flange:** the outboard threads are unused, so nothing needs extending. Use a new M4 × 10 button head. That gives 7 mm engagement with a 3 mm flange and 8 mm with 2 mm, and the manual says not to exceed 8 mm, so do not go longer with a 2 mm flange. A button head (2.2 mm) leaves 2 mm to P1. A socket cap head (4 mm) leaves only 0.25 mm, so avoid it. Screw lengths on the inboard side are not recorded anywhere in the repo or CAD, so I can't give their current engagement. Check on the robot.

**Flange shape:** an annulus with a bore of at least Ø72, so it clears the rotating Ø70 stub.

**Sweep check against X 97.5:**

| Part | Lowest X | Result |
| --- | --- | --- |
| P1 | 97.0, only within r 24 mm | Clear of a flange ring. Stays at X 101.75 or above for r 24 to 54 mm |
| Roll RS03 | 120.25 | Clear. It moves with pitch only |
| P2 | 103.7 at zero roll | **Crosses X 97.5 past 7.5° inward** |
| Yaw RS02 | 106.5 at 15° inward | Clear |

## Regenerating

`sweep.py` and `shell_gap.py` need STL exports of the arm parts, `cad_export.json` and `component_boxes_mm.json` from the layout folder on Joey-PC. `gaps.json` has the full results by pitch.
