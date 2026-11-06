# Waist yaw joint: is one RS03 enough?

2026-11-06. Hand calculation, not FEA and not a released design. Numbers come from `waist_loads.py` (point masses; run it to
reproduce). Torso coordinates as in `../torso-frame/README.md`: X left, Y up, Z forward, yaw axis on Y through X = Z = 0.

## Answer

**Yes for torque and speed, no as a bearing.** The RS03 has enough torque and speed to turn the torso, but it should not carry
the upper body's weight, its tipping moments, or fall loads through its own output bearing. Recommendation:

1. **Keep the RS03, add a support bearing** (crossed-roller ring around the actuator) so the RS03 carries yaw torque only.
2. **Add a torque fuse** between the RS03 output and the pelvis (a resettable friction slip clutch, or a printed shear key as the
   first cheap version), set at about 55 N·m, below the RS03's 60 N·m peak. A fall can twist the waist at 200 to 400 N·m,
   and no actuator option here survives that through its gearbox.
3. **Software caps:** yaw torque limit about 40 N·m and acceleration about 20 rad/s² with arms out and loaded.

Not an RS04: it only doubles torque that isn't needed, has the same unpublished bearing, adds 540 g, and takes one of the two
leg actuators. Not a reduction: torque margin isn't the problem, and a reduction adds backlash and height, and makes a fall
twist worse because the reflected rotor inertia grows with the ratio squared.

## Ratings used (vendor manual `hardware/docs/robstride/RS03_User_Manual_261013.pdf`)

| Item | RS03 | RS04 |
| --- | --- | --- |
| Peak torque | 60 N·m | 120 N·m |
| Rated torque | 20 N·m at 100 rpm, on a 215 × 220 mm aluminium heat sink | 40 N·m (345 × 345 sink) |
| Stall rating | about 13 N·m (stall heats one phase 1.41× harder) | |
| No-load speed | 200 rpm (20.9 rad/s) at 48 V | 200 rpm |
| Mass | 880 g | 1420 g |
| Output bearing axial / radial / moment | **not published** | **not published** |

The manual voids the warranty for damage from exceeding peak torque.

## Masses on the waist (estimates)

| Group | kg | Source |
| --- | --- | --- |
| Frame, brackets, gussets | 2.10 | torso-frame README, parts 1–11 |
| 2 shoulder pitch RS03 + brackets | 2.13 | torso-frame README |
| 13S2P pack | 2.20 | power/custom-21700-pack.md |
| Power board, fans, harness | 0.53 | torso-frame README |
| Compute (Pi + HAT, Jetson, third board, shelf) | 0.49 | torso-frame README |
| Shell, hardpoints, TPU pads | 1.71 | body-shell manufacturing README |
| Waist adapter, head allowance | 1.12 | head is a 1 kg placeholder |
| Two arms (roll RS03, 2 RS02, RS00, links, 0.4 kg hand allowance, cables) | 2 × 3.33 | left-arm layout, link READMEs; arm CoM 156 mm from the joint line |
| **Upper body on the waist** | **16.9** (18.9 with 1 kg in each hand) | |

## Case 1: yaw with arms out and a payload

Yaw is a gravity-free axis, so holding a pose needs almost no torque. Torque is inertia × angular acceleration.

| Pose (1 kg in each hand) | Yaw inertia, kg·m² | 20 rad/s² | 40 rad/s² |
| --- | --- | --- | --- |
| Arms down | 0.45 | 9 N·m | 18 N·m |
| Arms straight forward | 1.20 | 24 N·m | 48 N·m |
| T-pose (worst) | 2.02 | 40 N·m | 81 N·m |

20 rad/s² reaches 6 rad/s (340°/s, a fast human torso twist) in 0.3 s. That is inside the RS03 peak with margin except a
T-pose at 40 rad/s², which the acceleration cap prevents. Speed: 6 rad/s needed against about 21 rad/s no-load (about
17 rad/s at a nearly flat 39 V pack), so speed is fine. Continuous load is low, well under 13 N·m, as long as the RS03 has
a metal path to sink heat (bolt its housing to the aluminium plate, not only to PETG).

