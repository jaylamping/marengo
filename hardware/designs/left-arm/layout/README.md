# Left arm v1 layout

2026-11-05. Layout skeleton, not a part. Not released for printing, load, or power.

Views of the left arm master layout beside the torso. It fixes the five joint axes and actuator positions
on the existing shoulder (P1 L01, P2 L04B) so the upper arm can be designed against them. The CAD stays on
Joey-PC.

| Item | Value |
| --- | --- |
| Joints | pitch RS03, roll RS03, upper-arm yaw RS02, elbow RS02, lower-arm yaw RS00 |
| Links | 215 mm shoulder to elbow, 195 mm elbow to wrist, 90 mm hand stub |
| RS00 position | rear face 88.2 mm below the elbow axis (`RS00_OFFSET` 44 mm) |
| Mass | 2.37 kg, actuators and printed shoulder only |
| Shoulder pitch torque, arm horizontal | 2.27 N·m, actuators and printed shoulder only |

The torque excludes links, hand, fasteners, cables, and payload.

The arm is on robot left (+X in the torso STEP) and the elbow flexes forward (+Z). Forward comes from the
right arm sitting on -X, the right-arm CAD flex direction, and the Pi ports facing -Z.

**Elbow fold:** target 135°. With a 3 mm shell wall over the 78.5 mm actuators, the rear corner of the RS00 leaves
about 2.8 mm of air at 135° and touches at about 137°. This is a hand calculation, not a motion sweep. It leaves
51.4 mm from the RS00 output to the end of the forearm. There is no wrist pitch; the arm stays at five actuators.

The elbow and RS00 are mated to the skeleton, so `L_UPPER` and `RS00_OFFSET` move them.

Open items:

- Confirm forward on the physical robot. The fixed pitch bracket names disagree with the arm names.
- P1 hole pattern: the 7.83 mm residual looks like a 30° clocking miss, but that is unmeasured.
- Elbow output is one-sided, 22.7 mm outboard of the arm line, and its load is unchecked.
- No upper-arm or forearm structure, motion sweep, or cable routing yet.

Images: `LEFT_ARM_LAYOUT_FRONT.png`, `LEFT_ARM_LAYOUT_ISO.png`. The right arm shown is the old torso export,
for comparison only.
