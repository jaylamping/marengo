# Neck renders

`NECK_HEIGHT_OPTIONS.png`: the Argon head (`hardware/designs/head/concepts/bakeoff/argon/build_head.py`,
run as-is) on the rev B body shell in the shell generator scene, for the three neck heights. Each height is shown
neutral, at its look-down limit, and in a curious pose. `NECK_HEIGHT_CLASH_40.png` shows the head as modelled
at 40° down.

How to reproduce: run `build_head.py`, then serve `hardware/designs/body-shell/concept/generator/` with three@0.170.0.
Link the head's `out/` as `giantout/A`, and write a `giantout/AV/parts.json` with each file prefixed `A/`.
Then run `node shoot.js "$(cat shots.json)"`. The scene takes these neck-study query params, all defaulting to 0:
- `lift`: how much taller the neck is, in mm. The bellows placeholder stretches by that much and the head moves up.
- `pitch`: positive looks down, in degrees.
- `roll`, `yaw`: in degrees.

All rotations are about the U-joint at 70 + lift mm above the collar bottom. `hz=-6` puts the collar bottom at
F1 + 34 (the head thread's placement).