Loads the output bearing would see if it carried the torso alone: 186 N axial static (about 370 N walking), 19 N·m tipping
moment with both arms forward holding 1 kg, plus about 11 N·m from walking sway. Plausibly survivable for a QDD output
bearing, but RobStride doesn't publish the limit, and the fall case below is far beyond it.

## Case 2: arm swing reaction

| Motion | Yaw torque into the waist |
| --- | --- |
| Arm raised forward, swung sideways by the roll RS03 (roll axis is vertical in that pose), 40 rad/s², one arm | 15 N·m |
| Same, both arms the same way | 30 N·m |
| Arms counter-swinging in pitch (gait), 40 rad/s² | 16 N·m |
| Ceiling: both roll RS03s driven at their 60 N·m peak | 120 N·m |

Normal swings are inside the RS03 rating. The 120 N·m ceiling only happens if both roll RS03s hit their peak in the same
direction; the arm torque caps prevent that, and the yaw fuse protects the waist RS03 if it ever happens.

## Case 3: ground-level tip-over (design load case)

Robot about 27 kg (assumes 10 kg of pelvis and legs), CoM 0.85 m, 225 J of potential energy, shoulder hits the ground at
about 5.6 m/s.

| Load | Estimate |
| --- | --- |
| Bending at the waist as the torso lands first and stops the legs through it (15 to 30 ms) | 350 to 700 N·m, 0.7 to 1.4 kN shear |
| Twist from a shoulder-first landing (friction 0.5 at 0.2 m) | 200 to 400 N·m |

Both are 3× to 12× past the RS03's 60 N·m torque rating, and the bending would go straight through its unrated output
bearing. An RS04 at 120 N·m doesn't change that. Only a support bearing (for bending and shear) and a torque fuse (for
twist) do. This matches the arm rule: a cheap printed link breaks before a $500 actuator does.

## Proposed layout (for the frame thread to fit)

- Crossed-roller ring of about 100 bore, 130 OD, 16 to 20 high (RB10016 class; **check the vendor sheet for the static moment
  rating before gate 2**). It goes around the Ø99 RS03 body, so the actuator nests inside the bearing rather than stacking
  under it. OD 130 fits the 140-deep frame bottom.
- Inner ring to the torso (aluminium plate on the bottom rails, which also heat-sinks the RS03 housing). Outer ring to the
  pelvis. The RS03 output drives the pelvis through the fuse and a coupling that doesn't pass bending (pins with float or an
  Oldham-style disc), so the bearing, not the RS03, carries every non-torque load.
- Yaw hard stops on the bearing rings, not on the actuator, at the chosen range (±90° suggested).
- The RS03 is still 57 mm tall against the 40 mm waist bay (frame README open item). Nesting doesn't fix the height on its own;
  the bay or the +70 waist section has to take it.

## Unverified

Masses of the head, hands, legs and pelvis are placeholders. The fall model is a rigid rod, which is crude; the bending and
twist numbers are order-of-magnitude. The RS03 output bearing rating is unknown (also asked of RobStride for the arm). The
crossed-roller rating is from a size class, not a chosen part.

## Frame-side constraints (frame thread, 2026-11-06 06:28Z)

- Frame bottom is the 150 × 140 rectangle of 2020 at Y 155 to 175. Everything below Y 155 belongs to the hip stage.
- Mount the bearing plate with M5 T-nuts in the free under-face slots of the four bottom rails. The plate may use the full
  150 × 140 footprint and grow outboard, provided it stays below Y 155 outside |X| 75 / |Z| 70 (shell abdomen band on H6 at Y 165).
- The battery pack bottom is at Y 183, about 8 mm above a plate at Y 149 to 155. If the plate heat-sinks the RS03, insulate or
  shield the pack side (cells rated 60 °C).
- OD 130 fits in plan with no clash in rev A.
- Height: ring (16 to 20) + RS03 (57) needs about 73 to 77 mm against the 40 mm waist bay (pelvis draft top at Y 115). Either the
  pelvis top drops or the frame rises (the +70 waist section). Joey's call, at the hip stage.
- Still owed: photo of the built frame bottom showing how the current waist adapter bolts on.
