# Left arm moving interfaces

2026-11-07. Joey's rule: every moving surface runs on a bearing. Gaps below are measured from the current STL meshes in arm coordinates.

**Result:** no joint runs printed-on-printed or printed-on-steel in normal motion, so nothing new needs buying. The 6808 and the actuators' own bearings carry every joint. One seam is tight enough to watch, the 0.5 mm gap at the lower-arm yaw.

| Joint or interface | What carries it | Sliding surfaces in normal motion | Evidence |
| --- | --- | --- | --- |
| Shoulder pitch | RS03 output bearing | None | Actuator stator and output are the only link |
| Shoulder roll | RS03 output bearing | None | As above. Inward roll limit keeps P2 clear of the collar heads: 7.5 mm at roll 0, 2.8 mm at 5 degrees |
| Upper-arm yaw at the hub | Yaw RS02 output bearing | None | The hub bolts to the output, so it turns with it. Arm to P2 and spacer is 32 mm minimum at ±90 |
| Elbow, outboard | Elbow RS02 output bearing | None | Fork bolts to the output |
| Elbow, inboard | 6808 in the carrier, on the fork stub | None | Both races are fixed to their parts. The outer race is clamped by the retainers, the inner race sits on the stub |
| Fork inboard cheek to carrier face | Nothing, it's a gap | None | Cheek at X 139.7, carrier at X 140.5, 0.8 mm. About 0.9 mm on the arm, because the carrier sits 0.09 further out |
| Fork finger to carrier recess | Nothing, it's a gap | None | Finger tip X 146.0, recess floor X 147.5, 1.5 mm |
| Fork finger to hoop screw heads | Nothing, it's a gap | None | At least 0.6 mm at the upper three holes, 6.8 mm at the lower three. Check by hand at assembly |
| Fork to hoop and upper arm | Nothing, it's a gap | None | Fork is at r 59.9 or more over X 151 to 172, the hoop stops at r 42. At full fold, 5.2 mm to the RS00 and 5.9 mm to the fork crown |
| Fork stub to carrier bore | Nothing, it's a gap | None | Stub r 20 against the seat lip r 25 |
| Lower-arm yaw: distal tube on the RS00 | RS00 output bearing | None | Distal tube bolts to the RS00 output |
| Lower-arm yaw: fork skirt to distal tube | Nothing, it's a gap | None, but **watch this one** | End seam, 0.5 mm: tube ends Y 73.76, skirt starts Y 74.26. Radial gap inside is 1.9 mm. Check it at assembly and under a side load |
| Elbow cable loop | Rolls, it doesn't slide | Light contact possible | The loop flexes in free air under the guard. If it rubs the guard, the fix is a smoother guard wall or PTFE tape, not a bearing. Guard is still in draft |
| Lower-arm ribbon clock-spring | Flexes in its groove | Ribbon on printed wall | Designed to wind against the wall. It carries no load. Review when the ribbon is chosen |

## End stops

The fork finger on the carrier stop faces is an end stop, not a running surface. It only touches at 0 and 124 degrees, and it's the intended weak link. The soft limits stay inside it.

## Side load

The 0.8 mm cheek gap holds in normal motion. A hard side load can bend the fork enough to close it. That's contact under overload, not running, and rig test 7 records it. If the rig shows the gap closing in normal motion, the cheapest catalog fix is a PTFE thrust washer on the stub, 40 ID, 0.5 thick.

## Rig

| Interface | What carries it | Notes |
| --- | --- | --- |
| Fork sector in the housing | 6808 on the stub | Same as the arm |
| Dummy output to housing core | M8 bolt in a printed bore, spacer on the housing | **Printed on steel, on purpose.** The rig stands in for the RS02 bearing and only moves slowly, under test. It's a fixture, not a running joint. Grease it |
| Lever | Bolted | No motion |

## Clamped joints

The hoop, carrier and hub joints are clamped PETG and don't move in service. They're checked in `prints/upper-arm/FEA_SUMMARY.md`.
