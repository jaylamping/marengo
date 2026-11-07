# Elbow dummy output test plan

2026-11-06. Draft. Joey chose to keep the compact, centred elbow. The upper arm prints now, and this rig runs before the forearm goes on. The robot stays stationary and supported until the legs exist, so this is the stage to break things: **the one hard rule is that a printed part gives before an RS02 or RS03 is damaged.** Until test 7 has run, no sideways push over about 40 N at the hand on the real arm: the FEA puts local yield at the hoop seat near 70 N. No real RS02 is loaded in any test.

Print files, times and the print order are in `../prints/test-pass-1/`. Pass levels marked **proposed** need Joey's agreement before the first test.

## What this answers

1. Which part fails first at the stop, and at what torque?
2. What torque does the printed stop actually fail at, and does it fail before anything else?
3. Does the stop survive a limp arm dropping onto it?
4. Do the screw joints hold until the printed parts fail?
5. What is the real strength across layers, on Joey's process?
6. Does the bearing seat, stub and retainer arrangement fit, run free and assemble?

**What it cannot answer:** whether the RS02 output bearing survives. A dummy has no RS02 bearing, and we have no rating. The tests give the side moment and force at which the printed parts give way. Those stay as numbers to compare once a rating or a measured limit exists, so "arm breaks first" stays unproven until then.

The dummy's thread is not the RS02's either, since the RS02 output alloy is unknown. The thread result shows the printed joint's limit, not the RS02's.

## Validation pass conditions

Agreed 2026-11-06. The first validation pass has no steel or metal stop, and the printed stop is a wear part that should break before anything else.

- **Design load is 17 N·m**, the RS02's own peak. Target at that load is SF 1.5 and layer SF 1.3.
- **34 N·m is not a design case for this pass.** It stays in this plan as a measured point only.
- **The shoulder-through-stop case is covered by operation:** a software torque cap, a position soft limit 2 to 3 degrees inside each hard stop, and supervised bench use only.
- **The stop is tested to failure** so the real number replaces the FEA estimate.

## Load cases

Elbow to hand is 285 mm (195 + 90). All loads below are at the elbow axis.

| Case | Elbow torque | Source |
| --- | --- | --- |
| Actuator peak | 17 N·m | RS02 peak, also the FEA case |
| Shoulder through stop, information only this pass | 34 N·m | 60 N·m shoulder over 500 mm gives 120 N at the hand, over 285 mm |
| Limp drop | 2 J | Forearm 0.48 kg with its centre about 95 mm below the elbow, plus a hand assumed at 0.5 kg at 285 mm, falling through 90 degrees. About 1.9 J |

A full ground-level tip-over is not defined yet. Breaking energy from test 5 gives the number to compare when it is.

Load from a hanging weight on a lever:

| Elbow torque | At 300 mm | At 500 mm |
| --- | --- | --- |
| 17 N·m | 5.8 kg | 3.5 kg |
| 34 N·m | 11.6 kg | 6.9 kg |
| 50 N·m | 17.0 kg | 10.2 kg |

## Equipment

Everything here is meant to be possible at home.

- Bench or heavy table with clamps, and a floor free of feet and pets under the load.
- Lever: 25 mm square steel or aluminium tube, or a 2020 extrusion, 600 mm, with holes at 300 and 500 mm **measured from the elbow axis, not from the end of the lever**. The socket floor is 100 mm from the axis, and the M5 holes are 10 and 30 mm from the inserted end of the lever. Include the lever's own weight in the torque.
- Weights: dumbbell plates or water jugs in a bag, weighed first. A luggage scale is for pull tests and for checking, not for setting a static load, since it is hard to hold steady.
- Dial indicator or calipers, phone camera, marking compound or a dry-erase marker for contact patches.
- Angle reading: phone inclinometer on the lever.
- Face shield, or at least safety glasses, for every load test. PETG can fail suddenly.
- A catch for every load test: a block or strap 20 to 30 mm under the weight bag, so a failure drops the load a short way instead of swinging it through the full arc. Nobody stands in the swing plane. Lower weights on with a jack or by hand from the side.
- For the drop test, count the lever (about 0.3 kg) and the sector in the energy, not only the 2 kg.

## Test parts

