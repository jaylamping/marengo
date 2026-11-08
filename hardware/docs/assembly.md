# Assembly

Step-by-step physical build instructions for Marengo.

_(Populate as the design stabilizes.)_

Nothing is written yet. The right 5-DOF bench arm was assembled from the local SolidWorks assembly (`cad/arm/marengo_arm_right_asm`), PETG/PLA printed brackets and Robstride actuators; its geometry is recorded in [`docs/commissioning/right-arm-urdf-from-cad-2026-10-03.md`](../../docs/commissioning/right-arm-urdf-from-cad-2026-10-03.md). Bench mounting, payload and support rules are in [`docs/safety.md`](../../docs/safety.md) and the [limb playbook](../../docs/commissioning/limb-playbook.md).

## Left forearm, option A (draft, not released)

The assembly order, with the insertion and driver-access check for every step, is in [`designs/left-arm/forearm/README.md`](../designs/left-arm/forearm/README.md#assembly-order-and-driver-access-standard-check-every-new-part).

- **Tool: the 6 fork-rim screws (M3 × 14, step 4) need a 2.5 mm ball-end hex key.** The sleeve body is in the way of a straight key, so the key goes in tilted 12–13° outward.
- Every other screw takes a straight key.
- Bolt the RS00 ring and the coupling to the RS00 on the bench before the RS00 goes into the fork. Their screws can't be reached once it is in.
