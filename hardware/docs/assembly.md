# Assembly

Step-by-step physical build instructions for Marengo.

_(Populate as the design stabilizes.)_

Nothing is written yet. The right 5-DOF bench arm was assembled from the local SolidWorks assembly (`cad/arm/marengo_arm_right_asm`), PETG/PLA printed brackets and Robstride actuators; its geometry is recorded in [`docs/commissioning/right-arm-urdf-from-cad-2026-10-03.md`](../../docs/commissioning/right-arm-urdf-from-cad-2026-10-03.md). Bench mounting, payload and support rules are in [`docs/safety.md`](../../docs/safety.md) and the [limb playbook](../../docs/commissioning/limb-playbook.md).

## Left forearm, option A (draft, not released)

The assembly order, with the insertion and driver-access check for every step, is in [`designs/left-arm/forearm/README.md`](../designs/left-arm/forearm/README.md#assembly-order-and-driver-access-standard-check-every-new-part).

- **Tool: the 6 fork-rim screws (M3 × 14, step 4) need a 2.5 mm ball-end hex key.** The sleeve body is in the way of a straight key, so the key goes in tilted 12–13° outward.
- Every other screw takes a straight key.
- Bolt the RS00 ring and the coupling to the RS00 on the bench before the RS00 goes into the fork. Their screws can't be reached once it is in.

## Left upper arm clamshell (draft 1, not released)

Parts and checks: [`designs/left-arm/upper-arm/clamshell/cad/README.md`](../designs/left-arm/upper-arm/clamshell/cad/README.md). The carrier is the released one, unchanged.

1. **Inserts.** Heat-set 8 Ruthex M3 × 5.7:
   - 4 in the upper-half seam posts;
   - 2 in the upper-half hub-lug posts;
   - 2 in the seat ring, from the OD at Y 246.5, one each side.
2. **Ring.** Fit the carrier, ring and elbow RS02 with the carrier screws as before (M3 × 25 cut to 24, DIN 6796 + 9021, 0.35 N·m).
3. **Trunk.** Lay the trunk into the lower half:
   - In through the Ø10 back entry below the hub. Tie it to the boss just inside.
   - Fuses in the upper-half pockets on hook-and-loop.
   - RS02 lead through the ring-top slot.
   - RS00 branch out through the Ø6.8 back-inboard exit at Y 270. Tie it to the boss next to that exit.
4. **Hub.** Set the hub disc's spigot into the lower half, with its lugs on the lug posts.
5. **Close the halves.** Close the upper half over the lower (the pins locate it). Fit, from below:
   - 4 × M3 × 30 at the seam posts;
   - 2 × M3 × 40 at the hub lugs;
   - 2 × M3 × 12 through each half into the ring inserts.
   Snug them evenly. No thread-locker during testing.
6. **Loop clip.**
   - Push the clip's two spigots into the carrier's screw-access bores at 44° and 84° until the foot sits on the carrier face.
   - Lay the loom in the groove and cable-tie it through the tunnel under the floor.
   - The braid ends inside the clip.
   - The upper-arm free length to the forearm clip is 190 mm.
7. **Carrier screw retorque (1 h, 1 day, 1 week).** The clip covers two of the six carrier screws, at 44° and 84°.
   - **Pull it straight off by hand**: the crush ribs give only 0.1 mm interference, so no tool is needed and nothing gets damaged.
   - Cut the cable tie first if the loom is tight.
   - Retorque all six, then push the clip back.