| Part | Material | Notes |
| --- | --- | --- |
| Layer coupons | PETG, P2S process | Bend bars, 80 × 12 × 6 mm |
| Fit coupon | PETG | Ø52 seat and Ø40 stub with the floor bore, one real 6808 |
| Carrier | PETG | Real part after the CAD fixes |
| Fork sector | PETG | Fork cut across the tube near Y 118, keeping both cheeks, gussets, stub and finger, with a solid crown and a socket for the lever. Printed with the outboard cheek down |
| Dummy housing | PETG, printed, over-built | RS02 stator stand-in: 6 M3 at r 36.5 on the real Ø73 circle, using the same hole clock as the hoop, the carrier clamped straight on the printed face as on the arm, into captive nuts |
| Dummy output | PETG, printed | 6 M4 on Ø24 into captive nuts that slide in from the rim, and three Ø4 steel dowels. An M8 hex head sits in a pocket behind the nuts, so torque can enter here. Stack in `../prints/test-pass-1/PRINT_ORDER.md` |
| Real parts | | One 6808, the planned M3 and M4 screws and washers. No steel rings or sleeves |

Use the real screw types and lengths from the fixed design. Torque values come from CAD work, and are recorded for each joint.

## Print process

Same for every printed part, and the process used for P1 and P2: P2S, 0.4 mm High Flow nozzle, Generic PETG, Cool Plate Super Tack, 0.20 mm layers, 6 walls, 100% rectilinear infill, normal supports, auto 45 degree threshold, Z gap 0.20, interface 2/2, 255 °C nozzle, 70 °C bed.

Dry the filament the same way for all parts, keep the spool the same if possible, and record the spool for each print.

## Print and test order

Each step can stop the plan, so the later prints are not spent until it passes.

| Step | Print | Test | Needs SolidWorks |
| --- | --- | --- | --- |
| 1 | Layer coupons | Test 1 | Simple parts only |
| 2 | Fit coupon | Test 2 | Yes, after the floor bore fix |
| 3 | Carrier | Test 3 | Yes, after the gate fixes |
| 4 | Fork sector, dummy parts | Tests 4 to 7 | Yes |
| 5 | Full upper arm and forearm | Only after tests 1 to 7 pass, with lengths and wiring openings frozen | Yes |

Print two carriers and two fork sectors if the first run looks good, since tests 5 and 6 break parts. Do the non-destructive tests first on each set.

## Tests

### Test 1: layer coupons

Five bars printed standing up (layers across the bar) and five lying flat, in the same print session.

Three-point bend, 60 mm span, weight hung from the centre through a Ø8 rod. Bend stress is 3FL / 2bh².

| Bar | Force at the assumed value |
| --- | --- |
| Standing, 22.5 MPa | 108 N, about 11 kg |
| Lying flat, 45 MPa | 216 N, about 22 kg |

Add load in 1 kg steps to failure, 30 seconds per step. Record each failure load and where it broke.

**Proposed pass:** the mean standing result reaches 22.5 MPa, and the lowest bar reaches 20 MPa. Bend results usually read higher than a pure tension test, so treat a marginal pass as a fail. If the mean is below 22.5 MPa, scale every layer SF in the record by the ratio.

### Test 2: fit coupon

Fit the 6808 by hand. Measure the seat and stub with calipers in three directions.

**Pass:** the bearing seats without force beyond firm hand pressure, the stub turns freely with the outer race held, the inner race does not touch the floor, and axial float can be felt. Record the measured offsets and put them in the CAD before printing the carrier.

### Test 3: carrier alone

Inspect the seat, bridge over the recess, pocket slivers and floor. Fit the 6808, retainers and washers.

**Pass:** roundness of the seat within 0.2 mm, no broken pocket walls, retainers clamp the outer race without cracking the pilot ligament, bearing still turns freely.

### Test 4: assembly

Assemble the full order on the dummy parts with real tools.

**Pass:** every screw can be driven and torqued in the documented order, and the joint can be taken apart again. Record the running gap at the cheek, which needs to stay clear through 0 to 124 degrees by hand, and check the contact patch at both stops with marking compound.

### Test 5: static stop

**Two ways of loading.** The torque wrench is the main case for this test, because it puts torque in at the output the way the RS02 does. It loads the M4 and dowel joint, the outboard cheek and the crown, as well as the stop. The lever only loads the stub and finger, so use it for the external load, drop and side tests.

- **Torque wrench:** 13 mm deep socket on both locked M8 nuts together, through the Ø30 stub bore from the inboard side. With the flats lined up, neither nut can turn against the other, so this works for both stops. Set them up as in the M8 stack in `../prints/test-pass-1/PRINT_ORDER.md`. Apply 17 N·m against each stop. The nuts are dry, held by the 15 N·m jam, so **check the paint line before each test and after every step.** If it has moved, that run doesn't count: relock the nuts and repeat.
- **If the nuts won't hold:** use the wrench in the tightening direction only, and re-mount the fixture for the other stop.
- **Wrench limit: 20 N·m.** Between the nuts the bolt carries the jam preload and the torsion together; at 15 N·m jam and 20 N·m torque it is at about 75% of class 10.9 yield, and at 25 N·m about 90%. Past 20 N·m use the lever, and note that the failure then loads only the stub and finger.
- **Lever:** as below.


