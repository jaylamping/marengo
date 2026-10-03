# Right-arm URDF from CAD (2026-10-03)

Result of [handoff-2026-10-03-right-arm-urdf.md](handoff-2026-10-03-right-arm-urdf.md) steps 1–3, done on the Windows CAD host. Steps 4–5 (gravity preview and the bench hold) are still to run from the Pi session.

Source: `cad/assemblies/marengo_arm_right_asm.SLDASM` (local CAD, read only; nothing saved) through the SolidWorks MCP. Starting file: the Pi's `/opt/marengo/assets/urdf/marengo.urdf`, read 2026-10-03.

## What changed in `assets/urdf/marengo.urdf`

- Joint origins and the five arm link inertials now come from CAD.
- Hard limits are the Pi's taught limits (ADR 0017): pitch lower −1.43134, roll −0.099415…3.22308, elbow −1.63191…1.60728, lower-arm yaw ±3.24409. Soft limits and axes are unchanged.
- Visual cylinder lengths follow the new joint spacing.

| Joint origin (xyz, m) | Placeholder | CAD |
|---|---|---|
| `right_shoulder_pitch` | `0 0 0` | `0 0 0` (pitch actuator output face centre) |
| `right_shoulder_roll` | `0.05 0 0` | `0 0.07349 0` |
| `right_upper_arm_yaw` | `0.08 0 0` | `0.00115 -0.00001 0` |
| `right_elbow_pitch` | `0 0 -0.24` | `0 0 -0.23036` |
| `right_lower_arm_yaw` | `0 0 -0.20` | `0.00013 0.00006 -0.09968` (RS00 output face) |

The shoulder is close to spherical. The roll axis crosses the pitch axis 73.5 mm outboard of the pitch output face. The yaw axis passes 1.15 mm behind that point. The elbow axis crosses the yaw axis 230.4 mm below it, and the forearm (RS00) axis passes within 0.13 mm of the elbow axis.

| Link | Mass (kg) | COM in link frame (m) | Made of |
|---|---|---|---|
| `right_shoulder_pitch_link` | 1.0101 | −0.00378 0.06933 −0.00005 | pitch bracket + roll RS03 |
| `right_shoulder_roll_link` | 0.4925 | −0.00209 −0.00001 −0.07141 | roll bracket + upper-arm yaw RS02 |
| `right_upper_arm_link` | 0.5075 | 0.00003 −0.00779 −0.21516 | upper arm + elbow RS02 |
| `right_forearm_link` | 0.3910 | 0.00066 0.00342 −0.06048 | elbow piece + lower-arm yaw RS00 |
| `right_hand_link` | 0.0822 | −0.00003 0 −0.04123 | lower arm |

Total moving mass 2.48 kg. Inertia tensors are about each link COM in link axes. Products of inertia were converted from the SolidWorks sign convention (SolidWorks reports +∫xy dm; URDF uses −∫xy dm).

## Frames and zero pose

The CAD assembly origin is the pitch output face centre: +Y is up, +Z is outboard, and +X is the direction the elbow flexes. The URDF frame is taken so the existing, bench-sign-tested joint directions hold:

- **Pitch:** +q raises the arm forward (Wave, soft range −50°…+180°).
- **Elbow:** +q flexes it up/forward (`44ac3c9`).
- **Roll:** +q abducts.

With the URDF axes (`0 1 0`, `0 1 0`, `1 0 0`), that puts the arm's front at **−X**, outboard at **+Y**, and up at **+Z**. This is the usual humanoid frame turned 180° about Z, and the CAD reads as a right arm in it. (`kinematics.md` says +X forward for the humanoid; the bench base frame is the exception.)

The zero pose follows the CAD:

| Joint | q = 0 is | CAD stop near there |
|---|---|---|
| Roll | upper arm plumb | adduction stop 1.46° abducted |
| Upper-arm yaw | middle of the CAD limit mate (120…240°): elbow axis lateral, elbow flexing forward | — |
| Elbow | straight | extension stop 5.3° flexed |
| Lower-arm yaw | middle of its CAD limit mate (30…150°) | — |

If mechanical zero is set against the roll or elbow stop, the model at q = 0 is off by those angles. At rest that is worth at most about 0.07 Nm on roll and 0.03 Nm on pitch and elbow, still under the 0.20 Nm gate. The lower-arm yaw zero doesn't matter for gravity, because the lower arm's COM is on that axis.

## Masses

