# Left arm v1 layout

2026-11-05. Layout skeleton, not a part. Not released for printing, load, or power.

Views of the left arm master layout beside the torso. It fixes the five joint axes and actuator positions
on the existing shoulder (P1 L01, P2 L04B) so the upper arm can be designed against them. The CAD stays on
Joey-PC.

| Item | Value |
| --- | --- |
| Joints | pitch RS03, roll RS03, upper-arm yaw RS02, elbow RS02, lower-arm yaw RS00 |
| Links | 215 mm shoulder to elbow, 195 mm elbow to wrist, 90 mm hand stub |
| RS00 position | rear face 88.2 mm below the elbow axis (`RS00_OFFSET` 40 mm) |
| Mass | 2.37 kg, actuators and printed shoulder only |
| Shoulder pitch torque, arm horizontal | 2.26 N·m, actuators and printed shoulder only |

The torque excludes links, hand, fasteners, cables, and payload.

**Elbow fold:** the RS00 was moved 40 mm toward the wrist so a full-depth upper-arm shell clears it at 135° of
flexion. Its rear inner corner sits 42.2 mm forward of the yaw axis there, 3 mm past the 39.25 mm shell half-depth.
This is a hand calculation, not a motion sweep. It leaves 55.4 mm from the RS00 output to the wrist.

Images: `LEFT_ARM_LAYOUT_FRONT.png`, `LEFT_ARM_LAYOUT_ISO.png`. The right arm shown is the old torso export,
for comparison only.
