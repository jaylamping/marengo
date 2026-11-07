# Neck: approach options and recommendation (rev 0, 2026-11-07)

Thread "Design the neck mechanism". This is the approach decision only. No CAD yet.

Picture: [NECK_OPTIONS.png](NECK_OPTIONS.png) (front view, to scale, Argon head baseline).
Loads: [neck_loads.py](neck_loads.py) (hand calc, every input is an assumption until the head is weighed).

## What the neck has to do

- Carry the Argon head (`hardware/designs/head/concepts/bakeoff/argon/build_head.py`): jaw bottom at z 50,
  dark ringed neck tube Ø72 (ribs Ø82) from the collar up into the jaw, collar Ø150 at z 0 to 14.
  The visible neck is only about 36 mm tall, so nothing big can live inside it.
- Point the head: yaw (look left/right) and pitch (look down at the hands, look up at people).
  Roll (the curious head tilt) is optional. See decision 1.
- Pass cables: USB3 for the stereo camera, power for the head, and the servo bus if servos sit up top.
- Survive the design load case (ground-level tip-over). The head is the first thing to hit the floor
  from the front. Cheap printed parts must give before any actuator.

## Load estimate (assumption-labelled)

Head mass is unknown. Estimate: printed shell ~280 g, jaw ~100 g, inner frame ~80 g, stereo camera
60 to 100 g, two round displays ~25 g, jaw servo ~55 g, speaker + mics + head MCU ~55 g, cables and
screws ~80 g, so about 0.75 to 0.8 kg. Designed for **1.0 kg nominal, 1.3 kg upper bound**.

From `neck_loads.py` (COM assumed at z 140, 10 mm forward; 20 rad/s² acceleration):

| Pitch pivot | 1.0 kg, 40° down | 1.3 kg, 40° down |
|---|---|---|
| High: U-joint just above the jaw floor (z 70) | 0.70 N·m | 0.91 N·m |
| Low: at the collar (z 10) | 1.32 N·m | 1.72 N·m |

Yaw needs about **0.06 to 0.08 N·m** (no gravity term).

For scale: an RS00 is 5 N·m rated / 14 N·m peak, 310 g, 57 × 57 × 51.4 mm (vendor manual,
`hardware/docs/robstride/RS00_User_Manual_260713.pdf` §1.3 to 1.4). A Feetech STS3215 is
about 1.0 N·m rated / 2.9 N·m stall at 12 V, 55 g, 45.2 × 24.7 × 35 mm, steel gears, 12-bit magnetic
encoder (RobotShop vendor listing; backlash not published).

So an RS00 is about 70 times what yaw needs, and the high pivot keeps pitch inside one small servo's
rated torque even at the upper-bound mass. **Put the pitch pivot high**, inside the bottom of the head:
half the torque, the visible neck column stays upright like the Giant's, and the head moves on top of it.

## Inventory check

Joey's RS00s are allocated, one per arm (lower-arm yaw). There are no spares. Any RS00 in the neck is
a new purchase.

## Decision 1: how many axes

- **2 axes (yaw + pitch).** Covers looking around and looking at the hands. One fewer servo.
- **3 axes (yaw + pitch + roll).** Adds the head tilt, which is the main gesture for "inquisitive".
  With a two-servo parallel linkage (below), roll costs one more ~$50 servo and nothing else:
  the same two servos do pitch when they move together and roll when they move opposite,
  and they share the pitch load.

Recommendation: **3 axes.** Joey's minimalism rule asks whether leaving it out hurts. For a head whose
brief is "Inquisitive, Brave, Endearing", losing the tilt does. Retrofitting roll later means
redesigning the neck top, so decide now.

## Decision 2: which actuators

| | A. All RobStride | B. Hybrid | C. All bus servos (recommended) |
|---|---|---|---|
| Yaw | RS00 under the collar | RS00 under the collar | STS3215-class, fixed in the torso top, pinion to a ring gear on a turntable |
| Pitch/roll | 2 × RS00 | 2 × STS3215-class | 2 × STS3215-class riding on the yaw turntable, pushrods up the hollow neck to a U-joint |
| Mass added | ~930 g, ~620 g of it in the head | ~420 g | ~165 g, almost none in the head |
| New buys | 3 × RS00 | 1 × RS00 + 2 servos | 3 servos + USB bus adapter |
| Buses | CAN only | CAN + servo TTL bus | servo TTL bus only |
| Power | 48 V motor bus (E-stop cuts it) | both | 12 V (needs an E-stop cut, see below) |
| Fit | RS00 yaw hangs to Y ~444, eating ~11 mm of the compute bay (the Jetson box is only a placeholder; dropping it to Y 399-444 leaves ~2 mm over the Pi). Two RS00 side by side are 103 mm wide in a 122 mm cranium, in the jaw's swing | same ~11 mm of compute bay | fits between the top rails; about 2 mm to the Jetson top (needs the frame thread to confirm) |
| Feel | smooth, quiet, back-drivable | smooth yaw, geared pitch | geared, some whine, some backlash |
| In a fall | expensive motors take the hit | RS00 safe, servos sacrificial | all sacrificial: printed gear teeth and U-joint shear pins give first, servos are cheap |