| Part | Mass (g) | Basis | COM at q = 0, base frame (m) |
|---|---|---|---|
| pitch bracket | 130.1 | PLA estimate, V 259.5 cm³, A 60 272 mm² | 0.0001 0.0408 0.0000 |
| roll RS03 | 880 | datasheet | −0.0044 0.0735 −0.0001 |
| roll bracket | 87.5 | PLA estimate, V 168.0 cm³, A 42 265 mm² | −0.0170 0.0735 −0.0331 |
| upper-arm yaw RS02 | 405 | datasheet | 0.0011 0.0735 −0.0797 |
| upper arm | 102.5 | PLA estimate, V 110.9 cm³, A 69 301 mm² | 0.0013 0.0692 −0.1552 |
| elbow RS02 | 405 | datasheet | 0.0011 0.0648 −0.2303 |
| elbow piece | 81.0 | PLA estimate, V 140.0 cm³, A 43 201 mm² | 0.0039 0.0898 −0.2408 |
| lower-arm yaw RS00 | 310 | datasheet | 0.0013 0.0735 −0.3039 |
| lower arm | 82.2 | PLA estimate, V 82.5 cm³, A 60 615 mm² | 0.0012 0.0735 −0.3713 |

- **Actuators.** RobStride datasheet masses (RS03 880 g, RS02 405 g, RS00 310 g), with the COM from the vendor CAD (on the actuator axis). RS02 and RS00 inertia is the CAD shape scaled to that mass. The RS03 STEP is mostly housing shells, so its inertia is a solid Ø99 × 56.6 mm cylinder.
- **Printed parts.** PLA, 1.24 g/cm³, 0.6 mm nozzle, 2 walls, 3 top and 3 bottom layers, 20 % infill. Each body's mass is estimated as ρ·(S + 0.2·(V − S)), where S = min(V, 1.1 mm × surface area). The estimate gives 81 g for the elbow piece; the 2026-07-22 bench weigh of the distal stack was ~82 g. Going from 15 % infill and 0.9 mm walls to 25 % and 1.3 mm changes the total by −3 %/+3 %, which moves the rest torques by under 0.003 Nm.
- **Not modelled.** Fasteners, heat-set inserts and cables, probably ~0.1 kg in all.
- **Re-weighing.** To use weighed masses, change a part's mass in the table and redo that link's mass-weighted COM. Subtract the link's joint point to get it in the link frame. Joint points in the base frame (m): roll (0, 0.0735, 0); upper-arm yaw (0.0012, 0.0735, 0); elbow (0.0012, 0.0735, −0.2304); lower-arm yaw (0.0013, 0.0735, −0.3300).

Three part files carry hidden full copies of actuators, and SolidWorks includes hidden bodies in mass properties:

- the pitch bracket has 23 bodies of the torso pitch RS03
- the roll bracket has one RS02
- the upper arm has one RS02

A straight export would have double-counted both RS02s and hung the torso pitch actuator on the moving arm, so these bodies are excluded. The SolidWorks MCP now reports them: `solidworks_body_mass_properties` gives visible and hidden totals, and `get_mass_properties` warns about hidden bodies.

## Predicted τ_g (Nm)

Computed from the new URDF the way `armee-dynamics` does it (virtual work, joint origins translate COMs). This computation reproduces −1.595 Nm at rest for the old placeholders.

| Pose (rad) | pitch | roll | upper yaw | elbow | lower yaw |
|---|---|---|---|---|---|
| rest, all 0 | +0.034 | −0.026 | 0.000 | −0.003 | 0.000 |
| pitch 0.48 | +1.338 | −0.023 | −0.012 | +0.157 | +0.015 |
| pitch 0.48, elbow 0.5 | +1.466 | −0.023 | −0.012 | +0.286 | +0.028 |
| roll 0.5 | +0.034 | +1.335 | +0.001 | −0.002 | 0.000 |
| pitch 1.57 | +2.832 | 0.000 | −0.026 | +0.346 | +0.033 |

The rest pose passes the 0.20 Nm gate with margin. Pitch τ_g is positive and rises with pitch.

## Open items

1. **Elevated-pose reference.** The handoff's reference was about +0.57 Nm at q_pitch ≈ 0.48 (2026-08-12 hold). This model predicts +1.34 Nm, and PLA assumptions move that by only ±0.05 Nm. The RS00 lower-arm yaw entered the config on 2026-08-11, and I couldn't confirm what was mounted for that hold. Without the forearm stack (elbow piece, RS00, lower arm), the model gives about 0.7 Nm. Hold torque also includes static friction. Check the current arm with a supported hold near 0.48 before trusting either number; if the residual is large, weigh the parts.
2. **`right_lower_arm_yaw` axis.** In CAD, the RS00 axis runs along the forearm (Z at q = 0); the URDF and `kinematics.md` say Y. I left it at Y, which changes nothing at q = 0. Changing it to Z needs a sign check and a look at the Consul viewer.
3. **Limits.** `kinematics.md` tables keep the design limits. The bench URDF carries the wider taught envelope (ADR 0017).

## Next (Pi session)

1. `pi_sync_bench_urdf`. This URDF already carries the Pi limits, so the sync doesn't clobber them.
2. `pi_gravity_preview` at rest, which should match the table above (|τ_g| < 0.20 Nm). Then check pitch 0.48: expect about +1.34 Nm, positive and rising.
3. Continue with handoff step 5: the supported hold at mechanical zero, `pi_candump_summary`, and `pi_logs_last_fault`.
