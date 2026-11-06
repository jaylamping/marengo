# Stepped torso frame

2026-11-06. Two-box 2020 frame: a wide upper box at the shoulders on the existing lower cage. Locked with the shell baseline. FEA and the cut list follow.

Coordinates: torso STEP frame, mm. X lateral, Y up, Z forward. Y is measured from the pelvis bottom.

## Step height

| Item | Y | From shoulder axis |
| --- | --- | --- |
| Shoulder axis | 424.86 | 0 |
| Top of upper box | 495 | +70.14 |
| **Bottom of upper box, shared rail underside** | **353.62** | **-71.24** |
| Shared rail top | 373.62 | -51.24 |
| Gusset plate bottom | 285 | -139.86 |
| Bottom of lower frame | 155 | -269.86 |

| Width | Half-width X |
| --- | --- |
| Upper box outer | 113 |
| Lower box outer | 75 |
| Step | 38 |
| Pitch RS03 output face | 135 |
| Arm joint line | 208.49 |

Depth is ±70 over 2020, the same in both tiers.

## Gusset plate

`gusset_plate_left.step` and `gusset_plate_left.dxf`. Four plates, two per step corner, on the front and rear faces. The files are the left side at the front. Mirror X for the right.

| Item | Value |
| --- | --- |
| Outline points, X Y | 55 285, 75 285, 113 335, 113 425, 93 425, 93 373.62, 55 373.62 |
| Corner radius | 8 |
| Holes | 9 × Ø5.5 for M5 T-nuts |
| Lower post holes | X 65, Y 297, 317, 337 |
| Shared rail holes | Y 363.62, X 62, 84, 104 |
| Upper post holes | X 103, Y 385, 400, 415 |
| Sits on | Z ±70 frame faces |
| Outer face, 10 mm plate | Z ±80 |

The outline and thickness may change after the fall-case study.
