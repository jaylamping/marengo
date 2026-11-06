# Proportion check: body shell baseline vs an adult male scaled to 5 ft

2026-11-06. This checks the approved shell baseline: frame widened 38 mm per side, waist +10 mm per side, shoulder joints at Y ±208.5. The arm is the current skeleton (215 / 195 / 90 mm). Legs and head are not designed yet, so they are left out.

## Method

- **Body reference:** ANSUR II (2012 U.S. Army survey, men). 5th, 50th and 95th percentiles are taken from the NCSU summary table of Gordon et al. 2014. All values are scaled by 1524 / 1755 = 0.868, so the reference man is 5 ft tall like the robot.
- **Segment lengths:** ANSUR measures arm lengths between surface landmarks (acromion to elbow tip), which overstates joint-to-joint length. Arm lengths are therefore also checked against the Drillis & Contini joint-centre ratios (fraction of height H).
- **Waist and chest breadth:** these are not in that table. The values marked "approx." are typical ANSUR II male means from memory. Treat them as roughly ±5 %.
- **Robot dimensions:** taken from the concept generator (`generator/scene.js`, frame ±113, `fw=38`, `wx=10`).

## Results

| Dimension | Robot | Human, scaled to 5 ft (5th / 50th / 95th) | Robot vs median | Verdict |
| --- | --- | --- | --- | --- |
| Shoulder joint spacing (vs biacromial) | 417 | 334 / 360 / 388 (D&C 0.259H: 395) | +16 % | Above 95th: heroic |
| Outer shoulder width, across roll RS03s (vs bideltoid) | 516 | 399 / 442 / 492 | +17 % | 5 % above 95th: heroic |
| Shell chest width at the shoulders | 324 | ~288 chest breadth (approx.) | +12 % | Broad, fine |
| Shell chest width 111 mm below the shoulder | 278 | ~288 (approx.) | −3 % | Human |
| Chest depth (shell) | 212 | 183 / 220 / 259 | −4 % | Human |
| Chest bottom (shell) | 178 | — | — | See waist |
| Waist (dark bands) | 156 | ~276 waist breadth (approx.); hip 268 / 299 / — | −43 % | **Far narrower than human** |
| Shoulder joint height, frame as built | 1165 | 1250 (D&C 0.818H: 1247) | −7 % | Short torso |
| Shoulder joint height, +70 mm waist section | 1235 | 1250 | −1 % | Human |
| Shoulder to waist, frame as built | 310 | ~333 (D&C) | −7 % | Slightly short |
| Upper arm, joint to joint | 215 | 283 (D&C 0.186H); ANSUR shoulder–elbow 315 | −24 % | **Short** |
| Forearm, joint to joint | 195 | 223 (D&C 0.146H) | −12 % | Short |
| Hand | 90 (stub) | 165 (D&C) / 168 | — | Placeholder |

### Ratios (these drive the look more than the absolute sizes)

| Ratio | Robot | Median man | Athletic V-taper |
| --- | --- | --- | --- |
| Outer shoulder ÷ waist | 516 / 156 = 3.3 | 442 / 276 ≈ 1.6 | ~1.8–2.0 |
| Shell shoulder ÷ waist | 324 / 156 = 2.1 | — | — |
| Upper arm ÷ shoulder spacing | 215 / 417 = 0.52 | 283 / 360 ≈ 0.79 | — |

## What it says

1. **Shoulders are heroic, on purpose.**
   - The +38 mm frame puts the outer shoulder about 5 % beyond a 95th-percentile man at this height.
   - The +25 mm option would land exactly on the 95th percentile (491 mm).
   - The frame as built (441 mm) was almost exactly the median.
   - So +38 reads as superhero-broad. +25 reads as a very broad real man.
2. **The waist is the biggest departure.**
   - At 156 mm it is about half a human waist. Against these shoulders the taper is roughly twice as strong as a real athletic build.
   - That is a robot style choice (an exposed dark core, as in the film reference), not a human proportion.
   - An athletic human V-taper (ratio 1.8–2.0) at this shoulder width would put the waist at about 260–290 mm. A middle ground that still reads robotic would be about 200–220 mm.
3. **The arms are short for these shoulders.**
   - Joint-to-joint, the upper arm is 24 % under human and the forearm 12 % under.
   - Widening the shoulders makes this more visible: the arm-to-shoulder ratio is 0.52 against about 0.79 in a person.
   - A human-proportioned 5 ft arm would be about 280 / 220 mm.
   - This conflicts with Joey's sense that the old 230 mm upper arm already looked too long. That impression came from a much narrower torso, so it's worth re-judging against this baseline. The arm thread owns this.
4. **Torso height:** the +70 mm waist section brings the shoulder height to within 1 % of human. Without it, the shoulders sit 7 % low and the head will look tall.
5. **Chest depth and mid-chest width are human.** No change is needed.

## Suggested next step

Pick a style target, then adjust the rest to match:

- **Robot-stylised (current):** keep the heroic shoulders and slim core. Accept the short arms, or lengthen them to about 240–250 mm.
- **Human-proportioned:** come back to about +25 mm shoulders, widen the waist to about 250 mm, use arms of about 280 / 220 mm, and add the +70 mm waist section.

## Sources

- Gordon, C. C. et al. (2014). *2012 Anthropometric Survey of U.S. Army Personnel: Methods and Summary Statistics*. https://apps.dtic.mil/sti/pdfs/ADA611869.pdf
- NCSU Ergonomics Center, *Anthropometric Summary Data Tables*, built from the survey above. https://www.ergocenter.ncsu.edu/wp-content/uploads/sites/18/2017/09/Anthropometric-Summary-Data-Tables.pdf
- Drillis, R. and Contini, R. (1966), body segment lengths as fractions of height, as tabulated in Winter, *Biomechanics and Motor Control of Human Movement*.