Why C: the loads are tiny; there are no spare RS00s; an RS00 yaw costs ~11 mm of compute-bay height; and C keeps one
bus, one supply and the lightest head. It also follows "blow cheap parts, not $500 actuators":
every part in the neck is cheap and printed parts are the fuse.

What C gives up: RobStride smoothness and back-drivability. If the servo yaw looks or sounds cheap on
the bench, yaw can be upgraded to B later, because the turntable and its bearing are separate from
the drive. That only works if the frame thread leaves room under the collar.

## How C would work (concept, for the layout step)

- **Yaw stage.** A printed turntable at the collar on a thin-section bearing (61812 class, 60 × 78 × 10)
  carries all head loads. The yaw servo never carries bending. A fixed yaw servo drives a ring gear on
  the turntable through a printed pinion, about 3:1. That ratio cuts backlash at the head and makes the
  printed teeth the torque fuse. Range ±90°.
- **Pitch/roll.** Two servos hang under the turntable and turn with it. Pushrods run up the hollow
  neck to two ball links on the head plate, either side of a U-joint at about z 70.
  Range targets: pitch 40° down / 25° up, roll ±20°. The chin must clear the chest at 40° down (head thread).
- **Head side.** The head plate on the U-joint carries the cranium. The jaw hangs from the cranium at
  the ear bosses and swings around the neck.
- **Cables.** No slip ring. A service loop coiled in the collar handles ±90° yaw: one thin flexible
  USB3 cable (camera, or a small hub in the head), 12 V + GND for the head electronics, and the servo bus
  if needed. A small MCU in the head (round displays, jaw servo, mics, speaker amp) on USB2 keeps the
  conductor count low. A ribbon clock-spring like the elbow's is the fallback if the loop fights the servo.

## Things this touches in other threads

1. **Head ("Design the robot's head").** The jaw floor has a neck hole r 42 around a r 36 neck. If the jaw
   opens about the ear bosses, the floor around the neck moves roughly 25 to 30 mm at 15° of jaw opening.
   The hole needs to be a slot, or the jaw floor opens at the back, or the dark ringed neck becomes a TPU
   bellows boot that also hides the pitch/roll gap. My pick is the bellows, because the rings are already
   there. Also needed: chin clearance at 40° down, and space for a U-joint at about z 70.
2. **Frame ("Finalize the internal torso frame").** Under the collar, between the top rails, the neck needs
   about 75 × 100 × 50 mm, plus the yaw servo beside it. That is about 2 mm above the Jetson's top
   placeholder (Y 455) if the collar sits 10 mm above the frame top. Frame thread (04:46Z): the frame ends
   at Y 495 and doesn't set the collar height (head/shell do); the Jetson box is a placeholder since Joey packs
   the bay himself. Fixed limits: top rails Y 475-495 at Z ±50..70, usable bay |X| < 78 between the shoulder
   motors, top-rail T-slots free except the H1/H2 nuts (front X ±60, rear X ±85). The neck also needs a mount: a plate on the top rails with T-nuts.
3. **Power ("Plan the custom power and peripheral board").** The servos run on 12 V from the 150 W brick.
   Expect about 1 A typical and 8 A worst case (three stalls at 2.7 A each). **The brick is ahead of the
   E-stop, so the E-stop would not stop the neck.** The cheapest fix is a second NC contact block on Joey's
   E-stop switching the neck's 12 V feed. This depends on the E-stop model, which is still pending.
4. **Calibration loop.** Servo backlash, unpublished but likely around 1°, is about 9 mm at 0.5 m.
   The head camera alone can't be the reference for arm calibration. Keep the bench webcam as the
   reference, or have the head camera see a fixed tag on the torso each time.

## Next steps once Joey picks

1. Agree the neck envelope with the frame and head threads.
2. Confirm servo specs from Feetech's own datasheet. The STS3250 (~50 kg·cm) is the same size if
   more margin is wanted.
3. Lay it out in SolidWorks on Joey-PC (new `cad/neck/` region folder, added per the folder rule).
4. Gate-2 review before any print. Print a dummy-head mass test rig before the real head exists.
