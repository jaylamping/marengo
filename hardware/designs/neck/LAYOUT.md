# Neck layout study, option C (rev 0, 2026-11-07)

**DECIDED (Joey 2026-11-07 06:50Z): neck +10 mm** ("+10 looks the best to me"). The visible neck goes from 36 to ~46 mm, the U-joint sits at z 80, and the head can look down ~34° (design range: pitch 34° down / 25° up, roll ±20°, yaw ±90°).

Joey picked option C (three bus servos) at 06:30Z. This study checks that it packages before any SolidWorks
work. Script: [layout_study.py](layout_study.py) (output in [layout_study_output.txt](layout_study_output.txt)).
Picture: [LAYOUT_STUDY.png](LAYOUT_STUDY.png) (to scale, schematic).

## Layout

- **Yaw.** A hollow yaw tube turns in a 61814 bearing (70 × 90 × 10) at z 4 to 14, inside the collar.
  The bearing sits on a printed tower (OD ~120) standing on the two top rails. A fixed yaw servo at Y +56
  drives a ring gear (r 44) on the tube through a pinion (r 12), about 3.7:1, via a window in the tower.
- **Pitch and roll.** Two servos lie flat, stacked, under the tube and turn with it (sweep r 39, limit 48
  inside the 100 mm box depth). Each horn (r 19) drives a pushrod up the tube to a ball stud on the head
  plate at X -18, Y ±15. The head plate sits on a U-joint at z 70. Rods together give pitch; rods
  opposite give roll.
- **Neck look.** The ringed neck is a TPU bellows boot from the collar to the head shell. Pushrods,
  U-joint post and cables all fit inside it (rod envelope r 28.4, boot bore ~r 33.5).

## Checks (all assumption-labelled: collar bottom 10 mm above the rail face; servo shaft offset assumed)

| Check | Result |
|---|---|
| Horn angle at the worst ROM corner (pitch -40/+25, roll ±20) | 57° from level, limit 65°: OK |
| Rods inside the bearing bore and the bellows | r 28.4 vs 35 / 33.5: OK |
| Rotating stack inside the frame box | r 39.3 vs 48: OK |
| Stack bottom | Y 455, exactly the Jetson placeholder top (placeholder only, Joey packs the bay) |
| Servo torque at 1.3 kg, 40° down, 20 rad/s² | 0.56 N·m per servo vs 0.98 rated: OK |
| Yaw torque at the servo | ~0.02 N·m: trivial |
| **Chin vs collar on look-down** | **chin hits the collar past ~24° down** with the neck as modelled |

## The one real problem: look-down range

With the Argon head as modelled (36 mm of visible neck, U-joint at z 70), the chin (X 83, Z 50) swings into
the collar past about 24° of look-down. At 40° it reaches z 1.

| Neck taller by | Look-down limit |
|---|---|
| 0 mm (as modelled) | 24° |
| +10 mm | 34° |
| +20 mm | 45° |
| +30 mm | 60° |

Is 24° enough? With 24° of head pitch and a typical stereo camera's ~35° half vertical field of view, the camera
sees down to ~59° below horizontal. Hands at waist height, about 0.4 m in front, are around 50° below eye
level, so they would sit near the bottom edge of the image. The robot could look at its hands, but only just,
and without the visible "looking down" gesture.

## Update: collar height (2026-11-07, from the shell generator scene)

The head thread places the head with `hz=-6`, which puts the collar bottom at F1 + 34, i.e. **34 mm** above the top-rail
face (Y 529), not the 10 mm assumed above. The shell dome closes at F1 + 48. This helps: the whole stack can sit 24 mm
higher, so its bottom lands near Y 479, out of the compute bay. The chin-vs-collar look-down limits don't change
(they are relative to the collar). Renders: [renders/NECK_HEIGHT_OPTIONS.png](renders/NECK_HEIGHT_OPTIONS.png).

## SolidWorks layout

Done: [LAYOUT_SW.md](LAYOUT_SW.md).

## Next

1. Joey: neck height (decision card in the thread).
2. Confirm the collar height over the rail face with the body-shell side (assumed 10 mm).
3. Feetech datasheet for the shaft position and horn spline, then SolidWorks layout on Joey-PC (`cad/neck/`).