Set the sector against the 0 degree stop, then the 124 degree stop (the mechanical travel is 123.4 to 124 degrees). Mount the dummy housing to a rigid vertical board so the elbow axis is horizontal, otherwise a hanging weight gives no elbow torque. Re-mount the board, or correct by the cosine of the lever angle, to load the far stop.

1. Load at 17 N·m for 30 seconds, unload, 10 times at each stop. Measure the stop angle before and after.
2. On the second set, at the stop that looks weaker, add 2 N·m steps from 17 N·m, 30 seconds each, to failure. Record whether it passed 34 N·m.

**Proposed pass:**

- 17 N·m: no cracks, and stop angle change under 0.5 degrees over 10 cycles.
- Failure is at 25.5 N·m or above, which is 1.5 times the design load.
- First failure is in the printed stop features, not at the M3 joint to the housing and not in a thread.

Record the failure torque, which part broke first, the path of the crack, and photos.

### Test 6: stop impact

Mount as test 5, with no weights left on the lever. Use a known mass at a known lever distance, and lift it a measured height above the stop position. Release it onto the stop. Energy is mgh.

| Drop with 2 kg at the lever end | Energy |
| --- | --- |
| 25 mm | 0.5 J |
| 50 mm | 1 J |
| 100 mm | 2 J |
| 200 mm | 3.9 J |

Three drops at each level, inspecting between levels. Stop at failure. Use a fresh set if test 5 went to failure.

**Proposed pass:** 2 J three times at each stop with no cracks and stop angle change under 1 degree. The first failure is in the printed stop features. Record the breaking energy.

### Test 7: side load on the screw joints

Hold the dummy housing and pull the fork sector along the elbow axis through the lever clamp, then apply a side moment with a weight on the lever turned 90 degrees. Build up in steps to failure, on a set that has not been broken.

Start at 100 N along the axis, and at a 19.5 N·m side moment, the load behind the 385 N couple. Increase by 25% steps.

**Proposed pass:** both starting loads held for 30 seconds with no cracks. The printed parts fail, or a screw pulls through the PETG, before a steel nut gives. The fixture has no aluminium thread, so this tests only the printed joint. Record the failure load, and the matching forces at the 6808 and output face, using the 50.6 mm span. These are the numbers to compare with an RS02 limit.

Replace any nut or screw that has been loaded to failure.

## Record

For each test: date, spool, print orientation, part revision, screw torque, room temperature, load steps, failure load, failed part, photos. Put results in `ELBOW_DUMMY_OUTPUT_RESULTS.md` beside this plan.

### Test 8: clamp creep

The hoop and carrier clamp PETG directly, so preload falls as the plastic relaxes. Fit the M3s **dry**, with the DIN 9021 and DIN 6796 stack as on the arm, and tighten to the torque in `../prints/upper-arm/ASSEMBLY_ORDER.md`. Paint-mark each head across to the carrier.

1. Measure how far each screw turns on a retorque at 1 hour, 1 day and 1 week, before any test loads. Re-mark each time. The housing has 100% infill round the screws and under the clamp face, so this measures the joint.
2. Repeat at 1 hour after test 5, the stop cycles.

**Proposed pass:** under about 15 degrees at the 1-day and 1-week retorques, and no witness-mark movement between carrier and housing after the stop cycles. No thread-locker on the rig: its screws stay dry, checked by the marks, as on the arm. Record the turn at each screw. If the marks move, the clamp has slipped.

### Test 9: reuse the bearing

After the break tests, press the 6808 out. Spin it by hand and feel for roughness, notches and play. Only a smooth bearing with no play goes in the arm. A rough one goes in the bin, and the second 6808 takes its place. Check any screw you plan to reuse for bending or thread damage.

## After the tests

| Result | Action |
| --- | --- |
| All pass, printed stop fails first at 25.5 N·m or above | Fix lengths and wiring openings, then fit the forearm. Failing between 25.5 and 34 N·m is a pass. 34 N·m is only recorded |
| Stop fails under 25.5 N·m | Enlarge the finger and recess, or move the weak feature to the carrier, and retest |
| Thread or screw joint fails first | Change the joint before any more strength work |
| Carrier mounting fails first | Rework the carrier; the upper arm hoop may be next in line |
| Layer strength under 22.5 MPa | Rescale all layer SFs and recheck both parts |
| Failure load is high against any RS02 limit later found | Add a designed weak point, or return to a wider span |

## Open before the first print

- Joey to agree the proposed pass levels.
- CAD fixes from the gate, fixture and coupon parts, once SolidWorks is free.
- Screw torques, and the hand mass in the limp drop case.
