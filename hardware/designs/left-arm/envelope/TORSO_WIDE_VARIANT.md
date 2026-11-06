# Wider torso variant, +38 mm per side

2026-11-05. A study only: no torso CAD, master layout or URDF has been changed. Member sizes come from `marengo_torso_asm` and need checking against the real frame.

CAD uses X lateral, which is robot Y.

## Positions

| Item | Now | At +38 |
| --- | --- | --- |
| Frame outer | ±75 (150 wide) | ±113 (226 wide) |
| Inner clear width | 110 | 186 |
| Pitch RS03 rear faces | ±40.4 | ±78.4 |
| Pitch RS03 housing face | ±94.5 | ±132.5 |
| Pitch RS03 output face | ±97 | **±135** |
| Back of a 3 mm collar flange | ±97.5 | ±135.5 |
| Arm line, shoulder to wrist | ±170.49 | **±208.49** |

So ±208.5 is right for the joint line. ±110.5 is not a position in this stack. The face is ±135, and `scene.js` agrees, with `FACE = 97.5 + FW`.

## Frame members

| Member | Qty | Now | At +38 | Change |
| --- | --- | --- | --- | --- |
| Top and bottom front/rear rails, X direction | 4 | 110 mm | **186 mm** | Recut or replace |
| Corner posts | 4 | 340 mm | 340 mm | None |
| Top and bottom side rails, Z direction | 4 | 100 mm | 100 mm | Move out 38 mm |
| Shelf side rails, Z direction, at Y 354 to 374 | 2 | 100 mm | 100 mm | Move out 38 mm |
| 2028 corner brackets | 20 | | | Same parts. Those on X rails shift with the rail ends |

That is 304 mm more extrusion, about 150 g assuming 0.5 g/mm.

## Other parts

| Part | Effect |
| --- | --- |
| Pitch brackets | Same parts, moved 38 mm with the side rails, so they stay 156.8 mm apart at the inner faces instead of 80.8 mm |
| Compute shelf | **Must change.** It spans the full 110 mm between the rails, so it needs 186 mm or 38 mm adapters each side |
| Pi 5 and standoffs | No conflict if centred. It keeps the same offset on the wider shelf |
| Waist RS03 | Stays central. The adapter is not in this assembly, and the doc only says it bolts to the bottom rectangle. **Check it on the robot:** front/rear rails are fine, side rails would need a new adapter |
| PDB | Not in the CAD, so I can't check it. It only gains space |
| Torso layout part | X bounds ±130 need updating with the frame |
| Harness | Shoulder runs grow about 38 mm per side |
| URDF and docs | Shoulder origin moves to ±208.49. `kinematics.md` and both torso docs need new widths |
| Right arm | Same shift, and it is still to be designed from this skeleton |
| Waist yaw | Arm mass sits 38 mm further out, which raises yaw inertia. Not calculated |

## Gap check

Shell `fa63d48` with `fw=38`, `wx=10`. Section is the concept's superellipse, not a flat wall. Each part is swept over pitch -50 to 180° and, for the upper arm and elbow RS02, yaw ±90°. P1 and the roll RS03 follow pitch only. The collar bore (r 43) is treated as open.

| Part | Min gap at 0° roll | Min gap at 5° inward | Where, at 5° | With a r 93 socket, at 5° |
| --- | --- | --- | --- | --- |
| P1 to collar | 4.2 mm | 4.2 mm | Back of the collar ring | 4.2 mm |
| Roll RS03 | 42.1 mm | 42.1 mm | Socket | 42.1 mm |
| P2 and spacer | **-13.9 mm** | **-16.0 mm** | Top of shoulder, pitch 150° | 2.2 mm, collar face at pitch -50° |
| Yaw RS02 | 7.3 mm | **-0.3 mm** | Shoulder point, pitch 150° | 0.0 mm |
| Tube | 13.1 mm | 2.1 mm | Shoulder point, pitch 140° | 2.1 mm |
| Upper arm with carrier, elbow RS02 | 12.4 mm | 1.3 mm | Shoulder point, pitch 140°, yaw ±90° | 1.3 mm |

Outward and overhead sweeps, roll 10 to 170°: P2 is -9.8 mm with the r 62 socket and 15.5 mm with r 93. Everything else stays at 4.2 mm or more.

**Two shell changes clear this:**

1. Socket radius 93 mm instead of 62 mm. P2 sweeps out to r 88 around the pitch axis, so r 62 cannot clear it.
2. Shoulder point half-width 155 mm or less at the height near Y 505, instead of 162 mm. The arm passes there at pitch 140 to 150° with inward roll.

That leaves P1 at 4.2 mm, or 2 mm to a button head, and P2 at 2.2 mm. Both are the existing roll stop and collar issues, not width issues.

| Height below shoulder axis | Shell half-width | Max for 5 mm at 5° inward, outside r 62 | Limited by |
| --- | --- | --- | --- |
| -10 (above) | 157 | 140.8 | P2 |
| 55 | 155 | 139.6 | P2 |
| 111 | 139 | 154.0 | Upper arm |
| 160 | 116 | 139.4 | Upper arm |
| 200 | 108 | 134.8 | Upper arm |

The two P2 rows fall away with the larger socket. The sides are otherwise clear by 15 mm or more.

For reference, the +25 mm option against shell `3265ea6` gives the same pattern: P2 -16.0 mm, upper arm 1.1 mm before the script fix to P1 and the bore. `shell_gaps.json` holds the current base shoulder and +38 results.
